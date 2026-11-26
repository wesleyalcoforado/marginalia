//! Cenarios do smoke: a imagem (barras, filtro em voo, janela e tile, zoom, rotacao, ajustes, redimensionar).

use super::*;

/// As barras do painel pelas mesmas funcoes que a trilha e a caixa numerica
/// chamam: valores da barra (o arrasto) e soltar, um numero digitado na caixa
/// (e um invalido), o duplo clique (neutro) e o olho (antes/depois, tambem
/// trocando de imagem). Termina com os ajustes neutros e o olho ligado.
pub(super) async fn smoke_barras(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    use crate::quick_note::Key;
    use crate::slider::Bar;
    fn e(x: impl std::fmt::Display) -> String {
        x.to_string()
    }
    // Barra: 10 movimentos seguidos do arrasto e soltar. Fica so o ultimo valor
    // (memoria, imagem na tela e appdata no disco) e o appdata e gravado no maximo
    // uma vez para a sequencia inteira (contador das gravacoes montadas).
    let writes = |m: &MainView| m.state().app_data_writes();
    wait(view_window, cx).await?;
    let before = view_window.update(cx, |m, _, _| writes(m)).map_err(e)?;
    for v in [3, 5, 9, 12, 16, 20, 25, 29, 33, 37] {
        view_window.update(cx, |m, w, cx| m.set_bar(Bar::Brightness, v, w, cx)).map_err(e)?;
    }
    let no_drag = view_window.update(cx, |m, _, _| writes(m)).map_err(e)? - before;
    view_window.update(cx, |m, w, cx| m.adjustment_dropped(w, cx)).map_err(e)?;
    wait(view_window, cx).await?;
    let (mem, disk, screen) = view_window.update(cx, |m, _, _| three_adjustments(m)).map_err(e)??;
    let total = view_window.update(cx, |m, _, _| writes(m)).map_err(e)? - before;
    println!(
        "smoke: barra de brilho, 10 passos ate 37 e soltar -> brilho {} (disco {}, na tela {:?}); appdata gravado {total}x (durante o arrasto {no_drag}x)",
        mem.brightness,
        disk.brightness,
        screen.as_ref().map(|t| t.brightness)
    );
    if (mem.brightness - 0.37).abs() > 1e-6 || disk != mem || screen.as_ref() != Some(&mem) {
        return Err("a barra nao ficou com o ultimo valor (memoria, disco e tela)".into());
    }
    if total > 1 || no_drag != 0 {
        return Err(format!("10 passos da barra gravaram o appdata {total} vezes ({no_drag} durante o arrasto); esperava no maximo 1, depois de soltar"));
    }
    // Caixa numerica: "50" + Enter no contraste (2,5 por dentro), depois um texto invalido.
    let in_textbox = |b: Bar, t: &'static str| {
        move |m: &mut MainView, w: &mut gpui::Window, cx: &mut gpui::Context<MainView>| {
            let textbox = m.numeric_textbox(b).clone();
            textbox.update(cx, |n, cx| {
                n.set_text(t, cx);
                n.edit(Key::Finish, w, cx);
            });
        }
    };
    view_window.update(cx, in_textbox(Bar::Contrast, "50")).map_err(e)?;
    wait(view_window, cx).await?;
    let (mem, disk, _) = view_window.update(cx, |m, _, _| three_adjustments(m)).map_err(e)??;
    let textbox = view_window.update(cx, |m, _, cx| m.numeric_textbox(Bar::Contrast).read(cx).text().to_string()).map_err(e)?;
    println!("smoke: caixa do contraste \"50\" + Enter -> contraste {} (disco {}), caixa {textbox:?}", mem.contrast, disk.contrast);
    if (mem.contrast - 2.5).abs() > 1e-6 || disk != mem || textbox != "+50" {
        return Err("o numero digitado na caixa nao valeu".into());
    }
    view_window.update(cx, in_textbox(Bar::Contrast, "abc")).map_err(e)?;
    wait(view_window, cx).await?;
    let (mem2, _, _) = view_window.update(cx, |m, _, _| three_adjustments(m)).map_err(e)??;
    let textbox = view_window.update(cx, |m, _, cx| m.numeric_textbox(Bar::Contrast).read(cx).text().to_string()).map_err(e)?;
    println!("smoke: caixa do contraste \"abc\" + Enter -> contraste {} (fica), caixa {textbox:?}", mem2.contrast);
    if mem2 != mem || textbox != "+50" {
        return Err("texto invalido na caixa mudou o valor".into());
    }
    // Duplo clique (marcador ou rotulo): o contraste volta ao neutro, o brilho fica.
    view_window.update(cx, |m, w, cx| m.restore_bar(Bar::Contrast, w, cx)).map_err(e)?;
    wait(view_window, cx).await?;
    let (mem, disk, _) = view_window.update(cx, |m, _, _| three_adjustments(m)).map_err(e)??;
    println!("smoke: duplo clique no contraste -> contraste {} brilho {} (disco igual: {})", mem.contrast, mem.brightness, disk == mem);
    if mem.contrast != 1.0 || (mem.brightness - 0.37).abs() > 1e-6 || disk != mem {
        return Err("o duplo clique nao voltou so o contraste ao neutro".into());
    }
    // Olho: desligado mostra a imagem sem ajustes, sem apagar nada (memoria e disco).
    view_window.update(cx, |m, w, cx| m.toggle_preview_adjustments(w, cx)).map_err(e)?;
    wait(view_window, cx).await?;
    let (shows, (mem, disk, screen)) =
        view_window.update(cx, |m, _, _| three_adjustments(m).map(|t| (m.state().show_adjustments(), t))).map_err(e)??;
    println!("smoke: olho desligado -> na tela {:?}; memoria brilho {} (disco {})", screen.as_ref().map(|t| t.is_neutral()), mem.brightness, disk.brightness);
    if shows || screen != Some(Adjustments::default()) || (mem.brightness - 0.37).abs() > 1e-6 || disk != mem {
        return Err("o olho desligado apagou os ajustes ou nao tirou da tela".into());
    }
    // Trocar de imagem com o olho desligado: a outra tambem aparece sem ajustes (nada velho na tela).
    let moved = view_window.update(cx, |m, w, cx| m.go_next(w, cx)).map_err(e)?;
    if moved {
        wait(view_window, cx).await?;
        let screen = view_window.update(cx, |m, _, _| m.state().screen_adjustments().cloned()).map_err(e)?;
        println!("smoke: olho desligado na imagem seguinte -> na tela sem ajustes: {}", screen == Some(Adjustments::default()));
        if screen != Some(Adjustments::default()) {
            return Err("com o olho desligado a imagem seguinte apareceu com ajustes".into());
        }
        view_window.update(cx, |m, w, cx| m.go_prev(w, cx)).map_err(e)?;
        wait(view_window, cx).await?;
    }
    view_window.update(cx, |m, w, cx| m.toggle_preview_adjustments(w, cx)).map_err(e)?;
    wait(view_window, cx).await?;
    let (shows, (mem, _, screen)) =
        view_window.update(cx, |m, _, _| three_adjustments(m).map(|t| (m.state().show_adjustments(), t))).map_err(e)??;
    println!("smoke: olho ligado de novo -> brilho na tela {:?}", screen.as_ref().map(|t| t.brightness));
    if !shows || screen.as_ref() != Some(&mem) {
        return Err("ligar o olho nao devolveu os ajustes a tela".into());
    }
    // Teclado de verdade na caixa do brilho (painel aberto): ↑ e Shift-↑ mudam
    // o valor; letras (w, i, ]) sao texto, nao atalhos; Esc devolve; digitar e Enter valem.
    let is_open = view_window.update(cx, |m, _, _| m.layout().open_adjustments).map_err(e)?;
    if !is_open {
        view_window.update(cx, |m, _, cx| m.toggle_adjustments(cx)).map_err(e)?;
    }
    wait_idle(view_window, cx).await?;
    view_window
        .update(cx, |m, w, cx| {
            m.numeric_textbox(Bar::Brightness).read(cx).focus_handle().focus(w);
            cx.notify();
        })
        .map_err(e)?;
    wait_idle(view_window, cx).await?;
    let in_number = view_window.update(cx, |m, w, cx| m.current_focus(w, cx) == Focus::Number).map_err(e)?;
    key(view_window, cx, "up").await?;
    key(view_window, cx, "shift-up").await?;
    wait(view_window, cx).await?;
    let arrows_after = view_window.update(cx, |m, _, _| m.state().adjustments.clone()).map_err(e)?;
    let before_framing = view_window.update(cx, |m, _, cx| m.viewer().read(cx).framing().clone()).map_err(e)?;
    typing(view_window, cx, "wi]").await?;
    wait_idle(view_window, cx).await?;
    let (letters_after, textbox, after_framing) = view_window
        .update(cx, |m, _, cx| {
            (
                m.state().adjustments.clone(),
                m.numeric_textbox(Bar::Brightness).read(cx).text().to_string(),
                m.viewer().read(cx).framing().clone(),
            )
        })
        .map_err(e)?;
    key(view_window, cx, "escape").await?;
    let (textbox_esc, focus_esc) = view_window
        .update(cx, |m, w, cx| (m.numeric_textbox(Bar::Brightness).read(cx).text().to_string(), m.current_focus(w, cx)))
        .map_err(e)?;
    view_window.update(cx, |m, w, cx| m.numeric_textbox(Bar::Brightness).read(cx).focus_handle().focus(w)).map_err(e)?;
    wait_idle(view_window, cx).await?;
    typing(view_window, cx, "-25").await?;
    key(view_window, cx, "enter").await?;
    wait(view_window, cx).await?;
    let final_ = view_window.update(cx, |m, _, _| m.state().adjustments.clone()).map_err(e)?;
    // Com o foco ainda na caixa (Enter nao o tira), Restaurar: a caixa mostra o valor novo.
    view_window.update(cx, |m, w, cx| m.command(MenuAction::Restore, w, cx)).map_err(e)?;
    wait(view_window, cx).await?;
    let (textbox_rest, focus_rest) = view_window
        .update(cx, |m, w, cx| (m.numeric_textbox(Bar::Brightness).read(cx).text().to_string(), m.current_focus(w, cx)))
        .map_err(e)?;
    println!("smoke: Restaurar com a caixa do brilho em foco ({focus_rest:?}) -> caixa {textbox_rest:?}");
    if textbox_rest != "0" {
        return Err("a caixa focada ficou com o valor velho depois de Restaurar".into());
    }
    println!(
        "smoke: teclado na caixa do brilho (foco numero {in_number}) -> setas {}; \"wi]\" na caixa {textbox:?}, brilho {} inverter {} enquadramento igual {}; Esc -> caixa {textbox_esc:?} foco {focus_esc:?}; \"-25\" + Enter -> {}",
        arrows_after.brightness,
        letters_after.brightness,
        letters_after.invert,
        before_framing == after_framing,
        final_.brightness
    );
    let near = |a: f32, b: f32| (a - b).abs() < 1e-6;
    if !in_number
        || !near(arrows_after.brightness, 0.48)
        || letters_after != arrows_after
        || textbox != "wi]"
        || before_framing != after_framing
        || textbox_esc != "+48"
        || focus_esc != Focus::Image
        || !near(final_.brightness, -0.25)
    {
        return Err("o teclado na caixa numerica nao fez o esperado".into());
    }
    // Mouse na trilha do contraste, pelos handlers que o listener da trilha e o
    // fio do arrasto chamam (com a posicao pintada de verdade): apertar a 3/4,
    // arrastar ate 1/4 (passando fora da trilha) e soltar; duplo clique no marcador.
    let (left, width) = view_window
        .update(cx, |m, _, _| m.screen_track(Bar::Contrast))
        .map_err(e)?
        .ok_or("a trilha do contraste nao foi pintada")?;
    view_window.update(cx, |m, w, cx| m.press_track(Bar::Contrast, left + 0.75 * width, 1, w, cx)).map_err(e)?;
    let pressed = view_window.update(cx, |m, _, _| m.state().adjustments.contrast).map_err(e)?;
    for f in [0.6, 0.4, 0.25] {
        view_window.update(cx, |m, w, cx| m.drag_bar(left + f * width, true, w, cx)).map_err(e)?;
    }
    view_window.update(cx, |m, w, cx| m.drop_bar(w, cx)).map_err(e)?;
    wait(view_window, cx).await?;
    let (dragged, disk, screen) = view_window.update(cx, |m, _, _| three_adjustments(m)).map_err(e)??;
    let x = left + 0.25 * width;
    view_window.update(cx, |m, w, cx| m.press_track(Bar::Contrast, x, 1, w, cx)).map_err(e)?;
    view_window.update(cx, |m, w, cx| m.drop_bar(w, cx)).map_err(e)?;
    view_window.update(cx, |m, w, cx| m.press_track(Bar::Contrast, x, 2, w, cx)).map_err(e)?;
    view_window.update(cx, |m, w, cx| m.drop_bar(w, cx)).map_err(e)?;
    wait(view_window, cx).await?;
    let double = view_window.update(cx, |m, _, _| m.state().adjustments.contrast).map_err(e)?;
    println!(
        "smoke: trilha do contraste ({width:.0} px) -> apertar a 3/4: {pressed}; arrastar ate 1/4 e soltar: {} (disco {}, na tela {:?}); duplo clique no marcador: {double}",
        dragged.contrast,
        disk.contrast,
        screen.as_ref().map(|t| t.contrast)
    );
    if !near(pressed, 2.5) || !near(dragged.contrast, 0.5) || disk != dragged || screen.as_ref() != Some(&dragged) || double != 1.0 {
        return Err("o mouse na trilha nao fez o esperado".into());
    }
    // Arrasto que atravessa a troca de imagem pelo teclado: apertar a trilha numa
    // imagem, ir a seguinte (seta) e mover o mouse. O movimento nao vale na
    // imagem nova; o valor do aperto fica na imagem em que foi escolhido.
    view_window.update(cx, |m, w, cx| m.press_track(Bar::Contrast, left + 0.75 * width, 1, w, cx)).map_err(e)?;
    let (img1, a1) = view_window.update(cx, |m, _, _| (m.state().current.image.clone(), m.state().adjustments.clone())).map_err(e)?;
    let moved = view_window.update(cx, |m, w, cx| m.go_next(w, cx)).map_err(e)?;
    if moved {
        wait(view_window, cx).await?;
        let (img2, before_2) = view_window.update(cx, |m, _, _| (m.state().current.image.clone(), m.state().adjustments.clone())).map_err(e)?;
        for f in [0.5, 0.25] {
            view_window.update(cx, |m, w, cx| m.drag_bar(left + f * width, true, w, cx)).map_err(e)?;
        }
        view_window.update(cx, |m, w, cx| m.drop_bar(w, cx)).map_err(e)?;
        wait(view_window, cx).await?;
        let (after_2, d1, d2) = view_window
            .update(cx, |m, _, _| {
                let e = m.state();
                let d = AppData::load(e.app_data.base().to_path_buf()).ok();
                (e.adjustments.clone(), d.as_ref().map(|d| d.adjustments(&img1)), d.as_ref().map(|d| d.adjustments(&img2)))
            })
            .map_err(e)?;
        println!(
            "smoke: arrasto do contraste atravessando a troca de imagem -> {} contraste {} (disco {:?}); {} contraste {} -> {} (disco {:?})",
            img1.file,
            a1.contrast,
            d1.as_ref().map(|a| a.contrast),
            img2.file,
            before_2.contrast,
            after_2.contrast,
            d2.as_ref().map(|a| a.contrast)
        );
        if img1 == img2 || after_2 != before_2 || d2.as_ref() != Some(&before_2) || !near(a1.contrast, 2.5) || d1.as_ref() != Some(&a1) {
            return Err("o arrasto de uma barra atravessou a troca de imagem".into());
        }
        view_window.update(cx, |m, w, cx| m.go_prev(w, cx)).map_err(e)?;
        wait(view_window, cx).await?;
        let restore = Adjustments { contrast: 1.0, ..a1 };
        view_window.update(cx, |m, w, cx| m.set_adjustments(restore, w, cx)).map_err(e)?;
        wait(view_window, cx).await?;
    } else {
        view_window.update(cx, |m, w, cx| m.drop_bar(w, cx)).map_err(e)?;
        wait(view_window, cx).await?;
    }
    smoke_interrupt_final(view_window, cx).await?;
    if !is_open {
        view_window.update(cx, |m, _, cx| m.toggle_adjustments(cx)).map_err(e)?;
    }
    view_window.update(cx, |m, w, cx| m.set_adjustments(Adjustments::default(), w, cx)).map_err(e)?;
    wait(view_window, cx).await?;
    Ok(())
}

