//! Read the live state of local Claude Code sessions.
//!
//! Sources (all written by Claude Code itself, read-only here):
//!   ~/.claude/sessions/<pid>.json      name, cwd, status (busy/idle/waiting), timestamps
//!   ~/.claude/projects/*/<id>.jsonl    transcript: tool calls, last prompt, last answer
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;

use serde_json::Value;

use crate::util::now;

const TAIL_BYTES: u64 = 256 * 1024;
const SUBAGENT_MAX_SILENCE: f64 = 600.0; // an unfinished sub-agent silent for 10 min is ignored

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum State {
    Working,
    Idle,
    Waiting,
    Blocked,
    /// the resting creature shown when no session is running
    Rest,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Working => "working",
            State::Idle => "idle",
            State::Waiting => "waiting",
            State::Blocked => "blocked",
            State::Rest => "rest",
        }
    }
}

/// What a session looks like: its state, or "background" (turn finished, a background task still runs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Look {
    Working,
    Idle,
    Waiting,
    Blocked,
    Rest,
    Background,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Subagent {
    pub id: String,
    pub kind: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub id: String,
    pub pid: i64,
    pub name: String,
    pub cwd: String,
    pub status: String,
    pub state: State,
    pub background: bool,
    pub bg_task: String,
    pub subagents: Vec<Subagent>,
    pub waiting_for: String,
    /// seconds since 1970 when the current state began
    pub since: f64,
    pub started: f64,
    pub tool: Option<String>,
    pub tool_detail: String,
    pub last_text: String,
    pub prompt: String,
    /// (id, tool name, detail) of the latest tool calls
    pub tool_events: Vec<(String, String, String)>,
}

impl Default for Session {
    fn default() -> Self {
        Session {
            id: String::new(),
            pid: 0,
            name: String::new(),
            cwd: String::new(),
            status: "idle".into(),
            state: State::Idle,
            background: false,
            bg_task: String::new(),
            subagents: Vec::new(),
            waiting_for: String::new(),
            since: now(),
            started: 0.0,
            tool: None,
            tool_detail: String::new(),
            last_text: String::new(),
            prompt: String::new(),
            tool_events: Vec::new(),
        }
    }
}

impl Session {
    /// State for colour and label. A session running a background task moves like a working one
    /// but is shown in its own colour as "background".
    pub fn look(&self) -> Look {
        if self.background {
            return Look::Background;
        }
        match self.state {
            State::Working => Look::Working,
            State::Idle => Look::Idle,
            State::Waiting => Look::Waiting,
            State::Blocked => Look::Blocked,
            State::Rest => Look::Rest,
        }
    }
}

fn state_for(status: &str) -> State {
    match status {
        "busy" | "working" | "running" | "shell" => State::Working,
        "waiting" | "requires_action" => State::Waiting,
        "blocked" => State::Blocked,
        _ => State::Idle,
    }
}

