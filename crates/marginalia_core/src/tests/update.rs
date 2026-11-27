use super::*;

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

fn json(tag: &str) -> String {
    format!(r#"{{"tag_name":"{tag}","html_url":"https://github.com/x/y/releases/tag/{tag}","body":"notas"}}"#)
}

#[test]
fn parse_accepts_v_prefix_and_rejects_suffix() {
    assert_eq!(Version::parse("v1.2.3"), Some(Version { major: 1, minor: 2, patch: 3 }));
    assert_eq!(Version::parse("1.2.3"), Some(Version { major: 1, minor: 2, patch: 3 }));
    for bad in ["v1.2", "1.2.3-rc1", "", "1.2.x", "1.2.3.4", "v"] {
        assert_eq!(Version::parse(bad), None, "{bad:?}");
    }
    assert_eq!(v("0.2.0").to_string(), "0.2.0");
}

#[test]
fn compares_numerically() {
    assert!(v("0.10.0") > v("0.9.0"));
    assert!(v("1.0.0") > v("0.99.99"));
}

#[test]
fn parse_release_reads_fields() {
    let r = parse_release(&json("v0.2.0")).unwrap();
    assert_eq!(r.version, v("0.2.0"));
    assert_eq!(r.url, "https://github.com/x/y/releases/tag/v0.2.0");
    assert_eq!(r.notes, "notas");
    let no_body = r#"{"tag_name":"v0.2.0","html_url":"https://a.b/c"}"#;
    assert_eq!(parse_release(no_body).unwrap().notes, "");
    let http = r#"{"tag_name":"v0.2.0","html_url":"http://a.b/c"}"#;
    assert_eq!(parse_release(http), None);
    for bad in ["", "null", "<html>", r#"{"tag_name":null,"html_url":"https://a.b"}"#, r#"{"tag_name":"v1.2.3-rc1","html_url":"https://a.b"}"#] {
        assert_eq!(parse_release(bad), None, "{bad:?}");
    }
}

#[test]
fn evaluate_cases() {
    let cur = v("0.1.0");
    assert!(matches!(evaluate(cur, Some(&json("v0.2.0"))), Check::New(_)));
    assert_eq!(evaluate(cur, Some(&json("v0.1.0"))), Check::UpToDate);
    assert_eq!(evaluate(cur, Some(&json("v0.0.9"))), Check::UpToDate);
    assert_eq!(evaluate(cur, None), Check::Failed);
    assert_eq!(evaluate(cur, Some("lixo")), Check::Failed);
}

#[test]
fn notice_auto_hides_skipped_and_silences_failure() {
    let new = || evaluate(v("0.1.0"), Some(&json("v0.2.0")));
    assert_eq!(notice(new(), Some(v("0.2.0")), false), Notice::Nothing);
    assert!(matches!(notice(new(), Some(v("0.1.5")), false), Notice::Show(_)));
    assert!(matches!(notice(new(), None, false), Notice::Show(_)));
    assert_eq!(notice(Check::UpToDate, None, false), Notice::Nothing);
    assert_eq!(notice(Check::Failed, None, false), Notice::Nothing);
}

#[test]
fn notice_manual_always_answers() {
    let new = || evaluate(v("0.1.0"), Some(&json("v0.2.0")));
    assert!(matches!(notice(new(), Some(v("0.2.0")), true), Notice::Show(_)));
    assert_eq!(notice(Check::UpToDate, None, true), Notice::UpToDate);
    assert_eq!(notice(Check::Failed, None, true), Notice::Failed);
}

#[test]
fn due_cases() {
    let now = 1_000_000;
    assert!(due(None, now));
    assert!(!due(Some(now - 86_399), now));
    assert!(due(Some(now - 86_400), now));
    assert!(due(Some(now + 5), now));
}

#[test]
fn parse_rejects_plus_signs_and_due_survives_extreme_values() {
    assert_eq!(Version::parse("+1.+2.+3"), None);
    assert_eq!(Version::parse("1.2.-3"), None);
    assert!(due(Some(i64::MIN), 0));
    assert!(due(Some(i64::MAX), i64::MIN));
}
