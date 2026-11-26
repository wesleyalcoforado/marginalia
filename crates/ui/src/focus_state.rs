//! Foco do teclado entre a imagem, a nota rapida, a gaveta e o resto, as teclas
//! W/A/S/D e as da gaveta, sem gpui (regra R4). A view so traduz: qual
//! `FocusHandle` esta focado -> [`Focus`], a tecla -> [`transition`] /
//! [`pan_key`] / [`drawer_key`].

use crate::drawers::{DrawerAction, Drawer};

/// Quem tem o foco do teclado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    /// O visualizador (borda azul).
    Image,
    /// A caixa de nota rapida.
    QuickNote,
    /// Outro campo de texto (caixa "Renomear fonte") ou nada focado.
    Other,
    /// A gaveta aberta no painel esquerdo (as teclas dela valem).
    Drawer,
    /// A caixa numerica de uma barra do painel de ajustes (campo de texto).
    Number,
}

/// As teclas sao texto: a nota rapida, a caixa numerica de uma barra ou a
/// caixa "Renomear" (`rename`, que a view ve como [`Focus::Other`]) tem o
/// foco. Ai os atalhos de uma letra, W/A/S/D e as teclas da gaveta nao valem.
pub fn text_field(focus_state: Focus, rename: bool) -> bool {
    rename || matches!(focus_state, Focus::QuickNote | Focus::Number)
}

/// Tecla -> acao da gaveta aberta, SO com a gaveta em foco (fix round 1: com a
/// imagem em foco as teclas sao da imagem, e W/A/S/D nunca sao da gaveta).
/// Sem modificador: `drawers::drawer_key` (Backspace pede a exclusao); com Cmd nada.
pub fn drawer_key(
    key: &str,
    focus_state: Focus,
    open: Option<Drawer>,
    shift: bool,
    cmd: bool,
    ctrl: bool,
    alt: bool,
) -> Option<DrawerAction> {
    let g = open.filter(|_| focus_state == Focus::Drawer)?;
    if shift || ctrl || alt {
        return None;
    }
    if cmd {
        return None;
    }
    crate::drawers::drawer_key(g, key)
}

/// A tecla segurada (repeticao do teclado) vale para esta acao? So andar pela
/// lista repete: Enter e Backspace nao abrem nem pedem
/// exclusao de novo (fix round 2).
pub fn repetition_accepts(a: DrawerAction) -> bool {
    matches!(a, DrawerAction::Relocate(_))
}

/// O foco que vai para a caixa quando o texto pedido (clique ou Enter numa
/// linha) chegar. Antes disso a caixa fica desligada: focada, ela engoliria o
/// que se digitasse (com a carga adiada de um zip, segundos de texto perdido).
/// Ate la o foco fica onde estava (a gaveta); se a carga falhar ou for
/// cancelada, o pedido cai e o foco continua la.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FocusToTheLoad {
    pending: bool,
}

impl FocusToTheLoad {
    /// Clique/Enter numa linha: focar a caixa quando o texto chegar.
    pub fn request(&mut self) {
        self.pending = true;
    }

    /// Ha um pedido esperando o texto.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn pending(&self) -> bool {
        self.pending
    }

    /// Depois de cada efeito: `arrived` (o texto esta na caixa) e `loading`
    /// (a sessao ou a abertura da fonte ainda trazem o texto). `true`: focar a
    /// caixa agora (uma vez so).
    pub fn after(&mut self, arrived: bool, loading: bool) -> bool {
        if !self.pending {
            return false;
        }
        if arrived || !loading {
            self.pending = false;
        }
        arrived
    }
}

/// Comandos de foco.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Cmd-K (ou `/` com a imagem em foco): a nota rapida.
    FocusQuickNote,
    /// Cmd-Shift-K: a imagem, de qualquer foco.
    FocusImage,
    /// Esc: na nota volta a imagem; fora dela nao mexe no foco (Esc nunca sai do app).
    Esc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transition {
    To(Focus),
    /// O foco fica onde esta.
    Stays,
    /// A caixa esta desligada (sem vault / sem fonte): o foco fica e a barra mostra a dica.
    Refused,
}

/// Para onde o foco vai com o comando `c`. `enabled_quick_note`: a caixa aceita
/// texto (ha vault e fonte); desligada, Cmd-K e recusado (a view mostra a dica).
pub fn transition(current: Focus, c: Command, enabled_quick_note: bool) -> Transition {
    match (c, current) {
        (Command::FocusQuickNote, _) if enabled_quick_note => Transition::To(Focus::QuickNote),
        (Command::FocusQuickNote, _) => Transition::Refused,
        (Command::FocusImage, _) | (Command::Esc, Focus::QuickNote | Focus::Number) => Transition::To(Focus::Image),
        (Command::Esc, Focus::Image | Focus::Other | Focus::Drawer) => Transition::Stays,
    }
}

