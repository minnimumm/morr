use std::io;

use crossterm::{cursor, queue, style, terminal, ExecutableCommand};
use std::io::{stdout, Stdout, Write};
use std::marker::Sized;

pub type Result<S> = std::result::Result<S, ScreenError>;

#[derive(Debug, Clone, Copy)]
pub struct Point {
    x: u16,
    y: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    topleft: Point,
    botright: Point,
}


pub enum Colour {
    Normal
}

pub struct Style {
    pub attrs: Vec<Attribute>,
    pub colour: Colour, 
}

pub enum Attribute {
    Bold,
    Underline,
    Inverse,
}

impl Rect {
    pub fn from_topleft(topleft: Point, width: u16, height: u16) -> Self {
        Self {
            topleft: topleft,
            botright: Point {
                x: topleft.x + width,
                y: topleft.y + height,
            },
        }
    }

    pub fn from_botright(botright: Point, width: u16, height: u16) -> Self {
        Self {
            botright: botright,
            topleft: Point {
                x: botright.x - width,
                y: botright.y - height,
            },
        }
    }

    pub fn height(&self) -> u16 {
        self.botright.y - self.topleft.y
    }

    #[allow(unused)]
    pub fn width(&self) -> u16 {
        self.botright.x - self.botright.y
    }
}

pub trait Screen {
    fn init(header_height: u16, status_height: u16) -> Result<Self>
    where
        Self: Sized;
    fn header_bounds(&self) -> Rect;
    fn content_bounds(&self) -> Rect;
    fn status_bounds(&self) -> Rect;
    fn draw_header(&mut self, header: &[&str]) -> Result<()>;
    fn draw_status(&mut self, status: &str) -> Result<()>;
    fn draw_content(&mut self, content: &[&str]) -> Result<()>;
    fn draw(&mut self, content: &[StyledString], bound: &Rect) -> Result<()>;
    fn cleanup(&mut self) -> Result<()>;
}

pub struct ConsoleScreen {
    #[allow(unused)]
    bounds: Rect,
    header_bounds: Rect,
    content_bounds: Rect,
    status_bounds: Rect,
    out: Stdout,
}

#[derive(Debug)]
pub enum ScreenError {
    NotEnoughSpace {
        desired_height: u16,
        screen_height: u16,
    },
    SomeError,
}

impl From<io::Error> for ScreenError {
    fn from(_e: io::Error) -> Self {
        ScreenError::SomeError
    }
}

impl From<crossterm::ErrorKind> for ScreenError {
    fn from(e: crossterm::ErrorKind) -> Self {
        match e {
            crossterm::ErrorKind::IoError(_ioerr) => ScreenError::SomeError,
            crossterm::ErrorKind::FmtError(_fmterr) => ScreenError::SomeError,
            crossterm::ErrorKind::Utf8Error(_utferr) => ScreenError::SomeError,
            crossterm::ErrorKind::ParseIntError(_parseerr) => {
                ScreenError::SomeError
            }
            crossterm::ErrorKind::ResizingTerminalFailure(_msg) => {
                ScreenError::SomeError
            }
            _ => ScreenError::SomeError,
        }
    }
}

type StyledString<'a> = Vec<(&'a str, &'a Style)>;

impl Screen for ConsoleScreen {
    fn init(header_height: u16, status_height: u16) -> Result<Self> {
        terminal::enable_raw_mode()?;
        let mut out = stdout();
        out.execute(terminal::Clear(terminal::ClearType::All))?;
        out.execute(cursor::Hide)?;
        let (cols, rows) = terminal::size()?;
        let screen_bounds = Rect {
            topleft: Point { x: 0, y: 0 },
            botright: Point { x: cols, y: rows },
        };
        let desired_height = header_height + status_height + 1;
        if desired_height > rows {
            return Err(ScreenError::NotEnoughSpace {
                desired_height: desired_height,
                screen_height: rows,
            });
        }
        let header_bounds =
            Rect::from_topleft(screen_bounds.topleft, cols, header_height);
        let status_bounds =
            Rect::from_botright(screen_bounds.botright, cols, status_height);
        let content_bounds = Rect {
            topleft: Point {
                x: 0,
                y: header_bounds.botright.y,
            },
            botright: Point {
                x: cols,
                y: status_bounds.topleft.y,
            },
        };
        Ok(ConsoleScreen {
            bounds: screen_bounds,
            header_bounds: header_bounds,
            content_bounds: content_bounds,
            status_bounds: status_bounds,
            out: out,
        })
    }

    fn header_bounds(&self) -> Rect {
        self.header_bounds
    }

    fn content_bounds(&self) -> Rect {
        self.content_bounds
    }

    fn status_bounds(&self) -> Rect {
        self.status_bounds
    }

    fn draw_header(&mut self, header: &[&str]) -> Result<()> {
        let style = Style { attrs: vec![], colour: Colour::Normal };
        let arg: Vec<StyledString> = header.iter().map(|&s| vec![(s, &style)]).collect();
        self.draw(&arg, &self.header_bounds.clone())
    }

    fn draw_status(&mut self, status: &str) -> Result<()> {
        let style = Style { attrs: vec![Attribute::Inverse], colour: Colour::Normal };
        self.draw(&[vec![(status, &style)]], &self.status_bounds.clone())
    }

    fn draw(&mut self, content: &[StyledString], bounds: &Rect) -> Result<()> {
        let lines_to_draw = content.iter().take(bounds.height() as usize);
        for (i, line_parts) in lines_to_draw.enumerate() {
            let mv = cursor::MoveTo(bounds.topleft.x, bounds.topleft.y + i as u16);
            self.out.execute(mv)?;
            self.out.execute(terminal::Clear(terminal::ClearType::UntilNewLine))?;
            for (line_part, style) in line_parts {
                for attr in &style.attrs {
                    let s = match attr {
                        Attribute::Bold => style::Attribute::Bold,
                        Attribute::Underline => style::Attribute::Underlined,
                        Attribute::Inverse => style::Attribute::Reverse,
                    };
                    self.out.execute(style::SetAttribute(s))?;
                }
                self.out.execute(style::Print(line_part))?;
                self.out.execute(style::SetAttribute(style::Attribute::Reset))?;
            }
        }
        self.out.flush()?;
        Ok(())
    }

    fn draw_content(&mut self, content: &[&str]) -> Result<()> {
        let style = Style { attrs: vec![], colour: Colour::Normal };
        let arg: Vec<StyledString> = content.iter().map(|&s| vec![(s, &style)]).collect();
        self.draw(&arg, &self.content_bounds.clone())
    }

    fn cleanup(&mut self) -> Result<()> {
        queue!(self.out, terminal::Clear(terminal::ClearType::All))?;
        terminal::disable_raw_mode()?;
        Ok(())
    }
}
