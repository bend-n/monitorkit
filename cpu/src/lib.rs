#![feature(
    let_chains,
    iter_array_chunks,
    array_chunks,
    generic_const_exprs,
    portable_simd,
    iter_chain
)]
use anyhow::*;
use atools::prelude::*;
use collar::CollectArray;
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

pub struct Temps(File);
impl Temps {
    pub fn load() -> Result<Self> {
        for hwmon in std::fs::read_dir("/sys/class/hwmon/")?.filter_map(Result::ok) {
            if !read(Path::join(&hwmon.path(), "name"))?.starts_with("coretemp") {
                continue;
            }
            for f in std::fs::read_dir(hwmon.path())?.filter_map(Result::ok) {
                if let Some(n) = f.file_name().to_str()
                    && n.starts_with("temp")
                    && n.ends_with("input")
                {
                    let i = n
                        .bytes()
                        .filter(u8::is_ascii_digit)
                        .fold(0, |acc, x| acc * 10 + (x - b'0') as u64);

                    let name = read(Path::join(&hwmon.path(), format!("temp{i}_label")))
                        .context("alas")?;
                    if let Some(c) = core() {
                        if !name.contains(&c.to_string()) {
                            continue;
                        }
                    } else if !name.contains("Package") {
                        continue;
                    }

                    let f = File::open(f.path())?;
                    return Ok(Self(f));
                }
            }
        }
        bail!("h")
    }
    pub fn read(&mut self) -> Result<f64> {
        let mut o = String::default();
        self.0.seek(std::io::SeekFrom::Start(0))?;
        self.0.read_to_string(&mut o)?;
        Ok(o.trim().parse::<f64>().context("reading temps")? / 1000.0)
    }
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
        let name = x["Model name"]
            .replace("Core", "")
            .replace("(TM)", "")
            .replace("Intel", "")
            .replace("(R)", "");
        let name = regex::Regex::new(r"@ [0-9]+\.[0-9]+GHz")
            .unwrap()
            .replace(&name, "");
        let name = regex::Regex::new(r"[0-9]+th Gen")
            .unwrap()
            .replace(&name, "")
            .replace("  ", " ")
            .trim()
            .replace("  ", " ")
            .to_lowercase();
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