/// Um valor novo com o tile em voo NAO o interrompe (regra R2: o arrasto nao
/// congela): pede o tile de A (nitidez + passa-alta), e no instante em que ele
/// esta em voo (resultado ainda nao recebido: ele so e aceito na thread da
/// interface, que o smoke segura durante o `update`) muda para B e solta. Na tela
/// entram so o tile de A e depois o de B (nada de A depois de B); B fica na tela
/// e no disco. Numa fonte pequena o tile pode terminar entre duas olhadas: tenta
/// de novo com outro valor de A. Roda numa janela de 1/4 da pagina (o tile de A,
/// que agora termina, custa um quarto: num build de depuracao a pagina inteira de
/// 4000x3000 com nitidez e passa-alta leva ~11 s por tile); no fim volta ao
/// neutro e a janela da pagina inteira.
pub(super) async fn smoke_interrupt_final(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    let e = |x: gpui::Result<()>| x.map_err(|e| e.to_string());
    wait(view_window, cx).await?;
    // Zoom 2 no meio: o visualizador informa a janela de 1/4 da pagina.
    e(view_window.update(cx, |m, _, cx| {
        m.viewer().update(cx, |v, cx| v.set_framing(Framing { zoom: 2.0, cx: 0.5, cy: 0.5 }, cx))
    }))?;
    wait(view_window, cx).await?;
    let r = smoke_tile_in_flight(view_window, cx).await;
    e(view_window.update(cx, |m, w, cx| m.set_adjustments(Adjustments::default(), w, cx)))?;
    e(view_window.update(cx, |m, _, cx| m.viewer().update(cx, |v, cx| v.zoom_by(MenuAction::Adjust, cx))))?;
    wait(view_window, cx).await?;
    r
}

