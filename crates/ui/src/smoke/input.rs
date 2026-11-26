//! Cenarios do smoke: simular mouse e teclado e achar a posicao das coisas na janela.

use super::*;

/// Entrega um evento de mouse aos handlers do visualizador (os mesmos que o
/// gpui chama; `Window::dispatch_event` nao e utilizavel de fora no gpui 0.2.2).
pub(super) fn mouse(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, ev: Mouse) -> Result<(), String> {
    view_window
        .update(cx, |v, w, cx| {
            v.viewer().update(cx, |vw, cx| match &ev {
                Mouse::Down(e) => vw.on_down(e, w, cx),
                Mouse::Move(e) => vw.on_move(e, w, cx),
                Mouse::Up(e) => vw.on_up(e, w, cx),
            })
        })
        .map_err(|e| e.to_string())
}

pub(super) enum Mouse {
    Down(MouseDownEvent),
    Move(MouseMoveEvent),
    Up(MouseUpEvent),
}

/// Arrasto com o botao esquerdo de `a` a `b` (coordenadas da janela), com os modificadores dados.
pub(super) async fn drag(
    view_window: WindowHandle<MainView>,
    cx: &mut AsyncApp,
    a: Point<gpui::Pixels>,
    b: Point<gpui::Pixels>,
    modifiers: Modifiers,
) -> Result<(), String> {
    let btn = MouseButton::Left;
    mouse(view_window, cx, Mouse::Down(MouseDownEvent { button: btn, position: a, modifiers, click_count: 1, first_mouse: false }))?;
    for i in 1..=4 {
        let t = i as f32 / 4.0;
        let p = point(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        mouse(view_window, cx, Mouse::Move(MouseMoveEvent { position: p, pressed_button: Some(btn), modifiers }))?;
    }
    mouse(view_window, cx, Mouse::Up(MouseUpEvent { button: btn, position: b, modifiers, click_count: 1 }))?;
    cx.background_executor().timer(Duration::from_millis(30)).await;
    Ok(())
}

/// Uma tecla pela janela (sem segurar a view raiz: os listeners dela precisam atualiza-la).
pub(super) fn press(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, k: Keystroke) -> Result<bool, String> {
    cx.update_window(view_window.into(), |_, w, cx| w.dispatch_keystroke(k, cx)).map_err(|e| e.to_string())
}

/// Digita `text` tecla a tecla (o caminho de uma tecla de texto: listeners e depois o InputHandler).
pub(super) async fn typing(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, text: &str) -> Result<(), String> {
    for c in text.chars() {
        let key = if c == ' ' { "space".to_string() } else { c.to_string() };
        let k = Keystroke { modifiers: Modifiers::none(), key, key_char: Some(c.to_string()) };
        press(view_window, cx, k)?;
    }
    Ok(())
}

pub(super) async fn key(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, k: &str) -> Result<(), String> {
    // `cmd-` vira `secondary-`: a tecla chega como o sistema a entrega (Cmd no macOS, Ctrl no Windows).
    let k = Keystroke::parse(&crate::menus::gpui_to(k)).map_err(|e| e.to_string())?;
    press(view_window, cx, k)?;
    // Uma volta de pintura (o InputHandler da caixa e registrado ao pintar com foco).
    cx.background_executor().timer(Duration::from_millis(50)).await;
    Ok(())
}

/// Estado do foco: (caixa de nota focada, visualizador focado).
pub(super) fn focus_state(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(bool, bool), String> {
    view_window
        .update(cx, |v, w, cx| {
            (v.quick_note().read(cx).focus_handle().is_focused(w), v.viewer().read(cx).focus_handle().is_focused(w))
        })
        .map_err(|e| e.to_string())
}

/// `/`, digita e Cmd+Enter, conferindo a caixa e o foco a cada passo.
pub(super) async fn keyboard_quick_note(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, text: &str) -> Result<(), String> {
    key(view_window, cx, "/").await?;
    if focus_state(view_window, cx)? != (true, false) {
        return Err("`/` nao pos o foco na caixa de nota".into());
    }
    typing(view_window, cx, text).await?;
    let in_textbox = view_window.update(cx, |v, _, cx| v.quick_note().read(cx).text().to_string()).map_err(|e| e.to_string())?;
    if in_textbox != text {
        return Err(format!("caixa tem {in_textbox:?}, esperava {text:?}"));
    }
    key(view_window, cx, "cmd-enter").await?;
    // A gravacao (criar, fixar o id) roda no fundo; a caixa limpa quando termina.
    wait_idle(view_window, cx).await?;
    let after = view_window.update(cx, |v, _, cx| v.quick_note().read(cx).text().to_string()).map_err(|e| e.to_string())?;
    if !after.is_empty() || focus_state(view_window, cx)? != (false, true) {
        return Err(format!("depois do Cmd+Enter a caixa devia limpar ({after:?}) e o foco voltar a imagem"));
    }
    Ok(())
}

/// Um quadro pintado: as acoes da view registradas (e a disponibilidade dos
/// itens de menu) sao as do ultimo quadro.
pub(super) async fn image_frame(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    view_window.update(cx, |_, w, _| w.refresh()).map_err(|e| e.to_string())?;
    cx.background_executor().timer(Duration::from_millis(60)).await;
    Ok(())
}

/// O item de menu `a` esta ligado agora (o que o macOS pergunta ao abrir o menu)?
pub(super) async fn item_linked(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, a: MenuAction) -> Result<bool, String> {
    image_frame(view_window, cx).await?;
    // Como o `validateMenuItem` do gpui (`App::is_action_available`): no caminho do
    // foco desta janela (a do smoke pode nao ser a ativa) ou com um listener global.
    let g = action_to_gpui(a);
    let in_window = cx.update_window(view_window.into(), |_, w, cx| w.is_action_available(g.as_ref(), cx)).map_err(|e| e.to_string())?;
    let global = cx.update(|cx| cx.is_action_available(g.as_ref())).map_err(|e| e.to_string())?;
    Ok(in_window || global)
}

/// Escolhe o item de menu `a`: a acao vai pela janela, como o macOS a entrega
/// (`App::dispatch_action` -> `Window::dispatch_action`). Erro se estava desligado.
pub(super) async fn menu_by_the(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, a: MenuAction) -> Result<(), String> {
    if !item_linked(view_window, cx, a).await? {
        return Err(format!("item de menu {:?} desligado", menus::hint(a)));
    }
    cx.update_window(view_window.into(), |_, w, cx| w.dispatch_action(action_to_gpui(a), cx)).map_err(|e| e.to_string())?;
    cx.background_executor().timer(Duration::from_millis(60)).await;
    Ok(())
}

/// Ponto `(x, y)` da imagem na janela (para entregar eventos de mouse).
pub(super) fn in_window(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, p: (f32, f32)) -> Result<Point<gpui::Pixels>, String> {
    view_window
        .update(cx, |v, _, cx| {
            let q = v.viewer().read(cx).image_to_screen(p);
            point(px(q.x), px(q.y))
        })
        .map_err(|e| e.to_string())
}

/// A imagem e o ponto (x, y) da imagem continuam dentro da area e a ida e
/// volta janela <-> imagem fecha (o visualizador usa a area nova).
pub(super) fn aligned(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, p: (f32, f32)) -> Result<(bool, String), String> {
    view_window
        .update(cx, |v, _, cx| {
            let vw = v.viewer().read(cx);
            let (x, y, w, h) = vw.area();
            let q = vw.image_to_screen(p);
            let back = vw.screen_to_image(q);
            let inside = q.x >= x && q.x <= x + w && q.y >= y && q.y <= y + h;
            let closes = (back.0 - p.0).abs() < 1e-3 && (back.1 - p.1).abs() < 1e-3;
            let ok = inside && closes && vw.image_has() && !vw.error_shows();
            (ok, format!("ponto {p:?} -> janela ({:.1}, {:.1}) -> imagem ({:.4}, {:.4})", q.x, q.y, back.0, back.1))
        })
        .map_err(|e| e.to_string())
}

/// O layout atual e a largura da imagem que `layout::width_viewer` preve para ele.
pub(super) fn expected(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(layout::Layout, f32), String> {
    view_window
        .update(cx, |v, w, _| (v.layout(), layout::width_viewer(f32::from(w.viewport_size().width), &v.layout())))
        .map_err(|e| e.to_string())
}

/// O centro do grifo `id` da imagem aberta (o overlay desenhado), para conferir o alinhamento.
pub(super) fn highlight_point(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, id: &str) -> Result<(f32, f32), String> {
    view_window
        .update(cx, |v, _, _| match v.state().image_highlights().iter().find(|g| g.id() == id) {
            Some(Highlight::Rectangle { x, y, w, h, .. }) => Ok((x + w / 2.0, y + h / 2.0)),
            Some(Highlight::Pointer { x, y, .. }) => Ok((*x, *y)),
            None => Err(format!("o grifo {id} nao esta na imagem aberta")),
        })
        .map_err(|e| e.to_string())?
}
