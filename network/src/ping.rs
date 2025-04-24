#![feature(
    generic_arg_infer,
    import_trait_associated_functions,
    string_deref_patterns,
    let_chains,
    iter_array_chunks,
    array_chunks,
    portable_simd,
    iter_chain
)]
use anyhow::*;
use grapher::{truncwrite, Grapher};
use ping_rs::PingError;
use std::array;
use std::io::Write;
use std::io::{stdout, Read};
use std::net::{IpAddr, ToSocketAddrs};
use std::result::Result::Ok as Rok;
use std::thread::sleep;
use std::time::Duration;
use termion::color::*;
use termion::cursor::Hide;
use termion::raw::IntoRawMode;
use termion::screen::IntoAlternateScreen;
use termion::{async_stdin, clear, cursor, style};
fn ping(x: IpAddr) -> Result<u32> {
    match ping_rs::send_ping(&x, Duration::from_secs(60), b"", None) {
        Rok(x) => Ok(x.rtt),
        Err(PingError::TimedOut | PingError::IoPending) => Ok(!0),
        Err(x) => bail!("{x:?}"),
    }
}

fn resolve(host: &str) -> Result<IpAddr> {
    (host, 80)
        .to_socket_addrs()
        .context("resolve")?
        .map(|x| x.ip())
        .next()
        .ok_or(anyhow!("no resolution"))
}

fn main() -> Result<()> {
    let who = std::env::args().nth(1).ok_or(anyhow!("who to ping?"))?;
    let ip = resolve(&who)?;
    eprintln!("pinging {ip}");
    let _ = ping(ip)?;
    let [mut max, mut sum, mut points, mut dropped] = [0; _];
    let mut min = !0;

    let mut g = Grapher::new()?;

    let mut stdout = stdout().into_raw_mode()?.into_alternate_screen()?;
    let mut stdin = async_stdin();
    write!(stdout, "{}{}{}", Hide, clear::All, style::Reset).unwrap();
    'out: loop {
        let mut key = 0;
        while stdin.read(array::from_mut(&mut key)).unwrap() != 0 {
            match key {
                b'q' => break 'out,
                _ => (),
            }
        }
        let r = match ping(ip) {
            Rok(u32::MAX) | Err(_) => {
                dropped += 1;
                sum / points
            }
            Rok(r) => {
                sum += r;
                points += 1;
                min = min.min(r);
                max = max.max(r);
                r
            }
        };
        let avg = sum / points;

        let dat = g.data.iter().copied().filter(|&x| x != 0.0);
        let sum = dat
            .clone()
            .filter(|&x| x != 0.0)
            .map(|x| x - avg as f64)
            .sum::<f64>();
        let dev = (sum / (dat.count()) as f64).abs();

        g.push_point(r as f64);
        g.draw(
            |_| None,
            |x| {
                match max {
                    ..90 => x / 100.0,
                    ..240 => x / 250.0,
                    ..490 => x / 500.0,
                    ..990 => x / 1000.0,
                    _ => x / (max as f64 + 10.0),
                }
                .min(1.0)
            },
        )?;

        write!(g.buffer, "{}{}", White.fg_str(), cursor::Goto(1, 1))?;
        truncwrite!(
            g.buffer,
            " {who} ({ip}) ── last {r}ms ── min {min} avg {avg} max {max} dev ±{dev:.2}ms ── dropped {dropped} /{points}pckts ({:.1}%)",
            (dropped as f64 /points as f64)*100.0,
        )?;
        write!(stdout, "{}", Rgb(40, 185, 119).fg_string())?;
        stdout.write_all(&g.buffer)?;
        stdout.flush()?;

        sleep(Duration::from_millis(r.saturating_sub(100) as u64));
    }
    println!("\x1B[?7l");
    Ok(())
}
