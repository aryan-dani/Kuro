//! Windows working-set trim after heavy model unload.

#[cfg(windows)]
pub fn trim_working_set() {
    use windows_sys::Win32::System::ProcessStatus::K32EmptyWorkingSet;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};

    unsafe extern "C" {
        fn _heapmin() -> i32;
    }
    unsafe {
        let _ = _heapmin();
        let process = GetCurrentProcess();
        let _ = SetProcessWorkingSetSize(process, usize::MAX, usize::MAX);
        let _ = K32EmptyWorkingSet(process);
    }
}

#[cfg(not(windows))]
pub fn trim_working_set() {}

/// Best-effort resident set size in bytes (Windows).
#[cfg(windows)]
pub fn working_set_bytes() -> Option<u64> {
    use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    unsafe {
        let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) == 0 {
            return None;
        }
        Some(counters.WorkingSetSize as u64)
    }
}

#[cfg(not(windows))]
pub fn working_set_bytes() -> Option<u64> {
    None
}
