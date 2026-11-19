use std::panic::PanicHookInfo;
use std::sync::Once;

static INSTALL: Once = Once::new();

/// Append panic location and message to the rotating engine log.
pub fn install_panic_hook() {
    INSTALL.call_once(|| {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info: &PanicHookInfo<'_>| {
            let paths = kuro_core::KuroPaths::from_env();
            let loc = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_else(|| "unknown".into());
            let msg = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic".into());
            kuro_core::append_rotating_log(&paths.log(), &format!("panic {loc}: {msg}"), 1_000_000);
            default(info);
        }));
    });
}