pub(super) async fn smoke_tile_in_flight(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    fn e(x: impl std::fmt::Display) -> String {
        x.to_string()
    }
    let b =Adjustments { sharpen: 0.6, highpass: 0.7, brightness: 0.2, ..Adjustments::default() };
    for attempt in 1..=50u32 {
        let a = Adjustments { sharpen: 0.5 + attempt as f32 / 1000.0, highpass: 0.7, ..Adjustments::default() };
        wait(view_window, cx).await?;
        view_window.update(cx, |m, w, cx| m.set_adjustments(a.clone(), w, cx)).map_err(e)?;
        let start = Instant::now();
        let mut caught = None;
        while caught.is_none() && start.elapsed() < SMOKE_TIMEOUT {
            let (found, idle) = view_window
                .update(cx, |m, w, cx| {
                    if !m.tile_in_flight() {
                        return (None, m.idle());
                    }
                    let since = m.state().displays().1;
                    m.adjustment_changed(b.clone(), w, cx);
                    m.adjustment_dropped(w, cx);
                    (Some(since), false)
                })
                .map_err(e)?;
            if found.is_some() {
                caught = found;
            } else if idle {
                break;
            } else {
                cx.background_executor().timer(Duration::from_micros(200)).await;
            }
        }
        let Some(since) = caught else { continue };
        wait(view_window, cx).await?;
        let (displayed, (mem, disk, screen)) = view_window
            .update(cx, |m, _, _| {
                let (list, _) = m.state().displays();
                let after: Vec<_> = list.iter().filter(|x| x.number > since).cloned().collect();
                three_adjustments(m).map(|t| (after, t))
            })
            .map_err(e)??;
        // R2: o tile de A em voo NAO e interrompido pelo valor novo: ele sempre
        // chega (mostra A) e depois vem o de B. Nunca outro valor, nunca A depois de B.
        let tiles: Vec<_> = displayed.iter().filter(|x| x.quality == crate::load::Quality::Tile).map(|x| x.image_adjustments.clone()).collect();
        let order_ok = tiles == vec![a.clone(), b.clone()];
        println!(
            "smoke: valor novo com o tile em voo (tentativa {attempt}) -> tile de A nao interrompido: {}; depois: {:?}; na tela brilho {:?} (disco {})",
            tiles.first() == Some(&a),
            displayed.iter().map(|x| (x.quality, x.image_adjustments.brightness)).collect::<Vec<_>>(),
            screen.as_ref().map(|t| t.brightness),
            disk.brightness
        );
        if !order_ok || mem != b || disk != b || screen.as_ref() != Some(&b) {
            return Err(format!("tiles depois do valor novo {tiles:?}: esperava [A, B] (o tile de A em voo sempre chega), e B na tela e no disco"));
        }
        return Ok(());
    }
    Err("nao consegui pegar uma final em voo em 50 tentativas".into())
}

