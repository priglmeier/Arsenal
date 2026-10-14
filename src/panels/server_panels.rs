//! The settings panels that edit `config.json`: Server, Network, RCON, Gameplay and Operating.

use egui::{CollapsingHeader, ComboBox, Grid, RichText, Ui};

use crate::config::{JoinQueue, Rcon, PLATFORMS};
use crate::state::AppState;

use super::widgets::*;

fn grid(id: &str) -> Grid {
    Grid::new(id)
        .num_columns(2)
        .spacing([16.0, 8.0])
        .striped(true)
        .min_col_width(170.0)
}

pub fn server(state: &mut AppState, ui: &mut Ui) {
    let show_passwords = state.ui.show_passwords;
    let game = &mut state.config.game;

    grid("server_grid").show(ui, |ui| {
        text_row(
            ui,
            "Server Name",
            "The name shown in the server browser (at most 100 characters).",
            &mut game.name,
        );
        secret_row(
            ui,
            "Server Password",
            "Password required to join. Leave empty for a public server.",
            &mut game.password,
            show_passwords,
        );
        secret_row(
            ui,
            "Admin Password",
            "Lets an administrator log in with #login <password> in chat.",
            &mut game.password_admin,
            show_passwords,
        );
        int_row(
            ui,
            "Max Players",
            "Maximum number of players, 1 to 256.",
            &mut game.max_players,
            1,
            256,
        );
        bool_row(
            ui,
            "Server Visible",
            "Whether the server is listed in the server browser.",
            &mut game.visible,
        );
        bool_row(
            ui,
            "Cross Platform",
            "When on, console clients can connect (adds PLATFORM_PC and PLATFORM_XBL to the supported platforms if missing). Tick PLATFORM_PSN below to allow PlayStation.",
            &mut game.cross_platform,
        );
        text_row(
            ui,
            "Scenario ID",
            "Scenario to load, like {ECC61978EDCC2B5A}Missions/23_Campaign.conf. Use Select Scenario to pick one.",
            &mut game.scenario_id,
        );
    });
    ui.checkbox(&mut state.ui.show_passwords, "Show Passwords");

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button("Select Scenario...").clicked() {
            state.ui.show_scenarios = true;
        }
        if ui.button("Mission Header...").clicked() {
            state.ui.mission_header_text = match &state.config.game.game_properties.mission_header {
                Some(header) => serde_json::to_string_pretty(header).unwrap_or_default(),
                None => String::new(),
            };
            state.ui.mission_header_error = None;
            state.ui.show_mission_header = true;
        }
    });

    ui.add_space(6.0);
    ui.label("Supported Platforms")
        .on_hover_text("Platforms that may join. If none is ticked the server default (PC) is used.");
    ui.horizontal(|ui| {
        for platform in PLATFORMS {
            toggle_in_list(ui, &mut state.config.game.supported_platforms, platform);
        }
    });

    ui.add_space(6.0);
    CollapsingHeader::new(format!("Admins ({})", state.config.game.admins.len()))
        .id_source("admins_header")
        .default_open(true)
        .show(ui, |ui| {
            ui.label(
                RichText::new("Steam IDs or identity IDs. Listed players can #login without a password.")
                    .small()
                    .weak(),
            );
            string_list(
                ui,
                "admins",
                &mut state.config.game.admins,
                "Steam ID or Identity ID",
            );
        });
}

pub fn network(state: &mut AppState, ui: &mut Ui) {
    let cfg = &mut state.config;
    grid("network_grid").show(ui, |ui| {
        text_row(
            ui,
            "Bind Address",
            "Network interface the server socket binds to. Usually 0.0.0.0 or empty (all interfaces).",
            &mut cfg.bind_address,
        );
        port_row(
            ui,
            "Bind Port",
            "UDP port the server socket binds to. Default 2001.",
            &mut cfg.bind_port,
        );
        text_row(
            ui,
            "Public Address",
            "Public IP clients use to reach the server. Empty or 0.0.0.0 means it is detected automatically.",
            &mut cfg.public_address,
        );
        port_row(
            ui,
            "Public Port",
            "UDP port registered with the backend. Same as the bind port unless traffic is forwarded.",
            &mut cfg.public_port,
        );
        text_row(
            ui,
            "A2S Address",
            "Interface the Steam Query (A2S) socket binds to.",
            &mut cfg.a2s.address,
        );
        port_row(
            ui,
            "A2S Port",
            "UDP port for Steam Query requests. Default 17777.",
            &mut cfg.a2s.port,
        );
    });
}

