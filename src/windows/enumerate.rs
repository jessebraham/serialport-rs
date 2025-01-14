use std::collections::HashSet;
// use std::ffi::NulError;
use std::{mem, ptr};

// use winapi::ctypes::c_void;
// use winapi::shared::guiddef::*;
// use winapi::shared::minwindef::*;
// use winapi::shared::winerror::*;
// use winapi::um::cfgmgr32::*;
// use winapi::um::cguid::GUID_NULL;
// use winapi::um::setupapi::*;
// use winapi::um::winnt::{KEY_READ, REG_SZ};
// use winapi::um::winreg::*;

// use std::os::raw::c_void;
use windows_sys::core::GUID;
use windows_sys::Win32::Media::KernelStreaming::GUID_NULL;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::*;
use windows_sys::Win32::Foundation::{FALSE, MAX_PATH, INVALID_HANDLE_VALUE, FILETIME};
use windows_sys::Win32::System::Registry::*;
//https://github.com/microsoft/windows-rs/issues/881
type DWORD = u32;

type ULONG = u32;
//in windows-sys the RegQueryValueExW -> u32
type HRESULT = u32;
//https://docs.rs/winapi/latest/winapi/shared/minwindef/type.PBYTE.html
type PBYTE = *mut u8;

use crate::{Error, ErrorKind, Result, SerialPortInfo, SerialPortType, UsbPortInfo};

//https://docs.rs/winapi/latest/winapi/shared/winerror/fn.FAILED.html
pub fn FAILED(hr: HRESULT) -> bool {
    hr < 0
}
//https://docs.rs/winapi/latest/winapi/shared/winerror/fn.SUCCEEDED.html
pub fn SUCCEEDED(hr: HRESULT) -> bool {
    hr >= 0
}

/// takes normal Rust `str` and outputs a null terminated UTF-16 encoded string
fn as_utf16(utf8: &str) -> Vec<u16> {
    utf8.encode_utf16().chain(Some(0)).collect()
}

/// takes a UTF-16 encoded slice (null termination not required)
/// and converts to a UTF8 Rust string. Trailing null chars are removed
fn from_utf16_lossy_trimmed(utf16: &[u16]) -> String {
    String::from_utf16_lossy(utf16)
        .trim_end_matches(0 as char)
        .to_string()
}

/// According to the MSDN docs, we should use SetupDiGetClassDevs, SetupDiEnumDeviceInfo
/// and SetupDiGetDeviceInstanceId in order to enumerate devices.
/// https://msdn.microsoft.com/en-us/windows/hardware/drivers/install/enumerating-installed-devices
fn get_ports_guids() -> Result<Vec<GUID>> {
    // SetupDiGetClassDevs returns the devices associated with a particular class of devices.
    // We want the list of devices which are listed as COM ports (generally those that show up in the
    // Device Manager as "Ports (COM & LPT)" which is otherwise known as the "Ports" class).
    //
    // The list of system defined classes can be found here:
    // https://learn.microsoft.com/en-us/windows-hardware/drivers/install/system-defined-device-setup-classes-available-to-vendors
    let class_names = ["Ports", "Modem"];
    let mut guids: Vec<GUID> = Vec::new();
    for class_name in class_names {
        let class_name_w = as_utf16(class_name);
        let mut num_guids: DWORD = 1; // Initially assume that there is only 1 guid per name.
        let class_start_idx = guids.len(); // start idx for this name (for potential resize with multiple guids)

        // first attempt with size == 1, second with the size returned from the first try
        for _ in 0..2 {
            guids.resize(class_start_idx + num_guids as usize, GUID_NULL);
            let guid_buffer = &mut guids[class_start_idx..];
            // Find out how many GUIDs are associated with this class name.  num_guids will tell us how many there actually are.
            let res = unsafe {
                SetupDiClassGuidsFromNameW(
                    class_name_w.as_ptr(),
                    guid_buffer.as_mut_ptr(),
                    guid_buffer.len() as DWORD,
                    &mut num_guids,
                )
            };
            if res == FALSE {
                return Err(Error::new(
                    ErrorKind::Unknown,
                    "Unable to determine number of Ports GUIDs",
                ));
            }
            let len_cmp = guid_buffer.len().cmp(&(num_guids as usize));
            // under allocated
            if len_cmp == std::cmp::Ordering::Less {
                continue; // retry
            }
            // allocation > required len
            else if len_cmp == std::cmp::Ordering::Greater {
                guids.truncate(class_start_idx + num_guids as usize);
            }
            break; // next name
        }
    }
    Ok(guids)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HwidMatches<'hwid> {
    vid: &'hwid str,
    pid: &'hwid str,
    serial: Option<&'hwid str>,
    interface: Option<&'hwid str>,
}

