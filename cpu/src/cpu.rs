#![feature(
    let_chains,
    iter_array_chunks,
    array_chunks,
    generic_const_exprs,
    portable_simd,
    iter_chain
)]
use anyhow::*;
use comat::cwrite;
use cpu::*;
use grapher::{truncwrite, Grapher};
use std::array;
use std::convert::identity;
use std::io::Write;
use std::io::{stdout, Read};
use std::thread::sleep;
use std::time::Duration;
use termion::color::*;
use termion::cursor::Hide;
use termion::raw::IntoRawMode;
use termion::screen::IntoAlternateScreen;
use termion::{async_stdin, clear, cursor, style};

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
    let mut t = temps()?;

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

        g.draw(
            |y| {
                Some(
                    inter(
                        [243, 64, 64].map(|x| x as f32 / 255.0),  // red
                        [228, 197, 63].map(|x| x as f32 / 255.0), // yellow
                        y as f32 / (h - 1) as f32,
                    )
                    .map(|x| (x * 255.0) as u8),
                )
            },
            identity,
        )?;

        write!(g.buffer, "{}{}", White.fg_str(), cursor::Goto(1, 1))?;
        let name = &*info.name;
        let speed = speed()?;
        let temp = t.read()?;
        let fps = (1f32 / d).round();
        truncwrite!(
            g.buffer,
            " {fps}fps ──── {name} {core} @ {speed:.2} GHz ──── \u{1b}[0;34;31m{temp:.0}\u{1b}[0m °C",
        )?;
        stdout.write_all(&g.buffer)?;
        stdout.flush()?;

        g.push_point(usage()?.max(0.01));

        sleep(Duration::from_secs_f32(d));
    }
    println!("\x1B[?7l");
    Ok(())
}
