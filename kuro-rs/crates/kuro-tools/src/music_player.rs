use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rodio::{Decoder, OutputStream, Sink};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayError {
    NotConfigured,
    DeviceUnavailable,
    InvalidInput(String),
}

pub trait SinkLike {
    fn play(&mut self, path: &Path) -> Result<(), PlayError>;
    fn pause(&self);
    fn resume(&self);
    fn stop(&mut self);
    fn is_empty(&self) -> bool;
}

enum Cmd {
    Play(PathBuf, mpsc::Sender<Result<(), PlayError>>),
    Pause(mpsc::Sender<()>),
    Resume(mpsc::Sender<()>),
    Stop(mpsc::Sender<()>),
    Shutdown,
}

type SessionFn = Box<dyn FnMut() -> Result<Box<dyn SinkLike>, PlayError> + Send>;

struct RodioSession {
    _stream: OutputStream,
    sink: Sink,
}

impl SinkLike for RodioSession {
    fn play(&mut self, path: &Path) -> Result<(), PlayError> {
        let file = std::fs::File::open(path).map_err(|e| PlayError::InvalidInput(e.to_string()))?;
        let source = Decoder::new(std::io::BufReader::new(file)).map_err(|e| PlayError::InvalidInput(e.to_string()))?;
        self.sink.append(source);
        self.sink.play();
        Ok(())
    }
    fn pause(&self) {
        self.sink.pause();
    }
    fn resume(&self) {
        self.sink.play();
    }
    fn stop(&mut self) {
        self.sink.stop();
    }
    fn is_empty(&self) -> bool {
        self.sink.empty()
    }
}

fn rodio_sink() -> Result<Box<dyn SinkLike>, PlayError> {
    let (stream, handle) = OutputStream::try_default().map_err(|_| PlayError::DeviceUnavailable)?;
    let sink = Sink::try_new(&handle).map_err(|e| PlayError::InvalidInput(e.to_string()))?;
    Ok(Box::new(RodioSession { _stream: stream, sink }))
}

fn worker(rx: Receiver<Cmd>, mut factory: SessionFn) {
    let mut sink: Option<Box<dyn SinkLike>> = None;
    while let Ok(cmd) = rx.recv() {
        match cmd {
            Cmd::Play(path, reply) => {
                if sink.is_none() {
                    match factory() {
                        Ok(s) => sink = Some(s),
                        Err(e) => {
                            let _ = reply.send(Err(e));
                            continue;
                        }
                    }
                } else if let Some(s) = sink.as_mut() {
                    s.stop();
                }
                let result = sink.as_mut().expect("sink after open").play(&path);
                let _ = reply.send(result);
            }
            Cmd::Pause(ack) => {
                if let Some(s) = &sink {
                    s.pause();
                }
                let _ = ack.send(());
            }
            Cmd::Resume(ack) => {
                if let Some(s) = &sink {
                    s.resume();
                }
                let _ = ack.send(());
            }
            Cmd::Stop(ack) => {
                if let Some(mut s) = sink.take() {
                    s.stop();
                }
                let _ = ack.send(());
            }
            Cmd::Shutdown => break,
        }
    }
    if let Some(mut s) = sink {
        s.stop();
    }
}
pub struct MusicPlayer {
    tx: Option<SyncSender<Cmd>>,
    handle: Option<JoinHandle<()>>,
    factory: Option<SessionFn>,
}

impl Default for MusicPlayer {
    fn default() -> Self {
        Self::new_injectable(rodio_sink)
    }
}

impl MusicPlayer {
    pub fn new() -> Self {
        Self::default()
    }
    #[doc(hidden)]
    pub fn new_injectable<F>(factory: F) -> Self
    where
        F: FnMut() -> Result<Box<dyn SinkLike>, PlayError> + Send + 'static,
    {
        Self { tx: None, handle: None, factory: Some(Box::new(factory)) }
    }
    pub fn play(&mut self, path: &Path) -> Result<(), PlayError> {
        if !path.is_file() {
            return Err(PlayError::NotConfigured);
        }
        let (done_tx, done_rx) = mpsc::channel();
        self.post(Cmd::Play(path.to_path_buf(), done_tx))?;
        match done_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(r) => r,
            Err(RecvTimeoutError::Timeout) => Err(PlayError::InvalidInput("playback timed out".into())),
            Err(RecvTimeoutError::Disconnected) => Err(PlayError::InvalidInput("player stopped".into())),
        }
    }
    pub fn pause(&mut self) {
        let _ = self.control(Cmd::Pause);
    }
    pub fn resume(&mut self) {
        let _ = self.control(Cmd::Resume);
    }
    pub fn stop(&mut self) {
        let _ = self.control(Cmd::Stop);
    }
    fn ensure_thread(&mut self) {
        if self.tx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::sync_channel::<Cmd>(8);
        let factory = self.factory.take().expect("factory");
        let handle = thread::spawn(move || worker(rx, factory));
        self.tx = Some(tx);
        self.handle = Some(handle);
    }

    fn post(&mut self, cmd: Cmd) -> Result<(), PlayError> {
        self.ensure_thread();
        self.tx.as_ref().expect("thread").send(cmd).map_err(|_| PlayError::InvalidInput("player channel closed".into()))
    }

    fn control(&mut self, mk: impl FnOnce(mpsc::Sender<()>) -> Cmd) -> Result<(), PlayError> {
        if self.tx.is_none() {
            return Ok(());
        }
        let (done_tx, done_rx) = mpsc::channel();
        self.post(mk(done_tx))?;
        match done_rx.recv_timeout(Duration::from_millis(500)) {
            Ok(()) => Ok(()),
            Err(RecvTimeoutError::Timeout) => Err(PlayError::InvalidInput("control timed out".into())),
            Err(RecvTimeoutError::Disconnected) => Err(PlayError::InvalidInput("player stopped".into())),
        }
    }
}
impl Drop for MusicPlayer {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Cmd::Shutdown);
        }
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
