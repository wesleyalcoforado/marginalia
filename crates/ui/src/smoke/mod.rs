//! O smoke (`MARGINALIA_SMOKE=1`): abre a fonte, espera o primeiro quadro, aplica e
//! desfaz um ajuste, exercita as barras do painel (`smoke_barras`: valor da
//! barra e soltar, numero na caixa, duplo clique, o olho antes/depois), pede
//! o tile de uma janela visivel menor que a pagina (`smoke_window`),
//! percorre TODOS os quadros com `go_next` conferindo o
//! marcador de erro e sai (0 = ok, 1 = falha). `MARGINALIA_SMOKE_ERROS=n` exige
//! exatamente `n` quadros com erro. `MARGINALIA_APPDATA=pasta` troca a pasta
//! dos dados do app (o smoke nao deve tocar a do usuario). Com um vault aberto
//! o smoke tambem desenha grifos com eventos de mouse, digita notas com eventos
//! de teclado (e uma composicao de acento pelo InputHandler), cria notas e uma
//! tarefa, confere os arquivos no vault e reabre uma nota (`smoke_notes`). Depois
//! abre e fecha cada gaveta (Cmd-1/2/3) conferindo que a area da imagem encolhe e
//! volta (igual a `layout::width_viewer`, com a imagem deslocada para a
//! direita), abre e fecha o painel de ajustes da direita (`F`, sozinho e com a
//! gaveta de fontes) conferindo a area prevista e o grifo destacado alinhado
//! (`smoke_adjustments`), e que a imagem e o grifo seguem desenhados e alinhados; pela gaveta abre
//! uma nota, filtra as notas por busca (digitada), etiqueta e contador, pede a
//! exclusao com Backspace (e cancela), marca a tarefa como feita (x) e abre a imagem dela (`smoke_drawers`). O drop
//! (`smoke_drop`, tambem sem vault) chama o tratador direto: dois itens e um
//! caminho inexistente recusados com aviso, uma pasta abre como no Cmd-O. Por fim
//! (`smoke_source_movable`) abre uma pasta temporaria, MOVE a pasta com a fonte
//! aberta, confere o aviso "Fonte indisponivel" (e que as setas nao empilham
//! leituras), aponta a nova localizacao e confere que o mesmo quadro volta no
//! mesmo enquadramento; com vault tambem renomeia a fonte pela caixa da gaveta,
//! troca para um vault novo (a nota em edicao vai para o vault anterior e a
//! caixa limpa) e volta, e abre/fecha o seletor com Cmd-Shift-V. O salvamento
//! automatico (`smoke_autosave`): digitar sem Enter cria o rascunho e grava o
//! texto; com o relogio da sessao adiantado 11 s o id e fixado uma vez so.
//! A rotacao (`smoke_rotation`: Cmd-R pelo menu, quatro vezes, sem e com ajuste, conferindo
//! rotacao, textura girada, centro, janela do tile e dados do app; `smoke_rotation_highlights`, com
//! vault: grifos desenhados e clicados sobre a pagina girada, gravados na orientacao original).
//! Os registros (`smoke_records`: marcar inicio e fim pela faixa, fronteira compartilhada, "Mover para",
//! arvore da gaveta de notas, cancelar e excluir sem perder notas).
//! O painel das gavetas se redimensiona arrastando a borda (`smoke_resize_drawer`: largura limitada,
//! area da imagem acompanha, gravada nos dados do app).
//! As pastas temporarias do smoke sao apagadas na saida.

mod shortcuts_pc;
mod menu_bar;
mod input;
mod sources;
mod drawers_and_menus;
mod highlights;
mod image;
mod notes;
mod records;
mod update;

use crate::*;

use std::time::{Duration, Instant};

use gpui::{
    AsyncApp, EntityInputHandler, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
};
use marginalia_core::filters::Adjustments;
use marginalia_core::frontmatter::parse;
use marginalia_core::highlights::{Highlight, HighlightsFile};
use marginalia_core::model::{Framing, StateTask, Note, Task};

