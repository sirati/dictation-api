//! Native focus observers supplement cooperative application reports.
use crate::server::State;
use std::sync::{Arc, Mutex};
pub fn start(state: Arc<Mutex<State>>) {
    #[cfg(target_os = "linux")]
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("accessibility runtime");
        if let Err(error) = runtime.block_on(linux(state)) {
            eprintln!(
                "Accessibility focus unavailable: {error}. Explicit application reports remain available."
            );
        }
    });
    #[cfg(target_os = "windows")]
    std::thread::spawn(move || {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId,
        };
        let mut previous = 0usize;
        loop {
            let window = unsafe { GetForegroundWindow() };
            if window as usize != previous {
                previous = window as usize;
                let mut pid = 0;
                unsafe {
                    GetWindowThreadProcessId(window, &mut pid);
                }
                state
                    .lock()
                    .unwrap()
                    .core
                    .observe_activation(pid, format!("{previous}"));
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });
    #[cfg(target_os = "macos")]
    std::thread::spawn(move || {
        let mut previous = 0;
        loop {
            let pid = objc2::rc::autoreleasepool(|_| {
                let workspace = objc2_app_kit::NSWorkspace::sharedWorkspace();
                workspace
                    .frontmostApplication()
                    .map(|application| application.processIdentifier() as u32)
                    .unwrap_or(0)
            });
            if pid != previous {
                previous = pid;
                state
                    .lock()
                    .unwrap()
                    .core
                    .observe_activation(pid, pid.to_string());
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });
}
#[cfg(target_os = "linux")]
async fn linux(state: Arc<Mutex<State>>) -> Result<(), Box<dyn std::error::Error>> {
    use futures_util::StreamExt;
    let session = zbus::Connection::session().await?;
    let bus = zbus::Proxy::new(&session, "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus").await?;
    let address: String = bus.call("GetAddress", &()).await?;
    let connection = zbus::connection::Builder::address(address.as_str())?
        .build()
        .await?;
    let registry = zbus::Proxy::new(
        &connection,
        "org.a11y.atspi.Registry",
        "/org/a11y/atspi/registry",
        "org.a11y.atspi.Registry",
    )
    .await?;
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.a11y.atspi.Event.Window")?
        .build();
    let windows = zbus::MessageStream::for_match_rule(rule, &connection, Some(128)).await?;
    let object_rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.a11y.atspi.Event.Object")?
        .member("StateChanged")?
        .arg(0, "focused")?
        .build();
    let objects = zbus::MessageStream::for_match_rule(object_rule, &connection, Some(128)).await?;
    let mut signals = futures_util::stream::select(windows, objects);
    for event in [
        "window:activate",
        "window:deactivate",
        "object:state-changed:focused",
    ] {
        let _: () = registry
            .call("RegisterEvent", &(event, Vec::<String>::new(), ""))
            .await?;
    }
    let dbus = zbus::fdo::DBusProxy::new(&connection).await?;
    if let Ok(Some((pid, window))) = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        initial_linux_focus(&connection),
    )
    .await
    {
        state.lock().unwrap().core.initialize_focus(pid, window);
    }
    while let Some(Ok(message)) = signals.next().await {
        let header = message.header();
        let Some(sender) = header.sender() else {
            continue;
        };
        let Some(path) = header.path() else { continue };
        let Some(member) = header.member() else {
            continue;
        };
        let Ok(pid) = dbus
            .get_connection_unix_process_id(sender.to_owned().into())
            .await
        else {
            continue;
        };
        let mut s = state.lock().unwrap();
        match member.as_str() {
            "Activate" => s.core.observe_activation(pid, path.to_string()),
            "Deactivate" => s.core.observe_deactivation(pid, path.as_str()),
            "StateChanged" => {
                if let Ok(body) = message
                    .body()
                    .deserialize::<zbus::zvariant::Structure<'_>>()
                {
                    let enabled = body
                        .fields()
                        .get(1)
                        .and_then(|value| i32::try_from(value).ok());
                    if enabled == Some(1) {
                        s.core.observe_activation(pid, path.to_string());
                    } else if enabled == Some(0) {
                        s.core.observe_deactivation(pid, path.as_str());
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
async fn initial_linux_focus(connection: &zbus::Connection) -> Option<(u32, String)> {
    type Reference = (String, zbus::zvariant::OwnedObjectPath);
    let registry = zbus::Proxy::new(
        connection,
        "org.a11y.atspi.Registry",
        "/org/a11y/atspi/accessible/root",
        "org.a11y.atspi.Accessible",
    )
    .await
    .ok()?;
    let applications: Vec<Reference> = registry.call("GetChildren", &()).await.ok()?;
    let dbus = zbus::fdo::DBusProxy::new(connection).await.ok()?;
    for (name, path) in applications.into_iter().take(128) {
        let Ok(application) = zbus::Proxy::new(
            connection,
            name.as_str(),
            path.as_str(),
            "org.a11y.atspi.Accessible",
        )
        .await
        else {
            continue;
        };
        let Ok(windows) = application
            .call::<_, _, Vec<Reference>>("GetChildren", &())
            .await
        else {
            continue;
        };
        for (name, path) in windows.into_iter().take(128) {
            let Ok(window) = zbus::Proxy::new(
                connection,
                name.as_str(),
                path.as_str(),
                "org.a11y.atspi.Accessible",
            )
            .await
            else {
                continue;
            };
            let Ok(states) = window.call::<_, _, Vec<u32>>("GetState", &()).await else {
                continue;
            };
            // AT-SPI STATE_ACTIVE is bit 1. Only top-level windows are inspected.
            if states.first().is_some_and(|word| word & 2 != 0) {
                let bus_name = zbus::names::BusName::try_from(name.as_str()).ok()?;
                let pid = dbus.get_connection_unix_process_id(bus_name).await.ok()?;
                return Some((pid, path.to_string()));
            }
        }
    }
    None
}
