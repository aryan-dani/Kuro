#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Activation {
    pub source_id: String,
}

pub trait Trigger: Send + Sync {
    fn id(&self) -> &str;
    fn poll(&mut self) -> Option<Activation>;
}
