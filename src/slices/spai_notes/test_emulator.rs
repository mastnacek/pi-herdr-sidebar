//! Test double of `spai` that really writes files (no translation, no
//! network): enough for the Scratchpad and dialog tests to see records on disk.
//! A text containing `DUPLIKAT` is "a duplicate" until `--force`.
use super::input_highlighter::detect_spai_input;
use super::note::{SpaiFacets, SpaiNoteItem};
use super::spai_prefixes::strip_leading_prefix;
use super::storage_format::format_spai_markdown;
use crate::shared::spai_client::{CoreConfig, RunOut, Runner};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Emulator;

pub fn config() -> CoreConfig {
    CoreConfig {
        runner: Arc::new(Emulator),
    }
}

fn next_number(dir: &Path) -> u32 {
    let mut max = 0;
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if let Some(i) = name.find("SPAI-") {
            let n: String = name[i + 5..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            max = max.max(n.parse().unwrap_or(0));
        }
    }
    max + 1
}

/// Writes `text` as a record into `project/docs/spai`; returns the item.
pub fn write_note(project: &Path, name: &str, text: &str) -> SpaiNoteItem {
    let dir = project.join("docs").join("spai");
    std::fs::create_dir_all(&dir).unwrap();
    let first = text.lines().next().unwrap_or("").trim();
    let detected = detect_spai_input(first);
    let title = strip_leading_prefix(first)
        .unwrap_or(first)
        .trim()
        .to_string();
    let id = format!("SPAI-{:03}", next_number(&dir));
    let file_name = format!("2026-11-20-{}-note.md", id);
    let item = SpaiNoteItem {
        id,
        title,
        kind: detected.kind,
        status: detected.status,
        symbol: detected.status.symbol().to_string(),
        timestamp: "2026-11-20 10:00:00".to_string(),
        tags: Vec::new(),
        facets: SpaiFacets {
            project: Some(name.to_string()),
            ..Default::default()
        },
        body: text.trim().to_string(),
        file_path: dir.join(&file_name),
        file_name,
        extra_frontmatter: Vec::new(),
    };
    std::fs::write(&item.file_path, format_spai_markdown(&item)).unwrap();
    item
}

impl Runner for Emulator {
    fn run(&self, args: &[String], _cwd: &Path) -> Result<RunOut, String> {
        let json = |code: i32, data: String| {
            Ok(RunOut {
                code,
                stdout: format!("{{\"ok\":{},\"data\":{}}}", code == 0, data),
                stderr: String::new(),
            })
        };
        let value_of = |flag: &str| {
            args.iter()
                .position(|a| a == flag)
                .and_then(|i| args.get(i + 1))
        };
        match args.first().map(String::as_str) {
            Some("add") => {
                let text = &args[1];
                let project = PathBuf::from(value_of("--project").cloned().unwrap_or_default());
                let force = args.iter().any(|a| a == "--force");
                if text.contains("DUPLIKAT") && !force {
                    return json(
                        3,
                        r#"{"written":false,"duplicates":[{"id":"SPAI-099","project":"jiny","title":"Stejný záznam","score":0.93,"path":"x.md"}],"related":[]}"#.to_string(),
                    );
                }
                let name = project.file_name().unwrap().to_string_lossy().to_string();
                let item = write_note(&project, &name, text);
                json(
                    0,
                    serde_json::json!({"written": true, "id": item.id, "project": name,
                        "path": item.file_path.to_string_lossy(), "title": item.title,
                        "title_cs": item.title, "pending": [], "duplicates": [], "related": []})
                    .to_string(),
                )
            }
            Some("similar") => json(0, r#"{"duplicates":[],"related":[]}"#.to_string()),
            Some("set") => {
                let project = PathBuf::from(value_of("--project").cloned().unwrap_or_default());
                let dir = project.join("docs").join("spai");
                let (id, status) = (&args[1], &args[2]);
                for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                    let text = std::fs::read_to_string(e.path()).unwrap_or_default();
                    let Some(mut item) =
                        super::storage_format::parse_spai_markdown(&text, e.path())
                    else {
                        continue;
                    };
                    if &item.id == id {
                        item.status = super::note::SpaiStatus::from_str(status);
                        item.symbol = item.status.symbol().to_string();
                        std::fs::write(e.path(), format_spai_markdown(&item)).unwrap();
                        return json(0, "{}".to_string());
                    }
                }
                Ok(RunOut {
                    code: 1,
                    stdout: format!("{{\"ok\":false,\"error\":\"record not found: {id}\"}}"),
                    stderr: String::new(),
                })
            }
            _ => Err("unsupported".to_string()),
        }
    }
}