impl<'hwid> HwidMatches<'hwid> {
    fn new(hwid: &'hwid str) -> Option<Self> {
        // When we match something, update this so that we are always looking forward
        let mut hwid_tail = hwid;

        // VID_(?P<vid>[[:xdigit:]]{4})
        let vid_start = hwid.find("VID_")?;

        // We won't match for hex characters here. That can be done when parsed.
        let vid = hwid_tail.get(vid_start + 4..vid_start + 8)?;
        hwid_tail = hwid_tail.get(vid_start + 8..)?;

        // [&+]PID_(?P<pid>[[:xdigit:]]{4})
        let pid = if hwid_tail.starts_with("&PID_") || hwid_tail.starts_with("+PID_") {
            // We will let the hex parser fail if there are not hex digits.
            hwid_tail.get(5..9)?
        } else {
            return None;
        };
        hwid_tail = hwid_tail.get(9..)?;

        // (?:[&+]MI_(?P<iid>[[:xdigit:]]{2})){0,1}
        let iid = if hwid_tail.starts_with("&MI_") || hwid_tail.starts_with("+MI_") {
            // We will let the hex parser fail if there are not hex digits.
            let iid = hwid_tail.get(4..6);
            hwid_tail = hwid_tail.get(6..).unwrap_or(hwid_tail);

            iid
        } else {
            None
        };

        // ([\\+](?P<serial>\w+))? with slightly modified check for alphanumeric characters instead
        // of regex word character
        //
        // TODO: Fix returning no serial number at all for devices without one. The previous regex
        // and the code below return the first thing from the intance ID. See issue #203.
        let serial = if hwid_tail.starts_with('\\') || hwid_tail.starts_with('+') {
            hwid_tail.get(1..).and_then(|tail| {
                let index = tail
                    .char_indices()
                    .find(|&(_, char)| !char.is_alphanumeric())
                    .map(|(index, _)| index)
                    .unwrap_or(tail.len());
                tail.get(..index)
            })
        } else {
            None
        };

        Some(Self {
            vid,
            pid,
            serial,
            interface: iid,
        })
    }
}

/// Windows usb port information can be determined by the port's HWID string.
///
/// This function parses the HWID string using regex, and returns the USB port
/// information if the hardware ID can be parsed correctly. The manufacturer
/// and product names cannot be determined from the HWID string, so those are
/// set as None.
///
/// For composite USB devices, the HWID string will be for the interface. In
/// this case, the parent HWID string must be provided so that the correct
/// serial number can be determined.
///
/// Some HWID examples are:
///   - MicroPython pyboard:    USB\VID_F055&PID_9802\385435603432
///   - BlackMagic GDB Server:  USB\VID_1D50&PID_6018&MI_00\6&A694CA9&0&0000
///   - BlackMagic UART port:   USB\VID_1D50&PID_6018&MI_02\6&A694CA9&0&0002
///   - FTDI Serial Adapter:    FTDIBUS\VID_0403+PID_6001+A702TB52A\0000
fn parse_usb_port_info(hardware_id: &str, parent_hardware_id: Option<&str>) -> Option<UsbPortInfo> {
    let mut caps = HwidMatches::new(hardware_id)?;

    let interface = caps.interface.and_then(|m| u8::from_str_radix(m, 16).ok());

    if interface.is_some() {
        // If this is a composite device, we need to parse the parent's HWID to get the correct information.
        caps = HwidMatches::new(parent_hardware_id?)?;
    }

    Some(UsbPortInfo {
        vid: u16::from_str_radix(caps.vid, 16).ok()?,
        pid: u16::from_str_radix(caps.pid, 16).ok()?,
        serial_number: caps.serial.map(str::to_string),
        manufacturer: None,
        product: None,

        #[cfg(feature = "usbportinfo-interface")]
        interface,
    })
}

