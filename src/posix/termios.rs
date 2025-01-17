// A set of helper functions for working with the `termios` and `termios2` structs
use cfg_if::cfg_if;

use crate::{DataBits, FlowControl, Parity, Result, StopBits};
use nix::libc;

use std::os::unix::prelude::*;

cfg_if! {
    if #[cfg(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd",
        all(
            target_os = "linux",
            any(
                target_env = "musl",
                target_arch = "powerpc",
                target_arch = "powerpc64"
            )
        )
    ))] {
        pub(crate) type Termios = libc::termios;
    } else if #[cfg(any(
        target_os = "android",
        all(
            target_os = "linux",
            not(any(
                target_env = "musl",
                target_arch = "powerpc",
                target_arch = "powerpc64"
            ))
        )
    ))] {
        pub(crate) type Termios = libc::termios2;
    } else {
        compile_error!("Unsupported platform. See crate documentation for supported platforms");
    }
}

// The termios struct isn't used for storing the baud rate, but it can be affected by other
// calls in this lib to the IOSSIOSPEED ioctl. So whenever we get this struct, make sure to
// reset the input & output baud rates to a safe default. This is accounted for by the
// corresponding set_termios that is mac-specific and always calls IOSSIOSPEED.
#[cfg(any(target_os = "ios", target_os = "macos",))]
pub(crate) fn get_termios(fd: RawFd) -> Result<Termios> {
    use std::mem::MaybeUninit;

    let mut termios = MaybeUninit::uninit();
    let res = unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) };
    nix::errno::Errno::result(res)?;
    let mut termios = unsafe { termios.assume_init() };
    termios.c_ispeed = self::libc::B9600;
    termios.c_ospeed = self::libc::B9600;
    Ok(termios)
}

#[cfg(any(
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    all(
        target_os = "linux",
        any(
            target_env = "musl",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        )
    )
))]
pub(crate) fn get_termios(fd: RawFd) -> Result<Termios> {
    use std::mem::MaybeUninit;

    let mut termios = MaybeUninit::uninit();
    let res = unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) };
    nix::errno::Errno::result(res)?;
    unsafe { Ok(termios.assume_init()) }
}

#[cfg(any(
    target_os = "android",
    all(
        target_os = "linux",
        not(any(
            target_env = "musl",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        ))
    )
))]
pub(crate) fn get_termios(fd: RawFd) -> Result<Termios> {
    crate::posix::ioctl::tcgets2(fd)
}

#[cfg(any(target_os = "ios", target_os = "macos",))]
pub(crate) fn set_termios(fd: RawFd, termios: &libc::termios, baud_rate: u32) -> Result<()> {
    let res = unsafe { libc::tcsetattr(fd, libc::TCSANOW, termios) };
    nix::errno::Errno::result(res)?;

    // Note: attempting to set the baud rate on a pseudo terminal via this ioctl call will fail
    // with the `ENOTTY` error.
    if baud_rate > 0 {
        crate::posix::ioctl::iossiospeed(fd, &(baud_rate as libc::speed_t))?;
    }

    Ok(())
}

#[cfg(any(
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    all(
        target_os = "linux",
        any(
            target_env = "musl",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        )
    )
))]
pub(crate) fn set_termios(fd: RawFd, termios: &libc::termios) -> Result<()> {
    let res = unsafe { libc::tcsetattr(fd, libc::TCSANOW, termios) };
    nix::errno::Errno::result(res)?;
    Ok(())
}

#[cfg(any(
    target_os = "android",
    all(
        target_os = "linux",
        not(any(
            target_env = "musl",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        ))
    )
))]
pub(crate) fn set_termios(fd: RawFd, termios: &Termios) -> Result<()> {
    crate::posix::ioctl::tcsets2(fd, termios)
}