pub fn rcon(state: &mut AppState, ui: &mut Ui) {
    let show_passwords = state.ui.show_passwords;

    let mut enabled = state.config.rcon.is_some();
    if ui.checkbox(&mut enabled, "Enable RCON").changed() {
        state.config.rcon = if enabled { Some(Rcon::default()) } else { None };
    }
    let Some(rcon) = state.config.rcon.as_mut() else {
        ui.weak("RCON is off, so no rcon block is written to the config.");
        return;
    };

    ui.add_space(4.0);
    let permission = rcon.permission.clone();
    grid("rcon_grid").show(ui, |ui| {
        text_row(
            ui,
            "RCON Address",
            "Interface the RCON socket binds to.",
            &mut rcon.address,
        );
        port_row(ui, "RCON Port", "UDP port for RCON. Default 19999.", &mut rcon.port);
        secret_row(
            ui,
            "RCON Password",
            "Required. Must not contain spaces.",
            &mut rcon.password,
            show_passwords,
        );
        ui.label("RCON Permission").on_hover_text(
            "admin allows every command; monitor only allows commands that do not change server state.",
        );
        ComboBox::from_id_source("rcon_permission")
            .selected_text(permission)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut rcon.permission, "monitor".to_string(), "monitor");
                ui.selectable_value(&mut rcon.permission, "admin".to_string(), "admin");
            });
        ui.end_row();
        opt_row(
            ui,
            "RCON Max Clients",
            "Maximum simultaneous RCON clients, 1 to 16 (server default 16).",
            &mut rcon.max_clients,
            16,
            |ui, v| {
                ui.add(egui::DragValue::new(v).clamp_range(1..=16));
            },
        );
    });

    ui.add_space(6.0);
    CollapsingHeader::new(format!("Whitelist ({})", rcon.whitelist.len()))
        .id_source("rcon_whitelist")
        .show(ui, |ui| {
            ui.label(
                RichText::new("If set, only these commands can be executed.")
                    .small()
                    .weak(),
            );
            string_list(ui, "rcon_whitelist", &mut rcon.whitelist, "Command Name");
        });
    CollapsingHeader::new(format!("Blacklist ({})", rcon.blacklist.len()))
        .id_source("rcon_blacklist")
        .show(ui, |ui| {
            ui.label(
                RichText::new("These commands cannot be executed.")
                    .small()
                    .weak(),
            );
            string_list(ui, "rcon_blacklist", &mut rcon.blacklist, "Command Name");
        });
}

pub fn gameplay(state: &mut AppState, ui: &mut Ui) {
    let props = &mut state.config.game.game_properties;
    grid("gameplay_grid").show(ui, |ui| {
        int_row(
            ui,
            "Max View Distance",
            "Maximum view distance in meters, enforced by the server. 500 to 10000.",
            &mut props.server_max_view_distance,
            500,
            10000,
        );
        int_row(
            ui,
            "Min Grass Distance",
            "Minimum grass distance in meters. 0 forces nothing on clients, otherwise 50 to 150.",
            &mut props.server_min_grass_distance,
            0,
            150,
        );
        int_row(
            ui,
            "Network View Distance",
            "Maximum network streaming range of replicated entities. 500 to 5000.",
            &mut props.network_view_distance,
            500,
            5000,
        );
        bool_row(
            ui,
            "Disable Third Person",
            "Forces clients into the first person view.",
            &mut props.disable_third_person,
        );
        bool_row(
            ui,
            "Fast Validation",
            "Clients receive only the minimum data needed to validate the map. Keep on for internet servers.",
            &mut props.fast_validation,
        );
        bool_row(ui, "BattlEye", "Enables the BattlEye anti-cheat.", &mut props.battl_eye);
        bool_row(
            ui,
            "VON: Disable UI",
            "Forces clients to hide the voice over network UI.",
            &mut props.von_disable_ui,
        );
        bool_row(
            ui,
            "VON: Disable Direct Speech UI",
            "Forces clients to hide the direct speech UI.",
            &mut props.von_disable_direct_speech_ui,
        );
        bool_row(
            ui,
            "VON: Transmit Cross Faction",
            "Lets players transmit on other factions' radios. When off they can only listen.",
            &mut props.von_can_transmit_cross_faction,
        );
    });

    ui.add_space(8.0);
    let overrides = props.mission_header.as_ref().map_or(0, |h| h.len());
    ui.label(format!("Mission Header Overrides: {overrides}"));
    ui.label(
        RichText::new("Edit them from the Server panel with Mission Header...")
            .small()
            .weak(),
    );
}

