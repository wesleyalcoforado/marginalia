//! Logica da caixa de nota rapida do rodape, sem gpui (regra R4).
//!
//! [`Buffer`] e o texto editavel: cursor/selecao em bytes UTF-8, mais o texto
//! "marcado" da composicao (IME / teclas mortas: `´` + `a` = `á`). As
//! funcoes `*_utf16` sao o contrato do `EntityInputHandler` do gpui, que (como
//! o macOS) fala em unidades UTF-16; a view so as repassa.
//!
//! [`key`] decide o que uma tecla faz na caixa (Enter quebra a linha,
//! Cmd+Enter finaliza, Esc sai...); [`visible_lines`] / [`scroll`] dao a
//! altura da caixa (1 a 8 linhas) e a rolagem interna que segue o cursor. Teclas que produzem texto NAO passam por aqui:
//! chegam pelo `InputHandler` (`replace_utf_16`), ja compostas pelo sistema.

use std::ops::Range;

use unicode_normalization::char::is_combining_mark;

/// Acao de uma tecla na caixa de nota.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// Cmd+Enter: termina a nota (grava, fixa o id, limpa a caixa). Na caixa de
    /// uma linha ("Renomear"), Enter tambem ([`single_line`]).
    Finish,
    /// Enter ou Shift+Enter: quebra de linha (o salvamento e automatico).
    NewLine,
    /// Esc: volta para a imagem sem apagar o texto.
    Exit,
    EraseBefore,
    EraseAfter,
    /// Setas, Home/End, com Cmd/Option; `true` = Shift (estende a selecao).
    Relocate(Movement, bool),
    /// Cmd+V.
    Paste,
    /// Cmd+A.
    SelectAll,
    /// Cmd+C: copia o selecionado.
    Copy,
    /// Cmd+X: copia o selecionado e apaga.
    Cut,
    /// Enter com Alt/Ctrl: tratado e ignorado (o sistema inseriria um "\r" solto).
    Ignore,
}

/// Movimentos do cursor (com Shift, estendem a selecao).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Movement {
    Left,
    Right,
    /// Linha visual de cima / de baixo, na mesma coluna (x) do primeiro movimento vertical.
    Up,
    Down,
    /// Home, Cmd+Esquerda: inicio da linha VISUAL.
    LineStart,
    /// End, Cmd+Direita: fim da linha VISUAL.
    LineEnd,
    /// Cmd+Cima.
    TextStart,
    /// Cmd+Baixo.
    TextEnd,
    /// Option+Esquerda: inicio da palavra.
    BeforeWord,
    /// Option+Direita: fim da palavra.
    AfterWord,
}

/// Uma linha VISUAL da caixa: um pedaco de uma linha logica (entre `\n`) que
/// a quebra automatica pos numa linha da tela.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineVisual {
    /// Bytes do texto (sem o `\n`).
    pub strip: Range<usize>,
    /// Indice da linha logica.
    pub logic: usize,
    /// A linha logica continua na linha visual seguinte (quebra automatica).
    pub continues: bool,
    /// Ultimo lugar do cursor nesta linha visual: o fim, ou, numa linha que
    /// continua, antes do ultimo caractere (no fim exato o cursor ja e da de baixo).
    pub end_cursor: usize,
}

/// As linhas visuais do texto: as linhas logicas cortadas nas quebras que o
/// gpui calculou para a largura da caixa (`WrappedLine::wrap_boundaries`).
/// Sem gpui: a view passa as quebras (bytes dentro de cada linha logica).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Visual {
    lines: Vec<LineVisual>,
}

impl Visual {
    /// `breaks[n]`: onde a linha logica `n` quebra (bytes relativos a ela).
    /// Quebras invalidas (0, no fim ou alem, no meio de um caractere,
    /// repetidas) sao ignoradas; linha sem entrada nao quebra.
    pub fn de(text: &str, breaks: &[Vec<usize>]) -> Visual {
        let mut lines = Vec::new();
        for (n, l) in logical_lines(text).into_iter().enumerate() {
            let tl = &text[l.clone()];
            let mut qs: Vec<usize> = breaks
                .get(n)
                .map(|q| q.iter().copied().filter(|&q| q > 0 && q < tl.len() && tl.is_char_boundary(q)).collect())
                .unwrap_or_default();
            qs.sort_unstable();
            qs.dedup();
            let mut ini = l.start;
            for q in qs {
                let end = l.start + q;
                let before = text[ini..end].char_indices().last().map_or(ini, |(i, _)| ini + i);
                lines.push(LineVisual { strip: ini..end, logic: n, continues: true, end_cursor: before });
                ini = end;
            }
            lines.push(LineVisual { strip: ini..l.end, logic: n, continues: false, end_cursor: l.end });
        }
        Visual { lines }
    }

