use rusb::{DeviceHandle, UsbContext};
use std::time::Duration;

// A50 Protocol constants (matching the Python version)
pub const VID: u16 = 0x9886;
pub const PID: u16 = 0x002C;
pub const INTERFACE: u8 = 6;
pub const EP_OUT: u8 = 0x05;
pub const EP_IN: u8 = 0x85;
pub const TIMEOUT_MS: Duration = Duration::from_millis(3000);

pub const CMD_GET_HEADSET_STATUS: u8 = 0x54;
pub const CMD_GET_BATTERY_STATUS: u8 = 0x7C;

pub const DEVICE_NAME: &str = "Astro Gaming - Astro A50";

#[derive(Debug, Clone)]
pub struct HeadsetStatus {
    pub is_docked: bool,
    pub is_on: bool,
}

#[derive(Debug, Clone)]
pub struct BatteryStatus {
    pub is_charging: bool,
    pub charge_percent: u8,
}

pub struct DeviceInfo {
    pub handle: DeviceHandle<rusb::Context>,
    pub was_detached: bool,
}

/// Find and claim the A50 USB device.
pub fn open_device() -> Result<DeviceInfo, String> {
    let context = rusb::Context::new().map_err(|e| e.to_string())?;

    let handle = context
        .open_device_with_vid_pid(VID, PID)
        .ok_or("A50 device not found")?;

    let mut was_detached = false;

    // Try to detach kernel driver (may not be needed on WinUSB)
    match handle.kernel_driver_active(INTERFACE) {
        Ok(true) => {
            handle
                .detach_kernel_driver(INTERFACE)
                .map_err(|e| e.to_string())?;
            was_detached = true;
        }
        _ => {}
    }

    handle
        .claim_interface(INTERFACE)
        .map_err(|e| e.to_string())?;

    Ok(DeviceInfo {
        handle,
        was_detached,
    })
}

/// Release the A50 USB device. Returns the first error if any step fails.
pub fn release_device(info: DeviceInfo) -> Result<(), String> {
    let mut last_err = None;

    if let Err(e) = info.handle.release_interface(INTERFACE) {
        last_err = Some(format!("Failed to release interface: {}", e));
    }

    if info.was_detached {
        if let Err(e) = info.handle.attach_kernel_driver(INTERFACE) {
            last_err = Some(format!("Failed to reattach kernel driver: {}", e));
        }
    }

    match last_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Send a protocol request and return the response payload.
fn send_request(
    handle: &DeviceHandle<rusb::Context>,
    command: u8,
    payload: Option<&[u8]>,
) -> Option<Vec<u8>> {
    let mut request = vec![0x02, command];
    if let Some(p) = payload {
        request.push(p.len() as u8);
        request.extend_from_slice(p);
    } else {
        request.push(0);
    }

    if handle.write_bulk(EP_OUT, &request, TIMEOUT_MS).is_err() {
        return None;
    }

    let mut buf = [0u8; 64];
    match handle.read_bulk(EP_IN, &mut buf, TIMEOUT_MS) {
        Ok(read_len) => {
            if read_len >= 4 && buf[0] == 0x02 {
                let length = std::cmp::min(buf[2] as usize, read_len.saturating_sub(3));
                Some(buf[3..3 + length].to_vec())
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

/// Query headset dock/on status. Returns HeadsetStatus or None on failure.
pub fn get_headset_status(handle: &DeviceHandle<rusb::Context>) -> Option<HeadsetStatus> {
    let resp = send_request(handle, CMD_GET_HEADSET_STATUS, None)?;
    if !resp.is_empty() {
        Some(HeadsetStatus {
            is_docked: resp[0] & 0x01 != 0,
            is_on: resp[0] & 0x02 != 0,
        })
    } else {
        None
    }
}

/// Query battery status. Returns BatteryStatus or None on failure.
pub fn get_battery_status(handle: &DeviceHandle<rusb::Context>) -> Option<BatteryStatus> {
    let resp = send_request(handle, CMD_GET_BATTERY_STATUS, None)?;
    if !resp.is_empty() {
        Some(BatteryStatus {
            is_charging: resp[0] & 0x80 != 0,
            charge_percent: resp[0] & 0x7F,
        })
    } else {
        None
    }
}

/// Perform a single poll: open, query both statuses, release.
/// Returns (docked, on, charging, battery%, device_name) or error string.
pub fn poll_once() -> Result<(bool, bool, bool, u8, String), String> {
    let info = open_device()?;
    let result = (|| {
        let status = get_headset_status(&info.handle);
        let battery = get_battery_status(&info.handle);
        match (status, battery) {
            (Some(s), Some(b)) => Ok((s.is_docked, s.is_on, b.is_charging, b.charge_percent, DEVICE_NAME.to_string())),
            _ => Err("Failed to read device status".to_string()),
        }
    })();
    if let Err(e) = release_device(info) {
        // Log but don't overwrite the actual poll result
        eprintln!("USB release warning: {}", e);
    }
    result
}
