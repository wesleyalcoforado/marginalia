//! Cenarios do smoke: a nota rapida (digitar, autosave, multilinha e foco, flush).

use super::*;

pub(super) async fn smoke_notes(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    let root_dir = view_window
        .update(cx, |v, _, _| v.state().vault.as_ref().map(|v| v.root.clone()))
        .map_err(|e| e.to_string())?
        .ok_or("sem vault")?;
    let first = view_window
        .update(cx, |v, _, _| v.state().first_frame())
        .map_err(|e| e.to_string())?
        .ok_or("fonte sem quadros")?;
    view_window.update(cx, |v, w, cx| v.open_frame(first.clone(), w, cx)).map_err(|e| e.to_string())?;
    wait(view_window, cx).await?;
    // O vault pode ja ter coisas (outra rodada): confere so o que esta rodada criar.
    let v = Vault { root: root_dir.clone() };
    let ids_notes = |v: &Vault| -> Result<Vec<String>, String> {
        Ok(v.list_notes().map_err(|e| e.to_string())?.into_iter().map(|(p, _)| p.id).collect())
    };
    let notes_before = ids_notes(&v)?;
    let before_tasks: Vec<String> =
        v.list_tasks().map_err(|e| e.to_string())?.into_iter().map(|(t, _)| t.id).collect();
    let before_highlights = view_window.update(cx, |v, _, _| v.state().image_highlights().len()).map_err(|e| e.to_string())?;
    // zoom e posicao "do usuario"
    let fr = Framing { zoom: 2.0, cx: 0.45, cy: 0.4 };
    view_window
        .update(cx, |v, _, cx| v.viewer().update(cx, |vw, cx| vw.set_framing(fr.clone(), cx)))
        .map_err(|e| e.to_string())?;
    cx.background_executor().timer(Duration::from_millis(50)).await;

    // Shift+arrasto: retangulo de (0.35, 0.3) a (0.55, 0.5) da imagem, de baixo para cima
    let (a, b, c) = view_window
        .update(cx, |v, _, cx| {
            let vw = v.viewer().read(cx);
            let p = |x, y| {
                let q = vw.image_to_screen((x, y));
                point(px(q.x), px(q.y))
            };
            (p(0.55, 0.5), p(0.35, 0.3), p(0.6, 0.6))
        })
        .map_err(|e| e.to_string())?;
    let shift = Modifiers { shift: true, ..Modifiers::none() };
    let alt = Modifiers { alt: true, ..Modifiers::none() };
    drag(view_window, cx, a, b, shift).await?;
    // Shift+clique descuidado (2 px) e arrasto simples: nenhum grifo novo
    drag(view_window, cx, c, point(c.x + px(2.0), c.y + px(1.0)), shift).await?;
    let before_framing = view_window.update(cx, |v, _, cx| v.viewer().read(cx).framing().clone()).map_err(|e| e.to_string())?;
    drag(view_window, cx, c, point(c.x + px(40.0), c.y), Modifiers::none()).await?;
    let framing_mov = view_window.update(cx, |v, _, cx| v.viewer().read(cx).framing().clone()).map_err(|e| e.to_string())?;
    // volta ao enquadramento "do usuario" para conferir a nota
    view_window
        .update(cx, |v, _, cx| v.viewer().update(cx, |vw, cx| vw.set_framing(fr.clone(), cx)))
        .map_err(|e| e.to_string())?;
    cx.background_executor().timer(Duration::from_millis(50)).await;
    let (highlights, pending, in_screen) = view_window
        .update(cx, |v, _, cx| {
            let new_ones = v.state().image_highlights()[before_highlights..].to_vec();
            (new_ones, v.state().pending_highlight().cloned(), v.viewer().read(cx).screen_highlights() - before_highlights)
        })
        .map_err(|e| e.to_string())?;
    println!(
        "smoke: Shift+arrasto -> {} grifo(s) {:?}; Shift+clique de 2 px nao criou grifo; arrasto simples moveu cx {:.3} -> {:.3}; {in_screen} desenhado(s)",
        highlights.len(),
        highlights,
        before_framing.cx,
        framing_mov.cx
    );
    let g = match highlights.as_slice() {
        [g @ Highlight::Rectangle { x, y, w, h, .. }]
            if (x - 0.35).abs() < 0.01 && (y - 0.3).abs() < 0.01 && (w - 0.2).abs() < 0.01 && (h - 0.2).abs() < 0.01 =>
        {
            g.clone()
        }
        o => return Err(format!("esperava um retangulo ~(0.35, 0.3, 0.2, 0.2), veio {o:?}")),
    };
    if pending.as_ref() != Some(&g) || in_screen != 1 || framing_mov.cx >= before_framing.cx {
        return Err("grifo nao ficou pendente / nao foi desenhado / arrasto simples nao moveu".into());
    }
    let source = first.image.source.clone();
    let saved = HighlightsFile::load(&v, &source).map_err(|e| e.to_string())?;
    if saved.find(&first.image, g.id()).is_none() {
        return Err("grifo nao esta em grifos/<fonte>.json".into());
    }

    // Nota pelo teclado, com uma tecla morta (~ + a) entregue como o macOS entrega (setMarkedText + insertText)
    key(view_window, cx, "/").await?;
    typing(view_window, cx, "Maria da Conceiç").await?;
    view_window
        .update(cx, |v, w, cx| {
            v.quick_note().update(cx, |n, cx| {
                n.replace_and_mark_text_in_range(None, "~", None, w, cx);
                n.replace_text_in_range(None, "ã", w, cx);
            })
        })
        .map_err(|e| e.to_string())?;
    typing(view_window, cx, "o herdeira").await?;
    let text = view_window.update(cx, |v, _, cx| v.quick_note().read(cx).text().to_string()).map_err(|e| e.to_string())?;
    if text != "Maria da Conceição herdeira" {
        return Err(format!("caixa com {text:?}"));
    }
    key(view_window, cx, "cmd-enter").await?;
    wait_idle(view_window, cx).await?;
    let notes = v.list_notes().map_err(|e| e.to_string())?;
    let (p1, note_body) = notes.into_iter().find(|(p, _)| !notes_before.contains(&p.id)).ok_or("nenhuma nota criada")?;
    let ok = p1.image == first.image && p1.framing == fr && p1.highlight.as_deref() == Some(g.id()) && note_body == text;
    println!("smoke: nota {} -> imagem {:?} enquadramento {:?} grifo {:?}: {ok}", p1.id, p1.image, p1.framing, p1.highlight);
    if !ok {
        return Err(format!("nota com campos errados: {p1:?} {note_body:?}"));
    }
    if focus_state(view_window, cx)? != (false, true) {
        return Err("depois do Cmd+Enter o foco devia voltar a imagem".into());
    }
    let mirror = view_window.update(cx, |v, _, _| v.state().framing().clone()).map_err(|e| e.to_string())?;
    if mirror != fr {
        return Err(format!("AppState nao espelhou o enquadramento do visualizador: {mirror:?}"));
    }
    let conf = view_window.update(cx, |v, _, _| v.state().confirmation.clone()).map_err(|e| e.to_string())?;
    println!("smoke: barra mostra {conf:?}");

    // Tarefa: `[` na caixa e texto, nao o atalho de brilho
    let brightness = view_window.update(cx, |v, _, _| v.state().adjustments.brightness).map_err(|e| e.to_string())?;
    keyboard_quick_note(view_window, cx, "[] achar óbito do João").await?;
    let tasks = v.list_tasks().map_err(|e| e.to_string())?;
    let (t1, tcorpo): (Task, String) =
        tasks.into_iter().find(|(t, _)| !before_tasks.contains(&t.id)).ok_or("nenhuma tarefa criada")?;
    let brightness_2 = view_window.update(cx, |v, _, _| v.state().adjustments.brightness).map_err(|e| e.to_string())?;
    println!("smoke: tarefa {} -> imagem {:?} corpo {tcorpo:?}; brilho intocado: {}", t1.id, t1.image, brightness == brightness_2);
    if t1.image.as_ref() != Some(&first.image) || tcorpo != "achar óbito do João" || brightness != brightness_2 {
        return Err("tarefa errada ou `[` virou atalho".into());
    }

    // Mais duas notas em outras imagens (a 2a com um ponteiro por Option+clique)
    let mut created = vec![p1.clone()];
    for (i, text) in ["img 2: padrinho Jose Alves?", "img 3: cc. Ana, pp. 12"].iter().enumerate() {
        let moved = view_window.update(cx, |v, w, cx| v.go_next(w, cx)).map_err(|e| e.to_string())?;
        if !moved {
            break;
        }
        wait(view_window, cx).await?;
        if i == 0 {
            let p = view_window
                .update(cx, |v, _, cx| {
                    let q = v.viewer().read(cx).image_to_screen((0.7, 0.2));
                    point(px(q.x), px(q.y))
                })
                .map_err(|e| e.to_string())?;
            drag(view_window, cx, p, p, alt).await?;
        }
        keyboard_quick_note(view_window, cx, text).await?;
        let fresh = v
            .list_notes()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|(p, _)| p)
            .find(|p| !notes_before.contains(&p.id) && !created.contains(p))
            .ok_or("nota nao criada")?;
        created.push(fresh);
    }
    for p in &created {
        println!("smoke: nota criada {} em {}#{} grifo {:?}", p.id, p.image.file, p.image.page, p.highlight);
    }

    smoke_autosave(view_window, cx, &v).await?;
    smoke_highlights_screen(view_window, cx, &v).await?;
    smoke_menus(view_window, cx).await?;
    smoke_bar_menu(view_window, cx).await?;
    smoke_shortcuts_pc(view_window, cx).await?;
    smoke_rotation_highlights(view_window, cx, &v).await?;
    smoke_tools(view_window, cx, &v).await?;
    smoke_update(view_window, cx).await?;

    // Reabrir a primeira nota de outra imagem: volta a imagem, ao enquadramento e destaca o grifo
    view_window.update(cx, |v, w, cx| v.open_note(&p1.id, w, cx)).map_err(|e| e.to_string())??;
    wait(view_window, cx).await?;
    let (current, read_framing, emphasis) = view_window
        .update(cx, |v, _, cx| {
            let vw = v.viewer().read(cx);
            (v.state().current.image.clone(), vw.framing().clone(), vw.emphasis().map(str::to_string))
        })
        .map_err(|e| e.to_string())?;
    println!("smoke: open_nota({}) -> {:?} enquadramento {read_framing:?} destaque {emphasis:?}", p1.id, current.file);
    if current != p1.image || read_framing != p1.framing || emphasis.as_deref() != Some(g.id()) {
        return Err("open_nota nao restaurou o quadro".into());
    }

    // Corrida da carga: Shift+arrasto e Option+clique logo depois de navegar (imagem nova
    // ainda carregando, a tela com a anterior) nao criam grifo.
    let total = |v: &Vault| HighlightsFile::load(v, &source).map(|f| f.images.values().flatten().count()).unwrap_or(0);
    go_to_first_frame(view_window, cx).await?;
    let before = total(&v);
    let (de, to) = view_window
        .update(cx, |m, w, cx| {
            let de = m.state().current.image.clone();
            m.go_next(w, cx);
            let to = m.state().current.image.clone();
            let (a, b) = (point(px(300.0), px(200.0)), point(px(500.0), px(400.0)));
            m.viewer().update(cx, |vw, cx| {
                vw.on_down(&MouseDownEvent { button: MouseButton::Left, position: a, modifiers: shift, click_count: 1, first_mouse: false }, w, cx);
                vw.on_move(&MouseMoveEvent { position: b, pressed_button: Some(MouseButton::Left), modifiers: shift }, w, cx);
                vw.on_up(&MouseUpEvent { button: MouseButton::Left, position: b, modifiers: shift, click_count: 1 }, w, cx);
                vw.on_down(&MouseDownEvent { button: MouseButton::Left, position: a, modifiers: alt, click_count: 1, first_mouse: false }, w, cx);
            });
            (de, to)
        })
        .map_err(|e| e.to_string())?;
    if de == to {
        return Err(format!("corrida da carga: go_next nao mudou de imagem ({}): a corrida nao foi testada", de.file));
    }
    wait(view_window, cx).await?;
    let after = total(&v);
    println!("smoke: Shift+arrasto e Option+clique durante a carga: grifos {before} -> {after}");
    if after != before {
        return Err("grifo criado durante a carga".into());
    }

    // Enquadramento velho: nota enviada com o quadro seguinte ainda carregando leva o alvo
    // do quadro novo (ajustar), nao o zoom da imagem anterior que ainda esta na tela.
    let old = Framing { zoom: 5.0, cx: 0.9, cy: 0.9 };
    go_to_first_frame(view_window, cx).await?;
    let load_before = ids_notes(&v)?;
    let (de, target) = view_window
        .update(cx, |m, w, cx| {
            let de = m.state().current.image.clone();
            m.go_next(w, cx);
            m.viewer().update(cx, |vw, cx| vw.set_framing(old.clone(), cx));
            let target = m.state().current.image.clone();
            m.finish_quick_note("durante a carga", w, cx);
            (de, target)
        })
        .map_err(|e| e.to_string())?;
    if de == target {
        return Err(format!("nota durante a carga: go_next nao mudou de imagem ({}): a corrida nao foi testada", de.file));
    }
    wait(view_window, cx).await?;
    let p = v
        .list_notes()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|(p, c)| !load_before.contains(&p.id) && c == "durante a carga")
        .map(|(p, _)| p)
        .ok_or("nota durante a carga nao foi criada")?;
    println!("smoke: nota durante a carga -> {} em {} com {:?}", p.id, p.image.file, p.framing);
    if p.image != target || p.framing != viewer_math::default_framing() {
        return Err("nota enviada durante a carga levou o enquadramento velho".into());
    }

    smoke_drawers(view_window, cx, &v, &p1, &created, &t1, g.id()).await?;
    smoke_records(view_window, cx, &v).await?;
    smoke_resize_drawer(view_window, cx).await?;
    smoke_drop(view_window, cx).await?;
    smoke_source_movable(view_window, cx).await?;
    smoke_flush(view_window, cx, &v).await?;

    // Esc na imagem nao sai mais do app; Esc na caixa volta a imagem e guarda o rascunho
    view_window.update(cx, |m, w, cx| m.focus_image(w, cx)).map_err(|e| e.to_string())?;
    key(view_window, cx, "escape").await?;
    key(view_window, cx, "/").await?;
    typing(view_window, cx, "rascunho").await?;
    key(view_window, cx, "escape").await?;
    let draft = view_window.update(cx, |m, _, cx| m.quick_note().read(cx).text().to_string()).map_err(|e| e.to_string())?;
    let (in_textbox, in_image) = focus_state(view_window, cx)?;
    println!("smoke: Esc na imagem: app segue aberto; Esc na caixa: foco na imagem {in_image}, rascunho {draft:?}");
    if draft != "rascunho" || in_textbox || !in_image {
        return Err("Esc na caixa devia voltar a imagem mantendo o rascunho".into());
    }

    // Cmd-Q com o foco na caixa sai do app (e o on_app_quit apaga o cache de zips).
    // O texto digitado logo antes (dentro do debounce) vai para o disco no flush da saida.
    key(view_window, cx, "/").await?;
    typing(view_window, cx, " final").await?;
    let id = view_window.update(cx, |m, _, cx| (m.quick_note().read(cx).text().to_string(), m.idle())).map_err(|e| e.to_string())?;
    println!("smoke: Cmd-Q com a caixa em foco e {:?} ainda por gravar (ocioso {})", id.0, id.1);
    let _ = press(view_window, cx, Keystroke::parse(&crate::menus::gpui_to("cmd-q")).map_err(|e| e.to_string())?);
    // Se o Cmd-Q funcionou o processo ja terminou aqui; se nao, o smoke falha.
    cx.background_executor().timer(Duration::from_secs(1)).await;
    Err("Cmd-Q nao saiu do app".into())
}

