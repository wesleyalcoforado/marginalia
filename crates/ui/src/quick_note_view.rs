//! Caixa de nota rapida do rodape (gpui). O gpui 0.2.2 nao tem campo de texto
//! pronto: esta view e um editor minimo sobre `quick_note::Buffer` (sem gpui).
//!
//! Texto e acentos chegam pelo `EntityInputHandler`: no macOS o gpui manda a
//! tecla primeiro aos listeners e, se ninguem parar a propagacao, ao
//! `NSTextInputContext`, que chama `setMarkedText` (tecla morta `´`, `~`, `` ` ``,
//! `^`: o acento fica marcado) e `insertText` (`á`, `ã`, `à`, `ç`... ja
//! compostos). Por isso o listener de teclas so trata as teclas de edicao de
//! `quick_note::key` (Enter, Cmd+Enter, Esc, setas, apagar, Cmd-X/C/V/A) e
//! deixa o resto seguir. Cmd-X/C/V/A sao `KeyBinding` no contexto
//! `menus::TEXT_CONTEXT` desta caixa (casam antes do listener) e itens do
//! menu Editar; os tres caminhos chamam [`QuickNoteBox::edit`]. Durante uma composicao o gpui entrega a tecla ao
//! sistema antes dos listeners (Enter conclui a composicao, nao quebra a linha).
//!
//! Multilinha (Task 8): Enter quebra a linha (o salvamento e automatico),
//! Cmd+Enter finaliza. A caixa cresce ate `quick_note::MAX_LINES_VISIBLE`
//! linhas e depois rola por dentro (`quick_note::scroll`: segue o cursor ao
//! digitar/mover; a roda do mouse rola livre). A caixa "Renomear" usa o mesmo
//! editor em modo de uma linha ([`QuickNoteBox::single_line`]): Enter confirma.
//!
//! Quebra automatica (fix round 1): cada linha logica e moldada na largura da
//! caixa (`shape_text` -> `WrappedLine`); altura, rolagem, cursor, selecao,
//! clique e a janela do IME contam LINHAS VISUAIS (`quick_note::Visual`). As
//! setas cima/baixo, Home/End etc. andam pelas linhas visuais
//! (`Buffer::relocate_cursor` medido pelo [`Mold`]).

use std::borrow::Cow;
use std::ops::Range;

use gpui::{
    div, fill, point, prelude::*, px, relative, rgb, rgba, size, App, Bounds, ClipboardItem, ContentMask, Context, CursorStyle,
    ElementId, ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle, Font, GlobalElementId,
    DispatchPhase, Hsla, KeyDownEvent, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ScrollWheelEvent,
    ShapedLine, SharedString, Style, TextAlign, TextRun, UTF16Selection, UnderlineStyle, Window, WrappedLine,
};

use crate::menus;
use crate::quick_note::{self, key, Buffer, LineVisual, Measure, Key, Visual};


/// O que a caixa pede a view raiz.
pub enum QuickNoteEvent {
    /// Cmd+Enter (na de uma linha, Enter): terminar com este texto (a caixa so
    /// e limpa quando o disco tiver o texto).
    Finish(String),
    /// Esc: voltar o foco para a imagem (o texto fica).
    Exit,
    /// O texto mudou (fora de uma composicao de acento): o salvamento automatico.
    Changed(String),
}

pub struct QuickNoteBox {
    focus_handle: FocusHandle,
    buf: Buffer,
    enabled: bool,
    /// Caixa de uma linha ("Renomear"): Enter confirma em vez de quebrar a linha.
    single_line: bool,
    /// So aceita algarismos (digitados, compostos ou colados), no maximo tantos: a caixa "Ir para".
    so_digits: Option<usize>,
    /// Botao esquerdo apertado dentro da caixa: o mouse arrastando seleciona.
    dragging: bool,
    placeholder: SharedString,
    /// Ultimo layout pintado (linhas logicas moldadas com quebra e as linhas visuais).
    mold: Mold,
    /// Estilo e largura do texto na ultima pintura (para moldar fora dela: teclas, altura).
    style: Option<QuickNoteStyle>,
    width: Option<Pixels>,
    limits: Option<Bounds<Pixels>>,
    line_height: Pixels,
    /// Primeira linha mostrada (rolagem interna, passando de 8 linhas).
    first: usize,
    /// O texto ou o cursor mudou: a proxima pintura rola ate o cursor.
    follow_cursor: bool,
    /// Resto (em linhas) da roda do mouse ainda nao aplicado.
    runs: f32,
    /// Tamanho da fonte em px (Configuracoes); `None`: o tamanho pequeno padrao do app.
    font_size: Option<f32>,
}

