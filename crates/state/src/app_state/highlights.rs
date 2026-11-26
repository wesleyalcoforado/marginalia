//! Grifos da imagem atual: criar, selecionar, excluir, desfazer, cor.

use super::*;

impl AppState {
    /// Espelha o enquadramento do visualizador (zoom/centro), limitado como la.
    pub fn set_framing(&mut self, e: Framing) {
        self.fr = viewer_math::clamp(e);
    }

    /// Enquadramento atual (o que a proxima nota guarda).
    pub fn framing(&self) -> &Framing {
        &self.fr
    }

    /// Imagem atual, se houver uma aberta.
    pub(super) fn current_image(&self) -> Option<&ImageRef> {
        (self.source_has() && !self.current.image.file.is_empty()).then_some(&self.current.image)
    }

    /// Le `highlights/<source>.json` do vault (arquivo pequeno e local). Ilegivel:
    /// avisa e recusa gravar por cima ate a fonte ser reaberta.
    pub(super) fn load_highlights(&mut self, source: &str) {
        let read_log = self.vault.as_ref().map(|v| HighlightsFile::load(v, source).map_err(|e| e.to_string()));
        match read_log {
            Some(g) => self.adopt_highlights(source, g),
            None => {
                self.annot.highlights = HighlightsFile::default();
                self.annot.error_highlights = None;
                self.annot.highlights_version += 1;
            }
        }
    }

    /// Passa a usar os grifos lidos de `highlights/<source>.json` (aqui ou no fundo).
    pub(super) fn adopt_highlights(&mut self, source: &str, g: Result<HighlightsFile, String>) {
        self.annot.highlights_version += 1;
        self.annot.error_highlights = None;
        match g {
            Ok(f) => self.annot.highlights = f,
            Err(e) => {
                self.annot.highlights = HighlightsFile::default();
                let m = t!("warning.highlights.unreadable", source = format!("{source:?}"), e = e);
                self.warning = Some(m.clone());
                self.annot.error_highlights = Some(m);
            }
        }
    }

    /// Grava `g` (coordenadas ja limitadas a 0..=1, ver `annotate`) nos grifos
    /// da imagem atual em `highlights/<source>.json` e o guarda como "grifo
    /// pendente": a proxima nota desta imagem fica ligada a ele.
    ///
    /// Regra da cor: um grifo SEM cor propria (`cor: None`, o que `annotate`
    /// produz) nasce com [`AppState::current_color`], mas so quando ela nao e a
    /// padrao; na cor padrao o arquivo continua sem `color`. Um grifo que ja traz
    /// cor a mantem. Id vazio e recusado.
    pub fn commit_highlight(&mut self, g: Highlight) -> Result<(), String> {
        let vault = self.vault.as_ref().ok_or_else(|| QuickNoteError::NoVault.to_string())?;
        let image = self.current_image().ok_or_else(|| QuickNoteError::NoImage.to_string())?.clone();
        if let Some(e) = &self.annot.error_highlights {
            return Err(e.clone());
        }
        if g.id().is_empty() {
            return Err(t!("error.highlight.no_id"));
        }
        let no_color = matches!(&g, Highlight::Rectangle { color: None, .. } | Highlight::Pointer { color: None, .. });
        let g = if no_color && self.current_color() != DEFAULT_COLOR { g.with_color(self.app_data.highlight_color()) } else { g };
        if self.annot.highlights.images.values().flatten().any(|x| x.id() == g.id()) {
            return Err(t!("error.highlight.id_repeated", id = format!("{:?}", g.id())));
        }
        let mut new = self.annot.highlights.clone();
        new.add(&image, g.clone()).map_err(|e| t!("error.highlight.refused", e))?;
        new.save(vault, &image.source).map_err(|e| t!("error.highlight.write", e))?;
        self.annot.highlights = new;
        self.annot.highlights_version += 1;
        self.annot.pending = Some((image, g));
        Ok(())
    }

    /// Seleciona o grifo `id` da imagem atual (`None`: limpa). Um id que a
    /// imagem atual nao tem deixa sem selecao.
    pub fn select(&mut self, id: Option<String>) {
        self.annot.selected = id.filter(|i| self.current_image().is_some_and(|img| self.annot.highlights.find(img, i).is_some()));
    }