/// Salvamento automatico pela caixa: a 1a letra cria o rascunho (id provisorio)
/// e o texto vai ao disco com debounce; 10 s sem digitar (o relogio da sessao e
/// adiantado: `tick_quick_note_in`) fixam o id uma vez so; ticks seguintes nao
/// renomeiam; digitar depois atualiza o mesmo arquivo; Cmd+Enter limpa a caixa.
pub(super) async fn smoke_autosave(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, v: &Vault) -> Result<(), String> {
    let before: Vec<String> = v.list_notes().map_err(|e| e.to_string())?.into_iter().map(|(p, _)| p.id).collect();
    let new = |v: &Vault| -> Result<Vec<(Note, String)>, String> {
        Ok(v.list_notes().map_err(|e| e.to_string())?.into_iter().filter(|(p, _)| !before.contains(&p.id)).collect())
    };
    key(view_window, cx, "/").await?;
    typing(view_window, cx, "Ana do autosave").await?;
    wait_idle(view_window, cx).await?;
    let (p, note_body) = match new(v)?.as_slice() {
        [x] => x.clone(),
        o => return Err(format!("esperava 1 rascunho, veio {o:?}")),
    };
    let mark = p.extra.get("draft").and_then(|x| x.as_bool());
    println!("smoke: autosave: digitado sem Enter -> {} rascunho {mark:?} corpo {note_body:?}", p.id);
    if !p.id.contains("-sem-texto") || mark != Some(true) || note_body != "Ana do autosave" {
        return Err("a 1a letra devia criar o rascunho e o debounce gravar o texto".into());
    }
    let after = Instant::now() + Duration::from_secs(11);
    view_window.update(cx, |m, _, cx| m.tick_quick_note_in(after, cx)).map_err(|e| e.to_string())?;
    wait_idle(view_window, cx).await?;
    let (p2, body_2) = match new(v)?.as_slice() {
        [x] => x.clone(),
        o => return Err(format!("esperava 1 nota depois de fixar, veio {o:?}")),
    };
    // mais ticks (o relogio bem adiante): o id nao muda de novo
    for s in [25, 60] {
        let t = Instant::now() + Duration::from_secs(s);
        view_window.update(cx, |m, _, cx| m.tick_quick_note_in(t, cx)).map_err(|e| e.to_string())?;
        wait_idle(view_window, cx).await?;
    }
    let ids: Vec<String> = new(v)?.into_iter().map(|(p, _)| p.id).collect();
    println!(
        "smoke: autosave: 11 s sem digitar -> {} (rascunho {:?}), {} existe: {}; ticks depois: {ids:?}",
        p2.id,
        p2.extra.get("draft"),
        p.id,
        v.root.join(format!("notas/{}.md", p.id)).exists()
    );
    // `contains`: uma rodada anterior no mesmo minuto deixa o id com sufixo (`-2`)
    if !p2.id.contains("-ana-do-autosave") || p2.extra.get("draft").is_some() || body_2 != note_body || ids != [p2.id.clone()] {
        return Err("o id devia ser fixado uma vez pelo texto".into());
    }
    typing(view_window, cx, " filha").await?;
    wait_idle(view_window, cx).await?;
    let (p3, body_3) = match new(v)?.as_slice() {
        [x] => x.clone(),
        o => return Err(format!("digitar depois de fixar criou/renomeou: {o:?}")),
    };
    println!("smoke: autosave: digitar depois de fixado -> {} corpo {body_3:?}", p3.id);
    if p3.id != p2.id || body_3 != "Ana do autosave filha" {
        return Err("digitar depois de fixado devia atualizar o mesmo arquivo".into());
    }
    key(view_window, cx, "cmd-enter").await?;
    wait_idle(view_window, cx).await?;
    let textbox = view_window.update(cx, |m, _, cx| m.quick_note().read(cx).text().to_string()).map_err(|e| e.to_string())?;
    let ids: Vec<String> = new(v)?.into_iter().map(|(p, _)| p.id).collect();
    println!("smoke: autosave: Cmd+Enter -> caixa {textbox:?}, notas novas {ids:?}");
    if !textbox.is_empty() || ids != [p2.id.clone()] || focus_state(view_window, cx)? != (false, true) {
        return Err("Cmd+Enter devia limpar a caixa sem renomear de novo".into());
    }
    smoke_multiline_and_focus(view_window, cx, v).await
}

