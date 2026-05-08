#[cfg(target_os = "macos")]
pub fn install() -> anyhow::Result<()> {
    install_macos()
}

#[cfg(target_os = "linux")]
pub fn install() -> anyhow::Result<()> {
    install_linux()
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn install() -> anyhow::Result<()> {
    anyhow::bail!("--install-cron is not supported on this platform.");
}

#[cfg(target_os = "macos")]
fn install_macos() -> anyhow::Result<()> {
    let home = std::env::var("HOME")?;
    let launch_agents = format!("{}/Library/LaunchAgents", home);
    std::fs::create_dir_all(&launch_agents)?;

    let plist_path = format!("{}/com.mlocate.mupdatedb.plist", launch_agents);
    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.mlocate.mupdatedb</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}/bin/mupdatedb</string>
        <string>--quiet</string>
    </array>
    <key>StartCalendarInterval</key>
    <dict>
        <key>Hour</key>
        <integer>3</integer>
        <key>Minute</key>
        <integer>0</integer>
    </dict>
    <key>StandardOutPath</key>
    <string>{}/Library/Logs/mlocate-mupdatedb.log</string>
    <key>StandardErrorPath</key>
    <string>{}/Library/Logs/mlocate-mupdatedb.log</string>
</dict>
</plist>"#,
        home, home, home
    );

    std::fs::write(&plist_path, plist_content)?;
    println!("LaunchAgent installed at: {}", plist_path);
    println!("To activate: launchctl load {}", plist_path);
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_linux() -> anyhow::Result<()> {
    let home = std::env::var("HOME")?;
    let systemd_dir = format!("{}/.config/systemd/user", home);
    std::fs::create_dir_all(&systemd_dir)?;

    let service_path = format!("{}/mupdatedb.service", systemd_dir);
    let service_content = format!(
        r#"[Unit]
Description=Update mlocate database

[Service]
Type=oneshot
ExecStart={}/bin/mupdatedb --quiet
"#,
        home
    );
    std::fs::write(&service_path, service_content)?;

    let timer_path = format!("{}/mupdatedb.timer", systemd_dir);
    let timer_content = r#"[Unit]
Description=Daily mlocate database update

[Timer]
OnCalendar=daily
Persistent=true

[Install]
WantedBy=timers.target
"#;
    std::fs::write(&timer_path, timer_content)?;

    println!("systemd units installed at:");
    println!("  {}", service_path);
    println!("  {}", timer_path);
    println!("To activate:");
    println!("  systemctl --user enable mupdatedb.timer");
    println!("  systemctl --user start mupdatedb.timer");
    Ok(())
}
