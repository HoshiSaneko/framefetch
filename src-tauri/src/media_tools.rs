use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

fn executable_name(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

fn executable(path: &Path) -> bool {
    if !path.is_file() { return false; }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return path.metadata().map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false);
    }
    #[cfg(not(unix))]
    true
}

pub fn tool(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    let name = executable_name(name);
    let mut roots = vec![app.path().resource_dir().map_err(|e| e.to_string())?.join("bin")];
    if cfg!(debug_assertions) {
        roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() { roots.push(parent.join("bin")); }
    }
    #[cfg(target_os = "macos")]
    roots.extend([PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/local/bin")]);
    if let Some(path) = std::env::var_os("PATH") { roots.extend(std::env::split_paths(&path)); }
    roots.into_iter().map(|root| root.join(&name)).find(|p| executable(p))
        .ok_or_else(|| format!("缺少下载组件 {name}，请准备下载组件后重试"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_match_the_host_platform() {
        assert_eq!(executable_name("yt-dlp"), if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" });
    }
    #[cfg(unix)]
    #[test]
    fn rejects_non_executable_files() {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!("framefetch-tool-{}", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"test").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!executable(&path));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(executable(&path));
        std::fs::remove_file(path).unwrap();
    }
}
