//! Cenarios do smoke: fontes (drop, fonte que muda de lugar, renomear e trocar de vault).

use super::*;

/// Task 15: a fonte some com a imagem aberta (pasta movida, como um NAS
/// desmontado/remontado em outro lugar), aviso, setas sem acumular, nova
/// localizacao e o mesmo quadro de volta; com vault, renomear, trocar de vault
/// (o rascunho fica) e o seletor pelo Cmd-Shift-V.
/// Arrastar e soltar (Task 11), chamando o tratador do drop direto (o gpui nao
/// deixa simular o `FileDropEvent` de fora): dois itens e um caminho que nao
/// existe sao recusados com aviso; sem vault, uma pasta valida tambem e
/// recusada; com vault, uma pasta temporaria abre como no Cmd-O (e vira fonte).
pub(super) async fn smoke_drop(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    let pid = std::process::id();
    let base = std::env::temp_dir().join(format!("marginalia-smoke-drop-{pid}"));
    temporary(&base);
    let folder = base.join(format!("Fonte Solta {pid}"));
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    for i in 1..=2 {
        std::fs::copy(SAMPLE, folder.join(format!("s{i}.jpg"))).map_err(|e| e.to_string())?;
    }
    let before = view_window.update(cx, |v, _, _| v.state().current.image.source.clone()).map_err(|e| e.to_string())?;
    let drop_item = |view_window: WindowHandle<MainView>, cx: &mut AsyncApp, paths: Vec<PathBuf>| {
        view_window.update(cx, |v, w, cx| v.drop_item(&paths, w, cx)).map_err(|e| e.to_string())
    };
    let two = drop_item(view_window, cx, vec![folder.join("s1.jpg"), folder.join("s2.jpg")])?;
    let missing = drop_item(view_window, cx, vec![base.join("nao-existe.zip")])?;
    let vault_has = view_window.update(cx, |v, _, _| v.state().vault.is_some()).map_err(|e| e.to_string())?;
    let warning_folder = drop_item(view_window, cx, vec![folder.clone()])?;
    println!("smoke: drop de 2 itens -> {two:?}; inexistente -> {missing:?}; pasta (vault {vault_has}) -> aviso {warning_folder:?}");
    let name = open_source::default_name(&folder);
    if !vault_has {
        // Sem vault o aviso do vault vem antes de olhar o que foi solto.
        let after = view_window.update(cx, |v, _, _| v.state().current.image.source.clone()).map_err(|e| e.to_string())?;
        let of_vault = |a: &Option<String>| a.as_ref().is_some_and(|a| a.contains("abra um vault"));
        if !of_vault(&two) || !of_vault(&missing) || !of_vault(&warning_folder) || after != before {
            return Err("drop sem vault devia ser recusado com aviso".into());
        }
        return Ok(());
    }
    if !two.is_some_and(|a| a.contains("um item so")) || !missing.is_some_and(|a| a.contains("nao encontrado")) {
        return Err("drop invalido nao foi recusado com aviso".into());
    }
    if warning_folder.is_some() {
        return Err("drop de uma pasta valida com vault deu aviso".into());
    }
    // O mesmo caminho do Cmd-O: o nome e decidido no fundo e a abertura comeca depois.
    let start = Instant::now();
    loop {
        let source = view_window.update(cx, |v, _, _| v.state().current.image.source.clone()).map_err(|e| e.to_string())?;
        if source == name {
            break;
        }
        if start.elapsed() > SMOKE_TIMEOUT {
            return Err(format!("a pasta solta nao abriu em {SMOKE_TIMEOUT:?} (fonte atual {source:?})"));
        }
        cx.background_executor().timer(Duration::from_millis(50)).await;
    }
    wait(view_window, cx).await?;
    let (file, screen, root_dir) = view_window
        .update(cx, |v, _, cx| {
            let vw = v.viewer().read(cx);
            (
                v.state().current.image.file.clone(),
                vw.image_has() && !vw.error_shows(),
                v.state().vault.as_ref().map(|v| v.root.clone()),
            )
        })
        .map_err(|e| e.to_string())?;
    let registered = root_dir.is_some_and(|r| r.join(format!("fontes/{name}.md")).is_file());
    println!("smoke: drop da pasta {} -> fonte {name:?} quadro {file} na tela {screen}; em fontes/ {registered}", folder.display());
    if file != "s1.jpg" || !screen || !registered {
        return Err("a pasta solta nao abriu como no Cmd-O".into());
    }
    Ok(())
}

