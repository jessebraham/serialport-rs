//! serialport-rs is a cross-platform serial port library.
//!
//! The goal of this library is to expose a cross-platform and platform-specific API for enumerating
//! and using blocking I/O with serial ports. This library exposes a similar API to that provided
//! by [Qt's `QSerialPort` library](https://doc.qt.io/qt-5/qserialport.html).
//!
//! # Feature Overview
//!
//! The library has been organized such that there is a high-level `SerialPort` trait that provides
//! a cross-platform API for accessing serial ports. This is the preferred method of interacting
//! with ports and as such is part of the `prelude`. The `open*()` and `available_ports()` functions
//! in the root provide cross-platform functionality.
//!
//! For platform-specific functionaly, this crate is split into a `posix` and `windows` API with
//! corresponding `TTYPort` and `COMPort` structs (that both implement the `SerialPort` trait).
//! Using the platform-specific `open*()` functions will return the platform-specific port object
//! which allows access to platform-specific functionality.

#![deny(
    missing_docs,
    missing_debug_implementations,
    missing_copy_implementations,
    unused
)]
// Don't worry about needing to `unwrap()` or otherwise handle some results in
// doc tests.
#![doc(test(attr(allow(unused_must_use))))]

use std::convert::From;
use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::time::Duration;

#[cfg(unix)]
mod posix;
#[cfg(unix)]
pub use posix::{BreakDuration, TTYPort};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::COMPort;

/// A type for results generated by interacting with serial ports
///
/// The `Err` type is hard-wired to [`serialport::Error`](struct.Error.html).
pub type Result<T> = std::result::Result<T, Error>;

/// Categories of errors that can occur when interacting with serial ports
///
/// This list is intended to grow over time and it is not recommended to
/// exhaustively match against it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// The device is not available.
    ///
    /// This could indicate that the device is in use by another process or was
    /// disconnected while performing I/O.
    NoDevice,

    /// A parameter was incorrect.
    InvalidInput,

    /// An unknown error occurred.
    Unknown,

    /// An I/O error occurred.
    ///
    /// The type of I/O error is determined by the inner `io::ErrorKind`.
    Io(io::ErrorKind),
}

/// An error type for serial port operations
#[derive(Debug)]
pub struct Error {
    /// The kind of error this is
    pub kind: ErrorKind,
    /// A description of the error suitable for end-users
    pub description: String,
}

impl Error {
    /// Instantiates a new error
    pub fn new<T: Into<String>>(kind: ErrorKind, description: T) -> Self {
        Error {
            kind,
            description: description.into(),
        }
    }

    /// Returns the corresponding `ErrorKind` for this error.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        fmt.write_str(&self.description)
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        &self.description
    }
}

impl From<io::Error> for Error {
    fn from(io_error: io::Error) -> Error {
        Error::new(ErrorKind::Io(io_error.kind()), format!("{}", io_error))
    }
}

impl From<Error> for io::Error {
    fn from(error: Error) -> io::Error {
        let kind = match error.kind {
            ErrorKind::NoDevice => io::ErrorKind::NotFound,
            ErrorKind::InvalidInput => io::ErrorKind::InvalidInput,
            ErrorKind::Unknown => io::ErrorKind::Other,
            ErrorKind::Io(kind) => kind,
        };

        io::Error::new(kind, error.description)
    }
}

/// Number of bits per character
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DataBits {
    /// 5 bits per character
    Five,

    /// 6 bits per character
    Six,

    /// 7 bits per character
    Seven,

    /// 8 bits per character
    Eight,
}

/// Parity checking modes
///
/// When parity checking is enabled (`Odd` or `Even`) an extra bit is transmitted with
/// each character. The value of the parity bit is arranged so that the number of 1 bits in the
/// character (including the parity bit) is an even number (`Even`) or an odd number
/// (`Odd`).
///
/// Parity checking is disabled by setting `None`, in which case parity bits are not
/// transmitted.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Parity {
    /// No parity bit.
    None,

    /// Parity bit sets odd number of 1 bits.
    Odd,

    /// Parity bit sets even number of 1 bits.
    Even,
}

/// Number of stop bits
///
/// Stop bits are transmitted after every character.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum StopBits {
    /// One stop bit.
    One,

    /// Two stop bits.
    Two,
}

/// Flow control modes
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FlowControl {
    /// No flow control.
    None,

    /// Flow control using XON/XOFF bytes.
    Software,

    /// Flow control using RTS/CTS signals.
    Hardware,
}