pub fn operating(state: &mut AppState, ui: &mut Ui) {
    let op = &mut state.config.operating;
    ui.label(
        RichText::new("Tick a setting to write it to the config; unticked settings use the server default.")
            .small()
            .weak(),
    );
    grid("operating_grid").show(ui, |ui| {
        opt_row(
            ui,
            "Lobby Player Synchronize",
            "Sends the list of player identities to the GameAPI with each heartbeat. Server default: on.",
            &mut op.lobby_player_synchronise,
            true,
            |ui, v| {
                ui.checkbox(v, "");
            },
        );
        opt_row(
            ui,
            "Join Queue Max Size",
            "Maximum number of players waiting in the join queue.",
            &mut op.join_queue,
            JoinQueue::default(),
            |ui, v| {
                ui.add(egui::DragValue::new(&mut v.max_size).clamp_range(0..=10000));
            },
        );
        opt_row(
            ui,
            "Disable Server Shutdown",
            "Keeps the server running if the backend connection is lost.",
            &mut op.disable_server_shutdown,
            true,
            |ui, v| {
                ui.checkbox(v, "");
            },
        );
        opt_row(
            ui,
            "Disable Crash Reporter",
            "Disables the automatic server side crash report.",
            &mut op.disable_crash_reporter,
            true,
            |ui, v| {
                ui.checkbox(v, "");
            },
        );
        opt_row(
            ui,
            "Disable AI",
            "Stops the AI world from initializing or ticking. Disables all AI on the server.",
            &mut op.disable_ai,
            true,
            |ui, v| {
                ui.checkbox(v, "");
            },
        );
        opt_row(
            ui,
            "Player Save Time (s)",
            "Interval in seconds for saving players. Server default 120.",
            &mut op.player_save_time,
            120,
            |ui, v| {
                ui.add(egui::DragValue::new(v).clamp_range(1..=86400));
            },
        );
        opt_row(
            ui,
            "AI Limit",
            "Maximum number of AIs. Negative values are ignored (no limit). Server default -1.",
            &mut op.ai_limit,
            -1,
            |ui, v| {
                ui.add(egui::DragValue::new(v).clamp_range(-1..=100000));
            },
        );
        opt_row(
            ui,
            "Slot Reservation Timeout (s)",
            "How long a slot stays reserved for a kicked player, 5 to 300 seconds. Server default 60.",
            &mut op.slot_reservation_timeout,
            60,
            |ui, v| {
                ui.add(egui::DragValue::new(v).clamp_range(5..=300));
            },
        );
    });

    ui.add_space(8.0);
    let mut navmesh_on = op.disable_navmesh_streaming.is_some();
    if ui
        .checkbox(&mut navmesh_on, "Disable Navmesh Streaming")
        .on_hover_text(
            "Loads the whole navmesh into memory: faster AI at the cost of up to hundreds of MB. \
             An empty list disables streaming for all navmeshes; add names to limit it to those.",
        )
        .changed()
    {
        op.disable_navmesh_streaming = if navmesh_on { Some(Vec::new()) } else { None };
    }
    if let Some(list) = op.disable_navmesh_streaming.as_mut() {
        ui.label(
            RichText::new("Empty list = all navmeshes. Names like Soldiers or BTRlike limit it.")
                .small()
                .weak(),
        );
        string_list(ui, "navmesh_list", list, "Navmesh Name");
    }
}
