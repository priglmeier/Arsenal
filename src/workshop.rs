//! Helpers for the Arma Reforger Workshop website: reading a mod's page address and
//! opening the site in the user's web browser.
//!
//! A mod page address looks like
//! `https://reforger.armaplatform.com/workshop/6A6B74B4B6D7ACF2-MSTP_Taconite_AGI`
//! where the first 16 characters are the mod ID and the text after the dash is its name.

use std::io;
use std::process::{Command, Stdio};

use crate::config::validate::is_mod_id;

/// Where "Browse..." sends the user: the newest Workshop mods.
pub const WORKSHOP_URL: &str = "https://reforger.armaplatform.com/workshop?sort=newest";

const MOD_PATH: &str = "/workshop/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkshopMod {
    pub mod_id: String,
    /// Empty when the link only contained an ID.
    pub name: String,
}

/// Reads a mod ID and name from a Workshop page address. Also accepts the bare
/// `ID-Name` part or just an ID. Returns `None` when no valid mod ID is found.
pub fn parse_link(input: &str) -> Option<WorkshopMod> {
    let input = input.trim();
    let tail = match input.find(MOD_PATH) {
        Some(i) => &input[i + MOD_PATH.len()..],
        None => input,
    };
    // Stop at the end of the path segment: sub pages, query strings and anchors are ignored.
    let slug = tail.split(['/', '?', '#']).next().unwrap_or("");
    let (id, name) = slug.split_once('-').unwrap_or((slug, ""));
    if !is_mod_id(id) {
        return None;
    }
    Some(WorkshopMod {
        mod_id: id.to_ascii_uppercase(),
        name: percent_decode(name),
    })
}

/// Decodes `%20` style escapes. Anything that is not a valid escape is kept as is.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(value) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(value);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Opens a web address in the default browser on Windows, Linux and macOS.
pub fn open_in_browser(url: &str) -> io::Result<()> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = Command::new("open");
        c.arg(url);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };

    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    // Reap the helper process so it does not linger as a zombie.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_full_workshop_address() {
        let found = parse_link("https://reforger.armaplatform.com/workshop/6A6B74B4B6D7ACF2-MSTP_Taconite_AGI")
            .unwrap();
        assert_eq!(found.mod_id, "6A6B74B4B6D7ACF2");
        assert_eq!(found.name, "MSTP_Taconite_AGI");
    }

    #[test]
    fn ignores_sub_pages_queries_and_whitespace() {
        let found = parse_link("  https://reforger.armaplatform.com/workshop/6A6B74B4B6D7ACF2-MSTP_Taconite_AGI/dependencies?tab=1#top \n")
            .unwrap();
        assert_eq!(found.mod_id, "6A6B74B4B6D7ACF2");
        assert_eq!(found.name, "MSTP_Taconite_AGI");
    }

    #[test]
    fn keeps_dashes_inside_the_name() {
        let found = parse_link("https://reforger.armaplatform.com/workshop/59727DAE364DEADB-Weapon-Switching-Fix").unwrap();
        assert_eq!(found.name, "Weapon-Switching-Fix");
    }

    #[test]
    fn accepts_bare_slug_or_id() {
        assert_eq!(parse_link("6a6b74b4b6d7acf2-Name").unwrap().mod_id, "6A6B74B4B6D7ACF2");
        let only_id = parse_link("6A6B74B4B6D7ACF2").unwrap();
        assert_eq!(only_id.mod_id, "6A6B74B4B6D7ACF2");
        assert_eq!(only_id.name, "");
    }

    #[test]
    fn decodes_percent_escapes() {
        let found = parse_link("https://reforger.armaplatform.com/workshop/6A6B74B4B6D7ACF2-My%20Mod").unwrap();
        assert_eq!(found.name, "My Mod");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    #[test]
    fn rejects_other_text() {
        assert_eq!(parse_link(""), None);
        assert_eq!(parse_link("https://example.com/workshop/not-a-mod"), None);
        assert_eq!(parse_link("https://reforger.armaplatform.com/workshop?page=5&sort=newest"), None);
        assert_eq!(parse_link("6A6B74B4B6D7ACF"), None);
    }
}
