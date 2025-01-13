use std::mem::MaybeUninit;
use winapi::shared::minwindef::*;
use winapi::um::commapi::*;
use winapi::um::winbase::*;
use winapi::um::winnt::HANDLE;

use crate::{DataBits, FlowControl, Parity, Result, StopBits};

pub(crate) fn get_dcb(handle: HANDLE) -> Result<DCB> {
    let mut dcb: DCB = unsafe { MaybeUninit::zeroed().assume_init() };
    dcb.DCBlength = std::mem::size_of::<DCB>() as u32;

    if unsafe { GetCommState(handle, &mut dcb) } != 0 {
        Ok(dcb)
    } else {
        Err(super::error::last_os_error())
    }
}

/// Initialize the DCB struct
/// Set all values that won't be affected by `SerialPortBuilder` options.
pub(crate) fn init(dcb: &mut DCB) {
    // dcb.DCBlength
    // dcb.BaudRate
    // dcb.BitFields
    // dcb.wReserved
    // dcb.XonLim
    // dcb.XoffLim
    // dcb.ByteSize
    // dcb.Parity
    // dcb.StopBits
    dcb.XonChar = 17;
    dcb.XoffChar = 19;
    dcb.ErrorChar = '\0' as winapi::ctypes::c_char;
    dcb.EofChar = 26;
    // dcb.EvtChar
    // always true for communications resources
    dcb.set_fBinary(TRUE as DWORD);
    // dcb.set_fParity()
    // dcb.set_fOutxCtsFlow()
    // serialport-rs doesn't support toggling DSR: so disable fOutxDsrFlow
    dcb.set_fOutxDsrFlow(FALSE as DWORD);
    dcb.set_fDtrControl(DTR_CONTROL_DISABLE);
    // disable because fOutxDsrFlow is disabled as well
    dcb.set_fDsrSensitivity(FALSE as DWORD);
    // dcb.set_fTXContinueOnXoff()
    // dcb.set_fOutX()
    // dcb.set_fInX()
    dcb.set_fErrorChar(FALSE as DWORD);
    // fNull: when set to TRUE null bytes are discarded when received.
    // null bytes won't be discarded by serialport-rs
    dcb.set_fNull(FALSE as DWORD);
    // dcb.set_fRtsControl()
    // serialport-rs does not handle the fAbortOnError behaviour, so we must make sure it's not enabled
    dcb.set_fAbortOnError(FALSE as DWORD);
}

pub(crate) fn set_dcb(handle: HANDLE, mut dcb: DCB) -> Result<()> {
    if unsafe { SetCommState(handle, &mut dcb as *mut _) != 0 } {
        Ok(())
    } else {
        Err(super::error::last_os_error())
    }
}

pub(crate) fn set_baud_rate(dcb: &mut DCB, baud_rate: u32) {
    dcb.BaudRate = baud_rate as DWORD;
}

pub(crate) fn set_data_bits(dcb: &mut DCB, data_bits: DataBits) {
    dcb.ByteSize = match data_bits {
        DataBits::Five => 5,
        DataBits::Six => 6,
        DataBits::Seven => 7,
        DataBits::Eight => 8,
    };
}

pub(crate) fn set_parity(dcb: &mut DCB, parity: Parity) {
    dcb.Parity = match parity {
        Parity::None => NOPARITY,
        Parity::Odd => ODDPARITY,
        Parity::Even => EVENPARITY,
    };

    dcb.set_fParity(if parity == Parity::None { FALSE } else { TRUE } as DWORD);
}

pub(crate) fn set_stop_bits(dcb: &mut DCB, stop_bits: StopBits) {
    dcb.StopBits = match stop_bits {
        StopBits::One => ONESTOPBIT,
        StopBits::OnePointFive => ONE5STOPBITS,
        StopBits::Two => TWOSTOPBITS,
    };
}

pub(crate) fn set_flow_control(dcb: &mut DCB, flow_control: FlowControl) {
    match flow_control {
        FlowControl::None => {
            dcb.set_fOutxCtsFlow(0);
            dcb.set_fRtsControl(0);
            dcb.set_fOutX(0);
            dcb.set_fInX(0);
        }
        FlowControl::Software => {
            dcb.set_fOutxCtsFlow(0);
            dcb.set_fRtsControl(0);
            dcb.set_fOutX(1);
            dcb.set_fInX(1);
        }
        FlowControl::Hardware => {
            dcb.set_fOutxCtsFlow(1);
            dcb.set_fRtsControl(1);
            dcb.set_fOutX(0);
            dcb.set_fInX(0);
        }
    }
}
