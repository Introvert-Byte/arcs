use std::path::Path;

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{:.1} {}", size, UNITS[unit])
    }
}

pub fn ensure_parent(path: impl AsRef<Path>) -> std::io::Result<()> {
    if let Some(p) = path.as_ref().parent() {
        std::fs::create_dir_all(p)?;
    }
    Ok(())
}

pub fn current_os_arch() -> (String, String) {
    (
        std::env::consts::OS.to_string(),
        std::env::consts::ARCH.to_string(),
    )
}