struct PortDevices {
    /// Handle to a device information set.
    hdi: HDEVINFO,

    /// Index used by iterator.
    dev_idx: DWORD,
}

impl PortDevices {
    // Creates PortDevices object which represents the set of devices associated with a particular
    // Ports class (given by `guid`).
    pub fn new(guid: &GUID) -> Self {

        //https://github.com/microsoft/windows-rs/issues/3093
        //in windows-sys 0.52.0
        let null:isize = 0;
        
        PortDevices {
            //in windows-sys 0.59.0
            // hdi: unsafe { SetupDiGetClassDevsW(guid, ptr::null(), ptr::null_mut(), DIGCF_PRESENT) },

            //in windows-sys 0.52.0
            hdi: unsafe { SetupDiGetClassDevsW(guid, ptr::null(), null, DIGCF_PRESENT) },
            dev_idx: 0,
        }
    }
}

impl Iterator for PortDevices {
    type Item = PortDevice;

    /// Iterator which returns a PortDevice from the set of PortDevices associated with a
    /// particular PortDevices class (guid).
    fn next(&mut self) -> Option<PortDevice> {
        let mut port_dev = PortDevice {
            hdi: self.hdi,
            devinfo_data: SP_DEVINFO_DATA {
                cbSize: mem::size_of::<SP_DEVINFO_DATA>() as DWORD,
                ClassGuid: GUID_NULL,
                DevInst: 0,
                Reserved: 0,
            },
        };
        let res =
            unsafe { SetupDiEnumDeviceInfo(self.hdi, self.dev_idx, &mut port_dev.devinfo_data) };
        if res == FALSE {
            None
        } else {
            self.dev_idx += 1;
            Some(port_dev)
        }
    }
}

impl Drop for PortDevices {
    fn drop(&mut self) {
        // Release the PortDevices object allocated in the constructor.
        unsafe {
            SetupDiDestroyDeviceInfoList(self.hdi);
        }
    }
}

struct PortDevice {
    /// Handle to a device information set.
    hdi: HDEVINFO,

    /// Information associated with this device.
    pub devinfo_data: SP_DEVINFO_DATA,
}

