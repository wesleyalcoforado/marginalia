use super::*;

fn empty() -> Layout {
    Layout::default()
}

#[test]
fn tools_height_fits() {
    assert!(TOOLS_HEIGHT >= 28.0 && TOOLS_HEIGHT <= TITLE_HEIGHT, "{TOOLS_HEIGHT}");
}

#[test]
fn same_icon_closes_and_other_swap() {
    let mut l = empty();
    l.toggle_left(Drawer::Notes);
    assert_eq!(l.left, Some(Drawer::Notes));
    l.toggle_left(Drawer::Notes);
    assert_eq!(l.left, None, "o mesmo icone fecha");
    l.toggle_left(Drawer::Sources);
    l.toggle_left(Drawer::Tasks);
    assert_eq!(l.left, Some(Drawer::Tasks), "outro icone troca o painel (um so por vez)");
}

#[test]
fn adjustments_independent_of_the_panel_left() {
    let mut l = empty();
    l.toggle_adjustments();
    assert!(l.open_adjustments);
    l.toggle_left(Drawer::Notes);
    assert_eq!(l, Layout { left: Some(Drawer::Notes), open_adjustments: true });
    l.toggle_left(Drawer::Notes);
    assert!(l.open_adjustments, "fechar o esquerdo nao fecha os ajustes");
    l.toggle_left(Drawer::Sources);
    l.toggle_adjustments();
    assert_eq!(l, Layout { left: Some(Drawer::Sources), open_adjustments: false });
}

#[test]
fn drawers_follow_the_same_rule() {
    let mut g = Drawers::default();
    assert!(g.toggle(Drawer::Notes));
    assert!(g.toggle(Drawer::Sources));
    assert_eq!(Layout::de(&g, true), Layout { left: Some(Drawer::Sources), open_adjustments: true });
    assert!(!g.toggle(Drawer::Sources));
    assert_eq!(Layout::de(&g, false), empty());
}

#[test]
fn width_viewer_recomputes_in_each_combination() {
    let j = 1200.0;
    let base = j - ACTIVITIES - RIGHT_BAR;
    assert_eq!(width_viewer(j, &empty()), base);
    for g in Drawer::ALL {
        let l = Layout { left: Some(g), open_adjustments: false };
        assert_eq!(width_viewer(j, &l), base - LEFT_PANEL, "{g:?}");
        let l = Layout { left: Some(g), open_adjustments: true };
        assert_eq!(width_viewer(j, &l), base - LEFT_PANEL - ADJUSTMENTS_PANEL, "{g:?} + ajustes");
    }
    let l = Layout { left: None, open_adjustments: true };
    assert_eq!(width_viewer(j, &l), base - ADJUSTMENTS_PANEL);
    let w = widths(j, &Layout { left: Some(Drawer::Notes), open_adjustments: true });
    assert_eq!(w.left + w.viewer + w.image_adjustments + ACTIVITIES + RIGHT_BAR, j, "as colunas somam a janela");
}

#[test]
fn window_narrow_never_stays_negative() {
    let all = Layout { left: Some(Drawer::Tasks), open_adjustments: true };
    for j in [0.0, 10.0, 60.0, 88.0, 100.0, 300.0, 500.0, 700.0, -50.0, f32::NAN] {
        let w = widths(j, &all);
        assert!(w.viewer >= 0.0 && w.left >= 0.0 && w.image_adjustments >= 0.0, "{j}: {w:?}");
    }
    // Com espaco para as barras a imagem fica com VIEWER_MIN e os paineis cedem.
    let w = widths(500.0, &all);
    assert_eq!(w.viewer, VIEWER_MIN);
    assert_eq!(w.image_adjustments, 500.0 - ACTIVITIES - RIGHT_BAR - VIEWER_MIN, "o de ajustes fica com o resto");
    assert_eq!(w.left, 0.0, "o esquerdo cede primeiro");
    let w = widths(700.0, &all);
    assert_eq!((w.viewer, w.image_adjustments), (VIEWER_MIN, ADJUSTMENTS_PANEL));
    assert_eq!(w.left, 700.0 - ACTIVITIES - RIGHT_BAR - VIEWER_MIN - ADJUSTMENTS_PANEL);
    // Menor que as barras: a imagem fica com o que sobrar (zero), nunca negativa.
    assert_eq!(width_viewer(60.0, &all), 0.0);
    assert_eq!(width_viewer(150.0, &empty()), 150.0 - ACTIVITIES - RIGHT_BAR);
}

#[test]
fn messages_stay_with_the_width_and_the_text_shrinks() {
    // Janela larga: as mensagens podem ocupar tudo menos o minimo do texto.
    assert_eq!(width_max_messages(1200.0), 1200.0 - STATUS_MIN);
    assert!(width_max_messages(1200.0) > 800.0, "um aviso longo cabe inteiro numa janela comum");
    // Mais larga, mais espaco: cresce com a janela.
    assert!(width_max_messages(1600.0) > width_max_messages(1200.0));
    // Estreita: nunca negativo.
    for j in [0.0, 100.0, STATUS_MIN, -10.0, f32::NAN] {
        let m = width_max_messages(j);
        assert!(m >= 0.0, "{j}: {m}");
    }
    assert_eq!(width_max_messages(STATUS_MIN), 0.0);
}

