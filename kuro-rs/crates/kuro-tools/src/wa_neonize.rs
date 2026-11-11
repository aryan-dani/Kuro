//! In-process WhatsApp send via the neonize Go bridge DLL (same session as Python).

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use libloading::Library;

const LINK_SPOKEN: &str = "I opened a link code. On your phone, open WhatsApp, then Linked devices, then Link a device, and scan it. Then ask me again.";

/// DeviceProps bytes matching tools/wa_link.py `_props()` for os=Kuro / DESKTOP.
const DEVICE_PROPS: &[u8] = &[
    0x0a, 0x04, 0x4b, 0x75, 0x72, 0x6f, 0x18, 0x07, 0x20, 0x00, 0x2a, 0x08, 0x08, 0x00, 0x10, 0x01, 0x18, 0x10, 0x28,
    0x00,
];

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

type SendMessageFn = unsafe extern "C" fn(*const c_char, *const u8, c_int, *const u8, c_int) -> *mut GoBytes;
type IsConnectedFn = unsafe extern "C" fn(*const c_char) -> bool;
type IsLoggedInFn = unsafe extern "C" fn(*const c_char) -> bool;
type StopFn = unsafe extern "C" fn(*const c_char);
type FreeBytesFn = unsafe extern "C" fn(*mut GoBytes);

#[repr(C)]
struct GoBytes {
    ptr: *mut u8,
    size: usize,
}

struct Api {
    _lib: Library,
    neonize: NeonizeFn,
    send_message: SendMessageFn,
    is_connected: IsConnectedFn,
    is_logged_in: IsLoggedInFn,
    stop: StopFn,
    free_bytes: FreeBytesFn,
}

enum Cmd {
    Send { digits: String, text: String, reply: Sender<Result<(), String>> },
}

struct State {
    ready: AtomicBool,
    qr_open: AtomicBool,
    tx: Mutex<Option<Sender<Cmd>>>,
}

static STATE: OnceLock<Arc<State>> = OnceLock::new();

fn state() -> Arc<State> {
    STATE
        .get_or_init(|| {
            let st = Arc::new(State {
                ready: AtomicBool::new(false),
                qr_open: AtomicBool::new(false),
                tx: Mutex::new(None),
            });
            let (tx, rx) = mpsc::channel::<Cmd>();
            *st.tx.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
            let worker = st.clone();
            thread::Builder::new().name("kuro-wa".into()).spawn(move || worker_main(worker, rx)).ok();
            st
        })
        .clone()
}

