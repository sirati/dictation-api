use crate::core::Core;
use dictation_protocol::*;
use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    path::Path,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};
type Writers = HashMap<String, (mpsc::SyncSender<Message>, TcpStream)>;
#[derive(Default)]
pub struct State {
    pub core: Core,
    writers: Writers,
}
impl State {
    fn flush(&mut self) {
        while !self.core.events.is_empty() {
            for (id, event) in std::mem::take(&mut self.core.events) {
                self.send(&id, Message::Event { event });
            }
        }
    }
    fn send(&mut self, id: &str, message: Message) {
        if self
            .writers
            .get(id)
            .is_some_and(|(sender, _)| sender.try_send(message).is_err())
        {
            if let Some((_, stream)) = self.writers.remove(id) {
                let _ = stream.shutdown(Shutdown::Both);
            }
            self.core.disconnect(id);
        }
    }
}
fn write_message(stream: &mut TcpStream, message: &Message) -> std::io::Result<()> {
    serde_json::to_writer(&mut *stream, message)?;
    stream.write_all(b"\n")
}
fn line(reader: &mut BufReader<TcpStream>) -> std::io::Result<String> {
    let mut value = String::new();
    reader
        .by_ref()
        .take((MAX_FRAME_BYTES + 1) as u64)
        .read_line(&mut value)?;
    if value.len() > MAX_FRAME_BYTES || !value.ends_with('\n') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid frame",
        ));
    }
    Ok(value)
}
fn peer(
    mut stream: TcpStream,
    token: String,
    state: Arc<Mutex<State>>,
) -> Result<(), Box<dyn std::error::Error>> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let hello: Hello = serde_json::from_str(&line(&mut reader)?)?;
    if hello.version != PROTOCOL_VERSION || hello.token != token {
        return Err("authentication failed".into());
    }
    stream.set_read_timeout(None)?;
    let id = uuid::Uuid::new_v4().to_string();
    let (sender, receiver) = mpsc::sync_channel(256);
    {
        let mut s = state.lock().unwrap();
        if s.writers.len() >= 256 {
            return Err("connection limit reached".into());
        }
        s.writers.insert(id.clone(), (sender, stream.try_clone()?));
    }
    thread::spawn(move || {
        for message in receiver {
            if write_message(&mut stream, &message).is_err() {
                break;
            }
        }
        let _ = stream.shutdown(Shutdown::Both);
    });
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        loop {
            let command: Command = serde_json::from_str(&line(&mut reader)?)?;
            let mut s = state.lock().unwrap();
            let result = s.core.request(&id, command.request);
            let (reply, error) = match result {
                Ok(reply) => (Some(reply), None),
                Err(error) => (None, Some(error)),
            };
            s.send(
                &id,
                Message::Response {
                    id: command.id,
                    reply,
                    error,
                },
            );
            s.flush();
        }
    })();
    let mut s = state.lock().unwrap();
    s.writers.remove(&id);
    s.core.disconnect(&id);
    s.flush();
    result
}
pub fn run(home: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(home)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(home, fs::Permissions::from_mode(0o700))?;
    }
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(home.join("broker.lock"))?;
    fs2::FileExt::try_lock_exclusive(&lock)?;
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    let token = format!("{}{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let endpoint = Endpoint {
        version: PROTOCOL_VERSION,
        address: listener.local_addr()?,
        token: token.clone(),
    };
    let path = home.join("endpoint.json");
    let temporary = home.join("endpoint.tmp");
    fs::write(&temporary, serde_json::to_vec(&endpoint)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
    }
    if path.exists() {
        fs::remove_file(&path)?;
    }
    fs::rename(temporary, &path)?;
    let state = Arc::new(Mutex::new(State::default()));
    #[cfg(target_os = "linux")]
    if std::env::var_os("DICTATION_API_NO_DBUS").is_none() {
        crate::dbus::start(path);
    }
    if std::env::var_os("DICTATION_API_NO_NATIVE_FOCUS").is_none() {
        crate::focus::start(state.clone());
    }
    let expiry = state.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(200));
            let mut s = expiry.lock().unwrap();
            s.core.expire();
            s.flush();
        }
    });
    for stream in listener.incoming() {
        let state = state.clone();
        let token = token.clone();
        if let Ok(stream) = stream {
            thread::spawn(move || {
                let _ = peer(stream, token, state);
            });
        }
    }
    Ok(())
}
