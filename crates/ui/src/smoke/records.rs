//! Smoke dos registros: marca inicio e fim pela faixa (os mesmos metodos que os
//! botoes chamam), confere a imagem de fronteira compartilhada, a arvore da
//! gaveta de notas, "Mover para" e que cancelar/excluir nunca apaga notas.

use super::*;
use crate::app_state::RecordStrip;
use marginalia_core::model::Position;
use marginalia_core::vault::DocKind;

pub(super) async fn smoke_records(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, v: &Vault) -> Result<(), String> {
    go_to_first_frame(view_window, cx).await?;
    let img1 = view_window.update(cx, |m, _, _| m.state().current.image.clone()).map_err(|e| e.to_string())?;
    let records = |v: &Vault| v.list_records().map_err(|e| e.to_string());

    // 1. Marcar o inicio: a faixa abre a caixa de nome; Enter grava um registro sem fim.
    view_window.update(cx, |m, w, cx| m.start_mark_start(w, cx)).map_err(|e| e.to_string())?;
    let editing = view_window.update(cx, |m, _, _| m.record_editing().is_some()).map_err(|e| e.to_string())?;
    view_window.update(cx, |m, w, cx| m.confirm_name_record("R1", w, cx)).map_err(|e| e.to_string())?;
    let r1 = records(v)?.into_iter().next().map(|(r, _)| r).ok_or("o registro R1 nao foi gravado")?;
    let strip = view_window.update(cx, |m, _, _| m.state().record_strip()).map_err(|e| e.to_string())?;
    println!("smoke: registro R1 marcado em {} (caixa aberta: {editing}); faixa {strip:?}", img1.file);
    if !editing || r1.start != Position::de(&img1) || r1.end.is_some() || !matches!(strip, Some(RecordStrip::Em { .. })) {
        return Err("marcar o inicio do registro falhou".into());
    }

    // 2. Fim na imagem seguinte; ali mesmo comeca R2 (imagem de fronteira).
    view_window.update(cx, |m, w, cx| m.go_next(w, cx)).map_err(|e| e.to_string())?;
    wait(view_window, cx).await?;
    let img2 = view_window.update(cx, |m, _, _| m.state().current.image.clone()).map_err(|e| e.to_string())?;
    view_window.update(cx, |m, w, cx| m.record_end_marker(&r1.id, w, cx)).map_err(|e| e.to_string())?;
    view_window.update(cx, |m, w, cx| m.start_mark_start(w, cx)).map_err(|e| e.to_string())?;
    view_window.update(cx, |m, w, cx| m.confirm_name_record("R2", w, cx)).map_err(|e| e.to_string())?;
    let all = records(v)?;
    let r2 = all.iter().map(|(r, _)| r).find(|r| r.name == "R2").cloned().ok_or("R2 nao foi gravado")?;
    let r_1_read = all.iter().map(|(r, _)| r).find(|r| r.id == r1.id).cloned().ok_or("R1 sumiu")?;
    let strip = view_window.update(cx, |m, _, _| m.state().record_strip()).map_err(|e| e.to_string())?;
    println!("smoke: R1 termina em {}; R2 comeca ali; faixa {strip:?}", img2.file);
    let shared = matches!(&strip, Some(RecordStrip::Em { record, shared: true, .. }) if record.id == r1.id);
    if r_1_read.end != Some(Position::de(&img2)) || r2.start != Position::de(&img2) || !shared {
        return Err("fronteira compartilhada: R1/R2 nao ficaram como esperado".into());
    }

    // 3. Uma nota na imagem de fronteira: vai a R1; "Mover para" a leva a R2 e grava o campo.
    let note = v
        .create_note("nota na fronteira", img2.clone(), Framing { zoom: 1.0, cx: 0.5, cy: 0.5 }, None, chrono::Local::now().naive_local())
        .map_err(|e| e.to_string())?;
    view_window.update(cx, |m, _, cx| m.refresh(cx)).map_err(|e| e.to_string())?;
    wait_lists(view_window, cx).await?;
    let owner = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp, id: &str| {
        view_window
            .update(cx, |m, _, _| {
                m.drawers().lists().notes.iter().find(|p| p.id == id).and_then(|p| m.drawers().note_record(p)).map(|r| r.name.clone())
            })
            .map_err(|e| e.to_string())
    };
    let before = owner(view_window, cx, &note.id)?;
    view_window.update(cx, |m, _, cx| m.relocate_record_item(DocKind::Note, &note.id, Some(&r2.id), cx)).map_err(|e| e.to_string())?;
    wait_lists(view_window, cx).await?;
    let after = owner(view_window, cx, &note.id)?;
    let field = v.list_notes().map_err(|e| e.to_string())?.into_iter().find(|(p, _)| p.id == note.id).and_then(|(p, _)| p.record);
    println!("smoke: nota na fronteira: {before:?} -> {after:?} (campo registro: {field:?})");
    if before.as_deref() != Some("R1") || after.as_deref() != Some("R2") || field.as_deref() != Some(r2.id.as_str()) {
        return Err("mover a nota entre os registros da fronteira falhou".into());
    }

    // 4. A gaveta de notas desenha a arvore (Cmd-1) sem quebrar a area da imagem.
    key(view_window, cx, "cmd-1").await?;
    let (lines, open) = view_window
        .update(cx, |m, _, _| (m.drawers().tree_notes().len(), m.drawers().open()))
        .map_err(|e| e.to_string())?;
    area_painted(view_window, cx).await?;
    key(view_window, cx, "cmd-1").await?;
    println!("smoke: gaveta de notas aberta ({open:?}) com {lines} linhas de arvore");
    if open != Some(Drawer::Notes) || lines < 4 {
        return Err("a arvore de registros nao apareceu na gaveta de notas".into());
    }

    // 5. Cancelar (sem fim) e excluir (com fim, pede confirmacao) nunca apagam a nota.
    view_window.update(cx, |m, w, cx| m.request_delete_record(&r2.id, false, w, cx)).map_err(|e| e.to_string())?;
    view_window.update(cx, |m, w, cx| m.request_delete_record(&r1.id, true, w, cx)).map_err(|e| e.to_string())?;
    view_window.update(cx, |m, w, cx| m.delete_record(&r1.id, w, cx)).map_err(|e| e.to_string())?;
    wait_lists(view_window, cx).await?;
    let left_over = records(v)?.len();
    let notes = v.list_notes().map_err(|e| e.to_string())?.iter().filter(|(p, _)| p.id == note.id).count();
    println!("smoke: registros apagados ({left_over} restantes); a nota continua: {}", notes == 1);
    if left_over != 0 || notes != 1 {
        return Err("apagar registros mexeu nas notas".into());
    }
    Ok(())
}

