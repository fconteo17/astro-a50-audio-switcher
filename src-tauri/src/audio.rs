use windows::{
    core::*,
    Win32::Media::Audio::*,
    Win32::System::Com::*,
    Win32::System::Com::StructuredStorage::*,
    Win32::Foundation::*,
};
use std::ptr;

/// Audio endpoint role constants
const EROLE_MULTIMEDIA: ERole = ERole(0);
const EROLE_COMMUNICATIONS: ERole = ERole(1);
const EROLE_CONSOLE: ERole = ERole(2);

/// IPolicyConfig CLSID and IID (undocumented COM interface)
const CLSID_CPOLICYCONFIG: GUID = GUID::from_values(0x8BE54D09, 0x6B35, 0x4F37, &[0xBE, 0xCB, 0x07, 0xE5, 0xBA, 0x9A, 0x3B, 0x12]);
const IID_IPOLICYCONFIG: GUID = GUID::from_values(0x870AF99C, 0x171D, 0x4F9E, &[0xAF, 0x0D, 0xE6, 0x3D, 0xF4, 0x0C, 0x3B, 0xC9]);

/// An audio endpoint discovered on the system.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AudioEndpoint {
    pub id: String,
    pub name: String,
}

/// Enumerate all active render (playback) audio endpoints.
pub fn enumerate_endpoints() -> Result<Vec<AudioEndpoint>, String> {
    unsafe {
        CoInitialize(None).map_err(|e| format!("COM init failed: {}", e))?;
        let result = enumerate_endpoints_inner();
        CoUninitialize();
        result
    }
}

fn enumerate_endpoints_inner() -> Result<Vec<AudioEndpoint>, String> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&IMMDeviceEnumerator::IID, None, CLSCTX_ALL)
                .map_err(|e| format!("Failed to create IMMDeviceEnumerator: {}", e))?;

        let collection = enumerator
            .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
            .map_err(|e| format!("Failed to enumerate endpoints: {}", e))?;

        let count = collection.GetCount().map_err(|e| format!("GetCount failed: {}", e))?;

        let mut endpoints = Vec::new();
        for i in 0..count {
            let device = collection.Item(i).map_err(|e| format!("Item({}) failed: {}", i, e))?;
            let id = device.GetId().map_err(|e| format!("GetId failed: {}", e))?;
            let id_str = id.to_string().map_err(|e| format!("ToString failed: {}", e))?;

            let props = device.OpenPropertyStore(STGM_READ)
                .map_err(|e| format!("OpenPropertyStore failed: {}", e))?;

            let mut name = String::from("(unknown)");
            // PKEY_Device_FriendlyName = {A45C254E-DF1C-4EFD-A312-CB0D5A489D73}, pid=2
            let pkey = PROPERTYKEY {
                fmtid: GUID::from_values(0xA45C254E, 0xDF1C, 0x4EFD, &[0xA3, 0x12, 0xCB, 0x0D, 0x5A, 0x48, 0x9D, 0x73]),
                pid: 2,
            };

            if let Ok(prop) = props.GetValue(&pkey) {
                if let Ok(s) = prop.to_string() {
                    name = s;
                }
            }

            endpoints.push(AudioEndpoint { id: id_str, name });
        }

        Ok(endpoints)
    }
}

/// Set the default audio endpoint for a specific role using IPolicyConfig COM.
fn set_default_endpoint(device_id: &str, role: ERole) -> Result<(), String> {
    unsafe {
        let mut ptr: *mut core::ffi::c_void = ptr::null_mut();
        CoCreateInstance(
            &CLSID_CPOLICYCONFIG,
            None,
            CLSCTX_ALL,
            &IID_IPOLICYCONFIG,
            &mut ptr,
        )
        .map_err(|e| format!("CoCreateInstance IPolicyConfig failed: {}", e))?;

        if ptr.is_null() {
            return Err("IPolicyConfig pointer is null".to_string());
        }

        let device_id_wide: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
        let device_id_ptr = PCWSTR::from_raw(device_id_wide.as_ptr());

        // IPolicyConfig vtable layout (0-indexed from IUnknown):
        // 0: QueryInterface, 1: AddRef, 2: Release
        // 3-11: other IPolicyConfig methods
        // 12: SetDefaultEndpoint (method index 9 after IUnknown = vtable index 12)
        // Based on reverse-engineering by SoundSwitch, EarTrumpet, and other projects
        // using CPolicyConfigVista CLSID {8BE54D09-...}.
        // NOTE: If audio switching fails, try offset 13 (some Windows builds differ).
        let vtable = *(ptr as *const *const usize);
        let set_default_fn = *((vtable as usize + 12 * std::mem::size_of::<usize>()) as *const extern "system" fn(*mut core::ffi::c_void, PCWSTR, u32) -> HRESULT);

        let hr = set_default_fn(ptr, device_id_ptr, role.0);

        // Release the COM object (vtable index 2)
        let release_fn = *((vtable as usize + 2 * std::mem::size_of::<usize>()) as *const extern "system" fn(*mut core::ffi::c_void) -> u32);
        release_fn(ptr);

        hr.ok().map_err(|e| format!("SetDefaultEndpoint failed: {}", e))
    }
}

/// Switch audio devices for a dock state transition.
/// game_device_name and voice_device_name are friendly names from enumerate_endpoints.
/// If same_device is true, voice_device_name is ignored and game device is used for all roles.
pub fn switch_audio(
    game_device_name: &str,
    voice_device_name: &str,
    same_device: bool,
) -> Result<String, String> {
    let endpoints = enumerate_endpoints()?;

    let game_endpoint = endpoints
        .iter()
        .find(|e| e.name == game_device_name)
        .ok_or_else(|| format!("Game device not found: '{}'", game_device_name))?;

    // Set game device for multimedia and console roles
    set_default_endpoint(&game_endpoint.id, EROLE_MULTIMEDIA)?;
    set_default_endpoint(&game_endpoint.id, EROLE_CONSOLE)?;

    if same_device {
        // Same device for communications too
        set_default_endpoint(&game_endpoint.id, EROLE_COMMUNICATIONS)?;
    } else {
        let voice_endpoint = endpoints
            .iter()
            .find(|e| e.name == voice_device_name)
            .ok_or_else(|| format!("Voice device not found: '{}'", voice_device_name))?;
        set_default_endpoint(&voice_endpoint.id, EROLE_COMMUNICATIONS)?;
    }

    let voice_name = if same_device { game_device_name } else { voice_device_name };
    Ok(format!("Game: {}, Voice: {}", game_device_name, voice_name))
}
