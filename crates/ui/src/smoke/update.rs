//! Cenario do smoke: o aviso de versao nova. Um servidor local (nada de internet) faz o papel do
//! GitHub; `MARGINALIA_UPDATE_URL` aponta para ele.

use super::*;
use marginalia_core::update::Version;
use std::io::{Read, Write};
use std::net::TcpListener;

/// Atende `n` conexoes devolvendo `body` com status 200; devolve o endereco.
fn serve(body: String, n: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("porta local");
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    std::thread::spawn(move || {
        for _ in 0..n {
            let Ok((mut s, _)) = listener.accept() else { break };
            let mut buf = [0u8; 2048];
            let _ = s.read(&mut buf);
            let resp = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            let _ = s.write_all(resp.as_bytes());
        }
    });
    format!("http://127.0.0.1:{port}/")
}

/// Espera ate a janela do aviso abrir (ou nao) por no maximo 10 s; `want` e o que se espera.
async fn wait_update(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, want: bool) -> Result<bool, String> {
    let e = |x: &dyn std::fmt::Display| x.to_string();
    let start = Instant::now();
    loop {
        let open = view_window.update(cx, |m, _, cx| m.open_update(cx)).map_err(|x| e(&x))?;
        if open == want || start.elapsed() > Duration::from_secs(10) {
            return Ok(open);
        }
        cx.background_executor().timer(Duration::from_millis(50)).await;
    }
}

/// Menu Ajuda > Buscar atualizacoes com uma release nova abre o aviso; automatico duas vezes abre
/// uma so; versao pulada cala o automatico mas nao o manual; servidor fora do ar responde "falha".
pub(super) async fn smoke_update(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    let e = |x: &dyn std::fmt::Display| x.to_string();
    let release = r###"{"tag_name":"v9.9.9","html_url":"https://example.com/r","body":"## Novidades\n* uma coisa"}"###.to_string();
    std::env::set_var("MARGINALIA_UPDATE_URL", serve(release, 8));

    menu_by_the(view_window, cx, MenuAction::CheckUpdates).await?;
    if !wait_update(view_window, cx, true).await? {
        return Err("Buscar atualizacoes devia abrir o aviso".into());
    }
    view_window.update(cx, |m, _, cx| m.close_update(cx)).map_err(|x| e(&x))?;
    wait_idle(view_window, cx).await?;

    // Automatico duas vezes na mesma sessao: so um aviso.
    for _ in 0..2 {
        view_window.update(cx, |m, _, cx| m.check_updates(false, cx)).map_err(|x| e(&x))?;
        cx.background_executor().timer(Duration::from_millis(600)).await;
    }
    if !view_window.update(cx, |m, _, cx| m.open_update(cx)).map_err(|x| e(&x))? {
        return Err("o automatico devia abrir o aviso uma vez".into());
    }
    view_window.update(cx, |m, _, cx| m.close_update(cx)).map_err(|x| e(&x))?;
    wait_idle(view_window, cx).await?;

    // Versao pulada: o automatico nao abre, o manual abre.
    let v999 = Version::parse("9.9.9").ok_or("versao")?;
    view_window.update(cx, |m, _, cx| m.skip_update(v999, cx)).map_err(|x| e(&x))?;
    view_window.update(cx, |m, _, cx| m.check_updates(false, cx)).map_err(|x| e(&x))?;
    cx.background_executor().timer(Duration::from_millis(600)).await;
    if view_window.update(cx, |m, _, cx| m.open_update(cx)).map_err(|x| e(&x))? {
        return Err("versao pulada nao devia avisar no automatico".into());
    }
    menu_by_the(view_window, cx, MenuAction::CheckUpdates).await?;
    if !wait_update(view_window, cx, true).await? {
        return Err("o manual devia avisar mesmo da versao pulada".into());
    }
    view_window.update(cx, |m, _, cx| m.close_update(cx)).map_err(|x| e(&x))?;

    // Servidor fora do ar: o manual responde com a janela de falha.
    let closed = {
        let l = TcpListener::bind("127.0.0.1:0").map_err(|x| e(&x))?;
        format!("http://127.0.0.1:{}/", l.local_addr().map_err(|x| e(&x))?.port())
    };
    std::env::set_var("MARGINALIA_UPDATE_URL", closed);
    menu_by_the(view_window, cx, MenuAction::CheckUpdates).await?;
    if !wait_update(view_window, cx, true).await? {
        return Err("sem rede, o manual devia abrir a janela de falha".into());
    }
    view_window.update(cx, |m, _, cx| m.close_update(cx)).map_err(|x| e(&x))?;
    std::env::remove_var("MARGINALIA_UPDATE_URL");
    println!("smoke: aviso de versao ok");
    Ok(())
}
