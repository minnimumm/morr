use std::io;

use std::marker::Sized;

#[path = "./cons.rs"]
mod cons;

use cons::{Cmd, Con};

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
    Normal,
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
    fn init(
        header_height: u16,
        status_height: u16,
    ) -> Result<Self, ScreenError>
    where
        Self: Sized;
    fn header_bounds(&self) -> Rect;
    fn content_bounds(&self) -> Rect;
    fn status_bounds(&self) -> Rect;
    fn draw_header(&mut self, header: &[&str]) -> io::Result<()>;
    fn draw_status(&mut self, status: &str) -> io::Result<()>;
    fn draw_content(&mut self, content: &[&str]) -> io::Result<()>;
    fn draw(
        &mut self,
        content: &[StyledString],
        bound: &Rect,
    ) -> io::Result<()>;
}

pub struct ConsoleScreen {
    #[allow(unused)]
    bounds: Rect,
    cons: Con,
    header_bounds: Rect,
    content_bounds: Rect,
    status_bounds: Rect,
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

type StyledString<'a> = Vec<(&'a str, &'a Style)>;

impl ConsoleScreen {
    fn new(
        header_height: u16,
        status_height: u16,
    ) -> Result<Self, ScreenError> {
        let mut cons = Con::new()?;
        let start_commands = vec![Cmd::ClearScreen, Cmd::HideCursor];
        let win = cons.size()?;
        cons.execute(start_commands)?;
        let origin = Point { x: 0, y: 0 };
        let screen_bounds = Rect::from_topleft(origin, win.width, win.height);
        let desired_height = header_height + status_height + 1;
        if desired_height > win.height {
            return Err(ScreenError::NotEnoughSpace {
                desired_height: desired_height,
                screen_height: win.height,
            });
        }
        let header_bounds =
            Rect::from_topleft(screen_bounds.topleft, win.width, header_height);
        let status_bounds = Rect::from_botright(
            screen_bounds.botright,
            win.width,
            status_height,
        );
        let content_bounds = Rect {
            topleft: Point {
                x: 0,
                y: header_bounds.botright.y,
            },
            botright: Point {
                x: win.width,
                y: status_bounds.topleft.y,
            },
        };
        Ok(ConsoleScreen {
            cons: cons,
            bounds: screen_bounds,
            header_bounds: header_bounds,
            content_bounds: content_bounds,
            status_bounds: status_bounds,
        })
    }
}

impl Drop for ConsoleScreen {
    fn drop(&mut self) {
        self.cons
            .execute(vec![
                Cmd::Pos { x: 0, y: 0 },
                Cmd::ClearScreen,
                Cmd::ShowCursor,
            ])
            .unwrap()
    }
}

impl Screen for ConsoleScreen {
    fn init(
        header_height: u16,
        status_height: u16,
    ) -> Result<Self, ScreenError> {
        Self::new(header_height, status_height)
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

    fn draw_header(&mut self, header: &[&str]) -> io::Result<()> {
        let style = Style {
            attrs: vec![],
            colour: Colour::Normal,
        };
        let arg: Vec<StyledString> =
            header.iter().map(|&s| vec![(s, &style)]).collect();
        self.draw(&arg, &self.header_bounds.clone())
    }

    fn draw_status(&mut self, status: &str) -> io::Result<()> {
        let style = Style {
            attrs: vec![Attribute::Inverse],
            colour: Colour::Normal,
        };
        self.draw(&[vec![(status, &style)]], &self.status_bounds.clone())
    }

    fn draw(
        &mut self,
        content: &[StyledString],
        bounds: &Rect,
    ) -> io::Result<()> {
        let lines_to_draw = content.iter().take(bounds.height() as usize);
        let batch = lines_to_draw.enumerate().flat_map(|(i, line_parts)| {
            let move_and_clear = vec![
                Cmd::Pos {
                    x: bounds.topleft.x,
                    y: bounds.topleft.y + i as u16,
                },
                Cmd::ClearLine,
            ];
            let print =
                line_parts.into_iter().flat_map(|(line_part, style)| {
                    let mut commands = vec![];
                    commands.extend(style.attrs.iter().map(
                        |attr| match attr {
                            Attribute::Bold => Cmd::Bold,
                            Attribute::Underline => Cmd::Underline,
                            Attribute::Inverse => Cmd::Inverse,
                        },
                    ));
                    commands.push(Cmd::Print {
                        content: String::from(*line_part),
                    });
                    commands.push(Cmd::Reset);
                    commands
                });
            move_and_clear.into_iter().chain(print)
        });
        self.cons.execute(batch)
    }

    fn draw_content(&mut self, content: &[&str]) -> io::Result<()> {
        let style = Style {
            attrs: vec![],
            colour: Colour::Normal,
        };
        let arg: Vec<StyledString> =
            content.iter().map(|&s| vec![(s, &style)]).collect();
        self.draw(&arg, &self.content_bounds.clone())
    }
}