impl PortDevice {
    /// Retrieves the device instance id string associated with this device's parent.
    /// This is useful for determining the serial number of a composite USB device.
    fn parent_instance_id(&mut self) -> Option<String> {
        let mut result_buf = [0u16; MAX_PATH as usize];
        let mut parent_device_instance_id = 0;

        let res =
            unsafe { CM_Get_Parent(&mut parent_device_instance_id, self.devinfo_data.DevInst, 0) };
        if res == CR_SUCCESS {
            let buffer_len = result_buf.len() - 1;
            let res = unsafe {
                CM_Get_Device_IDW(
                    parent_device_instance_id,
                    result_buf.as_mut_ptr(),
                    buffer_len as ULONG,
                    0,
                )
            };

            if res == CR_SUCCESS {
                Some(from_utf16_lossy_trimmed(&result_buf))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Retrieves the device instance id string associated with this device. Some examples of
    /// instance id strings are:
    /// * MicroPython Board:  USB\VID_F055&PID_9802\385435603432
    /// * FTDI USB Adapter:   FTDIBUS\VID_0403+PID_6001+A702TB52A\0000
    /// * Black Magic Probe (Composite device with 2 UARTS):
    ///   * GDB Port:       USB\VID_1D50&PID_6018&MI_00\6&A694CA9&0&0000
    ///   * UART Port:      USB\VID_1D50&PID_6018&MI_02\6&A694CA9&0&0002
    ///
    /// Reference: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/device-instance-ids
    fn instance_id(&mut self) -> Option<String> {
        let mut result_buf = [0u16; MAX_DEVICE_ID_LEN as usize];
        let working_buffer_len = result_buf.len() - 1; // always null terminated
        let mut desired_result_len = 0; // possibly larger than the buffer
        let res = unsafe {
            SetupDiGetDeviceInstanceIdW(
                self.hdi,
                &mut self.devinfo_data,
                result_buf.as_mut_ptr(),
                working_buffer_len as DWORD,
                &mut desired_result_len,
            )
        };
        if res == FALSE {
            // Try to retrieve hardware id property.
            self.property(SPDRP_HARDWAREID)
        } else {
            let actual_result_len = working_buffer_len.min(desired_result_len as usize);
            Some(from_utf16_lossy_trimmed(&result_buf[..actual_result_len]))
        }
    }

    /// Retrieves the problem status of this device. For example, `CM_PROB_DISABLED` indicates
    /// the device has been disabled in Device Manager.
    fn problem(&mut self) -> Option<ULONG> {
        let mut status = 0;
        let mut problem_number = 0;
        let res = unsafe {
            CM_Get_DevNode_Status(
                &mut status,
                &mut problem_number,
                self.devinfo_data.DevInst,
                0,
            )
        };
        if res == CR_SUCCESS {
            Some(problem_number)
        } else {
            None
        }
    }

    // Retrieves the port name (i.e. COM6) associated with this device.
    pub fn name(&mut self) -> String {
        // https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdiopendevregkey
        let hkey = unsafe {
            SetupDiOpenDevRegKey(
                self.hdi,
                &mut self.devinfo_data,
                DICS_FLAG_GLOBAL,
                0,
                DIREG_DEV,
                KEY_READ,
            )
        };

        //in windows-sys 0.59.0
        // if hkey as *mut c_void == INVALID_HANDLE_VALUE {

        //in windows-sys 0.52.0
        if hkey as isize == INVALID_HANDLE_VALUE {
            // failed to open registry key. Return empty string as the failure case
            return String::new();
        }

        // https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regqueryvalueexw
        let mut port_name_buffer = [0u16; MAX_PATH as usize];
        let buffer_byte_len = 2 * port_name_buffer.len() as DWORD;
        let mut byte_len = buffer_byte_len;
        let mut value_type = 0;

        let value_name = as_utf16("PortName");
        let err = unsafe {
            RegQueryValueExW(
                hkey,
                value_name.as_ptr(),
                ptr::null_mut(),
                &mut value_type,
                port_name_buffer.as_mut_ptr() as *mut u8,
                &mut byte_len,
            )
        };
        unsafe { RegCloseKey(hkey) };
        if FAILED(err) {
            // failed to query registry for some reason. Return empty string as the failure case
            return String::new();
        }
        // https://learn.microsoft.com/en-us/windows/win32/sysinfo/registry-value-types
        if value_type != REG_SZ || byte_len % 2 != 0 || byte_len > buffer_byte_len {
            // read something but it wasn't the expected registry type
            return String::new();
        }
        // len of u16 chars, not bytes
        let len = buffer_byte_len as usize / 2;
        let port_name = &port_name_buffer[0..len];

        from_utf16_lossy_trimmed(port_name)
    }

    // Determines the port_type for this device, and if it's a USB port populate the various fields.
    pub fn port_type(&mut self) -> SerialPortType {
        self.instance_id()
            .map(|s| (s, self.parent_instance_id())) // Get parent instance id if it exists.
            .and_then(|(d, p)| parse_usb_port_info(&d, p.as_deref()))
            .map(|mut info: UsbPortInfo| {
                info.manufacturer = self.property(SPDRP_MFG);
                info.product = self.property(SPDRP_FRIENDLYNAME);
                SerialPortType::UsbPort(info)
            })
            .unwrap_or(SerialPortType::Unknown)
    }

    // Retrieves a device property and returns it, if it exists. Returns None if the property
    // doesn't exist.
    fn property(&mut self, property_id: DWORD) -> Option<String> {
        let mut value_type = 0;
        let mut property_buf = [0u16; MAX_PATH as usize];

        let res = unsafe {
            SetupDiGetDeviceRegistryPropertyW(
                self.hdi,
                &mut self.devinfo_data,
                property_id,
                &mut value_type,
                property_buf.as_mut_ptr() as PBYTE,
                property_buf.len() as DWORD,
                ptr::null_mut(),
            )
        };

        if res == FALSE || value_type != REG_SZ {
            return None;
        }

        // Using the unicode version of 'SetupDiGetDeviceRegistryProperty' seems to report the
        // entire mfg registry string. This typically includes some driver information that we should discard.
        // Example string: 'FTDI5.inf,%ftdi%;FTDI'
        from_utf16_lossy_trimmed(&property_buf)
            .split(';')
            .last()
            .map(str::to_string)
    }
}

/// Not all COM ports are listed under the "Ports" device class
/// The full list of COM ports is available from the registry at
/// HKEY_LOCAL_MACHINE\HARDWARE\DEVICEMAP\SERIALCOMM
///
/// port of https://learn.microsoft.com/en-us/windows/win32/sysinfo/enumerating-registry-subkeys
fn get_registry_com_ports() -> HashSet<String> {
    let mut ports_list = HashSet::new();

    let reg_key = as_utf16("HARDWARE\\DEVICEMAP\\SERIALCOMM");
    let key_ptr = reg_key.as_ptr();

    //https://github.com/microsoft/windows-rs/issues/3093
    //in windows-sys 0.59.0
    // let mut ports_key = std::ptr::null_mut();

    //in windows-sys 0.52.0
    let mut ports_key:isize = 0;

    // SAFETY: ffi, all inputs are correct
    let open_res =
        unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, key_ptr, 0, KEY_READ, &mut ports_key) };
    if SUCCEEDED(open_res) {
        let mut class_name_buff = [0u16; MAX_PATH as usize];
        let mut class_name_size = MAX_PATH as u32;
        let mut sub_key_count = 0;
        let mut largest_sub_key = 0;
        let mut largest_class_string = 0;
        let mut num_key_values = 0;
        let mut longest_value_name = 0;
        let mut longest_value_data = 0;
        let mut size_security_desc = 0;
        let mut last_write_time = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        // SAFETY: ffi, all inputs are correct
        let query_res = unsafe {
            RegQueryInfoKeyW(
                ports_key,
                class_name_buff.as_mut_ptr(),
                &mut class_name_size,
                std::ptr::null_mut(),
                &mut sub_key_count,
                &mut largest_sub_key,
                &mut largest_class_string,
                &mut num_key_values,
                &mut longest_value_name,
                &mut longest_value_data,
                &mut size_security_desc,
                &mut last_write_time,
            )
        };
        if SUCCEEDED(query_res) {
            for idx in 0..num_key_values {
                let mut val_name_buff = [0u16; MAX_PATH as usize];
                let mut val_name_size = MAX_PATH as u32;
                let mut value_type = 0;
                let mut val_data = [0u16; MAX_PATH as usize];
                let buffer_byte_len = 2 * val_data.len() as DWORD; // len doubled
                let mut byte_len = buffer_byte_len;

                // SAFETY: ffi, all inputs are correct
                let res = unsafe {
                    RegEnumValueW(
                        ports_key,
                        idx,
                        val_name_buff.as_mut_ptr(),
                        &mut val_name_size,
                        std::ptr::null_mut(),
                        &mut value_type,
                        val_data.as_mut_ptr() as *mut u8,
                        &mut byte_len,
                    )
                };
                if FAILED(res)
                    || value_type != REG_SZ // only valid for text values
                    || byte_len % 2 != 0 // out byte len should be a multiple of u16 size
                    || byte_len > buffer_byte_len
                {
                    break;
                }
                // key data is returned as u16
                // SAFETY: data_size is checked and pointer is valid
                let val_data = from_utf16_lossy_trimmed(unsafe {
                    let utf16_len = byte_len / 2; // utf16 len
                    std::slice::from_raw_parts(val_data.as_ptr(), utf16_len as usize)
                });
                ports_list.insert(val_data);
            }
        }
        // SAFETY: ffi, all inputs are correct
        unsafe { RegCloseKey(ports_key) };
    }
    ports_list
}

/// List available serial ports on the system.
pub fn available_ports() -> Result<Vec<SerialPortInfo>> {
    let mut ports = Vec::new();
    for guid in get_ports_guids()? {
        let port_devices = PortDevices::new(&guid);
        for mut port_device in port_devices {
            // Ignore nonfunctional devices
            if port_device.problem() != Some(0) {
                continue;
            }

            let port_name = port_device.name();

            debug_assert!(
                port_name.as_bytes().last().map_or(true, |c| *c != b'\0'),
                "port_name has a trailing nul: {:?}",
                port_name
            );

            // This technique also returns parallel ports, so we filter these out.
            if port_name.starts_with("LPT") {
                continue;
            }

            ports.push(SerialPortInfo {
                port_name,
                port_type: port_device.port_type(),
            });
        }
    }
    // ports identified through the registry have no additional information
    let mut raw_ports_set = get_registry_com_ports();
    if raw_ports_set.len() > ports.len() {
        // remove any duplicates. HashSet makes this relatively cheap
        for port in ports.iter() {
            raw_ports_set.remove(&port.port_name);
        }
        // add remaining ports as "unknown" type
        for raw_port in raw_ports_set {
            ports.push(SerialPortInfo {
                port_name: raw_port,
                port_type: SerialPortType::Unknown,
            })
        }
    }
    Ok(ports)
}

#[cfg(test)]
mod tests {
    use super::*;

