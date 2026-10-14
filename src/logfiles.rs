//! Following the server's own log files while it runs. The server writes `console.log`,
//! `error.log` and `script.log` into its logs folder; most of what it prints never reaches the
//! process output, so these files are the real log.

use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use crate::server::{LineKind, LogLine};

/// The files that are followed: tab name and file name.
pub const FILES: [(&str, &str); 3] = [
    ("Console", "console.log"),
    ("Error", "error.log"),
    ("Script", "script.log"),
];

const MAX_LINES: usize = 50_000;
/// A file that is already big when first seen is read from its last part only.
const FIRST_READ_BYTES: u64 = 2 * 1024 * 1024;
/// Most bytes read from one file in one poll, so a huge burst cannot stall the window.
const MAX_READ_BYTES: u64 = 8 * 1024 * 1024;
const POLL_EVERY: Duration = Duration::from_millis(500);

#[derive(Default)]
struct Tail {
    /// Bytes of the file already read.
    pos: u64,
    /// The end of the last read, when it did not finish with a line break.
    partial: Vec<u8>,
    lines: VecDeque<LogLine>,
}

impl Tail {
    fn reset(&mut self) {
        *self = Tail::default();
    }

    fn read_new(&mut self, path: &Path, first_time: bool) {
        let Ok(len) = fs::metadata(path).map(|m| m.len()) else {
            return;
        };
        if len < self.pos {
            // The file was replaced or emptied: start again.
            self.reset();
        }
        if len == self.pos {
            return;
        }
        let Ok(mut file) = File::open(path) else {
            return;
        };

        let mut skip_first_line = false;
        if first_time && self.pos == 0 && len > FIRST_READ_BYTES {
            self.pos = len - FIRST_READ_BYTES;
            skip_first_line = true;
        }
        if file.seek(SeekFrom::Start(self.pos)).is_err() {
            return;
        }
        let want = (len - self.pos).min(MAX_READ_BYTES);
        let mut buf = Vec::with_capacity(want as usize);
        if file.take(want).read_to_end(&mut buf).is_err() {
            return;
        }
        self.pos += buf.len() as u64;

        let mut data = std::mem::take(&mut self.partial);
        data.extend_from_slice(&buf);
        if skip_first_line {
            // The read began in the middle of a line.
            match data.iter().position(|&b| b == b'\n') {
                Some(i) => {
                    data.drain(..=i);
                }
                None => data.clear(),
            }
        }

        let mut start = 0;
        while let Some(offset) = data[start..].iter().position(|&b| b == b'\n') {
            let end = start + offset;
            let text = String::from_utf8_lossy(&data[start..end]);
            self.lines.push_back(LogLine {
                kind: kind_of(&text),
                text: text.trim_end_matches('\r').to_string(),
            });
            start = end + 1;
        }
        self.partial = data[start..].to_vec();
        while self.lines.len() > MAX_LINES {
            self.lines.pop_front();
        }
    }
}

/// Lines the server marks with `(E)` or `(F)` are shown as errors.
fn kind_of(text: &str) -> LineKind {
    if text.contains("(E)") || text.contains("(F)") {
        LineKind::Err
    } else {
        LineKind::Out
    }
}

pub struct LogFiles {
    root: Option<PathBuf>,
    /// The folder the files are being read from right now.
    dir: Option<PathBuf>,
    tails: [Tail; 3],
    last_poll: Option<Instant>,
}

impl Default for LogFiles {
    fn default() -> Self {
        Self::new()
    }
}

impl LogFiles {
    pub fn new() -> Self {
        Self {
            root: None,
            dir: None,
            tails: Default::default(),
            last_poll: None,
        }
    }

