use std::io;
use winapi::shared::minwindef::BOOL;

pub fn err_if_false<O: FnMut() -> BOOL>(mut op: O) -> io::Result<()> {
    if op() != 1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
