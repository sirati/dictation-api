//! Minimal receiving application. Replace the print with insertion into your captured target.
use dictation_api::{Application, AudioFeatures, Client, Event};
use std::time::Duration;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::connect(
        Application {
            id: "example.receiver".into(),
            name: "Example receiver".into(),
            icon: None,
            process_id: std::process::id(),
        },
        None,
    )?;
    let activation = client.activate()?;
    loop {
        match client.next_event(Duration::from_secs(1))? {
            Some(Event::Offer { session, .. }) => {
                println!("capture original document and selection for {session}");
                client.respond(&session, true, AudioFeatures::default())?;
            }
            Some(Event::Transcript {
                session,
                transcript,
            }) => {
                println!("text={:?}, submit={}", transcript.text, transcript.submit);
                client.acknowledge(&session, true, None)?;
                client.deactivate(activation)?;
                break;
            }
            Some(Event::Disconnected { .. }) => break,
            _ => {}
        }
    }
    Ok(())
}
