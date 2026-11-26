//! Cenarios do smoke: os grifos (ferramentas, rotacao, selecao na tela).

use super::*;

/// Com vault: grifos sobre a pagina girada. Um retangulo (Shift+arrasto) e uma seta
/// (Option+arrasto) desenhados entre pontos da imagem (a tela os acha por `image_to_screen`,
/// que ja leva a rotacao em conta) entram em `highlights/<source>.json` nas coordenadas da imagem
/// SEM girar, em 90 e 180 graus; um clique na borda do retangulo o seleciona (o grifo gira
/// junto com a pagina). Depois os dois grifos saem e a rotacao volta a 0.
/// Barra de ferramentas e ferramentas de grifo (etapa 1 da limpeza da interface): liga a
/// caixa pelo menu (o mesmo `command` do botao), arrasta SEM modificador e confere o
/// grifo no JSON, clica no grifo (seleciona, a ferramenta continua), Esc desliga a
/// ferramenta sem soltar a selecao e o seguinte solta; a seta; esconder a barra desliga a
/// ferramenta e fica nos dados do app; a ajuda abre e fecha por Esc e por `?`; as Configuracoes abrem pelo menu, gravam a escolha e fecham por Esc.
pub(super) async fn smoke_tools(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, v: &Vault) -> Result<(), String> {
    fn e(x: impl std::fmt::Display) -> String {
        x.to_string()
    }
    use annotate::Tool;
    let no_error = view_window.update(cx, |m, _, _| m.state().source_has() && m.state().error.is_none()).map_err(e)?;
    if !no_error {
        return Ok(());
    }
    let image = view_window.update(cx, |m, _, _| m.state().current.image.clone()).map_err(e)?;
    let root_dir = v.root.clone();
    let tool = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp| {
        view_window.update(cx, |m, _, cx| m.viewer().read(cx).tool()).map_err(|x| x.to_string())
    };
    view_window.update(cx, |m, w, cx| m.focus_image(w, cx)).map_err(e)?;
    view_window
        .update(cx, |m, _, cx| m.viewer().update(cx, |vw, cx| vw.set_framing(viewer_math::default_framing(), cx)))
        .map_err(e)?;
    area_painted(view_window, cx).await?;
    if !view_window.update(cx, |m, _, _| m.visible_tools()).map_err(e)? {
        return Err("a barra de ferramentas devia comecar visivel".into());
    }
    let near = |v: &serde_json::Value, expected: f64| v.as_f64().is_some_and(|x| (x - expected).abs() < 0.01);

    // Caixa: ligada pelo menu, arrasto sem modificador desenha e a ferramenta continua ligada.
    menu_by_the(view_window, cx, MenuAction::HighlightBox).await?;
    if tool(view_window, cx)? != Tool::Textbox {
        return Err("GrifoCaixa devia ligar a ferramenta caixa".into());
    }
    let before = window_ids(view_window, cx)?;
    let (p, q) = (in_window(view_window, cx, (0.2, 0.3))?, in_window(view_window, cx, (0.5, 0.6))?);
    drag(view_window, cx, p, q, Modifiers::none()).await?;
    let ret = highlights_window(view_window, cx)?
        .into_iter()
        .find(|g| !before.contains(&g.id().to_string()))
        .ok_or("a ferramenta caixa nao criou o retangulo")?;
    let j = json_highlight(&root_dir, &image, ret.id())?.ok_or("retangulo nao esta no JSON")?;
    println!("smoke: ferramenta caixa: arrasto sem modificador -> JSON {j}");
    if j["kind"] != "rectangle" || !near(&j["x"], 0.2) || !near(&j["y"], 0.3) || !near(&j["w"], 0.3) || !near(&j["h"], 0.3) {
        return Err(format!("retangulo da ferramenta fora das coordenadas da imagem: {j}"));
    }
    if tool(view_window, cx)? != Tool::Textbox {
        return Err("a ferramenta devia continuar ligada depois de desenhar".into());
    }
    // Clique sem arrastar com a ferramenta ligada seleciona o grifo (nao desenha nada).
    let n = window_ids(view_window, cx)?.len();
    let c0 = in_window(view_window, cx, (0.2, 0.45))?;
    drag(view_window, cx, c0, point(c0.x + px(2.0), c0.y + px(1.0)), Modifiers::none()).await?;
    let id = Some(ret.id().to_string());
    if selection_window(view_window, cx)? != (id.clone(), id.clone()) || window_ids(view_window, cx)?.len() != n {
        return Err("clique com a ferramenta ligada devia so selecionar o retangulo".into());
    }
    // Esc: desliga a ferramenta e a selecao fica; o segundo Esc solta o grifo.
    key(view_window, cx, "escape").await?;
    if tool(view_window, cx)? != Tool::No || selection_window(view_window, cx)?.0 != id {
        return Err("o primeiro Esc devia so desligar a ferramenta".into());
    }
    key(view_window, cx, "escape").await?;
    if selection_window(view_window, cx)?.0.is_some() {
        return Err("o segundo Esc devia soltar a selecao".into());
    }

    // Seta: arrasto sem modificador; clique no vazio nao cria grifo.
    menu_by_the(view_window, cx, MenuAction::ArrowHighlight).await?;
    let before = window_ids(view_window, cx)?;
    let (p, q) = (in_window(view_window, cx, (0.6, 0.2))?, in_window(view_window, cx, (0.8, 0.7))?);
    drag(view_window, cx, p, q, Modifiers::none()).await?;
    let arrow = highlights_window(view_window, cx)?
        .into_iter()
        .find(|g| !before.contains(&g.id().to_string()))
        .ok_or("a ferramenta seta nao criou a seta")?;
    let j = json_highlight(&root_dir, &image, arrow.id())?.ok_or("seta nao esta no JSON")?;
    if j["kind"] != "pointer" || !near(&j["x"], 0.8) || !near(&j["y"], 0.7) || !near(&j["tail"][0], 0.6) {
        return Err(format!("seta da ferramenta fora das coordenadas da imagem: {j}"));
    }
    let n = window_ids(view_window, cx)?.len();
    let empty = in_window(view_window, cx, (0.05, 0.9))?;
    drag(view_window, cx, empty, point(empty.x + px(2.0), empty.y + px(1.0)), Modifiers::none()).await?;
    if window_ids(view_window, cx)?.len() != n {
        return Err("clique no vazio com a ferramenta seta nao pode criar grifo".into());
    }

    // Esconder a barra pelo menu Exibir desliga a ferramenta e vai para os dados do app.
    menu_by_the(view_window, cx, MenuAction::Tools).await?;
    wait_idle(view_window, cx).await?;
    let (visible, hides_in_data) =
        view_window.update(cx, |m, _, _| (m.visible_tools(), m.state().app_data.hidden_tools())).map_err(e)?;
    if visible || !hides_in_data || tool(view_window, cx)? != Tool::No {
        return Err("ocultar a barra devia desligar a ferramenta e guardar o valor".into());
    }
    menu_by_the(view_window, cx, MenuAction::Tools).await?;
    if !view_window.update(cx, |m, _, _| m.visible_tools()).map_err(e)? {
        return Err("a barra devia voltar a aparecer".into());
    }

    // Ajuda: o menu abre, Esc fecha, `?` abre e `?` fecha.
    let open = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp| view_window.update(cx, |m, _, _| m.open_help()).map_err(|x| x.to_string());
    menu_by_the(view_window, cx, MenuAction::Help).await?;
    if !open(view_window, cx)? {
        return Err("o menu Ajuda devia abrir a ajuda".into());
    }
    key(view_window, cx, "escape").await?;
    if open(view_window, cx)? {
        return Err("Esc devia fechar a ajuda".into());
    }
    key(view_window, cx, "?").await?;
    if !open(view_window, cx)? {
        return Err("`?` devia abrir a ajuda".into());
    }
    key(view_window, cx, "?").await?;
    if open(view_window, cx)? {
        return Err("`?` devia fechar a ajuda".into());
    }

    // Sobre: o menu abre uma janela propria (uma so, mesmo pedindo de novo) e fechar a tira.
    {
        let view_windows = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp| view_window.update(cx, |_, _, cx| cx.windows().len()).map_err(e);
        let before = view_windows(view_window, cx)?;
        menu_by_the(view_window, cx, MenuAction::About).await?;
        if !view_window.update(cx, |m, _, cx| m.open_about(cx)).map_err(e)? || view_windows(view_window, cx)? != before + 1 {
            return Err("o menu Sobre devia abrir uma janela propria".into());
        }
        println!("smoke: sobre aberta");
        if let Some(ms) = std::env::var("MARGINALIA_SMOKE_PAUSA").ok().and_then(|v| v.parse::<u64>().ok()) {
            cx.background_executor().timer(Duration::from_millis(ms)).await;
        }
        menu_by_the(view_window, cx, MenuAction::About).await?;
        if view_windows(view_window, cx)? != before + 1 {
            return Err("pedir o Sobre de novo nao devia abrir uma segunda janela".into());
        }
        view_window.update(cx, |m, _, cx| m.close_about(cx)).map_err(e)?;
        wait_idle(view_window, cx).await?;
        if view_window.update(cx, |m, _, cx| m.open_about(cx)).map_err(e)? || view_windows(view_window, cx)? != before {
            return Err("fechar o Sobre devia tirar a janela".into());
        }
        println!("smoke: sobre ok");
    }

    // Configuracoes: o menu abre, escolher "reabrir a ultima fonte: sim" grava nos dados do app,
    // Esc fecha; "Restaurar padroes" volta ao nao. Com a tela aberta as teclas nao vazam.
    {
        use crate::config::Id;
        let open = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp| view_window.update(cx, |m, _, _| m.config_open()).map_err(e);
        let base = view_window.update(cx, |m, _, _| m.state().app_data.base().to_path_buf()).map_err(e)?;
        menu_by_the(view_window, cx, MenuAction::Settings).await?;
        if !open(view_window, cx)? {
            return Err("o menu Configuracoes devia abrir a tela".into());
        }
        println!("smoke: configuracoes abertas");
        if let Some(ms) = std::env::var("MARGINALIA_SMOKE_PAUSA").ok().and_then(|v| v.parse::<u64>().ok()) {
            cx.background_executor().timer(Duration::from_millis(ms)).await;
        }
        view_window.update(cx, |m, _, cx| m.choose_config(Id::ReopenLastSource, 0, cx)).map_err(e)?;
        wait_idle(view_window, cx).await?;
        if !AppData::load(base.clone()).map_err(|x| x.to_string())?.reopen_last_source() {
            return Err("escolher 'sim' devia gravar reabrir_ultima_fonte nos dados do app".into());
        }
        // Ritmo do salvamento da nota: grava no disco e chega a sessao da nota na hora.
        use marginalia_core::app_data::QuickNoteRhythm;
        view_window.update(cx, |m, _, cx| m.choose_config(Id::QuickNoteRhythm, 2, cx)).map_err(e)?;
        wait_idle(view_window, cx).await?;
        let from_session = view_window.update(cx, |m, _, _| m.quick_note_rhythm()).map_err(e)?;
        if from_session != QuickNoteRhythm::Calm || AppData::load(base.clone()).map_err(|x| x.to_string())?.quick_note_rhythm() != QuickNoteRhythm::Calm {
            return Err(format!("escolher 'com calma' devia valer na sessao ({from_session:?}) e no disco"));
        }
        // Tema: troca a paleta da janela na hora e fica no disco; o claro e o sepia chegam a pintar um
        // quadro sem erro.
        use marginalia_core::app_data::Theme;
        for (i, theme) in [(1, Theme::Light), (2, Theme::Sepia)] {
            view_window.update(cx, |m, _, cx| m.choose_config(Id::Theme, i, cx)).map_err(e)?;
            wait_idle(view_window, cx).await?;
            if crate::theme::current_theme() != theme || AppData::load(base.clone()).map_err(|x| x.to_string())?.theme() != theme {
                return Err(format!("escolher o tema {theme:?} devia valer na janela e nos dados do app"));
            }
            if let Some(ms) = std::env::var("MARGINALIA_SMOKE_PAUSA").ok().and_then(|v| v.parse::<u64>().ok()) {
                cx.background_executor().timer(Duration::from_millis(ms)).await;
            }
        }
        // Letra da nota e largura do painel: valem na hora e ficam no disco.
        use marginalia_core::app_data::FontSize;
        view_window.update(cx, |m, _, cx| m.choose_config(Id::QuickNoteFont, 2, cx)).map_err(e)?;
        view_window.update(cx, |m, _, cx| m.choose_config(Id::PanelWidth, 2, cx)).map_err(e)?;
        wait_idle(view_window, cx).await?;
        let (font, width) = view_window.update(cx, |m, _, cx| (m.quick_note_size_font(cx), m.panel_width())).map_err(e)?;
        let disk = AppData::load(base.clone()).map_err(|x| x.to_string())?;
        if font != Some(FontSize::Large.pixels()) || width != 460.0 {
            return Err(format!("letra grande e painel largo devem valer na tela (letra {font:?}, largura {width})"));
        }
        if disk.quick_note_font() != FontSize::Large || disk.drawers_width() != Some(460.0) {
            return Err("letra e largura escolhidas devem estar nos dados do app".into());
        }
        key(view_window, cx, "escape").await?;
        if open(view_window, cx)? {
            return Err("Esc devia fechar as Configuracoes".into());
        }
        menu_by_the(view_window, cx, MenuAction::Settings).await?;
        view_window.update(cx, |m, _, cx| m.restore_config(cx)).map_err(e)?;
        wait_idle(view_window, cx).await?;
        let restored = AppData::load(base).map_err(|x| x.to_string())?;
        if crate::theme::current_theme() != Theme::Dark || restored.theme() != Theme::Dark {
            return Err("restaurar padroes devia voltar ao tema escuro".into());
        }
        if restored.reopen_last_source() || restored.quick_note_rhythm() != QuickNoteRhythm::Normal {
            return Err("restaurar padroes devia desligar reabrir_ultima_fonte e voltar o ritmo ao normal".into());
        }
        if view_window.update(cx, |m, _, _| m.quick_note_rhythm()).map_err(e)? != QuickNoteRhythm::Normal {
            return Err("restaurar padroes devia valer na sessao da nota".into());
        }
        let (font, width) = view_window.update(cx, |m, _, cx| (m.quick_note_size_font(cx), m.panel_width())).map_err(e)?;
        if font != Some(FontSize::Media.pixels()) || width != crate::drawers::WIDTH || restored.drawers_width().is_some() {
            return Err(format!("restaurar padroes devia voltar letra media e painel medio (letra {font:?}, largura {width})"));
        }
        menu_by_the(view_window, cx, MenuAction::Settings).await?;
        if open(view_window, cx)? {
            return Err("o menu devia fechar as Configuracoes".into());
        }
    }

    // Limpeza: os dois grifos saem.
    for id in [ret.id().to_string(), arrow.id().to_string()] {
        view_window.update(cx, |m, _, cx| m.select_highlight(&image, Some(id), cx)).map_err(e)?;
        view_window.update(cx, |m, _, cx| m.delete_highlight_selected(cx)).map_err(e)?;
    }
    wait(view_window, cx).await?;
    println!("smoke: ferramentas e ajuda ok");
    Ok(())
}