    use quickcheck_macros::quickcheck;

    #[test]
    fn from_utf16_lossy_trimmed_trimming_empty() {
        assert_eq!("", from_utf16_lossy_trimmed(&[]));
        assert_eq!("", from_utf16_lossy_trimmed(&[0]));
    }

    #[test]
    fn from_utf16_lossy_trimmed_trimming() {
        let test_str = "Testing";
        let wtest_str: Vec<u16> = as_utf16(test_str);
        let wtest_str_trailing = wtest_str
            .iter()
            .copied()
            .chain([0, 0, 0, 0]) // add some null chars
            .collect::<Vec<_>>();
        let and_back = from_utf16_lossy_trimmed(&wtest_str_trailing);

        assert_eq!(test_str, and_back);
    }

    // Check that passing some random data to HwidMatches::new() does not cause a panic.
    #[quickcheck]
    fn quickcheck_hwidmatches_new_does_not_panic_from_random_input(hwid: String) -> bool {
        let _ = HwidMatches::new(&hwid);
        true
    }

    // Corner cases which might not always represent what we want to/should parse. But they at
    // least illustrate how we are parsing device identification strings today.
    #[test]
    fn test_hwidmatches_new_corner_cases() {
        assert!(HwidMatches::new("").is_none());
        assert!(HwidMatches::new("ROOT").is_none());
        assert!(HwidMatches::new("ROOT\\").is_none());
        assert!(HwidMatches::new("USB\\").is_none());
        assert!(HwidMatches::new("USB\\VID_1234").is_none());
        assert!(HwidMatches::new("USB\\PID_1234").is_none());
        assert!(HwidMatches::new("USB\\MI_12").is_none());

        assert_eq!(
            HwidMatches::new("VID_1234&PID_5678").unwrap(),
            HwidMatches {
                vid: "1234",
                pid: "5678",
                serial: None,
                interface: None,
            }
        );

        assert_eq!(
            HwidMatches::new("ABC\\VID_1234&PID_5678&MI_90").unwrap(),
            HwidMatches {
                vid: "1234",
                pid: "5678",
                serial: None,
                interface: Some("90"),
            }
        );

        assert_eq!(
            HwidMatches::new("FTDIBUS\\VID_1234&PID_5678&MI_90").unwrap(),
            HwidMatches {
                vid: "1234",
                pid: "5678",
                serial: None,
                interface: Some("90"),
            }
        );

        assert_eq!(
            HwidMatches::new("USB\\VID_1234+PID_5678+MI_90").unwrap(),
            HwidMatches {
                vid: "1234",
                pid: "5678",
                serial: None,
                interface: Some("90"),
            }
        );

        assert_eq!(
            HwidMatches::new("FTDIBUS\\VID_1234+PID_5678\\0000").unwrap(),
            HwidMatches {
                vid: "1234",
                pid: "5678",
                serial: Some("0000"),
                interface: None,
            }
        );
    }