/// As camadas do visualizador sao as do estado (mesmo tamanho, mesma janela do
/// tile; nunca a cheia junto com base ou tile). Vale a qualquer instante: o
/// estado aceita e o visualizador troca a textura no mesmo `receive`.
pub(super) fn layers_mirrored(m: &MainView, cx: &gpui::App) -> Result<(), String> {
    let (full, base, tile) = m.viewer().read(cx).screen_layers();
    let st = m.state();
    let (is_base, is_tile) = (st.base_display().map(|c| c.sz), st.tile_display().map(|(c, j)| (c.sz, j)));
    if base != is_base || tile != is_tile || (full.is_some() && (base.is_some() || tile.is_some())) {
        return Err(format!("camadas do visualizador {:?} != estado {:?}", (full, base, tile), (is_base, is_tile)));
    }
    Ok(())
}

/// Depois de `wait`, com o ajuste `a` ativo: a janela que o visualizador
/// avisou e a calculada do enquadramento atual (pixels fisicos da tela), o
/// estado guarda a mesma, o tile na tela e o dela com exatamente o tamanho
/// dela, a base dos mesmos valores fica por baixo e nada sobra da cheia.
pub(super) fn check_window(m: &MainView, w: &gpui::Window, cx: &gpui::App, a: &Adjustments, label: &str) -> Result<String, String> {
    layers_mirrored(m, cx)?;
    let vw = m.viewer().read(cx);
    let st = m.state();
    let sz = st.page_sz().ok_or("sem pagina")?;
    let (_, _, aw, ah) = vw.area();
    // O enquadramento publico esta nas coordenadas da imagem; a conta da janela e no espaco
    // de exibicao (a pagina girada), de onde a janela volta para a grade da pagina sem girar.
    let rot = vw.rotation();
    let fr = vw.framing();
    let (dcx, dcy) = rot.display_to((fr.cx, fr.cy));
    let display_framing = Framing { zoom: fr.zoom, cx: dcx, cy: dcy };
    let expected = crate::viewer_math::window_visible_rotated(&display_framing, (aw, ah), (sz.0 as f32, sz.1 as f32), rot, w.scale_factor())
        .ok_or("area sem tamanho")?;
    let (full, base, tile) = vw.screen_layers();
    let msg = format!(
        "{label}: zoom {:.2} centro ({:.3}, {:.3}) fator {} rotacao {} -> janela {:?} na grade {:?}; tile {:?}; base {:?}",
        fr.zoom,
        fr.cx,
        fr.cy,
        w.scale_factor(),
        rot.degrees(),
        expected.region,
        expected.grid,
        tile.map(|t| t.0),
        base
    );
    if vw.window_by_inform(w).is_some() || vw.given_window() != Some(expected) || st.view_window() != Some(expected) {
        return Err(format!("{msg}: janela avisada {:?}, no estado {:?}", vw.given_window(), st.view_window()));
    }
    if full.is_some() || base.is_none() || tile != Some((rot.display_sz(expected.size_px()), expected)) || st.screen_adjustments() != Some(a) {
        return Err(format!("{msg}: camadas erradas (cheia {full:?}) ou ajuste na tela {:?}", st.screen_adjustments()));
    }
    Ok(msg)
}

/// O visualizador informa a janela visivel e desenha base + tile (com ajuste
/// ativo): no zoom 1, num pan de 20 passos, num zoom pelas teclas e num pan logo
/// depois de mudar o valor. Em cada passo as camadas do visualizador sao as do
/// estado; no fim de cada um a janela avisada e a do enquadramento, o tile na
/// tela e o dela (exatamente o tamanho dela) e a base dos mesmos valores esta
/// por baixo; nenhum tile do valor velho depois do novo; nada gravado no pan.
/// No fim volta a ajustar a janela e ao neutro (so a cheia).
pub(super) async fn smoke_window(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    use crate::load::Quality;
    fn e(x: impl std::fmt::Display) -> String {
        x.to_string()
    }
    let fr = |zoom: f32, cx: f32, cy: f32| Framing { zoom, cx, cy };
    let a = Adjustments { sharpen: 0.4, highpass: 0.3, ..Adjustments::default() };
    view_window.update(cx, |m, w, cx| m.set_adjustments(a.clone(), w, cx)).map_err(e)?;
    wait(view_window, cx).await?;
    println!("smoke: {}", view_window.update(cx, |m, w, cx| check_window(m, w, cx, &a, "janela no zoom 1")).map_err(e)??);

    // Pan continuo no zoom 2: um aviso por quadro, a agenda junta a rajada.
    let (written, since) = view_window.update(cx, |m, _, _| (m.state().app_data_writes(), m.state().displays().1)).map_err(e)?;
    let mut warned = Vec::new();
    for i in 0..20u32 {
        let target = fr(2.0, 0.3 + 0.02 * i as f32, 0.45);
        view_window.update(cx, |m, _, cx| m.viewer().update(cx, |v, cx| v.set_framing(target, cx))).map_err(e)?;
        await_frame(view_window, cx).await?;
        let j = view_window.update(cx, |m, _, cx| layers_mirrored(m, cx).map(|_| m.viewer().read(cx).given_window())).map_err(e)??;
        if warned.last() != Some(&j) {
            warned.push(j);
        }
    }
    wait(view_window, cx).await?;
    let msg = view_window.update(cx, |m, w, cx| check_window(m, w, cx, &a, "pan de 20 passos no zoom 2")).map_err(e)??;
    let (tiles, written_after) = view_window
        .update(cx, |m, _, _| {
            let st = m.state();
            (st.displays().0.iter().filter(|x| x.number > since && x.quality == Quality::Tile).count(), st.app_data_writes())
        })
        .map_err(e)?;
    println!("smoke: {msg}; {} janelas avisadas, {tiles} tile(s) na tela; gravacoes {}", warned.len(), written_after - written);
    if tiles == 0 || tiles > warned.len() || written_after != written {
        return Err(format!("pan: {tiles} tiles para {} janelas, {} gravacoes", warned.len(), written_after - written));
    }

    // Zoom pelas teclas (+ tres vezes, em torno do centro).
    for _ in 0..3 {
        view_window.update(cx, |m, _, cx| m.viewer().update(cx, |v, cx| v.zoom_by(MenuAction::ZoomUp, cx))).map_err(e)?;
        await_frame(view_window, cx).await?;
        view_window.update(cx, |m, _, cx| layers_mirrored(m, cx)).map_err(e)??;
    }
    wait(view_window, cx).await?;
    println!("smoke: {}", view_window.update(cx, |m, w, cx| check_window(m, w, cx, &a, "zoom + tres vezes")).map_err(e)??);

    // Valor novo e pan logo em seguida: o que aparece nunca mistura os valores
    // (camadas espelhadas a cada passo) e no fim tudo e do valor novo.
    let b = Adjustments { sharpen: 0.7, highpass: 0.1, brightness: 0.1, ..Adjustments::default() };
    let since = view_window.update(cx, |m, _, _| m.state().displays().1).map_err(e)?;
    view_window.update(cx, |m, w, cx| m.set_adjustments(b.clone(), w, cx)).map_err(e)?;
    for i in 0..10u32 {
        let target = fr(3.0, 0.7 - 0.03 * i as f32, 0.6);
        view_window.update(cx, |m, _, cx| m.viewer().update(cx, |v, cx| v.set_framing(target, cx))).map_err(e)?;
        await_frame(view_window, cx).await?;
        view_window.update(cx, |m, _, cx| layers_mirrored(m, cx)).map_err(e)??;
    }
    wait(view_window, cx).await?;
    let msg = view_window.update(cx, |m, w, cx| check_window(m, w, cx, &b, "valor novo e pan logo depois")).map_err(e)??;
    let tiles: Vec<bool> = view_window
        .update(cx, |m, _, _| {
            m.state().displays().0.iter().filter(|x| x.number > since && x.quality == Quality::Tile).map(|x| x.image_adjustments == b).collect()
        })
        .map_err(e)?;
    println!("smoke: {msg}; tiles depois do valor novo (true = novo): {tiles:?}");
    if tiles.last() != Some(&true) || tiles.windows(2).any(|w| w[0] && !w[1]) {
        return Err(format!("tile do valor velho depois do novo: {tiles:?}"));
    }

    // De volta: ajustar a janela e o neutro (so a cheia, sem camadas).
    view_window.update(cx, |m, _, cx| m.viewer().update(cx, |v, cx| v.zoom_by(MenuAction::Adjust, cx))).map_err(e)?;
    wait(view_window, cx).await?;
    println!("smoke: {}", view_window.update(cx, |m, w, cx| check_window(m, w, cx, &b, "ajustar a janela")).map_err(e)??);
    view_window.update(cx, |m, w, cx| m.set_adjustments(Adjustments::default(), w, cx)).map_err(e)?;
    wait(view_window, cx).await?;
    let layers = view_window.update(cx, |m, _, cx| layers_mirrored(m, cx).map(|_| m.viewer().read(cx).screen_layers())).map_err(e)??;
    println!("smoke: neutro -> camadas (cheia, base, tile) {layers:?}");
    if layers.0.is_none() || layers.1.is_some() || layers.2.is_some() {
        return Err("sem ajuste devia sobrar so a textura cheia".into());
    }
    Ok(())
}

