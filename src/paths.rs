//! The standard folder layout under the Arma Reforger Server folder, and discovery of
//! the save points the server writes. Everything is built with `Path::join`, so the
//! separators are right on both Windows and Linux.
//!
//! Layout used by Arsenal (relative to the server folder):
//!
//! ```text
//! saves                          -profile (the server adds its own "profile" folder inside it)
//! saves/profile/.save/game/<scenario>/playthrough000/savepoint186
//! saves/logs                     -logsDir (also the default: <-profile>/logs)
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const SAVES_DIR: &str = "saves";
const PROFILE_DIR: &str = "profile";
const LOGS_DIR: &str = "logs";
/// Hidden folder the game keeps inside the profile folder for saved sessions.
const SAVE_DIR: &str = ".save";
const GAME_DIR: &str = "game";

/// The folders Arsenal proposes when the server folder is chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardFolders {
    pub profile: PathBuf,
    pub logs: PathBuf,
}

/// One `savepoint###` folder found in the `.save` folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePoint {
    /// The scenario folder, like `ADA0A27D7010ED07-MSTP-Taconite-Conflict`.
    pub scenario: String,
    /// Like `playthrough000`.
    pub playthrough: String,
    /// Like `savepoint186`.
    pub name: String,
    pub path: PathBuf,
    pub modified: Option<SystemTime>,
}

/// The server folder itself, given either that folder or the path to the executable in it.
pub fn server_root(server_path: &Path) -> PathBuf {
    if server_path.is_file() {
        if let Some(parent) = server_path.parent() {
            return parent.to_path_buf();
        }
    }
    server_path.to_path_buf()
}

/// `profile` is the value for `-profile`. The server does not use that folder directly: it
/// works in a `profile` folder inside it (see [`save_dir`]). Pointing `-profile` at
/// `saves/profile` would make the server use `saves/profile/profile` and miss every save.
pub fn standard_folders(root: &Path) -> StandardFolders {
    let saves = root.join(SAVES_DIR);
    StandardFolders {
        profile: saves.clone(),
        logs: saves.join(LOGS_DIR),
    }
}

/// The value `-profile` had in Arsenal's first versions (`saves/profile`). It leads the
/// server to an empty `saves/profile/profile`, so it is replaced when found.
pub fn old_profile_value(root: &Path) -> PathBuf {
    root.join(SAVES_DIR).join(PROFILE_DIR)
}

/// Where the game keeps saved sessions, given the value of `-profile`.
pub fn save_dir(profile_arg: &Path) -> PathBuf {
    profile_arg.join(PROFILE_DIR).join(SAVE_DIR)
}

/// The folder logs end up in. `value` is the `-logsDir` setting: a full path is used as is,
/// a bare name is a folder inside `<profile>/logs`, and empty means the default `<profile>/logs`.
pub fn resolve_logs(profile: Option<&Path>, value: &str) -> Option<PathBuf> {
    let value = value.trim();
    if !value.is_empty() && Path::new(value).is_absolute() {
        return Some(PathBuf::from(value));
    }
    let logs = profile?.join(LOGS_DIR);
    Some(if value.is_empty() { logs } else { logs.join(value) })
}

/// Finds every `<save>/game/<scenario>/playthrough*/savepoint*` folder. Newest first within
/// each playthrough. A missing or unreadable folder just gives an empty list.
pub fn find_savepoints(save_dir: &Path) -> Vec<SavePoint> {
    let mut found = Vec::new();
    for scenario in sub_dirs(&save_dir.join(GAME_DIR)) {
        for playthrough in sub_dirs(&scenario) {
            if !name_starts_with(&playthrough, "playthrough") {
                continue;
            }
            for savepoint in sub_dirs(&playthrough) {
                if !name_starts_with(&savepoint, "savepoint") {
                    continue;
                }
                found.push(SavePoint {
                    scenario: file_name(&scenario),
                    playthrough: file_name(&playthrough),
                    name: file_name(&savepoint),
                    modified: fs::metadata(&savepoint).and_then(|m| m.modified()).ok(),
                    path: savepoint,
                });
            }
        }
    }
    found.sort_by(|a, b| {
        a.scenario
            .cmp(&b.scenario)
            .then_with(|| a.playthrough.cmp(&b.playthrough))
            .then_with(|| trailing_number(&b.name).cmp(&trailing_number(&a.name)))
            .then_with(|| b.name.cmp(&a.name))
    });
    found
}

fn sub_dirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map_or(false, |t| t.is_dir()))
        .map(|entry| entry.path())
        .collect();
    dirs.sort();
    dirs
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn name_starts_with(path: &Path, prefix: &str) -> bool {
    file_name(path).to_ascii_lowercase().starts_with(prefix)
}

/// `savepoint186` -> 186. Names without digits sort as 0.
fn trailing_number(name: &str) -> u64 {
    name.trim_start_matches(|c: char| !c.is_ascii_digit())
        .parse()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("arsenal-paths-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn standard_folders_are_under_saves() {
        let root = Path::new("server");
        let folders = standard_folders(root);
        assert_eq!(folders.profile, root.join("saves"));
        assert_eq!(folders.logs, root.join("saves").join("logs"));
        assert_eq!(old_profile_value(root), root.join("saves").join("profile"));
        assert_eq!(
            save_dir(&folders.profile),
            root.join("saves").join("profile").join(".save")
        );
    }

    #[test]
    fn root_of_an_executable_is_its_folder() {
        let dir = temp_dir("root");
        let exe = dir.join("ArmaReforgerServer");
        fs::write(&exe, b"").unwrap();
        assert_eq!(server_root(&exe), dir);
        assert_eq!(server_root(&dir), dir);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn logs_resolve_like_the_server_does() {
        let profile = Path::new("p");
        assert_eq!(resolve_logs(Some(profile), ""), Some(profile.join("logs")));
        assert_eq!(
            resolve_logs(Some(profile), "Session1"),
            Some(profile.join("logs").join("Session1"))
        );
        assert_eq!(resolve_logs(None, ""), None);

        let absolute = std::env::temp_dir().join("elsewhere");
        let text = absolute.to_string_lossy().into_owned();
        assert_eq!(resolve_logs(None, &text), Some(absolute));
    }

    #[test]
    fn savepoints_are_found_newest_first() {
        let save = temp_dir("saves").join(".save");
        let playthrough = save
            .join("game")
            .join("ADA0A27D7010ED07-MSTP-Taconite-Conflict")
            .join("playthrough000");
        for name in ["savepoint9", "savepoint186", "savepoint10", "notes"] {
            fs::create_dir_all(playthrough.join(name)).unwrap();
        }
        // Files and unrelated folders are ignored.
        fs::write(playthrough.join("savepoint11"), b"").unwrap();
        fs::create_dir_all(save.join("game").join("Other").join("misc")).unwrap();

        let found = find_savepoints(&save);
        let names: Vec<&str> = found.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["savepoint186", "savepoint10", "savepoint9"]);
        assert_eq!(found[0].scenario, "ADA0A27D7010ED07-MSTP-Taconite-Conflict");
        assert_eq!(found[0].playthrough, "playthrough000");
        assert_eq!(found[0].path, playthrough.join("savepoint186"));
        let _ = fs::remove_dir_all(save.parent().unwrap());
    }

    #[test]
    fn a_missing_save_folder_gives_an_empty_list() {
        assert!(find_savepoints(Path::new("/definitely/not/here/.save")).is_empty());
    }
}