impl EventEmitter<QuickNoteEvent> for QuickNoteBox {}

impl QuickNoteBox {
    pub fn new(cx: &mut Context<Self>) -> Self {
        QuickNoteBox {
            focus_handle: cx.focus_handle(),
            buf: Buffer::new(),
            enabled: false,
            single_line: false,
            so_digits: None,
            dragging: false,
            placeholder: "".into(),
            mold: Mold::empty(),
            style: None,
            width: None,
            limits: None,
            line_height: px(18.0),
            first: 0,
            follow_cursor: true,
            runs: 0.0,
            font_size: None,
        }
    }

    pub fn font_size(&self) -> Option<f32> {
        self.font_size
    }

    /// Tamanho da fonte em px (`None`: o padrao pequeno do app). Repinta se mudou.
    pub fn set_size_font(&mut self, t: Option<f32>, cx: &mut Context<Self>) {
        if self.font_size != t {
            self.font_size = t;
            cx.notify();
        }
    }

    /// Caixa numerica: tudo o que nao e algarismo e descartado antes de entrar no texto, e
    /// ela nunca passa de `max` algarismos (assim o numero cabe numa linha so).
    pub fn so_digits(mut self, max: usize) -> Self {
        self.so_digits = Some(max);
        self
    }

    /// Troca o limite de algarismos de uma caixa numerica; o texto que ja passa do novo
    /// limite e cortado.
    pub fn limit_digits(&mut self, max: usize, cx: &mut Context<Self>) {
        if self.so_digits == Some(max) {
            return;
        }
        self.so_digits = Some(max);
        if self.buf.text().chars().count() > max {
            let cut: String = self.buf.text().chars().take(max).collect();
            self.set_text(&cut, cx);
        }
        cx.notify();
    }

    /// Ha texto selecionado na caixa (so para o smoke conferir).
    pub fn selection_has(&self) -> bool {
        !self.buf.selected().is_empty()
    }

