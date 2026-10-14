//! Running the dedicated server as a child process, and capturing its output.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

use crate::launch::display_arg;

const MAX_LOG_LINES: usize = 50_000;

/// File name of the server binary on this platform.
pub const SERVER_EXE: &str = if cfg!(windows) {
    "ArmaReforgerServer.exe"
} else {
    "ArmaReforgerServer"
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineKind {
    Out,
    Err,
    /// A message from Arsenal itself rather than from the server.
    System,
}

#[derive(Clone, Debug)]
pub struct LogLine {
    pub kind: LineKind,
    pub text: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ServerEvent {
    None,
    Exited {
        success: bool,
        /// True when the exit followed a Stop request from the launcher.
        requested: bool,
    },
}

pub struct ServerProcess {
    child: Option<Child>,
    tx: Sender<LogLine>,
    rx: Receiver<LogLine>,
    pub log: VecDeque<LogLine>,
    pub started_at: Option<Instant>,
    stop_requested: bool,
    kill_deadline: Option<Instant>,
}

impl Default for ServerProcess {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerProcess {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            child: None,
            tx,
            rx,
            log: VecDeque::new(),
            started_at: None,
            stop_requested: false,
            kill_deadline: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    pub fn start(&mut self, exe: &Path, args: &[String]) -> Result<()> {
        if self.child.is_some() {
            bail!("The server is already running");
        }
        let exe = std::path::absolute(exe)
            .with_context(|| format!("Invalid server path {}", exe.display()))?;

        let mut cmd = Command::new(&exe);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(dir) = exe.parent() {
            cmd.current_dir(dir);
        }
        hide_console_window(&mut cmd);

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Could not start {}", exe.display()))?;
        if let Some(out) = child.stdout.take() {
            spawn_reader(out, self.tx.clone(), LineKind::Out);
        }
        if let Some(err) = child.stderr.take() {
            spawn_reader(err, self.tx.clone(), LineKind::Err);
        }

        let shown: Vec<String> = args.iter().map(|a| display_arg(a)).collect();
        self.push_system(format!("Started {} {}", exe.display(), shown.join(" ")));
        self.child = Some(child);
        self.started_at = Some(Instant::now());
        self.stop_requested = false;
        self.kill_deadline = None;
        Ok(())
    }

    /// Asks the server to stop. On Linux this sends SIGTERM and falls back to a hard
    /// kill after 15 seconds; on Windows the process is terminated immediately.
    pub fn stop(&mut self) {
        if self.child.is_none() {
            return;
        }
        self.stop_requested = true;
        self.push_system("Stopping server...");

        #[cfg(unix)]
        {
            if let Some(pid) = self.pid() {
                let _ = Command::new("kill")
                    .arg("-TERM")
                    .arg(pid.to_string())
                    .status();
            }
            self.kill_deadline = Some(Instant::now() + Duration::from_secs(15));
        }
        #[cfg(not(unix))]
        {
            if let Some(child) = self.child.as_mut() {
                let _ = child.kill();
            }
        }
    }

    /// Call once per frame. Collects output and notices when the process has exited.
    pub fn poll(&mut self) -> ServerEvent {
        while let Ok(line) = self.rx.try_recv() {
            self.push_line(line);
        }

        if let Some(deadline) = self.kill_deadline {
            if Instant::now() > deadline {
                if let Some(child) = self.child.as_mut() {
                    let _ = child.kill();
                }
                self.kill_deadline = None;
            }
        }

        let status = match self.child.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(status)) => status,
                Ok(None) => return ServerEvent::None,
                Err(_) => return ServerEvent::None,
            },
            None => return ServerEvent::None,
        };

        // Pick up any output that arrived between the last drain and the exit.
        thread::sleep(Duration::from_millis(50));
        while let Ok(line) = self.rx.try_recv() {
            self.push_line(line);
        }

        self.child = None;
        self.started_at = None;
        self.kill_deadline = None;
        let requested = std::mem::take(&mut self.stop_requested);
        self.push_system(format!("Server exited ({status})"));
        ServerEvent::Exited {
            success: status.success(),
            requested,
        }
    }

    pub fn clear_log(&mut self) {
        self.log.clear();
    }

    pub fn push_system(&mut self, text: impl Into<String>) {
        self.push_line(LogLine {
            kind: LineKind::System,
            text: format!("[Arsenal] {}", text.into()),
        });
    }

    fn push_line(&mut self, line: LogLine) {
        self.log.push_back(line);
        while self.log.len() > MAX_LOG_LINES {
            self.log.pop_front();
        }
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(windows)]
fn hide_console_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console_window(_cmd: &mut Command) {}

/// Reads a pipe line by line (lossily, so odd bytes never end the stream) and forwards each line.
fn spawn_reader<R: Read + Send + 'static>(pipe: R, tx: Sender<LogLine>, kind: LineKind) {
    thread::spawn(move || {
        let mut reader = BufReader::new(pipe);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let text = String::from_utf8_lossy(&buf)
                        .trim_end_matches(['\r', '\n'])
                        .to_string();
                    if tx.send(LogLine { kind, text }).is_err() {
                        break;
                    }
                }
            }
        }
    });
}

