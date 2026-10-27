use dictation_api::{AudioFeatures, Event, Provider};
use std::time::Duration;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = Provider::connect("Example speech recognizer", None)?;
    let session = provider.begin(AudioFeatures::default())?;
    loop {
        match provider.next_event(Duration::from_secs(10))? {
            Some(Event::Ready { application, .. }) => {
                println!("recipient={:?}", application.as_ref().map(|a| &a.name));
                if application.is_none() {
                    provider.cancel(&session)?;
                    break;
                }
                provider.complete(
                    &session,
                    &std::env::args()
                        .nth(1)
                        .unwrap_or_else(|| "Hello from dictation".into()),
                    false,
                )?;
            }
            Some(Event::Finished { applied, error, .. }) => {
                println!("applied={applied}, error={error:?}");
                break;
            }
            Some(Event::Cancelled { reason, .. }) => {
                println!("cancelled: {reason}");
                break;
            }
            None => return Err("no broker event within ten seconds".into()),
            _ => {}
        }
    }
    Ok(())
}