/// [`check_window`] e mais: a base na tela e do valor `a` (nada de base de
/// outro valor por baixo) e as camadas do visualizador sao da imagem atual.
pub(super) fn check_value(m: &MainView, w: &gpui::Window, cx: &gpui::App, a: &Adjustments, label: &str) -> Result<String, String> {
    let msg = check_window(m, w, cx, a, label)?;
    let st = m.state();
    if st.base_adjustments() != Some(a) {
        return Err(format!("{msg}: base de outro valor na tela ({:?})", st.base_adjustments()));
    }
    if m.viewer().read(cx).screen_image() != Some(&st.current.image) {
        return Err(format!("{msg}: camadas de outra imagem ({:?})", m.viewer().read(cx).screen_image()));
    }
    Ok(msg)
}

/// Dez valores de uma barra (nitidez, com passa-alta ligada) com o zoom
/// afastado (pagina inteira) e de novo com o zoom aproximado, numa imagem
/// recem-aberta; depois pan e zoom. No fim de cada fase: a janela avisada e a do
/// enquadramento, o tile na tela e o dela com o tamanho exato, tile e base so do
/// ultimo valor e da imagem atual, nenhum tile de um valor anterior depois de um
/// mais novo, e o appdata gravado uma vez por sequencia (nenhuma no pan/zoom).
pub(super) async fn smoke_values_with_zoom(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    use crate::load::Quality;
    use crate::slider::Bar;
    fn e(x: impl std::fmt::Display) -> String {
        x.to_string()
    }
    let moved = view_window.update(cx, |m, w, cx| m.go_next(w, cx)).map_err(e)?;
    wait(view_window, cx).await?;
    let with_error = view_window.update(cx, |m, _, _| m.state().error.is_some()).map_err(e)?;
    if with_error {
        println!("smoke: imagem seguinte com erro: valores com zoom nao exercitados");
        if moved {
            view_window.update(cx, |m, w, cx| m.go_prev(w, cx)).map_err(e)?;
            wait(view_window, cx).await?;
        }
        return Ok(());
    }
    let base = Adjustments { highpass: 0.3, ..view_window.update(cx, |m, _, _| m.state().adjustments.clone()).map_err(e)? };
    view_window.update(cx, |m, w, cx| m.set_adjustments(base.clone(), w, cx)).map_err(e)?;
    wait(view_window, cx).await?;
    let phases: [(&str, Framing, [i32; 10]); 2] = [
        ("zoom afastado", Framing { zoom: 1.0, cx: 0.5, cy: 0.5 }, [10, 14, 19, 23, 28, 32, 37, 41, 46, 50]),
        ("zoom aproximado", Framing { zoom: 4.0, cx: 0.35, cy: 0.6 }, [47, 43, 38, 34, 29, 25, 20, 16, 11, 7]),
    ];
    let mut last = base.clone();
    for (label, fr, steps) in phases {
        view_window.update(cx, |m, _, cx| m.viewer().update(cx, |v, cx| v.set_framing(fr.clone(), cx))).map_err(e)?;
        wait(view_window, cx).await?;
        let (before, since) = view_window.update(cx, |m, _, _| (m.state().app_data_writes(), m.state().displays().1)).map_err(e)?;
        let values: Vec<Adjustments> = steps.iter().map(|v| Bar::Sharpness.apply(&last, *v)).collect();
        for v in steps {
            view_window.update(cx, |m, w, cx| m.set_bar(Bar::Sharpness, v, w, cx)).map_err(e)?;
            await_frame(view_window, cx).await?;
            view_window.update(cx, |m, _, cx| layers_mirrored(m, cx)).map_err(e)??;
        }
        let no_drag = view_window.update(cx, |m, _, _| m.state().app_data_writes()).map_err(e)? - before;
        view_window.update(cx, |m, w, cx| m.adjustment_dropped(w, cx)).map_err(e)?;
        wait(view_window, cx).await?;
        last = values[9].clone();
        let msg = view_window.update(cx, |m, w, cx| check_value(m, w, cx, &last, label)).map_err(e)??;
        let (order, written, disk) = view_window
            .update(cx, |m, _, _| {
                let st = m.state();
                let order: Vec<Option<usize>> = st
                    .displays()
                    .0
                    .iter()
                    .filter(|x| x.number > since && x.quality == Quality::Tile)
                    .map(|x| values.iter().position(|v| *v == x.image_adjustments))
                    .collect();
                (order, st.app_data_writes() - before, three_adjustments(m).map(|t| t.1))
            })
            .map_err(e)?;
        let disk = disk?;
        println!(
            "smoke: 10 valores da nitidez no {msg}; tiles na tela (passo do valor): {order:?}; appdata gravado {written}x (no arrasto {no_drag}x), disco nitidez {}",
            disk.sharpen
        );
        if order.last() != Some(&Some(9)) || order.iter().any(|p| p.is_none()) || order.windows(2).any(|w| w[0] > w[1]) {
            return Err(format!("{label}: tiles fora de ordem ou de outro valor: {order:?}"));
        }
        if written != 1 || no_drag != 0 || disk != last {
            return Err(format!("{label}: appdata gravado {written}x ({no_drag}x no arrasto), disco {disk:?}"));
        }
    }
    // Pan e zoom com o ultimo valor: nada gravado, tudo do valor e da imagem atuais.
    let before = view_window.update(cx, |m, _, _| m.state().app_data_writes()).map_err(e)?;
    for i in 0..10u32 {
        let target = Framing { zoom: 4.0, cx: 0.35 + 0.03 * i as f32, cy: 0.6 - 0.02 * i as f32 };
        view_window.update(cx, |m, _, cx| m.viewer().update(cx, |v, cx| v.set_framing(target, cx))).map_err(e)?;
        await_frame(view_window, cx).await?;
        view_window.update(cx, |m, _, cx| layers_mirrored(m, cx)).map_err(e)??;
    }
    for z in [MenuAction::ZoomDown, MenuAction::ZoomUp, MenuAction::ZoomUp] {
        view_window.update(cx, |m, _, cx| m.viewer().update(cx, |v, cx| v.zoom_by(z, cx))).map_err(e)?;
        await_frame(view_window, cx).await?;
        view_window.update(cx, |m, _, cx| layers_mirrored(m, cx)).map_err(e)??;
    }
    wait(view_window, cx).await?;
    let msg = view_window.update(cx, |m, w, cx| check_value(m, w, cx, &last, "pan e zoom depois dos valores")).map_err(e)??;
    let written = view_window.update(cx, |m, _, _| m.state().app_data_writes()).map_err(e)? - before;
    println!("smoke: {msg}; appdata gravado {written}x");
    if written != 0 {
        return Err(format!("pan/zoom gravaram o appdata {written}x"));
    }
    // De volta: ajustar, o neutro nesta imagem e a imagem anterior.
    view_window.update(cx, |m, _, cx| m.viewer().update(cx, |v, cx| v.zoom_by(MenuAction::Adjust, cx))).map_err(e)?;
    view_window.update(cx, |m, w, cx| m.set_adjustments(Adjustments::default(), w, cx)).map_err(e)?;
    wait(view_window, cx).await?;
    if moved {
        view_window.update(cx, |m, w, cx| m.go_prev(w, cx)).map_err(e)?;
        wait(view_window, cx).await?;
    }
    Ok(())
}

