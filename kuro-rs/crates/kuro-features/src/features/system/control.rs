use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemCall {
    VolumeSet(u8),
    Mute,
    Lock,
}

pub trait SystemControl: Send + Sync {
    fn volume_set(&self, level: u8);
    fn mute(&self);
    fn lock(&self);
}

#[derive(Debug, Default)]
pub struct NoopSystemControl;

impl SystemControl for NoopSystemControl {
    fn volume_set(&self, _level: u8) {}
    fn mute(&self) {}
    fn lock(&self) {}
}

#[derive(Debug, Default)]
pub struct FakeSystemControl {
    calls: Mutex<Vec<SystemCall>>,
}

impl FakeSystemControl {
    pub fn recorded(&self) -> Vec<SystemCall> {
        self.calls.lock().expect("fake system calls").clone()
    }
}

impl SystemControl for FakeSystemControl {
    fn volume_set(&self, level: u8) {
        self.calls.lock().expect("fake system calls").push(SystemCall::VolumeSet(level));
    }

    fn mute(&self) {
        self.calls.lock().expect("fake system calls").push(SystemCall::Mute);
    }

    fn lock(&self) {
        self.calls.lock().expect("fake system calls").push(SystemCall::Lock);
    }
}
