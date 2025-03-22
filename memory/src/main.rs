#![feature(let_chains, iter_array_chunks, array_chunks, portable_simd, iter_chain)]
use anyhow::{Context, Result};
use comat::cwrite;
use grapher::Grapher;
use std::array;
use std::collections::HashMap;
use std::convert::identity;
use std::fs::read_to_string as read;
use std::io::Write;
use std::io::{stdout, Read};
use std::thread::sleep;
use std::time::Duration;
use termion::color::*;
use termion::cursor::Hide;
use termion::raw::IntoRawMode;
use termion::screen::IntoAlternateScreen;
use termion::{async_stdin, clear, cursor, style};

fn info() -> Result<[u64; 2]> {
    let x = read("/proc/meminfo").context("no!!")?;
    let x = x
        .lines()
        .filter_map(|l| l.split_once(":").map(|(a, b)| (a, b.trim())))
        .collect::<HashMap<_, _>>();
    let [t, f] = [x["MemTotal"], x["MemAvailable"]].map(|x| {
        x.bytes()
            .filter(u8::is_ascii_digit)
            .fold(0, |acc, x| acc * 10 + (x - b'0') as u64)
    });
    return Ok([t, t - f]);
}

fn usage() -> Result<f64> {
    let [t, f] = info()?;
    Ok(f as f64 / t as f64)
}

fn main() -> Result<()> {
    let mut g = Grapher::new()?;
    g.push_point(usage()?);

    let mut d = 0.1;

    let mut stdout = stdout().into_raw_mode()?.into_alternate_screen()?;
    let mut stdin = async_stdin();
    write!(stdout, "{}{}{}", Hide, clear::All, style::Reset).unwrap();
    'out: loop {
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
        let [tot, used] = info()?.map(|x| x as f64 / (1024. * 1024.));
        let tot = tot.round();
        g.draw(|_| None, identity)?;

        write!(g.buffer, "{}{}", White.fg_str(), cursor::Goto(1, 1))?;
        let fps = (1f32 / d).round();
        cwrite!(g.buffer, " {fps}fps ──── {used:.2} / {tot:.2}",)?;
        write!(stdout, "{}", Rgb(0x8d, 0x1a, 0xc5).fg_string())?;
        stdout.write_all(&g.buffer)?;
        stdout.flush()?;

        g.push_point(usage()?);

        sleep(Duration::from_secs_f32(d));
    }
    println!("\x1B[?7l");
    Ok(())
}