use crate::app_state::Frame;
use crate::drawers::Drawer;
use crate::focus_state::Focus;

/// Teto de espera do smoke por uma carga (abrir a fonte, um quadro).
const SMOKE_TIMEOUT: Duration = Duration::from_secs(20);

use self::{shortcuts_pc::*, menu_bar::*, input::*, sources::*, drawers_and_menus::*, highlights::*, image::*, notes::*, records::*, update::*};

/// Espera a view ficar ociosa (sem carga pendente), com fonte, e o
/// visualizador ter pintado. Pede uma pintura a cada ~200 ms (a janela do smoke
/// pode estar sem foco e o gpui so pinta quando ha motivo).
/// Imagens minimas na fonte do smoke: as corridas da carga navegam a partir
/// do 1o quadro e precisam de um quadro seguinte de verdade.
pub(super) const SMOKE_MIN_IMAGES: usize = 3;

/// Volta ao 1o quadro da fonte (para as corridas da carga terem um quadro
/// seguinte). Fonte com menos de [`SMOKE_MIN_IMAGES`] imagens: erro claro.
pub(super) async fn go_to_first_frame(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    let index = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp| {
        view_window.update(cx, |m, _, _| m.state().index()).map_err(|e| e.to_string())
    };
    let (_, n) = index(view_window, cx)?.ok_or("smoke: a imagem atual nao esta na fonte")?;
    if n < SMOKE_MIN_IMAGES {
        return Err(format!("smoke precisa de pelo menos {SMOKE_MIN_IMAGES} imagens na fonte (tem {n})"));
    }
    // Volta ate o anterior nao andar mais (o inicio da fonte).
    for _ in 0..1000 {
        let moved = view_window.update(cx, |m, w, cx| m.go_prev(w, cx)).map_err(|e| e.to_string())?;
        wait(view_window, cx).await?;
        if !moved {
            break;
        }
    }
    match index(view_window, cx)? {
        Some((1, _)) => Ok(()),
        o => Err(format!("smoke: nao consegui voltar ao 1o quadro (parou em {o:?} de {n})")),
    }
}

