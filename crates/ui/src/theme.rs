//! Cores do app por **papel** (fundo, borda, texto, acento, aviso...), num lugar so e sem gpui:
//! o mesmo estado visual tem sempre a mesma cor em todo o app, e cada tema (escuro, claro,
//! sepia) e uma [`Palette`] completa. A view le `theme::current().<papel>` na hora de desenhar;
//! trocar o tema (Configuracoes) so muda qual paleta `current` devolve e repinta a janela.
//!
//! Regras:
//! - nenhuma cor de tema fica escrita fora deste arquivo (o teste `sem_cor_solta` confere); o que
//!   nao muda com o tema (as cores dos grifos, a alca de selecao, o vermelho dos botoes de perigo,
//!   as sombras) fica fixo no lugar onde e usado e entra na lista do teste;
//! - cor nova = papel novo nas tres paletas (o teste de contraste cobre texto sobre fundo).

use marginalia_core::app_data::Theme;
use std::sync::atomic::{AtomicU8, Ordering};

/// Todas as cores de tema, uma por papel (`0xRRGGBB`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    // Superficies.
    /// Fundo da janela e do visualizador.
    pub background: u32,
    /// Barras (titulo, status, ferramentas), paineis e telas sobrepostas.
    pub bar: u32,
    /// Barra de atividades e fundo de linha de gaveta.
    pub lateral: u32,
    /// Cartoes (convite para abrir uma fonte, seletor de vault).
    pub card: u32,
    /// Listas e menus que abrem por cima (suspensos, "Promover como").
    pub raised: u32,
    /// Campos onde se digita e listas fundas.
    pub field: u32,
    /// Mouse em cima de algo sem fundo proprio; tambem o fundo da dica (tooltip).
    pub hover_soft: u32,
    /// Botao solido e o mesmo com o mouse em cima.
    pub btn: u32,
    pub button_hover: u32,
    // Linhas e controles.
    pub border: u32,
    pub strong_border: u32,
    pub soft_border: u32,
    /// Trilha das barras e borda da dica.
    pub track: u32,
    /// Controle em repouso (barra de rolagem) e em destaque (mouse em cima, guia, marca do meio).
    pub control: u32,
    pub control_hover: u32,
    // Texto.
    pub text: u32,
    /// Texto realcado (opcao ativa, icone ligado).
    pub strong_text: u32,
    pub medium_text: u32,
    pub soft_text: u32,
    pub weak_text: u32,
    /// Texto sobre uma superficie de acento preenchida.
    pub accent_about: u32,
    // Acento (ligado, selecionado, foco).
    pub accent: u32,
    pub accent_hover: u32,
    /// Fundo de linha selecionada e de opcao ativa.
    pub background_accent: u32,
    /// Botao de ferramenta ligado: fundo e borda.
    pub linked_background: u32,
    pub linked_border: u32,
    /// Realce ao soltar um item na janela: fundo e borda.
    pub drop_background: u32,
    pub drop_border: u32,
    /// Titulo de grupo da ajuda e cursor da nota.
    pub emphasis: u32,
    /// Borda da gaveta aberta.
    pub open_mark: u32,
    // Estados.
    pub warning: u32,
    pub background_warning: u32,
    pub border_warning: u32,
    pub ok: u32,
    pub error: u32,
    pub background_error: u32,
    pub border_error: u32,
    pub text_error: u32,
}

/// O visual de sempre.
pub const DARK: Palette = Palette {
    background: 0x202020,
    bar: 0x252525,
    lateral: 0x2c2c2c,
    card: 0x2a2a2a,
    raised: 0x303030,
    field: 0x181818,
    hover_soft: 0x3a3a3a,
    btn: 0x3d3d3d,
    button_hover: 0x505050,
    border: 0x3c3c3c,
    strong_border: 0x4a4a4a,
    soft_border: 0x2f2f2f,
    track: 0x5a5a5a,
    control: 0x6a6a6a,
    control_hover: 0x8a8a8a,
    text: 0xe0e0e0,
    strong_text: 0xffffff,
    medium_text: 0xc0c0c0,
    soft_text: 0x9a9a9a,
    weak_text: 0x808080,
    accent_about: 0xffffff,
    accent: 0x5b8def,
    accent_hover: 0x7aa2f7,
    background_accent: 0x34405a,
    linked_background: 0x2b4a7a,
    linked_border: 0x4a7ac0,
    drop_background: 0x2a3a4a,
    drop_border: 0x4a90d9,
    emphasis: 0x8fb0ff,
    open_mark: 0xff9a4a,
    warning: 0xf0c060,
    background_warning: 0x4a3a1a,
    border_warning: 0x806040,
    ok: 0x7ad67a,
    error: 0xff6a6a,
    background_error: 0x4a2828,
    border_error: 0xc04848,
    text_error: 0xffd0d0,
};

