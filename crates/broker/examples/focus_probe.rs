//! Probe native focus without making any explicit activation report.
use dictation_api::{Application, AudioFeatures, Client, Event, Provider};
use std::time::{Duration, Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pid: u32 = std::env::args()
        .nth(1)
        .ok_or("usage: focus_probe PROCESS_ID")?
        .parse()?;
    let application = Client::connect(
        Application {
            id: "native-focus-probe".into(),
            name: "Native focus fixture".into(),
            icon: None,
            process_id: pid,
        },
        None,
    )?;
    let provider = Provider::connect("Native focus validation", None)?;
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        let session = provider.begin(AudioFeatures::default())?;
        if let Some(Event::Offer {
            session: offered, ..
        }) = application.next_event(Duration::from_millis(100))?
        {
            if offered != session {
                return Err("unexpected offer".into());
            }
            application.respond(&session, true, AudioFeatures::default())?;
            let ready = provider.next_event(Duration::from_secs(2))?;
            if !matches!(ready,Some(Event::Ready{application:Some(a),..}) if a.process_id==pid) {
                return Err("wrong native recipient".into());
            }
            provider.complete(&session, "native focus transcript", false)?;
            if !matches!(
                application.next_event(Duration::from_secs(2))?,
                Some(Event::Transcript { .. })
            ) {
                return Err("missing native transcript".into());
            }
            application.acknowledge(&session, true, None)?;
            if !matches!(
                provider.next_event(Duration::from_secs(2))?,
                Some(Event::Finished { applied: true, .. })
            ) {
                return Err("missing acknowledgement".into());
            }
            println!("native-focus-confirmed pid={pid}");
            return Ok(());
        }
        let _ = provider.next_event(Duration::from_secs(1))?;
        provider.cancel(&session)?;
        let _ = provider.next_event(Duration::from_secs(1))?;
    }
    Err("native focus not observed within 20 seconds".into())
}
