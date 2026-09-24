use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn session_dir_slug(cwd: &str) -> String {
    let mut slug = String::from("--");
    for c in cwd.chars() {
        if c == ':' || c == '\\' || c == '/' {
            slug.push('-');
        } else {
            slug.push(c);
        }
    }
    slug.push_str("--");
    slug
}

pub fn find_session_file(session_id: &str, cwd: &str) -> Option<PathBuf> {
    let base = crate::shared::dirs_home()?
        .join(".pi")
        .join("agent")
        .join("sessions");
    if session_id.is_empty() {
        return None;
    }

    let slug = session_dir_slug(cwd);
    let scoped = base.join(&slug);
    if let Some(hit) = newest_session_in(&scoped, session_id) {
        return Some(hit);
    }

    for dir in fs::read_dir(&base).ok()?.flatten() {
        if dir.path().is_dir() {
            if let Some(hit) = newest_session_in(&dir.path(), session_id) {
                return Some(hit);
            }
        }
    }
    None
}

fn newest_session_in(dir: &Path, session_id: &str) -> Option<PathBuf> {
    let mut best: Option<(PathBuf, SystemTime)> = None;
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".jsonl") && name.contains(session_id) {
            if let Ok(meta) = entry.metadata() {
                let mtime = meta.modified().ok()?;
                match &best {
                    None => best = Some((entry.path(), mtime)),
                    Some((_, t)) if mtime > *t => best = Some((entry.path(), mtime)),
                    _ => {}
                }
            }
        }
    }
    best.map(|(p, _)| p)
}

pub fn find_newest_session(cwd: Option<&str>) -> Option<PathBuf> {
    let base = crate::shared::dirs_home()?
        .join(".pi")
        .join("agent")
        .join("sessions");

    if let Some(cwd) = cwd {
        let slug = session_dir_slug(cwd);
        let scoped = base.join(&slug);
        if let Some(hit) = newest_session_in(&scoped, "") {
            return Some(hit);
        }
    }

    let mut best: Option<(PathBuf, SystemTime)> = None;
    for dir in fs::read_dir(&base).ok()?.flatten() {
        if dir.path().is_dir() {
            if let Some(hit) = newest_session_in(&dir.path(), "") {
                if let Ok(meta) = fs::metadata(&hit) {
                    if let Ok(mtime) = meta.modified() {
                        match &best {
                            None => best = Some((hit, mtime)),
                            Some((_, t)) if mtime > *t => best = Some((hit, mtime)),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    best.map(|(p, _)| p)
}

pub fn find_newest_session_scoped(cwd: Option<&str>) -> Option<PathBuf> {
    let base = crate::shared::dirs_home()?
        .join(".pi")
        .join("agent")
        .join("sessions");
    let cwd = cwd?;
    let slug = session_dir_slug(cwd);
    let scoped = base.join(&slug);
    newest_session_in(&scoped, "")
}
