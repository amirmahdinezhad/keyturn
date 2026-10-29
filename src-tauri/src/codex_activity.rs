//! Read-only activity checks for rollout files held open by the live desktop backend.
//! Stored chat content is never returned to the webview; only lifecycle state is used.
use std::{collections::BTreeSet, fs::File, io::{Read, Seek, SeekFrom}, path::{Path, PathBuf}};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Activity { Idle, Running(u32), Unknown }

/// Associate sessions with the running desktop process, rather than stale on-disk chats.
pub fn probe(desktop_pids: &[i32]) -> Activity {
    if desktop_pids.is_empty() { return Activity::Idle; }
    let processes = match std::process::Command::new("/bin/ps").args(["-axo", "pid=,ppid=,comm="]).output() {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).into_owned(),
        _ => return Activity::Unknown,
    };
    let backends = backend_pids(&processes, desktop_pids);
    if backends.is_empty() { return Activity::Unknown; }
    let pids = backends.iter().map(i32::to_string).collect::<Vec<_>>().join(",");
    let open = match std::process::Command::new("/usr/sbin/lsof").args(["-b", "-a", "-p", &pids, "-Fn"]).output() {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).into_owned(),
        _ => return Activity::Unknown,
    };
    let paths: BTreeSet<PathBuf> = open.lines().filter_map(|line| line.strip_prefix('n'))
        .filter(|path| path.contains("/sessions/") && path.ends_with(".jsonl"))
        .map(PathBuf::from).collect();
    let mut running = 0; let mut unknown = false;
    for path in paths {
        match rollout_activity(&path) {
            Some(Activity::Running(_)) => running += 1,
            Some(Activity::Unknown) => unknown = true,
            _ => {},
        }
    }
    if running > 0 { Activity::Running(running) } else if unknown { Activity::Unknown } else { Activity::Idle }
}

fn backend_pids(processes: &str, roots: &[i32]) -> Vec<i32> {
    let rows: Vec<_> = processes.lines().filter_map(|line| {
        let mut fields = line.split_whitespace();
        Some((fields.next()?.parse::<i32>().ok()?, fields.next()?.parse::<i32>().ok()?, fields.collect::<Vec<_>>().join(" ")))
    }).collect();
    let mut descendants: BTreeSet<_> = roots.iter().copied().collect();
    for _ in 0..64 {
        let before = descendants.len();
        for (pid, parent, _) in &rows { if descendants.contains(parent) { descendants.insert(*pid); } }
        if before == descendants.len() { break; }
    }
    rows.iter().filter(|(pid, _, command)| descendants.contains(pid) && Path::new(command).file_name().is_some_and(|name| name == "codex"))
        .map(|(pid, _, _)| *pid).collect()
}

/// None excludes CLI/extension sessions, even if they descend from desktop tools.
fn rollout_activity(path: &Path) -> Option<Activity> {
    let mut file = match File::open(path) { Ok(file) => file, Err(_) => return Some(Activity::Unknown) };
    let mut header = Vec::new();
    if (&mut file).take(64 * 1024).read_to_end(&mut header).is_err() { return Some(Activity::Unknown); }
    let metadata = header.split(|byte| *byte == b'\n').next().and_then(|line| serde_json::from_slice::<Value>(line).ok());
    let Some(metadata) = metadata else { return Some(Activity::Unknown); };
    if metadata["type"] != "session_meta" { return Some(Activity::Unknown); }
    if metadata["payload"]["originator"] != "Codex Desktop" {
        return if metadata["payload"]["source"] == "vscode" { Some(Activity::Unknown) } else { None };
    }
    let length = match file.metadata() { Ok(meta) => meta.len(), Err(_) => return Some(Activity::Unknown) };
    // A bounded tail also prevents a large tool-result payload from consuming unbounded memory.
    let start = length.saturating_sub(4 * 1024 * 1024);
    if file.seek(SeekFrom::Start(start)).is_err() { return Some(Activity::Unknown); }
    let mut tail = Vec::new();
    if file.take(4 * 1024 * 1024).read_to_end(&mut tail).is_err() { return Some(Activity::Unknown); }
    if start > 0 { if let Some(newline) = tail.iter().position(|byte| *byte == b'\n') { tail.drain(..=newline); } }
    lifecycle(&tail)
}

fn lifecycle(tail: &[u8]) -> Option<Activity> {
    if !tail.ends_with(b"\n") { return Some(Activity::Unknown); }
    for line in tail.split(|byte| *byte == b'\n').rev().filter(|line| !line.is_empty()) {
        let Ok(event) = serde_json::from_slice::<Value>(line) else { continue; };
        if event["type"] != "event_msg" { continue; }
        match event["payload"]["type"].as_str() {
            Some("task_started" | "turn_started") => return Some(Activity::Running(1)),
            Some("task_complete" | "task_completed" | "turn_completed" | "turn_aborted" | "task_aborted") => return Some(Activity::Idle),
            _ => {},
        }
    }
    Some(Activity::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(kind: &str) -> String { format!("{{\"type\":\"event_msg\",\"payload\":{{\"type\":\"{kind}\"}}}}\n") }
    #[test]
    fn only_unfinished_turns_count_as_running() {
        assert_eq!(lifecycle(event("task_started").as_bytes()), Some(Activity::Running(1)));
        for end in ["task_complete", "turn_completed", "turn_aborted"] {
            assert_eq!(lifecycle(format!("{}{}", event("task_started"), event(end)).as_bytes()), Some(Activity::Idle));
            assert_eq!(lifecycle(format!("{}{}{}", event("task_started"), event(end), event("task_started")).as_bytes()), Some(Activity::Running(1)));
        }
        assert_eq!(lifecycle(b"{\"type\":\"response_item\"}\n"), Some(Activity::Unknown));
        assert_eq!(lifecycle(format!("{}{{\"type\":", event("task_complete")).as_bytes()), Some(Activity::Unknown), "a concurrent partial write cannot prove the desktop is idle");
    }
    #[test]
    fn unrelated_cli_backends_are_not_desktop_activity() {
        let ps = "1 0 /sbin/launchd\n10 1 /Applications/ChatGPT\n11 10 /Applications/CodexCLI/codex\n12 11 /bin/zsh\n13 1 /usr/local/bin/codex\n14 12 /usr/local/bin/codex\n";
        assert_eq!(backend_pids(ps, &[10]), [11, 14]);
    }
    #[test]
    fn session_origin_and_terminal_events_are_checked_without_chat_text() {
        let path = std::env::temp_dir().join(format!("keyturn-activity-{}.jsonl", uuid::Uuid::new_v4()));
        for (origin, expected) in [("Codex Desktop", Some(Activity::Running(1))), ("codex_exec", None)] {
            std::fs::write(&path, format!("{{\"type\":\"session_meta\",\"payload\":{{\"originator\":\"{origin}\"}}}}\n{}", event("task_started"))).unwrap();
            assert_eq!(rollout_activity(&path), expected);
        }
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    #[ignore = "read-only check against an explicitly supplied live desktop PID"]
    fn live_desktop_activity_metadata() {
        let pid = std::env::var("KEYTURN_TEST_CODEX_PID").unwrap().parse().unwrap();
        let activity = probe(&[pid]);
        println!("Desktop activity: {activity:?}");
        assert!(matches!(activity, Activity::Running(_)));
    }
}