/// Task 8: Cmd-K / Esc / Cmd-Shift-K levam o foco (e a borda azul) entre a
/// imagem e a nota; Enter quebra a linha (texto multilinha com uma composicao
/// de acento na 2a linha) e o arquivo tem as linhas; passando de 8 linhas a
/// caixa rola e o cursor fica visivel; Cmd+Enter finaliza (id fixado, caixa
/// limpa, foco na imagem); W/A/S/D movem o enquadramento no sentido certo so
/// com a imagem em foco; clicar na linha da nota carrega o texto na caixa.
pub(super) async fn smoke_multiline_and_focus(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, v: &Vault) -> Result<(), String> {
    let before: Vec<String> = v.list_notes().map_err(|e| e.to_string())?.into_iter().map(|(p, _)| p.id).collect();
    let new = |v: &Vault| -> Result<Vec<(Note, String)>, String> {
        Ok(v.list_notes().map_err(|e| e.to_string())?.into_iter().filter(|(p, _)| !before.contains(&p.id)).collect())
    };
    let textbox = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp| -> Result<String, String> {
        view_window.update(cx, |m, _, cx| m.quick_note().read(cx).text().to_string()).map_err(|e| e.to_string())
    };

    // Foco: Cmd-K -> nota; Esc -> imagem; Cmd-K e Cmd-Shift-K -> imagem; Cmd-Shift-K na imagem fica.
    view_window.update(cx, |m, w, cx| m.focus_image(w, cx)).map_err(|e| e.to_string())?;
    let mut steps = Vec::new();
    for (k, expected) in [
        ("cmd-k", (true, false)),
        ("escape", (false, true)),
        ("cmd-k", (true, false)),
        ("cmd-shift-k", (false, true)),
        ("cmd-shift-k", (false, true)),
    ] {
        key(view_window, cx, k).await?;
        let f = focus_state(view_window, cx)?;
        steps.push(format!("{k} -> nota {} imagem {}", f.0, f.1));
        if f != expected {
            return Err(format!("foco errado depois de {k}: {steps:?}"));
        }
    }
    println!("smoke: foco: {}", steps.join("; "));

    // Texto multilinha: Enter quebra a linha (nao finaliza), acento composto na 2a linha.
    key(view_window, cx, "cmd-k").await?;
    typing(view_window, cx, "Linha um").await?;
    key(view_window, cx, "enter").await?;
    typing(view_window, cx, "Conceiç").await?;
    view_window
        .update(cx, |m, w, cx| {
            m.quick_note().update(cx, |n, cx| {
                n.replace_and_mark_text_in_range(None, "~", None, w, cx);
                n.replace_text_in_range(None, "ã", w, cx);
            })
        })
        .map_err(|e| e.to_string())?;
    typing(view_window, cx, "o").await?;
    let mut expected = String::from("Linha um\nConceição");
    for i in 3..=12 {
        key(view_window, cx, "enter").await?;
        let l = format!("linha {i}");
        typing(view_window, cx, &l).await?;
        expected.push('\n');
        expected.push_str(&l);
    }
    wait_idle(view_window, cx).await?;
    let in_textbox = textbox(view_window, cx)?;
    let focused = focus_state(view_window, cx)?;
    let (first, line_cursor) = view_window
        .update(cx, |m, _, cx| {
            let n = m.quick_note().read(cx);
            (n.first_line(), n.text().lines().count() - 1)
        })
        .map_err(|e| e.to_string())?;
    let (draft, note_body) = match new(v)?.as_slice() {
        [x] => x.clone(),
        o => return Err(format!("esperava 1 rascunho multilinha, veio {o:?}")),
    };
    println!(
        "smoke: multilinha: Enter quebrou {} linhas sem finalizar (foco na nota {}); arquivo {} com {} linhas, igual a caixa: {}; \
         rolagem interna: 1a linha visivel {first}, cursor na linha {line_cursor}",
        in_textbox.lines().count(),
        focused.0,
        draft.id,
        note_body.lines().count(),
        note_body == in_textbox
    );
    if in_textbox != expected || note_body != expected || !draft.id.contains("-sem-texto") || focused != (true, false) {
        return Err(format!("texto multilinha errado: caixa {in_textbox:?}, arquivo {} {note_body:?}", draft.id));
    }
    if first == 0 || line_cursor < first || line_cursor >= first + quick_note::MAX_LINES_VISIBLE {
        return Err(format!("passando de 8 linhas a caixa devia rolar ate o cursor: 1a {first}, cursor {line_cursor}"));
    }

    // Cmd+Enter finaliza: id fixado pelo texto, caixa limpa, foco na imagem.
    key(view_window, cx, "cmd-enter").await?;
    wait_idle(view_window, cx).await?;
    let (pinned, body_2) = match new(v)?.as_slice() {
        [x] => x.clone(),
        o => return Err(format!("Cmd+Enter devia deixar 1 nota, veio {o:?}")),
    };
    let (after, conf) = (textbox(view_window, cx)?, view_window.update(cx, |m, _, _| m.state().confirmation.clone()).map_err(|e| e.to_string())?);
    println!("smoke: Cmd+Enter -> {} (rascunho {:?}), caixa {after:?}, foco {:?}, barra {conf:?}", pinned.id, pinned.extra.get("draft"), focus_state(view_window, cx)?);
    if pinned.id.contains("-sem-texto") || pinned.extra.get("draft").is_some() || body_2 != expected || !after.is_empty() || focus_state(view_window, cx)? != (false, true) {
        return Err("Cmd+Enter devia fixar o id, limpar a caixa e devolver o foco a imagem".into());
    }

    // W/A/S/D com a imagem em foco: W mostra o que esta acima (cy diminui), S abaixo, A a esquerda, D a direita.
    let framing_0 = Framing { zoom: 4.0, cx: 0.5, cy: 0.5 };
    view_window.update(cx, |m, _, cx| m.viewer().update(cx, |vw, cx| vw.set_framing(framing_0.clone(), cx))).map_err(|e| e.to_string())?;
    let fr = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp| -> Result<Framing, String> {
        view_window.update(cx, |m, _, cx| m.viewer().read(cx).framing().clone()).map_err(|e| e.to_string())
    };
    let mut wasd = Vec::new();
    for (k, f) in [
        ("w", (|a: &Framing, b: &Framing| b.cy < a.cy && b.cx == a.cx) as fn(&Framing, &Framing) -> bool),
        ("s", |a, b| b.cy > a.cy && b.cx == a.cx),
        ("a", |a, b| b.cx < a.cx && b.cy == a.cy),
        ("d", |a, b| b.cx > a.cx && b.cy == a.cy),
        ("shift-w", |a, b| a == b),
        ("alt-d", |a, b| a == b),
    ] {
        let a = fr(view_window, cx)?;
        key(view_window, cx, k).await?;
        let b = fr(view_window, cx)?;
        wasd.push(format!("{k}: ({:.4}, {:.4}) -> ({:.4}, {:.4})", a.cx, a.cy, b.cx, b.cy));
        if !f(&a, &b) {
            return Err(format!("WASD no sentido errado: {wasd:?}"));
        }
    }
    // Com a nota em foco, `w` e texto (o enquadramento fica); esvaziar a caixa apaga o rascunho.
    let a = fr(view_window, cx)?;
    key(view_window, cx, "cmd-k").await?;
    typing(view_window, cx, "w").await?;
    let b = fr(view_window, cx)?;
    let text_w = textbox(view_window, cx)?;
    key(view_window, cx, "cmd-a").await?;
    key(view_window, cx, "backspace").await?;
    key(view_window, cx, "escape").await?;
    wait_idle(view_window, cx).await?;
    let left_over_w = new(v)?.into_iter().filter(|(_, c)| c == "w").count();
    println!("smoke: WASD com a imagem em foco: {}; com a nota em foco `w` foi texto {text_w:?} e o enquadramento ficou {}; rascunho \"w\" apagado {}", wasd.join("; "), a == b, left_over_w == 0);
    if a != b || text_w != "w" || left_over_w != 0 || focus_state(view_window, cx)? != (false, true) {
        return Err("`w` com a nota em foco devia ser texto (e o rascunho sair ao esvaziar)".into());
    }

    // Gaveta com foco proprio (fix round 1): Cmd-1 abre a de notas COM o foco.
    let focus_of = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp| -> Result<Focus, String> {
        view_window.update(cx, |m, w, cx| m.current_focus(w, cx)).map_err(|e| e.to_string())
    };
    let states = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp| -> Result<Vec<String>, String> {
        view_window
            .update(cx, |m, _, _| m.drawers().lists().notes.iter().map(|p| p.id.clone()).collect())
            .map_err(|e| e.to_string())
    };
    view_window.update(cx, |m, w, cx| m.focus_image(w, cx)).map_err(|e| e.to_string())?;
    key(view_window, cx, "cmd-1").await?;
    wait_lists(view_window, cx).await?;
    let f_open = focus_of(view_window, cx)?;
    let i = view_window
        .update(cx, |m, _, _| m.drawers().notes_visible().iter().position(|p| p.id == pinned.id))
        .map_err(|e| e.to_string())?
        .ok_or("a nota multilinha nao esta na gaveta")?;
    view_window.update(cx, |m, _, _| m.drawers_mut().select(i)).map_err(|e| e.to_string())?;
    // W/A/S/D com a gaveta em foco: nada (nem descarta, nem move a imagem).
    let (e0, s0) = (fr(view_window, cx)?, states(view_window, cx)?);
    for k in ["w", "a", "s", "d"] {
        key(view_window, cx, k).await?;
    }
    wait_lists(view_window, cx).await?;
    let (e1, s1) = (fr(view_window, cx)?, states(view_window, cx)?);
    // Com a imagem em foco as teclas da gaveta nao fazem nada nela.
    key(view_window, cx, "cmd-shift-k").await?;
    let f_image = focus_of(view_window, cx)?;
    let img0 = view_window.update(cx, |m, _, _| m.state().current.image.clone()).map_err(|e| e.to_string())?;
    for k in ["backspace", "x", "cmd-backspace"] {
        key(view_window, cx, k).await?;
    }
    wait_lists(view_window, cx).await?;
    let (s2, conf_exc, img1) = view_window
        .update(cx, |m, _, _| {
            (
                m.drawers().lists().notes.iter().map(|p| p.id.clone()).collect::<Vec<_>>(),
                m.drawers().deletion_confirming().is_some(),
                m.state().current.image.clone(),
            )
        })
        .map_err(|e| e.to_string())?;
    println!(
        "smoke: gaveta: Cmd-1 -> foco {f_open:?}; WASD na gaveta: imagem parada {}, notas iguais {}; \
         Cmd-Shift-K -> {f_image:?}; Backspace/x/Cmd-Backspace com a imagem em foco: notas iguais {}, confirmacao {conf_exc}",
        e0 == e1,
        s0 == s1,
        s0 == s2
    );
    if f_open != Focus::Drawer || e0 != e1 || s0 != s1 || f_image != Focus::Image || s0 != s2 || conf_exc || img0 != img1 {
        return Err("as teclas da gaveta deviam valer so com ela em foco (e W/A/S/D nunca)".into());
    }
    // Enter na linha (gaveta em foco) = o clique: abre a imagem, leva o texto e o foco a caixa (R26).
    view_window.update(cx, |m, w, cx| m.focus_drawer(w, cx)).map_err(|e| e.to_string())?;
    // Vault lento: ate o texto chegar o foco fica na gaveta (a caixa, desligada,
    // engoliria as teclas) e a barra diz "carregando nota…" (fix round 2).
    view_window.update(cx, |m, _, _| m.delay_writes_quick_note(Some(Duration::from_millis(400)))).map_err(|e| e.to_string())?;
    key(view_window, cx, "enter").await?;
    let (f_during, loading, disabled) = view_window
        .update(cx, |m, w, cx| (m.current_focus(w, cx), m.quick_note_loading(), !m.quick_note().read(cx).enabled()))
        .map_err(|e| e.to_string())?;
    view_window.update(cx, |m, _, _| m.delay_writes_quick_note(None)).map_err(|e| e.to_string())?;
    println!("smoke: Enter na linha com o vault lento: foco {f_during:?}, carregando nota {loading}, caixa desligada {disabled}");
    if f_during != Focus::Drawer || !loading || !disabled {
        return Err("ate o texto chegar o foco devia ficar na gaveta, com a caixa desligada".into());
    }
    wait(view_window, cx).await?;
    wait_idle(view_window, cx).await?;
    let (loaded, image) =
        view_window.update(cx, |m, _, cx| (m.quick_note().read(cx).text().to_string(), m.state().current.image.clone())).map_err(|e| e.to_string())?;
    let f_line = focus_of(view_window, cx)?;
    key(view_window, cx, "cmd-1").await?;
    println!(
        "smoke: Enter na linha {} -> imagem {}, caixa com {} linhas (igual ao arquivo: {}), foco {f_line:?}",
        pinned.id,
        image.file,
        loaded.lines().count(),
        loaded == expected
    );
    if loaded != expected || image != pinned.image || f_line != Focus::QuickNote {
        return Err(format!("Enter/clique na linha devia abrir a imagem, carregar o texto e focar a caixa: {loaded:?} {f_line:?}"));
    }
    // Termina a edicao (sem mudar nada): a caixa fica vazia para o resto do smoke.
    key(view_window, cx, "cmd-k").await?;
    key(view_window, cx, "cmd-enter").await?;
    wait_idle(view_window, cx).await?;
    let (empty, ids) = (textbox(view_window, cx)?.is_empty(), new(v)?.into_iter().map(|(p, c)| (p.id, c == expected)).collect::<Vec<_>>());
    if !empty || ids != [(pinned.id.clone(), true)] {
        return Err(format!("finalizar a nota carregada mudou algo: caixa vazia {empty}, {ids:?}"));
    }

    // Quebra automatica (I2): uma linha longa com acentos ocupa varias linhas
    // VISUAIS da caixa; o arquivo guarda a linha inteira, sem quebras inseridas.
    let stretch = "Coração de mãe, pão e açúcar no café da manhã; órfão, ímã e lição ";
    key(view_window, cx, "cmd-k").await?;
    typing(view_window, cx, &stretch.repeat(4)).await?;
    view_window
        .update(cx, |m, w, cx| {
            m.quick_note().update(cx, |n, cx| {
                n.replace_and_mark_text_in_range(None, "´", None, w, cx);
                n.replace_text_in_range(None, "é", w, cx);
            })
        })
        .map_err(|e| e.to_string())?;
    let long = format!("{}é", stretch.repeat(4));
    wait_idle(view_window, cx).await?;
    view_window.update(cx, |_, w, _| w.refresh()).map_err(|e| e.to_string())?;
    cx.background_executor().timer(Duration::from_millis(80)).await;
    let (visual, logical, in_textbox) = view_window
        .update(cx, |m, _, cx| {
            let n = m.quick_note().read(cx);
            (n.visual_lines(), n.text().lines().count(), n.text().to_string())
        })
        .map_err(|e| e.to_string())?;
    let note_body = new(v)?.into_iter().find(|(p, _)| p.id != pinned.id).map(|(_, c)| c);
    println!(
        "smoke: quebra automatica: {} caracteres em {logical} linha logica -> {visual} linhas visuais; arquivo sem quebras inseridas: {}",
        long.chars().count(),
        note_body.as_deref() == Some(long.trim_end())
    );
    if in_textbox != long || logical != 1 || visual <= logical || note_body.as_deref() != Some(long.trim_end()) {
        return Err(format!("quebra automatica: caixa {in_textbox:?}, {visual} visuais, arquivo {note_body:?}"));
    }
    // Esvaziar a caixa apaga o rascunho (o resto do smoke comeca limpo).
    key(view_window, cx, "cmd-a").await?;
    key(view_window, cx, "backspace").await?;
    key(view_window, cx, "escape").await?;
    wait_idle(view_window, cx).await?;
    let left_over: Vec<String> = new(v)?.into_iter().map(|(p, _)| p.id).filter(|id| *id != pinned.id).collect();
    if !left_over.is_empty() {
        return Err(format!("o rascunho da linha longa devia sair ao esvaziar: {left_over:?}"));
    }
    Ok(())
}

