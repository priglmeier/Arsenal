//! Startup parameters for `ArmaReforgerServer`, and the command line built from them.
//!
//! Reference: https://community.bistudio.com/wiki/Arma_Reforger:Startup_Parameters
//!
//! Parameters are described as data (`PARAMS`) so the Launch panel can render every
//! one of them from the same table. Anything not covered can go in the free-form
//! "extra arguments" field.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// A bare switch with no value.
    Flag,
    /// A value that is optional; the switch is passed alone when the value is empty.
    OptText,
    Text,
    Folder,
    Int { min: i64, max: i64 },
    OptInt { min: i64, max: i64 },
    Choice(&'static [&'static str]),
}

pub struct ParamSpec {
    pub flag: &'static str,
    pub label: &'static str,
    pub help: &'static str,
    pub group: &'static str,
    pub kind: Kind,
    /// Value shown when the parameter is first enabled.
    pub default: &'static str,
}

pub const GROUPS: &[&str] = &[
    "Performance",
    "Network",
    "Logging",
    "Paths and Addons",
    "Backend and AI",
];

const LOG_LEVELS: &[&str] = &["normal", "warning", "error", "fatal"];
const VM_ERROR_MODES: &[&str] = &["silent", "log_only", "full", "fatal"];

pub const PARAMS: &[ParamSpec] = &[
    // --- Performance ---
    ParamSpec {
        flag: "-maxFPS",
        label: "Limit Server Max FPS",
        help: "Limits the server to the given target FPS. 60 is recommended.",
        group: "Performance",
        kind: Kind::Int { min: 1, max: 1000 },
        default: "60",
    },
    ParamSpec {
        flag: "-autoreload",
        label: "Auto Reload Scenario",
        help: "Reloads the scenario after the given number of seconds once the session has ended, without shutting the server down.",
        group: "Performance",
        kind: Kind::Int { min: 1, max: 86400 },
        default: "10",
    },
    ParamSpec {
        flag: "-loadSessionSave",
        label: "Load Session Save",
        help: "Loads a previous session. Leave the file name empty to load the latest save.",
        group: "Performance",
        kind: Kind::OptText,
        default: "",
    },
    ParamSpec {
        flag: "-logStats",
        label: "Log Performance Stats",
        help: "Logs performance statistics at the given interval in milliseconds.",
        group: "Performance",
        kind: Kind::OptInt { min: 100, max: 3_600_000 },
        default: "1000",
    },
    ParamSpec {
        flag: "-jobsysShortWorkerCount",
        label: "Short Job Worker Threads",
        help: "Number of threads for short jobs. Capped by the CPU count.",
        group: "Performance",
        kind: Kind::Int { min: 1, max: 256 },
        default: "4",
    },
    ParamSpec {
        flag: "-jobsysLongWorkerCount",
        label: "Long Job Worker Threads",
        help: "Number of threads for long jobs. Capped by the CPU count.",
        group: "Performance",
        kind: Kind::Int { min: 1, max: 256 },
        default: "4",
    },
    // --- Network ---
    ParamSpec {
        flag: "-bindIP",
        label: "Override Bind IP",
        help: "Overrides the bind and register address from the server config.",
        group: "Network",
        kind: Kind::Text,
        default: "0.0.0.0",
    },
    ParamSpec {
        flag: "-bindPort",
        label: "Override Bind Port",
        help: "Overrides the bind and register port from the server config.",
        group: "Network",
        kind: Kind::Int { min: 1, max: 65535 },
        default: "2001",
    },
    ParamSpec {
        flag: "-a2sIpAddress",
        label: "Override A2S Address",
        help: "Steam Query Protocol bind IP.",
        group: "Network",
        kind: Kind::Text,
        default: "0.0.0.0",
    },
    ParamSpec {
        flag: "-a2sPort",
        label: "Override A2S Port",
        help: "Steam Query Protocol bind port.",
        group: "Network",
        kind: Kind::Int { min: 1, max: 65535 },
        default: "17777",
    },
    ParamSpec {
        flag: "-nds",
        label: "Network Dynamic Simulation",
        help: "Diameter in cells of replicated entities streamed to each client. Default 2, 0 turns it off. Higher values widen the view range and lower performance.",
        group: "Network",
        kind: Kind::Int { min: 0, max: 100 },
        default: "2",
    },
    ParamSpec {
        flag: "-nwkResolution",
        label: "Spatial Map Resolution",
        help: "Cell resolution of the spatial map, 100 to 1000 m. Smaller values reduce pop-in but shorten the networked view range.",
        group: "Network",
        kind: Kind::Int { min: 100, max: 1000 },
        default: "500",
    },
    ParamSpec {
        flag: "-staggeringBudget",
        label: "Staggering Budget",
        help: "Stationary spatial map cells processed per tick, 1 to 10201. Uses the NDS diameter when unset.",
        group: "Network",
        kind: Kind::Int { min: 1, max: 10201 },
        default: "1000",
    },
    ParamSpec {
        flag: "-streamingBudget",
        label: "Streaming Budget",
        help: "Global streaming budget shared equally between all connections. Cannot go below 100.",
        group: "Network",
        kind: Kind::Int { min: 100, max: 1_000_000 },
        default: "10000",
    },
    ParamSpec {
        flag: "-streamsDelta",
        label: "Streams Delta",
        help: "Stops opening new streams for a client in a tick when the gap to the client's open streams exceeds this value. 1 to 1000, default 100.",
        group: "Network",
        kind: Kind::Int { min: 1, max: 1000 },
        default: "100",
    },
    ParamSpec {
        flag: "-rpl-timeout-ms",
        label: "Replication Timeout (ms)",
        help: "Client and server timeout in milliseconds.",
        group: "Network",
        kind: Kind::Int { min: 1000, max: 600_000 },
        default: "10000",
    },
    // --- Logging ---
    ParamSpec {
        flag: "-logLevel",
        label: "Log Level",
        help: "Minimum severity that is logged. 'normal' logs everything.",
        group: "Logging",
        kind: Kind::Choice(LOG_LEVELS),
        default: "normal",
    },
    ParamSpec {
        flag: "-logsDir",
        label: "Logs Directory",
        help: "Where logs, dumps and temporary files are stored.",
        group: "Logging",
        kind: Kind::Folder,
        default: "",
    },
    ParamSpec {
        flag: "-keepNumOfLogs",
        label: "Logs to Keep",
        help: "Maximum number of log folders to keep. Default 10.",
        group: "Logging",
        kind: Kind::Int { min: 1, max: 1000 },
        default: "10",
    },
    ParamSpec {
        flag: "-logAppend",
        label: "Append to Existing Logs",
        help: "Keeps the existing log history instead of clearing it on start.",
        group: "Logging",
        kind: Kind::Flag,
        default: "",
    },
    ParamSpec {
        flag: "-VMErrorMode",
        label: "Script VM Error Mode",
        help: "How script virtual machine errors are reported.",
        group: "Logging",
        kind: Kind::Choice(VM_ERROR_MODES),
        default: "full",
    },
    ParamSpec {
        flag: "-noThrow",
        label: "No Error Dialogs",
        help: "Disables all error message dialogs.",
        group: "Logging",
        kind: Kind::Flag,
        default: "",
    },
    ParamSpec {
        flag: "-disableCrashReporter",
        label: "Disable Crash Reporter",
        help: "Stops the crash reporter from appearing or sending reports.",
        group: "Logging",
        kind: Kind::Flag,
        default: "",
    },
    // --- Paths and addons ---
    ParamSpec {
        flag: "-profile",
        label: "Profile",
        help: "Folder the server keeps its data in, as a folder name or a full path. The server works in a \"profile\" folder inside it, so saves end up in <Profile>/profile/.save.",
        group: "Paths and Addons",
        kind: Kind::Folder,
        default: "",
    },
    ParamSpec {
        flag: "-addons",
        label: "Load Addons",
        help: "Comma separated mod IDs to load on start, without brackets.",
        group: "Paths and Addons",
        kind: Kind::Text,
        default: "",
    },
    ParamSpec {
        flag: "-addonsDir",
        label: "Addon Search Folders",
        help: "Comma separated extra directories to search for mods.",
        group: "Paths and Addons",
        kind: Kind::Text,
        default: "",
    },
    ParamSpec {
        flag: "-addonDownloadDir",
        label: "Addon Download Folder",
        help: "Where the game and Workshop download addons; files go into an 'addons' subfolder.",
        group: "Paths and Addons",
        kind: Kind::Folder,
        default: "",
    },
    ParamSpec {
        flag: "-addonTempDir",
        label: "Addon Temp Folder",
        help: "Directory for temporary content.",
        group: "Paths and Addons",
        kind: Kind::Folder,
        default: "",
    },
    ParamSpec {
        flag: "-createDB",
        label: "Regenerate Database",
        help: "Forces the database file to be regenerated.",
        group: "Paths and Addons",
        kind: Kind::Flag,
        default: "",
    },
    // --- Backend and AI ---
    ParamSpec {
        flag: "-noBackend",
        label: "No Backend",
        help: "Hosts the server without using the Arma Reforger backend.",
        group: "Backend and AI",
        kind: Kind::Flag,
        default: "",
    },
    ParamSpec {
        flag: "-backendLocalStorage",
        label: "Local Storage",
        help: "Uses local JSON files in the profile folder instead of the backend.",
        group: "Backend and AI",
        kind: Kind::Flag,
        default: "",
    },
    ParamSpec {
        flag: "-backendDisableStorage",
        label: "Disable Storage",
        help: "Disables all loads and saves, online and local.",
        group: "Backend and AI",
        kind: Kind::Flag,
        default: "",
    },
    ParamSpec {
        flag: "-backendFreshSession",
        label: "Fresh Session",
        help: "Skips the initial load from the server config or script.",
        group: "Backend and AI",
        kind: Kind::Flag,
        default: "",
    },
    ParamSpec {
        flag: "-disableAI",
        label: "Disable AI",
        help: "Prevents the AI world from initializing or ticking. Disables all AI.",
        group: "Backend and AI",
        kind: Kind::Flag,
        default: "",
    },
    ParamSpec {
        flag: "-aiLimit",
        label: "AI Limit",
        help: "Caps the number of AIs that can spawn. Overrides the value in the server config.",
        group: "Backend and AI",
        kind: Kind::Int { min: 0, max: 100_000 },
        default: "100",
    },
    ParamSpec {
        flag: "-aiPartialSim",
        label: "AI Partial Simulation",
        help: "Number of batches before all AIs are processed at full detail.",
        group: "Backend and AI",
        kind: Kind::Int { min: 0, max: 1000 },
        default: "1",
    },
];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ParamState {
    pub enabled: bool,
    pub value: String,
}

