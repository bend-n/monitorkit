#![feature(
    let_chains,
    iter_array_chunks,
    array_chunks,
    portable_simd,
    iter_chain,
    round_char_boundary
)]
use anyhow::Result;
use std::collections::VecDeque;
use std::io::Write;
use std::iter::zip;
use std::simd::prelude::*;
use termion::color::*;
use termion::{clear, cursor};
use unicode_width::*;
pub fn inter([a, b, c]: [f32; 3], [d, e, f]: [f32; 3], fc: f32) -> [f32; 3] {
    [a + (d - a) * fc, b + (e - b) * fc, c + (f - c) * fc]
}

pub struct Grapher {
    pub buffer: Vec<u8>,
    pub data: VecDeque<f64>,
}
impl Grapher {
    pub fn new() -> Result<Self> {
        Ok(Self {
            buffer: Vec::with_capacity(1 << 20),
            data: VecDeque::from(vec![0.0; termion::terminal_size()?.1 as usize * 2]),
        })
    }
    pub fn push_point(&mut self, x: f64) {
        self.data.push_front(x);
        self.data.pop_back();
    }

    pub fn draw(
        &mut self,
        mut color: impl FnMut(u16) -> Option<[u8; 3]>,
        mut mapping: impl FnMut(f64) -> f64,
    ) -> Result<&mut Vec<u8>> {
        let Grapher {
            buffer: output,
            data,
        } = self;
        output.clear();
        let (w, h) = termion::terminal_size()?;
        if w * 2 < data.len() as u16 {
            for _ in 0..data.len() as u16 - w * 2 {
                data.pop_back();
            }
        }
        if w * 2 > data.len() as u16 {
            for _ in 0..w * 2 - data.len() as u16 {
                data.push_back(0.0);
            }
        }
        assert_eq!(data.len(), (w * 2) as usize);

        let string = data
            .iter()
            .rev()
            .array_chunks::<2>()
            .map(|column| {
                let mut data = [vec![false; (h * 4) as usize], vec![false; (h * 4) as usize]];
                for (e, c) in zip(column, &mut data) {
                    let e = mapping(*e);
                    let clen = c.len();
                    c[clen - ((h as f64 * e * 4.0).round().min(h as f64 * 4.0) as usize)..]
                        .fill(true);
                }
                let a = zip(data[0].array_chunks::<4>(), data[1].array_chunks::<4>())
                    .map(|(a, b)| braille([*a, *b]))
                    .collect::<Vec<u8>>();
                assert_eq!(a.len(), h as usize);
                a
            })
            .collect::<Vec<_>>();
        assert_eq!(string.len(), w as usize);
        write!(output, "\x1B[?7h{}{}", clear::All, cursor::Goto(1, 1))?;
        for y in 0..h as usize {
            if let Some([r, g, b]) = color(y as u16) {
                write!(output, "{}", Rgb(r, g, b).fg_string())?;
            }

            for x in 0..w as usize {
                output.extend(bl(string[x][y]));
            }
            if y as u16 != h - 1 {
                output.extend(b"\r\n");
            }
        }
        Ok(output)
    }
}

fn braille(dots: [[bool; 4]; 2]) -> u8 {
    let x = unsafe { dots.as_ptr().cast::<u8x8>().read_unaligned() };
    let x = simd_swizzle!(x, [0, 1, 2, /* */ 4, 5, 6, /* */ 3, 7]);
    x.simd_eq(Simd::splat(1)).to_bitmask() as u8
}
#[inline]
fn bl(x: u8) -> [u8; 3] {
    let mut b = [0; 3];
    char::from_u32(0x2800 + x as u32)
        .unwrap()
        .encode_utf8(&mut b);
    b
}
#[implicit_fn::implicit_fn]
pub fn truncate_to(x: &str, cols: u16) -> String {
    if x.width() < cols as _ {
        return x.to_string();
    }
    let mut i = x.chars().scan(0, |length, x| {
        *length += x.width().unwrap_or(0);
        Some((x, *length))
    });
    use itertools::Itertools;
    let o = i
        .take_while_ref(_.1 < cols as usize)
        .map(_.0)
        .collect::<Vec<char>>();
    o.into_iter()
        .chain(i.next().map(|_| '…'))
        .collect::<String>()
}

pub fn truncate(x: &str) -> anyhow::Result<String> {
    let columns = termion::terminal_size()?.0;
    Ok(truncate_to(x, columns))
}
#[macro_export]
macro_rules! truncwrite {
    ($into:expr, $x:literal $(, $args:expr)* $(,)?) => {
        $into.write(grapher::truncate(&format!($x $(, $args)*))?.as_bytes());
    };
}
