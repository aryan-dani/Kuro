use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MusicCall {
    Pause,
    Resume,
    Stop,
}

/// Port for music transport. Live playback stays outside this crate.
pub trait MusicControl: Send + Sync {
    fn pause(&self) -> Result<String, String>;
    fn resume(&self) -> Result<String, String>;
    fn stop(&self) -> Result<String, String>;
}

#[derive(Debug, Default)]
pub struct NoopMusicControl;

impl MusicControl for NoopMusicControl {
    fn pause(&self) -> Result<String, String> {
        Ok("Nothing is playing.".into())
    }

    fn resume(&self) -> Result<String, String> {
        Ok("Nothing to resume.".into())
    }

    fn stop(&self) -> Result<String, String> {
        Ok("Music stopped.".into())
    }
}

#[derive(Debug, Default)]
pub struct FakeMusicControl {
    calls: Mutex<Vec<MusicCall>>,
    fail: Mutex<Option<String>>,
    pause_reply: Mutex<Option<String>>,
    resume_reply: Mutex<Option<String>>,
    stop_reply: Mutex<Option<String>>,
}

impl FakeMusicControl {
    pub fn recorded(&self) -> Vec<MusicCall> {
        self.calls.lock().expect("fake music calls").clone()
    }

    pub fn fail_with(&self, message: impl Into<String>) {
        *self.fail.lock().expect("fake music fail") = Some(message.into());
    }

    pub fn set_pause_reply(&self, reply: impl Into<String>) {
        *self.pause_reply.lock().expect("fake music pause") = Some(reply.into());
    }

    pub fn set_resume_reply(&self, reply: impl Into<String>) {
        *self.resume_reply.lock().expect("fake music resume") = Some(reply.into());
    }

    pub fn set_stop_reply(&self, reply: impl Into<String>) {
        *self.stop_reply.lock().expect("fake music stop") = Some(reply.into());
    }
}

impl MusicControl for FakeMusicControl {
    fn pause(&self) -> Result<String, String> {
        self.calls.lock().expect("fake music calls").push(MusicCall::Pause);
        if let Some(msg) = self.fail.lock().expect("fake music fail").clone() {
            return Err(msg);
        }
        Ok(self.pause_reply.lock().expect("fake music pause").clone().unwrap_or_else(|| "Nothing is playing.".into()))
    }

    fn resume(&self) -> Result<String, String> {
        self.calls.lock().expect("fake music calls").push(MusicCall::Resume);
        if let Some(msg) = self.fail.lock().expect("fake music fail").clone() {
            return Err(msg);
        }
        Ok(self.resume_reply.lock().expect("fake music resume").clone().unwrap_or_else(|| "Nothing to resume.".into()))
    }

    fn stop(&self) -> Result<String, String> {
        self.calls.lock().expect("fake music calls").push(MusicCall::Stop);
        if let Some(msg) = self.fail.lock().expect("fake music fail").clone() {
            return Err(msg);
        }
        Ok(self.stop_reply.lock().expect("fake music stop").clone().unwrap_or_else(|| "Music stopped.".into()))
    }
}
