//! Serde model of the Arma Reforger dedicated server `config.json`.
//!
//! Reference: https://community.bistudio.com/wiki/Arma_Reforger:Server_Config
//!
//! Every struct carries a flattened `extra` map. Any key this launcher does not
//! know about (new game versions add keys regularly) is kept and written back
//! unchanged instead of being silently dropped.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

pub type Extra = Map<String, Value>;

/// Values accepted by `game.supportedPlatforms`.
pub const PLATFORMS: &[&str] = &["PLATFORM_PC", "PLATFORM_XBL", "PLATFORM_PSN"];

fn is_false(v: &bool) -> bool {
    !*v
}

#[allow(clippy::ptr_arg)]
fn is_empty_vec<T>(v: &Vec<T>) -> bool {
    v.is_empty()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerConfig {
    pub bind_address: String,
    pub bind_port: u16,
    pub public_address: String,
    pub public_port: u16,
    pub a2s: A2s,
    /// RCON is optional; `None` means the whole `rcon` object is left out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rcon: Option<Rcon>,
    pub game: Game,
    pub operating: Operating,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".into(),
            bind_port: 2001,
            public_address: String::new(),
            public_port: 2001,
            a2s: A2s::default(),
            rcon: None,
            game: Game::default(),
            operating: Operating::default(),
            extra: Extra::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct A2s {
    pub address: String,
    pub port: u16,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for A2s {
    fn default() -> Self {
        Self {
            address: "0.0.0.0".into(),
            port: 17777,
            extra: Extra::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Rcon {
    pub address: String,
    pub port: u16,
    pub password: String,
    /// "admin" or "monitor".
    pub permission: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_clients: Option<u32>,
    pub blacklist: Vec<String>,
    pub whitelist: Vec<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for Rcon {
    fn default() -> Self {
        Self {
            address: "0.0.0.0".into(),
            port: 19999,
            password: String::new(),
            permission: "monitor".into(),
            max_clients: None,
            blacklist: Vec::new(),
            whitelist: Vec::new(),
            extra: Extra::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Game {
    pub name: String,
    pub password: String,
    pub password_admin: String,
    /// Steam IDs and/or identity IDs.
    pub admins: Vec<String>,
    pub scenario_id: String,
    pub max_players: u32,
    pub visible: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub cross_platform: bool,
    #[serde(skip_serializing_if = "is_empty_vec")]
    pub supported_platforms: Vec<String>,
    pub game_properties: GameProperties,
    pub mods: Vec<Mod>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for Game {
    fn default() -> Self {
        Self {
            name: "My Arma Reforger Server".into(),
            password: String::new(),
            password_admin: String::new(),
            admins: Vec::new(),
            scenario_id: "{ECC61978EDCC2B5A}Missions/23_Campaign.conf".into(),
            max_players: 64,
            visible: true,
            cross_platform: false,
            supported_platforms: Vec::new(),
            game_properties: GameProperties::default(),
            mods: Vec::new(),
            extra: Extra::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameProperties {
    pub server_max_view_distance: u32,
    pub server_min_grass_distance: u32,
    pub network_view_distance: u32,
    pub disable_third_person: bool,
    pub fast_validation: bool,
    #[serde(rename = "battlEye")]
    pub battl_eye: bool,
    #[serde(rename = "VONDisableUI")]
    pub von_disable_ui: bool,
    #[serde(rename = "VONDisableDirectSpeechUI")]
    pub von_disable_direct_speech_ui: bool,
    #[serde(rename = "VONCanTransmitCrossFaction")]
    pub von_can_transmit_cross_faction: bool,
    /// Free-form overrides of the scenario's mission header.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mission_header: Option<Map<String, Value>>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for GameProperties {
    fn default() -> Self {
        Self {
            server_max_view_distance: 1600,
            server_min_grass_distance: 0,
            network_view_distance: 1500,
            disable_third_person: false,
            fast_validation: true,
            battl_eye: true,
            von_disable_ui: false,
            von_disable_direct_speech_ui: false,
            von_can_transmit_cross_faction: false,
            mission_header: None,
            extra: Extra::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Mod {
    pub mod_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// The `operating` block. Every field is optional: `None` means the key is not
/// written and the server uses its own default.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Operating {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lobby_player_synchronise: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub join_queue: Option<JoinQueue>,
    /// Absent: nothing disabled. Empty list: navmesh streaming disabled for all.
    /// Filled list: disabled only for the listed navmeshes (game 1.2.0 and later).
    /// Older configs used a plain boolean, which is converted on load.
    #[serde(
        deserialize_with = "de_navmesh",
        skip_serializing_if = "Option::is_none"
    )]
    pub disable_navmesh_streaming: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_server_shutdown: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_crash_reporter: Option<bool>,
    #[serde(rename = "disableAI", skip_serializing_if = "Option::is_none")]
    pub disable_ai: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player_save_time: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_reservation_timeout: Option<u32>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct JoinQueue {
    pub max_size: u32,
    #[serde(flatten)]
    pub extra: Extra,
}

fn de_navmesh<'de, D>(d: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(d)?;
    Ok(match value {
        Some(Value::Bool(true)) => Some(Vec::new()),
        Some(Value::Array(items)) => Some(
            items
                .into_iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
        ),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The example config from the BI wiki, as supplied for this project.
    const EXAMPLE: &str = r#"{
        "bindAddress": "0.0.0.0",
        "bindPort": 2001,
        "publicAddress": "192.168.9.10",
        "publicPort": 2001,
        "a2s": { "address": "192.168.9.10", "port": 17777 },
        "rcon": {
            "address": "192.168.9.10",
            "port": 19999,
            "password": "changeme_withoutspaces",
            "permission": "monitor",
            "blacklist": [],
            "whitelist": []
        },
        "game": {
            "name": "Server Name - Mission Name",
            "password": "",
            "passwordAdmin": "changeme",
            "admins": ["76561198200329058"],
            "scenarioId": "{ECC61978EDCC2B5A}Missions/23_Campaign.conf",
            "maxPlayers": 32,
            "visible": true,
            "crossPlatform": true,
            "supportedPlatforms": ["PLATFORM_PC", "PLATFORM_XBL"],
            "gameProperties": {
                "serverMaxViewDistance": 2500,
                "serverMinGrassDistance": 50,
                "networkViewDistance": 1000,
                "disableThirdPerson": true,
                "fastValidation": true,
                "battlEye": true,
                "VONDisableUI": true,
                "VONDisableDirectSpeechUI": true,
                "missionHeader": {
                    "m_iPlayerCount": 40,
                    "m_eEditableGameFlags": 6,
                    "m_eDefaultGameFlags": 6,
                    "other": "values"
                }
            },
            "mods": [
                { "modId": "59727DAE364DEADB", "name": "WeaponSwitching", "version": "1.0.1" },
                { "modId": "59727DAE32981C7D", "name": "Explosive Goats beta", "version": "0.5.42" }
            ]
        },
        "operating": {
            "lobbyPlayerSynchronise": true,
            "joinQueue": { "maxSize": 12 },
            "disableNavmeshStreaming": ["Soldiers", "BTRlike"]
        }
    }"#;

    #[test]
    fn parses_the_example_config() {
        let cfg: ServerConfig = serde_json::from_str(EXAMPLE).unwrap();
        assert_eq!(cfg.bind_port, 2001);
        assert_eq!(cfg.game.max_players, 32);
        assert_eq!(cfg.game.mods.len(), 2);
        assert_eq!(cfg.game.mods[0].mod_id, "59727DAE364DEADB");
        assert_eq!(cfg.rcon.as_ref().unwrap().port, 19999);
        assert_eq!(cfg.operating.join_queue.as_ref().unwrap().max_size, 12);
        assert_eq!(
            cfg.operating.disable_navmesh_streaming,
            Some(vec!["Soldiers".to_string(), "BTRlike".to_string()])
        );
        let header = cfg.game.game_properties.mission_header.as_ref().unwrap();
        assert_eq!(header["m_iPlayerCount"], 40);
    }

    #[test]
    fn round_trip_is_lossless() {
        let first: ServerConfig = serde_json::from_str(EXAMPLE).unwrap();
        let text = serde_json::to_string_pretty(&first).unwrap();
        let second: ServerConfig = serde_json::from_str(&text).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn unknown_keys_survive_a_round_trip() {
        let json = r#"{
            "bindPort": 2002,
            "someFutureKey": { "a": 1 },
            "game": { "name": "x", "newGameKey": [1, 2, 3], "mods": [
                { "modId": "AAAAAAAAAAAAAAAA", "name": "m", "required": true }
            ] }
        }"#;
        let cfg: ServerConfig = serde_json::from_str(json).unwrap();
        let out = serde_json::to_value(&cfg).unwrap();
        assert_eq!(out["someFutureKey"]["a"], 1);
        assert_eq!(out["game"]["newGameKey"][2], 3);
        assert_eq!(out["game"]["mods"][0]["required"], true);
    }

    #[test]
    fn old_boolean_navmesh_setting_is_converted() {
        let cfg: ServerConfig =
            serde_json::from_str(r#"{ "operating": { "disableNavmeshStreaming": true } }"#)
                .unwrap();
        assert_eq!(cfg.operating.disable_navmesh_streaming, Some(Vec::new()));
        let cfg: ServerConfig =
            serde_json::from_str(r#"{ "operating": { "disableNavmeshStreaming": false } }"#)
                .unwrap();
        assert_eq!(cfg.operating.disable_navmesh_streaming, None);
    }

    #[test]
    fn optional_blocks_are_left_out_when_unset() {
        let out = serde_json::to_value(ServerConfig::default()).unwrap();
        assert!(out.get("rcon").is_none());
        assert!(out["game"].get("crossPlatform").is_none());
        assert!(out["operating"].get("playerSaveTime").is_none());
        assert_eq!(out["game"]["gameProperties"]["battlEye"], true);
    }

    #[test]
    fn special_key_names_are_written_exactly() {
        let mut cfg = ServerConfig::default();
        cfg.operating.disable_ai = Some(true);
        let out = serde_json::to_value(&cfg).unwrap();
        assert_eq!(out["operating"]["disableAI"], true);
        let props = &out["game"]["gameProperties"];
        assert!(props.get("VONDisableUI").is_some());
        assert!(props.get("VONDisableDirectSpeechUI").is_some());
        assert!(props.get("VONCanTransmitCrossFaction").is_some());
    }
}
