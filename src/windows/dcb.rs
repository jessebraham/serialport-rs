use std::mem::MaybeUninit;
// use winapi::shared::minwindef::*;
// use winapi::um::commapi::*;
// use winapi::um::winbase::*;
// use winapi::um::winnt::HANDLE;

use windows_sys::Win32::Devices::Communication::*;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::{TRUE, FALSE};
use windows_sys::Win32::System::WindowsProgramming::DTR_CONTROL_DISABLE;
//https://github.com/microsoft/windows-rs/issues/881
type DWORD = u32;

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
    //dcb.ErrorChar = '\0' as winapi::ctypes::c_char;
    //https://github.com/microsoft/windows-rs/issues/2188
    // dcb.ErrorChar = '\0' as i8;
    dcb.ErrorChar = '\0' as u8;
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

//https://github.com/rust-lang/rfcs/issues/314
//currently windows-sys does not implement bit fields
// pub struct MyDCB(DCB);
pub trait MyTrait {
    fn set_fBinary(&mut self, fBinary: DWORD) -> ();
    fn set_fParity(&mut self, fParity: DWORD) -> ();
    fn set_fOutxCtsFlow(&mut self, fOutxCtsFlow: DWORD) -> ();
    fn set_fOutxDsrFlow(&mut self, fOutxDsrFlow: DWORD) -> ();
    fn set_fDtrControl(&mut self, fDtrControl: DWORD) -> ();
    fn set_fDsrSensitivity(&mut self, fDsrSensitivity: DWORD) -> ();
    fn set_fTXContinueOnXoff(&mut self, fTXContinueOnXoff: DWORD) -> ();
    fn set_fOutX(&mut self, fOutX: DWORD) -> ();
    fn set_fInX(&mut self, fInX: DWORD) -> ();
    fn set_fErrorChar(&mut self, fErrorChar: DWORD) -> ();
    fn set_fNull(&mut self, fNull: DWORD) -> ();
    fn set_fRtsControl(&mut self, fRtsControl: DWORD) -> ();
    fn set_fAbortOnError(&mut self, fAbortOnError: DWORD) -> ();

    fn fBinary(&self) -> DWORD;
    fn fParity(&self) -> DWORD;
    fn fOutxCtsFlow(&self) -> DWORD;
    fn fOutxDsrFlow(&self) -> DWORD;
    fn fDtrControl(&self) -> DWORD;
    fn fDsrSensitivity(&self) -> DWORD;
    fn fTXContinueOnXoff(&self) -> DWORD;
    fn fOutX(&self) -> DWORD;
    fn fInX(&self) -> DWORD;
    fn fErrorChar(&self) -> DWORD;
    fn fNull(&self) -> DWORD;
    fn fRtsControl(&self) -> DWORD;
    fn fAbortOnError(&self) -> DWORD;

}

