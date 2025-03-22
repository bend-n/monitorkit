#![feature(
    generic_arg_infer,
    import_trait_associated_functions,
    string_deref_patterns,
    deref_patterns,
    let_chains,
    iter_array_chunks,
    array_chunks,
    generic_const_exprs,
    portable_simd,
    iter_chain
)]
use anyhow::*;
use collar::CollectArray as _;
use comat::cwrite;
use grapher::Grapher;
use parking_lot::Mutex;
use std::array;
use std::convert::identity;
use std::ffi::{c_char, CStr};
use std::fmt::Display;
use std::fs::{read_dir, read_to_string as read};
use std::io::{stdout, Read};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;
use termion::color::*;
use termion::cursor::Hide;
use termion::raw::IntoRawMode;
use termion::screen::IntoAlternateScreen;
use termion::{async_stdin, clear, cursor, style};

#[repr(C)]
#[derive(Default)]
struct igt_devices_print_format {
    ty: u32,
    option: u32,
    numeric: bool,
    codename: bool,
}
#[repr(C)]
struct strong<const N: usize>([c_char; N]);
impl<const N: usize> Default for strong<N> {
    fn default() -> Self {
        Self([0; N])
    }
}
impl<const N: usize> Display for strong<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", unsafe {
            CStr::from_ptr(self.0.as_ptr()).to_str().unwrap()
        })
    }
}

#[repr(C)]
#[derive(Default)]
struct igt_device_card {
    subsystem: strong<256>,
    card: strong<256>,
    render: strong<256>,
    pci_slot_name: strong<13>,
    pci_vendor: u16,
    pci_device: u16,
}

unsafe extern "C" {
    fn igt_devices_scan();
    fn igt_devices_print(opts: *const igt_devices_print_format);
    fn igt_device_find_integrated_card(card: *mut igt_device_card) -> bool;
    fn igt_device_get_pretty_name(card: *const igt_device_card, numeric: bool) -> *const c_char;
}

fn uh() -> Result<()> {
    dbg!("???");
    let x = std::fs::read_dir("/sys/devices")?
        .filter_map(Result::ok)
        .filter(|x| {
            x.file_name()
                .into_string()
                .is_ok_and(|x| x.starts_with("pci"))
        })
        .filter_map(|x| read_dir(x.path()).ok())
        .flatten()
        .filter_map(Result::ok)
        .find(|x| read(x.path().join("uevent")).is_ok_and(|x| x.contains("i915")))
        .ok_or(anyhow!("intelless"))?
        .path();

    // println!("{:?}", x.location().unwrap());
    let pciid = read(x.join("uevent"))
        .unwrap()
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
        .unwrap();
    dbg!(name);
    Ok(())
}
fn main() -> Result<()> {
    use atools::Join;

    let mut c = Command::new("intel_gpu_top")
        .args(["-s", "100", "-c"])
        .stdout(Stdio::piped())
        .stdin(Stdio::inherit())
        .spawn()?;
    let r = c.stdout.take().unwrap();
    static last: Mutex<[f64; 7]> = Mutex::new([f64::NAN; _]);
    std::thread::spawn(move || {
        // Freq MHz req,Freq MHz act,IRQ /s,RC6 %,Power W gpu,Power W pkg,RCS %,RCS se,RCS wa,BCS %,BCS se,BCS wa,VCS %,VCS se,VCS wa,VECS %,VECS se,VECS wa
        for line in BufReader::new(r).lines().skip(1) {
            let line = line.unwrap();
            let mut splat = line.split(",").map(|x| x.parse::<f64>().unwrap());
            let x = splat.collect_array::<6>();
            let busy = splat.array_chunks::<3>().map(|x| x[0]).sum::<f64>();
            *last.lock() = x.join(busy / 100.);
        }
    });

    // [f_req, f_act, irq, rc6, power_gpu, power_package, busy]
    unsafe { igt_devices_scan() };
    let mut card = igt_device_card::default();
    let ret = unsafe { igt_device_find_integrated_card(&mut card) };
    if !ret {
        bail!("igpuless!! (i dont have an arc gpu it should be simple to add that tho)")
    }
    let name = unsafe {
        CStr::from_ptr(igt_device_get_pretty_name(&card, false))
            .to_str()
            .unwrap()
    };

    fn inter([a, b, c]: [f32; 3], [d, e, f]: [f32; 3], fc: f32) -> [f32; 3] {
        [a + (d - a) * fc, b + (e - b) * fc, c + (f - c) * fc]
    }

    let mut g = Grapher::new()?;
    sleep(Duration::from_secs_f32(0.5));
    if let std::result::Result::Ok(Some(_)) = c.try_wait() {
        return Ok(());
    }
    let d = 0.1;
    let mut stdout = stdout().into_raw_mode()?.into_alternate_screen()?;
    let mut stdin = async_stdin();
    write!(stdout, "{}{}{}", Hide, clear::All, style::Reset).unwrap();
    'out: loop {
        if let std::result::Result::Ok(Some(_)) = c.try_wait() {
            return Ok(());
        }
        let (_, h) = termion::terminal_size()?;

        let mut key = 0;
        while stdin.read(array::from_mut(&mut key)).unwrap() != 0 {
            match key {
                b'q' => break 'out,

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
        let [f_req, f_act, irq, _rc6, power_gpu, power_package, busy] = *last.lock();
        cwrite!(
            g.buffer,
            " {name} @ {f_req:.0}/ {f_act:.0} MHz ─── {power_gpu:.1}/ {power_package:.1} W ─── {irq:.0} irqs/s"
        )?;
        stdout.write_all(&g.buffer)?;
        stdout.flush()?;

        sleep(Duration::from_secs_f32(d));
        g.push_point(busy);
    }
    println!("\x1B[?7l");
    Ok(())
}