/// The launcher side of "Server Management": which parameters are on, and the
/// restart behavior. Saved with the launcher settings, not in the server config.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LaunchOptions {
    pub params: BTreeMap<String, ParamState>,
    pub extra_args: String,
    pub restart_on_crash: bool,
    pub auto_restart: bool,
    /// 24 hour `HH:MM`, local time.
    pub auto_restart_time: String,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            params: BTreeMap::new(),
            extra_args: String::new(),
            restart_on_crash: false,
            auto_restart: false,
            auto_restart_time: "04:00".into(),
        }
    }
}

/// The folder parameters (`-profile`, `-addonsDir`, `-addonDownloadDir`, `-addonTempDir`) that are
/// enabled and filled in. The scenario listing needs them to find mods that are stored outside the
/// server folder.
pub fn path_args(opts: &LaunchOptions) -> Vec<String> {
    let mut args = Vec::new();
    for flag in ["-profile", "-addonsDir", "-addonDownloadDir", "-addonTempDir"] {
        if let Some(state) = opts.params.get(flag) {
            let value = state.value.trim();
            if state.enabled && !value.is_empty() {
                args.push(flag.to_string());
                args.push(value.to_string());
            }
        }
    }
    args
}

/// Builds the argument list: `-config <path>`, every enabled parameter, then the extra arguments.
pub fn build_args(config_path: &Path, opts: &LaunchOptions) -> Vec<String> {
    let mut args = vec!["-config".to_string(), config_path.to_string_lossy().into_owned()];

    for spec in PARAMS {
        let Some(state) = opts.params.get(spec.flag) else {
            continue;
        };
        if !state.enabled {
            continue;
        }
        let value = state.value.trim();
        match spec.kind {
            Kind::Flag => args.push(spec.flag.to_string()),
            Kind::OptText | Kind::OptInt { .. } => {
                args.push(spec.flag.to_string());
                if !value.is_empty() {
                    args.push(value.to_string());
                }
            }
            Kind::Text | Kind::Folder | Kind::Int { .. } | Kind::Choice(_) => {
                if !value.is_empty() {
                    args.push(spec.flag.to_string());
                    args.push(value.to_string());
                }
            }
        }
    }

    args.extend(split_args(&opts.extra_args));
    args
}

