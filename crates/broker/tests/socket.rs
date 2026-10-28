use dictation_api::{Application, AudioFeatures, Client, Event, Provider};
use std::{
    process::{Child, Command},
    time::{Duration, Instant},
};
struct Broker(Child);
impl Drop for Broker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn event_client(client: &Client) -> Event {
    client
        .next_event(Duration::from_secs(3))
        .unwrap()
        .expect("client event")
}
fn event_provider(provider: &Provider) -> Event {
    provider
        .next_event(Duration::from_secs(3))
        .unwrap()
        .expect("provider event")
}
#[test]
fn original_recipient_over_authenticated_socket() {
    let home = tempfile::tempdir().unwrap();
    let _broker = Broker(
        Command::new(
            std::env::var_os("DICTATION_BROKER_EXE")
                .unwrap_or_else(|| env!("CARGO_BIN_EXE_dictation-broker").into()),
        )
        .arg("run")
        .env("DICTATION_API_NO_NATIVE_FOCUS", "1")
        .env("DICTATION_API_NO_DBUS", "1")
        .env("DICTATION_API_HOME", home.path())
        .spawn()
        .unwrap(),
    );
    let endpoint = home.path().join("endpoint.json");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !endpoint.exists() {
        assert!(Instant::now() < deadline, "broker startup");
        std::thread::sleep(Duration::from_millis(20));
    }
    let application = |id: &str| Application {
        id: id.into(),
        name: format!("Application {id}"),
        icon: Some("data:image/png;base64,aA==".into()),
        process_id: std::process::id(),
    };
    let a = Client::connect(application("a"), Some(&endpoint)).unwrap();
    let b = Client::connect(application("b"), Some(&endpoint)).unwrap();
    let provider = Provider::connect("Speech engine", Some(&endpoint)).unwrap();
    let stale = a.activate().unwrap();
    b.activate().unwrap();
    a.deactivate(stale).unwrap();
    let session = provider
        .begin(AudioFeatures {
            level: true,
            spectrum: true,
        })
        .unwrap();
    assert!(matches!(event_client(&b),Event::Offer{session:s,..} if s==session));
    assert!(a.next_event(Duration::from_millis(50)).unwrap().is_none());
    a.activate().unwrap();
    b.respond(
        &session,
        true,
        AudioFeatures {
            level: true,
            spectrum: false,
        },
    )
    .unwrap();
    assert!(
        matches!(event_provider(&provider),Event::Ready{application:Some(app),audio,..} if app.id=="b" && app.icon.is_some() && audio.level && !audio.spectrum)
    );
    provider
        .audio(
            &session,
            dictation_api::AudioFrame {
                level: Some(0.4),
                spectrum: None,
            },
        )
        .unwrap();
    assert!(matches!(event_client(&b),Event::Audio{frame,..} if frame.level==Some(0.4)));
    provider.processing(&session).unwrap();
    assert!(matches!(event_client(&b),Event::Processing{session:s} if s==session));
    provider.processing(&session).unwrap();
    assert!(b.next_event(Duration::from_millis(50)).unwrap().is_none());
    provider
        .complete(&session, "captured selection", true)
        .unwrap();
    assert!(
        matches!(event_client(&b),Event::Transcript{transcript,..} if transcript.text=="captured selection" && transcript.submit)
    );
    b.acknowledge(&session, true, None).unwrap();
    assert!(matches!(
        event_provider(&provider),
        Event::Finished { applied: true, .. }
    ));
}