    /// `t` como a caixa o aceita: numerica, so os algarismos que ainda cabem (o texto
    /// selecionado sera substituido, entao libera lugar).
    fn accepted<'a>(&self, t: &'a str) -> Cow<'a, str> {
        match self.so_digits {
            Some(max) => {
                let used = self.buf.text().chars().count().saturating_sub(self.buf.selected().chars().count());
                let fit = max.saturating_sub(used);
                Cow::Owned(quick_note::so_digits(t).chars().take(fit).collect())
            }
            None => Cow::Borrowed(t),
        }
    }

    /// Modo de uma linha (caixa "Renomear fonte"): Enter confirma.
    pub fn single_line(mut self) -> Self {
        self.single_line = true;
        self
    }

    /// Primeira linha visivel (rolagem interna; para o smoke).
    pub fn first_line(&self) -> usize {
        self.first
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    pub fn text(&self) -> &str {
        self.buf.text()
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Liga/desliga a caixa (desligada sem vault) e troca a dica mostrada vazia.
    pub fn configure(&mut self, enabled: bool, placeholder: impl Into<SharedString>, cx: &mut Context<Self>) {
        let placeholder = placeholder.into();
        if self.enabled != enabled || self.placeholder != placeholder {
            self.enabled = enabled;
            self.placeholder = placeholder;
            cx.notify();
        }
    }

    /// Limpa a caixa; no meio de uma composicao (IME / tecla morta) espera ela
    /// terminar e so o que foi composto fica (uma nota nova).
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.buf.clear_or_postpone();
        self.follow_cursor = true;
        cx.notify();
    }

    /// Troca o texto por `t`, todo selecionado (digitar substitui). Usado na
    /// caixa de "Renomear fonte", que reaproveita este editor.
    pub fn set_text(&mut self, t: &str, cx: &mut Context<Self>) {
        self.buf.clear();
        self.buf.insert(t);
        self.buf.select_all();
        self.follow_cursor = true;
        cx.notify();
    }

    /// Texto carregado de uma nota ou tarefa para editar: o cursor vai para o
    /// fim (continuar a transcricao), sem selecionar (uma tecla nao apaga tudo).
    pub fn load_text(&mut self, t: &str, cx: &mut Context<Self>) {
        self.buf.clear();
        self.buf.insert(t);
        self.follow_cursor = true;
        cx.notify();
    }

    fn on_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        let m = &crate::normalize_modifiers(k.modifiers);
        let Some(t) = key(&k.key, m.shift, m.platform, m.control, m.alt) else {
            return; // texto: segue para o InputHandler
        };
        cx.stop_propagation();
        self.edit(t, window, cx);
    }

    /// Uma acao de edicao: as teclas (`on_key`) e os itens do menu Editar
    /// (Recortar, Copiar, Colar, Selecionar tudo: acoes do gpui no contexto
    /// [`menus::TEXT_CONTEXT`], que casam antes de `on_key`) chamam isto.
    pub fn edit(&mut self, t: Key, window: &mut Window, cx: &mut Context<Self>) {
        let t = if self.single_line { quick_note::single_line(t) } else { t };
        self.follow_cursor = true;
        let before = self.buf.text().to_string();
        match t {
            Key::Finish => cx.emit(QuickNoteEvent::Finish(self.buf.text().to_string())),
            Key::Exit => cx.emit(QuickNoteEvent::Exit),
            Key::Ignore => {}
            // Copiar nao muda o texto (vale tambem com a caixa desligada).
            Key::Copy => {
                let s = self.buf.selected();
                if !s.is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(s.to_string()));
                }
            }
            _ if !self.enabled => {}
            Key::Cut => {
                if let Some(s) = self.buf.cut() {
                    cx.write_to_clipboard(ClipboardItem::new_string(s));
                }
            }
            Key::NewLine => self.buf.insert("\n"),
            Key::EraseBefore => self.buf.erase_before(),
            Key::EraseAfter => self.buf.erase_after(),
            Key::Relocate(m, extend) => {
                // Moldado agora (o texto pode ter mudado depois da ultima pintura).
                let mold = self.now_mold(window);
                self.buf.relocate_cursor(m, extend, &mold.visual, &mold);
            }
            Key::SelectAll => self.buf.select_all(),
            Key::Paste => {
                if let Some(s) = cx.read_from_clipboard().and_then(|i| i.text()) {
                    let pasted = s.replace("\r\n", "\n").replace('\r', "\n");
                    self.buf.insert(&self.accepted(&pasted));
                }
            }
        }
        self.buf.conclude_cleanup_postponed();
        if self.buf.text() != before {
            self.changed(cx);
        }
        cx.notify();
    }

    /// Avisa a view raiz do texto novo, exceto no meio de uma composicao
    /// (o acento marcado ainda nao e texto: o fim da composicao avisa).
    fn changed(&self, cx: &mut Context<Self>) {
        if self.buf.marked().is_none() {
            cx.emit(QuickNoteEvent::Changed(self.buf.text().to_string()));
        }
    }

    fn on_mouse_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled {
            return;
        }
        self.focus_handle.focus(window);
        if let Some(off) = self.point_index(ev.position) {
            self.click(off, ev.click_count, ev.modifiers.shift);
        }
        cx.notify();
    }

    /// Clique no offset `off`: 1 poe o cursor (com Shift estende), 2 seleciona a palavra, 3+ a linha.
    /// Separado do evento para o smoke exercitar a mesma logica.
    pub fn click(&mut self, off: usize, clicks: usize, shift: bool) {
        match clicks {
            0 | 1 if shift => self.buf.extend_ate(off),
            0 | 1 => self.buf.position(off),
            2 => self.buf.select_word(off),
            _ => self.buf.select_line(off),
        }
        self.dragging = clicks <= 1;
        self.follow_cursor = true;
    }

    /// O texto selecionado (para o smoke).
    pub fn selected(&self) -> &str {
        self.buf.selected()
    }

    /// Mouse arrastando com o botao apertado: a selecao segue o ponteiro (mesmo fora da caixa).
    fn drag(&mut self, p: Point<Pixels>, pressed: bool, cx: &mut Context<Self>) {
        if !self.dragging {
            return;
        }
        if !pressed {
            self.dragging = false;
            return;
        }
        if let Some(off) = self.point_index(p) {
            self.buf.extend_ate(off);
            self.follow_cursor = true;
            cx.notify();
        }
    }

    /// Roda do mouse/trackpad: rola as linhas da caixa (sem mexer no cursor).
    fn on_scroll(&mut self, ev: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let lh = self.line_height;
        let dy = f32::from(ev.delta.pixel_delta(lh).y) / f32::from(lh).max(1.0);
        if !dy.is_finite() {
            return;
        }
        // Delta positivo = conteudo desce (mostra as linhas de cima).
        self.runs -= dy;
        let whole = self.runs.trunc();
        if whole != 0.0 {
            self.runs -= whole;
            let total = self.mold.visual.len();
            let target = (self.first as f32 + whole).max(0.0) as usize;
            self.first = quick_note::scroll(target, 0, total, false);
            self.follow_cursor = false;
            cx.stop_propagation();
            cx.notify();
        }
    }

    /// Offset (bytes) mais proximo de um ponto da janela (no ultimo layout pintado).
    fn point_index(&self, p: Point<Pixels>) -> Option<usize> {
        let b = self.limits?;
        if self.buf.empty() {
            return Some(0);
        }
        let line = (f32::from(p.y - b.top()) / f32::from(self.line_height)).floor().max(0.0) as usize + self.first;
        let line = line.min(self.mold.visual.len() - 1);
        Some(self.mold.index(line, p.x - b.left()).min(self.buf.text().len()))
    }

    /// Ponto (canto superior esquerdo) do offset `off` no ultimo layout (com a quebra e a rolagem).
    fn index_point(&self, off: usize) -> Option<Point<Pixels>> {
        let b = self.limits?;
        let line = self.mold.visual.line_of(off);
        let y = b.top() + self.line_height * (line as f32 - self.first as f32);
        Some(point(b.left() + self.mold.x(line, off), y))
    }

    /// Linhas VISUAIS no ultimo layout (com a quebra automatica; para o smoke).
    pub fn visual_lines(&self) -> usize {
        self.mold.visual.len()
    }

    /// O texto moldado agora (o estilo e a largura da ultima pintura), para as
    /// teclas que andam pelas linhas da tela.
    fn now_mold(&self, window: &Window) -> Mold {
        let style = self.style.clone().unwrap_or_else(|| QuickNoteStyle::de(window));
        shape(&self.buf, &style, self.width, window)
    }
}

