//! Cenarios do smoke: menus, gavetas e filtros de notas.

use super::*;

/// Menus do macOS (Task 10): a barra montada da arvore de `menus`, e itens
/// escolhidos pela janela mudando o estado como as teclas/botoes: gaveta de
/// notas, zoom, painel de ajustes, inverter, foco na nota e na imagem, excluir e
/// desfazer grifo, excluir nota (so a confirmacao), fechar e reabrir a fonte;
/// e itens desligados quando nao se aplicam.
pub(super) async fn smoke_menus(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    let bar = cx.update(|cx| cx.get_menus()).map_err(|e| e.to_string())?.ok_or("sem barra de menus")?;
    let tree = menus::tree();
    let names: Vec<String> = bar.iter().map(|m| format!("{} ({})", m.name, m.items.len())).collect();
    println!("smoke: menus do macOS: {}", names.join(", "));
    if bar.len() != tree.len() || bar.iter().zip(&tree).any(|(m, d)| m.name.as_ref() != d.title || m.items.len() != d.items.len()) {
        return Err("a barra de menus nao e a arvore de menus::arvore".into());
    }
    view_window.update(cx, |m, w, cx| m.focus_image(w, cx)).map_err(|e| e.to_string())?;
    let enabled = item_linked;
    let no_sel = enabled(view_window, cx, MenuAction::DeleteHighlight).await?;
    let copy_img = enabled(view_window, cx, MenuAction::Copy).await?;
    let close = enabled(view_window, cx, MenuAction::CloseSource).await?;
    println!("smoke: imagem em foco, sem selecao: Excluir grifo {no_sel}, Copiar {copy_img}, Fechar fonte {close}");
    if no_sel || copy_img || !close {
        return Err("sem selecao Excluir grifo e Copiar (fora da caixa) deviam estar desligados".into());
    }

    // Exibir > Notas = Cmd-1: abre a gaveta com o foco nela; Cmd-1 fecha.
    let width = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp| {
        view_window.update(cx, |m, _, cx| m.viewer().read(cx).area().2).map_err(|e| e.to_string())
    };
    let l0 = width(view_window, cx)?;
    menu_by_the(view_window, cx, MenuAction::Notes).await?;
    // A lista de notas (deste smoke) chega do fundo.
    let start = Instant::now();
    while view_window.update(cx, |m, _, _| m.drawers().note_selected().is_none()).map_err(|e| e.to_string())? {
        if start.elapsed() > SMOKE_TIMEOUT {
            return Err("a gaveta de notas aberta pelo menu nao mostrou nenhuma linha".into());
        }
        image_frame(view_window, cx).await?;
    }
    image_frame(view_window, cx).await?;
    let (open, in_drawer, l1) = view_window
        .update(cx, |m, w, cx| (m.drawers().open(), m.current_focus(w, cx), m.viewer().read(cx).area().2))
        .map_err(|e| e.to_string())?;
    // Editar > Excluir nota/tarefa… com a gaveta em foco: a mesma confirmacao de Cmd-Backspace (Esc cancela).
    let delete = enabled(view_window, cx, MenuAction::DeleteItem).await?;
    let mut confirmed = false;
    if delete {
        menu_by_the(view_window, cx, MenuAction::DeleteItem).await?;
        confirmed = view_window.update(cx, |m, _, _| m.drawers().deletion_confirming().is_some()).map_err(|e| e.to_string())?;
        key(view_window, cx, "escape").await?;
    }
    let cancelled = view_window.update(cx, |m, _, _| m.drawers().deletion_confirming().is_none()).map_err(|e| e.to_string())?;
    // O dialogo abre com o foco em Cancelar: Enter fecha sem excluir; Tab leva o foco a Excluir.
    let mut enter_not_deletes = true;
    if delete {
        let notes = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp| {
            view_window.update(cx, |m, _, _| m.drawers().lists().notes.len()).map_err(|e| e.to_string())
        };
        let before = notes(view_window, cx)?;
        menu_by_the(view_window, cx, MenuAction::DeleteItem).await?;
        key(view_window, cx, "tab").await?;
        let no_delete = view_window
            .update(cx, |m, _, _| m.drawers().deletion_focus() == crate::drawers::DeletionButton::Delete)
            .map_err(|e| e.to_string())?;
        key(view_window, cx, "tab").await?;
        key(view_window, cx, "enter").await?;
        let just_closed = view_window.update(cx, |m, _, _| m.drawers().deletion_confirming().is_none()).map_err(|e| e.to_string())?;
        // O foco sai da gaveta (Cmd-Shift-K, como um clique na imagem): o pedido cai.
        menu_by_the(view_window, cx, MenuAction::DeleteItem).await?;
        let open = view_window.update(cx, |m, _, _| m.drawers().deletion_confirming().is_some()).map_err(|e| e.to_string())?;
        key(view_window, cx, "cmd-shift-k").await?;
        // O on_blur da gaveta roda num ciclo seguinte de efeitos: espera alguns quadros.
        let start = Instant::now();
        let after_focus = view_window.update(cx, |m, w, cx| m.current_focus(w, cx)).map_err(|e| e.to_string())?;
        let fell = loop {
            image_frame(view_window, cx).await?;
            if view_window.update(cx, |m, _, _| m.drawers().deletion_confirming().is_none()).map_err(|e| e.to_string())? {
                break true;
            }
            if start.elapsed() > Duration::from_secs(2) {
                break false;
            }
        };
        view_window.update(cx, |m, w, cx| m.focus_drawer(w, cx)).map_err(|e| e.to_string())?;
        enter_not_deletes = no_delete && just_closed && open && fell && notes(view_window, cx)? == before;
        println!(
            "smoke: confirmacao de excluir: Tab -> Excluir {no_delete}; Enter em Cancelar fechou {just_closed}; foco {after_focus:?} cancelou {fell} (aberta {open}); linhas {before} -> {}",
            notes(view_window, cx)?
        );
    }
    if !enter_not_deletes {
        return Err("confirmacao de excluir: Enter em Cancelar ou o foco fora da gaveta deviam fechar sem excluir".into());
    }
    key(view_window, cx, "cmd-1").await?;
    image_frame(view_window, cx).await?;
    let (closed, l2) = view_window.update(cx, |m, _, cx| (m.drawers().open(), m.viewer().read(cx).area().2)).map_err(|e| e.to_string())?;
    println!(
        "smoke: menu Exibir > Notas -> gaveta {open:?}, foco {in_drawer:?}, imagem {l0:.0} -> {l1:.0} px; \
         Excluir nota/tarefa… ligado {delete}, confirmacao {confirmed}, Esc cancelou {cancelled}; Cmd-1 -> {closed:?}, {l2:.0} px"
    );
    if open != Some(Drawer::Notes)
        || in_drawer != Focus::Drawer
        || (l0 - l1 - crate::drawers::WIDTH).abs() > 1.0
        || closed.is_some()
        || (l2 - l0).abs() > 1.0
    {
        return Err("Exibir > Notas devia fazer o mesmo que Cmd-1".into());
    }
    if !delete || !confirmed || !cancelled {
        return Err("Excluir nota/tarefa… devia abrir a confirmacao".into());
    }

    // Exibir > Aumentar zoom = `+`; Ajustar a janela = `0`.
    view_window.update(cx, |m, w, cx| m.focus_image(w, cx)).map_err(|e| e.to_string())?;
    let zoom = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp| {
        view_window.update(cx, |m, _, cx| m.viewer().read(cx).framing().zoom).map_err(|e| e.to_string())
    };
    let z0 = zoom(view_window, cx)?;
    key(view_window, cx, "+").await?;
    let z1 = zoom(view_window, cx)?;
    menu_by_the(view_window, cx, MenuAction::ZoomUp).await?;
    let z2 = zoom(view_window, cx)?;
    menu_by_the(view_window, cx, MenuAction::Adjust).await?;
    let adjusted = view_window.update(cx, |m, _, cx| m.viewer().read(cx).framing().clone()).map_err(|e| e.to_string())?;
    println!("smoke: zoom {z0:.3} -> `+` {z1:.3} -> menu Aumentar zoom {z2:.3}; Ajustar à janela -> {adjusted:?}");
    if !(z1 > z0 && ((z2 / z1) - (z1 / z0)).abs() < 1e-3) || adjusted != viewer_math::default_framing() {
        return Err("os itens de zoom deviam fazer o mesmo que `+` e `0`".into());
    }

    // Exibir > Ajustes de imagem = Cmd-F; Imagem > Inverter = Cmd-I. As teclas soltas antigas (`f`, `i`) nao fazem mais nada.
    menu_by_the(view_window, cx, MenuAction::AdjustmentsPanel).await?;
    let panel = view_window.update(cx, |m, _, _| m.layout().open_adjustments).map_err(|e| e.to_string())?;
    key(view_window, cx, "f").await?; // solta: nada acontece
    let panel_f = view_window.update(cx, |m, _, _| m.layout().open_adjustments).map_err(|e| e.to_string())?;
    menu_by_the(view_window, cx, MenuAction::Invert).await?;
    let inv = view_window.update(cx, |m, _, _| m.state().adjustments.invert).map_err(|e| e.to_string())?;
    key(view_window, cx, "i").await?; // solta: nada acontece
    let inv_i = view_window.update(cx, |m, _, _| m.state().adjustments.invert).map_err(|e| e.to_string())?;
    wait(view_window, cx).await?;
    println!("smoke: menu Ajustes de imagem -> painel {panel}, `f` solta -> {panel_f}; menu Inverter -> {inv}, `i` solta -> {inv_i}");
    // desfaz pelo menu o que o menu fez (as teclas soltas nao desfazem mais)
    menu_by_the(view_window, cx, MenuAction::AdjustmentsPanel).await?;
    menu_by_the(view_window, cx, MenuAction::Invert).await?;
    if !panel || !panel_f || !inv || !inv_i {
        return Err("Ajustes de imagem / Inverter so valem pelo menu (Cmd-F / Cmd-I); `f` / `i` soltas nao mudam nada".into());
    }

    // Navegar > Focar nota = Cmd-K (Copiar liga, Desfazer desliga); Focar imagem = Cmd-Shift-K.
    menu_by_the(view_window, cx, MenuAction::FocusQuickNote).await?;
    let in_quick_note = focus_state(view_window, cx)?;
    let (copy, undo) = (enabled(view_window, cx, MenuAction::Copy).await?, enabled(view_window, cx, MenuAction::Undo).await?);
    menu_by_the(view_window, cx, MenuAction::FocusImage).await?;
    let in_image = focus_state(view_window, cx)?;
    println!("smoke: menu Focar nota -> (caixa, imagem) {in_quick_note:?}, Copiar {copy}, Desfazer {undo}; Focar imagem -> {in_image:?}");
    if in_quick_note != (true, false) || !copy || undo || in_image != (false, true) {
        return Err("Focar nota / Focar imagem pelo menu deviam fazer o mesmo que Cmd-K / Cmd-Shift-K".into());
    }

    // Editar > Excluir grifo (= Delete) e Desfazer (= Cmd+Z): o mesmo grifo volta, selecionado.
    let shift = Modifiers { shift: true, ..Modifiers::none() };
    let (a, b) = (in_window(view_window, cx, (0.55, 0.15))?, in_window(view_window, cx, (0.7, 0.3))?);
    let before = window_ids(view_window, cx)?;
    drag(view_window, cx, a, b, shift).await?;
    let g = highlights_window(view_window, cx)?
        .into_iter()
        .find(|g| !before.contains(&g.id().to_string()))
        .ok_or("o retangulo do smoke de menus nao foi criado")?;
    let c = in_window(view_window, cx, (0.62, 0.22))?;
    drag(view_window, cx, c, point(c.x + px(1.0), c.y), Modifiers::none()).await?;
    menu_by_the(view_window, cx, MenuAction::DeleteHighlight).await?;
    let vanished = !window_ids(view_window, cx)?.contains(&g.id().to_string());
    let delete_after = enabled(view_window, cx, MenuAction::DeleteHighlight).await?;
    menu_by_the(view_window, cx, MenuAction::Undo).await?;
    let (came_back, sel) = (highlights_window(view_window, cx)?.contains(&g), selection_window(view_window, cx)?);
    println!("smoke: menu Excluir grifo -> sumiu {vanished} (item desligado depois: {}); Desfazer -> voltou {came_back}, selecao {sel:?}", !delete_after);
    if !vanished || delete_after || !came_back || sel.0.as_deref() != Some(g.id()) {
        return Err("Excluir grifo / Desfazer pelo menu deviam fazer o mesmo que Delete / Cmd+Z".into());
    }
    // tira o retangulo de volta (o resto do smoke conta os grifos desta imagem)
    menu_by_the(view_window, cx, MenuAction::DeleteHighlight).await?;

    // Arquivo > Fechar fonte: sem imagem, itens da imagem desligados; reabrir volta ao mesmo quadro.
    let (file_path, name, before_frame) = view_window
        .update(cx, |m, _, _| (m.state().open_source_path(), m.state().current.image.source.clone(), m.state().current.image.clone()))
        .map_err(|e| e.to_string())?;
    let file_path = file_path.ok_or("fonte aberta sem caminho")?;
    menu_by_the(view_window, cx, MenuAction::CloseSource).await?;
    wait_idle(view_window, cx).await?;
    let (has, in_screen) = view_window.update(cx, |m, _, cx| (m.state().source_has(), m.viewer().read(cx).image_has())).map_err(|e| e.to_string())?;
    let disabled = [MenuAction::CloseSource, MenuAction::Invert, MenuAction::Next, MenuAction::ZoomUp];
    let mut enabled_items = Vec::new();
    for a in disabled {
        if enabled(view_window, cx, a).await? {
            enabled_items.push(a);
        }
    }
    view_window.update(cx, |m, w, cx| m.open_path(file_path, name.clone(), ZipMode::Direct, w, cx)).map_err(|e| e.to_string())?;
    wait(view_window, cx).await?;
    let after = view_window.update(cx, |m, _, _| m.state().current.image.clone()).map_err(|e| e.to_string())?;
    println!(
        "smoke: menu Fechar fonte -> fonte {has}, imagem na tela {in_screen}, ainda ligados {enabled_items:?}; reaberta em {}#{} (antes {}#{})",
        after.file, after.page, before_frame.file, before_frame.page
    );
    if has || in_screen || !enabled_items.is_empty() || after != before_frame {
        return Err("Fechar fonte devia fechar, desligar os itens da imagem e guardar a posicao".into());
    }
    view_window.update(cx, |m, w, cx| m.focus_image(w, cx)).map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn smoke_drawers(
    view_window: WindowHandle<MainView>,
    cx: &mut AsyncApp,
    v: &Vault,
    p1: &Note,
    created: &[Note],
    t1: &Task,
    highlight: &str,
) -> Result<(), String> {
    wait_lists(view_window, cx).await?;
    let base = area_painted(view_window, cx).await?;
    let (l0, p0) = expected(view_window, cx)?;
    println!("smoke: layout {l0:?}: area da imagem {:.0} (prevista {p0:.0})", base.2);
    if l0 != layout::Layout::default() || (base.2 - p0).abs() > 1.0 {
        return Err(format!("area sem paineis {base:?} != prevista {p0}"));
    }
    // Cada gaveta abre no painel esquerdo (a area da imagem encolhe LARGURA px) e fecha (volta).
    for g in Drawer::ALL {
        // A ordem dos icones: Cmd-1 notas, Cmd-2 fontes, Cmd-3 tarefas.
        let shortcut = format!("cmd-{}", match g { Drawer::Notes => 1, Drawer::Sources => 2, Drawer::Tasks => 3 });
        key(view_window, cx, &shortcut).await?;
        let (open, width) =
            view_window.update(cx, |v, _, _| (v.drawers().open(), v.drawers().busy_width())).map_err(|e| e.to_string())?;
        let a = area_painted(view_window, cx).await?;
        let (la, pa) = expected(view_window, cx)?;
        let (ok_middle, txt) = aligned(view_window, cx, (0.5, 0.5))?;
        key(view_window, cx, &shortcut).await?;
        let closed = view_window.update(cx, |v, _, _| v.drawers().open()).map_err(|e| e.to_string())?;
        let b = area_painted(view_window, cx).await?;
        println!(
            "smoke: {shortcut} abre {:?} a esquerda: area da imagem {:.0}x{:.0} -> {:.0}x{:.0} (prevista {pa:.0}, x {:.0} -> {:.0}), imagem desenhada e {txt}: {ok_middle}; fecha: {:.0}x{:.0}",
            open, base.2, base.3, a.2, a.3, base.0, a.0, b.2, b.3
        );
        let shrank = width == drawers::WIDTH && (base.2 - a.2 - width).abs() < 2.0 && (a.3 - base.3).abs() < 1.0;
        // Painel a esquerda: a imagem comeca LARGURA px mais a direita.
        let a_left = (a.0 - base.0 - width).abs() < 2.0;
        if open != Some(g)
            || la.left != Some(g)
            || closed.is_some()
            || !shrank
            || !a_left
            || (a.2 - pa).abs() > 1.0
            || !ok_middle
            || (b.2 - base.2).abs() > 0.5
        {
            return Err(format!("gaveta {g:?}: area {base:?} -> {a:?} -> {b:?} (prevista {pa}), alinhado {ok_middle}"));
        }
    }
    // Ajustes na imagem da 1a nota (o grifo dela fica desenhado: o overlay precisa seguir alinhado).
    view_window.update(cx, |v, w, cx| v.open_note(&p1.id, w, cx)).map_err(|e| e.to_string())??;
    wait(view_window, cx).await?;
    smoke_adjustments(view_window, cx, base, highlight).await?;

    // Notas: Cmd-1, escolhe p1 pelas setas (selecao) e Enter abre a nota.
    key(view_window, cx, "cmd-1").await?;
    wait_lists(view_window, cx).await?;
    let i = view_window
        .update(cx, |v, _, _| v.drawers().notes_visible().iter().position(|p| p.id == p1.id))
        .map_err(|e| e.to_string())?
        .ok_or("p1 nao esta na gaveta de abertas")?;
    view_window.update(cx, |v, _, _| v.drawers_mut().select(i)).map_err(|e| e.to_string())?;
    key(view_window, cx, "enter").await?;
    wait(view_window, cx).await?;
    let (current, open, emphasis) = view_window
        .update(cx, |v, _, cx| {
            let e = v.state();
            (e.current.image.clone(), e.note_open().map(str::to_string), v.viewer().read(cx).emphasis().map(str::to_string))
        })
        .map_err(|e| e.to_string())?;
    // O grifo destacado, com a gaveta aberta, cai dentro da area (menor) e no lugar certo.
    let center = match view_window.update(cx, |v, _, _| v.state().image_highlights().iter().find(|g| g.id() == highlight).cloned()) {
        Ok(Some(marginalia_core::highlights::Highlight::Rectangle { x, y, w, h, .. })) => (x + w / 2.0, y + h / 2.0),
        o => return Err(format!("grifo {highlight} sumiu: {o:?}")),
    };
    let (ok_highlight, txt) = aligned(view_window, cx, center)?;
    println!("smoke: gaveta de notas, Enter na linha {i} -> {} aberta {open:?} destaque {emphasis:?}; grifo {txt}: {ok_highlight}", current.file);
    if current != p1.image || open.as_deref() != Some(p1.id.as_str()) || emphasis.as_deref() != Some(highlight) || !ok_highlight {
        return Err("Enter na gaveta nao abriu a nota".into());
    }

    // Enter = o clique (R26): o texto de p1 veio para a caixa, com o foco. Termina
    // a edicao sem mudar nada e devolve o foco a gaveta (as teclas dela so valem la).
    wait_idle(view_window, cx).await?;
    let (in_textbox, focus_enter) =
        view_window.update(cx, |m, w, cx| (m.quick_note().read(cx).text().to_string(), m.current_focus(w, cx))).map_err(|e| e.to_string())?;
    if focus_enter != Focus::QuickNote || in_textbox.is_empty() {
        return Err(format!("Enter na gaveta devia levar o texto e o foco a caixa: {in_textbox:?} {focus_enter:?}"));
    }
    key(view_window, cx, "cmd-enter").await?;
    wait_idle(view_window, cx).await?;
    view_window.update(cx, |m, w, cx| m.focus_drawer(w, cx)).map_err(|e| e.to_string())?;
    smoke_drawers_rest(view_window, cx, v, created, t1, p1).await
}

/// Filtros das notas, tarefa feita e gaveta de fontes.
pub(super) async fn smoke_drawers_rest(
    view_window: WindowHandle<MainView>,
    cx: &mut AsyncApp,
    v: &Vault,
    created: &[Note],
    t1: &Task,
    p1: &Note,
) -> Result<(), String> {
    let base = view_window.update(cx, |v, _, cx| v.viewer().read(cx).area()).map_err(|e| e.to_string())?;
    let base = (base.0, base.1, base.2 + drawers::WIDTH, base.3);
    let other = created.get(1).ok_or("faltou a 2a nota")?;
    view_window.update(cx, |m, w, cx| m.focus_drawer(w, cx)).map_err(|e| e.to_string())?;
    smoke_filters_notes(view_window, cx, v, p1).await?;
    // Tarefas: Cmd-3 troca de gaveta; x marca feita; o filtro feitas mostra; Enter abre a imagem dela
    key(view_window, cx, "cmd-3").await?;
    wait_lists(view_window, cx).await?;
    let i = view_window
        .update(cx, |v, _, _| v.drawers().visible_tasks().iter().position(|t| t.id == t1.id))
        .map_err(|e| e.to_string())?
        .ok_or("a tarefa nao esta nas abertas")?;
    view_window.update(cx, |v, _, _| v.drawers_mut().select(i)).map_err(|e| e.to_string())?;
    key(view_window, cx, "x").await?;
    wait_lists(view_window, cx).await?;
    let tmd = std::fs::read_to_string(v.root.join(format!("tarefas/{}.md", t1.id))).map_err(|e| e.to_string())?;
    let state = parse::<Task>(&tmd).map_err(|e| e.to_string())?.0.state;
    view_window
        .update(cx, |v, _, _| v.drawers_mut().set_filter_tasks(drawers::TasksFilter::Done))
        .map_err(|e| e.to_string())?;
    let i = view_window
        .update(cx, |v, _, _| v.drawers().visible_tasks().iter().position(|t| t.id == t1.id))
        .map_err(|e| e.to_string())?;
    // sai da imagem da tarefa para conferir que Enter volta a ela
    view_window.update(cx, |m, w, cx| m.go_next(w, cx)).map_err(|e| e.to_string())?;
    wait(view_window, cx).await?;
    view_window.update(cx, |v, _, _| v.drawers_mut().select(i.unwrap_or(0))).map_err(|e| e.to_string())?;
    key(view_window, cx, "enter").await?;
    wait(view_window, cx).await?;
    wait_idle(view_window, cx).await?;
    let (current, in_textbox, focus_enter) = view_window
        .update(cx, |v, w, cx| (v.state().current.image.clone(), v.quick_note().read(cx).text().to_string(), v.current_focus(w, cx)))
        .map_err(|e| e.to_string())?;
    println!(
        "smoke: x -> tarefas/{}.md state {state:?}; filtro feitas na linha {i:?}; Enter -> {}#{}, caixa {in_textbox:?}, foco {focus_enter:?}",
        t1.id, current.file, current.page
    );
    if state != StateTask::Done || i.is_none() || Some(&current) != t1.image.as_ref() || focus_enter != Focus::QuickNote || in_textbox.is_empty() {
        return Err("tarefa pela gaveta falhou".into());
    }
    // Termina a edicao da tarefa carregada sem mudar nada (a caixa fica vazia).
    key(view_window, cx, "cmd-enter").await?;
    wait_idle(view_window, cx).await?;

    // Fontes: Cmd-2 lista a fonte aberta com status ok; Esc fecha a gaveta e a imagem volta a largura toda
    key(view_window, cx, "cmd-2").await?;
    wait_lists(view_window, cx).await?;
    let sources = view_window
        .update(cx, |v, _, _| v.drawers().sources().iter().map(|f| (f.name.clone(), f.status)).collect::<Vec<_>>())
        .map_err(|e| e.to_string())?;
    key(view_window, cx, "escape").await?;
    let just_closed = view_window.update(cx, |v, _, _| v.drawers().open().is_none()).map_err(|e| e.to_string())?;
    let end = area_painted(view_window, cx).await?;
    println!("smoke: gaveta de fontes {sources:?}; Esc fecha: {just_closed}; area {:.0}x{:.0}", end.2, end.3);
    if !sources.iter().any(|(n, s)| *n == p1.image.source && *s == drawers::DrawerSourceStatus::Ok) || !just_closed || (end.2 - base.2).abs() > 0.5 {
        return Err("gaveta de fontes / Esc falhou".into());
    }
    println!(
        "smoke: arquivos: {} | {}",
        v.root.join(format!("notas/{}.md", other.id)).display(),
        v.root.join(format!("tarefas/{}.md", t1.id)).display()
    );
    Ok(())
}

/// Filtros da gaveta de notas (Cmd-1 aberta, foco na gaveta): uma nota com
/// `#etiqueta` aparece depois da recarga; a busca e digitada com eventos de
/// teclado e filtra a cada letra; o contador diz "N de M"; a etiqueta ligada
/// filtra; o x da busca e a etiqueta desligada devolvem a lista toda.
pub(super) async fn smoke_filters_notes(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, v: &Vault, p1: &Note) -> Result<(), String> {
    let current_time = chrono::Local::now().naive_local();
    v.create_note("achado no inventario #smokeetiqueta", p1.image.clone(), p1.framing.clone(), None, current_time)
        .map_err(|e| e.to_string())?;
    view_window.update(cx, |v, _, cx| v.refresh(cx)).map_err(|e| e.to_string())?;
    wait_lists(view_window, cx).await?;
    let (total, tags, counter_0) = view_window
        .update(cx, |v, _, _| (v.drawers().count_notes().1, v.drawers().vault_tags(), v.drawers().counter_notes()))
        .map_err(|e| e.to_string())?;
    if !tags.contains(&"smokeetiqueta".to_string()) || counter_0 != format!("{total} notas") {
        return Err(format!("a nota etiquetada nao chegou: {tags:?} {counter_0:?}"));
    }
    // busca digitada: sem resultado, depois so a nota etiquetada
    view_window.update(cx, |m, w, cx| m.focus_search(w, cx)).map_err(|e| e.to_string())?;
    cx.background_executor().timer(Duration::from_millis(50)).await;
    typing(view_window, cx, "zzzqq").await?;
    let (textbox, vis, counter, empty) = view_window
        .update(cx, |m, _, cx| {
            (m.text_search(cx), m.drawers().count_notes().0, m.drawers().counter_notes(), m.drawers().empty())
        })
        .map_err(|e| e.to_string())?;
    println!("smoke: busca digitada {textbox:?} -> {vis} nota(s), contador {counter:?}, vazio {empty:?}");
    if textbox != "zzzqq" || vis != 0 || counter != format!("0 de {total}") || empty.as_deref() != Some("Nenhuma nota com esses filtros") {
        return Err("a busca digitada nao filtrou".into());
    }
    view_window.update(cx, |m, _, cx| m.clear_search(cx)).map_err(|e| e.to_string())?;
    view_window.update(cx, |m, w, cx| m.focus_search(w, cx)).map_err(|e| e.to_string())?;
    cx.background_executor().timer(Duration::from_millis(50)).await;
    typing(view_window, cx, "INVENTARIO").await?;
    let (vis, counter) = view_window
        .update(cx, |m, _, _| (m.drawers().count_notes().0, m.drawers().counter_notes()))
        .map_err(|e| e.to_string())?;
    println!("smoke: busca \"INVENTARIO\" (sem diferenciar caixa) -> {vis} nota(s), contador {counter:?}");
    if vis != 1 || counter != format!("1 de {total}") {
        return Err("a busca sem diferenciar caixa falhou".into());
    }
    // Enter devolve o foco a gaveta; o x limpa; a etiqueta filtra do mesmo jeito
    key(view_window, cx, "enter").await?;
    let drawer_focus = view_window.update(cx, |m, w, cx| m.current_focus(w, cx)).map_err(|e| e.to_string())?;
    view_window.update(cx, |m, _, cx| m.clear_search(cx)).map_err(|e| e.to_string())?;
    view_window.update(cx, |m, _, _| m.drawers_mut().toggle_tag("smokeetiqueta")).map_err(|e| e.to_string())?;
    let (vis, counter, on_items) = view_window
        .update(cx, |m, _, _| (m.drawers().count_notes().0, m.drawers().counter_notes(), m.drawers().notes_filters().tags.len()))
        .map_err(|e| e.to_string())?;
    println!("smoke: Enter -> foco {drawer_focus:?}; etiqueta ligada -> {vis} nota(s), contador {counter:?}");
    if drawer_focus != Focus::Drawer || vis != 1 || on_items != 1 || counter != format!("1 de {total}") {
        return Err("a etiqueta nao filtrou".into());
    }
    view_window.update(cx, |m, _, _| m.drawers_mut().toggle_tag("smokeetiqueta")).map_err(|e| e.to_string())?;
    let counter = view_window.update(cx, |m, _, _| m.drawers().counter_notes()).map_err(|e| e.to_string())?;
    if counter != format!("{total} notas") {
        return Err(format!("desligar a etiqueta devia devolver a lista toda: {counter:?}"));
    }
    // Backspace com a gaveta em foco pede a exclusao (nada e apagado); Esc cancela.
    key(view_window, cx, "backspace").await?;
    let asked = view_window.update(cx, |m, _, _| m.drawers().deletion_confirming().map(|c| c.title())).map_err(|e| e.to_string())?;
    key(view_window, cx, "escape").await?;
    let (cancelled, after) = view_window
        .update(cx, |m, _, _| (m.drawers().deletion_confirming().is_none(), m.drawers().count_notes().1))
        .map_err(|e| e.to_string())?;
    println!("smoke: Backspace na gaveta -> {asked:?}; Esc cancela {cancelled}; notas {after}");
    if asked.as_deref() != Some("Excluir esta nota?") || !cancelled || after != total {
        return Err("Backspace na gaveta de notas devia pedir a confirmacao e Esc cancelar".into());
    }
    Ok(())
}
