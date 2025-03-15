#![feature(let_chains, iter_array_chunks, array_chunks, portable_simd, iter_chain)]
use anyhow::Result;
use std::collections::VecDeque;
use std::io::Write;
use std::iter::zip;
use std::simd::prelude::*;
use termion::color::*;
use termion::{clear, cursor};

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

    pub fn draw(&mut self, mut color: impl FnMut(u16) -> [u8; 3]) -> Result<&mut Vec<u8>> {
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
        write!(
            output,
            "\x1B[?7h{}{}{}",
            clear::All,
            Blue.fg_str(),
            cursor::Goto(1, 1)
        )?;
        for y in 0..h as usize {
            let [r, g, b] = color(y as u16);
            write!(output, "{}", Rgb(r, g, b).fg_string())?;

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

fn bl(x: u8) -> [u8; 3] {
    let mut b = [0; 3];
    char::from_u32(0x2800 + x as u32)
        .unwrap()
        .encode_utf8(&mut b);
    b
}