/// Specifies which buffer or buffers to purge when calling [`clear`]
///
/// [`clear`]: trait.SerialPort.html#tymethod.clear
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ClearBuffer {
    /// Specify to clear data received but not read
    Input,
    /// Specify to clear data written but not yet transmitted
    Output,
    /// Specify to clear both data received and data not yet transmitted
    All,
}

/// A struct containing all serial port settings
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialPortBuilder {
    /// The port name, usually the device path
    path: String,
    /// The baud rate in symbols-per-second
    baud_rate: u32,
    /// Number of bits used to represent a character sent on the line
    data_bits: DataBits,
    /// The type of signalling to use for controlling data transfer
    flow_control: FlowControl,
    /// The type of parity to use for error checking
    parity: Parity,
    /// Number of bits to use to signal the end of a character
    stop_bits: StopBits,
    /// Amount of time to wait to receive data before timing out
    timeout: Duration,
}

impl SerialPortBuilder {
    /// Set the path to the serial port
    pub fn path<'a>(mut self, path: impl Into<std::borrow::Cow<'a, str>>) -> Self {
        self.path = path.into().as_ref().to_owned();
        self
    }

    /// Set the baud rate in symbols-per-second
    pub fn baud_rate(mut self, baud_rate: u32) -> Self {
        self.baud_rate = baud_rate;
        self
    }

    /// Set the number of bits used to represent a character sent on the line
    pub fn data_bits(mut self, data_bits: DataBits) -> Self {
        self.data_bits = data_bits;
        self
    }

    /// Set the type of signalling to use for controlling data transfer
    pub fn flow_control(mut self, flow_control: FlowControl) -> Self {
        self.flow_control = flow_control;
        self
    }

    /// Set the type of parity to use for error checking
    pub fn parity(mut self, parity: Parity) -> Self {
        self.parity = parity;
        self
    }

    /// Set the number of bits to use to signal the end of a character
    pub fn stop_bits(mut self, stop_bits: StopBits) -> Self {
        self.stop_bits = stop_bits;
        self
    }

    /// Set the amount of time to wait to receive data before timing out
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Open a cross-platform interface to the port with the specified settings
    pub fn open(self) -> Result<Box<dyn SerialPort>> {
        #[cfg(unix)]
        return posix::TTYPort::open(&self).map(|p| Box::new(p) as Box<dyn SerialPort>);

        #[cfg(windows)]
        return windows::COMPort::open(&self).map(|p| Box::new(p) as Box<dyn SerialPort>);

        #[cfg(not(any(unix, windows)))]
        Err(Error::new(
            ErrorKind::Unknown,
            "open() not implemented for platform",
        ))
    }

    /// Open a platform-specific interface to the port with the specified settings
    #[cfg(unix)]
    pub fn open_native(self) -> Result<TTYPort> {
        posix::TTYPort::open(&self)
    }

    /// Open a platform-specific interface to the port with the specified settings
    #[cfg(windows)]
    pub fn open_native(self) -> Result<COMPort> {
        windows::COMPort::open(&self)
    }
}

/// A trait for serial port devices
///
/// This trait is all that's necessary to implement a new serial port driver
/// for a new platform.
pub trait SerialPort: Send + io::Read + io::Write {
    // Port settings getters

    /// Returns the name of this port if it exists.
    ///
    /// This name may not be the canonical device name and instead be shorthand.
    /// Additionally it may not exist for virtual ports.
    fn name(&self) -> Option<String>;

    /// Returns the current baud rate.
    ///
    /// This may return a value different from the last specified baud rate depending on the
    /// platform as some will return the actual device baud rate rather than the last specified
    /// baud rate.
    fn baud_rate(&self) -> Result<u32>;

    /// Returns the character size.
    ///
    /// This function returns `None` if the character size could not be determined. This may occur
    /// if the hardware is in an uninitialized state or is using a non-standard character size.
    /// Setting a baud rate with `set_char_size()` should initialize the character size to a
    /// supported value.
    fn data_bits(&self) -> Result<DataBits>;

    /// Returns the flow control mode.
    ///
    /// This function returns `None` if the flow control mode could not be determined. This may
    /// occur if the hardware is in an uninitialized state or is using an unsupported flow control
    /// mode. Setting a flow control mode with `set_flow_control()` should initialize the flow
    /// control mode to a supported value.
    fn flow_control(&self) -> Result<FlowControl>;

