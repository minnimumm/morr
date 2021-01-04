use std::io;
use std::io::Write;

#[cfg(target_family = "unix")]
mod unix;

#[cfg(target_family = "unix")]
use std::os::unix::io::IntoRawFd;
#[cfg(target_family = "unix")]
use unix::err_if_neg;

#[cfg(target_family = "windows")]
mod windows;

#[cfg(target_family = "windows")]
use windows::WinCon;

use std::io::Read;

#[derive(Debug)]
pub enum Cmd {
    ShowCursor,
    HideCursor,
    ClearScreen,
    ClearLine,
    Bold,
    Underline,
    Inverse,
    Reset,
    Print { content: String },
    Pos { x: u16, y: u16 },
}

pub struct Win {
    pub width: u16,
    pub height: u16,
}

#[cfg(target_family = "unix")]
pub struct UnixCon {
    pub output: std::io::Stdout,
    pub input: std::io::Stdin,
    buf: [u8; 2],
    orig_termios: libc::termios,
    fd: std::os::unix::io::RawFd,
}

impl UnixCon {
    fn read_event(self) -> Result<Event, std::io::Error> {
        let n = self.input.lock().read(&mut self.buf)?;
        let event = match self.buf[..n] {
            [b'a'] => Event::A,
            [b'b'] => Event::B,
            [b'c'] => Event::C,
            [b'd'] => Event::D,
            [b'e'] => Event::E,
            [b'f'] => Event::F,
            [b'g'] => Event::G,
            [b'h'] => Event::H,
            [b'i'] => Event::I,
            [b'j'] => Event::J,
            [b'k'] => Event::K,
            [b'l'] => Event::L,
            [b'm'] => Event::M,
            [b'n'] => Event::N,
            [b'o'] => Event::O,
            [b'p'] => Event::P,
            [b'q'] => Event::Q,
            [b'r'] => Event::R,
            [b's'] => Event::S,
            [b't'] => Event::T,
            [b'u'] => Event::U,
            [b'v'] => Event::V,
            [b'w'] => Event::W,
            [b'x'] => Event::X,
            [b'y'] => Event::Y,
            [b'z'] => Event::Z,
        };
        Ok(event)
    }
}

#[cfg(target_family = "unix")]
impl Drop for UnixCon {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(self.fd, libc::TCSAFLUSH, &mut self.orig_termios);
        }
    }
}

pub struct Con {
    #[cfg(target_family = "unix")]
    pub con: UnixCon,
    #[cfg(target_family = "windows")]
    pub con: WinCon,
}

enum Event {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Zero,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    ExlamationMark,
    DoubleQuote,
    NumberSign,
    DollarSign,
    PercentSign,
    Ampersand,
    SingleQuote,
    OpeningParenthesis,
    ClosingParenthesis,
    Asterisk,
    PlusSign,
    Comma,
    MinusSign,
    Dot,
    ForwardSlash,
    Colon,
    SemiColon,
    LessThanSign,
    EqualSign,
    MoreThanSign,
    QuestionMark,
    AtSign,
    OpeningBracket,
    BackwardSlash,
    ClosingBracket,
    Caret,
    Underscore,
    GraveAccent,
    OpeningBraces,
    VerticalLine,
    ClosingBraces,
    Tilde,
    LeftArrow,
    RightArrow,
    UpArrow,
    DownArrow,
}

#[cfg(target_family = "unix")]
impl Con {
    pub fn new() -> io::Result<Self> {
        let mut orig_termios = unsafe { std::mem::zeroed() };
        let fd = std::fs::File::open("/dev/tty")?.into_raw_fd();
        err_if_neg(|| unsafe { libc::tcgetattr(fd, &mut orig_termios) })?;
        let mut attrs = unsafe { std::mem::zeroed() };
        unsafe { libc::cfmakeraw(&mut attrs) };
        err_if_neg(|| unsafe { libc::tcsetattr(fd, libc::TCSANOW, &attrs) })?;
        Ok(Self {
            con: UnixCon {
                output: std::io::stdout(),
                input: std::io::stdin(),
                orig_termios: orig_termios,
                fd: fd,
            },
        })
    }

    fn ansi(command: &Cmd) -> String {
        match command {
            Cmd::ShowCursor => String::from("\x1B[?25h"),
            Cmd::HideCursor => String::from("\x1B[?25l"),
            Cmd::ClearScreen => String::from("\x1B[2J"),
            Cmd::Pos { x, y } => format!("\x1B[{y};{x}H", x = x + 1, y = y + 1),
            Cmd::ClearLine => String::from("\x1B[2K"),
            Cmd::Print { content } => content.clone(),
            Cmd::Reset => String::from("\x1B[0m"),
            Cmd::Inverse => String::from("\x1B[7m"),
            Cmd::Bold => String::from("\x1B[1m"),
            Cmd::Underline => String::from("\x1B[4m"),
        }
    }

    pub fn size(&self) -> io::Result<Win> {
        let mut winsize: libc::winsize = unsafe { std::mem::zeroed() };
        unix::err_if_neg(|| unsafe {
            libc::ioctl(self.con.fd, libc::TIOCGWINSZ, &mut winsize)
        })?;
        Ok(Win {
            width: winsize.ws_col,
            height: winsize.ws_row,
        })
    }

    pub fn execute<I>(&mut self, commands: I) -> io::Result<()>
    where
        I: IntoIterator<Item = Cmd>, {
        let batch: String =
            commands.into_iter().map(|cmd| Self::ansi(&cmd)).collect();
        self.con.output.write_all(batch.as_bytes())?;
        self.con.output.flush()
    }
}

#[cfg(target_family = "windows")]
impl Con for WinCon {
    pub fn new() -> io::Result<Self> {
        unimplemented!()
    }

    pub fn size(&self) -> io::Result<Win> {
        unimplemented!()
    }

    fn execute<I>(&mut self, commands: &I) -> io::Result<()>
    where
        I: Iterator<ConsoleCommand>, {
        unimplemented!()
    }
}
