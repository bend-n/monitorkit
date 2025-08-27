#![feature(import_trait_associated_functions, iter_array_chunks, portable_simd)]
use anyhow::*;
use grapher::inter;
use grapher::truncwrite;
use grapher::Grapher;
use nvml_wrapper::{
    enum_wrappers::device::{Clock, TemperatureSensor},
    Nvml,
};
use pretty_bytes::converter::convert;
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
#[implicit_fn::implicit_fn]
fn main() -> Result<()> {
    let vram = std::env::args().nth(1).is_some();
    let nvml = Nvml::init().context("nvml(nvidia) less")?;
    let device = nvml.device_by_index(0).context("nvidialess")?;
    let name = device.name()?;
    let name = name.replace("NVIDIA GeForce RTX", "RTX");

    let architecture = device.architecture()?;
    let mut g = Grapher::new()?;
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
                        [155, 221, 58].map(_ as f32 / 255.0),
                        [118, 185, 0].map(_ as f32 / 255.0),
                        y as f32 / (h - 1) as f32,
                    )
                    .map(|x| (x * 255.0) as u8),
                )
            },
            identity,
        )?;

        write!(g.buffer, "{}{}", White.fg_str(), cursor::Goto(1, 1))?;

        let temperature = device.temperature(TemperatureSensor::Gpu)?;
        let max = device.temperature_threshold(
            nvml_wrapper::enum_wrappers::device::TemperatureThreshold::Slowdown,
        )?;

        let mem_info = device.memory_info()?;
        let util = device.utilization_rates()?.gpu;

        let graphics_clock = device.clock_info(Clock::Graphics)?;
        let mem_clock = device.clock_info(Clock::Memory)?;
        let usage = device.power_usage()? / 1000;
        let of = device.power_management_limit()? / 1000;

        let used_mem = convert(mem_info.used as _);
        let total_mem = convert(mem_info.total as _);
        if vram {
            truncwrite!(
                g.buffer,
                "{name} {used_mem}/ {total_mem} vRAM speed {mem_clock} MHZ"
            )?;
        } else {
            truncwrite!(g.buffer, "{name} ({architecture}) @ {temperature}°C (max {max}°C) {usage}W/ {of}W {util}% usage\n\r")?;
            truncwrite!(g.buffer, "{graphics_clock} MHz mem {mem_clock} MHz\n\r")?;
            truncwrite!(g.buffer, "{used_mem}/ {total_mem} vRAM\n\r")?;
        }
        stdout.write_all(&g.buffer)?;
        stdout.flush()?;

        sleep(Duration::from_secs_f32(d));
        if vram {
            g.push_point(mem_info.used as f64 / mem_info.total as f64);
        } else {
            g.push_point(util as f64 / 1e2);
        }
    }
    println!("\x1B[?7l");

    Ok(())
}
