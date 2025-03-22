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
use std::array;
use std::convert::identity;
use std::fs::read_to_string as read;
use std::io::Write;
use std::io::{stdout, Read};
use std::path::PathBuf;
use std::result::Result::Ok as Rok;
use std::sync::OnceLock;
use std::thread::sleep;
use std::time::Duration;
use termion::color::*;
use termion::cursor::Hide;
use termion::raw::IntoRawMode;
use termion::screen::IntoAlternateScreen;
use termion::{async_stdin, clear, cursor, style};

fn fetch<U, E: Into<anyhow::Error>>(find: &str, f: impl FnOnce(&str) -> Result<U, E>) -> Result<U> {
    f(read(path.get().unwrap().join("uevent"))?
        .lines()
        .filter_map(|x| x.split_once('='))
        .find(|x| x.0 == find)
        .ok_or(anyhow!("no power??"))?
        .1)
    .map_err(Into::into)
}

static path: OnceLock<PathBuf> = OnceLock::new();
fn main() -> Result<()> {
    let x = std::fs::read_dir("/sys/class/power_supply")?
        .filter_map(Result::ok)
        .find(|x| matches!(read(x.path().join("type")), Rok(x) if x.trim_ascii() == "Battery"))
        .ok_or(anyhow!("batless"))?
        .path();
    path.set(x.clone()).unwrap();
    let manufacturer = fetch("POWER_SUPPLY_MANUFACTURER", |x| Ok(x.to_string()))?;
    let model = fetch("POWER_SUPPLY_MODEL_NAME", |x| Ok(x.to_string()))?;
    let cap = fetch("POWER_SUPPLY_ENERGY_FULL_DESIGN", str::parse::<f64>)? / 1e6;

    let charge = match &*std::env::args().nth(1).unwrap_or("charge".into()) {
        x @ ("charge" | "usage") => x == "charge",
        _ => bail!("arg must be {{usage, charge}}"),
    };

    let mut g = Grapher::new()?;
    let mut d = 1;

    let mut stdout = stdout().into_raw_mode()?.into_alternate_screen()?;
    let mut stdin = async_stdin();
    write!(stdout, "{}{}{}", Hide, clear::All, style::Reset).unwrap();
    let mut second = 0;
    'out: loop {
        let mut key = 0;
        while stdin.read(array::from_mut(&mut key)).unwrap() != 0 {
            match key {
                b'q' => break 'out,
                b'+' => d = (d + 1).min(100),
                b'-' if d > 1 => d -= 1,
                b'-' => d = 1,
                _ => (),
            }
        }

        g.draw(|_| None, identity)?;
        let current = fetch("POWER_SUPPLY_CAPACITY", str::parse::<f64>)?;
        let whs = fetch("POWER_SUPPLY_ENERGY_NOW", str::parse::<f64>)? / 1e6;
        let usage = fetch("POWER_SUPPLY_POWER_NOW", str::parse::<f64>)? / 1e6;

        write!(g.buffer, "{}{}", White.fg_str(), cursor::Goto(1, 1))?;
        truncwrite!(
            g.buffer,
            " {d}s/f ──── {usage:.2}W → ── {current}% {whs:.1}Wh / {cap:.0}Wh ── {manufacturer} {model}"
        )?;
        write!(stdout, "{}", Rgb(40, 185, 119).fg_string())?;
        stdout.write_all(&g.buffer)?;
        stdout.flush()?;
        if second % (d * 2) == 0 {
            g.push_point(if charge { current / 1e2 } else { usage / 40. });
        }

        sleep(Duration::from_millis(500));
        second += 1;
    }
    println!("\x1B[?7l");
    Ok(())
}