/// Fonte, tamanho e cor do texto da caixa (capturados na pintura: fora do
/// elemento o estilo de texto da janela e outro).
#[derive(Clone)]
struct QuickNoteStyle {
    source: Font,
    size: Pixels,
    color: Hsla,
}

impl QuickNoteStyle {
    fn de(window: &Window) -> QuickNoteStyle {
        let s = window.text_style();
        QuickNoteStyle { source: s.font(), size: s.font_size.to_pixels(window.rem_size()), color: s.color }
    }
}

/// O texto moldado para a largura da caixa: uma `WrappedLine` por linha logica
/// (com as quebras automaticas do gpui) e as linhas visuais (`quick_note::Visual`).
struct Mold {
    lines: Vec<(WrappedLine, Range<usize>)>,
    visual: Visual,
}

impl Mold {
    fn empty() -> Mold {
        Mold { lines: Vec::new(), visual: Visual::de("", &[]) }
    }

    /// A linha logica da linha visual `i` e a faixa da linha visual.
    fn parts(&self, i: usize) -> Option<(&WrappedLine, &Range<usize>, &LineVisual)> {
        let lv = self.visual.lines().get(i)?;
        let (w, strip) = self.lines.get(lv.logic)?;
        Some((w, strip, lv))
    }

    /// x do offset `off` dentro da linha visual `i`.
    fn x(&self, i: usize, off: usize) -> Pixels {
        let Some((w, strip, lv)) = self.parts(i) else { return px(0.0) };
        let u = &w.unwrapped_layout;
        let local = |o: usize| o.clamp(strip.start, strip.end) - strip.start;
        u.x_for_index(local(off)) - u.x_for_index(local(lv.strip.start))
    }