/// Rotacao (Cmd-L/Cmd-R, pelo menu como o macOS entrega): quatro giros para a direita sem ajuste
/// e depois com ajuste (nitidez e passa-alta). A cada giro: a rotacao do visualizador, do estado
/// e dos dados do app; a textura na tela com os lados trocados em 90/270 graus; o enquadramento
/// (nas coordenadas da imagem) igual, isto e, o ponto do centro continua no centro; a ida e volta
/// janela <-> imagem; e, com ajuste, a janela avisada, a do estado e o tile e a base nos
/// tamanhos certos (`check_value`). No fim a rotacao volta a 0 e sai dos dados do app.
pub(super) async fn smoke_rotation(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    use marginalia_core::rotation::Rotation;
    fn e(x: impl std::fmt::Display) -> String {
        x.to_string()
    }
    view_window.update(cx, |m, w, cx| m.focus_image(w, cx)).map_err(e)?;
    view_window.update(cx, |m, w, cx| m.set_adjustments(Adjustments::default(), w, cx)).map_err(e)?;
    wait(view_window, cx).await?;
    let (image, sz, base) = view_window
        .update(cx, |m, _, _| (m.state().current.image.clone(), m.state().page_sz(), m.state().app_data.base().to_path_buf()))
        .map_err(e)?;
    let sz = sz.ok_or("sem pagina")?;
    // Fora do centro, para o ponto do centro ser conferivel (nas coordenadas da imagem).
    let fr = Framing { zoom: 3.0, cx: 0.3, cy: 0.6 };
    view_window.update(cx, |m, _, cx| m.viewer().update(cx, |v, cx| v.set_framing(fr.clone(), cx))).map_err(e)?;
    area_painted(view_window, cx).await?;

    let a = Adjustments { sharpen: 0.4, highpass: 0.3, ..Adjustments::default() };
    for (phase, adjustment) in [("sem ajuste", None), ("com ajuste", Some(&a))] {
        if let Some(a) = adjustment {
            view_window.update(cx, |m, w, cx| m.set_adjustments(a.clone(), w, cx)).map_err(e)?;
            wait(view_window, cx).await?;
        }
        let mut rot = Rotation::R0;
        for _ in 0..4 {
            menu_by_the(view_window, cx, MenuAction::RotateRight).await?;
            wait(view_window, cx).await?;
            rot = rot.clockwise();
            let (vrot, srot, was_read, full, stored) = view_window
                .update(cx, |m, _, cx| {
                    let vw = m.viewer().read(cx);
                    let st = m.state();
                    (vw.rotation(), st.rotation(), vw.framing(), vw.screen_layers().0, st.app_data.rotation(&st.current.image))
                })
                .map_err(e)?;
            println!("smoke: rotacao {phase}: {}° -> visualizador {vrot:?}, estado {srot:?}, dados do app {stored:?}; enquadramento {was_read:?}; cheia {full:?}", rot.degrees());
            if vrot != rot || srot != rot || stored != rot {
                return Err(format!("rotacao {phase}: esperava {rot:?}, veio {vrot:?} / {srot:?} / {stored:?}"));
            }
            let near = |x: f32, y: f32| (x - y).abs() < 1e-4;
            if !near(was_read.zoom, fr.zoom) || !near(was_read.cx, fr.cx) || !near(was_read.cy, fr.cy) {
                return Err(format!("rotacao {phase}: o enquadramento (coordenadas da imagem) mudou: {fr:?} -> {was_read:?}"));
            }
            match adjustment {
                None if full != Some(rot.display_sz(sz)) => {
                    return Err(format!("rotacao {phase}: textura {full:?}, esperava {:?}", rot.display_sz(sz)));
                }
                Some(a) => {
                    let msg = view_window.update(cx, |m, w, cx| check_value(m, w, cx, a, &format!("rotacao {}", rot.degrees()))).map_err(e)??;
                    println!("smoke: {msg}");
                }
                None => {}
            }
            for p in [(0.3, 0.6), (0.31, 0.59)] {
                let (ok, msg) = aligned(view_window, cx, p)?;
                if !ok {
                    return Err(format!("rotacao {phase} {}°: {msg}", rot.degrees()));
                }
            }
        }
        // Quatro giros voltam ao zero, e a rotacao zero sai dos dados do app (em disco tambem).
        let (stored, disk) = view_window
            .update(cx, |m, _, _| (m.state().app_data.rotation(&m.state().current.image), AppData::load(base.clone()).map(|d| d.rotation(&image))))
            .map_err(e)?;
        if stored != Rotation::R0 || disk.map_err(e)? != Rotation::R0 {
            return Err(format!("rotacao {phase}: depois de quatro giros devia voltar a 0"));
        }
    }
    // Girar para a esquerda desfaz um giro para a direita; a rotacao chega ao disco.
    menu_by_the(view_window, cx, MenuAction::RotateRight).await?;
    wait(view_window, cx).await?;
    let disk = AppData::load(base.clone()).map(|d| d.rotation(&image)).map_err(e)?;
    if disk != Rotation::R90 {
        return Err(format!("a rotacao nao chegou ao disco: {disk:?}"));
    }
    menu_by_the(view_window, cx, MenuAction::RotateLeft).await?;
    wait(view_window, cx).await?;
    let (rot, disk) = view_window
        .update(cx, |m, _, _| (m.state().rotation(), AppData::load(base.clone()).map(|d| d.rotation(&image))))
        .map_err(e)?;
    if rot != Rotation::R0 || disk.map_err(e)? != Rotation::R0 {
        return Err("Cmd-L devia desfazer o Cmd-R (estado e disco)".into());
    }
    view_window.update(cx, |m, w, cx| m.set_adjustments(Adjustments::default(), w, cx)).map_err(e)?;
    wait(view_window, cx).await?;
    Ok(())
}