/// Finds a scenario reference such as `{ECC61978EDCC2B5A}Missions/23_Campaign.conf` in a log line.
pub fn extract_scenario(line: &str) -> Option<String> {
    extract_all_scenarios(line).into_iter().next()
}

/// Finds every scenario reference in a block of text (a log line or a web page). JSON-escaped
/// slashes (`\/`) are accepted.
pub fn extract_all_scenarios(text: &str) -> Vec<String> {
    let text = text.replace("\\/", "/");
    let mut found: Vec<String> = Vec::new();
    for (start, _) in text.match_indices('{') {
        let rest = &text[start..];
        let Some(end) = rest.find('}') else {
            continue;
        };
        let guid = &rest[1..end];
        if guid.len() != 16 || !guid.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let tail = &rest[end + 1..];
        let stop = tail
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '\\' | ','))
            .unwrap_or(tail.len());
        let path = &tail[..stop];
        if path.to_ascii_lowercase().ends_with(".conf") {
            let id = format!("{{{guid}}}{path}");
            if !found.contains(&id) {
                found.push(id);
            }
        }
    }
    found
}

/// Downloads the Workshop "Scenarios" page of one mod and returns the scenarios it lists.
pub fn workshop_scenarios(mod_id: &str, name: &str) -> Result<Vec<String>, String> {
    let slug: String = name
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    let url = if slug.is_empty() {
        format!("https://reforger.armaplatform.com/workshop/{mod_id}/scenarios")
    } else {
        format!("https://reforger.armaplatform.com/workshop/{mod_id}-{slug}/scenarios")
    };
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(20))
        .user_agent("Arsenal")
        .build();
    let body = agent
        .get(&url)
        .call()
        .map_err(|e| format!("{url}: {e}"))?
        .into_string()
        .map_err(|e| format!("{url}: {e}"))?;
    Ok(extract_all_scenarios(&body))
}

/// What a `-listScenarios` run produced.
pub struct ScenarioReport {
    pub scenarios: Vec<String>,
    /// The command line that was run, for the log.
    pub command: String,
    /// The last lines of server output, to help work out why a list is short or empty.
    pub tail: Vec<String>,
    pub error: Option<String>,
    /// One line per mod describing what the Workshop lookup returned.
    pub workshop_notes: Vec<String>,
}

/// Runs `ArmaReforgerServer -listScenarios` in the background and sends back every
/// scenario it prints, together with the command and the tail of its output.
pub fn list_scenarios(
    exe: PathBuf,
    extra_args: Vec<String>,
    mods: Vec<(String, String)>,
) -> Receiver<ScenarioReport> {
    let (result_tx, result_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut report = run_list_scenarios(&exe, &extra_args);
        // The server only prints base-game scenarios, so ask the Workshop about each mod too.
        for (id, name) in &mods {
            match workshop_scenarios(id, name) {
                Ok(list) => {
                    report
                        .workshop_notes
                        .push(format!("{name} ({id}): {} scenarios", list.len()));
                    for sc in list {
                        if !report.scenarios.contains(&sc) {
                            report.scenarios.push(sc);
                        }
                    }
                }
                Err(err) => report.workshop_notes.push(format!("{name} ({id}): lookup failed: {err}")),
            }
        }
        let _ = result_tx.send(report);
    });
    result_rx
}

