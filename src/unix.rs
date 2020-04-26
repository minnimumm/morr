use std::io;

pub fn err_if_neg<O: FnMut() -> libc::c_int>(mut op: O) -> io::Result<()> {
    if op() < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