pub fn home_dir() -> PathBuf {
    #[cfg(windows)]
    let h = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let h = std::env::var_os("HOME");
    h.map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

fn claude_dir() -> PathBuf {
    match std::env::var("CLAUDE_CONFIG_DIR") {
        Ok(v) if !v.is_empty() => {
            if let Some(rest) = v.strip_prefix("~") {
                PathBuf::from(format!("{}{}", home_dir().display(), rest))
            } else {
                PathBuf::from(v)
            }
        }
        _ => home_dir().join(".claude"),
    }
}

#[cfg(target_os = "linux")]
fn pid_alive(pid: i64) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(all(unix, not(target_os = "linux")))]
fn pid_alive(pid: i64) -> bool {
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    if pid <= 0 || pid > i32::MAX as i64 {
        return false;
    }
    // SAFETY: signal 0 only checks whether the process exists
    if unsafe { kill(pid as i32, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(1) // EPERM: exists, not ours
}

#[cfg(windows)]
fn pid_alive(pid: i64) -> bool {
    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut core::ffi::c_void;
        fn GetExitCodeProcess(h: *mut core::ffi::c_void, code: *mut u32) -> i32;
        fn CloseHandle(h: *mut core::ffi::c_void) -> i32;
    }
    if pid <= 0 || pid > u32::MAX as i64 {
        return false;
    }
    // SAFETY: plain Win32 calls with a handle we own
    unsafe {
        let h = OpenProcess(0x1000, 0, pid as u32); // PROCESS_QUERY_LIMITED_INFORMATION
        if h.is_null() {
            return false;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(h, &mut code);
        CloseHandle(h);
        ok != 0 && code == 259 // STILL_ACTIVE
    }
}

fn short_path(path: &str) -> String {
    let home = home_dir().to_string_lossy().to_string();
    if !path.is_empty() && !home.is_empty() {
        if path == home {
            return "~".into();
        }
        if let Some(rest) = path.strip_prefix(&home) {
            if rest.starts_with(std::path::MAIN_SEPARATOR) || rest.starts_with('/') {
                return format!("~{rest}");
            }
        }
    }
    path.to_string()
}

fn base_name(path: &str) -> String {
    let p = path.trim_end_matches(['/', '\\']);
    p.rsplit(['/', '\\']).next().unwrap_or("").to_string()
}

fn find_transcript(dir: &Path, session_id: &str, cwd: &str) -> Option<PathBuf> {
    if session_id.is_empty() {
        return None;
    }
    let file = format!("{session_id}.jsonl");
    if !cwd.is_empty() {
        let folder: String = cwd.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
        let p = dir.join("projects").join(folder).join(&file);
        if p.exists() {
            return Some(p);
        }
    }
    let mut hits: Vec<PathBuf> = fs::read_dir(dir.join("projects"))
        .ok()?
        .flatten()
        .map(|e| e.path().join(&file))
        .filter(|p| p.exists())
        .collect();
    hits.sort();
    hits.into_iter().next()
}

fn one_line(text: &str, limit: usize) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(limit).collect()
}

fn tool_detail(inp: Option<&Value>) -> String {
    let Some(obj) = inp.and_then(Value::as_object) else { return String::new() };
    for key in ["description", "command", "file_path", "path", "pattern", "url", "query", "skill", "prompt"] {
        if let Some(v) = obj.get(key).and_then(Value::as_str) {
            if !v.trim().is_empty() {
                if key == "file_path" || key == "path" {
                    let b = base_name(v);
                    return one_line(if b.is_empty() { v } else { &b }, 80);
                }
                return one_line(v, 80);
            }
        }
    }
    String::new()
}

#[derive(Clone, Debug, Default)]
struct Transcript {
    tool: Option<String>,
    tool_detail: String,
    last_text: String,
    prompt: String,
    tool_events: Vec<(String, String, String)>,
    bg_task: String,
}

fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

fn parse_transcript_text(data: &str, cut_first: bool) -> Transcript {
    let mut info = Transcript::default();
    let mut events: Vec<(String, String, String)> = Vec::new();
    let mut lines = data.lines();
    if cut_first {
        lines.next(); // first line is probably cut in half
    }
    for line in lines {
        let Ok(obj) = serde_json::from_str::<Value>(line) else { continue };
        let typ = obj.get("type").and_then(Value::as_str).unwrap_or("");
        let content = obj.get("message").and_then(|m| m.get("content"));
        if typ == "assistant" {
            let Some(items) = content.and_then(Value::as_array) else { continue };
            for item in items {
                if !item.is_object() {
                    continue;
                }
                match item.get("type").and_then(Value::as_str) {
                    Some("tool_use") => {
                        let name = item.get("name").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or("tool");
                        let detail = tool_detail(item.get("input"));
                        let id = item
                            .get("id")
                            .and_then(Value::as_str)
                            .filter(|s| !s.is_empty())
                            .map(str::to_string)
                            .unwrap_or_else(|| events.len().to_string());
                        events.push((id, name.to_string(), detail.clone()));
                        info.tool = Some(name.to_string());
                        if truthy(item.get("input").and_then(|i| i.get("run_in_background"))) {
                            // the latest task started in the background
                            info.bg_task = if detail.is_empty() { name.to_string() } else { detail.clone() };
                        }
                        info.tool_detail = detail;
                    }
                    Some("text") => {
                        let t = item.get("text").and_then(Value::as_str).unwrap_or("");
                        if !t.trim().is_empty() {
                            info.last_text = one_line(t, 120);
                        }
                    }
                    _ => {}
                }
            }
        } else if typ == "user" && !truthy(obj.get("isMeta")) && !truthy(obj.get("isSidechain")) {
            let mut text: Option<String> = None;
            match content {
                Some(Value::String(s)) => text = Some(s.clone()),
                Some(Value::Array(items)) => {
                    let parts: Vec<&str> = items
                        .iter()
                        .filter(|c| c.get("type").and_then(Value::as_str) == Some("text"))
                        .map(|c| c.get("text").and_then(Value::as_str).unwrap_or(""))
                        .collect();
                    let has_result = items.iter().any(|c| c.get("type").and_then(Value::as_str) == Some("tool_result"));
                    if !parts.is_empty() && !has_result {
                        text = Some(parts.join(" "));
                    }
                }
                _ => {}
            }
            if let Some(t) = text {
                if !t.trim().is_empty() && !t.trim_start().starts_with('<') {
                    info.prompt = one_line(&t, 120);
                }
            }
        }
    }
    let n = events.len();
    info.tool_events = events.split_off(n.saturating_sub(30));
    info
}

fn mtime_secs(path: &Path) -> Option<f64> {
    let m = fs::metadata(path).ok()?.modified().ok()?;
    Some(m.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs_f64())
}

fn subagents(transcript: &Path) -> Vec<Subagent> {
    let folder = transcript.with_extension("").join("subagents");
    let Ok(rd) = fs::read_dir(&folder) else { return Vec::new() };
    let t = now();
    let mut out = Vec::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(id) = name.strip_prefix("agent-").and_then(|n| n.strip_suffix(".meta.json")) else { continue };
        let meta = e.path();
        let log = folder.join(format!("agent-{id}.jsonl"));
        let Some(mt) = mtime_secs(if log.exists() { &log } else { &meta }) else { continue };
        if t - mt > SUBAGENT_MAX_SILENCE {
            continue;
        }
        let Some(m) = fs::read_to_string(&meta).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()) else {
            continue;
        };
        if truthy(m.get("finished")) {
            continue;
        }
        out.push(Subagent {
            id: id.to_string(),
            kind: m.get("agentType").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or("agent").to_string(),
            description: one_line(m.get("description").and_then(Value::as_str).unwrap_or(""), 60),
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Reads the sessions and remembers parsed transcripts between calls.
#[derive(Default)]
pub struct Reader {
    cache: HashMap<PathBuf, ((u64, Option<SystemTime>), Transcript)>,
}

impl Reader {
    pub fn new() -> Self {
        Self::default()
    }

    fn transcript(&mut self, path: &Path) -> Transcript {
        let Ok(md) = fs::metadata(path) else { return Transcript::default() };
        let key = (md.len(), md.modified().ok());
        if let Some((k, info)) = self.cache.get(path) {
            if *k == key {
                return info.clone();
            }
        }
        let offset = md.len().saturating_sub(TAIL_BYTES);
        let read = || -> std::io::Result<Vec<u8>> {
            let mut f = fs::File::open(path)?;
            f.seek(SeekFrom::Start(offset))?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            Ok(buf)
        };
        let Ok(bytes) = read() else { return Transcript::default() };
        let info = parse_transcript_text(&String::from_utf8_lossy(&bytes), offset > 0);
        self.cache.insert(path.to_path_buf(), (key, info.clone()));
        info
    }

    /// Every live Claude Code session.
    pub fn read(&mut self) -> Vec<Rc<Session>> {
        let dir = claude_dir();
        let mut files: Vec<PathBuf> = fs::read_dir(dir.join("sessions"))
            .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect())
            .unwrap_or_default();
        files.sort();
        let mut result = Vec::new();
        let mut seen: HashSet<PathBuf> = HashSet::new();
        for path in files {
            let Some(raw) = fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()) else {
                continue;
            };
            let pid = raw.get("pid").and_then(|p| p.as_i64().or_else(|| p.as_str().and_then(|s| s.parse().ok())));
            let Some(pid) = pid else { continue };
            if !pid_alive(pid) {
                continue; // stale file of a session that has ended
            }
            let s = |k: &str| raw.get(k).and_then(Value::as_str).filter(|v| !v.is_empty());
            let ms = |k: &str| raw.get(k).and_then(Value::as_f64).filter(|v| *v != 0.0);
            let cwd = s("cwd").unwrap_or("").to_string();
            let sid = s("sessionId").map(str::to_string).unwrap_or_else(|| pid.to_string());
            let status = s("status").unwrap_or("idle").to_string();
            let since_ms = ms("statusUpdatedAt").or_else(|| ms("updatedAt")).or_else(|| ms("startedAt"));
            let tpath = find_transcript(&dir, &sid, &cwd);
            let t = match &tpath {
                Some(p) => {
                    seen.insert(p.clone());
                    self.transcript(p)
                }
                None => Transcript::default(),
            };
            let waiting_for = match raw.get("waitingFor") {
                Some(Value::Object(o)) => o
                    .get("type")
                    .and_then(Value::as_str)
                    .filter(|v| !v.is_empty())
                    .or_else(|| o.get("kind").and_then(Value::as_str).filter(|v| !v.is_empty()))
                    .unwrap_or("input")
                    .to_string(),
                Some(Value::String(v)) => v.clone(),
                Some(v) if truthy(Some(v)) => v.to_string(),
                _ => String::new(),
            };
            let name = s("name").map(str::to_string).unwrap_or_else(|| {
                let b = base_name(&cwd);
                if b.is_empty() { format!("session {pid}") } else { b }
            });
            result.push(Rc::new(Session {
                id: sid,
                pid,
                name,
                cwd: short_path(&cwd),
                state: state_for(&status),
                background: status == "shell",
                status,
                bg_task: t.bg_task,
                subagents: tpath.as_deref().map(subagents).unwrap_or_default(),
                waiting_for,
                since: since_ms.map(|v| v / 1000.0).unwrap_or_else(now),
                started: ms("startedAt").unwrap_or(0.0) / 1000.0,
                tool: t.tool,
                tool_detail: t.tool_detail,
                last_text: t.last_text,
                prompt: t.prompt,
                tool_events: t.tool_events,
            }));
        }
        self.cache.retain(|p, _| seen.contains(p));
        result
    }
}

// --------------------------------------------------------------------------- demo data

const DEMO: [(&str, &str, f64); 3] = [
    ("Web shop", "~/dev/webshop", 0.0),
    ("Screensaver", "~/dev/claude-screensaver", 13.0),
    ("Infra audit", "~/dev/infra", 27.0),
];
const DEMO_TOOLS: [(&str, &str); 9] = [
    ("Read", "inventory.yaml"),
    ("Grep", "def parse_"),
    ("Bash", "Run unit tests"),
    ("Edit", "scene.py"),
    ("Glob", "**/*.md"),
    ("Bash", "git status"),
    ("Write", "README.md"),
    ("WebFetch", "docs.python.org"),
    ("Agent", "Explore network configs"),
];
const DEMO_TEXT: [&str; 3] = [
    "All 42 tests pass. Want me to commit?",
    "Done - the parser now handles empty lines.",
    "I found three hosts with outdated firmware.",
];

/// Fake sessions cycling through all states, for previews. `t` = seconds since the demo began.
pub fn demo_sessions(t: f64) -> Vec<Rc<Session>> {
    let now = now();
    let mut out = Vec::new();
    let period = 42.0;
    for (i, (name, cwd, off)) in DEMO.iter().enumerate() {
        let p = (t + off).rem_euclid(period);
        let (state, start) = if p < 20.0 {
            (State::Working, 0.0)
        } else if p < 29.0 {
            (State::Waiting, 20.0)
        } else {
            (State::Idle, 29.0)
        };
        // tool calls only happen while working; afterwards the list is frozen
        let n = ((t + off - (p - 20.0).max(0.0)) / 2.3).floor() as i64;
        let events: Vec<(String, String, String)> = ((n - 6).max(0)..=n.max(0))
            .map(|k| {
                let (tool, detail) = DEMO_TOOLS[((k * 7 + i as i64) as usize) % DEMO_TOOLS.len()];
                (format!("{i}-{k}"), tool.to_string(), detail.to_string())
            })
            .collect();
        let last = events.last().cloned().unwrap_or_default();
        let subs = ["Lifecycle and catalogue mechanics", "Project and logistics code", "Test coverage of the parser"];
        let subagents = if i == 0 && state == State::Working {
            subs.iter()
                .take(2 + ((t / 15.0) as usize % 2))
                .enumerate()
                .map(|(k, d)| Subagent { id: format!("demo{k}"), kind: "Explore".into(), description: d.to_string() })
                .collect()
        } else {
            Vec::new()
        };
        out.push(Rc::new(Session {
            id: format!("demo-{i}"),
            pid: 1000 + i as i64,
            name: name.to_string(),
            cwd: cwd.to_string(),
            status: if state == State::Working { "busy".into() } else { state.as_str().into() },
            state,
            waiting_for: if state == State::Waiting { "permission to run Bash".into() } else { String::new() },
            since: now - (p - start),
            started: now - 3600.0,
            tool: Some(last.1),
            tool_detail: last.2,
            last_text: DEMO_TEXT[i % DEMO_TEXT.len()].into(),
            prompt: "Refactor the parser and add tests".into(),
            tool_events: events,
            background: false,
            bg_task: String::new(),
            subagents,
        }));
    }
    out.push(Rc::new(Session {
        // a test bench: turn finished, suite still running as a background task
        id: "demo-bg".into(),
        pid: 1099,
        name: "Test bench".into(),
        cwd: "~/dev/webshop".into(),
        status: "shell".into(),
        state: State::Working,
        background: true,
        bg_task: "Wait for all test rows to finish".into(),
        waiting_for: String::new(),
        since: now - t.rem_euclid(900.0) - 120.0,
        started: now - 7200.0,
        tool: Some("Bash".into()),
        tool_detail: "Wait for all test rows to finish".into(),
        last_text: "The test rows are running in the background.".into(),
        prompt: "Run the test bench".into(),
        tool_events: vec![("bg-0".into(), "Bash".into(), "Wait for all test rows to finish".into())],
        subagents: Vec::new(),
    }));
    out
}

/// The sessions as JSON text (what `sessions.py` prints) - for comparing both versions.
pub fn to_json(list: &[Rc<Session>]) -> String {
    let arr: Vec<Value> = list
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id, "pid": s.pid, "name": s.name, "cwd": s.cwd, "status": s.status,
                "state": s.state.as_str(), "background": s.background, "bg_task": s.bg_task,
                "subagents": s.subagents.iter().map(|a| serde_json::json!({
                    "id": a.id, "type": a.kind, "description": a.description})).collect::<Vec<_>>(),
                "waiting_for": s.waiting_for, "since": s.since, "started": s.started,
                "tool": s.tool, "tool_detail": s.tool_detail, "last_text": s.last_text,
                "prompt": s.prompt, "tool_events": s.tool_events.len(),
            })
        })
        .collect();
    serde_json::to_string_pretty(&arr).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_tail() {
        let data = concat!(
            r#"{"type":"user","message":{"content":"Fix   the\nbug"}}"#,
            "\n",
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"On it."},{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"cargo test","run_in_background":true}}]}}"#,
            "\n",
            r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"ok"},{"type":"text","text":"ignored"}]}}"#,
            "\n",
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t2","name":"Read","input":{"file_path":"/a/b/c.rs"}}]}}"#,
            "\nnot json\n",
            r#"{"type":"user","isMeta":true,"message":{"content":"meta"}}"#,
        );
        let t = parse_transcript_text(data, false);
        assert_eq!(t.prompt, "Fix the bug");
        assert_eq!(t.last_text, "On it.");
        assert_eq!(t.tool.as_deref(), Some("Read"));
        assert_eq!(t.tool_detail, "c.rs");
        assert_eq!(t.bg_task, "cargo test");
        assert_eq!(t.tool_events.len(), 2);
        assert_eq!(t.tool_events[0], ("t1".to_string(), "Bash".to_string(), "cargo test".to_string()));
    }

    #[test]
    fn demo_has_all_states() {
        let mut seen = HashSet::new();
        for i in 0..42 {
            for s in demo_sessions(i as f64) {
                seen.insert(s.look());
            }
        }
        assert!(seen.contains(&Look::Working) && seen.contains(&Look::Waiting));
        assert!(seen.contains(&Look::Idle) && seen.contains(&Look::Background));
    }
}
