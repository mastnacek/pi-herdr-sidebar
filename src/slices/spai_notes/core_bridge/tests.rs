//! The bridge with a scripted `spai` (no process, no network, no real backlog),
//! plus one test against the real binary when it exists.
use super::testkit::{duplicate, out, written, FakeRunner};
use super::*;

fn proj() -> PathBuf {
    PathBuf::from("proj")
}

#[test]
fn add_passes_argv_and_parses_the_written_record() {
    let (cfg, fake) = FakeRunner::config(vec![written("SPAI-001", "alpha", "a.md", &[])]);
    let r = add_record(&cfg, &proj(), ". Opravit přihlášení", false).unwrap();
    let AddOutcome::Written(w) = r else { panic!("{r:?}") };
    assert_eq!(w.id, "SPAI-001");
    assert_eq!(w.title_cs.as_deref(), Some("Oprava přihlášení"));
    assert_eq!(w.notice(), "✓ SPAI-001 · Oprava přihlášení");
    assert_eq!(w.related.len(), 1, "related is information only");
    assert!(!w.related[0].duplicate);
    let call = &fake.calls()[0];
    assert_eq!(
        call,
        &[
            "add", ". Opravit přihlášení", "--project", "proj", "--source", "herdr-sidebar",
            "--json"
        ]
    );
}

#[test]
fn offline_write_warns_about_pending() {
    let (cfg, _) = FakeRunner::config(vec![written("SPAI-002", "alpha", "a.md", &["translate_en", "embed"])]);
    let AddOutcome::Written(w) = add_record(&cfg, &proj(), ". x", false).unwrap() else {
        panic!()
    };
    assert_eq!(w.pending, ["translate_en", "embed"]);
    assert_eq!(w.warnings, ["no OpenRouter API key: nothing translated"]);
    assert!(w.notice().contains("bez sítě, čeká: translate_en, embed"), "{}", w.notice());
}

#[test]
fn a_duplicate_is_exit_3_and_force_is_passed() {
    let (cfg, fake) = FakeRunner::config(vec![
        duplicate("beta", "SPAI-005", "Stejné"),
        written("SPAI-001", "alpha", "a.md", &[]),
    ]);
    match add_record(&cfg, &proj(), ". Stejné", false).unwrap() {
        AddOutcome::Duplicates(h) => {
            assert_eq!(h.len(), 1);
            assert!(h[0].duplicate);
            assert_eq!((h[0].project.as_str(), h[0].id.as_str()), ("beta", "SPAI-005"));
            assert_eq!(h[0].percent(), 91);
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        add_record(&cfg, &proj(), ". Stejné", true).unwrap(),
        AddOutcome::Written(_)
    ));
    let calls = fake.calls();
    assert!(!calls[0].contains(&"--force".to_string()));
    assert!(calls[1].contains(&"--force".to_string()));
}

#[test]
fn failure_shows_the_message_of_spai() {
    let (cfg, _) = FakeRunner::config(vec![out(
        1,
        r#"{"code":1,"command":"set","error":"record not found: SPAI-009","ok":false}"#,
        "chyba: record not found: SPAI-009\n",
    )]);
    let e = set_status(&cfg, &proj(), "SPAI-009", "done").unwrap_err();
    assert_eq!(e, "record not found: SPAI-009");
}

#[test]
fn set_similar_and_search_use_their_commands() {
    let (cfg, fake) = FakeRunner::config(vec![
        out(0, r#"{"ok":true,"data":{"changed":true}}"#, ""),
        out(
            0,
            r#"{"ok":true,"data":{"duplicates":[],"related":[{"id":"SPAI-3","project":"p","title":"T","score":0.8,"path":"r.md"}]}}"#,
            "",
        ),
        out(0, r#"{"ok":true,"data":{"results":[{"path":"a.md"},{"path":"b.md"}]}}"#, ""),
    ]);
    set_status(&cfg, &proj(), "SPAI-001", "done").unwrap();
    let h = similar(&cfg, &proj(), ". t").unwrap();
    assert!(!h[0].duplicate);
    let paths = search_paths(&cfg, &proj(), "dotaz").unwrap();
    assert!(paths.contains(&PathBuf::from("a.md")) && paths.len() == 2);
    let c = fake.calls();
    assert_eq!(c[0], ["set", "SPAI-001", "done", "--project", "proj", "--json"]);
    assert_eq!(c[1], ["similar", ". t", "--json"]);
    assert_eq!(c[2], ["search", "dotaz", "--limit", "50", "--json"]);
}

#[test]
fn a_missing_binary_gives_a_clear_message() {
    let cfg = CoreConfig {
        runner: Arc::new(ProcessRunner {
            bin: None,
            env: Vec::new(),
        }),
    };
    let e = add_record(&cfg, &proj(), ". x", false).unwrap_err();
    assert_eq!(e, NOT_FOUND);
    assert!(e.contains("SPAI_BIN") && e.contains("cargo install"));
}

/// The real `spai` (SPAI_BIN, PATH or ~/.cargo/bin) in a temp sandbox with every
/// path redirected and no API key. Skipped when the binary is not installed.
#[test]
fn real_binary_writes_offline_and_stops_a_duplicate() {
    let Some(bin) = find_binary() else {
        eprintln!("spai not installed: skipped");
        return;
    };
    let root = std::env::temp_dir().join(format!("sidebar_spai_real_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (a, b) = (root.join("a"), root.join("b"));
    for p in [&a, &b] {
        std::fs::create_dir_all(p.join(".git")).unwrap();
        std::fs::create_dir_all(p.join("docs").join("spai")).unwrap();
    }
    std::fs::create_dir_all(root.join("home")).unwrap();
    let home = root.join("home").to_string_lossy().to_string();
    let env = |k: &str, v: &str| (k.to_string(), v.to_string());
    let cfg = CoreConfig {
        runner: Arc::new(ProcessRunner {
            bin: Some(bin),
            env: vec![
                env("SPAI_DB", &root.join("index.db").to_string_lossy()),
                env("HOME", &home),
                env("USERPROFILE", &home),
                env("APPDATA", &home),
                env("LOCALAPPDATA", &home),
                env("HERDR_PLUGIN_CONFIG_DIR", &root.join("cfg").to_string_lossy()),
                env("OPENROUTER_API_KEY", ""),
            ],
        }),
    };
    let text = ". Opravit přihlášení uživatele";
    let AddOutcome::Written(w) = add_record(&cfg, &a, text, false).unwrap() else {
        panic!("not written")
    };
    assert!(!w.pending.is_empty(), "no key → pending: {w:?}");
    assert!(w.path.is_file());
    let r = add_record(&cfg, &b, text, false).unwrap();
    let AddOutcome::Duplicates(h) = r else { panic!("{r:?}") };
    assert_eq!(h[0].id, w.id);
    assert!(matches!(add_record(&cfg, &b, text, true).unwrap(), AddOutcome::Written(_)));
    set_status(&cfg, &a, &w.id, "done").unwrap();
    let md = std::fs::read_to_string(&w.path).unwrap();
    assert!(md.contains("status: done") && md.contains("title_cs:"), "{md}");
    let _ = std::fs::remove_dir_all(&root);
}
