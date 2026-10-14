//! Validation of a server config against the limits documented on the BI wiki.

use std::collections::HashSet;
use std::net::IpAddr;

use super::model::{ServerConfig, PLATFORMS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub severity: Severity,
    /// Dotted path of the setting, e.g. `game.maxPlayers`.
    pub field: String,
    pub message: String,
}

struct Report(Vec<Issue>);

impl Report {
    fn error(&mut self, field: &str, message: impl Into<String>) {
        self.0.push(Issue {
            severity: Severity::Error,
            field: field.into(),
            message: message.into(),
        });
    }

    fn warn(&mut self, field: &str, message: impl Into<String>) {
        self.0.push(Issue {
            severity: Severity::Warning,
            field: field.into(),
            message: message.into(),
        });
    }

    fn port(&mut self, field: &str, port: u16) {
        if port == 0 {
            self.error(field, "Port must be between 1 and 65535.");
        }
    }

    fn range(&mut self, field: &str, value: i64, min: i64, max: i64) {
        if value < min || value > max {
            self.error(field, format!("Must be between {min} and {max} (currently {value})."));
        }
    }

    fn ip(&mut self, field: &str, value: &str, required: bool) {
        if value.trim().is_empty() {
            if required {
                self.warn(field, "Should be set to an IP address.");
            }
        } else if value.trim().parse::<IpAddr>().is_err() {
            self.error(field, format!("'{value}' is not a valid IP address."));
        }
    }
}

pub fn validate(cfg: &ServerConfig) -> Vec<Issue> {
    let mut r = Report(Vec::new());

    r.ip("bindAddress", &cfg.bind_address, false);
    r.port("bindPort", cfg.bind_port);
    r.ip("publicAddress", &cfg.public_address, false);
    r.port("publicPort", cfg.public_port);

    r.ip("a2s.address", &cfg.a2s.address, true);
    r.port("a2s.port", cfg.a2s.port);

    if let Some(rcon) = &cfg.rcon {
        r.ip("rcon.address", &rcon.address, true);
        r.port("rcon.port", rcon.port);
        if rcon.password.is_empty() {
            r.error("rcon.password", "RCON is enabled but has no password.");
        } else if rcon.password.contains(char::is_whitespace) {
            r.error("rcon.password", "The RCON password must not contain spaces.");
        } else if rcon.password.chars().count() < 3 {
            r.warn("rcon.password", "Very short password; use at least 3 characters.");
        }
        if rcon.permission != "admin" && rcon.permission != "monitor" {
            r.error("rcon.permission", "Must be 'admin' or 'monitor'.");
        }
        if let Some(max) = rcon.max_clients {
            r.range("rcon.maxClients", max as i64, 1, 16);
        }
    }

    let game = &cfg.game;
    if game.name.trim().is_empty() {
        r.warn("game.name", "The server has no name.");
    }
    if game.name.chars().count() > 100 {
        r.error("game.name", "The server name can be at most 100 characters.");
    }
    r.range("game.maxPlayers", game.max_players as i64, 1, 256);

    if game.scenario_id.trim().is_empty() {
        r.error("game.scenarioId", "No scenario selected.");
    } else if !looks_like_scenario_id(&game.scenario_id) {
        r.warn(
            "game.scenarioId",
            "Expected a path like {ECC61978EDCC2B5A}Missions/23_Campaign.conf.",
        );
    }

    for admin in &game.admins {
        if !looks_like_admin_id(admin) {
            r.warn(
                "game.admins",
                format!("'{admin}' is neither a 17 digit Steam ID nor an identity ID."),
            );
        }
    }

    for platform in &game.supported_platforms {
        if !PLATFORMS.contains(&platform.as_str()) {
            r.warn(
                "game.supportedPlatforms",
                format!("Unknown platform '{platform}'."),
            );
        }
    }

    let props = &game.game_properties;
    r.range(
        "gameProperties.serverMaxViewDistance",
        props.server_max_view_distance as i64,
        500,
        10000,
    );
    let grass = props.server_min_grass_distance;
    if grass != 0 && !(50..=150).contains(&grass) {
        r.error(
            "gameProperties.serverMinGrassDistance",
            "Must be 0 (not forced) or between 50 and 150.",
        );
    }
    r.range(
        "gameProperties.networkViewDistance",
        props.network_view_distance as i64,
        500,
        5000,
    );
    if !props.battl_eye {
        r.warn("gameProperties.battlEye", "BattlEye is disabled.");
    }

    let mut seen = HashSet::new();
    for (i, m) in game.mods.iter().enumerate() {
        let field = format!("game.mods[{i}]");
        if !is_mod_id(&m.mod_id) {
            r.error(
                &field,
                format!("'{}' is not a valid mod ID (16 hexadecimal characters).", m.mod_id),
            );
        } else if !seen.insert(m.mod_id.to_ascii_uppercase()) {
            r.warn(&field, format!("Mod {} is listed more than once.", m.mod_id));
        }
    }

    let op = &cfg.operating;
    if let Some(v) = op.slot_reservation_timeout {
        r.range("operating.slotReservationTimeout", v as i64, 5, 300);
    }
    if let Some(v) = op.player_save_time {
        if v == 0 {
            r.warn("operating.playerSaveTime", "A save interval of 0 seconds is unusual.");
        }
    }

    r.0
}