pub(super) async fn smoke_source_movable(view_window: WindowHandle<MainView>, cx: &mut AsyncApp) -> Result<(), String> {
    let pid = std::process::id();
    let base = std::env::temp_dir().join(format!("marginalia-smoke-fontes-{pid}"));
    temporary(&base);
    let folder = base.join(format!("Fonte Movel {pid}"));
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    for i in 1..=12 {
        std::fs::copy(SAMPLE, folder.join(format!("p{i:02}.jpg"))).map_err(|e| e.to_string())?;
    }
    let name = open_source::default_name(&folder);
    view_window.update(cx, |v, w, cx| v.open_path(folder.clone(), name.clone(), ZipMode::Direct, w, cx)).map_err(|e| e.to_string())?;
    wait(view_window, cx).await?;
    let root_dir = view_window.update(cx, |v, _, _| v.state().vault.as_ref().map(|v| v.root.clone())).map_err(|e| e.to_string())?;
    let md = root_dir.as_ref().map(|r| r.join(format!("fontes/{name}.md")));
    let md_before = md.as_ref().map(std::fs::read_to_string).transpose().map_err(|e| e.to_string())?;

    // A pasta muda de lugar com a fonte aberta; abrir um quadro fora do pre-carregamento le do disco e falha.
    let moved = base.join(format!("Fonte Movida {pid}"));
    std::fs::rename(&folder, &moved).map_err(|e| e.to_string())?;
    let fr = Framing { zoom: 2.0, cx: 0.25, cy: 0.75 };
    let mut f = Frame::new(&name, "p11.jpg", 1);
    f.framing = Some(fr.clone());
    view_window.update(cx, |v, w, cx| v.open_frame(f.clone(), w, cx)).map_err(|e| e.to_string())?;
    wait(view_window, cx).await?;
    let (pending, marker) = view_window
        .update(cx, |v, _, cx| (v.state().pending().cloned(), v.viewer().read(cx).error_shows()))
        .map_err(|e| e.to_string())?;
    let pending = pending.ok_or("a fonte movida nao virou aviso de fonte indisponivel")?;
    println!("smoke: pasta movida com a fonte aberta -> aviso {:?} ({}); marcador de erro: {marker}", pending.title(), pending.explanation());
    if pending.reason != relink::Reason::Unavailable || pending.name != name || !marker || !pending.title().contains("Fonte indisponível") {
        return Err(format!("aviso errado: {pending:?}"));
    }
    // Setas durante a queda: nenhuma leitura nova, o quadro fica.
    for _ in 0..3 {
        key(view_window, cx, "right").await?;
    }
    let (idle, current) =
        view_window.update(cx, |v, _, _| (v.idle(), v.state().current.clone())).map_err(|e| e.to_string())?;
    println!("smoke: 3 setas durante a queda -> ocioso {idle}, quadro {}", current.image.file);
    if !idle || current.image.file != "p11.jpg" {
        return Err("setas durante a queda empilharam trabalho".into());
    }
    // Nova localizacao (o que o botao faz depois do painel do sistema).
    view_window.update(cx, |v, w, cx| v.relink(&name, moved.clone(), w, cx)).map_err(|e| e.to_string())?;
    wait(view_window, cx).await?;
    let (current, read_framing, image, pending, map) = view_window
        .update(cx, |v, _, cx| {
            let e = v.state();
            let map = e.vault.as_ref().and_then(|vt| e.app_data.source_path(&vt.root, &name));
            let vw = v.viewer().read(cx);
            (e.current.image.clone(), vw.framing().clone(), vw.image_has() && !vw.error_shows(), e.pending().cloned(), map)
        })
        .map_err(|e| e.to_string())?;
    let md_after = md.as_ref().map(std::fs::read_to_string).transpose().map_err(|e| e.to_string())?;
    println!(
        "smoke: nova localizacao {} -> {} na tela {image}, enquadramento {read_framing:?}, aviso {:?}, mapa {map:?}, fontes/ intocado {}",
        moved.display(),
        current.file,
        pending.map(|p| p.title()),
        md_before == md_after
    );
    if current.file != "p11.jpg" || !image || read_framing != fr || (root_dir.is_some() && map.as_ref() != Some(&moved)) || md_before != md_after {
        return Err("depois da nova localizacao o quadro nao voltou".into());
    }
    if !view_window.update(cx, |v, w, cx| v.go_next(w, cx)).map_err(|e| e.to_string())? {
        return Err("nao navega depois da nova localizacao".into());
    }
    wait(view_window, cx).await?;

    let Some(root_dir) = root_dir else {
        println!("smoke: sem vault: renomear e trocar de vault nao exercitados");
        return Ok(());
    };
    smoke_rename_and_vaults(view_window, cx, &root_dir, &base, &name, &moved).await
}

