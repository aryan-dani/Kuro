use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeskCall {
    Scene(String),
}

/// Port for desk smart-plug scenes. Live actuation stays outside this crate.
pub trait DeskControl: Send + Sync {
    fn apply_scene(&self, scene: &str) -> Result<(), String>;
}

#[derive(Debug, Default)]
pub struct NoopDeskControl;

impl DeskControl for NoopDeskControl {
    fn apply_scene(&self, _scene: &str) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct FakeDeskControl {
    calls: Mutex<Vec<DeskCall>>,
    fail: Mutex<Option<String>>,
}

impl FakeDeskControl {
    pub fn recorded(&self) -> Vec<DeskCall> {
        self.calls.lock().expect("fake desk calls").clone()
    }

    pub fn fail_with(&self, message: impl Into<String>) {
        *self.fail.lock().expect("fake desk fail") = Some(message.into());
    }
}

impl DeskControl for FakeDeskControl {
    fn apply_scene(&self, scene: &str) -> Result<(), String> {
        self.calls.lock().expect("fake desk calls").push(DeskCall::Scene(scene.to_string()));
        if let Some(msg) = self.fail.lock().expect("fake desk fail").clone() {
            return Err(msg);
        }
        Ok(())
    }
}
