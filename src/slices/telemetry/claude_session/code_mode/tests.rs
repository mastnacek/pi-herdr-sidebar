use super::*;

#[test]
fn settings_env_string_one_enables() {
    assert!(settings_enable_code_mode(
        r#"{"env":{"CLAUDE_CODE_EXPERIMENTAL_CODE_MODE":"1"},"model":"x"}"#
    ));
}

#[test]
fn settings_without_env_or_with_zero_do_not() {
    assert!(!settings_enable_code_mode(r#"{"model":"x"}"#));
    assert!(!settings_enable_code_mode(r#"{"env":{}}"#));
    assert!(!settings_enable_code_mode(
        r#"{"env":{"CLAUDE_CODE_EXPERIMENTAL_CODE_MODE":"0"}}"#
    ));
    assert!(!settings_enable_code_mode("not json"));
}

#[test]
fn setting_outside_env_is_ignored() {
    assert!(!settings_enable_code_mode(
        r#"{"CLAUDE_CODE_EXPERIMENTAL_CODE_MODE":"1"}"#
    ));
}

#[test]
fn repl_mode_line_needs_true() {
    assert!(is_repl_mode_line("repl-mode", Some(true)));
    assert!(!is_repl_mode_line("repl-mode", Some(false)));
    assert!(!is_repl_mode_line("repl-mode", None));
    assert!(!is_repl_mode_line("mode", Some(true)));
}

#[test]
fn transcript_beats_setting() {
    assert_eq!(resolve(true, false), CodeMode::Transcript);
    assert_eq!(resolve(false, true), CodeMode::Config);
    assert_eq!(resolve(false, false), CodeMode::Off);
    assert!(!CodeMode::Off.is_on() && CodeMode::Config.is_on());
}
