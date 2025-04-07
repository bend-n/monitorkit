#![feature(portable_simd, string_deref_patterns, duration_millis_float)]
use anyhow::*;
use collar::CollectArray;
use grapher::{Grapher, truncwrite};
use human_bytes::human_bytes;
use std::array;
use std::fs::read_to_string as read;
use std::io::Write;
use std::io::{Read, stdout};
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
    let disk = match std::env::args().nth(1) {
        None => {
            let fs = std::fs::read_dir("/sys/block")?
                .filter_map(Result::ok)
                .filter_map(_.file_name().into_string().ok())
                .collect::<Vec<_>>();
            eprintln!("no args! must be one of {fs:?}");
            exit(1)
        }
        Some(x) => x,
    };
    let write = match std::env::args().nth(2) {
        Some("r") => false,
        Some("w") => true,
        Some(_) | None => {
            eprintln!("requires r/w arg");
            exit(1)
        }
    };
    let drive = std::fs::read_dir("/sys/block")?
        .filter_map(Result::ok)
        .find(_.file_name().to_string_lossy().starts_with(&disk))
        .ok_or(anyhow!("no network"))?
        .path();

    let name = read(drive.join("device").join("model"))?;
    let name = name.trim();

    // https://www.kernel.org/doc/html/latest/block/stat.html
    let ticks = || {
        let [_rio, _rm, rs, _rt, _wio, _wm, ws, _wt] = read(drive.join("stat"))?
            .split_whitespace()
            .filter_map(_.parse::<u32>().ok())
            .collect_array();

        Ok(write.then_some(ws).unwrap_or(rs))
    };

    let mut g = Grapher::new()?;
    let mut d = 0.1;

    let mut stdout = stdout().into_raw_mode()?.into_alternate_screen()?;
    let mut stdin = async_stdin();
    write!(stdout, "{}{}{}", Hide, clear::All, style::Reset).unwrap();
    let mut max = 1f64;
    let mut last = ticks()?;

    let mut i = Instant::now();
    'out: loop {
        sleep(Duration::from_secs_f32(d));

        let pass = i.elapsed().as_millis_f64();
        let new = ticks()?;
        i = Instant::now();

        let δ = (new - last) * 512;
        let r = δ as f64 / pass;
        g.push_point(r);
        max = max.max(r);
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

        g.draw(|_| None, _ / max as f64)?;
        write!(g.buffer, "{}{}", White.fg_str(), cursor::Goto(1, 1))?;

        let fps = (1f32 / d).round();
        truncwrite!(
            g.buffer,
            " {fps}fps ──── {name} ────　ceil {}/s ──── curr {}/s ─ {}",
            human_bytes(max as f64 * 1e3),
            human_bytes(r as f64 * 1e3), // 1e3
            ["read", "write"][write as usize],
        )?;

        write!(stdout, "{}", Rgb(106, 186, 212).fg_string())?;

        stdout.write_all(&g.buffer)?;
        stdout.flush()?;
    }
    println!("\x1B[?7l");
    return Ok(());
}