/// O painel de ajustes da direita (icone sempre visivel, ou `F`): encaixa sem
/// cobrir a imagem (a area encolhe `ADJUSTMENTS_PANEL` e a origem nao muda),
/// independe do painel esquerdo (com os dois abertos a area e a prevista) e o
/// grifo/ponto segue alinhado; fechar volta a area de `base`.
pub(super) async fn smoke_adjustments(
    view_window: WindowHandle<MainView>,
    cx: &mut AsyncApp,
    base: (f32, f32, f32, f32),
    highlight: &str,
) -> Result<(), String> {
    let p = highlight_point(view_window, cx, highlight)?;
    let emphasis = view_window.update(cx, |v, _, cx| v.viewer().read(cx).emphasis().map(str::to_string)).map_err(|e| e.to_string())?;
    if emphasis.as_deref() != Some(highlight) {
        return Err(format!("o grifo {highlight} devia estar destacado: {emphasis:?}"));
    }
    view_window.update(cx, |v, w, cx| v.focus_image(w, cx)).map_err(|e| e.to_string())?;
    menu_by_the(view_window, cx, MenuAction::AdjustmentsPanel).await?;
    let a = area_painted(view_window, cx).await?;
    let (la, pa) = expected(view_window, cx)?;
    let (ok_a, txt_a) = aligned(view_window, cx, p)?;
    // Com a gaveta de fontes tambem aberta (Cmd-2): os dois encaixam, a imagem fica no meio.
    key(view_window, cx, "cmd-2").await?;
    let d = area_painted(view_window, cx).await?;
    let (ld, pd) = expected(view_window, cx)?;
    let (ok_d, txt_d) = aligned(view_window, cx, p)?;
    key(view_window, cx, "cmd-2").await?;
    let still = view_window.update(cx, |v, _, _| v.layout()).map_err(|e| e.to_string())?;
    // Fecha pelo metodo do icone (o mesmo que o clique chama).
    view_window.update(cx, |v, _, cx| v.toggle_adjustments(cx)).map_err(|e| e.to_string())?;
    let b = area_painted(view_window, cx).await?;
    let (lb, _) = expected(view_window, cx)?;
    println!(
        "smoke: ajustes (F) a direita: area {:.0}x{:.0} -> {:.0}x{:.0} (prevista {pa:.0}, x {:.0} -> {:.0}); grifo {txt_a}: {ok_a}",
        base.2, base.3, a.2, a.3, base.0, a.0
    );
    println!(
        "smoke: ajustes + fontes: area {:.0} (prevista {pd:.0}, x {:.0}); grifo {txt_d}: {ok_d}; fechar fontes deixa ajustes {:?}; icone fecha: {:.0}x{:.0} {lb:?}",
        d.2, d.0, still.open_adjustments, b.2, b.3
    );
    let ok = la == layout::Layout { left: None, open_adjustments: true }
        && (base.2 - a.2 - layout::ADJUSTMENTS_PANEL).abs() < 2.0
        && (a.2 - pa).abs() < 1.0
        && (a.0 - base.0).abs() < 0.5
        && (a.3 - base.3).abs() < 1.0
        && ok_a
        && ld == layout::Layout { left: Some(Drawer::Sources), open_adjustments: true }
        && (d.2 - pd).abs() < 1.0
        && (d.0 - base.0 - drawers::WIDTH).abs() < 2.0
        && ok_d
        && still == layout::Layout { left: None, open_adjustments: true }
        && lb == layout::Layout::default()
        && (b.2 - base.2).abs() < 0.5;
    if !ok {
        return Err(format!("painel de ajustes: area {base:?} -> {a:?} / {d:?} -> {b:?}"));
    }
    smoke_resize(view_window, cx, base, p).await
}

/// Redimensiona a janela (`Window::resize`) com os dois paineis abertos: a area
/// da imagem segue `layout::width_viewer` (larga: paineis cheios; estreita: os
/// paineis cedem e a imagem fica com `VIEWER_MIN`) e o ponto `p` segue alinhado.
/// Volta ao tamanho original e fecha os paineis (a area volta a `base`).
pub(super) async fn smoke_resize(
    view_window: WindowHandle<MainView>,
    cx: &mut AsyncApp,
    base: (f32, f32, f32, f32),
    p: (f32, f32),
) -> Result<(), String> {
    let original = view_window.update(cx, |_, w, _| w.viewport_size()).map_err(|e| e.to_string())?;
    view_window.update(cx, |v, _, cx| v.toggle_adjustments(cx)).map_err(|e| e.to_string())?;
    key(view_window, cx, "cmd-2").await?;
    for width in [1000.0_f32, 700.0, f32::from(original.width)] {
        let target = size(px(width), original.height);
        view_window.update(cx, |_, w, _| w.resize(target)).map_err(|e| e.to_string())?;
        // O macOS aplica o tamanho de forma assincrona: espera a janela ter a largura pedida.
        let start = Instant::now();
        loop {
            let current_time = view_window.update(cx, |_, w, _| f32::from(w.viewport_size().width)).map_err(|e| e.to_string())?;
            if (current_time - width).abs() < 1.0 {
                break;
            }
            if start.elapsed() > SMOKE_TIMEOUT {
                return Err(format!("a janela nao chegou a {width} px (esta com {current_time})"));
            }
            cx.background_executor().timer(Duration::from_millis(5)).await;
        }
        let a = area_painted(view_window, cx).await?;
        let (l, pv) = expected(view_window, cx)?;
        let w = view_window.update(cx, |_, w, _| f32::from(w.viewport_size().width)).map_err(|e| e.to_string())?;
        let ls = layout::widths(w, &l);
        let (ok, txt) = aligned(view_window, cx, p)?;
        println!(
            "smoke: janela {w:.0} px com {l:?}: area da imagem {:.0} (prevista {pv:.0}; esquerdo {:.0}, ajustes {:.0}); grifo {txt}: {ok}",
            a.2, ls.left, ls.image_adjustments
        );
        if (a.2 - pv).abs() > 1.0 || !ok || (a.0 - layout::ACTIVITIES - ls.left).abs() > 1.0 {
            return Err(format!("janela {w}: area {a:?} != prevista {pv} ({ls:?})"));
        }
        if width == 700.0 && (pv != layout::VIEWER_MIN || ls.left >= drawers::WIDTH) {
            return Err(format!("janela estreita: os paineis deviam ceder ({ls:?})"));
        }
    }
    key(view_window, cx, "cmd-2").await?;
    view_window.update(cx, |v, _, cx| v.toggle_adjustments(cx)).map_err(|e| e.to_string())?;
    let end = area_painted(view_window, cx).await?;
    println!("smoke: tamanho original e paineis fechados: area {:.0}x{:.0}", end.2, end.3);
    if (end.2 - base.2).abs() > 0.5 || (end.3 - base.3).abs() > 0.5 {
        return Err(format!("a area nao voltou: {base:?} -> {end:?}"));
    }
    Ok(())
}