    pub fn lines(&self) -> &[LineVisual] {
        &self.lines
    }

    /// Quantas linhas visuais (pelo menos 1).
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// A linha visual do offset `off` (numa quebra: a de baixo, onde o cursor e desenhado).
    pub fn line_of(&self, off: usize) -> usize {
        self.lines.iter().rposition(|l| l.strip.start <= off).unwrap_or(0)
    }

    /// `off` limitado a linha visual `i` (`faixa.start..=fim_cursor`).
    pub fn limit(&self, i: usize, off: usize) -> usize {
        match self.lines.get(i) {
            Some(l) => off.clamp(l.strip.start, l.end_cursor),
            None => off,
        }
    }
}

/// Medidas da tela que o movimento vertical precisa (a view mede com o layout
/// do gpui; os testes, com uma fonte monoespacada).
pub trait Measure {
    /// x (px) do offset `off` dentro da linha visual `line`.
    fn x(&self, line: usize, off: usize) -> f32;
    /// Offset mais proximo de `x` na linha visual `line`.
    fn index(&self, line: usize, x: f32) -> usize;
}

/// Faixas em bytes de cada linha logica (sem o `\n`).
fn logical_lines(text: &str) -> Vec<Range<usize>> {
    let mut v = Vec::new();
    let mut ini = 0;
    for (i, c) in text.char_indices() {
        if c == '\n' {
            v.push(ini..i);
            ini = i + 1;
        }
    }
    v.push(ini..text.len());
    v
}

fn of_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// A caixa cresce ate tantas linhas; passando disso, rola por dentro.
pub const MAX_LINES_VISIBLE: usize = 8;

/// Altura de linha usada quando a medida do gpui vem degenerada.
const HEIGHT_LINE_DEFAULT: f32 = 18.0;

/// Caixa de uma linha so (a de "Renomear fonte"): Enter confirma.
pub fn single_line(t: Key) -> Key {
    match t {
        Key::NewLine | Key::Finish => Key::Finish,
        o => o,
    }
}

/// So os algarismos de `t` (0-9): o que uma caixa "so numeros" aceita de uma digitacao ou colagem.
pub fn so_digits(t: &str) -> String {
    t.chars().filter(char::is_ascii_digit).collect()
}

/// Quantos algarismos tem `n` (no minimo 1): o que cabe numa caixa "ir para" de `n` itens.
pub fn digits(n: usize) -> usize {
    n.max(1).to_string().len()
}

/// Quantas linhas a caixa mostra para um texto de `total` linhas: de 1 a [`MAX_LINES_VISIBLE`].
pub fn visible_lines(total: usize) -> usize {
    total.clamp(1, MAX_LINES_VISIBLE)
}

/// Folga (px) a direita das linhas quebradas: o cursor no fim de uma linha
/// visual cheia (End) fica dentro da caixa, nao cortado na borda.
pub const BREAK_SLACK: f32 = 2.0;

/// Largura em que o texto da caixa quebra: a da caixa menos [`BREAK_SLACK`]
/// (pelo menos 1 px; largura invalida: sem quebra).
pub fn break_width(textbox: f32) -> Option<f32> {
    textbox.is_finite().then(|| (textbox - BREAK_SLACK).max(1.0))
}

/// Altura da area de texto da caixa: `linhas_visiveis(total) * altura_linha`.
pub fn textbox_height(total: usize, line_height: f32) -> f32 {
    let lh = if line_height.is_finite() && line_height > 0.0 { line_height } else { HEIGHT_LINE_DEFAULT };
    visible_lines(total) as f32 * lh
}