pub const LIGHT: Palette = Palette {
    background: 0xe8e8e8,
    bar: 0xf5f5f5,
    lateral: 0xebebeb,
    card: 0xffffff,
    raised: 0xffffff,
    field: 0xffffff,
    hover_soft: 0xdedede,
    btn: 0xdcdcdc,
    button_hover: 0xc8c8c8,
    border: 0xd2d2d2,
    strong_border: 0xbdbdbd,
    soft_border: 0xe4e4e4,
    track: 0xb4b4b4,
    control: 0x9a9a9a,
    control_hover: 0x7a7a7a,
    text: 0x1f1f1f,
    strong_text: 0x000000,
    medium_text: 0x454545,
    soft_text: 0x626262,
    weak_text: 0x6f6f6f,
    accent_about: 0xffffff,
    accent: 0x2563eb,
    accent_hover: 0x4a82f0,
    background_accent: 0xcddcfa,
    linked_background: 0xd3e1fb,
    linked_border: 0x7aa2ee,
    drop_background: 0xdbe8fb,
    drop_border: 0x4a90d9,
    emphasis: 0x1d4ed8,
    open_mark: 0xd9731a,
    warning: 0x8a5200,
    background_warning: 0xfff1c9,
    border_warning: 0xe0b45a,
    ok: 0x1b6e34,
    error: 0xc62828,
    background_error: 0xfbdcdc,
    border_error: 0xd46a6a,
    text_error: 0x8f1d1d,
};

pub const SEPIA: Palette = Palette {
    background: 0xe4d7ba,
    bar: 0xefe3c9,
    lateral: 0xe7dabd,
    card: 0xf7edd6,
    raised: 0xf9f0dc,
    field: 0xfbf5e6,
    hover_soft: 0xdccfae,
    btn: 0xd9caa6,
    button_hover: 0xc8b78d,
    border: 0xd3c3a0,
    strong_border: 0xbfae86,
    soft_border: 0xe0d3b4,
    track: 0xbba97f,
    control: 0xa2916a,
    control_hover: 0x8a7850,
    text: 0x3a2e1c,
    strong_text: 0x241a0c,
    medium_text: 0x5c4c34,
    soft_text: 0x6f5f42,
    weak_text: 0x7b6a4a,
    accent_about: 0xffffff,
    accent: 0x8a5a1f,
    accent_hover: 0xa06a28,
    background_accent: 0xe2cf9c,
    linked_background: 0xe8d7a8,
    linked_border: 0xb8903e,
    drop_background: 0xeadcb4,
    drop_border: 0xb8903e,
    emphasis: 0x7a4a12,
    open_mark: 0xb8610f,
    warning: 0x8a5200,
    background_warning: 0xf6e2a8,
    border_warning: 0xc9a050,
    ok: 0x3d6b22,
    error: 0xb3261e,
    background_error: 0xf3d0c4,
    border_error: 0xc0705a,
    text_error: 0x7e2a1e,
};

/// A paleta de um tema.
pub fn palette(t: Theme) -> &'static Palette {
    match t {
        Theme::Dark => &DARK,
        Theme::Light => &LIGHT,
        Theme::Sepia => &SEPIA,
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

fn code(t: Theme) -> u8 {
    match t {
        Theme::Dark => 0,
        Theme::Light => 1,
        Theme::Sepia => 2,
    }
}

/// O tema que a janela usa agora. Escolhido na partida e pelas Configuracoes (`set`).
pub fn current_theme() -> Theme {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Theme::Light,
        2 => Theme::Sepia,
        _ => Theme::Dark,
    }
}

/// Troca o tema. Quem chama repinta as janelas.
pub fn set(t: Theme) {
    CURRENT.store(code(t), Ordering::Relaxed);
}

/// A paleta do tema atual: `rgb(theme::current().border)`.
pub fn current() -> &'static Palette {
    palette(current_theme())
}

#[cfg(test)]
#[path = "tests/theme.rs"]
mod tests;
