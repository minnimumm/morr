use crossterm::event;
use crossterm::event::Event::Key;
use crossterm::event::KeyCode::{
    Char, Down, Left, PageDown, PageUp, Right, Up,
};
use crossterm::event::{Event, KeyEvent, KeyModifiers};
use memmap::Mmap;
use std::env;
use std::fs::File;
use std::iter;

mod line_reader;
mod screen;

use line_reader::{Buffer, LinesRange};
use screen::{Rect, Screen};

fn main() -> Result<(), MorrError> {
    let filename = env::args().nth(1).expect("No file name passed");
    let buf = File::open(&filename)
        .and_then(|file| unsafe { Mmap::map(&file) })
        .unwrap();
    let header_height = 3;
    let status = 1;
    let mut screen = screen::ConsoleScreen::init(header_height, status)?;
    let mut buffer = Buffer::new(&buf);
    let max_lines =
        screen.content_bounds().height() + screen.header_bounds().height();
    let lines = buffer.read(&LinesRange::pos(0..max_lines as usize));
    let (header, rest) = &lines.lines.split_at(header_height as usize);
    screen.draw_header(header)?;
    let initial_mode = Mode::Viewing {
        current_range: lines.range,
    };
    screen.draw_content(rest)?;
    screen.draw_status(&filename)?;
    let header_end: usize = header.iter().map(|line| line.len() + 1).sum();
    let buffer = Buffer::new(&buf[header_end..]);
    let mut state = State::new(initial_mode, header, screen, buffer);
    state.run(iter::repeat_with(event::read).flatten())
}

#[allow(unused)]
enum Direction {
    Forward,
    Backward,
}

#[allow(unused)]
struct Search {
    direction: Direction,
    search_string: String,
}

enum Mode {
    #[allow(unused)]
    Searching {
        search: Search,
    },
    Viewing {
        current_range: LinesRange,
    },
    #[allow(unused)]
    Command {
        args: Vec<String>,
    },
}

impl Mode {
    fn consume<I, S>(
        &mut self,
        events: &mut I,
        screen: &mut S,
        buf: &mut Buffer,
    ) -> Option<Mode>
    where
        I: Iterator<Item = Event>,
        S: screen::Screen, {
        match self {
            Mode::Viewing {
                ref mut current_range,
            } => {
                let commands = Mode::parse_normal(events);
                for cmd in commands {
                    match cmd {
                        Reaction::V(vmove) => {
                            Mode::view(vmove, current_range, screen, buf)
                                .unwrap();
                        }
                        Reaction::Quit => return None,
                        _ => return None,
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn view<S>(
        vmove: VerticalMove,
        current_range: &mut LinesRange,
        screen: &mut S,
        buf: &mut Buffer,
    ) -> screen::Result<()>
    where
        S: Screen, {
        let new_range =
            mv(vmove, current_range.clone(), screen.content_bounds());
        let requested_nr = new_range.range.size_hint().0;
        let lines = buf.read(&new_range);
        let lines = match requested_nr - lines.lines.len() {
            0 => lines,
            lack => buf.read(&new_range.extendl(lack)),
        };
        if lines.range != *current_range {
            *current_range = lines.range;
            return screen.draw_content(&lines.lines);
        }
        Ok(())
    }

    fn parse_normal<'a, I: Iterator<Item = Event>>(
        events: &'a mut I,
    ) -> impl Iterator<Item = Reaction> + 'a {
        events.flat_map(|evt| match evt {
            Key(KeyEvent {
                code: Char('q'), ..
            }) => Some(Reaction::Quit),
            Key(KeyEvent {
                code: Char('j'), ..
            }) => Some(Reaction::V(VerticalMove::LineDown)),
            Key(KeyEvent { code: Up, .. }) => {
                Some(Reaction::V(VerticalMove::LineUp))
            }
            Key(KeyEvent { code: Down, .. }) => {
                Some(Reaction::V(VerticalMove::LineDown))
            }
            Key(KeyEvent {
                code: Char('k'), ..
            }) => Some(Reaction::V(VerticalMove::LineUp)),
            Key(KeyEvent {
                code: Char('d'),
                modifiers: KeyModifiers::CONTROL,
            }) => Some(Reaction::V(VerticalMove::HalfPageDown)),
            Key(KeyEvent {
                code: Char('u'),
                modifiers: KeyModifiers::CONTROL,
            }) => Some(Reaction::V(VerticalMove::HalfPageUp)),
            Key(KeyEvent { code: PageDown, .. }) => {
                Some(Reaction::V(VerticalMove::PageDown))
            }
            Key(KeyEvent { code: PageUp, .. }) => {
                Some(Reaction::V(VerticalMove::PageUp))
            }
            Key(KeyEvent {
                code: Char('G'), ..
            }) => Some(Reaction::V(VerticalMove::Bottom)),
            Key(KeyEvent {
                code: Char('g'), ..
            }) => Some(Reaction::V(VerticalMove::Top)),
            Key(KeyEvent { code: Left, .. }) => {
                Some(Reaction::H(HorizontalMove::Left))
            }
            Key(KeyEvent { code: Right, .. }) => {
                Some(Reaction::H(HorizontalMove::Right))
            }
            _ => None,
        })
    }
}

struct State<'a, S: screen::Screen> {
    #[allow(unused)]
    header: &'a [&'a str],
    mode_stack: Vec<Mode>,
    #[allow(unused)]
    last_search: Option<Search>,
    buffer: Buffer<'a>,
    #[allow(unused)]
    commands: Vec<Vec<String>>,
    screen: S,
}

impl<'a, S: screen::Screen> State<'a, S> {
    fn new(
        mode: Mode,
        header: &'a [&'a str],
        screen: S,
        buffer: Buffer<'a>,
    ) -> Self {
        State {
            header: header,
            mode_stack: vec![mode],
            last_search: None,
            commands: vec![vec![]],
            screen: screen,
            buffer: buffer,
        }
    }

    fn run<I: Iterator<Item = Event>>(
        &mut self,
        mut events: I,
    ) -> Result<(), MorrError> {
        let evts = &mut events;
        while let Some(mut mode) = self.mode_stack.pop() {
            if let Some(next_mode) =
                mode.consume(evts, &mut self.screen, &mut self.buffer)
            {
                self.mode_stack.push(mode);
                self.mode_stack.push(next_mode);
            }
        }
        Ok(())
    }
}

fn mv(
    mv: VerticalMove,
    current_line_range: LinesRange,
    bounds: Rect,
) -> LinesRange {
    let max_lines = bounds.height() as usize;
    match mv {
        VerticalMove::Top => LinesRange::pos(0..max_lines),
        VerticalMove::Bottom => LinesRange::neg(0..max_lines),
        VerticalMove::LineUp => current_line_range.shiftl(1),
        VerticalMove::LineDown => current_line_range.shiftr(1),
        VerticalMove::PageUp => current_line_range.shiftl(max_lines),
        VerticalMove::PageDown => current_line_range.shiftr(max_lines),
        VerticalMove::HalfPageUp => current_line_range.shiftl(max_lines / 2),
        VerticalMove::HalfPageDown => current_line_range.shiftr(max_lines / 2),
    }
}

#[derive(Debug)]
enum MorrError {
    ScreenError { err: screen::ScreenError },
}

impl From<screen::ScreenError> for MorrError {
    fn from(e: screen::ScreenError) -> Self {
        MorrError::ScreenError { err: e }
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

enum Reaction {
    Quit,
    V(VerticalMove),
    H(HorizontalMove),
}