// The Result return will seem pointless on platforms that support all parity variants, but we need
// it for the others.
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn set_parity(termios: &mut Termios, parity: Parity) -> Result<()> {
    match parity {
        Parity::None => {
            termios.c_cflag &= !(libc::PARENB | libc::PARODD);
            termios.c_iflag &= !libc::INPCK;
            termios.c_iflag |= libc::IGNPAR;
            #[cfg(any(target_os = "linux", target_os = "android"))]
            {
                termios.c_iflag &= !libc::CMSPAR;
            }
        }
        Parity::Odd => {
            termios.c_cflag |= libc::PARENB | libc::PARODD;
            termios.c_iflag |= libc::INPCK;
            termios.c_iflag &= !libc::IGNPAR;
            #[cfg(any(target_os = "linux", target_os = "android"))]
            {
                termios.c_iflag &= !libc::CMSPAR;
            }
        }
        Parity::Even => {
            termios.c_cflag &= !libc::PARODD;
            termios.c_cflag |= libc::PARENB;
            termios.c_iflag |= libc::INPCK;
            termios.c_iflag &= !libc::IGNPAR;
            #[cfg(any(target_os = "linux", target_os = "android"))]
            {
                termios.c_iflag &= !libc::CMSPAR;
            }
        }
        #[cfg(any(target_os = "linux", target_os = "android"))]
        Parity::Mark => {
            termios.c_cflag |= libc::PARODD;
            termios.c_cflag |= libc::PARENB;
            termios.c_iflag |= libc::INPCK;
            termios.c_iflag &= !libc::IGNPAR;
            termios.c_iflag |= libc::CMSPAR;
        }
        #[cfg(any(target_os = "linux", target_os = "android"))]
        Parity::Space => {
            termios.c_cflag &= !libc::PARODD;
            termios.c_cflag |= libc::PARENB;
            termios.c_iflag |= libc::INPCK;
            termios.c_iflag &= !libc::IGNPAR;
            termios.c_iflag |= libc::CMSPAR;
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        Parity::Mark => {
            return Err(crate::Error::new(
                crate::ErrorKind::InvalidInput,
                "Mark parity not supported on this platform",
            ));
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        Parity::Space => {
            return Err(crate::Error::new(
                crate::ErrorKind::InvalidInput,
                "Space parity not supported on this platform",
            ));
        }
    };
    Ok(())
}

pub(crate) fn set_flow_control(termios: &mut Termios, flow_control: FlowControl) {
    match flow_control {
        FlowControl::None => {
            termios.c_iflag &= !(libc::IXON | libc::IXOFF);
            termios.c_cflag &= !libc::CRTSCTS;
        }
        FlowControl::Software => {
            termios.c_iflag |= libc::IXON | libc::IXOFF;
            termios.c_cflag &= !libc::CRTSCTS;
        }
        FlowControl::Hardware => {
            termios.c_iflag &= !(libc::IXON | libc::IXOFF);
            termios.c_cflag |= libc::CRTSCTS;
        }
    };
}

pub(crate) fn set_data_bits(termios: &mut Termios, data_bits: DataBits) {
    let size = match data_bits {
        DataBits::Five => libc::CS5,
        DataBits::Six => libc::CS6,
        DataBits::Seven => libc::CS7,
        DataBits::Eight => libc::CS8,
    };

    termios.c_cflag &= !libc::CSIZE;
    termios.c_cflag |= size;
}

pub(crate) fn set_stop_bits(termios: &mut Termios, stop_bits: StopBits) {
    match stop_bits {
        StopBits::One => termios.c_cflag &= !libc::CSTOPB,
        StopBits::Two => termios.c_cflag |= libc::CSTOPB,
    };
}

#[cfg(any(
    target_os = "android",
    all(
        target_os = "linux",
        not(any(
            target_env = "musl",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        ))
    )
))]
pub(crate) fn set_baud_rate(termios: &mut Termios, baud_rate: u32) -> Result<()> {
    termios.c_cflag &= !nix::libc::CBAUD;
    termios.c_cflag |= nix::libc::BOTHER;
    termios.c_ispeed = baud_rate;
    termios.c_ospeed = baud_rate;
    Ok(())
}

// BSDs use the baud rate as the constant value so there's no translation necessary
#[cfg(any(
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
pub(crate) fn set_baud_rate(termios: &mut Termios, baud_rate: u32) -> Result<()> {
    let res = unsafe { libc::cfsetspeed(termios, baud_rate.into()) };
    nix::errno::Errno::result(res)?;
    Ok(())
}

#[cfg(all(
    target_os = "linux",
    any(
        target_env = "musl",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    )
))]
pub(crate) fn set_baud_rate(termios: &mut Termios, baud_rate: u32) -> Result<()> {
    use crate::{Error, ErrorKind};

    use self::libc::{
        B1000000, B1152000, B1500000, B2000000, B2500000, B3000000, B3500000, B4000000, B460800,
        B500000, B576000, B921600,
    };
    use self::libc::{
        B110, B115200, B1200, B134, B150, B1800, B19200, B200, B230400, B2400, B300, B38400, B4800,
        B50, B57600, B600, B75, B9600,
    };

    let baud_rate = match baud_rate {
        50 => B50,
        75 => B75,
        110 => B110,
        134 => B134,
        150 => B150,
        200 => B200,
        300 => B300,
        600 => B600,
        1200 => B1200,
        1800 => B1800,
        2400 => B2400,
        4800 => B4800,
        9600 => B9600,
        19_200 => B19200,
        38_400 => B38400,
        57_600 => B57600,
        115_200 => B115200,
        230_400 => B230400,
        460_800 => B460800,
        500_000 => B500000,
        576_000 => B576000,
        921_600 => B921600,
        1_000_000 => B1000000,
        1_152_000 => B1152000,
        1_500_000 => B1500000,
        2_000_000 => B2000000,
        2_500_000 => B2500000,
        3_000_000 => B3000000,
        3_500_000 => B3500000,
        4_000_000 => B4000000,
        _ => return Err(Error::new(ErrorKind::InvalidInput, "Unsupported baud rate")),
    };
    let res = unsafe { libc::cfsetspeed(termios, baud_rate) };
    nix::errno::Errno::result(res)?;
    Ok(())
}