pub(super) async fn smoke_rotation_highlights(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, v: &Vault) -> Result<(), String> {
    fn e(x: impl std::fmt::Display) -> String {
        x.to_string()
    }
    let no_error = view_window.update(cx, |m, _, _| m.state().source_has() && m.state().error.is_none()).map_err(e)?;
    if !no_error {
        return Ok(());
    }
    let image = view_window.update(cx, |m, _, _| m.state().current.image.clone()).map_err(e)?;
    let root_dir = v.root.clone();
    view_window.update(cx, |m, w, cx| m.focus_image(w, cx)).map_err(e)?;
    view_window
        .update(cx, |m, _, cx| m.viewer().update(cx, |vw, cx| vw.set_framing(viewer_math::default_framing(), cx)))
        .map_err(e)?;
    area_painted(view_window, cx).await?;
    let shift = Modifiers { shift: true, ..Modifiers::none() };
    let alt = Modifiers { alt: true, ..Modifiers::none() };
    let near = |v: &serde_json::Value, expected: f64| v.as_f64().is_some_and(|x| (x - expected).abs() < 0.01);
    let mut done: Vec<String> = Vec::new();

    for (degrees, rotations) in [(90, 1), (180, 1)] {
        for _ in 0..rotations {
            menu_by_the(view_window, cx, MenuAction::RotateRight).await?;
        }
        wait(view_window, cx).await?;
        let rot = view_window.update(cx, |m, _, _| m.state().rotation()).map_err(e)?;
        if rot.degrees() != degrees {
            return Err(format!("esperava {degrees} graus, veio {rot:?}"));
        }
        // Retangulo da imagem (0.2, 0.3) a (0.5, 0.6).
        let before = window_ids(view_window, cx)?;
        let (p, q) = (in_window(view_window, cx, (0.2, 0.3))?, in_window(view_window, cx, (0.5, 0.6))?);
        drag(view_window, cx, p, q, shift).await?;
        let ret = highlights_window(view_window, cx)?
            .into_iter()
            .find(|g| !before.contains(&g.id().to_string()))
            .ok_or("o retangulo nao foi criado")?;
        let j = json_highlight(&root_dir, &image, ret.id())?.ok_or("retangulo nao esta no JSON")?;
        println!("smoke: rotacao {degrees}°: retangulo (0.2, 0.3)-(0.5, 0.6) da imagem -> JSON {j}");
        if j["kind"] != "rectangle" || !near(&j["x"], 0.2) || !near(&j["y"], 0.3) || !near(&j["w"], 0.3) || !near(&j["h"], 0.3) {
            return Err(format!("rotacao {degrees}°: retangulo gravado fora das coordenadas da imagem: {j}"));
        }
        done.push(ret.id().to_string());
        // Seta da cauda (0.6, 0.2) a ponta (0.8, 0.7), em outro lugar da pagina.
        let before = window_ids(view_window, cx)?;
        let (p, q) = (in_window(view_window, cx, (0.6, 0.2))?, in_window(view_window, cx, (0.8, 0.7))?);
        drag(view_window, cx, p, q, alt).await?;
        let arrow = highlights_window(view_window, cx)?
            .into_iter()
            .find(|g| !before.contains(&g.id().to_string()))
            .ok_or("a seta nao foi criada")?;
        let j = json_highlight(&root_dir, &image, arrow.id())?.ok_or("seta nao esta no JSON")?;
        println!("smoke: rotacao {degrees}°: seta (0.6, 0.2)->(0.8, 0.7) da imagem -> JSON {j}");
        if j["kind"] != "pointer"
            || !near(&j["x"], 0.8)
            || !near(&j["y"], 0.7)
            || !near(&j["tail"][0], 0.6)
            || !near(&j["tail"][1], 0.2)
        {
            return Err(format!("rotacao {degrees}°: seta gravada fora das coordenadas da imagem: {j}"));
        }
        done.push(arrow.id().to_string());
        // Clique na borda esquerda do retangulo (imagem (0.2, 0.45)): seleciona-o; no vazio, nada.
        let click = |p: Point<gpui::Pixels>| (p, point(p.x + px(2.0), p.y + px(1.0)));
        let (c0, c1) = click(in_window(view_window, cx, (0.2, 0.45))?);
        drag(view_window, cx, c0, c1, Modifiers::none()).await?;
        let sel = selection_window(view_window, cx)?;
        let (c0, c1) = click(in_window(view_window, cx, (0.05, 0.9))?);
        drag(view_window, cx, c0, c1, Modifiers::none()).await?;
        let empty = selection_window(view_window, cx)?;
        println!("smoke: rotacao {degrees}°: clique na borda -> {sel:?}; no vazio -> {empty:?}");
        let id = Some(ret.id().to_string());
        if sel != (id.clone(), id) || empty != (None, None) {
            return Err(format!("rotacao {degrees}°: o clique devia selecionar o retangulo girado junto com a pagina"));
        }
    }

    // Volta a 0 pelo outro sentido (Cmd-L duas vezes) e confere que os grifos continuam onde estavam.
    for _ in 0..2 {
        menu_by_the(view_window, cx, MenuAction::RotateLeft).await?;
    }
    wait(view_window, cx).await?;
    if view_window.update(cx, |m, _, _| m.state().rotation().degrees()).map_err(e)? != 0 {
        return Err("duas voltas para a esquerda devolvem a rotacao 0".into());
    }
    let (c0, c1) = {
        let p = in_window(view_window, cx, (0.2, 0.45))?;
        (p, point(p.x + px(2.0), p.y + px(1.0)))
    };
    drag(view_window, cx, c0, c1, Modifiers::none()).await?;
    // Os dois retangulos (gravados em 90 e em 180 graus) ocupam o mesmo lugar da imagem: o clique
    // acha um deles, o de cima.
    let found = selection_window(view_window, cx)?.0;
    if !found.as_ref().is_some_and(|id| *id == done[0] || *id == done[2]) {
        return Err(format!("com a rotacao 0 o retangulo devia estar onde foi gravado, o clique achou {found:?}"));
    }
    // Limpeza: os quatro grifos saem (selecionar e apagar, como Delete).
    for id in done {
        view_window.update(cx, |m, _, cx| m.select_highlight(&image, Some(id), cx)).map_err(e)?;
        view_window.update(cx, |m, _, cx| m.delete_highlight_selected(cx)).map_err(e)?;
    }
    wait(view_window, cx).await?;
    Ok(())
}