pub(super) async fn smoke_rename_and_vaults(
    view_window: WindowHandle<MainView>,
    cx: &mut AsyncApp,
    root_dir: &Path,
    base: &Path,
    name: &str,
    moved: &Path,
) -> Result<(), String> {
    // Renomear pela caixa da gaveta de fontes (o editor da nota rapida).
    let new = format!("Fonte Renomeada {}", std::process::id());
    key(view_window, cx, "cmd-2").await?;
    wait_lists(view_window, cx).await?;
    // O botao "Renomear" da linha (a caixa aparece na linha, com o foco).
    view_window.update(cx, |v, w, cx| v.start_rename(name, w, cx)).map_err(|e| e.to_string())?;
    view_window.update(cx, |_, w, _| w.refresh()).map_err(|e| e.to_string())?;
    cx.background_executor().timer(Duration::from_millis(80)).await;
    typing(view_window, cx, &new).await?;
    let in_textbox = view_window
        .update(cx, |v, _, cx| v.rename_textbox_text(cx))
        .map_err(|e| e.to_string())?;
    if in_textbox != new {
        return Err(format!("caixa de renomear com {in_textbox:?}, esperava {new:?}"));
    }
    key(view_window, cx, "enter").await?;
    wait_idle(view_window, cx).await?;
    wait_lists(view_window, cx).await?;
    let (source, screen, renaming, listed) = view_window
        .update(cx, |v, _, cx| {
            let vw = v.viewer().read(cx);
            (
                v.state().current.image.source.clone(),
                vw.image_has() && !vw.error_shows(),
                v.renaming().map(str::to_string),
                v.drawers().sources().iter().any(|f| f.name == new),
            )
        })
        .map_err(|e| e.to_string())?;
    let files = (root_dir.join(format!("fontes/{new}.md")).is_file(), root_dir.join(format!("fontes/{name}.md")).exists());
    println!("smoke: renomear {name:?} -> {source:?}; fontes/ (novo, antigo) {files:?}; na gaveta {listed}; imagem na tela {screen}");
    if source != new || files != (true, false) || !screen || renaming.is_some() || !listed {
        return Err("renomear a fonte falhou".into());
    }
    if !view_window.update(cx, |v, w, cx| v.go_prev(w, cx)).map_err(|e| e.to_string())? {
        return Err("nao navega depois de renomear".into());
    }
    wait(view_window, cx).await?;
    key(view_window, cx, "cmd-2").await?;

    // Trocar de vault com uma nota na caixa: ela e salva no vault em que comecou
    // (nunca no novo) e a caixa limpa.
    let vault_root = Vault { root: root_dir.to_path_buf() };
    let body_of = |v: &Vault, text: &str| -> Vec<String> {
        v.list_notes().unwrap_or_default().into_iter().filter(|(_, c)| c == text).map(|(p, _)| p.id).collect()
    };
    // O vault do smoke pode ter o mesmo rascunho de uma rodada anterior: conta so os desta rodada.
    let of_before = body_of(&vault_root, "rascunho do vault");
    key(view_window, cx, "/").await?;
    typing(view_window, cx, "rascunho do vault").await?;
    key(view_window, cx, "escape").await?;
    wait_idle(view_window, cx).await?;
    let other = base.join("vault-b");
    std::fs::create_dir_all(&other).map_err(|e| e.to_string())?;
    let missing = base.join("nao-existe");
    view_window.update(cx, |v, w, cx| v.switch_vault(missing.clone(), false, w, cx)).map_err(|e| e.to_string())?;
    wait_idle(view_window, cx).await?;
    let (still, warning) = view_window
        .update(cx, |v, _, _| (v.state().vault.as_ref().map(|v| v.root.clone()), v.state().warning.clone()))
        .map_err(|e| e.to_string())?;
    println!("smoke: trocar para {} -> vault {:?}, aviso {warning:?}", missing.display(), still);
    if still.as_deref() != Some(root_dir) || warning.is_none() {
        return Err("vault inexistente trocou o vault ativo".into());
    }
    view_window.update(cx, |v, w, cx| v.switch_vault(other.clone(), true, w, cx)).map_err(|e| e.to_string())?;
    wait_idle(view_window, cx).await?;
    wait_lists(view_window, cx).await?;
    let (active, sources, draft, source_has) = view_window
        .update(cx, |v, _, cx| {
            (
                v.state().vault.as_ref().map(|v| v.root.clone()),
                v.drawers().sources().len(),
                v.quick_note().read(cx).text().to_string(),
                v.state().source_has(),
            )
        })
        .map_err(|e| e.to_string())?;
    let no_old: Vec<String> =
        body_of(&vault_root, "rascunho do vault").into_iter().filter(|id| !of_before.contains(id)).collect();
    let no_new = body_of(&Vault { root: other.clone() }, "rascunho do vault");
    println!(
        "smoke: Criar vault aqui {} -> ativo {active:?}, {sources} fonte(s), caixa {draft:?}, fonte aberta {source_has}; \
         nota no vault anterior {no_old:?}, no novo {no_new:?}",
        other.display()
    );
    if active.as_deref() != Some(other.as_path()) || sources != 0 || !draft.is_empty() || source_has || !other.join("notas").is_dir() {
        return Err("trocar para o vault novo falhou".into());
    }
    if no_old.len() != 1 || !no_old[0].contains("-rascunho-do-vault") || !no_new.is_empty() {
        return Err("a nota em edicao devia ficar no vault anterior (e so nele)".into());
    }
    // Cmd-Shift-V abre o seletor (com os dois vaults); Esc fecha; volta ao vault do smoke.
    key(view_window, cx, "cmd-shift-v").await?;
    let (is_open, known_all) = view_window
        .update(cx, |v, _, _| (v.picker().is_open(), v.state().app_data.known_vaults().to_vec()))
        .map_err(|e| e.to_string())?;
    key(view_window, cx, "escape").await?;
    let closed = !view_window.update(cx, |v, _, _| v.picker().is_open()).map_err(|e| e.to_string())?;
    println!("smoke: Cmd-Shift-V abre o seletor {is_open} com {} vault(s) conhecidos; Esc fecha {closed}", known_all.len());
    if !is_open || !closed || !known_all.iter().any(|k| k == other.as_path()) || !known_all.iter().any(|k| k == root_dir) {
        return Err("seletor de vault pelo Cmd-Shift-V falhou".into());
    }
    // Uma fonte abrindo no vault B quando a troca de volta comeca, e uma segunda
    // troca pedida no meio da primeira (recusada: um trabalho de cada vez). A
    // abertura que chegar depois da troca e descartada; o viewer nao fica preso
    // em "carregando".
    let (refused, warning) = view_window
        .update(cx, |v, w, cx| {
            v.open_path(moved.to_path_buf(), new.clone(), ZipMode::Direct, w, cx);
            v.switch_vault(root_dir.to_path_buf(), false, w, cx);
            v.switch_vault(other.clone(), false, w, cx);
            (v.state().warning.clone().is_some_and(|a| a.contains("aguarde")), v.state().warning.clone())
        })
        .map_err(|e| e.to_string())?;
    wait_idle(view_window, cx).await?;
    wait_lists(view_window, cx).await?;
    let (active, loading, source, map_b) = view_window
        .update(cx, |v, _, cx| {
            (
                v.state().vault.as_ref().map(|v| v.root.clone()),
                v.viewer().read(cx).loading(),
                v.state().source_has().then(|| v.state().current.image.source.clone()),
                v.state().app_data.source_path(&other, &new),
            )
        })
        .map_err(|e| e.to_string())?;
    // Se a abertura chegou antes da troca terminar, entrou no vault B (e a fonte
    // continua, porque tambem e do vault do smoke); se chegou depois, sumiu.
    let order = if map_b.is_some() { "abertura antes da troca" } else { "abertura depois da troca (descartada)" };
    println!(
        "smoke: abrir no vault B + trocar de vault no meio: {order}; ativo {active:?}, fonte {source:?}, carregando {loading}; \
         segunda troca recusada {refused} ({warning:?})"
    );
    let consistent = if map_b.is_some() { source.as_deref() == Some(new.as_str()) } else { source.is_none() };
    if active.as_deref() != Some(root_dir) || loading || !refused || !consistent {
        return Err("abertura no meio da troca de vault deixou estado errado".into());
    }
    let of_back = view_window
        .update(cx, |v, _, _| {
            v.state().vault.as_ref().map(|v| v.root.clone()) == Some(root_dir.to_path_buf())
                && v.drawers().sources().iter().any(|f| f.name == new)
        })
        .map_err(|e| e.to_string())?;
    println!("smoke: de volta ao vault {} com a fonte renomeada: {of_back}", root_dir.display());
    if !of_back {
        return Err("voltar ao vault do smoke falhou".into());
    }
    // Reabre a fonte (fechada na troca) e esvazia a caixa para o resto do smoke.
    view_window
        .update(cx, |v, w, cx| v.open_path(moved.to_path_buf(), new.clone(), ZipMode::Direct, w, cx))
        .map_err(|e| e.to_string())?;
    wait(view_window, cx).await?;
    view_window.update(cx, |v, _, cx| v.quick_note().update(cx, |n, cx| n.clear(cx))).map_err(|e| e.to_string())?;
    Ok(())
}
