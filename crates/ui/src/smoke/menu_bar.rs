//! Barra de menus da janela (so fora do macOS, onde o sistema nao desenha a dela): onde o menu
//! aparece de fato (o gpui encaixou o menu em y=0, por cima da barra, quando o `anchored` partia
//! de um elemento centralizado) e o teclado (Alt+letra, setas, letras, Enter e Esc) pelo
//! caminho real das teclas.

use super::*;

use crate::menu_bar::{self, MenuBar};

/// Pede uma pintura e espera: a janela do smoke pode estar sem foco, e o gpui so pinta quando ha motivo.
async fn paint(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    view_window.update(cx, |_, w, _| w.refresh()).map_err(|e| e.to_string())?;
    cx.background_executor().timer(Duration::from_millis(250)).await;
    Ok(())
}

fn bar(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<MenuBar, String> {
    view_window.update(cx, |m, _, _| m.menu_bar().clone()).map_err(|e| e.to_string())
}

fn wait_config(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, open: bool) -> Result<bool, String> {
    view_window.update(cx, |m, _, _| m.config_open() == open).map_err(|e| e.to_string())
}

pub(super) async fn smoke_bar_menu(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    if cfg!(target_os = "macos") {
        println!("smoke: barra de menus da janela: so fora do macOS (la o sistema desenha o menu), pulado");
        return Ok(());
    }
    wait(view_window, cx).await?;
    if bar(view_window, cx)?.active() {
        return Err("a barra de menus devia comecar parada".into());
    }

    // 1. Posicao: o menu aberto pelo mouse fica colado embaixo do titulo, nao no topo da janela.
    view_window.update(cx, |v, _, cx| v.menu_click_title(0, cx)).map_err(|e| e.to_string())?;
    paint(view_window, cx).await?;
    let limits = view_window.update(cx, |v, _, _| v.limits_menu()).map_err(|e| e.to_string())?.ok_or("o menu aberto nao foi desenhado")?;
    // O `canvas` fica dentro da borda de 1 px da caixa do menu (um filho absoluto se posiciona dentro
    // da borda), entao a area medida e a da caixa menos 1 px de cada lado: caixa em (0, 59) 240 de
    // largura -> area em (1, 60) com 238.
    const BORDER: f32 = 1.0;
    let expected_top = layout::TITLE_HEIGHT + layout::MENU_HEIGHT - 1.0 + BORDER;
    let (x, y, l, a) = (f32::from(limits.origin.x), f32::from(limits.origin.y), f32::from(limits.size.width), f32::from(limits.size.height));
    println!("smoke: barra de menus: Arquivo aberto pelo mouse -> area interna x {x:.0} y {y:.0} {l:.0}x{a:.0} (topo esperado {expected_top:.0})");
    if (y - expected_top).abs() > 1.5 {
        return Err(format!("o menu devia abrir colado abaixo da barra (area interna em y={expected_top}), abriu com y={y}"));
    }
    if (x - BORDER).abs() > 1.5 || l < 240.0 - 2.0 * BORDER - 0.5 || a < 60.0 {
        return Err(format!("menu fora do esperado: x {x}, {l}x{a} (esperado x {BORDER}, largura >= {})", 240.0 - 2.0 * BORDER));
    }
    // Fechar de novo pelo mesmo clique, e o menu some.
    view_window.update(cx, |v, _, cx| v.menu_click_title(0, cx)).map_err(|e| e.to_string())?;
    paint(view_window, cx).await?;
    if bar(view_window, cx)?.active() || view_window.update(cx, |v, _, _| v.limits_menu()).map_err(|e| e.to_string())?.is_some() {
        return Err("clicar de novo no titulo devia fechar o menu".into());
    }

    // 2. Alt+letra abre o menu; Baixo marca o primeiro item; a letra de acesso aciona o item.
    key(view_window, cx, "alt-a").await?;
    let b = bar(view_window, cx)?;
    if b.open_at() != Some(0) || !b.keyboard() {
        return Err(format!("Alt+A devia abrir o primeiro menu (Arquivo): {b:?}"));
    }
    key(view_window, cx, "down").await?;
    let b = bar(view_window, cx)?;
    if b.item().is_none() {
        return Err(format!("Baixo devia marcar um item: {b:?}"));
    }
    // "c": a letra de acesso de Configuracoes (menu Arquivo).
    key(view_window, cx, "c").await?;
    paint(view_window, cx).await?;
    if bar(view_window, cx)?.active() || !wait_config(view_window, cx, true)? {
        return Err("a letra C no menu Arquivo devia acionar Configuracoes e fechar a barra".into());
    }
    key(view_window, cx, "escape").await?;
    if !wait_config(view_window, cx, false)? {
        return Err("Esc devia fechar as Configuracoes".into());
    }

    // 3. Alt+letra abre o menu da letra; Esc volta para a barra e outro Esc sai.
    key(view_window, cx, "alt-e").await?;
    let b = bar(view_window, cx)?;
    if b.open_at() != Some(1) || !b.keyboard() {
        return Err(format!("Alt+E devia abrir o segundo menu (Editar): {b:?}"));
    }
    key(view_window, cx, "escape").await?;
    let b = bar(view_window, cx)?;
    if b.open_at().is_some() || b.title() != Some(1) {
        return Err(format!("o 1o Esc devia voltar para a barra no mesmo titulo: {b:?}"));
    }
    key(view_window, cx, "escape").await?;
    if bar(view_window, cx)?.active() {
        return Err("o 2o Esc devia sair da barra".into());
    }

    // 4. Setas trocam de menu dando a volta (o menu continua aberto).
    let n = menu_bar::view(&menus::State::default()).len();
    key(view_window, cx, "alt-a").await?;
    key(view_window, cx, "left").await?;
    let last = bar(view_window, cx)?.open_at();
    key(view_window, cx, "right").await?;
    let first = bar(view_window, cx)?.open_at();
    key(view_window, cx, "escape").await?;
    key(view_window, cx, "escape").await?;
    if last != Some(n - 1) || first != Some(0) || bar(view_window, cx)?.active() {
        return Err(format!("as setas deviam dar a volta nos {n} menus: esquerda {last:?}, direita {first:?}"));
    }

    // 5. Com a barra em uso as teclas nao vazam para a nota nem para a imagem: o enquadramento fica.
    let before = view_window.update(cx, |v, _, cx| v.viewer().read(cx).framing().clone()).map_err(|e| e.to_string())?;
    key(view_window, cx, "alt-a").await?;
    key(view_window, cx, "w").await?;
    key(view_window, cx, "right").await?;
    key(view_window, cx, "escape").await?;
    key(view_window, cx, "escape").await?;
    let after = view_window.update(cx, |v, _, cx| v.viewer().read(cx).framing().clone()).map_err(|e| e.to_string())?;
    if before != after || bar(view_window, cx)?.active() {
        return Err("as teclas da barra de menus vazaram para a imagem".into());
    }
    println!("smoke: barra de menus: Alt+letra, setas, letra e Esc ok; teclas nao vazam para a imagem");
    Ok(())
}