    #[test]
    fn test_hwidmatches_new_standard_cases_ftdi() {
        assert_eq!(
            HwidMatches::new("FTDIBUS\\VID_1234+PID_5678+SERIAL123\\0000").unwrap(),
            HwidMatches {
                vid: "1234",
                pid: "5678",
                serial: Some("SERIAL123"),
                interface: None,
            }
        );
    }

    #[test]
    fn test_hwidmatches_new_standard_cases_usb() {
        assert_eq!(
            HwidMatches::new("USB\\VID_1234&PID_5678").unwrap(),
            HwidMatches {
                vid: "1234",
                pid: "5678",
                serial: None,
                interface: None,
            }
        );

        assert_eq!(
            HwidMatches::new("USB\\VID_1234&PID_5678&MI_90").unwrap(),
            HwidMatches {
                vid: "1234",
                pid: "5678",
                serial: None,
                interface: Some("90"),
            }
        );

        assert_eq!(
            HwidMatches::new("USB\\VID_1234&PID_5678\\SERIAL123").unwrap(),
            HwidMatches {
                vid: "1234",
                pid: "5678",
                serial: Some("SERIAL123"),
                interface: None,
            }
        );

        assert_eq!(
            HwidMatches::new("USB\\VID_1234&PID_5678&MI_90\\SERIAL123").unwrap(),
            HwidMatches {
                vid: "1234",
                pid: "5678",
                serial: Some("SERIAL123"),
                interface: Some("90"),
            }
        );
    }

