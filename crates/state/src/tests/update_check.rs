use super::*;
use marginalia_core::app_data::AppData;
use marginalia_core::update::{Check, Notice, Version};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Mutex;

/// As variaveis de ambiente sao do processo: os testes que as tocam andam um de cada vez.
static ENV: Mutex<()> = Mutex::new(());

/// Servidor de teste: atende uma conexao e devolve `status` + `body`; devolve o endereco.
fn serve_once(status: &'static str, body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            let mut buf = [0u8; 2048];
            let _ = s.read(&mut buf);
            let resp = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            let _ = s.write_all(resp.as_bytes());
        }
    });
    format!("http://127.0.0.1:{port}/")
}

fn release_json(tag: &str) -> String {
    format!(r#"{{"tag_name":"{tag}","html_url":"https://github.com/x/y/releases/tag/{tag}","body":"n"}}"#)
}

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

#[test]
fn fetch_returns_body_on_200() {
    let url = serve_once("200 OK", "ola".into());
    assert_eq!(fetch(&url), Some("ola".to_string()));
}

#[test]
fn fetch_none_on_404_and_on_connection_refused() {
    let url = serve_once("404 Not Found", "{}".into());
    assert_eq!(fetch(&url), None);
    let closed = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://127.0.0.1:{}/", l.local_addr().unwrap().port())
    };
    assert_eq!(fetch(&closed), None);
}

#[test]
fn fetch_none_when_body_over_1mib() {
    let url = serve_once("200 OK", "x".repeat(1024 * 1024 + 10));
    assert_eq!(fetch(&url), None);
}

#[test]
fn run_new_release_shows_in_auto_and_manual() {
    let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
    for manual in [false, true] {
        std::env::set_var("MARGINALIA_UPDATE_URL", serve_once("200 OK", release_json("v9.9.9")));
        let (check, notice) = run(v("0.1.0"), None, manual);
        assert!(matches!(check, Check::New(_)));
        assert!(matches!(notice, Notice::Show(_)));
    }
    std::env::remove_var("MARGINALIA_UPDATE_URL");
}

#[test]
fn run_skipped_auto_is_nothing_manual_shows() {
    let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("MARGINALIA_UPDATE_URL", serve_once("200 OK", release_json("v9.9.9")));
    assert_eq!(run(v("0.1.0"), Some(v("9.9.9")), false).1, Notice::Nothing);
    std::env::set_var("MARGINALIA_UPDATE_URL", serve_once("200 OK", release_json("v9.9.9")));
    assert!(matches!(run(v("0.1.0"), Some(v("9.9.9")), true).1, Notice::Show(_)));
    std::env::remove_var("MARGINALIA_UPDATE_URL");
}

#[test]
fn endpoint_uses_env_when_set_and_not_empty() {
    let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var("MARGINALIA_UPDATE_URL");
    assert_eq!(endpoint(), API_URL);
    std::env::set_var("MARGINALIA_UPDATE_URL", "");
    assert_eq!(endpoint(), API_URL);
    std::env::set_var("MARGINALIA_UPDATE_URL", "http://x/");
    assert_eq!(endpoint(), "http://x/");
    std::env::remove_var("MARGINALIA_UPDATE_URL");
}

#[test]
fn auto_enabled_cases() {
    let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var("MARGINALIA_UPDATE_URL");
    let t = tempfile::tempdir().unwrap();
    let mut d = AppData::new(t.path().to_path_buf());
    let now = 1_000_000;
    assert!(auto_enabled(&d, now, false));
    d.set_last_update_check(now - 100);
    assert!(!auto_enabled(&d, now, false), "intervalo nao venceu");
    d.set_last_update_check(now + 100);
    assert!(auto_enabled(&d, now, false), "relogio para tras");
    d.set_auto_update_check(false);
    assert!(!auto_enabled(&d, now, false), "desligado");
    d.set_auto_update_check(true);
    assert!(!auto_enabled(&d, now, true), "smoke sem url");
    std::env::set_var("MARGINALIA_UPDATE_URL", "http://127.0.0.1:1/");
    assert!(auto_enabled(&d, now, true), "smoke com url");
    std::env::remove_var("MARGINALIA_UPDATE_URL");
}
