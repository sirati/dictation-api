#![cfg(target_os = "linux")]
use dictation_api::{Application, AudioFeatures, Client, Event, Reply, Request};
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn dbus_provider_and_socket_client_share_the_same_sessions()
-> Result<(), Box<dyn std::error::Error>> {
    tokio::time::timeout(Duration::from_secs(15), scenario()).await?
}
async fn scenario() -> Result<(), Box<dyn std::error::Error>> {
    let home = tempfile::tempdir()?;
    let config = home.path().join("bus.conf");
    std::fs::write(
        &config,
        format!(
            "<busconfig><type>session</type><listen>unix:tmpdir={}</listen><auth>EXTERNAL</auth><policy context=\"default\"><allow send_destination=\"*\"/><allow receive_sender=\"*\"/><allow own=\"*\"/></policy></busconfig>",
            home.path().display()
        ),
    )?;
    let mut daemon = Process(
        Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()?,
    );
    let mut address = String::new();
    BufReader::new(daemon.0.stdout.take().unwrap()).read_line(&mut address)?;
    let _broker = Process(
        Command::new(env!("CARGO_BIN_EXE_dictation-broker"))
            .arg("run")
            .env("DICTATION_API_HOME", home.path())
            .env("DICTATION_API_NO_NATIVE_FOCUS", "1")
            .env("DBUS_SESSION_BUS_ADDRESS", address.trim())
            .spawn()?,
    );
    let endpoint = home.path().join("endpoint.json");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !endpoint.exists() {
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let connection = zbus::connection::Builder::address(address.trim())?
        .build()
        .await?;
    let proxy = zbus::Proxy::new(
        &connection,
        "org.dictationapi.Broker1",
        "/org/dictationapi/Broker1",
        "org.dictationapi.Broker1",
    )
    .await?;
    while proxy
        .call::<_, _, String>("Request", &(serde_json::to_string(&Request::Ping)?,))
        .await
        .is_err()
    {
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let request = |r: Request| serde_json::to_string(&r).unwrap();
    let _: String = proxy
        .call(
            "Request",
            &(request(Request::RegisterProvider {
                name: "D-Bus speech provider".into(),
            }),),
        )
        .await?;
    let client = Client::connect(
        Application {
            id: "socket".into(),
            name: "Socket client".into(),
            icon: None,
            process_id: std::process::id(),
        },
        Some(&endpoint),
    )?;
    client.activate()?;
    let started: String = proxy
        .call(
            "Request",
            &(request(Request::Begin {
                audio: AudioFeatures::default(),
            }),),
        )
        .await?;
    let Reply::Started { session } = serde_json::from_str(&started)? else {
        panic!("not started")
    };
    assert!(matches!(
        client.next_event(Duration::from_secs(2))?,
        Some(Event::Offer { .. })
    ));
    client.respond(&session, true, AudioFeatures::default())?;
    let ready: String = proxy.call("NextEvent", &(2000u32,)).await?;
    assert!(
        matches!(serde_json::from_str::<Event>(&ready)?,Event::Ready{application:Some(a),..} if a.id=="socket")
    );
    let _: String = proxy
        .call(
            "Request",
            &(request(Request::Complete {
                session: session.clone(),
                transcript: dictation_api::Transcript {
                    text: "D-Bus transcript".into(),
                    submit: true,
                },
            }),),
        )
        .await?;
    assert!(
        matches!(client.next_event(Duration::from_secs(2))?,Some(Event::Transcript{transcript,..}) if transcript.submit && transcript.text=="D-Bus transcript")
    );
    client.acknowledge(&session, true, None)?;
    let finished: String = proxy.call("NextEvent", &(2000u32,)).await?;
    assert!(matches!(
        serde_json::from_str::<Event>(&finished)?,
        Event::Finished { applied: true, .. }
    ));
    let _: () = proxy.call("Close", &()).await?;
    Ok(())
}
