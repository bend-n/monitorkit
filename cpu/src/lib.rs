#![feature(iter_array_chunks, generic_const_exprs, portable_simd)]
use anyhow::*;
use atools::prelude::*;
use collar::CollectArray;
use hwmons::{hwmons, Hwmon};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::fmt::Display;
use std::fs::{read_to_string as read, File};
use std::io::Read;
use std::io::Seek;
use std::mem::replace;
use std::path::Path;
use std::sync::OnceLock;

#[derive(Copy, Clone, Debug)]
pub enum ViewCore {
    All(u64),
    One(u64),
}
impl Display for ViewCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewCore::All(x) => write!(f, "..#{x}"),
            ViewCore::One(x) => write!(f, "#{x}"),
        }
    }
}

pub static CORE: OnceLock<ViewCore> = OnceLock::new();

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CpuInfo {
    pub name: String,
    pub speed: f64,
    pub count: u64,
}

#[implicit_fn::implicit_fn]
pub fn temps() -> Result<Hwmon> {
    if let x @ Some(_) = hwmons().find(_.contains("Package")) {
        if let Some(c) = core() {
            hwmons().find(_.contains(&c.to_string()))
        } else {
            x
        }
        .ok_or(anyhow!("no intel hwmon"))?
    } else {
        hwmons()
            .find(_.contains("Tccd"))
            .ok_or(anyhow!("no amd hwmon"))?
    }
    .load()
}

impl CpuInfo {
    pub fn read() -> Result<Self> {
        let x = String::from_utf8(
            std::process::Command::new("lscpu")
                .env("LC_ALL", "C")
                .output()
                .context("lscpuless")?
                .stdout,
        )
        .context("unable to parse lscpu output to UTF-8")?;
        let x = x
            .lines()
            .filter_map(|l| l.split_once(":").map(|(a, b)| (a, b.trim())))
            .collect::<HashMap<_, _>>();
        let name = x["Model name"];
        let name = if name.contains("Intel") {
            let name = name
                .replace("Core", "")
                .replace("(TM)", "")
                .replace("Intel", "")
                .replace("(R)", "");
            let name = regex::Regex::new(r"[0-9]+th Gen")
                .unwrap()
                .replace(&name, "")
                .replace("  ", " ")
                .trim()
                .replace("  ", " ");
            regex::Regex::new(r"@ [0-9]+\.[0-9]+GHz")
                .unwrap()
                .replace(&name, "")
                .to_lowercase()
        } else {
            name.replace("AMD Ryzen", "Ryzen")
                .replace("Processor", "")
                .trim()
                .to_owned()
        };

        Ok(Self {
            name: name,
            speed: x["CPU max MHz"].parse::<f64>().context("cpu mhz??")? / 1000.0,
            count: x["CPU(s)"].parse()?,
        })
    }
}

pub fn sped(core: u64) -> Result<f64> {
    Ok(read(format!(
        "/sys/devices/system/cpu/cpu{core}/cpufreq/scaling_cur_freq"
    ))
    .context("speed")?
    .replace('\n', "")
    .parse::<u64>()
    .context("reading speeds")? as f64
        / 1e6)
}

pub fn speed() -> Result<f64> {
    Ok(match *CORE.get().unwrap() {
        ViewCore::All(x) => (0..x).map(sped).sum::<Result<f64, _>>()? / x as f64,
        ViewCore::One(x) => sped(x)?,
    })
}

pub fn core() -> Option<u64> {
    CORE.get().copied().and_then(|x| match x {
        ViewCore::One(x) => Some(x),
        _ => None,
    })
}

pub fn usage() -> Result<f64> {
    let x = read("/proc/stat")?;
    let x = x
        .lines()
        .nth(core().map_or(0, |x| x as usize + 1))
        .ok_or(anyhow!("no procstat"))?;

    // https://www.linuxhowtos.org/System/procstat.htm
    let x @ [_user, _nice, _system, idle, iowait, _irq, _softirq] = x
        .split_whitespace()
        .skip(1)
        .map(|x| x.parse::<u64>())
        .try_collect_array()?;

    static LAST: Mutex<[u64; 2]> = Mutex::new([0; 2]);
    let [pi, pt] = &mut *LAST.lock();

    let idle = idle + iowait;
    let tot = x.sum();

    let idle = (idle - replace(pi, idle)) as f64;
    let tot = (tot - replace(pt, tot)) as f64;
    let r = (tot - idle) / tot;
    Ok((r == r).then_some(r).unwrap_or(0.0))
}