pub fn has_errors(issues: &[Issue]) -> bool {
    issues.iter().any(|i| i.severity == Severity::Error)
}

/// A mod ID is 16 hexadecimal characters.
pub fn is_mod_id(id: &str) -> bool {
    id.len() == 16 && id.chars().all(|c| c.is_ascii_hexdigit())
}

/// `{16 hex characters}path/to/scenario.conf`
pub fn looks_like_scenario_id(id: &str) -> bool {
    let id = id.trim();
    let Some(rest) = id.strip_prefix('{') else {
        return false;
    };
    let Some((guid, path)) = rest.split_once('}') else {
        return false;
    };
    is_mod_id(guid) && !path.is_empty() && path.to_ascii_lowercase().ends_with(".conf")
}

/// Steam64 IDs are 17 digits; identity IDs are UUIDs.
fn looks_like_admin_id(id: &str) -> bool {
    let steam = id.len() == 17 && id.chars().all(|c| c.is_ascii_digit());
    let uuid = id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    steam || uuid
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::model::{Mod, Rcon};

    fn fields(issues: &[Issue], severity: Severity) -> Vec<&str> {
        issues
            .iter()
            .filter(|i| i.severity == severity)
            .map(|i| i.field.as_str())
            .collect()
    }

    #[test]
    fn default_config_has_no_errors() {
        let issues = validate(&ServerConfig::default());
        assert!(!has_errors(&issues), "{issues:?}");
    }

    #[test]
    fn zero_ports_are_errors() {
        let mut cfg = ServerConfig::default();
        cfg.bind_port = 0;
        cfg.a2s.port = 0;
        let issues = validate(&cfg);
        let errors = fields(&issues, Severity::Error);
        assert!(errors.contains(&"bindPort"));
        assert!(errors.contains(&"a2s.port"));
    }

    #[test]
    fn player_and_distance_limits_are_enforced() {
        let mut cfg = ServerConfig::default();
        cfg.game.max_players = 257;
        cfg.game.game_properties.server_max_view_distance = 100;
        cfg.game.game_properties.server_min_grass_distance = 20;
        cfg.game.game_properties.network_view_distance = 9000;
        let issues = validate(&cfg);
        let errors = fields(&issues, Severity::Error);
        assert!(errors.contains(&"game.maxPlayers"));
        assert!(errors.contains(&"gameProperties.serverMaxViewDistance"));
        assert!(errors.contains(&"gameProperties.serverMinGrassDistance"));
        assert!(errors.contains(&"gameProperties.networkViewDistance"));
    }

    #[test]
    fn rcon_rules() {
        let mut cfg = ServerConfig::default();
        cfg.rcon = Some(Rcon {
            password: "has space".into(),
            permission: "root".into(),
            max_clients: Some(40),
            ..Rcon::default()
        });
        let issues = validate(&cfg);
        let errors = fields(&issues, Severity::Error);
        assert!(errors.contains(&"rcon.password"));
        assert!(errors.contains(&"rcon.permission"));
        assert!(errors.contains(&"rcon.maxClients"));
    }

    #[test]
    fn mod_ids_are_checked() {
        let mut cfg = ServerConfig::default();
        cfg.game.mods = vec![
            Mod { mod_id: "59727DAE364DEADB".into(), ..Mod::default() },
            Mod { mod_id: "59727DAE364DEADB".into(), ..Mod::default() },
            Mod { mod_id: "not-an-id".into(), ..Mod::default() },
        ];
        let issues = validate(&cfg);
        assert_eq!(fields(&issues, Severity::Error), vec!["game.mods[2]"]);
        assert_eq!(fields(&issues, Severity::Warning), vec!["game.mods[1]"]);
    }

    #[test]
    fn scenario_id_format() {
        assert!(looks_like_scenario_id("{ECC61978EDCC2B5A}Missions/23_Campaign.conf"));
        assert!(!looks_like_scenario_id("Missions/23_Campaign.conf"));
        assert!(!looks_like_scenario_id("{ECC61978}Missions/23_Campaign.conf"));
        assert!(!looks_like_scenario_id("{ECC61978EDCC2B5A}Missions/23_Campaign"));
    }

    #[test]
    fn admin_ids() {
        assert!(looks_like_admin_id("76561198200329058"));
        assert!(looks_like_admin_id("1b4c9f1e-8a3d-4d55-9c1a-2f6f0a7c9b10"));
        assert!(!looks_like_admin_id("somebody"));
    }
}
