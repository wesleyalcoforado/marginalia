//! Layout estilo VS Code, sem gpui (regra R4): qual painel esta encaixado a
//! esquerda (as gavetas de [`drawers`]), se o painel de ajustes da direita esta
//! aberto, quanto sobra para a imagem e o texto da barra de status.
//!
//! Da esquerda para a direita: barra de atividades ([`ACTIVITIES`] px, icones
//! Notas, Fontes, Tarefas e a engrenagem embaixo), o painel esquerdo (no
//! maximo [`drawers::WIDTH`] px), a imagem, o painel de ajustes (no maximo
//! [`ADJUSTMENTS_PANEL`] px) e a barra fina da direita ([`RIGHT_BAR`] px, com
//! o icone de ajustes sempre visivel). Os paineis encaixam (nao ficam por cima
//! da imagem): a area do visualizador encolhe, e como o visualizador mede a
//! area a cada quadro, zoom, arrasto e grifos seguem alinhados.
//!
//! O estado de qual painel esquerdo esta aberto mora em [`Drawers`] (as
//! gavetas sao os paineis); [`Layout`] e a fotografia dele mais o painel de
//! ajustes, e e aqui que mora a regra "um painel esquerdo por vez, o mesmo
//! icone fecha" que [`Drawers::toggle`] usa.

use crate::drawers::{self, Drawer, Drawers};
use texts::t;

/// Barra de atividades (esquerda), px.
pub const ACTIVITIES: f32 = 48.0;
/// Painel esquerdo aberto (as gavetas), px.
pub const LEFT_PANEL: f32 = drawers::WIDTH;
/// Barra fina da direita (icone de ajustes), px.
pub const RIGHT_BAR: f32 = 40.0;
/// Painel de ajustes aberto (direita), px.
pub const ADJUSTMENTS_PANEL: f32 = 280.0;
/// Largura minima garantida a imagem quando a janela tem espaco para ela.
/// Numa janela estreita os paineis cedem (primeiro o esquerdo, depois o de
/// ajustes) ate a imagem ter isto; as duas barras fixas nunca cedem.
pub const VIEWER_MIN: f32 = 200.0;
/// Caracteres de um nome (vault, fonte, arquivo) na barra de status antes do `…`.
pub const MAX_NAME: usize = 40;

/// Paineis encaixados: um a esquerda (ou nenhum) e o de ajustes a direita.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Layout {
    pub left: Option<Drawer>,
    pub open_adjustments: bool,
}

impl Layout {
    /// O layout atual: o painel esquerdo e a gaveta aberta em `g`.
    pub fn de(g: &Drawers, open_adjustments: bool) -> Layout {
        Layout { left: g.open(), open_adjustments }
    }

    /// Clique no icone `g` da barra de atividades: abre `g` (trocando o que
    /// estiver aberto) ou fecha, se `g` ja era o painel aberto.
    pub fn toggle_left(&mut self, g: Drawer) {
        self.left = Drawer::toggled(self.left, g);
    }

    /// Icone de ajustes (ou `F`): abre/fecha o painel da direita. Nao mexe no esquerdo.
    pub fn toggle_adjustments(&mut self) {
        self.open_adjustments = !self.open_adjustments;
    }
}

/// Larguras de cada coluna para uma janela de `view_window` px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Widths {
    pub left: f32,
    pub viewer: f32,
    pub image_adjustments: f32,
}

/// Divide `view_window` px entre os paineis abertos e a imagem. Nunca negativo: as
/// barras fixas ficam com o que couber; a imagem fica com pelo menos
/// `min(VIEWER_MIN, o que sobra das barras)`; os paineis ficam com o resto, o
/// de ajustes primeiro (o esquerdo cede antes). Com espaco, cada painel tem a
/// largura cheia e a imagem o resto.
pub fn widths(view_window: f32, l: &Layout) -> Widths {
    widths_with(view_window, l, LEFT_PANEL)
}

/// Como [`widths`], com o painel esquerdo na largura que o usuario escolheu.
pub fn widths_with(view_window: f32, l: &Layout, left_full: f32) -> Widths {
    let view_window = finite(view_window);
    let available = (view_window - ACTIVITIES - RIGHT_BAR).max(0.0);
    let min = VIEWER_MIN.min(available);
    let mut rest = available - min;
    let image_adjustments = if l.open_adjustments { ADJUSTMENTS_PANEL.min(rest) } else { 0.0 };
    rest -= image_adjustments;
    let left = if l.left.is_some() { left_full.min(rest) } else { 0.0 };
    Widths { left, viewer: (available - image_adjustments - left).max(0.0), image_adjustments }
}

/// Largura da area da imagem numa janela de `view_window` px (ver [`widths`]).
pub fn width_viewer(view_window: f32, l: &Layout) -> f32 {
    widths(view_window, l).viewer
}

/// Como [`width_viewer`], com a largura escolhida do painel esquerdo.
pub fn width_viewer_with(view_window: f32, l: &Layout, left_full: f32) -> f32 {
    widths_with(view_window, l, left_full).viewer
}

/// Largura do painel esquerdo para o ponteiro em `x` (px da janela): a borda
/// direita do painel acompanha o ponteiro, limitada a [`drawers::WIDTH_MIN`]..[`drawers::WIDTH_MAX`].
pub fn width_to_point(x: f32) -> f32 {
    if !x.is_finite() {
        return drawers::WIDTH;
    }
    (x - ACTIVITIES).clamp(drawers::WIDTH_MIN, drawers::WIDTH_MAX)
}

