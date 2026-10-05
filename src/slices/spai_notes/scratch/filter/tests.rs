//! Filter parser, fuzzy scoring, and match composition tests.
use super::*;

#[test]
fn parses_composed_tokens() {
    let q = FilterQuery::parse("/build @herdr :ui: !high");
    assert_eq!(q.text.as_deref(), Some("build"));
    assert_eq!(q.project.as_deref(), Some("herdr"));
    assert_eq!(q.tag.as_deref(), Some("ui"));
    assert_eq!(q.priority.as_deref(), Some("high"));
    assert!(!q.is_empty());

    assert!(FilterQuery::parse("").is_empty());
    let bare = FilterQuery::parse("!");
    assert_eq!(bare.priority.as_deref(), Some(""));
}

#[test]
fn fuzzy_scores_subsequences_with_word_start_bonus() {
    let full = fuzzy_score("build", "opravit build @herdr").unwrap();
    assert!(full > 0.5, "word-start hit scores high: {full}");

    let spread = fuzzy_score("obd", "opravit build @herdr").unwrap();
    assert!(spread > 0.0 && full > 0.5, "both scores reasonable");

    assert!(fuzzy_score("zrusen", "zrušení zakázky").is_some());
    // Diacritics fold on both sides ("š" <-> "s").
    assert!(fuzzy_score("sance", "Šance na úspěch").is_some());
    assert!(fuzzy_score("šance", "sance na uspech").is_some());

    assert!(fuzzy_score("xyz", "opravit build").is_none());
}

#[test]
fn status_filter_uses_the_mark() {
    let mk = |t: &str, o: LineOrigin| ScratchLine {
        text: t.to_string(),
        origin: o,
    };
    let lines = vec![
        mk(". open task", LineOrigin::New),
        mk("cont", LineOrigin::New),
        mk("x done task", LineOrigin::FromFile {
            path: Default::default(),
            id: "SPAI-001".into(),
            project: "herdr".into(),
        }),
    ];

    assert!(!matches(&lines, 0, &FilterQuery::default(), StatusFilter::Done, None));
    assert!(matches(&lines, 0, &FilterQuery::default(), StatusFilter::Open, None));
    assert!(matches(&lines, 2, &FilterQuery::default(), StatusFilter::Done, None));
    assert!(!matches(&lines, 2, &FilterQuery::default(), StatusFilter::Open, None));
}

#[test]
fn filter_matches_compose_with_and() {
    let mk = |t: &str| ScratchLine {
        text: t.to_string(),
        origin: LineOrigin::New,
    };
    let lines = vec![
        mk(". Fix build @herdr :ui: !high"),
        mk("detail"),
        mk(". Jiný úkol @jiný"),
    ];

    let q = FilterQuery::parse("build @herdr :ui:");
    assert!(matches(&lines, 0, &q, StatusFilter::All, None));
    assert!(!matches(&lines, 2, &q, StatusFilter::All, None));

    let proj_only = FilterQuery::parse("@herdr");
    assert!(matches(&lines, 0, &proj_only, StatusFilter::All, None));
    assert!(!matches(&lines, 2, &proj_only, StatusFilter::All, None));

    let semantic = vec![0usize];
    assert!(matches(
        &lines,
        0,
        &FilterQuery::default(),
        StatusFilter::All,
        Some(&semantic)
    ));
    assert!(!matches(
        &lines,
        2,
        &FilterQuery::default(),
        StatusFilter::All,
        Some(&semantic)
    ));
}
