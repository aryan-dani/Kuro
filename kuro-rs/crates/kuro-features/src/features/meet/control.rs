use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetRequest {
    pub contact: String,
    pub action: String,
}

/// Port for Google Meet creation and join. Live OAuth and HTTP stay in kuro-tools.
pub trait MeetControl: Send + Sync {
    fn create_meet(&self, contact: &str) -> Result<String, String>;
    fn join_meet(&self, contact: &str) -> Result<String, String>;
}

#[derive(Debug, Default)]
pub struct NoopMeetControl;

impl MeetControl for NoopMeetControl {
    /// Agent registry must not fake Meet success. Live create/join stay in kuro-tools.
    fn create_meet(&self, _contact: &str) -> Result<String, String> {
        Err("Meet is not wired through the agent. Say make a meet or join the meet.".into())
    }

    fn join_meet(&self, _contact: &str) -> Result<String, String> {
        Err("Meet is not wired through the agent. Say join the meet.".into())
    }
}

#[derive(Debug, Default)]
pub struct FakeMeetControl {
    calls: Mutex<Vec<MeetRequest>>,
    fail: Mutex<Option<String>>,
    open_reply: Mutex<Option<String>>,
    sent_reply: Mutex<Option<String>>,
    join_reply: Mutex<Option<String>>,
}

impl FakeMeetControl {
    pub fn recorded(&self) -> Vec<MeetRequest> {
        self.calls.lock().expect("fake meet calls").clone()
    }

    pub fn fail_with(&self, message: impl Into<String>) {
        *self.fail.lock().expect("fake meet fail") = Some(message.into());
    }

    pub fn set_open_reply(&self, reply: impl Into<String>) {
        *self.open_reply.lock().expect("fake meet open") = Some(reply.into());
    }

    pub fn set_sent_reply(&self, reply: impl Into<String>) {
        *self.sent_reply.lock().expect("fake meet sent") = Some(reply.into());
    }

    pub fn set_join_reply(&self, reply: impl Into<String>) {
        *self.join_reply.lock().expect("fake meet join") = Some(reply.into());
    }
}

impl MeetControl for FakeMeetControl {
    fn create_meet(&self, contact: &str) -> Result<String, String> {
        self.calls
            .lock()
            .expect("fake meet calls")
            .push(MeetRequest { contact: contact.to_string(), action: "create".into() });
        if let Some(msg) = self.fail.lock().expect("fake meet fail").clone() {
            return Err(msg);
        }
        let trimmed = contact.trim();
        if trimmed.is_empty() {
            Ok(self.open_reply.lock().expect("fake meet open").clone().unwrap_or_else(|| "The Meet is open.".into()))
        } else if let Some(reply) = self.sent_reply.lock().expect("fake meet sent").clone() {
            Ok(reply)
        } else {
            Ok(format!("Sent the Meet link to {trimmed}."))
        }
    }

    fn join_meet(&self, contact: &str) -> Result<String, String> {
        self.calls
            .lock()
            .expect("fake meet calls")
            .push(MeetRequest { contact: contact.to_string(), action: "join".into() });
        if let Some(msg) = self.fail.lock().expect("fake meet fail").clone() {
            return Err(msg);
        }
        Ok(self.join_reply.lock().expect("fake meet join").clone().unwrap_or_else(|| "The Meet is open.".into()))
    }
}
