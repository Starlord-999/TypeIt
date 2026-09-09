pub fn paste_text(text: &str) -> Result<(), String> {
    // Set clipboard (arboard is thread-safe). NSPasteboard writes can fail
    // transiently if another app touches the pasteboard at the same instant
    // (more likely now that streaming chunks paste several times per
    // utterance instead of once) — one retry clears this up in practice.
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    if clipboard.set_text(text).is_err() {
        std::thread::sleep(std::time::Duration::from_millis(80));
        clipboard.set_text(text).map_err(|e| e.to_string())?;
    }

    // Small delay to ensure clipboard is set
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Simulate Cmd+V via osascript (works from any thread, unlike enigo which
    // calls TSMGetInputSourceProperty requiring the main thread)
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("osascript")
            .args(["-e", r#"tell application "System Events" to keystroke "v" using command down"#])
            .output()
            .map_err(|e| format!("Failed to simulate paste: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "osascript paste failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    #[cfg(target_os = "windows")]
    {
        use enigo::{Enigo, Keyboard, Settings, Key, Direction};
        let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
        enigo.key(Key::Control, Direction::Press).map_err(|e| e.to_string())?;
        enigo.key(Key::Unicode('v'), Direction::Click).map_err(|e| e.to_string())?;
        enigo.key(Key::Control, Direction::Release).map_err(|e| e.to_string())?;
    }

    Ok(())
}