/// Arrasta a borda do painel das gavetas: a largura acompanha o ponteiro (limitada), a
/// area da imagem encolhe o mesmo tanto, a largura vai para os dados do app; depois volta ao padrao.
pub(super) async fn smoke_resize_drawer(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    let base = area_painted(view_window, cx).await?;
    key(view_window, cx, "cmd-1").await?;
    let open = area_painted(view_window, cx).await?;
    // alca apertada, ponteiro em x = barra de atividades + 450, solto
    let target = layout::ACTIVITIES + 450.0;
    view_window.update(cx, |m, _, cx| m.start_drag_drawer(cx)).map_err(|e| e.to_string())?;
    view_window.update(cx, |m, _, cx| m.drag_drawer(target, true, cx)).map_err(|e| e.to_string())?;
    let wide = area_painted(view_window, cx).await?;
    view_window.update(cx, |m, _, cx| m.drop_drawer(cx)).map_err(|e| e.to_string())?;
    wait_idle(view_window, cx).await?;
    let (width, no_disk) = view_window
        .update(cx, |m, _, _| {
            let d = AppData::load(m.state().app_data.base().to_path_buf()).map(|d| d.drawers_width());
            (m.drawers().width(), d)
        })
        .map_err(|e| e.to_string())?;
    let no_disk = no_disk.map_err(|e| e.to_string())?;
    println!(
        "smoke: borda arrastada para x={target}: largura {width}, area da imagem {:.0} -> {:.0} -> {:.0}, dados do app {no_disk:?}",
        base.2, open.2, wide.2
    );
    if (width - 450.0).abs() > 0.5 || ((open.2 - wide.2) - (450.0 - drawers::WIDTH)).abs() > 2.0 || no_disk != Some(450.0) {
        return Err("redimensionar o painel das gavetas falhou".into());
    }
    // volta ao padrao e fecha a gaveta (os passos seguintes contam com LARGURA)
    view_window.update(cx, |m, _, cx| m.start_drag_drawer(cx)).map_err(|e| e.to_string())?;
    view_window.update(cx, |m, _, cx| m.drag_drawer(layout::ACTIVITIES + drawers::WIDTH, true, cx)).map_err(|e| e.to_string())?;
    view_window.update(cx, |m, _, cx| m.drop_drawer(cx)).map_err(|e| e.to_string())?;
    key(view_window, cx, "cmd-1").await?;
    wait_idle(view_window, cx).await?;
    let end = area_painted(view_window, cx).await?;
    if (end.2 - base.2).abs() > 1.0 {
        return Err(format!("a area da imagem nao voltou ({:.0} != {:.0})", end.2, base.2));
    }
    Ok(())
}
