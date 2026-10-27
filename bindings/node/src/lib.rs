use dictation_api::Connection;
use napi_derive::napi;
fn error(e: impl std::fmt::Display) -> napi::Error {
    napi::Error::from_reason(e.to_string())
}
#[napi]
pub struct BrokerConnection {
    connection: Connection,
}
#[napi]
impl BrokerConnection {
    #[napi(constructor)]
    pub fn new(endpoint: Option<String>) -> napi::Result<Self> {
        Connection::connect(endpoint.as_deref().map(std::path::Path::new))
            .map(|connection| Self { connection })
            .map_err(error)
    }
    #[napi]
    pub async fn request(&self, json: String) -> napi::Result<String> {
        let connection = self.connection.clone();
        tokio::task::spawn_blocking(move || connection.request_json(&json))
            .await
            .map_err(error)?
            .map_err(error)
    }
    #[napi]
    pub async fn next_event(&self, timeout_ms: u32) -> napi::Result<Option<String>> {
        let connection = self.connection.clone();
        tokio::task::spawn_blocking(move || connection.next_event_json(timeout_ms))
            .await
            .map_err(error)?
            .map_err(error)
    }
    #[napi]
    pub fn close(&self) {
        let _ = self.connection.close();
    }
}
