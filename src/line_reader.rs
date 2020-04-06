use memchr::{memchr_iter, Memchr};
use std::cmp::min;
use std::iter;
use std::ops::Range;
use std::str;

#[derive(Debug, Clone, PartialEq)]
pub enum Sign {
    Pos,
    Neg,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinesRange {
    pub sign: Sign,
    pub range: Range<usize>,
}

fn shiftr(range: &Range<usize>, by: usize) -> Range<usize> {
    range.start + by..range.end + by
}

fn shiftl(range: &Range<usize>, by: usize) -> Range<usize> {
    let s = range.start.checked_sub(by).unwrap_or(0);
    let diff = range.start - s;
    s..range.end - diff
}

fn extendr(range: &Range<usize>, by: usize) -> Range<usize> {
    range.start..range.end + by
}

fn extendl(range: &Range<usize>, by: usize) -> Range<usize> {
    range.start.checked_sub(by).unwrap_or(0)..range.end
}

impl LinesRange {
    pub fn pos(range: Range<usize>) -> Self {
        Self {
            sign: Sign::Pos,
            range,
        }
    }

    pub fn neg(range: Range<usize>) -> Self {
        Self {
            sign: Sign::Neg,
            range,
        }
    }

    pub fn shiftl(&self, by: usize) -> LinesRange {
        match self.sign {
            Sign::Pos => LinesRange::pos(shiftl(&self.range, by)),
            Sign::Neg => LinesRange::neg(shiftr(&self.range, by)),
        }
    }

    pub fn shiftr(&self, by: usize) -> LinesRange {
        match self.sign {
            Sign::Pos => LinesRange::pos(shiftr(&self.range, by)),
            Sign::Neg => LinesRange::neg(shiftl(&self.range, by)),
        }
    }

    pub fn extendl(&self, by: usize) -> LinesRange {
        match self.sign {
            Sign::Pos => LinesRange::pos(extendl(&self.range, by)),
            Sign::Neg => LinesRange::neg(extendr(&self.range, by)),
        }
    }

    #[allow(unused)]
    pub fn extendr(&self, by: usize) -> LinesRange {
        match self.sign {
            Sign::Pos => LinesRange::pos(extendr(&self.range, by)),
            Sign::Neg => LinesRange::neg(extendl(&self.range, by)),
        }
    }

}

fn inverted(range: &Range<usize>, len: usize) -> Range<usize> {
    let start = len.checked_sub(range.end).unwrap_or(0);
    let end = len.checked_sub(range.start).unwrap_or(0);
    start..end
}

fn limit(range: &Range<usize>, to: usize) -> Range<usize> {
    min(to.checked_sub(1).unwrap_or(0), range.start)..min(to, range.end)
}

pub struct ReadLines<'a> {
    pub range: LinesRange,
    pub lines: Vec<&'a str>,
    pub buf_range: Range<usize>,
}

type Eols<'a> =
    iter::Chain<iter::Chain<iter::Once<usize>, Memchr<'a>>, iter::Once<usize>>;

pub struct LineReader<'a> {
    eols_forw: Vec<usize>,
    eols_back: Vec<usize>,
    eols_iter: Eols<'a>,
    buf: &'a [u8],
    full: bool,
}

impl<'a> LineReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        let it = iter::once(usize::max_value())
            .chain(memchr_iter(b'\n', &buf[..]))
            .chain(iter::once(buf.len()));
        LineReader {
            eols_forw: vec![],
            eols_back: vec![],
            eols_iter: it,
            buf: buf,
            full: false,
        }
    }

    fn extend<I: Iterator<Item = usize>>(
        eols: &mut Vec<usize>,
        it: &mut I,
        last_requested_line: usize,
        stop: usize,
    ) -> bool {
        if let Some(n) = (last_requested_line + 1).checked_sub(eols.len()) {
            for _ in 0..n {
                if let Some(idx) = it.next() {
                    eols.push(idx);
                } else {
                    return true;
                }
            }
            return eols.last().filter(|&&i| i == stop).is_some();
        }
        false
    }

    pub fn read(&mut self, range: &LinesRange) -> ReadLines {
        match range.sign {
            Sign::Pos => self.read_forw(&range.range),
            Sign::Neg => self.read_back(&range.range),
        }
    }

    fn read_forw(&mut self, range: &Range<usize>) -> ReadLines {
        if !self.full
            && Self::extend(
                &mut self.eols_forw,
                &mut self.eols_iter,
                range.end,
                self.buf.len(),
            )
        {
            self.eols_forw.extend(self.eols_back.iter().rev());
            self.eols_back.clear();
            self.full = true;
        }
        let available_lines = self.eols_forw.len().checked_sub(1).unwrap_or(0);
        let range = limit(range, available_lines);
        let slice = &self.eols_forw[range.start..range.end + 1];
        let s = slice.first().unwrap_or(&0).clone();
        let e = slice.last().unwrap_or(&0).clone();
        ReadLines {
            range: LinesRange::pos(range),
            buf_range: s..e,
            lines: self.lines(slice),
        }
    }

    fn read_back(&mut self, range: &Range<usize>) -> ReadLines {
        if self.full
            || Self::extend(
                &mut self.eols_back,
                &mut (&mut self.eols_iter).rev(),
                range.end,
                self.buf.len(),
            )
        {
            self.eols_forw.extend(self.eols_back.iter().rev());
            self.eols_back.clear();
            self.full = true;
            return self.read_forw(&inverted(range, self.eols_forw.len() - 1));
        }
        let available_lines = self.eols_back.len().checked_sub(1).unwrap_or(0);
        let range = limit(range, available_lines);
        let mut requested_eols = vec![];
        let slice = &self.eols_back[range.start..range.end + 1];
        let s = slice.first().unwrap_or(&0).clone();
        let e = slice.last().unwrap_or(&0).clone();
        requested_eols.extend(slice.iter().rev());
        ReadLines {
            range: LinesRange::neg(range),
            buf_range: s..e,
            lines: self.lines(&requested_eols),
        }
    }

    fn lines(&self, requested_eols: &[usize]) -> Vec<&'a str> {
        requested_eols
            .windows(2)
            .map(|p| p[0].overflowing_add(1).0..p[1])
            .map(|range| &self.buf[range])
            .map(str::from_utf8)
            .map(Result::unwrap)
            .collect()
    }
}
