use super::*;
use marginalia_core::update::{Release, Version};

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

fn release(notes: &str) -> Release {
    Release { version: v("0.2.0"), url: "https://github.com/x/y/releases/tag/v0.2.0".into(), notes: notes.into() }
}

#[test]
fn show_has_three_buttons_and_versions() {
    let c = content(&Notice::Show(release("")), v("0.1.0")).unwrap();
    assert_eq!(c.buttons, vec![Button::Download, Button::Later, Button::Skip]);
    assert!(c.detail.contains("0.1.0") && c.detail.contains("0.2.0"), "{}", c.detail);
    assert!(!c.heading.is_empty() && !c.title.is_empty());
}

#[test]
fn notes_are_trimmed_and_cleaned() {
    let c = content(&Notice::Show(release("## Novidades\n* a\n- b\n\n  \nc\nd\ne")), v("0.1.0")).unwrap();
    assert_eq!(c.notes, "Novidades\na\nb\nc");
    let long = "x".repeat(1000);
    let c = content(&Notice::Show(release(&long)), v("0.1.0")).unwrap();
    assert!(c.notes.ends_with('…'));
    assert_eq!(c.notes.chars().count(), 301);
}

#[test]
fn up_to_date_and_failed_have_only_ok() {
    let up = content(&Notice::UpToDate, v("0.1.0")).unwrap();
    assert_eq!(up.buttons, vec![Button::Ok]);
    assert!(up.detail.contains("0.1.0"));
    let failed = content(&Notice::Failed, v("0.1.0")).unwrap();
    assert_eq!(failed.buttons, vec![Button::Ok]);
    assert_ne!(up.heading, failed.heading);
}

#[test]
fn nothing_is_none() {
    assert!(content(&Notice::Nothing, v("0.1.0")).is_none());
}

#[test]
fn no_leftover_placeholders() {
    for n in [Notice::Show(release("n")), Notice::UpToDate, Notice::Failed] {
        let c = content(&n, v("0.1.0")).unwrap();
        for text in [&c.title, &c.heading, &c.detail, &c.notes] {
            assert!(!text.contains('{') && !text.contains('}'), "{text:?}");
        }
        for b in &c.buttons {
            assert!(!label(*b).is_empty());
        }
    }
}