fn dll_candidates() -> Vec<PathBuf> {
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

fn ensure_dll() -> Result<PathBuf, String> {
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

fn load_api(path: &Path) -> Result<Api, String> {
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

fn session_path() -> PathBuf {
    kuro_core::KuroPaths::from_env().wa_session()
}

fn session_likely() -> bool {
    std::fs::metadata(session_path()).map(|m| m.len() > 4096).unwrap_or(false)
}

fn pb_varint(mut v: u64, out: &mut Vec<u8>) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn pb_bytes(field: u32, data: &[u8], out: &mut Vec<u8>) {
    pb_varint(u64::from((field << 3) | 2), out);
    pb_varint(data.len() as u64, out);
    out.extend_from_slice(data);
}

fn pb_string(field: u32, s: &str, out: &mut Vec<u8>) {
    pb_bytes(field, s.as_bytes(), out);
}

fn pb_varint_field(field: u32, v: u64, out: &mut Vec<u8>) {
    pb_varint(u64::from(field << 3), out);
    pb_varint(v, out);
}

fn encode_jid(digits: &str) -> Vec<u8> {
    let mut out = Vec::new();
    pb_string(1, digits, &mut out);
    pb_varint_field(2, 0, &mut out);
    pb_varint_field(3, 0, &mut out);
    pb_varint_field(4, 0, &mut out);
    pb_string(5, "s.whatsapp.net", &mut out);
    pb_varint_field(6, 0, &mut out);
    out
}

fn encode_text_message(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    pb_string(1, text, &mut out);
    out
}

fn show_qr(code: &str) {
    let folder = kuro_core::KuroPaths::from_env().root;
    let _ = std::fs::create_dir_all(&folder);
    let path = folder.join("wa_link.txt");
    let _ = std::fs::write(&path, code);
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", &path.to_string_lossy()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

static QR_FLAG: AtomicBool = AtomicBool::new(false);
static READY_FLAG: AtomicBool = AtomicBool::new(false);

unsafe extern "C" fn on_qr(_uuid: *mut c_void, qr_ptr: *mut c_void) {
    if qr_ptr.is_null() {
        return;
    }
    let c = unsafe { CStr::from_ptr(qr_ptr as *const c_char) };
    if let Ok(code) = c.to_str() {
        QR_FLAG.store(true, Ordering::SeqCst);
        show_qr(code);
    }
}

unsafe extern "C" fn on_login(_uuid: *mut c_void, _ok: bool) {}

unsafe extern "C" fn on_event(_name: *const c_char, _data: *mut c_void, _len: c_int, ev: c_int) {
    // 3 = connected (matches tools/wa_link.py list_func index)
    if ev == 3 {
        READY_FLAG.store(true, Ordering::SeqCst);
        QR_FLAG.store(false, Ordering::SeqCst);
    }
    if ev == 6 || ev == 12 {
        READY_FLAG.store(false, Ordering::SeqCst);
    }
}

unsafe extern "C" fn on_log(_data: *mut c_void, _len: c_int) {}

fn worker_main(st: Arc<State>, rx: Receiver<Cmd>) {
    let Ok(dll) = ensure_dll() else {
        return;
    };
    let Ok(api) = load_api(&dll) else {
        return;
    };
    let session = session_path();
    if let Some(parent) = session.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Neonize blocks until Stop; keep these pointers alive for the whole worker.
    let name = Box::leak(Box::new(CString::new(session.to_string_lossy().as_bytes()).unwrap_or_default()));
    let uuid = Box::leak(Box::new(CString::new("kuro").unwrap_or_default()));
    let level = Box::leak(Box::new(CString::new("INFO").unwrap_or_default()));
    let subscribers: &'static [u8] = Box::leak(Box::new([3_u8, 6, 12]));
    let props: &'static [u8] = DEVICE_PROPS;

    let neonize = api.neonize;
    // Pass pointer addresses as usize so the join thread is Send.
    let name_addr = name.as_ptr() as usize;
    let uuid_addr = uuid.as_ptr() as usize;
    let level_addr = level.as_ptr() as usize;
    let subs_addr = subscribers.as_ptr() as usize;
    let props_addr = props.as_ptr() as usize;
    let subs_len = subscribers.len() as c_int;
    let props_len = props.len() as c_int;

    thread::spawn(move || unsafe {
        neonize(
            name_addr as *const c_char,
            uuid_addr as *const c_char,
            std::ptr::null(),
            0,
            0,
            level_addr as *const c_char,
            Some(on_qr),
            Some(on_login),
            Some(on_event),
            Some(on_log),
            subs_addr as *const u8,
            subs_len,
            props_addr as *const u8,
            props_len,
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
        );
    });

    let poll = Instant::now();
    while poll.elapsed() < Duration::from_secs(20) {
        if READY_FLAG.load(Ordering::SeqCst) || unsafe { (api.is_connected)(uuid.as_ptr()) } {
            READY_FLAG.store(true, Ordering::SeqCst);
            break;
        }
        if QR_FLAG.load(Ordering::SeqCst) {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    st.ready.store(READY_FLAG.load(Ordering::SeqCst), Ordering::SeqCst);
    st.qr_open.store(QR_FLAG.load(Ordering::SeqCst), Ordering::SeqCst);

    while let Ok(cmd) = rx.recv() {
        match cmd {
            Cmd::Send { digits, text, reply } => {
                st.ready.store(READY_FLAG.load(Ordering::SeqCst), Ordering::SeqCst);
                st.qr_open.store(QR_FLAG.load(Ordering::SeqCst), Ordering::SeqCst);
                let connected = unsafe { (api.is_connected)(uuid.as_ptr()) } || READY_FLAG.load(Ordering::SeqCst);
                if !connected {
                    let _ = reply.send(if QR_FLAG.load(Ordering::SeqCst) || st.qr_open.load(Ordering::SeqCst) {
                        Err(LINK_SPOKEN.into())
                    } else {
                        Err("WhatsApp is still linking. Try again in a moment.".into())
                    });
                    continue;
                }
                let jid = encode_jid(&digits);
                let msg = encode_text_message(&text);
                let result = unsafe {
                    let ptr = (api.send_message)(
                        uuid.as_ptr(),
                        jid.as_ptr(),
                        jid.len() as c_int,
                        msg.as_ptr(),
                        msg.len() as c_int,
                    );
                    if ptr.is_null() {
                        Err("I couldn't send that WhatsApp.".into())
                    } else {
                        let slice = std::slice::from_raw_parts((*ptr).ptr, (*ptr).size);
                        // SendMessageReturnFunction: Error is a string field; empty bytes => ok.
                        let err = parse_send_error(slice);
                        (api.free_bytes)(ptr);
                        if err.is_empty() { Ok(()) } else { Err("I couldn't send that WhatsApp.".into()) }
                    }
                };
                let _ = reply.send(result);
            }
        }
    }
    unsafe {
        (api.stop)(uuid.as_ptr());
    }
}

fn parse_send_error(bytes: &[u8]) -> String {
    // Best-effort: if protobuf contains a long ascii error string, surface failure.
    if bytes.is_empty() {
        return String::new();
    }
    // Field Error on return protos is typically a string; look for printable runs.
    let ascii: String = bytes.iter().map(|&b| if (32..127).contains(&b) { b as char } else { ' ' }).collect();
    let trimmed = ascii.split_whitespace().collect::<Vec<_>>().join(" ");
    if trimmed.len() > 8 && (trimmed.contains("error") || trimmed.contains("Error") || trimmed.contains("fail")) {
        return trimmed;
    }
    String::new()
}

/// Send plain text to a phone number (digits only). Speaks the same lines as tools/wa_link.py.
pub fn deliver_text(digits: &str, text: &str) -> Result<(), String> {
    if matches!(
        std::env::var("KURO_WA_DRY").ok().as_deref().map(str::to_ascii_lowercase).as_deref(),
        Some("1" | "true" | "yes" | "on")
    ) {
        return Err("I couldn't send that WhatsApp.".into());
    }
    let st = state();
    let started = Instant::now();
    let limit = if session_likely() { Duration::from_secs(12) } else { Duration::from_secs(6) };
    while started.elapsed() < limit {
        if st.ready.load(Ordering::SeqCst) || READY_FLAG.load(Ordering::SeqCst) {
            break;
        }
        if (st.qr_open.load(Ordering::SeqCst) || QR_FLAG.load(Ordering::SeqCst))
            && started.elapsed() > Duration::from_millis(1200)
        {
            return Err(LINK_SPOKEN.into());
        }
        thread::sleep(Duration::from_millis(50));
    }
    if !(st.ready.load(Ordering::SeqCst) || READY_FLAG.load(Ordering::SeqCst)) {
        if st.qr_open.load(Ordering::SeqCst) || QR_FLAG.load(Ordering::SeqCst) {
            return Err(LINK_SPOKEN.into());
        }
        return Err("WhatsApp is still linking. Try again in a moment.".into());
    }
    let (reply_tx, reply_rx) = mpsc::channel();
    let tx = st.tx.lock().unwrap_or_else(|e| e.into_inner());
    let Some(tx) = tx.as_ref() else {
        return Err("I couldn't send that WhatsApp.".into());
    };
    tx.send(Cmd::Send { digits: digits.to_string(), text: text.to_string(), reply: reply_tx })
        .map_err(|_| "I couldn't send that WhatsApp.".to_string())?;
    reply_rx.recv_timeout(Duration::from_secs(30)).map_err(|_| "I couldn't send that WhatsApp.".to_string())?
}

#[allow(dead_code)]
fn _logged_in_hint(api: &Api, uuid: &CString) -> bool {
    unsafe { (api.is_logged_in)(uuid.as_ptr()) }
}
