//! Neonize DLL loading and FFI symbol bindings.

use std::ffi::{c_char, c_int, c_void};
use std::path::{Path, PathBuf};

use libloading::Library;

type NeonizeFn = unsafe extern "C" fn(
    *const c_char,
    *const c_char,
    *const u8,
    c_int,
    c_int,
    *const c_char,
    Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
    Option<unsafe extern "C" fn(*mut c_void, bool)>,
    Option<unsafe extern "C" fn(*const c_char, *mut c_void, c_int, c_int)>,
    Option<unsafe extern "C" fn(*mut c_void, c_int)>,
    *const u8,
    c_int,
    *const u8,
    c_int,
    *const u8,
    c_int,
    *mut c_void,
) -> *mut c_void;

pub(crate) type SendMessageFn = unsafe extern "C" fn(*const c_char, *const u8, c_int, *const u8, c_int) -> *mut GoBytes;
pub(crate) type IsConnectedFn = unsafe extern "C" fn(*const c_char) -> bool;
pub(crate) type IsLoggedInFn = unsafe extern "C" fn(*const c_char) -> bool;
pub(crate) type StopFn = unsafe extern "C" fn(*const c_char);
pub(crate) type FreeBytesFn = unsafe extern "C" fn(*mut GoBytes);

#[repr(C)]
pub(crate) struct GoBytes {
    pub ptr: *mut u8,
    pub size: usize,
}

pub(crate) struct Api {
    pub _lib: Library,
    pub neonize: NeonizeFn,
    pub send_message: SendMessageFn,
    pub is_connected: IsConnectedFn,
    pub is_logged_in: IsLoggedInFn,
    pub stop: StopFn,
    pub free_bytes: FreeBytesFn,
}

pub(crate) fn dll_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(path) = std::env::var("KURO_NEONIZE_DLL") {
        out.push(PathBuf::from(path));
    }
    let local = kuro_core::KuroPaths::from_env().root.join("native").join("neonize-windows-amd64.dll");
    out.push(local);
    if let Ok(root) = std::env::var("KURO_ROOT") {
        out.push(
            PathBuf::from(root)
                .join(".venv")
                .join("Lib")
                .join("site-packages")
                .join("neonize")
                .join("neonize-windows-amd64.dll"),
        );
    }
    out.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join(".venv")
            .join("Lib")
            .join("site-packages")
            .join("neonize")
            .join("neonize-windows-amd64.dll"),
    );
    out
}

pub(crate) fn ensure_dll() -> Result<PathBuf, String> {
    let candidates = dll_candidates();
    if let Some(found) = candidates.into_iter().find(|p| p.is_file()) {
        let dest = kuro_core::KuroPaths::from_env().root.join("native").join("neonize-windows-amd64.dll");
        if found != dest {
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::copy(&found, &dest);
            if dest.is_file() {
                return Ok(dest);
            }
        }
        return Ok(found);
    }
    Err("I couldn't send that WhatsApp.".into())
}

pub(crate) fn load_api(path: &Path) -> Result<Api, String> {
    unsafe {
        let lib = Library::new(path).map_err(|_| "I couldn't send that WhatsApp.".to_string())?;
        let neonize = *lib.get::<NeonizeFn>(b"Neonize\0").map_err(|_| "I couldn't send that WhatsApp.".to_string())?;
        let send_message =
            *lib.get::<SendMessageFn>(b"SendMessage\0").map_err(|_| "I couldn't send that WhatsApp.".to_string())?;
        let is_connected =
            *lib.get::<IsConnectedFn>(b"IsConnected\0").map_err(|_| "I couldn't send that WhatsApp.".to_string())?;
        let is_logged_in =
            *lib.get::<IsLoggedInFn>(b"IsLoggedIn\0").map_err(|_| "I couldn't send that WhatsApp.".to_string())?;
        let stop = *lib.get::<StopFn>(b"Stop\0").map_err(|_| "I couldn't send that WhatsApp.".to_string())?;
        let free_bytes =
            *lib.get::<FreeBytesFn>(b"FreeBytesStruct\0").map_err(|_| "I couldn't send that WhatsApp.".to_string())?;
        Ok(Api { _lib: lib, neonize, send_message, is_connected, is_logged_in, stop, free_bytes })
    }
}
