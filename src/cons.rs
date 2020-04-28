use std::io;
use std::io::Write;
use std::ptr::null_mut;
use std::ffi::OsStr;
#[cfg(target_family = "windows")]
use std::os::windows::ffi::OsStrExt;

use winapi::um::consoleapi::{GetConsoleMode, SetConsoleMode, ReadConsoleInputW};
use winapi::um::{
    fileapi::{CreateFileW, OPEN_EXISTING},
    handleapi::{CloseHandle, INVALID_HANDLE_VALUE},
    processenv::GetStdHandle,
    winbase::{STD_INPUT_HANDLE, STD_OUTPUT_HANDLE},
    winnt::{
        FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE, HANDLE,
    },
};
use winapi::{
    shared::minwindef::DWORD,
    um::wincon::{
        ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT, ENABLE_WINDOW_INPUT, INPUT_RECORD, 
    },
};


#[cfg(target_family = "unix")]
mod unix;

#[cfg(target_family = "unix")]
use std::os::unix::io::IntoRawFd;
#[cfg(target_family = "unix")]
use unix::err_if_neg;

#[cfg(target_family = "windows")]
mod windows;

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
    orig_termios: libc::termios,
    fd: std::os::unix::io::RawFd,
}

#[cfg(target_family = "unix")]
impl Drop for UnixCon {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(self.fd, libc::TCSAFLUSH, &mut self.orig_termios);
        }
    }
}

#[cfg(target_family = "windows")]
pub struct WinCon {
    handle: HANDLE,
}

pub struct Con {
    #[cfg(target_family = "unix")]
    pub con: UnixCon,
    #[cfg(target_family = "windows")]
    pub con: WinCon,
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
impl Con {
    pub fn new() -> io::Result<Self> {
        let utf16: Vec<u16> = OsStr::new("CONIN$\0").encode_wide().collect();
        let utf16_ptr: *const u16 = utf16.as_ptr();
        let handle = unsafe {
            CreateFileW(
                utf16_ptr,
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null_mut(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        Ok(Self {
            con: WinCon {
                handle: handle
            }
        })
    }

    pub fn size(&self) -> io::Result<Win> {
        unimplemented!()
    }

    pub fn execute<I>(&mut self, commands: I) -> io::Result<()>
    where
        I: IntoIterator<Item = Cmd>, {
        unimplemented!()
    }
}
