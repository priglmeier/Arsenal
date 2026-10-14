//! Reading and writing the server config file.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use super::model::ServerConfig;

/// Loads a server config from disk. A UTF-8 byte order mark is tolerated.
pub fn load(path: &Path) -> Result<ServerConfig> {
    let text =
        fs::read_to_string(path).with_context(|| format!("Could not read {}", path.display()))?;
    let text = text.trim_start_matches('\u{feff}');
    serde_json::from_str(text).with_context(|| format!("{} is not a valid server config", path.display()))
}

/// Pretty prints with tab indentation, matching the BI examples.
pub fn to_pretty_string(cfg: &ServerConfig) -> Result<String> {
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    cfg.serialize(&mut ser)?;
    Ok(String::from_utf8(buf)?)
}

/// Saves the config. The previous file is copied to `<name>.bak` first, and the
/// new content is written to a temporary file and renamed into place so a crash
/// mid-write cannot leave a half-written config behind.
pub fn save(path: &Path, cfg: &ServerConfig) -> Result<()> {
    let json = to_pretty_string(cfg)?;

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Could not create {}", parent.display()))?;
        }
    }
    if path.exists() {
        let _ = fs::copy(path, backup_path(path));
    }

    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json.as_bytes())
        .with_context(|| format!("Could not write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("Could not replace {}", path.display()))?;
    Ok(())
}

fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".bak");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("arsenal-test-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn save_then_load_returns_the_same_config() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("server.json");
        let mut cfg = ServerConfig::default();
        cfg.game.name = "Test Server".into();
        cfg.game.admins.push("76561198200329058".into());
        save(&path, &cfg).unwrap();
        assert_eq!(load(&path).unwrap(), cfg);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn saving_twice_keeps_a_backup_of_the_first_version() {
        let dir = temp_dir("backup");
        let path = dir.join("server.json");
        let mut cfg = ServerConfig::default();
        cfg.game.name = "first".into();
        save(&path, &cfg).unwrap();
        cfg.game.name = "second".into();
        save(&path, &cfg).unwrap();
        let backup = load(&backup_path(&path)).unwrap();
        assert_eq!(backup.game.name, "first");
        assert_eq!(load(&path).unwrap().game.name, "second");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn output_is_tab_indented() {
        let text = to_pretty_string(&ServerConfig::default()).unwrap();
        assert!(text.contains("\n\t\"bindAddress\""));
    }

    #[test]
    fn invalid_json_gives_a_readable_error() {
        let dir = temp_dir("invalid");
        let path = dir.join("bad.json");
        fs::write(&path, "{ \"game\": { } \"operating\": {} }").unwrap();
        let err = load(&path).unwrap_err();
        assert!(format!("{err:#}").contains("not a valid server config"));
        let _ = fs::remove_dir_all(&dir);
    }
}