/// Saida do app sem sair da caixa: (1) o `flush_quick_note` (o mesmo do
/// `on_app_quit`) grava o texto e fixa o id sem nenhum blur antes; (2) com o
/// vault lento, o flush volta no prazo, o texto fica so no registro de
/// recuperacao, e a proxima abertura (`restore_quick_note`) o leva ao vault.
pub(super) async fn smoke_flush(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, v: &Vault) -> Result<(), String> {
    let rec = view_window
        .update(cx, |m, _, _| quick_note_session::recovery_file(m.state().app_data.base()))
        .map_err(|e| e.to_string())?;
    // O vault pode ter as mesmas notas de uma rodada anterior: so contam as desta rodada.
    let old: Vec<String> = v.list_notes().map_err(|e| e.to_string())?.into_iter().map(|(p, _)| p.id).collect();
    let with_body = |v: &Vault, note_body: &str| -> Result<Vec<Note>, String> {
        Ok(v.list_notes()
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|(p, c)| c == note_body && !old.contains(&p.id))
            .map(|(p, _)| p)
            .collect())
    };
    let clear_textbox = |m: &mut MainView, cx: &mut App| m.quick_note().update(cx, |n, cx| n.clear(cx));

    // (1) sem blur: digita e sai
    key(view_window, cx, "/").await?;
    typing(view_window, cx, "nota sem blur").await?;
    let warnings = view_window.update(cx, |m, _, _| m.flush_quick_note()).map_err(|e| e.to_string())?;
    let ps = with_body(v, "nota sem blur")?;
    println!(
        "smoke: flush sem blur -> {:?} rascunho {:?}, avisos {warnings:?}, registro {}",
        ps.iter().map(|p| &p.id).collect::<Vec<_>>(),
        ps.first().map(|p| p.extra.get("draft").is_some()),
        rec.exists()
    );
    if ps.len() != 1 || !ps[0].id.contains("-nota-sem-blur") || ps[0].extra.get("draft").is_some() || !warnings.is_empty() || rec.exists() {
        return Err("o flush da saida devia gravar o texto e fixar o id sem blur".into());
    }
    view_window.update(cx, |m, _, cx| clear_textbox(m, cx)).map_err(|e| e.to_string())?;
    key(view_window, cx, "escape").await?;
    wait_idle(view_window, cx).await?;

    // (2) vault lento: o prazo estoura e o texto fica no registro
    view_window.update(cx, |m, _, _| m.delay_writes_quick_note(Some(Duration::from_secs(5)))).map_err(|e| e.to_string())?;
    key(view_window, cx, "/").await?;
    typing(view_window, cx, "nota do registro").await?;
    let start = Instant::now();
    let warnings = view_window.update(cx, |m, _, _| m.flush_quick_note_with_deadline(Duration::from_millis(500))).map_err(|e| e.to_string())?;
    let took = start.elapsed();
    let records = quick_note_session::recovery_read(&rec)?;
    let texts: Vec<&str> = records.iter().map(|r| r.text.as_str()).collect();
    println!("smoke: flush com o vault lento -> voltou em {took:?}, avisos {warnings:?}, registro {texts:?}");
    if took > Duration::from_secs(2) || warnings.is_empty() || texts != ["nota do registro"] || !with_body(v, "nota do registro")?.is_empty() {
        return Err("o flush devia voltar no prazo e deixar o texto so no registro de recuperacao".into());
    }
    // on_release (janela) e on_app_quit (app) fazem dois flushes: o segundo nao apaga o registro
    let warnings_2 = view_window.update(cx, |m, _, _| m.flush_quick_note()).map_err(|e| e.to_string())?;
    let texts_2: Vec<String> = quick_note_session::recovery_read(&rec)?.into_iter().map(|r| r.text).collect();
    println!("smoke: segundo flush depois do prazo -> avisos {warnings_2:?}, registro {texts_2:?}");
    if texts_2 != ["nota do registro"] {
        return Err("o segundo flush apagou o registro de recuperacao".into());
    }
    view_window.update(cx, |m, _, cx| clear_textbox(m, cx)).map_err(|e| e.to_string())?;
    key(view_window, cx, "escape").await?;
    view_window.update(cx, |m, _, _| m.delay_writes_quick_note(None)).map_err(|e| e.to_string())?;

    // proxima abertura (simulada): o registro volta para o vault e sai
    view_window.update(cx, |m, _, cx| m.restore_quick_note(cx)).map_err(|e| e.to_string())?;
    wait_idle(view_window, cx).await?;
    let ps = with_body(v, "nota do registro")?;
    println!(
        "smoke: restaurar o registro -> {:?}, registro ainda existe: {}",
        ps.iter().map(|p| &p.id).collect::<Vec<_>>(),
        rec.exists()
    );
    if ps.len() != 1 || !ps[0].id.contains("-nota-do-registro") || ps[0].extra.get("draft").is_some() || rec.exists() {
        return Err("o registro de recuperacao devia voltar ao vault ao abrir".into());
    }
    Ok(())
}