fn run_list_scenarios(exe: &Path, extra_args: &[String]) -> ScenarioReport {
    let mut report = ScenarioReport {
        scenarios: Vec::new(),
        command: String::new(),
        tail: Vec::new(),
        error: None,
        workshop_notes: Vec::new(),
    };
    let exe = match std::path::absolute(exe) {
        Ok(p) => p,
        Err(err) => {
            report.error = Some(err.to_string());
            return report;
        }
    };
    let mut args = vec!["-listScenarios".to_string()];
    args.extend(extra_args.iter().cloned());
    report.command = format!("{} {}", exe.display(), args.join(" "));

    let mut cmd = Command::new(&exe);
    cmd.args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = exe.parent() {
        cmd.current_dir(dir);
    }
    hide_console_window(&mut cmd);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(err) => {
            report.error = Some(err.to_string());
            return report;
        }
    };
    let (tx, rx) = mpsc::channel::<LogLine>();
    if let Some(out) = child.stdout.take() {
        spawn_reader(out, tx.clone(), LineKind::Out);
    }
    if let Some(err) = child.stderr.take() {
        spawn_reader(err, tx.clone(), LineKind::Err);
    }
    drop(tx);

    // Mods may have to be loaded (and even downloaded) before their scenarios are printed, so
    // wait for the process to finish. Only give up early after a long silence once a list exists.
    let deadline = Instant::now() + Duration::from_secs(300);
    let mut last_new: Option<Instant> = None;
    let mut last_any = Instant::now();
    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(line) => {
                last_any = Instant::now();
                if let Some(id) = extract_scenario(&line.text) {
                    if !report.scenarios.contains(&id) {
                        report.scenarios.push(id);
                        last_new = Some(Instant::now());
                    }
                }
                report.tail.push(line.text);
                if report.tail.len() > 60 {
                    report.tail.remove(0);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if Instant::now() > deadline {
            break;
        }
        if let Some(t) = last_new {
            if t.elapsed() > Duration::from_secs(30) && last_any.elapsed() > Duration::from_secs(30) {
                break;
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_scenarios_in_log_lines() {
        let line = "12:00:01  DEFAULT : Scenario: {ECC61978EDCC2B5A}Missions/23_Campaign.conf";
        assert_eq!(
            extract_scenario(line).as_deref(),
            Some("{ECC61978EDCC2B5A}Missions/23_Campaign.conf")
        );
    }

    #[test]
    fn ignores_lines_without_a_scenario() {
        assert_eq!(extract_scenario("RESOURCES : loaded {not a guid} things"), None);
        assert_eq!(extract_scenario("{ECC61978EDCC2B5A}Prefabs/Thing.et"), None);
        assert_eq!(extract_scenario("plain text"), None);
    }

    #[test]
    fn skips_a_leading_brace_that_is_not_a_guid() {
        let line = "{x} then {002AF7323E0129AF}Missions/Tutorial.conf done";
        assert_eq!(
            extract_scenario(line).as_deref(),
            Some("{002AF7323E0129AF}Missions/Tutorial.conf")
        );
    }

    #[cfg(unix)]
    #[test]
    fn captures_output_and_exit_status() {
        let mut p = ServerProcess::new();
        p.start(
            Path::new("/bin/sh"),
            &["-c".into(), "echo hello; echo oops 1>&2; exit 3".into()],
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut event = ServerEvent::None;
        while Instant::now() < deadline {
            event = p.poll();
            if event != ServerEvent::None {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(event, ServerEvent::Exited { success: false, requested: false });
        assert!(p.log.iter().any(|l| l.kind == LineKind::Out && l.text == "hello"));
        assert!(p.log.iter().any(|l| l.kind == LineKind::Err && l.text == "oops"));
        assert!(!p.is_running());
    }

    #[cfg(unix)]
    #[test]
    fn stop_terminates_a_running_process() {
        let mut p = ServerProcess::new();
        p.start(Path::new("/bin/sh"), &["-c".into(), "sleep 30".into()]).unwrap();
        assert!(p.is_running());
        p.stop();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut event = ServerEvent::None;
        while Instant::now() < deadline {
            event = p.poll();
            if event != ServerEvent::None {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert!(matches!(event, ServerEvent::Exited { requested: true, .. }));
    }
}

#[cfg(test)]
mod workshop_tests {
    use super::*;

    #[test]
    fn finds_every_scenario_in_a_page() {
        let page = r#"<a>{07297ECFD68E369E}Missions\/FreedomFighters\/A.conf</a> "{D90CF0E8ECDD7D7D}Missions/B.conf",{x}"#;
        assert_eq!(
            extract_all_scenarios(page),
            vec![
                "{07297ECFD68E369E}Missions/FreedomFighters/A.conf".to_string(),
                "{D90CF0E8ECDD7D7D}Missions/B.conf".to_string()
            ]
        );
    }
}