    /// Pre-condicoes de gravar mudancas nos grifos da imagem atual.
    pub(super) fn can_write_highlights(&self) -> Result<ImageRef, String> {
        self.vault.as_ref().ok_or_else(|| QuickNoteError::NoVault.to_string())?;
        let image = self.current_image().ok_or_else(|| QuickNoteError::NoImage.to_string())?.clone();
        match &self.annot.error_highlights {
            Some(e) => Err(e.clone()),
            None => Ok(image),
        }
    }

    /// Exclui o grifo `id` da imagem atual e grava. O grifo fica guardado para
    /// [`AppState::undo_highlight`] (uma posicao). Se o grifo era o pendente, o
    /// destacado ou o selecionado, isso tambem cai. Falha ao gravar: erro e o
    /// estado em memoria intacto. Uma nota que ja apontava para ele continua
    /// apontando (abre sem destaque); a nota rapida em andamento que o reservou
    /// o solta sozinha ao terminar o rascunho.
    pub fn delete_highlight(&mut self, id: &str) -> Result<(), String> {
        let image = self.can_write_highlights()?;
        let Some(g) = self.annot.highlights.find(&image, id).cloned() else {
            return Err(t!("error.highlight.not_found", id = format!("{id:?}")));
        };
        let vault = self.vault.as_ref().ok_or_else(|| QuickNoteError::NoVault.to_string())?;
        let mut new = self.annot.highlights.clone();
        new.remove(&image, id);
        new.save(vault, &image.source).map_err(|e| t!("error.highlight.write_deletion", e))?;
        self.annot.highlights = new;
        self.annot.highlights_version += 1;
        if self.annot.pending.as_ref().is_some_and(|(_, p)| p.id() == id) {
            self.annot.pending = None;
        }
        if self.annot.emphasis.as_deref() == Some(id) {
            self.annot.emphasis = None;
        }
        if self.annot.selected.as_deref() == Some(id) {
            self.annot.selected = None;
        }
        self.annot.last_highlight_erased = Some((image, g));
        Ok(())
    }

    /// Desfaz a ultima exclusao: devolve o grifo (mesmo id e cor, no fim da
    /// lista, isto e, por cima) e o seleciona. `Ok(false)`: nada a desfazer, ou
    /// o grifo era de outra imagem (nunca vaza para a atual). NAO o torna
    /// pendente de novo. Erro (id que voltou a existir, arquivo ilegivel,
    /// falha ao gravar): nada muda e o desfazer continua valendo.
    pub fn undo_highlight(&mut self) -> Result<bool, String> {
        let Some((img, g)) = self.annot.last_highlight_erased.clone() else { return Ok(false) };
        if self.current_image() != Some(&img) {
            self.annot.last_highlight_erased = None;
            return Ok(false);
        }
        let image = self.can_write_highlights()?;
        if self.annot.highlights.images.values().flatten().any(|x| x.id() == g.id()) {
            return Err(t!("error.highlight.id_repeated", id = format!("{:?}", g.id())));
        }
        let vault = self.vault.as_ref().ok_or_else(|| QuickNoteError::NoVault.to_string())?;
        let mut new = self.annot.highlights.clone();
        new.add(&image, g.clone()).map_err(|e| t!("error.highlight.refused", e))?;
        new.save(vault, &image.source).map_err(|e| t!("error.highlight.write", e))?;
        self.annot.highlights = new;
        self.annot.highlights_version += 1;
        self.annot.last_highlight_erased = None;
        self.annot.selected = Some(g.id().to_string());
        Ok(true)
    }

    /// Ha uma exclusao de grifo para desfazer NESTA imagem (Cmd+Z vale).
    pub fn can_undo_highlight(&self) -> bool {
        self.annot.last_highlight_erased.as_ref().is_some_and(|(img, _)| self.current_image() == Some(img))
    }

    /// Cor dos novos grifos (dados do app; a padrao se nunca escolhida).
    pub fn current_color(&self) -> String {
        self.app_data.highlight_color().to_string()
    }