/// O grifo `id` como esta em `highlights/<source>.json` (JSON cru), se estiver.
pub(super) fn json_highlight(root_dir: &Path, img: &marginalia_core::model::ImageRef, id: &str) -> Result<Option<serde_json::Value>, String> {
    let fname = root_dir.join("grifos").join(format!("{}.json", img.source));
    let text = std::fs::read_to_string(&fname).map_err(|e| format!("{}: {e}", fname.display()))?;
    let j: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let key = format!("{}#{}", img.file, img.page);
    Ok(j["images"][key.as_str()].as_array().and_then(|gs| gs.iter().find(|g| g["id"] == id).cloned()))
}

/// Task 9: grifos na tela pelos handlers reais (mouse no visualizador, teclas
/// pela janela, a amostra do seletor de cor): retangulo, seta por Option+arrasto
/// (cauda -> ponta), seta curta por Option+clique, clique que seleciona sem
/// mover a imagem, cor pela paleta, Delete apaga, Cmd+Z desfaz, Esc tira a
/// selecao; Backspace/Cmd+Z com a caixa em foco nao tocam nos grifos. Confere
/// `highlights/<source>.json` (tipo, cor, cauda) a cada passo e, depois de reler
/// tudo do disco (dados do app e grifos, como ao reabrir), que as cores sao as
/// mesmas. `MARGINALIA_SMOKE_PAUSA=ms` para com seta, retangulo, selecao e a
/// paleta na tela (para uma captura da janela).
pub(super) async fn smoke_highlights_screen(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, v: &Vault) -> Result<(), String> {
    use marginalia_core::highlights::PALETTE;
    let img = view_window.update(cx, |m, _, _| m.state().current.image.clone()).map_err(|e| e.to_string())?;
    let root_dir = v.root.clone();
    view_window.update(cx, |m, w, cx| m.focus_image(w, cx)).map_err(|e| e.to_string())?;
    area_painted(view_window, cx).await?;
    view_window
        .update(cx, |m, _, cx| m.viewer().update(cx, |vw, cx| vw.set_framing(viewer_math::default_framing(), cx)))
        .map_err(|e| e.to_string())?;
    area_painted(view_window, cx).await?;
    let before = window_ids(view_window, cx)?;
    let new = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp, before: &[String]| -> Result<Highlight, String> {
        let gs = highlights_window(view_window, cx)?;
        let n: Vec<Highlight> = gs.into_iter().filter(|g| !before.contains(&g.id().to_string())).collect();
        match n.as_slice() {
            [g] => Ok(g.clone()),
            o => Err(format!("esperava 1 grifo novo, vieram {o:?}")),
        }
    };
    let shift = Modifiers { shift: true, ..Modifiers::none() };
    let alt = Modifiers { alt: true, ..Modifiers::none() };

    // (1) Shift+arrasto: retangulo na cor padrao (sem `color` no JSON)
    let (a, b) = (in_window(view_window, cx, (0.15, 0.6))?, in_window(view_window, cx, (0.4, 0.85))?);
    drag(view_window, cx, a, b, shift).await?;
    let ret = new(view_window, cx, &before)?;
    let j = json_highlight(&root_dir, &img, ret.id())?.ok_or("retangulo nao esta no JSON")?;
    println!("smoke: grifos na tela: retangulo {} -> JSON {j}", ret.id());
    if j["kind"] != "rectangle" || j.get("color").is_some() {
        return Err(format!("retangulo errado no JSON: {j}"));
    }

    // (2) cor pela paleta (o que a amostra chama), sem selecao: so a cor dos novos
    choose_color(view_window, cx, 1).await?;
    let current = view_window.update(cx, |m, _, _| m.state().current_color()).map_err(|e| e.to_string())?;
    if current != PALETTE[1] || json_highlight(&root_dir, &img, ret.id())?.is_some_and(|j| j.get("color").is_some()) {
        return Err(format!("a paleta sem selecao devia mudar so a cor dos novos (atual {current})"));
    }

    // (3) Option+arrasto: seta da cauda (0.3, 0.2) ate a ponta (0.7, 0.5), vermelha
    let before = window_ids(view_window, cx)?;
    let (a, b) = (in_window(view_window, cx, (0.3, 0.2))?, in_window(view_window, cx, (0.7, 0.5))?);
    drag(view_window, cx, a, b, alt).await?;
    let arrow = new(view_window, cx, &before)?;
    let j = json_highlight(&root_dir, &img, arrow.id())?.ok_or("seta nao esta no JSON")?;
    println!("smoke: Option+arrasto -> {arrow:?}; JSON {j}");
    let near = |v: &serde_json::Value, e: f64| v.as_f64().is_some_and(|x| (x - e).abs() < 0.01);
    if j["kind"] != "pointer"
        || j["color"] != PALETTE[1]
        || !near(&j["x"], 0.7)
        || !near(&j["y"], 0.5)
        || !near(&j["tail"][0], 0.3)
        || !near(&j["tail"][1], 0.2)
    {
        return Err(format!("seta errada no JSON: {j}"));
    }

    // (4) Option+clique: seta curta padrao com a ponta no clique, a cauda acima e a esquerda
    let before = window_ids(view_window, cx)?;
    let c = in_window(view_window, cx, (0.8, 0.8))?;
    drag(view_window, cx, c, point(c.x + px(1.0), c.y + px(1.0)), alt).await?;
    let short = new(view_window, cx, &before)?;
    println!("smoke: Option+clique -> {short:?}");
    match &short {
        Highlight::Pointer { x, y, tail: Some((tx, ty)), .. } if (x - 0.8).abs() < 0.01 && (y - 0.8).abs() < 0.01 && tx < x && ty < y => {}
        o => return Err(format!("seta curta errada: {o:?}")),
    }

    // (5) clique simples seleciona sem mover a imagem; no vazio tira a selecao
    let framing_0 = view_window.update(cx, |m, _, cx| m.viewer().read(cx).framing().clone()).map_err(|e| e.to_string())?;
    let click = |p: Point<gpui::Pixels>| (p, point(p.x + px(2.0), p.y + px(1.0)));
    let (p, q) = click(in_window(view_window, cx, (0.5, 0.35))?); // meio da seta
    drag(view_window, cx, p, q, Modifiers::none()).await?;
    let s1 = selection_window(view_window, cx)?;
    let (p, q) = click(in_window(view_window, cx, (0.25, 0.7))?); // dentro do retangulo
    drag(view_window, cx, p, q, Modifiers::none()).await?;
    let s2 = selection_window(view_window, cx)?;
    let (p, q) = click(in_window(view_window, cx, (0.9, 0.1))?); // vazio
    drag(view_window, cx, p, q, Modifiers::none()).await?;
    let s3 = selection_window(view_window, cx)?;
    let framing_1 = view_window.update(cx, |m, _, cx| m.viewer().read(cx).framing().clone()).map_err(|e| e.to_string())?;
    println!("smoke: cliques -> seta {s1:?}, retangulo {s2:?}, vazio {s3:?}; enquadramento {framing_0:?} -> {framing_1:?}");
    let sel = |id: &str| (Some(id.to_string()), Some(id.to_string()));
    if s1 != sel(arrow.id()) || s2 != sel(ret.id()) || s3 != (None, None) || framing_0 != framing_1 {
        return Err("clique devia selecionar (estado e tela) sem mover a imagem".into());
    }

    // (6) com o retangulo selecionado, a paleta o recolore (azul) e vira a cor dos novos
    let (p, q) = click(in_window(view_window, cx, (0.25, 0.7))?);
    drag(view_window, cx, p, q, Modifiers::none()).await?;
    let colors_0 = view_window.update(cx, |m, _, cx| m.viewer().read(cx).screen_colors()).map_err(|e| e.to_string())?;
    choose_color(view_window, cx, 3).await?;
    let j = json_highlight(&root_dir, &img, ret.id())?.ok_or("retangulo sumiu")?;
    let colors = view_window.update(cx, |m, _, cx| m.viewer().read(cx).screen_colors()).map_err(|e| e.to_string())?;
    let color_of = |cs: &[(String, u32)], id: &str| cs.iter().find(|(i, _)| i == id).map(|(_, c)| *c);
    println!("smoke: paleta com o retangulo selecionado -> JSON {j}; cores na tela {colors:?}");
    if j["color"] != PALETTE[3]
        || color_of(&colors, ret.id()) != Some(0x1e88e5)
        || color_of(&colors, arrow.id()) != Some(0xe53935)
        || color_of(&colors_0, arrow.id()) != color_of(&colors, arrow.id())
    {
        return Err("recolorir o selecionado mudou a cor errada".into());
    }
    // logo depois da amostra o foco segue na imagem: Delete apaga o retangulo, Cmd+Z o devolve (azul)
    let ret_json = json_highlight(&root_dir, &img, ret.id())?;
    let sample_focus = focus_state(view_window, cx)?;
    key(view_window, cx, "delete").await?;
    let vanished = json_highlight(&root_dir, &img, ret.id())?.is_none();
    key(view_window, cx, "cmd-z").await?;
    let came_back = json_highlight(&root_dir, &img, ret.id())? == ret_json;
    println!("smoke: depois da amostra -> foco (caixa, imagem) {sample_focus:?}; Delete apagou {vanished}; Cmd+Z devolveu azul {came_back}");
    if sample_focus != (false, true) || !vanished || !came_back {
        return Err("depois de clicar na amostra Delete/Cmd+Z deviam continuar valendo".into());
    }

    // seta selecionada para a captura (seta, retangulo, selecao e a paleta na tela)
    let (p, q) = click(in_window(view_window, cx, (0.5, 0.35))?);
    drag(view_window, cx, p, q, Modifiers::none()).await?;
    if let Some(ms) = std::env::var("MARGINALIA_SMOKE_PAUSA").ok().and_then(|s| s.parse::<u64>().ok()) {
        view_window.update(cx, |_, w, _| w.refresh()).map_err(|e| e.to_string())?;
        println!("smoke: pausa de {ms} ms para captura (seta selecionada, retangulo, paleta)");
        cx.background_executor().timer(Duration::from_millis(ms)).await;
    }

    // (7) Backspace e Cmd+Z com a caixa de nota em foco: nada nos grifos
    key(view_window, cx, "/").await?;
    key(view_window, cx, "backspace").await?;
    key(view_window, cx, "delete").await?;
    key(view_window, cx, "cmd-z").await?;
    key(view_window, cx, "escape").await?;
    let still = json_highlight(&root_dir, &img, arrow.id())?.is_some();
    let s = selection_window(view_window, cx)?;
    if !still || s != sel(arrow.id()) || focus_state(view_window, cx)? != (false, true) {
        return Err(format!("Backspace/Cmd+Z na caixa mexeram nos grifos (no JSON {still}, selecao {s:?})"));
    }

    // (8) Delete com a imagem em foco apaga; Cmd+Z devolve (mesmo id, cor, cauda) selecionado
    let before_json = json_highlight(&root_dir, &img, arrow.id())?;
    key(view_window, cx, "delete").await?;
    let erased = json_highlight(&root_dir, &img, arrow.id())?;
    let s_erased = selection_window(view_window, cx)?;
    key(view_window, cx, "cmd-z").await?;
    let returned = json_highlight(&root_dir, &img, arrow.id())?;
    let s_returned = selection_window(view_window, cx)?;
    println!("smoke: Delete -> no JSON {:?}, selecao {s_erased:?}; Cmd+Z -> {returned:?}, selecao {s_returned:?}", erased.is_some());
    if erased.is_some() || s_erased != (None, None) || returned != before_json || s_returned != sel(arrow.id()) {
        return Err("Delete/Cmd+Z nao apagaram e devolveram a seta".into());
    }
    // Backspace tambem apaga (e Cmd+Z de novo devolve)
    key(view_window, cx, "backspace").await?;
    let erased_2 = json_highlight(&root_dir, &img, arrow.id())?.is_some();
    key(view_window, cx, "cmd-z").await?;
    if erased_2 || json_highlight(&root_dir, &img, arrow.id())? != before_json {
        return Err("Backspace + Cmd+Z nao apagaram e devolveram a seta".into());
    }

    // (9) Esc com selecao tira a selecao e o foco fica na imagem; Esc de novo nao faz nada
    key(view_window, cx, "escape").await?;
    let s = selection_window(view_window, cx)?;
    key(view_window, cx, "escape").await?;
    if s != (None, None) || focus_state(view_window, cx)? != (false, true) {
        return Err(format!("Esc devia tirar a selecao e deixar o foco na imagem ({s:?})"));
    }

    // (10) "reabrir": dados do app e grifos relidos do disco; mesmas cores
    let base = view_window.update(cx, |m, _, _| m.state().app_data.base().to_path_buf()).map_err(|e| e.to_string())?;
    let disk_color = AppData::load(base).map_err(|e| e.to_string())?.highlight_color().to_string();
    let f = HighlightsFile::load(v, &img.source).map_err(|e| e.to_string())?;
    let of_disk: Vec<(String, String)> = [&ret, &arrow, &short]
        .iter()
        .map(|g| (g.id().to_string(), f.find(&img, g.id()).map(|x| x.color().to_string()).unwrap_or_default()))
        .collect();
    let before_screen = view_window.update(cx, |m, _, cx| m.viewer().read(cx).screen_colors()).map_err(|e| e.to_string())?;
    view_window.update(cx, |m, _, cx| m.refresh(cx)).map_err(|e| e.to_string())?;
    wait_lists(view_window, cx).await?;
    area_painted(view_window, cx).await?;
    let after_screen = view_window.update(cx, |m, _, cx| m.viewer().read(cx).screen_colors()).map_err(|e| e.to_string())?;
    println!("smoke: reaberto -> cor dos novos no disco {disk_color}; grifos no disco {of_disk:?}; cores na tela iguais: {}", before_screen == after_screen);
    let expected = vec![
        (ret.id().to_string(), PALETTE[3].to_string()),
        (arrow.id().to_string(), PALETTE[1].to_string()),
        (short.id().to_string(), PALETTE[1].to_string()),
    ];
    if disk_color != PALETTE[3] || of_disk != expected || before_screen != after_screen {
        return Err("as cores mudaram ao reler do disco".into());
    }

    // volta ao amarelo (o resto do smoke espera o estado de antes)
    choose_color(view_window, cx, 0).await?;
    area_painted(view_window, cx).await?;
    Ok(())
}