    #[test]
    fn test_parsing_usb_port_information() {
        let madeup_hwid = r"USB\VID_1D50&PID_6018+6&A694CA9&0&0000";
        let info = parse_usb_port_info(madeup_hwid, None).unwrap();
        // TODO: Fix returning no serial at all for devices without one. See issue #203.
        assert_eq!(info.serial_number, Some("6".to_string()));

        let bm_uart_hwid = r"USB\VID_1D50&PID_6018&MI_02\6&A694CA9&0&0000";
        let bm_parent_hwid = r"USB\VID_1D50&PID_6018\85A12F01";
        let info = parse_usb_port_info(bm_uart_hwid, Some(bm_parent_hwid)).unwrap();

        assert_eq!(info.vid, 0x1D50);
        assert_eq!(info.pid, 0x6018);
        assert_eq!(info.serial_number, Some("85A12F01".to_string()));
        #[cfg(feature = "usbportinfo-interface")]
        assert_eq!(info.interface, Some(2));

        let ftdi_serial_hwid = r"FTDIBUS\VID_0403+PID_6001+A702TB52A\0000";
        let info = parse_usb_port_info(ftdi_serial_hwid, None).unwrap();

        assert_eq!(info.vid, 0x0403);
        assert_eq!(info.pid, 0x6001);
        assert_eq!(info.serial_number, Some("A702TB52A".to_string()));
        #[cfg(feature = "usbportinfo-interface")]
        assert_eq!(info.interface, None);

        let pyboard_hwid = r"USB\VID_F055&PID_9802\385435603432";
        let info = parse_usb_port_info(pyboard_hwid, None).unwrap();

        assert_eq!(info.vid, 0xF055);
        assert_eq!(info.pid, 0x9802);
        assert_eq!(info.serial_number, Some("385435603432".to_string()));
        #[cfg(feature = "usbportinfo-interface")]
        assert_eq!(info.interface, None);

        let unicode_serial = r"USB\VID_F055&PID_9802\3854356β03432&test";
        let info = parse_usb_port_info(unicode_serial, None).unwrap();
        assert_eq!(info.serial_number.as_deref(), Some("3854356β03432"));

        let unicode_serial = r"USB\VID_F055&PID_9802\3854356β03432";
        let info = parse_usb_port_info(unicode_serial, None).unwrap();
        assert_eq!(info.serial_number.as_deref(), Some("3854356β03432"));

        let unicode_serial = r"USB\VID_F055&PID_9802\3854356β";
        let info = parse_usb_port_info(unicode_serial, None).unwrap();
        assert_eq!(info.serial_number.as_deref(), Some("3854356β"));
    }
}