    /// Returns the parity-checking mode.
    ///
    /// This function returns `None` if the parity mode could not be determined. This may occur if
    /// the hardware is in an uninitialized state or is using a non-standard parity mode. Setting
    /// a parity mode with `set_parity()` should initialize the parity mode to a supported value.
    fn parity(&self) -> Result<Parity>;

    /// Returns the number of stop bits.
    ///
    /// This function returns `None` if the number of stop bits could not be determined. This may
    /// occur if the hardware is in an uninitialized state or is using an unsupported stop bit
    /// configuration. Setting the number of stop bits with `set_stop-bits()` should initialize the
    /// stop bits to a supported value.
    fn stop_bits(&self) -> Result<StopBits>;

    /// Returns the current timeout.
    fn timeout(&self) -> Duration;

    // Port settings setters

    /// Sets the baud rate.
    ///
    /// ## Errors
    ///
    /// If the implementation does not support the requested baud rate, this function may return an
    /// `InvalidInput` error. Even if the baud rate is accepted by `set_baud_rate()`, it may not be
    /// supported by the underlying hardware.
    fn set_baud_rate(&mut self, baud_rate: u32) -> Result<()>;

    /// Sets the character size.
    fn set_data_bits(&mut self, data_bits: DataBits) -> Result<()>;

    /// Sets the flow control mode.
    fn set_flow_control(&mut self, flow_control: FlowControl) -> Result<()>;

    /// Sets the parity-checking mode.
    fn set_parity(&mut self, parity: Parity) -> Result<()>;

    /// Sets the number of stop bits.
    fn set_stop_bits(&mut self, stop_bits: StopBits) -> Result<()>;

    /// Sets the timeout for future I/O operations.
    fn set_timeout(&mut self, timeout: Duration) -> Result<()>;

    // Functions for setting non-data control signal pins

    /// Sets the state of the RTS (Request To Send) control signal.
    ///
    /// Setting a value of `true` asserts the RTS control signal. `false` clears the signal.
    ///
    /// ## Errors
    ///
    /// This function returns an error if the RTS control signal could not be set to the desired
    /// state on the underlying hardware:
    ///
    /// * `NoDevice` if the device was disconnected.
    /// * `Io` for any other type of I/O error.
    fn write_request_to_send(&mut self, level: bool) -> Result<()>;

    /// Writes to the Data Terminal Ready pin
    ///
    /// Setting a value of `true` asserts the DTR control signal. `false` clears the signal.
    ///
    /// ## Errors
    ///
    /// This function returns an error if the DTR control signal could not be set to the desired
    /// state on the underlying hardware:
    ///
    /// * `NoDevice` if the device was disconnected.
    /// * `Io` for any other type of I/O error.
    fn write_data_terminal_ready(&mut self, level: bool) -> Result<()>;

    // Functions for reading additional pins

    /// Reads the state of the CTS (Clear To Send) control signal.
    ///
    /// This function returns a boolean that indicates whether the CTS control signal is asserted.
    ///
    /// ## Errors
    ///
    /// This function returns an error if the state of the CTS control signal could not be read
    /// from the underlying hardware:
    ///
    /// * `NoDevice` if the device was disconnected.
    /// * `Io` for any other type of I/O error.
    fn read_clear_to_send(&mut self) -> Result<bool>;

    /// Reads the state of the Data Set Ready control signal.
    ///
    /// This function returns a boolean that indicates whether the DSR control signal is asserted.
    ///
    /// ## Errors
    ///
    /// This function returns an error if the state of the DSR control signal could not be read
    /// from the underlying hardware:
    ///
    /// * `NoDevice` if the device was disconnected.
    /// * `Io` for any other type of I/O error.
    fn read_data_set_ready(&mut self) -> Result<bool>;

    /// Reads the state of the Ring Indicator control signal.
    ///
    /// This function returns a boolean that indicates whether the RI control signal is asserted.
    ///
    /// ## Errors
    ///
    /// This function returns an error if the state of the RI control signal could not be read from
    /// the underlying hardware:
    ///
    /// * `NoDevice` if the device was disconnected.
    /// * `Io` for any other type of I/O error.
    fn read_ring_indicator(&mut self) -> Result<bool>;

    /// Reads the state of the Carrier Detect control signal.
    ///
    /// This function returns a boolean that indicates whether the CD control signal is asserted.
    ///
    /// ## Errors
    ///
    /// This function returns an error if the state of the CD control signal could not be read from
    /// the underlying hardware:
    ///
    /// * `NoDevice` if the device was disconnected.
    /// * `Io` for any other type of I/O error.
    fn read_carrier_detect(&mut self) -> Result<bool>;