/// Primeira linha mostrada (rolagem interna). `follow`: o texto ou o cursor
/// mudou, entao a janela de linhas anda o minimo para o cursor (`cursor`, a
/// linha dele) ficar visivel; sem `follow` (roda do mouse) so o limite vale.
pub fn scroll(first: usize, cursor: usize, total: usize, follow: bool) -> usize {
    let vis = visible_lines(total);
    let max = total.saturating_sub(vis);
    let mut p = first.min(max);
    if follow {
        if cursor < p {
            p = cursor;
        } else if cursor >= p + vis {
            p = cursor + 1 - vis;
        }
    }
    p.min(max)
}

/// Tecla (nome do gpui: `"enter"`, `"backspace"`, `"v"`...) -> acao. `None`:
/// a tecla segue para o sistema (texto, composicao) ou e ignorada.
pub fn key(key: &str, shift: bool, cmd: bool, ctrl: bool, alt: bool) -> Option<Key> {
    if key == "enter" {
        let t = match (cmd, ctrl || alt) {
            (_, true) => Key::Ignore,
            (true, false) => Key::Finish,
            (false, false) => Key::NewLine,
        };
        return Some(t);
    }
    if ctrl {
        return None;
    }
    use Movement::*;
    let mov = |m| Key::Relocate(m, shift);
    let t = match key {
        "escape" => Key::Exit,
        "backspace" => Key::EraseBefore,
        "delete" => Key::EraseAfter,
        "left" if cmd => mov(LineStart),
        "right" if cmd => mov(LineEnd),
        "left" if alt => mov(BeforeWord),
        "right" if alt => mov(AfterWord),
        "left" => mov(Left),
        "right" => mov(Right),
        "up" if cmd => mov(TextStart),
        "down" if cmd => mov(TextEnd),
        "up" => mov(Up),
        "down" => mov(Down),
        "home" => mov(LineStart),
        "end" => mov(LineEnd),
        // Cmd-Shift-V e o seletor de vault: segue para a view raiz.
        "v" if cmd && !shift => Key::Paste,
        "a" if cmd => Key::SelectAll,
        "c" if cmd && !shift => Key::Copy,
        "x" if cmd && !shift => Key::Cut,
        _ => return None,
    };
    Some(t)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Buffer {
    text: String,
    /// Selecao em bytes; vazia = cursor em `sel.end`.
    sel: Range<usize>,
    /// O cursor esta no inicio da selecao (selecionou para tras).
    inverted: bool,
    /// Trecho em composicao (sublinhado), em bytes.
    marked: Option<Range<usize>>,
    /// A caixa deve limpar quando a composicao terminar ([`Buffer::clear_or_postpone`]).
    clear_postponed: bool,
    /// Coluna (x) que Cima/Baixo seguidos mantem; qualquer outra acao esquece.
    target_column: Option<f32>,
}

impl Buffer {
    /// Move o cursor (`extend`: Shift, a selecao vai da ancora ate o cursor novo).
    /// `vis`/`med`: as linhas visuais da caixa e as medidas da tela (Cima/Baixo
    /// e Home/End andam pelas linhas da TELA, nao pelas do texto).
    pub fn relocate_cursor(&mut self, m: Movement, extend: bool, vis: &Visual, med: &dyn Measure) {
        use Movement::*;
        let c = self.cursor();
        let target = match m {
            Left if !extend && !self.sel.is_empty() => self.sel.start,
            Right if !extend && !self.sel.is_empty() => self.sel.end,
            Left => self.previous(c),
            Right => self.next(c),
            TextStart => 0,
            TextEnd => self.text.len(),
            LineStart => vis.lines().get(vis.line_of(c)).map_or(0, |l| l.strip.start),
            LineEnd => vis.lines().get(vis.line_of(c)).map_or(self.text.len(), |l| l.end_cursor),
            BeforeWord => self.before_word(c),
            AfterWord => self.after_word(c),
            Up | Down => {
                let i = vis.line_of(c);
                let x = self.target_column.unwrap_or_else(|| med.x(i, c));
                let target = match m {
                    Up if i == 0 => 0,
                    Up => vis.limit(i - 1, med.index(i - 1, x)),
                    _ if i + 1 >= vis.len() => self.text.len(),
                    _ => vis.limit(i + 1, med.index(i + 1, x)),
                };
                self.ir(target, extend);
                self.target_column = Some(x);
                return;
            }
        };
        self.ir(target, extend);
    }

    /// Poe o cursor em `off`; com `extend`, a selecao vai da ancora (a ponta
    /// que nao e o cursor) ate `off`.
    fn ir(&mut self, off: usize, extend: bool) {
        let mut off = off.min(self.text.len());
        while !self.text.is_char_boundary(off) {
            off -= 1;
        }
        if !extend {
            self.relocate(off);
            return;
        }
        let anchor = if self.sel.is_empty() {
            self.cursor()
        } else if self.inverted {
            self.sel.end
        } else {
            self.sel.start
        };
        self.sel = off.min(anchor)..off.max(anchor);
        self.inverted = off < anchor;
        self.marked = None;
        self.target_column = None;
    }

    /// Inicio da palavra antes de `off` (pula o que nao e palavra, depois a palavra).
    fn before_word(&self, off: usize) -> usize {
        let mut i = off;
        let mut in_word = false;
        for (j, c) in self.text[..off].char_indices().rev() {
            if of_word(c) {
                in_word = true;
            } else if in_word {
                break;
            }
            i = j;
        }
        i
    }

    /// Fim da palavra depois de `off`.
    fn after_word(&self, off: usize) -> usize {
        let mut in_word = false;
        for (j, c) in self.text[off..].char_indices() {
            if of_word(c) {
                in_word = true;
            } else if in_word {
                return off + j;
            }
        }
        self.text.len()
    }

    pub fn new() -> Buffer {
        Buffer::default()
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn selection(&self) -> Range<usize> {
        self.sel.clone()
    }

    pub fn inverted(&self) -> bool {
        self.inverted
    }

    pub fn marked(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    pub fn cursor(&self) -> usize {
        if self.inverted {
            self.sel.start
        } else {
            self.sel.end
        }
    }

    pub fn clear(&mut self) {
        *self = Buffer::default();
    }

    // ---- UTF-16 (contrato do InputHandler) ----

    /// Offset em bytes -> unidades UTF-16 (limitado ao fim do texto).
    pub fn to_utf_16(&self, bytes: usize) -> usize {
        let mut u = 0;
        for (i, c) in self.text.char_indices() {
            if i >= bytes {
                break;
            }
            u += c.len_utf16();
        }
        u
    }

    /// Unidades UTF-16 -> offset em bytes, numa fronteira de caractere
    /// (limitado ao fim do texto; metade de um par substituto arredonda para cima).
    pub fn of_utf_16(&self, u16s: usize) -> usize {
        let mut u = 0;
        for (i, c) in self.text.char_indices() {
            if u >= u16s {
                return i;
            }
            u += c.len_utf16();
        }
        self.text.len()
    }

    fn utf_16_strip(&self, r: &Range<usize>) -> Range<usize> {
        let (a, b) = (self.of_utf_16(r.start), self.of_utf_16(r.end));
        a.min(b)..a.max(b)
    }

    pub fn strip_to_utf_16(&self, r: &Range<usize>) -> Range<usize> {
        self.to_utf_16(r.start)..self.to_utf_16(r.end)
    }

    /// Texto na faixa UTF-16 pedida e a faixa realmente devolvida.
    pub fn text_utf_16(&self, r: Range<usize>) -> (String, Range<usize>) {
        let b = self.utf_16_strip(&r);
        (self.text[b.clone()].to_string(), self.strip_to_utf_16(&b))
    }

    pub fn selection_utf_16(&self) -> Range<usize> {
        self.strip_to_utf_16(&self.sel)
    }

    pub fn marked_utf_16(&self) -> Option<Range<usize>> {
        self.marked.as_ref().map(|r| self.strip_to_utf_16(r))
    }

    /// Faixa alvo de uma substituicao: a pedida, senao a marcada, senao a selecao.
    fn target(&self, r16: Option<Range<usize>>) -> Range<usize> {
        r16.map(|r| self.utf_16_strip(&r))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.sel.clone())
    }

    /// `insertText`: troca a faixa por `new` (texto final, ja composto) e
    /// encerra a composicao.
    pub fn replace_utf_16(&mut self, r16: Option<Range<usize>>, new: &str) {
        if self.clear_postponed && self.marked.is_some() {
            // A nota terminou durante a composicao: so o que foi composto fica.
            self.restart_with(new);
            return;
        }
        let target = self.target(r16);
        self.text.replace_range(target.clone(), new);
        let end = target.start + new.len();
        self.sel = end..end;
        self.inverted = false;
        self.marked = None;
        self.target_column = None;
    }

    /// Limpa a caixa, ou, no meio de uma composicao (IME / tecla morta),
    /// deixa para quando ela terminar (limpar agora desfaria a composicao no
    /// sistema). Devolve se limpou agora.
    pub fn clear_or_postpone(&mut self) -> bool {
        if self.marked.is_some() {
            self.clear_postponed = true;
            false
        } else {
            self.clear();
            true
        }
    }

    /// Ha uma limpeza esperando a composicao terminar.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn postponed_cleanup(&self) -> bool {
        self.clear_postponed
    }

    /// Fim da composicao por outro caminho (tecla, colar): a limpeza adiada
    /// acontece agora. Devolve se limpou.
    pub fn conclude_cleanup_postponed(&mut self) -> bool {
        if self.clear_postponed && self.marked.is_none() {
            self.clear();
            true
        } else {
            false
        }
    }

    fn restart_with(&mut self, t: &str) {
        self.clear();
        self.text = t.to_string();
        self.sel = t.len()..t.len();
    }

    /// `setMarkedText`: troca a faixa por `new` e o deixa marcado (em
    /// composicao). `sel16` e a selecao DENTRO de `new`, em UTF-16.
    pub fn replace_and_mark_utf_16(&mut self, r16: Option<Range<usize>>, new: &str, sel16: Option<Range<usize>>) {
        if self.clear_postponed && self.marked.is_some() && new.is_empty() {
            // Composicao cancelada depois de a nota terminar: a caixa limpa.
            self.clear();
            return;
        }
        let target = self.target(r16);
        self.text.replace_range(target.clone(), new);
        self.target_column = None;
        self.marked = (!new.is_empty()).then(|| target.start..target.start + new.len());
        let inside = |u: usize| {
            let mut n = 0;
            for (i, c) in new.char_indices() {
                if n >= u {
                    return i;
                }
                n += c.len_utf16();
            }
            new.len()
        };
        self.sel = match sel16 {
            Some(s) => {
                let (a, b) = (inside(s.start), inside(s.end));
                target.start + a.min(b)..target.start + a.max(b)
            }
            None => target.start + new.len()..target.start + new.len(),
        };
        self.inverted = false;
    }

    /// `unmarkText`: o texto marcado fica como esta.
    pub fn uncheck(&mut self) {
        if let Some(m) = self.marked.take().filter(|_| self.clear_postponed) {
            let t = self.text[m].to_string();
            self.restart_with(&t);
        }
    }

    // ---- edicao pelo teclado ----

    /// Insere `s` no lugar da selecao (colar, quebra de linha).
    pub fn insert(&mut self, s: &str) {
        self.marked = None;
        self.target_column = None;
        let target = self.sel.clone();
        self.text.replace_range(target.clone(), s);
        let end = target.start + s.len();
        self.sel = end..end;
        self.inverted = false;
    }

    /// Inicio do caractere visivel antes de `off` (pula marcas combinantes,
    /// para `a` + acento combinante sair inteiro).
    fn previous(&self, off: usize) -> usize {
        let mut i = off;
        for (j, c) in self.text[..off].char_indices().rev() {
            i = j;
            if !is_combining_mark(c) {
                break;
            }
        }
        i
    }

    /// Fim do caractere visivel que comeca em `off` (com as marcas combinantes).
    fn next(&self, off: usize) -> usize {
        let mut it = self.text[off..].char_indices();
        let Some((_, c)) = it.next() else { return off };
        let mut end = off + c.len_utf8();
        for (j, c) in it {
            if !is_combining_mark(c) {
                break;
            }
            end = off + j + c.len_utf8();
        }
        end
    }

    fn relocate(&mut self, off: usize) {
        self.sel = off..off;
        self.inverted = false;
        self.marked = None;
        self.target_column = None;
    }

    pub fn erase_before(&mut self) {
        if self.sel.is_empty() {
            let a = self.previous(self.sel.start);
            self.sel = a..self.sel.end;
        }
        self.insert("");
    }

    pub fn erase_after(&mut self) {
        if self.sel.is_empty() {
            let b = self.next(self.sel.end);
            self.sel = self.sel.start..b;
        }
        self.insert("");
    }

    // As quatro abaixo andam pelas linhas LOGICAS (testes do buffer); a caixa usa
    // `relocate_cursor`, que anda pelas linhas da tela.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn left(&mut self) {
        let target = if self.sel.is_empty() { self.previous(self.sel.start) } else { self.sel.start };
        self.relocate(target);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn right(&mut self) {
        let target = if self.sel.is_empty() { self.next(self.sel.end) } else { self.sel.end };
        self.relocate(target);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn line_start(&mut self) {
        let c = self.cursor();
        let i = self.text[..c].rfind('\n').map(|i| i + 1).unwrap_or(0);
        self.relocate(i);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn line_end(&mut self) {
        let c = self.cursor();
        let i = self.text[c..].find('\n').map(|i| c + i).unwrap_or(self.text.len());
        self.relocate(i);
    }

    /// O texto selecionado (vazio se e so o cursor).
    pub fn selected(&self) -> &str {
        &self.text[self.sel.clone()]
    }

    /// Recortar: devolve o selecionado e o apaga; sem selecao, nada muda.
    pub fn cut(&mut self) -> Option<String> {
        if self.sel.is_empty() {
            return None;
        }
        let s = self.selected().to_string();
        self.insert("");
        Some(s)
    }

    pub fn select_all(&mut self) {
        self.sel = 0..self.text.len();
        self.inverted = false;
        self.marked = None;
        self.target_column = None;
    }

    /// Poe o cursor em `off` (clique), ajustado a uma fronteira de caractere.
    pub fn position(&mut self, off: usize) {
        let mut off = off.min(self.text.len());
        while !self.text.is_char_boundary(off) {
            off -= 1;
        }
        self.relocate(off);
    }

    /// Shift+clique / arrasto: a selecao vai da ancora ate `off`.
    pub fn extend_ate(&mut self, off: usize) {
        self.ir(off, true);
    }

    /// Duplo clique: seleciona a palavra que contem `off` (ou que termina nele);
    /// fora de palavra, so poe o cursor.
    pub fn select_word(&mut self, off: usize) {
        let mut off = off.min(self.text.len());
        while !self.text.is_char_boundary(off) {
            off -= 1;
        }
        let after = self.text[off..].chars().next().is_some_and(of_word);
        let before = self.text[..off].chars().next_back().is_some_and(of_word);
        if !after && !before {
            self.relocate(off);
            return;
        }
        let ini = self.text[..off].char_indices().rev().take_while(|(_, c)| of_word(*c)).last().map_or(off, |(i, _)| i);
        let end = self.text[off..].char_indices().find(|(_, c)| !of_word(*c)).map_or(self.text.len(), |(i, _)| off + i);
        self.sel = ini..end;
        self.inverted = false;
        self.marked = None;
        self.target_column = None;
    }

    /// Triplo clique: seleciona a linha (logica) que contem `off`.
    pub fn select_line(&mut self, off: usize) {
        let off = off.min(self.text.len());
        let strip = self.lines().into_iter().find(|l| off <= l.end).unwrap_or(0..self.text.len());
        self.sel = strip;
        self.inverted = false;
        self.marked = None;
        self.target_column = None;
    }

    // ---- layout (linhas) ----

    /// Faixas em bytes de cada linha (sem o `\n`); texto vazio = uma linha vazia.
    pub fn lines(&self) -> Vec<Range<usize>> {
        let mut v = Vec::new();
        let mut ini = 0;
        for (i, c) in self.text.char_indices() {
            if c == '\n' {
                v.push(ini..i);
                ini = i + 1;
            }
        }
        v.push(ini..self.text.len());
        v
    }

    /// `(linha, offset dentro da linha)` de um offset do texto (linhas logicas;
    /// a tela usa as visuais, `Visual`).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn line_and_column(&self, off: usize) -> (usize, usize) {
        let lines = self.lines();
        for (n, l) in lines.iter().enumerate() {
            if off <= l.end {
                return (n, off.saturating_sub(l.start));
            }
        }
        let n = lines.len() - 1;
        (n, lines[n].len())
    }
}

#[cfg(test)]
#[path = "tests/quick_note.rs"]
mod tests;
