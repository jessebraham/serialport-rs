use std::io;
use std::ptr;

// use winapi::shared::minwindef::DWORD;
// use winapi::shared::winerror::*;
// use winapi::um::errhandlingapi::GetLastError;
// use winapi::um::winbase::{
//     FormatMessageW, FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS,
// };
// use winapi::um::winnt::{LANG_SYSTEM_DEFAULT, MAKELANGID, SUBLANG_SYS_DEFAULT, WCHAR};

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_ACCESS_DENIED, GetLastError};
use windows_sys::Win32::System::Diagnostics::Debug::{FormatMessageW, FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS};
use windows_sys::Win32::System::SystemServices::SUBLANG_SYS_DEFAULT;

//in windows-sys 0.59.0
//In windows-sys 0.52.0 there is no such
// use windows_sys::Win32::Globalization::LANG_SYSTEM_DEFAULT;

//in windows-sys 0.52.0
//https://docs.rs/windows-sys/latest/windows_sys/Win32/Globalization/constant.LANG_SYSTEM_DEFAULT.html
pub const LANG_SYSTEM_DEFAULT: i32 = 2048i32;

//https://github.com/microsoft/windows-rs/issues/881
type DWORD = u32;
type WORD = u16;
//https://docs.rs/winapi/latest/src/winapi/um/winnt.rs.html#129
type LANGID = WORD;
//https://github.com/microsoft/windows-rs/issues/1874
#[cfg(windows)]
type WCHAR = u16;
//https://docs.rs/winapi/latest/src/winapi/um/winnt.rs.html#776-778
fn MAKELANGID(p: WORD, s: WORD) -> LANGID {
    (s << 10) | p
}

use crate::{Error, ErrorKind};

pub fn last_os_error() -> Error {
    let errno = errno();

    let kind = match errno {
        ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND | ERROR_ACCESS_DENIED => ErrorKind::NoDevice,
        _ => ErrorKind::Io(io::ErrorKind::Other),
    };

    Error::new(kind, error_string(errno).trim())
}

// the rest of this module is borrowed from libstd

fn errno() -> u32 {
    unsafe { GetLastError() }
}

fn error_string(errnum: u32) -> String {
    #![allow(non_snake_case)]

    // This value is calculated from the macro
    // MAKELANGID(LANG_SYSTEM_DEFAULT, SUBLANG_SYS_DEFAULT)
    let langId = MAKELANGID(LANG_SYSTEM_DEFAULT as u16, SUBLANG_SYS_DEFAULT as u16) as DWORD;

    let mut buf = [0 as WCHAR; 2048];

    unsafe {
        let res = FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
            ptr::null_mut(),
            errnum as DWORD,
            langId as DWORD,
            buf.as_mut_ptr(),
            buf.len() as DWORD,
            ptr::null_mut(),
        );
        if res == 0 {
            // Sometimes FormatMessageW can fail e.g. system doesn't like langId,
            let fm_err = errno();
            return format!(
                "OS Error {} (FormatMessageW() returned error {})",
                errnum, fm_err
            );
        }

        let b = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        let msg = String::from_utf16(&buf[..b]);
        match msg {
            Ok(msg) => msg,
            Err(..) => format!(
                "OS Error {} (FormatMessageW() returned invalid UTF-16)",
                errnum
            ),
        }
    }
}