    /// Gets the number of bytes available to be read from the input buffer.
    ///
    /// # Errors
    ///
    /// This function may return the following errors:
    ///
    /// * `NoDevice` if the device was disconnected.
    /// * `Io` for any other type of I/O error.
    fn bytes_to_read(&self) -> Result<u32>;

    /// Get the number of bytes written to the output buffer, awaiting transmission.
    ///
    /// # Errors
    ///
    /// This function may return the following errors:
    ///
    /// * `NoDevice` if the device was disconnected.
    /// * `Io` for any other type of I/O error.
    fn bytes_to_write(&self) -> Result<u32>;

    /// Discards all bytes from the serial driver's input buffer and/or output buffer.
    ///
    /// # Errors
    ///
    /// This function may return the following errors:
    ///
    /// * `NoDevice` if the device was disconnected.
    /// * `Io` for any other type of I/O error.
    fn clear(&self, buffer_to_clear: ClearBuffer) -> Result<()>;

    // Misc methods

    /// Attempts to clone the `SerialPort`. This allow you to write and read simultaneously from the
    /// same serial connection. Please note that if you want a real asynchronous serial port you
    /// should look at [mio-serial](https://crates.io/crates/mio-serial) or
    /// [tokio-serial](https://crates.io/crates/tokio-serial).
    ///
    /// Also, you must be very carefull when changing the settings of a cloned `SerialPort` : since
    /// the settings are cached on a per object basis, trying to modify them from two different
    /// objects can cause some nasty behavior.
    ///
    /// # Errors
    ///
    /// This function returns an error if the serial port couldn't be cloned.
    fn try_clone(&self) -> Result<Box<dyn SerialPort>>;

    /// Start transmitting a break
    fn set_break(&self) -> Result<()>;

    /// Stop transmitting a break
    fn clear_break(&self) -> Result<()>;
}

/// Contains all possible USB information about a `SerialPort`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbPortInfo {
    /// Vendor ID
    pub vid: u16,
    /// Product ID
    pub pid: u16,
    /// Serial number (arbitrary string)
    pub serial_number: Option<String>,
    /// Manufacturer (arbitrary string)
    pub manufacturer: Option<String>,
    /// Product name (arbitrary string)
    pub product: Option<String>,
    /// Manufacturer from database (arbitrary string)
    pub manufacturer_database: Option<String>,
    /// Product name from database (arbitrary string)
    pub product_database: Option<String>,
}

/// The physical type of a `SerialPort`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerialPortType {
    /// The serial port is connected via USB
    UsbPort(UsbPortInfo),
    /// The serial port is connected via PCI (permanent port)
    PciPort,
    /// The serial port is connected via Bluetooth
    BluetoothPort,
    /// It can't be determined how the serial port is connected
    Unknown,
}

/// A device-independent implementation of serial port information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialPortInfo {
    /// The short name of the serial port
    pub port_name: String,
    /// The hardware device type that exposes this port
    pub port_type: SerialPortType,
}

/// Construct a builder of `SerialPort` objects
///
/// `SerialPort` objects are built using the Builder pattern through the `new` function. The
/// resultant `SerialPortBuilder` object can be copied, reconfigured, and saved making working with
/// multiple serial ports a little easier.
///
/// To open a new serial port:
/// ```fail
/// serialport::new("/dev/ttyUSB0", 9600).open().expect("Failed to open port");
/// ```
pub fn new<'a>(path: impl Into<std::borrow::Cow<'a, str>>, baud_rate: u32) -> SerialPortBuilder {
    SerialPortBuilder {
        path: path.into().into_owned(),
        baud_rate,
        data_bits: DataBits::Eight,
        flow_control: FlowControl::None,
        parity: Parity::None,
        stop_bits: StopBits::One,
        timeout: Duration::from_millis(0),
    }
}

/// Returns a list of all serial ports on system
///
/// It is not guaranteed that these ports exist or are available even if they're
/// returned by this function.
pub fn available_ports() -> Result<Vec<SerialPortInfo>> {
    #[cfg(unix)]
    return crate::posix::available_ports();

    #[cfg(windows)]
    return crate::windows::available_ports();

    #[cfg(not(any(unix, windows)))]
    Err(Error::new(
        ErrorKind::Unknown,
        "available_ports() not implemented for platform",
    ))
}
