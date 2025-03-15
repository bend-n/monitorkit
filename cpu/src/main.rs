#![feature(
    let_chains,
    iter_array_chunks,
    array_chunks,
    generic_const_exprs,
    portable_simd,
    iter_chain
)]
use anyhow::{anyhow, bail, ensure, Context, Result};
use atools::prelude::*;
use collar::CollectArray;
use comat::cwrite;
use grapher::Grapher;
use parking_lot::Mutex;
use std::array;
use std::collections::HashMap;
use std::fmt::Display;
use std::fs::{read_to_string as read, File};
use std::io::{stdout, Read};
use std::io::{Seek, Write};
use std::mem::replace;
use std::path::Path;
use std::sync::OnceLock;
use std::thread::sleep;
use std::time::Duration;
use termion::color::*;
use termion::cursor::Hide;
use termion::raw::IntoRawMode;
use termion::screen::IntoAlternateScreen;
use termion::{async_stdin, clear, cursor, style};

#[derive(Copy, Clone, Debug)]
enum ViewCore {
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

static CORE: OnceLock<ViewCore> = OnceLock::new();

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CpuInfo {
    pub name: String,
    pub speed: f64,
    pub count: u64,
}

struct Temps(File);
impl Temps {
    fn load() -> Result<Self> {
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
    fn read(&mut self) -> Result<f32> {
        let mut o = String::default();
        self.0.seek(std::io::SeekFrom::Start(0))?;
        self.0.read_to_string(&mut o)?;
        Ok(o.trim().parse::<f32>().context("reading temps")? / 1000.0)
    }
}

impl CpuInfo {
    fn read() -> Result<Self> {
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

fn sped(core: u64) -> Result<f64> {
    Ok(read(format!(
        "/sys/devices/system/cpu/cpu{core}/cpufreq/scaling_cur_freq"
    ))
    .context("speed")?
    .replace('\n', "")
    .parse::<u64>()
    .context("reading speeds")? as f64
        / 1e6)
}

fn speed() -> Result<f64> {
    Ok(match *CORE.get().unwrap() {
        ViewCore::All(x) => (0..x).map(sped).sum::<Result<f64, _>>()? / x as f64,
        ViewCore::One(x) => sped(x)?,
    })
}

fn core() -> Option<u64> {
    CORE.get().copied().and_then(|x| match x {
        ViewCore::One(x) => Some(x),
        _ => None,
    })
}

fn usage() -> Result<f64> {
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

fn main() -> Result<()> {
    fn inter([a, b, c]: [f32; 3], [d, e, f]: [f32; 3], fc: f32) -> [f32; 3] {
        [a + (d - a) * fc, b + (e - b) * fc, c + (f - c) * fc]
    }

    let info = CpuInfo::read()?;
    let core = std::env::args()
        .nth(1)
        .and_then(|x| x.parse::<u64>().ok())
        .map_or(ViewCore::All(info.count), ViewCore::One);
    CORE.set(core).unwrap();
    match core {
        ViewCore::One(x) => ensure!(x < info.count, "not enough cores"),
        _ => (),
    }
    let mut t = Temps::load()?;

    let mut g = Grapher::new()?;
    g.push_point(usage()?.max(0.01));

    let mut d = 0.1;

    let mut stdout = stdout().into_raw_mode()?.into_alternate_screen()?;
    let mut stdin = async_stdin();
    write!(stdout, "{}{}{}", Hide, clear::All, style::Reset).unwrap();

    'out: loop {
        let (_, h) = termion::terminal_size()?;

        let mut key = 0;
        while stdin.read(array::from_mut(&mut key)).unwrap() != 0 {
            match key {
                b'q' => break 'out,
                b'+' => d = (d + 0.1f32).min(1.0),
                b'-' if d >= 0.2 => d -= 0.1,
                b'-' if d >= 0.02 => d -= 0.01,
                b'-' => d = 0.00833,
                _ => (),
            }
        }

        g.draw(|y| {
            inter(
                [243, 64, 64].map(|x| x as f32 / 255.0),  // red
                [228, 197, 63].map(|x| x as f32 / 255.0), // yellow
                y as f32 / (h - 1) as f32,
            )
            .map(|x| (x * 255.0) as u8)
        })?;

        write!(g.buffer, "{}{}", White.fg_str(), cursor::Goto(1, 1))?;
        let name = &*info.name;
        let speed = speed()?;
        let temp = t.read()?;
        let fps = (1f32 / d).round();
        cwrite!(
            g.buffer,
            " {fps}fps ──── {name} {core} @ {speed:.2} GHz ──── {red}{temp}{reset} °C",
        )?;
        stdout.write_all(&g.buffer)?;
        stdout.flush()?;

        g.push_point(usage()?.max(0.01));

        sleep(Duration::from_secs_f32(d));
    }
    println!("\x1B[?7l");
    Ok(())
}
