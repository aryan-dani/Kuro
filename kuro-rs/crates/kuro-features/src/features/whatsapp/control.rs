use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhatsappRequest {
    pub contact: String,
    pub source: String,
    pub text: String,
}

/// Port for WhatsApp sends. Live neonize actuation stays in kuro-tools.
pub trait WhatsappControl: Send + Sync {
    fn send(&self, contact: &str, source: &str, text: &str) -> Result<String, String>;
}

#[derive(Debug, Default)]
pub struct NoopWhatsappControl;

impl WhatsappControl for NoopWhatsappControl {
    fn send(&self, _contact: &str, _source: &str, _text: &str) -> Result<String, String> {
        Ok("Sent.".into())
    }
}

#[derive(Debug, Default)]
pub struct FakeWhatsappControl {
    calls: Mutex<Vec<WhatsappRequest>>,
    fail: Mutex<Option<String>>,
    ok_reply: Mutex<Option<String>>,
}

impl FakeWhatsappControl {
    pub fn recorded(&self) -> Vec<WhatsappRequest> {
        self.calls.lock().expect("fake whatsapp calls").clone()
    }

    pub fn fail_with(&self, message: impl Into<String>) {
        *self.fail.lock().expect("fake whatsapp fail") = Some(message.into());
    }

    pub fn set_ok_reply(&self, reply: impl Into<String>) {
        *self.ok_reply.lock().expect("fake whatsapp ok") = Some(reply.into());
    }
}

impl WhatsappControl for FakeWhatsappControl {
    fn send(&self, contact: &str, source: &str, text: &str) -> Result<String, String> {
        self.calls.lock().expect("fake whatsapp calls").push(WhatsappRequest {
            contact: contact.to_string(),
            source: source.to_string(),
            text: text.to_string(),
        });
        if let Some(msg) = self.fail.lock().expect("fake whatsapp fail").clone() {
            return Err(msg);
        }
        Ok(self.ok_reply.lock().expect("fake whatsapp ok").clone().unwrap_or_else(|| "Sent.".into()))
    }
}