/// Escolhe a cor `i` pelo menuzinho da barra de ferramentas: abre pelo menu, confere que
/// abriu, clica na amostra e confere que fechou.
async fn choose_color(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, i: usize) -> Result<(), String> {
    menu_by_the(view_window, cx, MenuAction::HighlightColors).await?;
    if !view_window.update(cx, |m, _, _| m.open_colors()).map_err(|e| e.to_string())? {
        return Err("o menu de cores devia abrir".into());
    }
    view_window.update(cx, |m, _, cx| m.click_sample(i, cx)).map_err(|e| e.to_string())?;
    if view_window.update(cx, |m, _, _| m.open_colors()).map_err(|e| e.to_string())? {
        return Err("o menu de cores devia fechar ao escolher".into());
    }
    Ok(())
}

pub(super) fn highlights_window(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<Vec<Highlight>, String> {
    view_window.update(cx, |m, _, _| m.state().image_highlights().to_vec()).map_err(|e| e.to_string())
}

pub(super) fn window_ids(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<Vec<String>, String> {
    Ok(highlights_window(view_window, cx)?.iter().map(|g| g.id().to_string()).collect())
}

/// (selecionado no estado, selecionado desenhado na tela).
pub(super) fn selection_window(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(Option<String>, Option<String>), String> {
    view_window
        .update(cx, |m, _, cx| (m.state().annot.selected.clone(), m.viewer().read(cx).selected().map(str::to_string)))
        .map_err(|e| e.to_string())
}