pub(super) async fn wait(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<Duration, String> {
    let start = Instant::now();
    let mut paint = Instant::now();
    loop {
        // A janela visivel que o visualizador ainda vai avisar (imagem recem-chegada,
        // pan, zoom): a tela so esta pronta depois de ela sair e o tile dela chegar.
        let (idle, source_has, painted, by_inform) = view_window
            .update(cx, |v, w, cx| {
                let vw = v.viewer().read(cx);
                (v.idle(), v.state().source_has(), vw.painted().get(), vw.window_by_inform(w).is_some())
            })
            .map_err(|e| e.to_string())?;
        if by_inform {
            view_window.update(cx, |_, w, _| w.refresh()).map_err(|e| e.to_string())?;
        }
        let idle = idle && !by_inform;
        if idle && source_has && painted {
            let t = start.elapsed();
            // Folga para a pintura do novo estado acontecer.
            cx.background_executor().timer(Duration::from_millis(30)).await;
            return Ok(t);
        }
        if start.elapsed() > SMOKE_TIMEOUT {
            let failed: Vec<&str> = [("ocioso", idle), ("tem_fonte", source_has), ("pintou", painted)]
                .iter()
                .filter(|(_, ok)| !ok)
                .map(|(n, _)| *n)
                .collect();
            let d = view_window.update(cx, |v, _, _| v.debug()).unwrap_or_default();
            return Err(format!("nada pronto em {SMOKE_TIMEOUT:?}: falhou {} [{d}]", failed.join(", ")));
        }
        if paint.elapsed() >= Duration::from_millis(200) {
            view_window.update(cx, |_, w, _| w.refresh()).map_err(|e| e.to_string())?;
            paint = Instant::now();
        }
        cx.background_executor().timer(Duration::from_millis(2)).await;
    }
}

/// Espera so a view ficar ociosa (troca de vault, nova localizacao, renomear).
pub(super) async fn wait_idle(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    let start = Instant::now();
    while !view_window.update(cx, |v, _, _| v.idle()).map_err(|e| e.to_string())? {
        if start.elapsed() > SMOKE_TIMEOUT {
            return Err(format!("trabalho no fundo nao terminou em {SMOKE_TIMEOUT:?}"));
        }
        cx.background_executor().timer(Duration::from_millis(2)).await;
    }
    view_window.update(cx, |_, w, _| w.refresh()).map_err(|e| e.to_string())?;
    cx.background_executor().timer(Duration::from_millis(30)).await;
    Ok(())
}

/// Uma linha do relatorio do smoke, se a tela bate com o estado, e se ha erro.
pub(super) fn describe(v: &MainView, cx: &App) -> (String, bool, bool) {
    let e = v.state();
    let vw = v.viewer().read(cx);
    let (page, total) = e.page();
    let (i, n) = e.index().unwrap_or((0, 0));
    let total = total.map(|t| t.to_string()).unwrap_or_else(|| "?".into());
    let state = match &e.error {
        Some(m) => format!("ERRO, marcador visivel: {} ({m})", vw.error_shows()),
        None => format!("ok, imagem na tela: {}", vw.image_has()),
    };
    let consistent = match &e.error {
        Some(_) => vw.error_shows() && !vw.image_has(),
        None => vw.image_has() && !vw.error_shows(),
    };
    (format!("[{i}/{n}] {} pag {page}/{total}: {state}", e.current.image.file), consistent, e.error.is_some())
}

/// Ajustes na memoria, no disco (appdata relido) e os que a imagem na tela levou.
pub(super) fn three_adjustments(v: &MainView) -> Result<(Adjustments, Adjustments, Option<Adjustments>), String> {
    let e = v.state();
    let disk = AppData::load(e.app_data.base().to_path_buf()).map_err(|e| e.to_string())?.adjustments(&e.current.image);
    Ok((e.adjustments.clone(), disk, e.screen_adjustments().cloned()))
}

/// Espera o visualizador avisar a janela do enquadramento atual (um quadro).
pub(super) async fn await_frame(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    let start = Instant::now();
    loop {
        let pending = view_window.update(cx, |m, w, cx| m.viewer().read(cx).window_by_inform(w).is_some()).map_err(|e| e.to_string())?;
        if !pending {
            return Ok(());
        }
        if start.elapsed() > SMOKE_TIMEOUT {
            return Err("o visualizador nao avisou a janela visivel".into());
        }
        view_window.update(cx, |_, w, _| w.refresh()).map_err(|e| e.to_string())?;
        cx.background_executor().timer(Duration::from_millis(2)).await;
    }
}

pub(crate) async fn run_smoke(view_window: WindowHandle<MainView>, cx: &mut AsyncApp, expected: Option<usize>) -> Result<(), String> {
    let t = wait(view_window, cx).await?;
    println!("smoke: fonte aberta e primeiro quadro na tela em {:.0} ms", t.as_secs_f64() * 1e3);

    // Filtro: aplica "inverter" no primeiro quadro (se abriu sem erro) e desfaz.
    let with_error = view_window.update(cx, |v, _, _| v.state().error.is_some()).map_err(|e| e.to_string())?;
    if !with_error {
        let inv = Adjustments { invert: true, ..Adjustments::default() };
        view_window.update(cx, |v, w, cx| v.set_adjustments(inv, w, cx)).map_err(|e| e.to_string())?;
        let t = wait(view_window, cx).await?;
        let ok = view_window
            .update(cx, |v, _, cx| v.state().adjustments.invert && v.viewer().read(cx).image_has())
            .map_err(|e| e.to_string())?;
        println!("smoke: filtro inverter aplicado em {:.0} ms (previa, final e gravacao): {ok}", t.as_secs_f64() * 1e3);
        if !ok {
            return Err("filtro nao aplicado".into());
        }
        // Arrasto: 10 valores seguidos e soltar; fica o ultimo, na tela e no disco (gravado no fundo).
        let t0 = Instant::now();
        for i in 1..=10 {
            let a = Adjustments { invert: true, brightness: i as f32 * 0.02, ..Adjustments::default() };
            view_window.update(cx, |v, w, cx| v.adjustment_changed(a, w, cx)).map_err(|e| e.to_string())?;
        }
        let ui = t0.elapsed();
        view_window.update(cx, |v, w, cx| v.adjustment_dropped(w, cx)).map_err(|e| e.to_string())?;
        let t = wait(view_window, cx).await?;
        let (mem, disk) = view_window
            .update(cx, |v, _, _| {
                let e = v.state();
                let disk = AppData::load(e.app_data.base().to_path_buf()).map(|d| d.adjustments(&e.current.image));
                (e.adjustments.clone(), disk)
            })
            .map_err(|e| e.to_string())?;
        let disk = disk.map_err(|e| e.to_string())?;
        println!(
            "smoke: arrasto de 10 valores em {:.1} ms na interface, pronto {:.0} ms depois de soltar; brilho {} (disco {})",
            ui.as_secs_f64() * 1e3,
            t.as_secs_f64() * 1e3,
            mem.brightness,
            disk.brightness
        );
        if (mem.brightness - 0.2).abs() > 1e-6 || disk != mem {
            return Err("o arrasto nao ficou com o ultimo valor (na memoria e no disco)".into());
        }
        // Troca de imagem no meio do arrasto (sem soltar): o valor fica na imagem
        // em que foi escolhido, a proxima continua com o dela (memoria e disco).
        let middle = Adjustments { grayscale: true, ..Adjustments::default() };
        let (img1, img2, before_2) = view_window
            .update(cx, |v, w, cx| {
                v.adjustment_changed(middle.clone(), w, cx);
                let img1 = v.state().current.image.clone();
                v.go_next(w, cx);
                let img2 = v.state().current.image.clone();
                (img1, img2.clone(), v.state().app_data.adjustments(&img2))
            })
            .map_err(|e| e.to_string())?;
        if img1 != img2 {
            wait(view_window, cx).await?;
            let (mem1, mem2, d1, d2) = view_window
                .update(cx, |v, _, _| {
                    let e = v.state();
                    let d = AppData::load(e.app_data.base().to_path_buf());
                    let (d1, d2) = match d {
                        Ok(d) => (Some(d.adjustments(&img1)), Some(d.adjustments(&img2))),
                        Err(_) => (None, None),
                    };
                    (e.app_data.adjustments(&img1), e.adjustments.clone(), d1, d2)
                })
                .map_err(|e| e.to_string())?;
            println!(
                "smoke: troca de imagem no meio do arrasto -> {} cinza {} (disco {:?}); {} cinza {} (disco {:?})",
                img1.file,
                mem1.grayscale,
                d1.as_ref().map(|a| a.grayscale),
                img2.file,
                mem2.grayscale,
                d2.as_ref().map(|a| a.grayscale)
            );
            if mem1 != middle || d1.as_ref() != Some(&middle) || mem2 != before_2 || d2.as_ref() != Some(&before_2) {
                return Err("trocar de imagem no meio do arrasto levou o valor para a imagem errada".into());
            }
            view_window.update(cx, |v, w, cx| v.go_prev(w, cx)).map_err(|e| e.to_string())?;
            wait(view_window, cx).await?;
        }
        smoke_window(view_window, cx).await?;
        smoke_values_with_zoom(view_window, cx).await?;
        view_window.update(cx, |v, w, cx| v.set_adjustments(Adjustments::default(), w, cx)).map_err(|e| e.to_string())?;
        wait(view_window, cx).await?;
        smoke_barras(view_window, cx).await?;
        smoke_rotation(view_window, cx).await?;
        smoke_go_to(view_window, cx).await?;
    }

    let (mut frames, mut errors) = (0, 0);
    let (mut max_ui, mut max_load) = (Duration::ZERO, Duration::ZERO);
    loop {
        let (text, consistent, with_error) = view_window.update(cx, |v, _, cx| describe(v, cx)).map_err(|e| e.to_string())?;
        frames += 1;
        errors += with_error as usize;
        println!("smoke: {text}");
        if !consistent {
            return Err(format!("tela incoerente com o estado: {text}"));
        }
        let t0 = Instant::now();
        let moved = view_window.update(cx, |v, w, cx| v.go_next(w, cx)).map_err(|e| e.to_string())?;
        max_ui = max_ui.max(t0.elapsed());
        if !moved {
            break;
        }
        max_load = max_load.max(wait(view_window, cx).await?);
    }
    println!(
        "smoke: {frames} quadros percorridos, {errors} com erro; go_next na thread da interface: max {:.2} ms; \
         ate o quadro seguinte estar pronto: max {:.0} ms",
        max_ui.as_secs_f64() * 1e3,
        max_load.as_secs_f64() * 1e3
    );
    // Abrir um quadro com enquadramento guardado (como uma nota fara).
    let first = view_window
        .update(cx, |v, _, _| v.state().first_frame())
        .map_err(|e| e.to_string())?
        .ok_or("fonte sem quadros")?;
    let fr = Framing { zoom: 2.0, cx: 0.25, cy: 0.75 };
    let mut f = first.clone();
    f.framing = Some(fr.clone());
    view_window.update(cx, |v, w, cx| v.open_frame(f, w, cx)).map_err(|e| e.to_string())?;
    wait(view_window, cx).await?;
    let (was_read, forward, back) = view_window
        .update(cx, |v, _, cx| {
            let vw = v.viewer().read(cx);
            let forward = vw.image_to_screen((0.25, 0.75));
            (vw.framing().clone(), forward, vw.screen_to_image(forward))
        })
        .map_err(|e| e.to_string())?;
    println!(
        "smoke: open_frame({}) com enquadramento {fr:?} -> viewer {was_read:?}; imagem (0.25, 0.75) -> janela ({:.1}, {:.1}) -> imagem ({:.4}, {:.4})",
        first.image.file, forward.x, forward.y, back.0, back.1
    );
    let with_error = view_window.update(cx, |v, _, _| v.state().error.is_some()).map_err(|e| e.to_string())?;
    if !with_error && was_read != fr {
        return Err("open_frame nao aplicou o enquadramento".into());
    }

    if let Some(n) = expected.filter(|n| *n != errors) {
        return Err(format!("esperava {n} quadros com erro, vieram {errors}"));
    }
    let vault_has = view_window.update(cx, |v, _, _| v.state().vault.is_some()).map_err(|e| e.to_string())?;
    if vault_has {
        smoke_notes(view_window, cx).await
    } else {
        println!("smoke: sem vault (--vault): grifos e nota rapida nao exercitados");
        smoke_drop(view_window, cx).await?;
        smoke_source_movable(view_window, cx).await
    }
}

/// Espera a recarga das gavetas chegar.
pub(super) async fn wait_lists(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    let start = Instant::now();
    while view_window.update(cx, |v, _, _| v.drawers().loading()).map_err(|e| e.to_string())? {
        if start.elapsed() > SMOKE_TIMEOUT {
            return Err("as listas das gavetas nao chegaram".into());
        }
        cx.background_executor().timer(Duration::from_millis(2)).await;
    }
    Ok(())
}

/// Area do visualizador depois de uma volta de pintura (com a imagem desenhada de novo).
pub(super) async fn area_painted(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(f32, f32, f32, f32), String> {
    view_window.update(cx, |v, _, cx| v.viewer().read(cx).painted().set(false)).map_err(|e| e.to_string())?;
    view_window.update(cx, |_, w, _| w.refresh()).map_err(|e| e.to_string())?;
    wait(view_window, cx).await?;
    wait_lists(view_window, cx).await?;
    cx.background_executor().timer(Duration::from_millis(30)).await;
    view_window.update(cx, |v, _, cx| v.viewer().read(cx).area()).map_err(|e| e.to_string())
}