/// "Ir para" da barra de ferramentas (a mesma funcao que o Enter na caixa chama): um numero
/// acima do total vai ao ultimo, 0 vai ao primeiro, um numero no meio vai a ele e o que nao e
/// numero nao muda nada. Termina na primeira imagem.
pub(super) async fn smoke_go_to(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    let e = |x: gpui::Result<()>| x.map_err(|e| e.to_string());
    wait(view_window, cx).await?;
    let where_ = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp| -> Result<(usize, usize), String> {
        view_window
            .update(cx, |m, _, _| m.state().unit_go_to().map(|(_, a, n)| (a, n)))
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "sem posicao conhecida para o Ir para".to_string())
    };
    let (_, total) = where_(view_window, cx)?;
    // A caixa so aceita algarismos, e so tantos quanto o total tem (4 imagens: 1 algarismo).
    let max = state::quick_note::digits(total);
    e(view_window.update(cx, |m, w, cx| m.textbox_go_to().read(cx).focus_handle().focus(w)))?;
    cx.background_executor().timer(Duration::from_millis(50)).await;
    // Ao focar, a caixa mostra a imagem atual SEM selecionar nada: digitar acrescenta (e com a
    // caixa cheia nao entra nada), so apagar libera.
    let (text_0, selected) = view_window
        .update(cx, |m, _, cx| (m.textbox_go_to().read(cx).text().to_string(), m.textbox_go_to().read(cx).selection_has()))
        .map_err(|e| e.to_string())?;
    if selected {
        return Err(format!("a caixa do Ir para ficou com {text_0:?} selecionado ao focar"));
    }
    typing(view_window, cx, "9").await?;
    let full = view_window.update(cx, |m, _, cx| m.textbox_go_to().read(cx).text().to_string()).map_err(|e| e.to_string())?;
    if full != text_0 {
        return Err(format!("digitar com a caixa cheia deveria recusar: {text_0:?} virou {full:?}"));
    }
    // Selecao pelo mouse (mesma logica do clique): duplo clique pega o numero, clique solto desfaz,
    // Shift+clique estende, e digitar substitui o selecionado.
    let sel = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp, off: usize, n: usize, shift: bool| -> Result<String, String> {
        view_window
            .update(cx, |m, _, cx| {
                m.textbox_go_to().update(cx, |c, _| {
                    c.click(off, n, shift);
                    c.selected().to_string()
                })
            })
            .map_err(|e| e.to_string())
    };
    let double = sel(view_window, cx, 0, 2, false)?;
    if double != text_0 {
        return Err(format!("duplo clique devia selecionar {text_0:?}, selecionou {double:?}"));
    }
    if !sel(view_window, cx, 0, 1, false)?.is_empty() {
        return Err("clique simples devia desfazer a selecao".into());
    }
    if sel(view_window, cx, text_0.len(), 1, true)? != text_0 {
        return Err("Shift+clique devia estender ate o fim".into());
    }
    typing(view_window, cx, "9").await?;
    let after = view_window.update(cx, |m, _, cx| m.textbox_go_to().read(cx).text().to_string()).map_err(|e| e.to_string())?;
    if after != "9" {
        return Err(format!("digitar sobre o selecionado devia deixar \"9\", ficou {after:?}"));
    }
    view_window.update(cx, |m, _, cx| m.textbox_go_to().update(cx, |c, cx| c.load_text(&text_0, cx))).map_err(|e| e.to_string())?;
    key(view_window, cx, "backspace").await?;
    typing(view_window, cx, "a-.c 9").await?;
    e(view_window.update(cx, |m, w, cx| {
        m.textbox_go_to().update(cx, |n, cx| {
            n.replace_and_mark_text_in_range(None, "~", None, w, cx);
            n.replace_text_in_range(None, "~", w, cx);
        })
    }))?;
    let typed = view_window.update(cx, |m, _, cx| m.textbox_go_to().read(cx).text().to_string()).map_err(|e| e.to_string())?;
    if typed != "9" {
        return Err(format!("a caixa do Ir para aceitou {typed:?}, esperava \"9\""));
    }
    // Numero enorme: a caixa para no limite de algarismos do total (nao quebra em varias linhas).
    typing(view_window, cx, "98765432109876").await?;
    let long = view_window.update(cx, |m, _, cx| m.textbox_go_to().read(cx).text().to_string()).map_err(|e| e.to_string())?;
    if long.chars().count() != max {
        return Err(format!("a caixa do Ir para aceitou {long:?}, esperava {max} algarismo(s)"));
    }
    // Setinhas / ↑ ↓: vazia parte de onde estamos; sempre entre 1 e o total.
    e(view_window.update(cx, |m, w, cx| m.cancel_go_to(w, cx)))?;
    let (current, _) = where_(view_window, cx)?;
    let mut steps = Vec::new();
    for (delta, expected) in [(1, (current + 1).min(total)), (10, total), (-1, total - 1), (-10, 1)] {
        e(view_window.update(cx, |m, w, cx| m.step_go_to(delta, w, cx)))?;
        let t = view_window.update(cx, |m, _, cx| m.textbox_go_to().read(cx).text().to_string()).map_err(|e| e.to_string())?;
        steps.push(format!("{delta:+} -> {t}"));
        if t != expected.to_string() {
            return Err(format!("passo {delta:+} deixou {t:?}, esperava {expected} ({steps:?})"));
        }
    }
    e(view_window.update(cx, |m, w, cx| m.cancel_go_to(w, cx)))?;
    let mut seen = vec![format!("digitado {typed:?}"), format!("longo {long:?}"), format!("passos {steps:?}")];
    for (text, expected) in [("99999", total), ("0", 1), ("2", 2.min(total)), ("abc", 2.min(total)), ("-5", 1)] {
        e(view_window.update(cx, |m, w, cx| m.confirm_go_to(text, w, cx)))?;
        wait(view_window, cx).await?;
        let (current, _) = where_(view_window, cx)?;
        seen.push(format!("{text:?} -> {current}/{total}"));
        if current != expected {
            return Err(format!("Ir para {text:?} parou em {current}, esperava {expected} ({seen:?})"));
        }
    }
    // A caixa mostra a imagem atual e acompanha a navegacao (fora da digitacao).
    let textbox_text = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp| {
        view_window.update(cx, |m, _, cx| m.textbox_go_to().read(cx).text().to_string()).map_err(|e| e.to_string())
    };
    let (current, _) = where_(view_window, cx)?;
    if textbox_text(view_window, cx)? != current.to_string() {
        return Err(format!("a caixa do Ir para mostra {:?}, esperava {current}", textbox_text(view_window, cx)?));
    }
    if total > 1 {
        e(view_window.update(cx, |m, w, cx| m.go_next(w, cx).then_some(()).unwrap_or(())))?;
        wait(view_window, cx).await?;
        let t = textbox_text(view_window, cx)?;
        if t != (current + 1).to_string() {
            return Err(format!("depois de ir para a proxima, a caixa do Ir para mostra {t:?}, esperava {}", current + 1));
        }
        e(view_window.update(cx, |m, w, cx| m.go_prev(w, cx).then_some(()).unwrap_or(())))?;
        wait(view_window, cx).await?;
        seen.push(format!("acompanha a navegacao ({current} -> {} -> {current})", current + 1));
    }
    println!("smoke: Ir para: {}", seen.join("; "));
    Ok(())
}