    /// The logs folder from the settings. A change of folder starts the view over.
    pub fn set_root(&mut self, root: Option<PathBuf>) {
        if self.root != root {
            self.root = root;
            self.dir = None;
            self.last_poll = None;
            for tail in &mut self.tails {
                tail.reset();
            }
        }
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// The path of one of the followed files, if a log folder has been found.
    pub fn file_path(&self, index: usize) -> Option<PathBuf> {
        self.dir.as_ref().map(|dir| dir.join(FILES[index].1))
    }

    pub fn lines(&self, index: usize) -> &VecDeque<LogLine> {
        &self.tails[index].lines
    }

    /// Empties the view of one file; only lines written from now on are shown.
    pub fn clear(&mut self, index: usize) {
        let len = self
            .file_path(index)
            .and_then(|p| fs::metadata(p).ok())
            .map_or(0, |m| m.len());
        let tail = &mut self.tails[index];
        tail.lines.clear();
        tail.partial.clear();
        tail.pos = len;
    }

    /// Looks for new lines, at most twice a second.
    pub fn poll(&mut self) {
        if self.last_poll.map_or(false, |t| t.elapsed() < POLL_EVERY) {
            return;
        }
        self.last_poll = Some(Instant::now());
        let Some(root) = self.root.clone() else {
            return;
        };

        let found = active_dir(&root);
        let first_time = found != self.dir;
        if first_time {
            // A new run started (or the first look): follow its files from the beginning.
            self.dir = found;
            for tail in &mut self.tails {
                tail.reset();
            }
        }
        let Some(dir) = self.dir.clone() else {
            return;
        };
        for (index, (_, name)) in FILES.iter().enumerate() {
            let from_start = first_time || self.tails[index].pos == 0;
            self.tails[index].read_new(&dir.join(name), from_start);
        }
    }
}

/// The folder holding the current run's logs: the logs folder itself, or the newest
/// `logs_<date>` folder inside it, whichever has the most recently written `console.log`.
/// The server uses the first when `-logsDir` is given and the second otherwise.
fn active_dir(root: &Path) -> Option<PathBuf> {
    let mut candidates = vec![root.to_path_buf()];
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let is_run_folder = path.is_dir()
                && entry
                    .file_name()
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .starts_with("logs_");
            if is_run_folder {
                candidates.push(path);
            }
        }
    }
    candidates
        .into_iter()
        .filter_map(|dir| {
            let modified: SystemTime = fs::metadata(dir.join("console.log"))
                .and_then(|m| m.modified())
                .ok()?;
            Some((modified, dir))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, dir)| dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("arsenal-logfiles-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn follows_appended_lines_and_partial_lines() {
        let dir = temp_dir("tail");
        let path = dir.join("console.log");
        fs::write(&path, "one\r\ntwo\r\nthr").unwrap();

        let mut tail = Tail::default();
        tail.read_new(&path, true);
        let texts: Vec<_> = tail.lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["one", "two"]);

        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"ee (E) bad\n").unwrap();
        tail.read_new(&path, false);
        assert_eq!(tail.lines.len(), 3);
        assert_eq!(tail.lines[2].text, "three (E) bad");
        assert_eq!(tail.lines[2].kind, LineKind::Err);

        // A shorter file means a new run wrote it: read from the start again.
        fs::write(&path, "new\n").unwrap();
        tail.read_new(&path, false);
        assert_eq!(tail.lines.len(), 1);
        assert_eq!(tail.lines[0].text, "new");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn picks_the_folder_with_the_newest_console_log() {
        let root = temp_dir("active");
        let old = root.join("logs_2026-10-01_10-00-00");
        let new = root.join("logs_2026-10-02_10-00-00");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&new).unwrap();
        fs::write(old.join("console.log"), "a").unwrap();
        std::thread::sleep(Duration::from_millis(30));
        fs::write(new.join("console.log"), "b").unwrap();
        assert_eq!(active_dir(&root), Some(new.clone()));

        // With -logsDir the files sit in the folder itself.
        std::thread::sleep(Duration::from_millis(30));
        fs::write(root.join("console.log"), "c").unwrap();
        assert_eq!(active_dir(&root), Some(root.clone()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn no_logs_means_no_folder() {
        let root = temp_dir("none");
        assert_eq!(active_dir(&root), None);
        let _ = fs::remove_dir_all(&root);
    }
}
