//! A small command line front end. The GUI is the main interface; this exists for scripting.
//!
//! ```text
//! arsenal validate <config.json>   check a config file, exit code 1 if it has errors
//! arsenal args <config.json>       print the server arguments Arsenal would use
//! ```

use std::path::Path;

use crate::config::io;
use crate::config::validate::{has_errors, validate};
use crate::launch::{build_args, display_arg};
use crate::settings::AppSettings;

/// Returns `Some(exit_code)` when the arguments were a CLI command, `None` to start the GUI.
pub fn run(args: &[String]) -> Option<i32> {
    match args.first().map(String::as_str) {
        Some("validate") => Some(validate_command(&args[1..])),
        Some("args") => Some(args_command(&args[1..])),
        Some("--help") | Some("-h") | Some("help") => {
            print_help();
            Some(0)
        }
        _ => None,
    }
}

fn print_help() {
    println!("Arsenal - Arma Reforger Server Launcher");
    println!();
    println!("Run without arguments to open the GUI.");
    println!();
    println!("  arsenal validate <config.json>   check a server config; exit code 1 if it has errors");
    println!("  arsenal args <config.json>       print the server arguments from the saved launcher settings");
}

fn validate_command(args: &[String]) -> i32 {
    let Some(path) = args.first() else {
        eprintln!("usage: arsenal validate <config.json>");
        return 2;
    };
    let cfg = match io::load(Path::new(path)) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("{err:#}");
            return 1;
        }
    };
    let issues = validate(&cfg);
    for issue in &issues {
        println!("[{}] {}: {}", issue.severity.label(), issue.field, issue.message);
    }
    if issues.is_empty() {
        println!("No problems found.");
    }
    if has_errors(&issues) {
        1
    } else {
        0
    }
}

fn args_command(args: &[String]) -> i32 {
    let Some(path) = args.first() else {
        eprintln!("usage: arsenal args <config.json>");
        return 2;
    };
    let settings = AppSettings::load();
    let built = build_args(Path::new(path), &settings.launch);
    let shown: Vec<String> = built.iter().map(|a| display_arg(a)).collect();
    println!("{}", shown.join(" "));
    0
}
