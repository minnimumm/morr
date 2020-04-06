use crossterm::event::Event::Key;
use crossterm::event::KeyCode::{
    Char, Down, Left, PageDown, PageUp, Right, Up,
};
use crossterm::event::{Event, KeyEvent, KeyModifiers};
use crossterm::{cursor, event, queue, style, terminal};
use memmap::Mmap;
use std::env;
use std::fs::File;
use std::io::{stdout, Stdout, Write};
use std::iter;
use std::str;

mod line_reader;

use line_reader::{LineReader, LinesRange};

fn main() {
    let filename = env::args().nth(1).expect("No file name passed");
    let buf = File::open(&filename)
        .and_then(|file| unsafe { Mmap::map(&file) })
        .unwrap();
    let mut screen = ConsoleScreen::init(filename).unwrap();
    let events = iter::repeat_with(event::read).flat_map(|x| x);
    let commands = parse(events);
    let mut line_reader = LineReader::new(&buf);
    let lines = line_reader.read(&LinesRange::pos(0..screen.rows()));
    let mut current_range = lines.range;
    screen.draw(lines.lines);
    for cmd in commands {
        match cmd {
            Command::Quit => break,
            Command::V(vmove) => {
                let new_range = mv(vmove, current_range.clone(), screen.rows());
                let lines = line_reader.read(&new_range);
                let requested_nr = new_range.range.size_hint().0;
                let range = &lines.range;
                let lines = match requested_nr - lines.lines.len() {
                    0 => lines,
                    n => {
                        line_reader.read(&range.extendl(n))
                    }
                };
                if lines.range != current_range {
                    current_range = lines.range;
                    screen.draw(lines.lines)
                }
            }
            _ => {}
        }
    }
}

fn mv<'a>(
    mv: VerticalMove,
    current_line_range: LinesRange,
    rows: usize,
) -> LinesRange {
    match mv {
        VerticalMove::Top => LinesRange::pos(0..rows),
        VerticalMove::Bottom => LinesRange::neg(0..rows),
        VerticalMove::LineUp => current_line_range.shiftl(1),
        VerticalMove::LineDown => current_line_range.shiftr(1),
        VerticalMove::PageUp => current_line_range.shiftl(rows),
        VerticalMove::PageDown => current_line_range.shiftr(rows),
        VerticalMove::HalfPageUp => current_line_range.shiftl(rows / 2),
        VerticalMove::HalfPageDown => current_line_range.shiftr(rows / 2),
    }
}

trait Screen {
    fn rows(&self) -> usize;
    fn cols(&self) -> usize;
    fn draw(&mut self, lines: Vec<&str>);
    fn cleanup(&mut self);
}

struct ConsoleScreen {
    rows: u16,
    cols: u16,
    status: String,
    out: Stdout,
}

impl ConsoleScreen {
    fn init(filename: String) -> Result<Self, Box<dyn std::error::Error>> {
        terminal::enable_raw_mode()?;
        let (cols, rows) = terminal::size()?;
        Ok(ConsoleScreen {
            rows: rows - 1,
            cols,
            status: filename,
            out: stdout(),
        })
    }
}

impl Screen for ConsoleScreen {
    fn draw(&mut self, lines: Vec<&str>) {
        queue!(self.out, terminal::Clear(terminal::ClearType::All))
            .expect("Couldn't clear screen");
        for (i, line) in lines.iter().take(self.rows as usize).enumerate() {
            queue!(self.out, cursor::MoveTo(0, i as u16), style::Print(line))
                .expect("Couldn't move cursor")
        }
        queue!(
            self.out,
            cursor::MoveTo(0, self.rows),
            style::SetAttribute(style::Attribute::Reverse),
            style::Print(&self.status),
            style::ResetColor
        )
        .expect("Couldn't print line");
        self.out.flush().expect("Couldn't flush screen");
    }

    fn rows(&self) -> usize {
        self.rows.clone() as usize
    }

    fn cols(&self) -> usize {
        self.cols.clone() as usize
    }

    fn cleanup(&mut self) {
        queue!(self.out, terminal::Clear(terminal::ClearType::All))
            .expect("Can't clear terminal");
        terminal::disable_raw_mode().expect("Terminal problem");
    }
}

enum VerticalMove {
    Bottom,
    HalfPageDown,
    HalfPageUp,
    LineDown,
    LineUp,
    PageDown,
    PageUp,
    Top,
}

enum HorizontalMove {
    Left,
    Right,
}

enum Command {
    Quit,
    V(VerticalMove),
    H(HorizontalMove),
}

fn parse<I: Iterator<Item = Event>>(
    events: I,
) -> impl Iterator<Item = Command> {
    events.flat_map(|evt| match evt {
        Key(KeyEvent {
            code: Char('q'),
            modifiers: _,
        }) => Some(Command::Quit),
        Key(KeyEvent {
            code: Char('j'),
            modifiers: _,
        }) => Some(Command::V(VerticalMove::LineDown)),
        Key(KeyEvent {
            code: Up,
            modifiers: _,
        }) => Some(Command::V(VerticalMove::LineUp)),
        Key(KeyEvent {
            code: Down,
            modifiers: _,
        }) => Some(Command::V(VerticalMove::LineDown)),
        Key(KeyEvent {
            code: Char('k'),
            modifiers: _,
        }) => Some(Command::V(VerticalMove::LineUp)),
        Key(KeyEvent {
            code: Char('d'),
            modifiers: KeyModifiers::CONTROL,
        }) => Some(Command::V(VerticalMove::HalfPageDown)),
        Key(KeyEvent {
            code: Char('u'),
            modifiers: KeyModifiers::CONTROL,
        }) => Some(Command::V(VerticalMove::HalfPageUp)),
        Key(KeyEvent {
            code: PageDown,
            modifiers: _,
        }) => Some(Command::V(VerticalMove::PageDown)),
        Key(KeyEvent {
            code: PageUp,
            modifiers: _,
        }) => Some(Command::V(VerticalMove::PageUp)),
        Key(KeyEvent {
            code: Char('G'),
            modifiers: _,
        }) => Some(Command::V(VerticalMove::Bottom)),
        Key(KeyEvent {
            code: Char('g'),
            modifiers: _,
        }) => Some(Command::V(VerticalMove::Top)),
        Key(KeyEvent {
            code: Left,
            modifiers: _,
        }) => Some(Command::H(HorizontalMove::Left)),
        Key(KeyEvent {
            code: Right,
            modifiers: _,
        }) => Some(Command::H(HorizontalMove::Right)),
        _ => None,
    })
}
