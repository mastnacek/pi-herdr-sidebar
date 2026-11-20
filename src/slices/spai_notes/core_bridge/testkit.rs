//! A scripted `spai` for tests: answers are queued, calls are recorded.
use super::{CoreConfig, RunOut, Runner};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct FakeRunner {
    answers: Mutex<Vec<Result<RunOut, String>>>,
    pub calls: Mutex<Vec<Vec<String>>>,
}

impl FakeRunner {
    /// A config whose `spai` answers with `answers` in order (the last one repeats).
    pub fn config(answers: Vec<Result<RunOut, String>>) -> (CoreConfig, Arc<FakeRunner>) {
        let fake = Arc::new(FakeRunner {
            answers: Mutex::new(answers),
            calls: Mutex::new(Vec::new()),
        });
        (
            CoreConfig {
                runner: fake.clone(),
            },
            fake,
        )
    }

    pub fn calls(&self) -> Vec<Vec<String>> {
        self.calls.lock().unwrap().clone()
    }
}

impl Runner for FakeRunner {
    fn run(&self, args: &[String], _cwd: &Path) -> Result<RunOut, String> {
        self.calls.lock().unwrap().push(args.to_vec());
        let mut a = self.answers.lock().unwrap();
        if a.len() > 1 {
            a.remove(0)
        } else {
            a.first().cloned().unwrap_or_else(|| Err("no answer".into()))
        }
    }
}

pub fn out(code: i32, stdout: &str, stderr: &str) -> Result<RunOut, String> {
    Ok(RunOut {
        code,
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
    })
}

/// `spai add` answer: written (exit 0).
pub fn written(id: &str, project: &str, path: &str, pending: &[&str]) -> Result<RunOut, String> {
    let p: Vec<String> = pending.iter().map(|s| format!("\"{s}\"")).collect();
    out(
        0,
        &format!(
            r#"{{"command":"add","ok":true,"data":{{"written":true,"id":"{id}","project":"{project}","path":"{path}","title":"Fix login","title_cs":"Oprava přihlášení","pending":[{}],"duplicates":[],"related":[{{"id":"SPAI-007","project":"beta","title":"Příbuzné","score":0.82,"path":"b.md"}}]}}}}"#,
            p.join(",")
        ),
        if pending.is_empty() { "" } else { "pozor: no OpenRouter API key: nothing translated\n" },
    )
}

/// `spai add` answer: stopped by a duplicate (exit 3).
pub fn duplicate(project: &str, id: &str, title: &str) -> Result<RunOut, String> {
    out(
        3,
        &format!(
            r#"{{"command":"add","ok":false,"data":{{"written":false,"id":null,"path":null,"duplicates":[{{"id":"{id}","project":"{project}","title":"{title}","score":0.91,"path":"x.md"}}],"related":[]}}}}"#
        ),
        "",
    )
}
