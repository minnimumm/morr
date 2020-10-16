use winapi::um::wincon::SetCurrentConsoleFontEx;
use winapi::um::wincon::CONSOLE_FONT_INFOEX;
use winapi::um::wincon::GetCurrentConsoleFontEx;
use winapi::um::wincon::PCONSOLE_FONT_INFOEX;
use winapi::um::wincon::COMMON_LVB_UNDERSCORE;
use winapi::um::wincon::COMMON_LVB_REVERSE_VIDEO;
use winapi::um::wincon::SetConsoleTextAttribute;
use winapi::um::wincon::FOREGROUND_BLUE;
use winapi::um::wincon::FOREGROUND_RED;
use winapi::um::wincon::FOREGROUND_GREEN;
use winapi::um::wincon::CONSOLE_CURSOR_INFO;
use winapi::um::wincon::SetConsoleCursorInfo;
use winapi::um::wincon::WriteConsoleOutputCharacterW;
use winapi::um::wincon::WriteConsoleOutputCharacterA;
use winapi::um::wincontypes::COORD;
use winapi::um::wincon::SetConsoleCursorPosition;
use winapi::um::wincon::FillConsoleOutputAttribute;
use winapi::um::wincon::FillConsoleOutputCharacterA;
use winapi::shared::ntdef::NULL;
use winapi::um::consoleapi::WriteConsoleW;
use winapi::ctypes::c_void;
use winapi::um::wincon::GetConsoleScreenBufferInfo;
use winapi::um::wincon::CONSOLE_SCREEN_BUFFER_INFO;
use std::io;
use std::io::Write;
use std::convert::TryInto;
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
        ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT, ENABLE_WINDOW_INPUT, INPUT_RECORD, GetLargestConsoleWindowSize 
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
    in_handle: HANDLE,
    out_handle: HANDLE,
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
        let in_utf16: Vec<u16> = OsStr::new("CONIN$\0").encode_wide().collect();
        let in_utf16_ptr: *const u16 = in_utf16.as_ptr();
        let in_handle = unsafe {
            CreateFileW(
                in_utf16_ptr,
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null_mut(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        let out_utf16: Vec<u16> = OsStr::new("CONOUT$\0").encode_wide().collect();
        let out_utf16_ptr: *const u16 = out_utf16.as_ptr();
        let out_handle = unsafe {
            CreateFileW(
                out_utf16_ptr,
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
                in_handle: in_handle,
                out_handle: out_handle,
            }
        })
    }

    pub fn size(&self) -> io::Result<Win> { 
        let mut buffer_info: CONSOLE_SCREEN_BUFFER_INFO = unsafe { std::mem::zeroed() };
        unsafe { GetConsoleScreenBufferInfo(self.con.out_handle, &mut buffer_info) };
        // let win_size = unsafe { GetLargestConsoleWindowSize(self.con.out_handle) };
        let window_height: i16 = buffer_info.srWindow.Bottom - buffer_info.srWindow.Top;
        Ok(Win {
            width: buffer_info.srWindow.Right as u16 + 1,
            height: window_height as u16 + 1,
            // width: win_size.X as u16,
            // height: win_size.Y as u16,
        })
    }

    pub fn execute<I>(&mut self, commands: I) -> io::Result<()>
    where
        I: IntoIterator<Item = Cmd>, {
            commands.into_iter()
                .for_each(|cmd| match cmd {
                    Cmd::ShowCursor => {
                        let mut cursor_info = CONSOLE_CURSOR_INFO {
                            dwSize: 10 as u32,
                            bVisible: 1 as i32
                        };
                        { unsafe { SetConsoleCursorInfo(self.con.out_handle, &mut cursor_info ) }; }
                    },
                    Cmd::HideCursor => {
                        let mut cursor_info = CONSOLE_CURSOR_INFO {
                            dwSize: 10 as u32,
                            bVisible: 0 as i32
                        };
                        { unsafe { SetConsoleCursorInfo(self.con.out_handle, &mut cursor_info ) }; }
                    },
                    Cmd::ClearScreen => { self.clear(); },
                    Cmd::ClearLine => { self.clear_line(); },
                    Cmd::Bold => {
                        let mut font_info: CONSOLE_FONT_INFOEX = unsafe { std::mem::zeroed() };
                        unsafe { GetCurrentConsoleFontEx(self.con.out_handle, 0, &mut font_info) };
                        font_info.FontWeight = 700;
                        unsafe { SetCurrentConsoleFontEx(self.con.out_handle, 0, &mut font_info) };
                    },
                    Cmd::Underline => {
                        { unsafe { SetConsoleTextAttribute(
                            self.con.out_handle,
                            COMMON_LVB_UNDERSCORE
                        ) }};
                    },
                    Cmd::Inverse => {
                        let mut buffer_info: CONSOLE_SCREEN_BUFFER_INFO = unsafe { std::mem::zeroed() };
                        unsafe { GetConsoleScreenBufferInfo(self.con.out_handle, &mut buffer_info) };
                        { unsafe { SetConsoleTextAttribute(
                            self.con.out_handle,
                            COMMON_LVB_REVERSE_VIDEO | buffer_info.wAttributes
                        ) }};
                    },
                    Cmd::Reset => {},
                    Cmd::Print{content} => { self.print_at(content, 0, 0); },
                    Cmd::Pos{x, y} => { unsafe { SetConsoleCursorPosition(self.con.out_handle, COORD {X: x as i16, Y: y as i16}) }; },
                    _ => { },
                });

            Ok(())
    }

    fn print_at<P>(&self, content: String, x: P, y: P) -> io::Result<()> 
    where
        P: TryInto<i16>
    {
                        // { unsafe { SetConsoleTextAttribute(
                        //     self.con.out_handle,
                        //     FOREGROUND_GREEN
                        // ) }};
        let coords: COORD = x.try_into().and_then(|x_| y.try_into().map(|y_| COORD {X: x_, Y: y_})).map_err(|e| io::ErrorKind::Other)?;
        let console_handle: HANDLE = self.con.out_handle;
        let mut buffer_info: CONSOLE_SCREEN_BUFFER_INFO = unsafe { std::mem::zeroed() };
        unsafe { GetConsoleScreenBufferInfo(self.con.out_handle, &mut buffer_info) };

        // let top_left: COORD = COORD {X: 0, Y: 0 };
        // unsafe { SetConsoleCursorPosition(console_handle, top_left) };
        let utf16: Vec<u16> = content.encode_utf16().collect();
        let mut cells_written: u32 = 0;
        // write to console
        unsafe {
            WriteConsoleW(
                self.con.out_handle,
                utf16.as_ptr() as *const _ as *const c_void,
                utf16.len() as u32,
                &mut cells_written,
                NULL,
            );
        }
        // let utf16: Vec<u16> = content.encode_utf16().collect();
        // let mut cells_written: u32 = 0;
        // unsafe {WriteConsoleOutputCharacterW(
        //     console_handle,
        //     utf16.as_ptr(),
        //     utf16.len() as u32, 
        //     coords, 
        //     &mut cells_written
        // ) };
        // FillConsoleOutputAttribute(
        //     console, FOREGROUND_GREEN | FOREGROUND_RED | FOREGROUND_BLUE,
        //     screen.dwSize.X * screen.dwSize.Y, topLeft, &written
        // );
        // unsafe { SetConsoleCursorPosition(console_handle, coords) };
        Ok(())
            // get string from u8[] and parse it to an c_str
            // let utf8 = match std::str::from_utf8(b"AAAAA") {
            //     Ok(string) => string,
            //     Err(_) => {
            //         return Err(io::Error::new(
            //             io::ErrorKind::Other,
            //             "Could not parse to utf8 string",
            //         ));
            //     }
            // };


    }

    fn clear_line(&self) -> io::Result<()> {
        let console_handle: HANDLE = self.con.out_handle;
        let mut buffer_info: CONSOLE_SCREEN_BUFFER_INFO = unsafe { std::mem::zeroed() };
        unsafe { GetConsoleScreenBufferInfo(self.con.out_handle, &mut buffer_info) };
        let mut cells_written: u32 = 0;
        unsafe { FillConsoleOutputCharacterA(
            console_handle,
            b' '.try_into().unwrap(), 
            buffer_info.srWindow.Right.try_into().unwrap(), 
            buffer_info.dwCursorPosition, 
            &mut cells_written
        ) };
        Ok(())
    }

    fn clear(&self) -> io::Result<()> {
        let top_left: COORD = COORD {X: 0, Y: 0 };
        let console_handle: HANDLE = self.con.out_handle;
        let mut buffer_info: CONSOLE_SCREEN_BUFFER_INFO = unsafe { std::mem::zeroed() };
        unsafe { GetConsoleScreenBufferInfo(self.con.out_handle, &mut buffer_info) };

        let mut cells_written: u32 = 0;
        let window_height: i16 = buffer_info.srWindow.Bottom - buffer_info.srWindow.Top;
        unsafe { FillConsoleOutputCharacterA(
            console_handle,
            b' '.try_into().unwrap(), 
            // 1000 as u32,
            (buffer_info.srWindow.Right * window_height).try_into().unwrap(),
            // (buffer_info.dwSize.X * buffer_info.dwSize.Y).try_into().unwrap(), 
            top_left, 
            &mut cells_written
        ) };
        // FillConsoleOutputAttribute(
        //     console, FOREGROUND_GREEN | FOREGROUND_RED | FOREGROUND_BLUE,
        //     screen.dwSize.X * screen.dwSize.Y, topLeft, &written
        // );
        // unsafe { SetConsoleCursorPosition(console_handle, top_left) };
        Ok(())
    }
}
