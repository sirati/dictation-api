use dictation_broker::core::Core;
use dictation_protocol::*;
fn register(core: &mut Core, id: &str) {
    core.request(
        id,
        Request::RegisterClient {
            application: Application {
                id: id.into(),
                name: id.into(),
                icon: None,
                process_id: 1,
            },
        },
    )
    .unwrap();
}
fn begin(core: &mut Core) -> String {
    match core
        .request(
            "provider",
            Request::Begin {
                audio: AudioFeatures {
                    level: true,
                    spectrum: false,
                },
            },
        )
        .unwrap()
    {
        Reply::Started { session } => session,
        _ => panic!(),
    }
}
#[test]
fn stale_deactivation_and_focus_changes_preserve_session_recipient() {
    let mut core = Core::default();
    register(&mut core, "a");
    register(&mut core, "b");
    core.request(
        "provider",
        Request::RegisterProvider {
            name: "test".into(),
        },
    )
    .unwrap();
    let Reply::Activated { activation } = core.request("a", Request::Activate).unwrap() else {
        panic!()
    };
    core.request("b", Request::Activate).unwrap();
    core.request("a", Request::Deactivate { activation })
        .unwrap();
    let session = begin(&mut core);
    assert!(matches!(&core.events[0],(id,Event::Offer{..}) if id=="b"));
    core.request("a", Request::Activate).unwrap();
    core.request(
        "b",
        Request::Respond {
            session: session.clone(),
            accept: true,
            audio: AudioFeatures {
                level: true,
                spectrum: true,
            },
        },
    )
    .unwrap();
    assert!(
        matches!(&core.events[1],(_,Event::Ready{application:Some(a),audio,..}) if a.id=="b" && audio.level && !audio.spectrum)
    );
    assert!(
        core.request(
            "a",
            Request::Complete {
                session: session.clone(),
                transcript: Transcript {
                    text: "steal".into(),
                    submit: false
                }
            }
        )
        .is_err()
    );
    core.request(
        "provider",
        Request::Complete {
            session: session.clone(),
            transcript: Transcript {
                text: "original target".into(),
                submit: true,
            },
        },
    )
    .unwrap();
    assert!(
        matches!(&core.events[2],(id,Event::Transcript{transcript,..}) if id=="b" && transcript.submit)
    );
    core.request(
        "b",
        Request::Acknowledge {
            session,
            applied: true,
            error: None,
        },
    )
    .unwrap();
    assert!(matches!(&core.events[3],(id,Event::Finished{applied:true,..}) if id=="provider"));
}
#[test]
fn native_focus_on_unregistered_application_clears_recipient() {
    let mut core = Core::default();
    register(&mut core, "a");
    core.request(
        "provider",
        Request::RegisterProvider {
            name: "test".into(),
        },
    )
    .unwrap();
    core.request("a", Request::Activate).unwrap();
    core.observe_activation(999, "other-window".into());
    begin(&mut core);
    assert!(matches!(
        &core.events[0],
        (
            _,
            Event::Ready {
                application: None,
                ..
            }
        )
    ));
}
#[test]
fn stale_native_deactivation_cannot_clear_new_explicit_focus() {
    let mut core = Core::default();
    register(&mut core, "a");
    register(&mut core, "b");
    core.request(
        "provider",
        Request::RegisterProvider {
            name: "test".into(),
        },
    )
    .unwrap();
    core.observe_activation(1, "old".into());
    core.request("b", Request::Activate).unwrap();
    core.observe_deactivation(1, "old");
    begin(&mut core);
    assert!(matches!(&core.events[0],(id,Event::Offer{..}) if id=="b"));
}
