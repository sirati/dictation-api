//! Per-user service definitions. Installation does not require administrative rights.
use std::{path::PathBuf, process::Command};
pub fn install(start: bool) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let executable = std::env::current_exe()?;
    #[cfg(target_os = "linux")]
    {
        let root = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").expect("HOME")).join(".config")
            });
        let path = root.join("systemd/user/dictation-broker.service");
        std::fs::create_dir_all(path.parent().unwrap())?;
        let escaped = executable
            .to_str()
            .ok_or("non UTF-8 executable")?
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%");
        std::fs::write(
            &path,
            format!(
                "[Unit]\nDescription=Application dictation broker\nAfter=graphical-session.target\nPartOf=graphical-session.target\n\n[Service]\nExecStart=\"{escaped}\" run\nRestart=on-failure\n\n[Install]\nWantedBy=default.target\n"
            ),
        )?;
        if !start {
            return Ok(path);
        }
        if !Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status()?
            .success()
        {
            return Err("systemd reload failed".into());
        }
        if !Command::new("systemctl")
            .args(["--user", "enable", "--now", "dictation-broker.service"])
            .status()?
            .success()
        {
            return Err("service start failed".into());
        }
        Ok(path)
    }
    #[cfg(target_os = "macos")]
    {
        let path = PathBuf::from(std::env::var_os("HOME").ok_or("HOME missing")?)
            .join("Library/LaunchAgents/org.dictationapi.broker.plist");
        std::fs::create_dir_all(path.parent().unwrap())?;
        let executable = executable
            .to_str()
            .ok_or("non UTF-8 executable")?
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        std::fs::write(
            &path,
            format!(
                "<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>Label</key><string>org.dictationapi.broker</string><key>ProgramArguments</key><array><string>{executable}</string><string>run</string></array><key>RunAtLoad</key><true/><key>KeepAlive</key><true/></dict></plist>"
            ),
        )?;
        if !start {
            return Ok(path);
        }
        let uid = Command::new("id").arg("-u").output()?;
        let domain = format!("gui/{}", String::from_utf8(uid.stdout)?.trim());
        if !Command::new("launchctl")
            .arg("bootstrap")
            .arg(domain)
            .arg(&path)
            .status()?
            .success()
        {
            return Err("LaunchAgent bootstrap failed".into());
        }
        Ok(path)
    }
    #[cfg(target_os = "windows")]
    {
        let identity = Command::new("whoami.exe")
            .args(["/user", "/fo", "csv", "/nh"])
            .output()?;
        if !identity.status.success() {
            return Err("could not determine current user SID".into());
        }
        let output = String::from_utf8(identity.stdout)?;
        let sid = output
            .split(['"', ',', '\r', '\n'])
            .find(|field| field.starts_with("S-1-"))
            .ok_or("current user SID missing")?;
        let executable_text = executable
            .to_str()
            .ok_or("non UTF-8 executable")?
            .replace('\'', "''");
        let arguments = format!(
            "-NoLogo -NoProfile -NonInteractive -WindowStyle Hidden -Command \"& '{executable_text}' run\""
        );
        let escaped = arguments
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
<RegistrationInfo><Description>Per-user application dictation broker</Description></RegistrationInfo>
<Triggers><LogonTrigger><Enabled>true</Enabled><UserId>{sid}</UserId></LogonTrigger></Triggers>
<Principals><Principal id="CurrentUser"><UserId>{sid}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>
<Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><StartWhenAvailable>true</StartWhenAvailable><ExecutionTimeLimit>PT0S</ExecutionTimeLimit><RestartOnFailure><Interval>PT1M</Interval><Count>3</Count></RestartOnFailure></Settings>
<Actions Context="CurrentUser"><Exec><Command>powershell.exe</Command><Arguments>{escaped}</Arguments></Exec></Actions>
</Task>"#
        );
        let definition =
            std::env::temp_dir().join(format!("dictation-task-{}.xml", uuid::Uuid::new_v4()));
        // schtasks.exe expects XML with a Unicode BOM. UTF-16 avoids the
        // Windows task scheduler's locale-dependent encoding detection.
        let mut encoded = vec![0xff, 0xfe];
        for unit in xml.encode_utf16() { encoded.extend_from_slice(&unit.to_le_bytes()); }
        std::fs::write(&definition, encoded)?;
        let created = Command::new("schtasks.exe")
            .args(["/Create", "/TN", "DictationApiBroker", "/XML"])
            .arg(&definition)
            .arg("/F")
            .status();
        let _ = std::fs::remove_file(&definition);
        if !created?.success() {
            return Err("scheduled task creation failed".into());
        }
        if !start {
            return Ok(executable);
        }
        if !Command::new("schtasks.exe")
            .args(["/Run", "/TN", "DictationApiBroker"])
            .status()?
            .success()
        {
            return Err("scheduled task start failed".into());
        }
        Ok(executable)
    }
}

/// Remove only this broker's per-user service registration.
pub fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    {
        let status = Command::new("systemctl")
            .args(["--user", "disable", "--now", "dictation-broker.service"])
            .status()?;
        if !status.success() {
            return Err("could not stop and disable user service".into());
        }
        let root = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").expect("HOME")).join(".config")
            });
        std::fs::remove_file(root.join("systemd/user/dictation-broker.service"))?;
        if !Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status()?
            .success()
        {
            return Err("systemd reload failed".into());
        }
    }
    #[cfg(target_os = "macos")]
    {
        let uid = Command::new("id").arg("-u").output()?;
        let target = format!(
            "gui/{}/org.dictationapi.broker",
            String::from_utf8(uid.stdout)?.trim()
        );
        if !Command::new("launchctl")
            .args(["bootout", &target])
            .status()?
            .success()
        {
            return Err("LaunchAgent removal failed".into());
        }
        std::fs::remove_file(
            PathBuf::from(std::env::var_os("HOME").ok_or("HOME missing")?)
                .join("Library/LaunchAgents/org.dictationapi.broker.plist"),
        )?;
    }
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("schtasks.exe")
            .args(["/End", "/TN", "DictationApiBroker"])
            .status()?;
        if !Command::new("schtasks.exe")
            .args(["/Delete", "/TN", "DictationApiBroker", "/F"])
            .status()?
            .success()
        {
            return Err("scheduled task removal failed".into());
        }
    }
    Ok(())
}