#[test]
fn picker_fits_in_the_window() {
    let (x, w) = picker_textbox(1200.0);
    assert_eq!((x, w), (ACTIVITIES + PICKER_MARGIN, PICKER), "com espaco: depois da barra de atividades");
    for j in [0.0, 50.0, 300.0, 500.0, 520.0, 800.0, -5.0, f32::NAN] {
        let (x, w) = picker_textbox(j);
        let jj = if j.is_finite() { j.max(0.0) } else { 0.0 };
        assert!(x >= 0.0 && w >= 0.0 && w <= PICKER, "{j}: {x} {w}");
        assert!(x + w <= jj.max(x), "{j}: passa da borda ({x} + {w})");
    }
    let (x, w) = picker_textbox(300.0);
    assert_eq!((x, w), (PICKER_MARGIN, 300.0 - 2.0 * PICKER_MARGIN), "estreita: encosta e encolhe");
}

fn complete() -> StatusInfo {
    StatusInfo {
        vault: Some("pesquisa".into()),
        source: Some("livro-1890".into()),
        page: Some(3),
        total_pages: Some(12),
        index: Some(42),
        total_images: Some(300),
        file: Some("arquivo.jpg".into()),
    }
}

#[test]
fn complete_status() {
    assert_eq!(text_status(&complete()), "vault: pesquisa · fonte: livro-1890 · pág 3/12 · img 42/300 · arquivo.jpg");
}

#[test]
fn status_no_vault_no_source_no_file() {
    assert_eq!(text_status(&StatusInfo::default()), "sem vault · nenhuma fonte aberta");
    let s = StatusInfo { vault: Some("v".into()), ..Default::default() };
    assert_eq!(text_status(&s), "vault: v · nenhuma fonte aberta");
    let s = StatusInfo { vault: None, ..complete() };
    assert_eq!(text_status(&s), "sem vault · fonte: livro-1890 · pág 3/12 · img 42/300 · arquivo.jpg");
    let s = StatusInfo { file: None, index: None, ..complete() };
    assert_eq!(text_status(&s), "vault: pesquisa · fonte: livro-1890 · pág 3/12");
    let s = StatusInfo { total_pages: None, ..complete() };
    assert!(text_status(&s).contains("pág 3/? · "), "total ainda desconhecido");
}

#[test]
fn status_with_a_page_omits_page() {
    let s = StatusInfo { page: Some(1), total_pages: Some(1), ..complete() };
    assert_eq!(text_status(&s), "vault: pesquisa · fonte: livro-1890 · img 42/300 · arquivo.jpg");
}

#[test]
fn name_very_long_and_cut_in_the_middle() {
    let long = format!("{}fim.jpg", "á".repeat(200));
    let s = StatusInfo { file: Some(long.clone()), ..complete() };
    let t = text_status(&s);
    let name = t.rsplit(" · ").next().unwrap();
    assert_eq!(name.chars().count(), MAX_NAME);
    assert!(name.starts_with("ááá") && name.ends_with("fim.jpg") && name.contains('…'), "{name}");
    assert_eq!(shorten("curto.jpg", MAX_NAME), "curto.jpg");
    assert_eq!(shorten("abcdefghij", 5), "ab…ij");
    let s = StatusInfo { vault: Some("v".repeat(100)), source: Some("f".repeat(100)), ..complete() };
    assert!(text_status(&s).chars().count() < 2 * MAX_NAME + 60);
}

#[test]
fn name_of_the_folder_and_the_last_stretch_or_the_path_whole() {
    use std::path::Path;
    assert_eq!(folder_name(Path::new("/a/b/vault")), "vault");
    assert_eq!(folder_name(Path::new("/")), "/");
}

#[test]
fn widths_with_uses_the_width_chosen_and_gives_in_window_narrow() {
    let l = Layout { left: Some(Drawer::Notes), open_adjustments: false };
    let w = widths_with(1600.0, &l, 480.0);
    assert_eq!((w.left, w.viewer), (480.0, 1600.0 - ACTIVITIES - RIGHT_BAR - 480.0));
    // janela estreita: o painel cede ate a imagem ter VIEWER_MIN
    let w = widths_with(500.0, &l, 480.0);
    assert!(w.viewer >= VIEWER_MIN - 0.01 && w.left < 480.0, "{w:?}");
    // a versao sem largura usa o padrao
    assert_eq!(widths(1600.0, &l), widths_with(1600.0, &l, LEFT_PANEL));
    assert_eq!(width_viewer_with(1600.0, &l, 480.0), widths_with(1600.0, &l, 480.0).viewer);
}

#[test]
fn width_to_point_counts_the_start_of_the_bar_of_activities() {
    assert_eq!(width_to_point(ACTIVITIES + 400.0), 400.0);
    assert_eq!(width_to_point(0.0), drawers::WIDTH_MIN);
    assert_eq!(width_to_point(9000.0), drawers::WIDTH_MAX);
}
