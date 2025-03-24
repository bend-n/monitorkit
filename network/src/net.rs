#![feature(portable_simd, string_deref_patterns)]
use anyhow::*;
use grapher::{truncwrite, Grapher};
use std::array;
use std::fs::read_to_string as read;
use std::io::Write;
use std::io::{stdout, Read};
use std::ops::Not;
use std::os::unix::ffi::OsStrExt;
use std::process::exit;
use std::thread::sleep;
use std::time::{Duration, Instant};
use termion::color::*;
use termion::cursor::Hide;
use termion::raw::IntoRawMode;
use termion::screen::IntoAlternateScreen;
use termion::{async_stdin, clear, cursor, style};
#[implicit_fn::implicit_fn]
fn main() -> Result<()> {
    let up = match std::env::args().nth(1) {
        None => {
            eprintln!("no args!");
            exit(1)
        }
        Some("dl" | "download" | "d" | "down") => false,
        Some("up" | "upload") => true,
        Some(x) => {
            println!("{x} is not an arg, requires net");
            exit(1)
        }
    };
    let file = std::fs::read_dir("/sys/class/net/")?
        .filter_map(Result::ok)
        .find(|x| x.file_name().as_bytes().starts_with(b"lo").not())
        .ok_or(anyhow!("no network"))?;

    let pciid = read(file.path().join("device").join("uevent"))?
        .lines()
        .find(|x| x.starts_with("PCI_ID"))
        .ok_or(anyhow!("eh"))?
        .split_once("=")
        .unwrap()
        .1
        .split_once(":")
        .unwrap()
        .1
        .to_lowercase();

    let name = read("/usr/share/hwdata/pci.ids")?
        .lines()
        .filter(|x| x.starts_with('\t'))
        .find(|x| x[1..].starts_with(&pciid))
        .map(|x| x[1 + pciid.len()..].trim().to_string())
        .unwrap_or("default".into());
    let bytes = || {
        [
            read(file.path().join("statistics").join("rx_bytes"))
                .unwrap()
                .trim()
                .parse::<u64>()
                .unwrap(),
            read(file.path().join("statistics").join("tx_bytes"))
                .unwrap()
                .trim()
                .parse::<u64>()
                .unwrap(),
        ][up as usize]
    };

    let mut last = bytes();

    let mut g = Grapher::new()?;
    let mut d = 0.1;

    let mut stdout = stdout().into_raw_mode()?.into_alternate_screen()?;
    let mut stdin = async_stdin();
    write!(stdout, "{}{}{}", Hide, clear::All, style::Reset).unwrap();
    let mut max = 1e6f64;

    let mut i = Instant::now();
    'out: loop {
        sleep(Duration::from_secs_f32(d));

        let pass = i.elapsed().as_secs_f64();
        let new = bytes();
        i = Instant::now();
        let δ = (new - last) as f64 / pass;
        g.push_point(δ);
        max = max.max(δ);
        last = new;

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

        g.draw(|_| None, _ / max)?;
        write!(g.buffer, "{}{}", White.fg_str(), cursor::Goto(1, 1))?;

        let fps = (1f32 / d).round();
        truncwrite!(
            g.buffer,
            " {fps}fps ──── {name} ────　ceil {:.2}mb/s ──── curr {:.2}/s ─ {}",
            max / 1e6,
            δ / 1e6,
            ["rx", "tx"][up as usize],
        )?;

        write!(stdout, "{}", Rgb(106, 186, 212).fg_string())?;

        stdout.write_all(&g.buffer)?;
        stdout.flush()?;
    }
    println!("\x1B[?7l");
    return Ok(());
}
