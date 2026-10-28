//! Versioned messages shared by the broker and every language binding.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub const MAX_ICON_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct Application {
    pub id: String,
    pub name: String,
    /// A data URL or an absolute icon filename. Consumers must not execute SVG markup.
    pub icon: Option<String>,
    pub process_id: u32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct AudioFeatures {
    pub level: bool,
    pub spectrum: bool,
}
impl AudioFeatures {
    pub fn intersection(self, requested: Self) -> Self {
        Self {
            level: self.level && requested.level,
            spectrum: self.spectrum && requested.spectrum,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
pub struct AudioFrame {
    pub level: Option<f32>,
    /// Normalized magnitudes, ordered from low to high frequency.
    pub spectrum: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct Transcript {
    pub text: String,
    pub submit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    RegisterClient {
        application: Application,
        // Opt in to processing events; absent on older consumers.
        #[serde(default)]
        processing: bool,
    },
    RegisterProvider {
        name: String,
    },
    Activate,
    Deactivate {
        activation: u32,
    },
    Begin {
        audio: AudioFeatures,
    },
    Respond {
        session: String,
        accept: bool,
        audio: AudioFeatures,
    },
    Audio {
        session: String,
        frame: AudioFrame,
    },
    /// Capture ended; the provider is transcribing or waiting to transcribe.
    Processing {
        session: String,
    },
    Complete {
        session: String,
        transcript: Transcript,
    },
    Acknowledge {
        session: String,
        applied: bool,
        error: Option<String>,
    },
    Cancel {
        session: String,
    },
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Reply {
    Registered { connection: String },
    Activated { activation: u32 },
    Started { session: String },
    Ok,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    Offer {
        session: String,
        provider: String,
        audio: AudioFeatures,
    },
    Ready {
        session: String,
        application: Option<Application>,
        audio: AudioFeatures,
    },
    Audio {
        session: String,
        frame: AudioFrame,
    },
    Processing {
        session: String,
    },
    Transcript {
        session: String,
        transcript: Transcript,
    },
    Finished {
        session: String,
        applied: bool,
        error: Option<String>,
    },
    Cancelled {
        session: String,
        reason: String,
    },
    Disconnected {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}
impl ApiError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for ApiError {}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Command {
    pub id: u32,
    pub request: Request,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Response {
        id: u32,
        reply: Option<Reply>,
        error: Option<ApiError>,
    },
    Event {
        event: Event,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Hello {
    pub version: u32,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub version: u32,
    pub address: std::net::SocketAddr,
    pub token: String,
}

pub fn export_typescript(directory: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(directory)?;
    let definitions = [
        Application::decl(),
        AudioFeatures::decl(),
        AudioFrame::decl(),
        Transcript::decl(),
        Request::decl(),
        Reply::decl(),
        Event::decl(),
        ApiError::decl(),
        Command::decl(),
        Message::decl(),
    ];
    let text = definitions
        .iter()
        .map(|d| format!("export {d}\n"))
        .collect::<String>();
    std::fs::write(
        directory.join("protocol.ts"),
        format!("// Generated from dictation-protocol by ts-rs. Do not edit.\n{text}"),
    )?;
    Ok(())
}