    /// Offset mais proximo de `x` na linha visual `i` (sem passar para a de baixo).
    fn index(&self, i: usize, x: Pixels) -> usize {
        let Some((w, strip, lv)) = self.parts(i) else { return 0 };
        let u = &w.unwrapped_layout;
        let base = u.x_for_index(lv.strip.start - strip.start);
        let idx = strip.start + u.closest_index_for_x(x.max(px(0.0)) + base).min(strip.len());
        self.visual.limit(i, idx)
    }
}

impl Measure for Mold {
    fn x(&self, line: usize, off: usize) -> f32 {
        f32::from(Mold::x(self, line, off))
    }

    fn index(&self, line: usize, x: f32) -> usize {
        Mold::index(self, line, px(x))
    }
}

/// Molda `buf` (o trecho em composicao sublinhado) quebrando em `width`.
fn shape(buf: &Buffer, e: &QuickNoteStyle, width: Option<Pixels>, window: &Window) -> Mold {
    let base = TextRun { len: 0, font: e.source.clone(), color: e.color, background_color: None, underline: None, strikethrough: None };
    // Folga a direita: o cursor no fim de uma linha cheia nao fica cortado na borda.
    let width = width.and_then(|l| quick_note::break_width(f32::from(l))).map(px);
    let marked = buf.marked();
    let mut lines = Vec::new();
    let mut breaks = Vec::new();
    for strip in buf.lines() {
        let t: SharedString = buf.text()[strip.clone()].to_string().into();
        let mut runs = Vec::new();
        let m = marked
            .as_ref()
            .map(|m| (m.start.clamp(strip.start, strip.end) - strip.start, m.end.clamp(strip.start, strip.end) - strip.start));
        match m {
            Some((a, b)) if b > a => {
                let sub = UnderlineStyle { color: Some(e.color), thickness: px(1.0), wavy: false };
                runs.push(TextRun { len: a, ..base.clone() });
                runs.push(TextRun { len: b - a, underline: Some(sub), ..base.clone() });
                runs.push(TextRun { len: t.len() - b, ..base.clone() });
            }
            _ => runs.push(TextRun { len: t.len(), ..base.clone() }),
        }
        runs.retain(|r| r.len > 0);
        let w = window
            .text_system()
            .shape_text(t, e.size, &runs, width, None)
            .ok()
            .and_then(|v| v.into_iter().next())
            .unwrap_or_default();
        let q: Vec<usize> = w
            .wrap_boundaries()
            .iter()
            .filter_map(|b| w.runs().get(b.run_ix).and_then(|r| r.glyphs.get(b.glyph_ix)).map(|g| g.index))
            .collect();
        breaks.push(q);
        lines.push((w, strip));
    }
    let visual = Visual::de(buf.text(), &breaks);
    Mold { lines, visual }
}

impl EntityInputHandler for QuickNoteBox {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let (t, r) = self.buf.text_utf_16(range_utf16);
        actual_range.replace(r);
        Some(t)
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        Some(UTF16Selection { range: self.buf.selection_utf_16(), reversed: self.buf.inverted() })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.buf.marked_utf_16()
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.buf.uncheck();
        self.changed(cx);
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.enabled {
            let original = text;
            let text = self.accepted(text).into_owned();
            // Tecla que nao e algarismo: o texto nao muda (nem a selecao some), salvo para
            // fechar uma composicao em andamento (um acento solto).
            if self.so_digits.is_some() && text.is_empty() && !original.is_empty() && self.buf.marked().is_none() {
                return;
            }
            self.buf.replace_utf_16(range_utf16, &text);
            self.follow_cursor = true;
            self.changed(cx);
            cx.notify();
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.enabled {
            // Composicao (acento, IME) numa caixa numerica: so os algarismos valem.
            let filtered = self.accepted(new_text).into_owned();
            let new_selected_range_utf16 = if filtered == new_text { new_selected_range_utf16 } else { None };
            self.buf.replace_and_mark_utf_16(range_utf16, &filtered, new_selected_range_utf16);
            self.follow_cursor = true;
            // Composicao cancelada (marca vazia) volta ao texto de antes: avisa.
            self.changed(cx);
            cx.notify();
        }
    }