/// W/A/S/D -> direcao `(dx, dy)` em passos para `viewer_math::pan_by` (que move
/// a IMAGEM, como o arrasto): W mostra o que esta acima, entao a regiao
/// visivel sobe e a imagem desce (`dy = +1`); S o contrario; A mostra a
/// esquerda (`dx = +1`), D a direita. So com a imagem em foco (num campo de
/// texto as letras sao texto) e sem Shift/Cmd/Ctrl/Alt. Maiuscula sem Shift
/// (Caps Lock) vale.
pub fn pan_key(key: &str, focus_state: Focus, shift: bool, cmd: bool, ctrl: bool, alt: bool) -> Option<(f32, f32)> {
    if focus_state != Focus::Image || shift || cmd || ctrl || alt {
        return None;
    }
    match key {
        "w" | "W" => Some((0.0, 1.0)),
        "s" | "S" => Some((0.0, -1.0)),
        "a" | "A" => Some((1.0, 0.0)),
        "d" | "D" => Some((-1.0, 0.0)),
        _ => None,
    }
}

/// Teclas dos grifos na imagem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HighlightAction {
    /// Delete / Backspace: apaga o grifo selecionado.
    Delete,
    /// Cmd+Z: devolve o ultimo grifo apagado (nesta imagem).
    Undo,
    /// Esc: tira a selecao (antes do Esc de sempre).
    Deselect,
}

/// Tecla -> acao de grifo, SO com a imagem em foco (nunca num campo de texto:
/// Backspace apaga letra e Cmd+Z e da caixa; nem com a gaveta em foco, onde
/// Backspace pede a exclusao). Delete/Backspace e Esc so sem modificador e
/// com um grifo `selected`; Cmd+Z (sem Shift/Ctrl/Alt) so se ha o que
/// desfazer (`can_undo`). Fora disso `None`: a tecla segue o caminho de
/// sempre (Esc sem selecao continua sem mexer no foco).
#[allow(clippy::too_many_arguments)]
pub fn highlight_key(
    key: &str,
    focus_state: Focus,
    shift: bool,
    cmd: bool,
    ctrl: bool,
    alt: bool,
    selected: bool,
    can_undo: bool,
) -> Option<HighlightAction> {
    if focus_state != Focus::Image || shift || ctrl || alt {
        return None;
    }
    match (key, cmd) {
        ("delete" | "backspace", false) if selected => Some(HighlightAction::Delete),
        ("escape", false) if selected => Some(HighlightAction::Deselect),
        ("z" | "Z", true) if can_undo => Some(HighlightAction::Undo),
        _ => None,
    }
}

/// Esc com a imagem em foco, sem modificador e uma ferramenta de grifo ligada
/// (`is_on`): desliga a ferramenta (antes de soltar a selecao de um grifo).
pub fn esc_disables_tool(key: &str, focus_state: Focus, shift: bool, cmd: bool, ctrl: bool, alt: bool, is_on: bool) -> bool {
    is_on && key == "escape" && focus_state == Focus::Image && !(shift || cmd || ctrl || alt)
}

/// O que Cmd+Z / "Editar > Desfazer" desfaz.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetUndo {
    /// O ultimo grifo apagado (so com a imagem em foco, como `highlight_key`).
    Highlight,
}

/// O alvo do desfazer: num campo de texto (`in_text`: nota rapida ou
/// "Renomear") nada, porque a caixa nao tem desfazer e Cmd+Z nunca pode
/// desfazer um grifo enquanto se digita; com a imagem em foco o grifo
/// (`highlight_can`); fora disso nada.
/// Com a gaveta em foco e a confirmacao de excluir aberta
/// (`drawer_modal`), nada: ela fica com as teclas.
pub fn target_undo(
    focus_state: Focus,
    in_text: bool,
    highlight_can: bool,
    drawer_modal: bool,
) -> Option<TargetUndo> {
    if in_text || text_field(focus_state, false) || (focus_state == Focus::Drawer && drawer_modal) {
        return None;
    }
    if focus_state == Focus::Image && highlight_can {
        return Some(TargetUndo::Highlight);
    }
    None
}

#[cfg(test)]
#[path = "tests/focus_state.rs"]
mod tests;