impl MyTrait for DCB {
    fn set_fBinary(&mut self, f_binary: DWORD) -> () {
        self._bitfield =  (self._bitfield & !0b0000_0000_0000_0001) | (f_binary & 0x1);
    }
    fn set_fParity(&mut self, f_parity: DWORD) -> () {
        self._bitfield =  (self._bitfield & !0b0000_0000_0000_0010) | ((f_parity & 0x1) << 1);
    }
    fn set_fOutxCtsFlow(&mut self, f_outx_cts_flow: DWORD) -> () {
        self._bitfield =  (self._bitfield & !0b0000_0000_0000_0100) | ((f_outx_cts_flow & 0x1) << 2);
    }
    fn set_fOutxDsrFlow(&mut self, f_outx_dsr_flow: DWORD) -> () {
        self._bitfield =  (self._bitfield & !0b0000_0000_0000_1000) | ((f_outx_dsr_flow & 0x1) << 3);
    }
    fn set_fDtrControl(&mut self, f_dtr_control: DWORD) -> () {
        self._bitfield =  (self._bitfield & !0b0000_0000_0001_0000) | ((f_dtr_control & 0x1) << 4);
        self._bitfield =  (self._bitfield & !0b0000_0000_0010_0000) | ((f_dtr_control & 0x2) << 4);
    }
    fn set_fDsrSensitivity(&mut self, f_dsr_sensitivity: DWORD) -> () {
        self._bitfield =  (self._bitfield & !0b0000_0000_0100_0000) | ((f_dsr_sensitivity & 0x1) << 6);
    }
    fn set_fTXContinueOnXoff(&mut self, f_tx_continue_on_x_off: DWORD) -> () {
        self._bitfield =  (self._bitfield & !0b0000_0000_1000_0000) | ((f_tx_continue_on_x_off & 0x1) << 7);
    }
    fn set_fOutX(&mut self, f_out_x: DWORD) -> () {
        self._bitfield =  (self._bitfield & !0b0000_0001_0000_0000) | ((f_out_x & 0x1) << 8);
    }
    fn set_fInX(&mut self, f_in_x: DWORD) -> () {
        self._bitfield =  (self._bitfield & !0b0000_0010_0000_0000) | ((f_in_x & 0x1) << 9);
    }
    fn set_fErrorChar(&mut self, f_error_char: DWORD) -> () {
        self._bitfield =  (self._bitfield & !0b0000_0100_0000_0000) | ((f_error_char & 0x1) << 10);
    }
    fn set_fNull(&mut self, f_null: DWORD) -> () {
        self._bitfield =  (self._bitfield & !0b0000_1000_0000_0000) | ((f_null & 0x1) << 11);
    }
    fn set_fRtsControl(&mut self, f_rts_control: DWORD) -> () {
        self._bitfield =  (self._bitfield & !0b0001_0000_0000_0000) | ((f_rts_control & 0x1) << 12);
        self._bitfield =  (self._bitfield & !0b0010_0000_0000_0000) | ((f_rts_control & 0x2) << 12);
    }
    fn set_fAbortOnError(&mut self, f_abort_on_error: DWORD) -> () {
        self._bitfield =  (self._bitfield & !0b0100_0000_0000_0000) | ((f_abort_on_error & 0x1) << 14);
    }

    fn fBinary(&self) -> DWORD {
        let bit: u32 = self._bitfield & (1 << 0);
        bit
    }
    fn fParity(&self) -> DWORD {
        let bit: u32 = self._bitfield & (1 << 1);
        bit >> 1
    }
    fn fOutxCtsFlow(&self) -> DWORD {
        let bit: u32 = self._bitfield & (1 << 2);
        bit >> 2
    }
    fn fOutxDsrFlow(&self) -> DWORD {
        let bit: u32 = self._bitfield & (1 << 3);
        bit >> 3
    }
    fn fDtrControl(&self) -> DWORD {
        let bit1: u32 = self._bitfield & (1 << 4);
        let bit2: u32 = self._bitfield & (1 << 5);
        let bit:u32 = bit1 | bit2;
        bit >> 4
    }
    fn fDsrSensitivity(&self) -> DWORD {
        let bit: u32 = self._bitfield & (1 << 6);
        bit >> 6
    }
    fn fTXContinueOnXoff(&self) -> DWORD {
        let bit: u32 = self._bitfield & (1 << 7);
        bit >> 7
    }
    fn fOutX(&self) -> DWORD {
        let bit: u32 = self._bitfield & (1 << 8);
        bit >> 8
    }
    fn fInX(&self) -> DWORD {
        let bit: u32 = self._bitfield & (1 << 9);
        bit >> 9
    }
    fn fErrorChar(&self) -> DWORD {
        let bit: u32 = self._bitfield & (1 << 10);
        bit >> 10
    }
    fn fNull(&self) -> DWORD {
        let bit: u32 = self._bitfield & (1 << 11);
        bit >> 11
    }
    fn fRtsControl(&self) -> DWORD {
        let bit1: u32 = self._bitfield & (1 << 12);
        let bit2: u32 = self._bitfield & (1 << 13);
        let bit:u32 = bit1 | bit2;
        bit >> 12
    }
    fn fAbortOnError(&self) -> DWORD {
        let bit: u32 = self._bitfield & (1 << 14);
        bit >> 14
    }
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
