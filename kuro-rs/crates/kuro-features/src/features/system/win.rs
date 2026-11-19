//! Windows `SystemControl` implementation lands in a later slice.

#![allow(dead_code)]

#[cfg(windows)]
#[derive(Debug, Default)]
pub struct WinSystemControl;

#[cfg(windows)]
impl WinSystemControl {
    pub fn new() -> Self {
        Self
    }
}
