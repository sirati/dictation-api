//! Two-sided SDK for applications and dictation engines.
pub use dictation_protocol::*;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    net::{Shutdown, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
        mpsc,
    },
    time::Duration,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Broker(#[from] ApiError),
    #[error("broker disconnected")]
    Disconnected,
    #[error("broker request timed out")]
    Timeout,
    #[error("invalid broker response")]
    InvalidResponse,
}
pub type Result<T> = std::result::Result<T, Error>;
type Pending = Arc<Mutex<HashMap<u32, mpsc::Sender<std::result::Result<Reply, ApiError>>>>>;

pub fn default_home() -> PathBuf {
    if let Some(path) = std::env::var_os("DICTATION_API_HOME") {
        return path.into();
    }
    let dirs = directories::ProjectDirs::from("org", "dictationapi", "dictation-api")
        .expect("user home directory");
    dirs.runtime_dir()
        .unwrap_or_else(|| dirs.data_local_dir())
        .to_path_buf()
}
pub fn default_endpoint() -> PathBuf {
    default_home().join("endpoint.json")
}

struct Inner {
    writer: Mutex<TcpStream>,
    pending: Pending,
    events: Mutex<mpsc::Receiver<Event>>,
    next: AtomicU32,
}
impl Drop for Inner {
    fn drop(&mut self) {
        if let Ok(stream) = self.writer.lock() {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

#[derive(Clone)]
pub struct Connection(Arc<Inner>);
impl Connection {
    /// Connect using the broker's private discovery file. No network discovery is used.
    pub fn connect(endpoint: Option<&Path>) -> Result<Self> {
        let path = endpoint
            .map(Path::to_path_buf)
            .unwrap_or_else(default_endpoint);
        let endpoint: Endpoint = serde_json::from_slice(&std::fs::read(path)?)?;
        if !endpoint.address.ip().is_loopback() || endpoint.version != PROTOCOL_VERSION {
            return Err(Error::InvalidResponse);
        }
        let mut stream = TcpStream::connect_timeout(&endpoint.address, Duration::from_secs(3))?;
        stream.set_nodelay(true)?;
        serde_json::to_writer(
            &mut stream,
            &Hello {
                version: PROTOCOL_VERSION,
                token: endpoint.token,
            },
        )?;
        stream.write_all(b"\n")?;
        let read_stream = stream.try_clone()?;
        let pending: Pending = Arc::default();
        let pending_reader = pending.clone();
        let (events_tx, events_rx) = mpsc::sync_channel(256);
        std::thread::Builder::new()
            .name("dictation-api-reader".into())
            .spawn(move || {
                let mut reader = BufReader::new(read_stream);
                loop {
                    let mut line = String::new();
                    let count = match reader
                        .by_ref()
                        .take((MAX_FRAME_BYTES + 1) as u64)
                        .read_line(&mut line)
                    {
                        Ok(n) => n,
                        Err(_) => break,
                    };
                    if count == 0 || count > MAX_FRAME_BYTES || !line.ends_with('\n') {
                        break;
                    }
                    match serde_json::from_str::<Message>(&line) {
                        Ok(Message::Response { id, reply, error }) => {
                            if let Some(sender) = pending_reader.lock().unwrap().remove(&id) {
                                let result = match (reply, error) {
                                    (Some(reply), None) => Ok(reply),
                                    (_, Some(error)) => Err(error),
                                    _ => Err(ApiError::new("protocol", "missing reply")),
                                };
                                let _ = sender.send(result);
                            }
                        }
                        Ok(Message::Event { event }) => {
                            if events_tx.try_send(event).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                pending_reader.lock().unwrap().clear();
                let _ = reader.get_ref().shutdown(Shutdown::Both);
                let _ = events_tx.try_send(Event::Disconnected {
                    reason: "broker connection closed".into(),
                });
            })?;
        Ok(Self(Arc::new(Inner {
            writer: Mutex::new(stream),
            pending,
            events: Mutex::new(events_rx),
            next: AtomicU32::new(1),
        })))
    }
    pub fn request(&self, request: Request) -> Result<Reply> {
        let id = self.0.next.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        self.0.pending.lock().unwrap().insert(id, sender);
        let written = (|| -> Result<()> {
            let mut stream = self.0.writer.lock().unwrap();
            let bytes = serde_json::to_vec(&Command { id, request })?;
            if bytes.len() >= MAX_FRAME_BYTES {
                return Err(ApiError::new("too_large", "request exceeds frame limit").into());
            }
            stream.write_all(&bytes)?;
            stream.write_all(b"\n")?;
            Ok(())
        })();
        if let Err(error) = written {
            self.0.pending.lock().unwrap().remove(&id);
            return Err(error);
        }
        match receiver.recv_timeout(Duration::from_secs(10)) {
            Ok(result) => result.map_err(Error::Broker),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(Error::Disconnected),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.0.pending.lock().unwrap().remove(&id);
                Err(Error::Timeout)
            }
        }
    }
    pub fn next_event(&self, timeout: Duration) -> Result<Option<Event>> {
        match self.0.events.lock().unwrap().recv_timeout(timeout) {
            Ok(event) => Ok(Some(event)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(Error::Disconnected),
        }
    }
    pub fn close(&self) -> Result<()> {
        self.0.writer.lock().unwrap().shutdown(Shutdown::Both)?;
        Ok(())
    }
    pub fn request_json(&self, request: &str) -> Result<String> {
        Ok(serde_json::to_string(
            &self.request(serde_json::from_str(request)?)?,
        )?)
    }
    pub fn next_event_json(&self, timeout_ms: u32) -> Result<Option<String>> {
        self.next_event(Duration::from_millis(timeout_ms.into()))?
            .map(|event| serde_json::to_string(&event).map_err(Error::from))
            .transpose()
    }
}

#[derive(Clone)]
pub struct Client {
    connection: Connection,
}
impl Client {
    pub fn connect(application: Application, endpoint: Option<&Path>) -> Result<Self> {
        let connection = Connection::connect(endpoint)?;
        connection.request(Request::RegisterClient {
            application,
            processing: true,
        })?;
        Ok(Self { connection })
    }
    pub fn activate(&self) -> Result<u32> {
        match self.connection.request(Request::Activate)? {
            Reply::Activated { activation } => Ok(activation),
            _ => Err(Error::InvalidResponse),
        }
    }
    pub fn deactivate(&self, activation: u32) -> Result<()> {
        self.connection
            .request(Request::Deactivate { activation })?;
        Ok(())
    }
    pub fn respond(&self, session: &str, accept: bool, audio: AudioFeatures) -> Result<()> {
        self.connection.request(Request::Respond {
            session: session.into(),
            accept,
            audio,
        })?;
        Ok(())
    }
    pub fn acknowledge(&self, session: &str, applied: bool, error: Option<String>) -> Result<()> {
        self.connection.request(Request::Acknowledge {
            session: session.into(),
            applied,
            error,
        })?;
        Ok(())
    }
    pub fn cancel(&self, session: &str) -> Result<()> {
        self.connection.request(Request::Cancel {
            session: session.into(),
        })?;
        Ok(())
    }
    pub fn next_event(&self, timeout: Duration) -> Result<Option<Event>> {
        self.connection.next_event(timeout)
    }
    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}

#[derive(Clone)]
pub struct Provider {
    connection: Connection,
}
impl Provider {
    pub fn connect(name: &str, endpoint: Option<&Path>) -> Result<Self> {
        let connection = Connection::connect(endpoint)?;
        connection.request(Request::RegisterProvider { name: name.into() })?;
        Ok(Self { connection })
    }
    pub fn begin(&self, audio: AudioFeatures) -> Result<String> {
        match self.connection.request(Request::Begin { audio })? {
            Reply::Started { session } => Ok(session),
            _ => Err(Error::InvalidResponse),
        }
    }
    pub fn audio(&self, session: &str, frame: AudioFrame) -> Result<()> {
        self.connection.request(Request::Audio {
            session: session.into(),
            frame,
        })?;
        Ok(())
    }
    /// Call after Ready when microphone capture ends, before waiting for ASR.
    /// Repeated calls keep a slow processing session alive without duplicating
    /// the application's transition event.
    pub fn processing(&self, session: &str) -> Result<()> {
        self.connection.request(Request::Processing {
            session: session.into(),
        })?;
        Ok(())
    }
    pub fn complete(&self, session: &str, text: &str, submit: bool) -> Result<()> {
        self.connection.request(Request::Complete {
            session: session.into(),
            transcript: Transcript {
                text: text.into(),
                submit,
            },
        })?;
        Ok(())
    }
    pub fn cancel(&self, session: &str) -> Result<()> {
        self.connection.request(Request::Cancel {
            session: session.into(),
        })?;
        Ok(())
    }
    pub fn next_event(&self, timeout: Duration) -> Result<Option<Event>> {
        self.connection.next_event(timeout)
    }
    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}
