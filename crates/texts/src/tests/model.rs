use crate::model::expand;
use crate::register_shortcuts;
use std::sync::Mutex;

/// Os testes que registram resolvedor mexem num estado global: um de cada vez.
pub static LOCKS: Mutex<()> = Mutex::new(());

fn no_resolver() {
    register_shortcuts(|_| None);
}

#[test]
fn fills_names_and_positions() {
    assert_eq!(expand("ola {name}", &[("name", "Ana".into())]), "ola Ana");
    assert_eq!(expand("{0} e {1}", &[("0", "a".into()), ("1", "b".into())]), "a e b");
}

#[test]
fn keys_folded_are_literals() {
    assert_eq!(expand("{{x}}", &[]), "{x}");
}

#[test]
fn value_with_face_of_marker_not_and_reexpanded() {
    let _g = LOCKS.lock().unwrap_or_else(|e| e.into_inner());
    register_shortcuts(|_| Some("⌘O".into()));
    assert_eq!(expand("{name}", &[("name", "{n}".into())]), "{n}");
    assert_eq!(expand("{name}", &[("name", "{shortcut:abrir_fonte}".into())]), "{shortcut:abrir_fonte}");
    no_resolver();
}

#[test]
fn name_unknown_stays_as_written() {
    assert_eq!(expand("{foo}", &[]), "{foo}");
}

#[test]
fn marker_no_resolver_becomes_brackets() {
    let _g = LOCKS.lock().unwrap_or_else(|e| e.into_inner());
    no_resolver();
    assert_eq!(expand("abre {shortcut:abrir_fonte}", &[]), "abre [abrir_fonte]");
}

#[test]
fn marker_with_resolver_uses_the_resolver_and_id_unknown_stays_visible() {
    let _g = LOCKS.lock().unwrap_or_else(|e| e.into_inner());
    register_shortcuts(|id| (id == "abrir_fonte").then(|| "⌘O".to_string()));
    assert_eq!(expand("{shortcut:abrir_fonte}", &[]), "⌘O");
    assert_eq!(expand("{shortcut:x}", &[]), "[x]");
    no_resolver();
}
