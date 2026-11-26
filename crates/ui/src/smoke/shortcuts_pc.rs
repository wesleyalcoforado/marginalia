//! Os atalhos `SoMenu` fora do macOS (Ctrl-F, Ctrl-I, brilho, girar, Configuracoes...): no macOS o
//! menu nativo leva essas teclas ate a view; fora dele so uma `KeyBinding` sem contexto as entrega
//! (`menus::shortcuts_no_menu_native`). Aqui a tecla percorre o caminho real de uma tecla.

use super::*;

pub(super) async fn smoke_shortcuts_pc(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    if cfg!(target_os = "macos") {
        println!("smoke: atalhos do teclado sem menu nativo: so fora do macOS (la o menu entrega a tecla), pulado");
        return Ok(());
    }
    wait(view_window, cx).await?;
    key(view_window, cx, "cmd-shift-k").await?; // imagem em foco
    let image_adjustments = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp| view_window.update(cx, |m, _, _| m.layout().open_adjustments).map_err(|e| e.to_string());
    let invert = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp| view_window.update(cx, |m, _, _| m.state().adjustments.invert).map_err(|e| e.to_string());
    let brightness = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp| view_window.update(cx, |m, _, _| m.state().adjustments.brightness).map_err(|e| e.to_string());
    let contrast = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp| view_window.update(cx, |m, _, _| m.state().adjustments.contrast).map_err(|e| e.to_string());
    let rotation = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp| view_window.update(cx, |m, _, _| m.state().rotation()).map_err(|e| e.to_string());

    // Ctrl-F abre e fecha o painel de ajustes.
    let a0 = image_adjustments(view_window, cx)?;
    key(view_window, cx, "cmd-f").await?;
    let a1 = image_adjustments(view_window, cx)?;
    key(view_window, cx, "cmd-f").await?;
    let a2 = image_adjustments(view_window, cx)?;
    // Ctrl-I inverte e desfaz; brilho e contraste sobem e descem; Ctrl-Shift-R restaura tudo.
    let i0 = invert(view_window, cx)?;
    key(view_window, cx, "cmd-i").await?;
    wait(view_window, cx).await?;
    let i1 = invert(view_window, cx)?;
    let (b0, c0) = (brightness(view_window, cx)?, contrast(view_window, cx)?);
    key(view_window, cx, "cmd-]").await?;
    let b1 = brightness(view_window, cx)?;
    key(view_window, cx, "cmd-[").await?;
    key(view_window, cx, "cmd-[").await?;
    let b2 = brightness(view_window, cx)?;
    key(view_window, cx, "cmd-shift-]").await?;
    let c1 = contrast(view_window, cx)?;
    key(view_window, cx, "cmd-shift-r").await?;
    wait(view_window, cx).await?;
    let (ir, br, cr) = (invert(view_window, cx)?, brightness(view_window, cx)?, contrast(view_window, cx)?);
    // Girar: Ctrl-R e Ctrl-L.
    let r0 = rotation(view_window, cx)?;
    key(view_window, cx, "cmd-r").await?;
    let r1 = rotation(view_window, cx)?;
    key(view_window, cx, "cmd-l").await?;
    let r2 = rotation(view_window, cx)?;
    // Ctrl-, abre as Configuracoes; Esc fecha.
    key(view_window, cx, "cmd-,").await?;
    let conf = view_window.update(cx, |m, _, _| m.config_open()).map_err(|e| e.to_string())?;
    key(view_window, cx, "escape").await?;
    let conf_after = view_window.update(cx, |m, _, _| m.config_open()).map_err(|e| e.to_string())?;
    wait(view_window, cx).await?;
    println!(
        "smoke: atalhos sem menu nativo: Ctrl-F {a0} -> {a1} -> {a2}; Ctrl-I {i0} -> {i1}; brilho {b0} -> ] {b1} -> [[ {b2}; contraste {c0} -> {c1}; restaurar -> inverter {ir} brilho {br} contraste {cr}; girar {r0:?} -> {r1:?} -> {r2:?}; Ctrl-, {conf} -> Esc {conf_after}"
    );
    if a1 == a0 || a2 != a0 {
        return Err("Ctrl-F devia abrir e fechar o painel de ajustes".into());
    }
    if i1 == i0 {
        return Err("Ctrl-I devia inverter".into());
    }
    if !(b1 > b0) || !(b2 < b1) {
        return Err("Ctrl-] devia aumentar e Ctrl-[ diminuir o brilho".into());
    }
    if !(c1 > c0) {
        return Err("Ctrl-Shift-] devia aumentar o contraste".into());
    }
    let default = Adjustments::default();
    if ir != default.invert || br != default.brightness || cr != default.contrast {
        return Err(format!("Ctrl-Shift-R devia restaurar os ajustes padrao ({default:?})"));
    }
    if r1 == r0 || r2 != r0 {
        return Err("Ctrl-R devia girar e Ctrl-L voltar".into());
    }
    if !conf || conf_after {
        return Err("Ctrl-, devia abrir as Configuracoes e Esc fechar".into());
    }
    Ok(())
}