/// Altura da barra de status (px): cabe um botao de icone de 28 px.
pub const STATUS_HEIGHT: f32 = 28.0;
/// Altura da barra de titulo (px), que o app pinta sob os botoes do macOS (titulo nativo
/// transparente, `main.rs`). Acompanha a posicao dos botoes (`TRAFFIC_LIGHT` em `main.rs`:
/// centro deles na metade desta altura); mexer em uma pede mexer na outra.
pub const TITLE_HEIGHT: f32 = 32.0;
/// Altura da barra de menus (px) fora do macOS, onde o sistema nao a desenha: o app a pinta logo
/// abaixo da barra de titulo (`MainView::menu_bar`).
pub const MENU_HEIGHT: f32 = 28.0;
/// Altura da barra de ferramentas (px), logo abaixo da barra de titulo, sobre a imagem.
pub const TOOLS_HEIGHT: f32 = 32.0;
/// O que a esquerda da barra de status (botoes e o texto vault · fonte · ...)
/// guarda para si quando a janela permite.
pub const STATUS_MIN: f32 = 240.0;
/// Largura cheia do seletor de vault (px) e margem dele ate a borda da janela.
pub const PICKER: f32 = 460.0;
pub const PICKER_MARGIN: f32 = 8.0;

/// Ate quanto a caixa de mensagens (direita da barra de status: aviso,
/// confirmacao, carregando, Desfazer) pode crescer numa janela de `view_window` px.
/// As mensagens ficam com a largura natural ate aqui; quem encolhe (e corta com
/// `…`) e o texto vault · fonte · ..., que fica com pelo menos [`STATUS_MIN`]
/// quando a janela permite. Mensagem mais longa que isto e cortada, e o texto
/// inteiro fica na dica (tooltip). Nunca negativo.
pub fn width_max_messages(view_window: f32) -> f32 {
    (finite(view_window) - STATUS_MIN).max(0.0)
}

/// `view_window` sem NaN/infinito nem negativo.
fn finite(view_window: f32) -> f32 {
    if view_window.is_finite() { view_window.max(0.0) } else { 0.0 }
}

/// Posicao e largura do seletor de vault, ancorado na janela: `(esquerda,
/// largura)`. Com espaco, comeca depois da barra de atividades e tem
/// [`PICKER`] px; numa janela estreita encosta na margem e encolhe para caber
/// (nunca passa da borda, nunca negativo).
pub fn picker_textbox(view_window: f32) -> (f32, f32) {
    let j = finite(view_window);
    let bar_after = ACTIVITIES + PICKER_MARGIN;
    let x = if bar_after + PICKER + PICKER_MARGIN <= j { bar_after } else { PICKER_MARGIN.min(j) };
    (x, (j - x - PICKER_MARGIN).clamp(0.0, PICKER))
}

/// O que a barra de status mostra. Partes ausentes somem do texto.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StatusInfo {
    pub vault: Option<String>,
    pub source: Option<String>,
    pub page: Option<u32>,
    pub total_pages: Option<u32>,
    pub index: Option<usize>,
    pub total_images: Option<usize>,
    pub file: Option<String>,
}

/// Nome curto de uma pasta (o ultimo trecho; o caminho inteiro se nao ha).
pub fn folder_name(p: &std::path::Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

/// Uma mensagem passageira da barra de status, na ordem em que aparece.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusMessage {
    /// Em andamento (abrindo, carregando, trabalho exclusivo): cor neutra.
    Progress(String),
    Confirmation(String),
    Warning(String),
}

/// `vault: X · fonte: Y · pág 3/12 · img 42/300 · arquivo.jpg`.
///
/// - sem vault: `sem vault`; sem fonte: `nenhuma fonte aberta` (e nada de
///   pagina, imagem ou arquivo);
/// - pagina: so com mais de uma pagina (ou total ainda desconhecido: `pág 3/?`);
///   imagem de uma pagina so (`total_paginas == Some(1)`) nao mostra `pág`;
/// - `img i/n` so com os dois numeros;
/// - nomes com mais de [`MAX_NAME`] caracteres sao cortados no meio
///   ([`shorten`]), para o comeco e a extensao continuarem visiveis.
pub fn text_status(s: &StatusInfo) -> String {
    let mut parts = vec![match &s.vault {
        Some(v) => t!("status.vault", name = shorten(v, MAX_NAME)),
        None => t!("status.no_vault"),
    }];
    let Some(source) = &s.source else {
        parts.push(t!("status.source_no"));
        return parts.join(" · ");
    };
    parts.push(t!("status.source", name = shorten(source, MAX_NAME)));
    if let Some(p) = s.page {
        match s.total_pages {
            Some(1) => {}
            Some(total) => parts.push(t!("status.page", p = p, total = total)),
            None => parts.push(t!("status.page_incerta", p = p)),
        }
    }
    if let (Some(i), Some(n)) = (s.index, s.total_images) {
        parts.push(t!("status.image", i = i, n = n));
    }
    if let Some(a) = s.file.as_deref().filter(|a| !a.is_empty()) {
        parts.push(shorten(a, MAX_NAME));
    }
    parts.join(" · ")
}

/// Corta `s` para `max` caracteres (nao bytes) com `…` no meio: a primeira
/// metade (arredondada para baixo) do que cabe e o fim do nome. Ate `max`
/// caracteres fica como esta.
pub fn shorten(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max || max < 2 {
        return s.to_string();
    }
    let fits = max - 1;
    let start = fits / 2;
    let end = fits - start;
    let a: String = s.chars().take(start).collect();
    let b: String = s.chars().skip(n - end).collect();
    format!("{a}…{b}")
}

#[cfg(test)]
#[path = "tests/layout.rs"]
mod tests;
