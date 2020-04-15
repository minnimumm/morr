use crossterm::event::Event::Key;
use crossterm::event::KeyCode::{
    Char, Down, Left, PageDown, PageUp, Right, Up,
};
use crossterm::event::{Event, KeyEvent, KeyModifiers};
use crossterm::{cursor, event, queue, style, terminal};
use memmap::Mmap;
use std::env;
use std::fs::File;
use std::io;
use std::io::{stdout, Stdout, Write};
use std::iter;

mod line_reader;

use line_reader::{Buffer, LinesRange, ReadLines};

fn main() -> Result<(), DrawError> {
    let filename = env::args().nth(1).expect("No file name passed");
    let buf = File::open(&filename)
        .and_then(|file| unsafe { Mmap::map(&file) })
        .unwrap();
    let mut screen = ConsoleScreen::init().unwrap();
    let events = iter::repeat_with(event::read).flatten();
    let commands = parse(events);
    let rows = screen.rows();
    let mut line_reader = Buffer::new(&buf, &filename);
    let lines = line_reader.read(&LinesRange::pos(0..rows));
    let mut mode = NormalMode {
        line_reader: &mut line_reader,
        current_range: lines.range.clone(),
    };
    draw(&mut screen, mode.mk_draw_commands(lines))?;
    let draw_commands = commands
        .take_while(|cmd| !matches!(cmd, Command::Quit))
        .map(|cmd| match cmd {
            Command::V(vmove) => mode.process_move(vmove, rows),
            _ => vec![],
        });
    for commands in draw_commands {
        draw(&mut screen, commands)?;
    }
    Ok(())
}

struct NormalMode<'a> {
    line_reader: &'a mut Buffer<'a>,
    current_range: LinesRange,
}

impl<'a> NormalMode<'a> {
    fn process_move(
        &mut self,
        vmove: VerticalMove,
        rows: usize,
    ) -> Vec<DrawCommand<'a>> {
        let read_lines = self.move_and_read(vmove, rows);
        if read_lines.range != self.current_range {
            self.current_range = read_lines.range.clone();
            return self.mk_draw_commands(read_lines);
        }
        vec![]
    }

    fn mk_draw_commands(&self, lines: ReadLines<'a>) -> Vec<DrawCommand<'a>> {
        vec![
            DrawCommand::DrawContent { lines: lines },
            DrawCommand::DrawStatus {
                status: &self.line_reader.filename,
            },
        ]
    }

    fn move_and_read(
        &mut self,
        vmove: VerticalMove,
        rows: usize,
    ) -> ReadLines<'a> {
        let new_range = mv(vmove, self.current_range.clone(), rows);
        let requested_nr = new_range.range.size_hint().0;
        let lines = self.line_reader.read(&new_range);
        match requested_nr - lines.lines.len() {
            0 => lines,
            lack => self.line_reader.read(&new_range.extendl(lack)),
        }
    }
}

enum DrawCommand<'a> {
    DrawContent { lines: ReadLines<'a> },
    DrawStatus { status: &'a str },
}

fn mv(
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

fn draw<'a, S: Screen, I>(screen: &'a mut S, cmds: I) -> Result<(), DrawError>
where
    I: IntoIterator<Item = DrawCommand<'a>>, {
    cmds.into_iter().map(|cmd| screen.draw(cmd)).collect()
}

trait Screen {
    fn rows(&self) -> usize;
    fn cols(&self) -> usize;
    fn draw<'a>(&'a mut self, cmd: DrawCommand<'a>) -> Result<(), DrawError>;
    fn cleanup(&mut self);
}

struct ConsoleScreen {
    rows: u16,
    cols: u16,
    out: Stdout,
}

#[derive(Debug)]
enum DrawError {
    SomeError,
}

impl From<io::Error> for DrawError {
    fn from(_e: io::Error) -> Self {
        DrawError::SomeError
    }
}

impl From<crossterm::ErrorKind> for DrawError {
    fn from(e: crossterm::ErrorKind) -> Self {
        match e {
            crossterm::ErrorKind::IoError(_ioerr) => DrawError::SomeError,
            crossterm::ErrorKind::FmtError(_fmterr) => DrawError::SomeError,
            crossterm::ErrorKind::Utf8Error(_utferr) => DrawError::SomeError,
            crossterm::ErrorKind::ParseIntError(_parseerr) => {
                DrawError::SomeError
            }
            crossterm::ErrorKind::ResizingTerminalFailure(_msg) => {
                DrawError::SomeError
            }
            _ => DrawError::SomeError,
        }
    }
}

impl ConsoleScreen {
    fn init() -> Result<Self, Box<dyn std::error::Error>> {
        terminal::enable_raw_mode()?;
        let (cols, rows) = terminal::size()?;
        Ok(ConsoleScreen {
            rows: rows - 1,
            cols,
            out: stdout(),
        })
    }
}

impl Screen for ConsoleScreen {
    fn draw<'a>(&'a mut self, cmd: DrawCommand<'a>) -> Result<(), DrawError> {
        match cmd {
            DrawCommand::DrawContent { lines } => {
                queue!(self.out, terminal::Clear(terminal::ClearType::All))?;
                let lines_to_draw = lines.lines.iter().take(self.rows as usize);
                for (i, line) in lines_to_draw.enumerate() {
                    queue!(
                        self.out,
                        cursor::MoveTo(0, i as u16),
                        style::Print(line)
                    )?;
                }
            }
            DrawCommand::DrawStatus { status } => {
                queue!(
                    self.out,
                    cursor::MoveTo(0, self.rows),
                    style::SetAttribute(style::Attribute::Reverse),
                    style::Print(&status),
                    style::ResetColor
                )?;
            }
        };
        self.out.flush()?;
        Ok(())
    }

    fn rows(&self) -> usize {
        self.rows as usize
    }

    fn cols(&self) -> usize {
        self.cols as usize
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
            code: Char('q'), ..
        }) => Some(Command::Quit),
        Key(KeyEvent {
            code: Char('j'), ..
        }) => Some(Command::V(VerticalMove::LineDown)),
        Key(KeyEvent { code: Up, .. }) => {
            Some(Command::V(VerticalMove::LineUp))
        }
        Key(KeyEvent { code: Down, .. }) => {
            Some(Command::V(VerticalMove::LineDown))
        }
        Key(KeyEvent {
            code: Char('k'), ..
        }) => Some(Command::V(VerticalMove::LineUp)),
        Key(KeyEvent {
            code: Char('d'),
            modifiers: KeyModifiers::CONTROL,
        }) => Some(Command::V(VerticalMove::HalfPageDown)),
        Key(KeyEvent {
            code: Char('u'),
            modifiers: KeyModifiers::CONTROL,
        }) => Some(Command::V(VerticalMove::HalfPageUp)),
        Key(KeyEvent { code: PageDown, .. }) => {
            Some(Command::V(VerticalMove::PageDown))
        }
        Key(KeyEvent { code: PageUp, .. }) => {
            Some(Command::V(VerticalMove::PageUp))
        }
        Key(KeyEvent {
            code: Char('G'), ..
        }) => Some(Command::V(VerticalMove::Bottom)),
        Key(KeyEvent {
            code: Char('g'), ..
        }) => Some(Command::V(VerticalMove::Top)),
        Key(KeyEvent { code: Left, .. }) => {
            Some(Command::H(HorizontalMove::Left))
        }
        Key(KeyEvent { code: Right, .. }) => {
            Some(Command::H(HorizontalMove::Right))
        }
        _ => None,
    })
}