    /// Onde o sistema desenha a janela de candidatos / o acento pendente.
    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let a = self.index_point(self.buf.of_utf_16(range_utf16.start))?;
        let b = self.index_point(self.buf.of_utf_16(range_utf16.end))?;
        let width = if b.y == a.y { (b.x - a.x).max(px(1.0)) } else { px(1.0) };
        Some(Bounds::new(a, size(width, self.line_height)))
    }

    fn character_index_for_point(&mut self, p: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        let off = self.point_index(p)?;
        Some(self.buf.to_utf_16(off))
    }
}

impl Render for QuickNoteBox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus_handle.is_focused(window);
        let border = if focused { rgb(crate::theme::current().accent) } else { rgb(crate::theme::current().strong_border) };
        div()
            .flex_1()
            .min_w(px(0.0))
            .track_focus(&self.focus_handle)
            // Recortar/Copiar/Colar/Selecionar tudo: KeyBinding e menu Editar so
            // com esta caixa em foco (fora dela os itens ficam desligados).
            .key_context(menus::TEXT_CONTEXT)
            .on_action(cx.listener(|n, _: &crate::Cut, w, cx| n.edit(Key::Cut, w, cx)))
            .on_action(cx.listener(|n, _: &crate::Copy, w, cx| n.edit(Key::Copy, w, cx)))
            .on_action(cx.listener(|n, _: &crate::Paste, w, cx| n.edit(Key::Paste, w, cx)))
            .on_action(cx.listener(|n, _: &crate::SelectAll, w, cx| n.edit(Key::SelectAll, w, cx)))
            .cursor(if self.enabled { CursorStyle::IBeam } else { CursorStyle::Arrow })
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(border)
            .bg(if self.enabled { rgb(crate::theme::current().field) } else { rgb(crate::theme::current().bar) })
            .map(|d| match self.font_size {
                Some(t) => d.text_size(px(t)),
                None => d.text_sm(),
            })
            .child(Text { textbox: cx.entity() })
    }
}

/// Elemento que molda e pinta o texto (uma `WrappedLine` por linha logica,
/// quebrada na largura da caixa), o cursor, a selecao e o sublinhado da
/// composicao, e registra o InputHandler.
struct Text {
    textbox: Entity<QuickNoteBox>,
}

struct Paint {
    mold: Mold,
    style: QuickNoteStyle,
    /// A dica da caixa vazia.
    placeholder: Option<ShapedLine>,
    quads: Vec<PaintQuad>,
    line_height: Pixels,
    /// Primeira linha VISUAL mostrada (rolagem interna).
    first: usize,
}