    /// Escolhe a cor dos novos grifos e a grava nos dados do app. Com um grifo
    /// selecionado, recolore tambem ele (so ele) e grava os grifos ANTES: se
    /// isso falhar (ou o arquivo estiver ilegivel) volta o erro e nada muda,
    /// nem a cor padrao. Cor fora de `PALETTE`: recusada. Falha so ao gravar os
    /// dados do app: a cor vale nesta sessao e `warning` conta (como os ajustes).
    pub fn set_color(&mut self, color: &str) -> Result<(), String> {
        if !PALETTE.contains(&color) {
            return Err(t!("error.highlight.color_outside_of_the_palette", color = format!("{color:?}")));
        }
        let target = match (self.annot.selected.as_deref(), self.current_image()) {
            (Some(id), Some(img)) if self.annot.highlights.find(img, id).is_some() => Some((img.clone(), id.to_string())),
            _ => None,
        };
        if let Some((image, id)) = target {
            self.can_write_highlights()?;
            let vault = self.vault.as_ref().ok_or_else(|| QuickNoteError::NoVault.to_string())?;
            let mut new = self.annot.highlights.clone();
            new.recolor(&image, &id, color);
            new.save(vault, &image.source).map_err(|e| t!("error.highlight.write_color", e))?;
            self.annot.highlights = new;
            self.annot.highlights_version += 1;
            if let Some((_, p)) = self.annot.pending.as_mut().filter(|(_, p)| p.id() == id) {
                *p = p.clone().recolored(color);
            }
        }
        self.app_data.set_color_highlight(color).map_err(|e| e.to_string())?;
        match self.save_app_data() {
            Ok(()) => {}
            Err(e) => self.warning = Some(t!("warning.highlight.remember_color", e)),
        }
        Ok(())
    }

    /// Espelha o enquadramento do visualizador SO se ele mostra a imagem do
    /// quadro atual (chegou e sem erro). Enquanto o quadro novo carrega, o
    /// visualizador ainda tem o enquadramento da imagem anterior: fica valendo o
    /// alvo posto por `ir_to` (o guardado, ou ajustar). Devolve se aceitou.
    pub fn mirror_framing(&mut self, e: Framing) -> bool {
        if !self.displayed || self.error.is_some() {
            return false;
        }
        self.set_framing(e);
        true
    }

    /// [`AppState::commit_highlight`] para um grifo desenhado sobre `image`:
    /// recusa se o quadro atual ja e outro (o grifo nunca vai para outra pagina).
    pub fn commit_highlight_in(&mut self, image: &ImageRef, g: Highlight) -> Result<(), String> {
        if self.current_image() != Some(image) {
            return Err(t!("error.highlight.other_image", image.file, image.page));
        }
        self.commit_highlight(g)
    }

    /// Grifo desenhado nesta imagem que a proxima nota vai levar.
    pub fn pending_highlight(&self) -> Option<&Highlight> {
        let (img, g) = self.annot.pending.as_ref()?;
        (Some(img) == self.current_image()).then_some(g)
    }

    /// Grifos salvos da imagem atual (para desenhar).
    pub fn image_highlights(&self) -> &[Highlight] {
        match self.current_image() {
            Some(i) => self.annot.highlights.for_image(i),
            None => &[],
        }
    }

    /// Grifo a destacar (da nota aberta), se ainda existir nesta imagem.
    pub fn emphasis(&self) -> Option<&str> {
        let id = self.annot.emphasis.as_deref()?;
        let img = self.current_image()?;
        self.annot.highlights.find(img, id).map(|g| g.id())
    }

    /// Marca o grifo da nota aberta para destaque (some ao navegar).
    pub fn highlight(&mut self, id: Option<String>) {
        self.annot.emphasis = id;
    }

    /// O grifo `id` foi ligado a uma nota: deixa de ser o pendente (se ainda
    /// for ele; um grifo desenhado depois continua pendente).
    pub fn linked_highlight(&mut self, id: &str) {
        if self.annot.pending.as_ref().is_some_and(|(_, g)| g.id() == id) {
            self.annot.pending = None;
        }
    }

    /// Um rascunho apagado (caixa esvaziada) devolve o grifo dele: volta a ser o
    /// pendente se ainda existe, e da imagem atual, e nao ha outro pendente.
    pub fn restore_highlight_pending(&mut self, image: &ImageRef, id: &str) {
        if self.annot.pending.is_some() || self.current_image() != Some(image) {
            return;
        }
        if let Some(g) = self.annot.highlights.find(image, id).cloned() {
            self.annot.pending = Some((image.clone(), g));
        }
    }

    /// O documento `old` passou a se chamar `new` (id fixado): troca todos
    /// os ids guardados aqui de uma vez (nota aberta, pedido de nova
    /// localizacao). As gavetas trocam os delas
    /// (`Drawers::switch_id`).
    pub fn switch_id(&mut self, old: &str, new: &str) {
        if let Some((id, _)) = self.note_open.as_mut().filter(|(id, _)| id == old) {
            *id = new.to_string();
        }
        if let Some(p) = self.pending.as_mut().and_then(|p| p.note.as_mut()).filter(|p| *p == old) {
            *p = new.to_string();
        }
    }
}
