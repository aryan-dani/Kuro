//! In-process WhatsApp send via the neonize Go bridge DLL (same session as Python).

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::wa_ffi::{Api, ensure_dll, load_api};

const LINK_SPOKEN: &str = "I opened a link code. On your phone, open WhatsApp, then Linked devices, then Link a device, and scan it. Then ask me again.";

/// DeviceProps bytes matching tools/wa_link.py `_props()` for os=Kuro / DESKTOP.
const DEVICE_PROPS: &[u8] = &[
    0x0a, 0x04, 0x4b, 0x75, 0x72, 0x6f, 0x18, 0x07, 0x20, 0x00, 0x2a, 0x08, 0x08, 0x00, 0x10, 0x01, 0x18, 0x10, 0x28,
    0x00,
];

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

fn session_path() -> PathBuf {
    kuro_core::KuroPaths::from_env().wa_session()
}

fn session_likely() -> bool {
    std::fs::metadata(session_path()).map(|m| m.len() > 4096).unwrap_or(false)
}

fn log_wa(line: &str) {
    kuro_core::append_rotating_log(&kuro_core::KuroPaths::from_env().log(), line, 1_000_000);
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
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe { on_qr_inner(qr_ptr) }));
}

unsafe fn on_qr_inner(qr_ptr: *mut c_void) {
    if qr_ptr.is_null() {
        return;
    }
    let c = unsafe { CStr::from_ptr(qr_ptr as *const c_char) };
    if let Ok(code) = c.to_str() {
        QR_FLAG.store(true, Ordering::SeqCst);
        show_qr(code);
    }
}

unsafe extern "C" fn on_login(_uuid: *mut c_void, _ok: bool) {
    let _ = catch_unwind(AssertUnwindSafe(|| {}));
}

unsafe extern "C" fn on_event(_name: *const c_char, _data: *mut c_void, _len: c_int, ev: c_int) {
    let _ = catch_unwind(AssertUnwindSafe(|| on_event_inner(ev)));
}

fn on_event_inner(ev: c_int) {
    // 3 = connected (matches tools/wa_link.py list_func index)
    if ev == 3 {
        READY_FLAG.store(true, Ordering::SeqCst);
        QR_FLAG.store(false, Ordering::SeqCst);
    }
    if ev == 6 || ev == 12 {
        READY_FLAG.store(false, Ordering::SeqCst);
    }
}

unsafe extern "C" fn on_log(_data: *mut c_void, _len: c_int) {
    let _ = catch_unwind(AssertUnwindSafe(|| {}));
}

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
                let result = send_one(&api, uuid, &digits, &text);
                let _ = reply.send(result);
            }
        }
    }
    unsafe {
        (api.stop)(uuid.as_ptr());
    }
}

fn send_one(api: &Api, uuid: &CString, digits: &str, text: &str) -> Result<(), String> {
    let msg = crate::wa_proto::encode_text_message(text);
    let phone = crate::wa_proto::encode_jid(digits, "s.whatsapp.net");
    let lid = lookup_lid(api, uuid, &phone);
    let used_lid = lid.is_some();
    let target = lid.unwrap_or_else(|| phone.clone());
    if call_send(api, uuid, &target, &msg) {
        log_wa(if used_lid { "whatsapp send ok via=lid" } else { "whatsapp send ok via=phone" });
        return Ok(());
    }
    if used_lid && call_send(api, uuid, &phone, &msg) {
        log_wa("whatsapp send ok via=phone");
        return Ok(());
    }
    log_wa("whatsapp send fail");
    Err("I couldn't send that WhatsApp.".into())
}

fn lookup_lid(api: &Api, uuid: &CString, phone: &[u8]) -> Option<Vec<u8>> {
    let get_lid = api.get_lid?;
    let bytes = unsafe {
        let ptr = get_lid(uuid.as_ptr(), phone.as_ptr(), phone.len() as c_int);
        if ptr.is_null() {
            return None;
        }
        let owned = if (*ptr).ptr.is_null() || (*ptr).size == 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts((*ptr).ptr, (*ptr).size).to_vec()
        };
        (api.free_bytes)(ptr);
        owned
    };
    let user = crate::wa_proto::lid_user(&bytes)?;
    Some(crate::wa_proto::encode_jid(&user, "lid"))
}

fn call_send(api: &Api, uuid: &CString, jid: &[u8], msg: &[u8]) -> bool {
    unsafe {
        let ptr = (api.send_message)(uuid.as_ptr(), jid.as_ptr(), jid.len() as c_int, msg.as_ptr(), msg.len() as c_int);
        if ptr.is_null() {
            return false;
        }
        let ok = !(*ptr).ptr.is_null() && (*ptr).size > 0 && {
            let slice = std::slice::from_raw_parts((*ptr).ptr, (*ptr).size);
            crate::wa_proto::proto_error(slice).is_none()
        };
        (api.free_bytes)(ptr);
        ok
    }
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
    if (st.qr_open.load(Ordering::SeqCst) || QR_FLAG.load(Ordering::SeqCst))
        && !(st.ready.load(Ordering::SeqCst) || READY_FLAG.load(Ordering::SeqCst))
    {
        return Err(LINK_SPOKEN.into());
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