impl IntoElement for Text {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Text {
    /// Linhas visuais contadas no layout (a pintura confere com a largura real).
    type RequestLayoutState = usize;
    type PrepaintState = Paint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, usize) {
        // 1 a 8 linhas VISUAIS (com a quebra na largura da ultima pintura);
        // passando disso a caixa rola por dentro (`prepaint`).
        let textbox = self.textbox.read(cx);
        let n = if textbox.buf.empty() {
            1
        } else {
            shape(&textbox.buf, &QuickNoteStyle::de(window), textbox.width, window).visual.len()
        };
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = px(quick_note::textbox_height(n, f32::from(window.line_height()))).into();
        (window.request_layout(style, [], cx), n)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut usize,
        window: &mut Window,
        cx: &mut App,
    ) -> Paint {
        let textbox = self.textbox.read(cx);
        let buf = &textbox.buf;
        let style = QuickNoteStyle::de(window);
        let lh = window.line_height();
        // Moldado na largura real da caixa: as linhas visuais da tela.
        let mold = shape(buf, &style, Some(bounds.size.width), window);
        let placeholder = buf.empty().then(|| {
            let t = textbox.placeholder.clone();
            let run = TextRun {
                len: t.len(),
                font: style.source.clone(),
                color: rgb(crate::theme::current().weak_text).into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            window.text_system().shape_line(t, style.size, &[run], None)
        });
        // Rolagem interna: a janela de ate 8 linhas visuais segue o cursor ao digitar/mover.
        let line_cursor = mold.visual.line_of(buf.cursor());
        let first = quick_note::scroll(textbox.first, line_cursor, mold.visual.len(), textbox.follow_cursor);
        let top_of = |n: usize| bounds.top() + lh * (n as f32 - first as f32);
        let mut quads = Vec::new();
        let sel = buf.selection();
        if sel.is_empty() {
            if textbox.focus_handle.is_focused(window) && textbox.enabled {
                let x = mold.x(line_cursor, buf.cursor());
                quads.push(fill(Bounds::new(point(bounds.left() + x, top_of(line_cursor)), size(px(1.5), lh)), rgb(crate::theme::current().emphasis)));
            }
        } else {
            for (n, lv) in mold.visual.lines().iter().enumerate() {
                let strip = &lv.strip;
                let (a, b) = (sel.start.max(strip.start), sel.end.min(strip.end));
                if a > b || (a == b && !(sel.start <= strip.end && sel.end > strip.end)) {
                    continue;
                }
                let (xa, xb) = (mold.x(n, a), mold.x(n, b));
                let width = (xb - xa).max(px(4.0));
                quads.push(fill(Bounds::new(point(bounds.left() + xa, top_of(n)), size(width, lh)), rgba((crate::theme::current().accent << 8) | 0x55)));
            }
        }
        Paint { mold, style, placeholder, quads, line_height: lh, first }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout_lines: &mut usize,
        paint: &mut Paint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_state = self.textbox.read(cx).focus_handle.clone();
        window.handle_input(&focus_state, ElementInputHandler::new(bounds, self.textbox.clone()), cx);
        let lh = paint.line_height;
        let first = paint.first;
        let total = paint.mold.visual.len();
        let visible = quick_note::visible_lines(total);
        // So as linhas da janela, cortadas na caixa (o texto rolado nao vaza).
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            if let Some(p) = &paint.placeholder {
                let _ = p.paint(point(bounds.left(), bounds.top()), lh, window, cx);
            } else {
                // Cada linha logica pinta as suas linhas visuais a partir da primeira delas.
                let mut start = 0;
                for (n, (w, _)) in paint.mold.lines.iter().enumerate() {
                    let count = paint.mold.visual.lines()[start..].iter().take_while(|l| l.logic == n).count().max(1);
                    if start + count > first && start < first + visible {
                        let top = bounds.top() + lh * (start as f32 - first as f32);
                        let _ = w.paint(point(bounds.left(), top), lh, TextAlign::Left, None, window, cx);
                    }
                    start += count;
                }
            }
            for q in paint.quads.drain(..) {
                window.paint_quad(q);
            }
        });
        let mold = std::mem::replace(&mut paint.mold, Mold::empty());
        let style = paint.style.clone();
        let width = bounds.size.width;
        // A altura veio da largura da pintura anterior: se a quebra mudou, outro quadro.
        let n = if paint.placeholder.is_some() { 1 } else { total };
        if n != *layout_lines {
            window.refresh();
        }
        let textbox = self.textbox.downgrade();
        window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
            if phase != DispatchPhase::Capture {
                return;
            }
            let (p, pressed) = (ev.position, ev.pressed_button == Some(MouseButton::Left));
            let _ = textbox.update(cx, |c, cx| c.drag(p, pressed, cx));
        });
        let textbox = self.textbox.downgrade();
        window.on_mouse_event(move |ev: &MouseUpEvent, phase, _, cx| {
            if phase != DispatchPhase::Capture || ev.button != MouseButton::Left {
                return;
            }
            let _ = textbox.update(cx, |c, _| c.dragging = false);
        });
        self.textbox.update(cx, |c, _| {
            c.mold = mold;
            c.style = Some(style);
            c.width = Some(width);
            c.limits = Some(bounds);
            c.line_height = lh;
            c.first = first;
            c.follow_cursor = false;
        });
    }
}
