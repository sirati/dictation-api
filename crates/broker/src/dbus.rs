//! D-Bus front end. Each caller's unique bus name gets its own broker connection.
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
pub struct BrokerBus {
    endpoint: PathBuf,
    connections: Arc<Mutex<HashMap<String, dictation_api::Connection>>>,
}
impl BrokerBus {
    fn connection(
        &self,
        header: &zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<dictation_api::Connection> {
        let sender = header
            .sender()
            .ok_or_else(|| zbus::fdo::Error::AccessDenied("missing sender".into()))?
            .to_string();
        let mut connections = self.connections.lock().unwrap();
        if let Some(connection) = connections.get(&sender) {
            return Ok(connection.clone());
        }
        if connections.len() >= 256 {
            return Err(zbus::fdo::Error::LimitsExceeded("connection limit".into()));
        }
        let connection = dictation_api::Connection::connect(Some(&self.endpoint))
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        connections.insert(sender, connection.clone());
        Ok(connection)
    }
}
#[zbus::interface(name = "org.dictationapi.Broker1")]
impl BrokerBus {
    async fn request(
        &self,
        json: String,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<String> {
        let connection = self.connection(&header)?;
        tokio::task::spawn_blocking(move || connection.request_json(&json))
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }
    async fn next_event(
        &self,
        timeout_ms: u32,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<String> {
        let connection = self.connection(&header)?;
        tokio::task::spawn_blocking(move || connection.next_event_json(timeout_ms.min(30_000)))
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?
            .map(|v| v.unwrap_or_else(|| "null".into()))
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }
    fn close(&self, #[zbus(header)] header: zbus::message::Header<'_>) {
        if let Some(sender) = header.sender() {
            self.connections.lock().unwrap().remove(sender.as_str());
        }
    }
}
pub fn start(endpoint: PathBuf) {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("D-Bus runtime");
        let result: Result<(), Box<dyn std::error::Error>> = runtime.block_on(async {
            use futures_util::StreamExt;
            let connections = Arc::new(Mutex::default());
            let connection = zbus::connection::Builder::session()?
                .name("org.dictationapi.Broker1")?
                .serve_at(
                    "/org/dictationapi/Broker1",
                    BrokerBus {
                        endpoint,
                        connections: connections.clone(),
                    },
                )?
                .build()
                .await?;
            let proxy = zbus::fdo::DBusProxy::new(&connection).await?;
            let mut changes = proxy.receive_name_owner_changed().await?;
            while let Some(change) = changes.next().await {
                let args = change.args()?;
                if args.new_owner().is_none() {
                    connections.lock().unwrap().remove(args.name().as_str());
                }
            }
            Ok(())
        });
        if let Err(error) = result {
            eprintln!("D-Bus API unavailable: {error}");
        }
    });
}
