use anyhow::*;
use std::ffi::OsStr;
use std::fs::{File, read_to_string as read};
use std::io::{Read, Seek};
use std::ops::Deref;
use std::path::PathBuf;
#[derive(Debug)]
pub struct Hwmon(String, PathBuf, Option<File>);
impl Deref for Hwmon {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Hwmon {
    pub fn load(self) -> Result<Self> {
        File::open(&self.1)
            .map(|x| Self(self.0, self.1, Some(x)))
            .context("couldnt load hwmon")
    }
    #[implicit_fn::implicit_fn]
    pub fn read(&mut self) -> Result<f64> {
        let mut o = String::default();
        let f = self.2.as_mut().unwrap();
        f.seek(std::io::SeekFrom::Start(0))?;
        f.read_to_string(&mut o)?;
        o.trim()
            .parse::<f64>()
            .map(_ / 1000.0)
            .context("parsing hwmon")
    }
}
#[implicit_fn::implicit_fn]
pub fn hwmons() -> impl Iterator<Item = Hwmon> {
    std::fs::read_dir("/sys/class/hwmon/").into_iter().flat_map(
        _.into_iter()
            .filter_map(Result::ok)
            .map(_.path())
            .map(std::fs::read_dir)
            .filter_map(Result::ok)
            .flatten()
            .filter_map(Result::ok)
            .map(_.path())
            .filter(
                _.file_name()
                    .and_then(OsStr::to_str)
                    .is_some_and(|x| x.starts_with("temp") & x.ends_with("input")),
            )
            .filter_map(|f| {
                Some(Hwmon(
                    read(f.parent()?.join(format!("temp{}_label",
                    f.file_name()?.to_str()?.bytes().skip(4)
                        .take_while(u8::is_ascii_digit)
                        .fold(0, |acc, x| acc * 10 + (x - b'0') as u64))))
                    .ok()?
                    .trim()
                    .to_string(),
                    f,
                    None,
                ))
            }),
    )
}

#[test]
fn x() {
    dbg!(hwmons().collect::<Vec<_>>());
}