/// Splits a command line fragment on whitespace, honoring single and double quotes.
pub fn split_args(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;

    for c in input.chars() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                } else {
                    current.push(c);
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    quote = Some(c);
                    started = true;
                } else if c.is_whitespace() {
                    if started || !current.is_empty() {
                        out.push(std::mem::take(&mut current));
                        started = false;
                    }
                } else {
                    current.push(c);
                }
            }
        }
    }
    if started || !current.is_empty() {
        out.push(current);
    }
    out
}

/// Quotes an argument for display only (the process is spawned with an argument list, not a shell string).
pub fn display_arg(arg: &str) -> String {
    if arg.is_empty() || arg.contains(char::is_whitespace) {
        format!("\"{arg}\"")
    } else {
        arg.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn enable(opts: &mut LaunchOptions, flag: &str, value: &str) {
        opts.params.insert(
            flag.to_string(),
            ParamState { enabled: true, value: value.to_string() },
        );
    }

    #[test]
    fn config_path_always_comes_first() {
        let args = build_args(&PathBuf::from("/srv/server.json"), &LaunchOptions::default());
        assert_eq!(args, vec!["-config", "/srv/server.json"]);
    }

    #[test]
    fn enabled_parameters_are_added_in_table_order() {
        let mut opts = LaunchOptions::default();
        enable(&mut opts, "-noBackend", "");
        enable(&mut opts, "-maxFPS", "60");
        enable(&mut opts, "-logLevel", "warning");
        let args = build_args(&PathBuf::from("c.json"), &opts);
        assert_eq!(
            args,
            vec!["-config", "c.json", "-maxFPS", "60", "-logLevel", "warning", "-noBackend"]
        );
    }

    #[test]
    fn disabled_parameters_are_skipped() {
        let mut opts = LaunchOptions::default();
        opts.params.insert(
            "-maxFPS".into(),
            ParamState { enabled: false, value: "60".into() },
        );
        assert_eq!(build_args(&PathBuf::from("c.json"), &opts).len(), 2);
    }

    #[test]
    fn optional_values_may_be_left_out() {
        let mut opts = LaunchOptions::default();
        enable(&mut opts, "-loadSessionSave", "");
        enable(&mut opts, "-logStats", "");
        let args = build_args(&PathBuf::from("c.json"), &opts);
        assert!(args.contains(&"-loadSessionSave".to_string()));
        assert!(args.contains(&"-logStats".to_string()));
        assert_eq!(args.len(), 4);
    }

    #[test]
    fn required_values_that_are_empty_are_skipped() {
        let mut opts = LaunchOptions::default();
        enable(&mut opts, "-profile", "  ");
        assert_eq!(build_args(&PathBuf::from("c.json"), &opts).len(), 2);
    }

    #[test]
    fn extra_arguments_respect_quotes() {
        let mut opts = LaunchOptions::default();
        opts.extra_args = r#"-foo "a b" -bar 'c d' -baz"#.into();
        let args = build_args(&PathBuf::from("c.json"), &opts);
        assert_eq!(&args[2..], ["-foo", "a b", "-bar", "c d", "-baz"]);
    }

    #[test]
    fn parameter_table_has_no_duplicates_and_known_groups() {
        let mut seen = std::collections::HashSet::new();
        for spec in PARAMS {
            assert!(seen.insert(spec.flag), "duplicate {}", spec.flag);
            assert!(GROUPS.contains(&spec.group), "unknown group {}", spec.group);
            assert!(spec.flag.starts_with('-'));
        }
    }
}
