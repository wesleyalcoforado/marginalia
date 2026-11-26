//! Anotacoes da fonte aberta: os grifos, o que esta pendente para a proxima
//! nota, o que esta selecionado ou em destaque e o desfazer de uma posicao.
//! So guarda e zera estado; quem sabe de vault, quadro e disco e o `AppState`.

use marginalia_core::highlights::{Highlight, HighlightsFile};
use marginalia_core::model::ImageRef;

#[derive(Default)]
pub struct Annotations {
    /// Grifos da fonte aberta (`highlights/<source>.json`).
    pub(super) highlights: HighlightsFile,
    /// O arquivo de grifos estava ilegivel: nao gravar por cima.
    pub(super) error_highlights: Option<String>,
    /// Ultimo grifo desenhado e a imagem dele: vai na proxima nota.
    pub(super) pending: Option<(ImageRef, Highlight)>,
    /// Grifo da nota aberta por `open_note`.
    pub(super) emphasis: Option<String>,
    /// Grifo selecionado (id) na imagem atual; cai ao trocar de imagem, de fonte ou de vault.
    pub selected: Option<String>,
    /// Ultimo grifo excluido e a imagem dele (desfazer de uma posicao; cai ao
    /// sair da imagem, da fonte ou do vault).
    pub(super) last_highlight_erased: Option<(ImageRef, Highlight)>,
    /// Muda a cada troca local dos grifos (gravar, trocar de fonte/vault).
    pub(super) highlights_version: u64,
}

impl Annotations {
    /// Solta o que e da imagem atual: pendente, destaque, selecao e desfazer.
    /// Troca de fonte ou de vault.
    pub(super) fn image_drop(&mut self) {
        self.pending = None;
        self.emphasis = None;
        self.selected = None;
        self.last_highlight_erased = None;
    }

    /// Vai para outra imagem (ou pagina): o destaque e a selecao caem sempre; o
    /// pendente e o desfazer so valem na imagem em que nasceram.
    pub(super) fn go_to(&mut self, image: &ImageRef) {
        if self.pending.as_ref().is_some_and(|(img, _)| img != image) {
            // Um grifo pendente nunca vai para a nota de outra pagina.
            self.pending = None;
        }
        self.emphasis = None;
        self.selected = None;
        if self.last_highlight_erased.as_ref().is_some_and(|(img, _)| img != image) {
            // O desfazer de um grifo nunca vale em outra imagem.
            self.last_highlight_erased = None;
        }
    }

    /// Fecha a fonte: sem grifos, sem pendencias (a versao muda para quem
    /// guardou a anterior).
    pub(super) fn zero(&mut self) {
        self.highlights = HighlightsFile::default();
        self.error_highlights = None;
        self.highlights_version += 1;
        self.image_drop();
    }
}
