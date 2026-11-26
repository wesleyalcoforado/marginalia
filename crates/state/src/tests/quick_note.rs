use super::*;

fn typing(b: &mut Buffer, s: &str) {
    b.replace_utf_16(None, s);
}

#[test]
fn enter_break_the_line_and_cmd_enter_finishes() {
    // Task 8: o salvamento e automatico; Enter so quebra a linha (transcricoes).
    assert_eq!(key("enter", false, false, false, false), Some(Key::NewLine));
    assert_eq!(key("enter", true, false, false, false), Some(Key::NewLine));
    assert_eq!(key("enter", false, true, false, false), Some(Key::Finish));
    assert_eq!(key("enter", true, true, false, false), Some(Key::Finish), "Cmd+Shift+Enter tambem finaliza");
    assert_eq!(key("escape", false, false, false, false), Some(Key::Exit));
    // Alt/Ctrl+Enter: tratados e ignorados (nunca um "\r" solto do sistema)
    for (shift, cmd, ctrl, alt) in [(false, false, true, false), (false, false, false, true), (false, true, false, true), (false, true, true, false)] {
        assert_eq!(key("enter", shift, cmd, ctrl, alt), Some(Key::Ignore), "{shift} {cmd} {ctrl} {alt}");
    }
}

#[test]
fn textbox_of_a_line_confirms_with_enter() {
    // A caixa "Renomear fonte" reaproveita o editor: Enter (com ou sem Cmd) confirma.
    assert_eq!(single_line(Key::NewLine), Key::Finish);
    assert_eq!(single_line(Key::Finish), Key::Finish);
    for t in [Key::Exit, Key::Ignore, Key::EraseBefore, Key::Paste, Key::Relocate(Movement::Left, false)] {
        assert_eq!(single_line(t), t);
    }
}

/// Medida de teste: cada caractere tem 1 de largura (fonte monoespacada).
struct Mono<'a>(&'a str, &'a Visual);

impl Measure for Mono<'_> {
    fn x(&self, line: usize, off: usize) -> f32 {
        let ini = self.1.lines()[line].strip.start;
        self.0[ini..off.max(ini)].chars().count() as f32
    }
    fn index(&self, line: usize, x: f32) -> usize {
        let l = &self.1.lines()[line];
        let n = x.round().max(0.0) as usize;
        self.0[l.strip.clone()].char_indices().nth(n).map_or(l.strip.end, |(i, _)| l.strip.start + i)
    }
}

fn relocate(b: &mut Buffer, m: Movement, extend: bool, breaks: &[Vec<usize>]) {
    let t = b.text().to_string();
    let vis = Visual::de(&t, breaks);
    b.relocate_cursor(m, extend, &vis, &Mono(&t, &vis));
}

#[test]
fn lines_visual_follow_as_breaks() {
    let t = "abc def ghi\nxy";
    let v = Visual::de(t, &[vec![4, 8], vec![]]);
    let strips: Vec<_> = v.lines().iter().map(|l| (l.strip.clone(), l.logic, l.continues)).collect();
    assert_eq!(strips, vec![(0..4, 0, true), (4..8, 0, true), (8..11, 0, false), (12..14, 1, false)]);
    assert_eq!(v.len(), 4);
    // cursor na quebra fica no comeco da linha visual seguinte; no fim da linha logica, nela
    assert_eq!([v.line_of(0), v.line_of(3), v.line_of(4), v.line_of(11), v.line_of(12), v.line_of(99)], [0, 0, 1, 2, 3, 3]);
    // fim de uma linha quebrada: antes do ultimo caractere (senao o cursor cairia na de baixo)
    assert_eq!(v.lines()[0].end_cursor, 3);
    assert_eq!(v.lines()[2].end_cursor, 11);
    assert_eq!(v.limit(0, 50), 3);
    assert_eq!(v.limit(1, 0), 4);
    // quebras invalidas (0, no fim, fora, repetidas, fora de ordem, no meio de um caractere) sao ignoradas
    let t = "ção ab";
    // "ção ab": ç 0..2, ã 2..4, o 4, espaco 5, a 6, b 7
    let v = Visual::de(t, &[vec![0, 1, 9, 6, 4, 4, 3, 99]]);
    let strips: Vec<_> = v.lines().iter().map(|l| l.strip.clone()).collect();
    assert_eq!(strips, vec![0..4, 4..6, 6..8]);
    // texto vazio: uma linha vazia
    assert_eq!(Visual::de("", &[]).lines().iter().map(|l| l.strip.clone()).collect::<Vec<_>>(), vec![0..0]);
    // a altura da caixa conta linhas VISUAIS
    assert_eq!(textbox_height(Visual::de("abc def ghi\nxy", &[vec![4, 8]]).len(), 10.0), 40.0);
}

#[test]
fn up_and_down_store_the_column() {
    let mut b = Buffer::new();
    typing(&mut b, "abcdefgh\nab\nabcdefgh");
    b.position(6);
    relocate(&mut b, Movement::Down, false, &[]);
    assert_eq!(b.cursor(), 11, "linha curta: no fim dela");
    relocate(&mut b, Movement::Down, false, &[]);
    assert_eq!(b.cursor(), 18, "a coluna 6 volta na linha comprida");
    relocate(&mut b, Movement::Up, false, &[]);
    relocate(&mut b, Movement::Up, false, &[]);
    assert_eq!(b.cursor(), 6);
    relocate(&mut b, Movement::Up, false, &[]);
    assert_eq!(b.cursor(), 0, "acima da 1a linha: inicio do texto");
    b.position(15);
    relocate(&mut b, Movement::Down, false, &[]);
    assert_eq!(b.cursor(), b.text().len(), "abaixo da ultima: fim do texto");
    // qualquer outro movimento esquece a coluna
    b.position(6);
    relocate(&mut b, Movement::Down, false, &[]);
    relocate(&mut b, Movement::Left, false, &[]);
    relocate(&mut b, Movement::Down, false, &[]);
    assert_eq!(b.cursor(), 12 + 1, "coluna nova: 1");
}

#[test]
fn up_and_down_walk_by_the_lines_broken() {
    // uma linha logica so, quebrada em 3 linhas visuais de 4
    let mut b = Buffer::new();
    typing(&mut b, "abc def ghij");
    let q = [vec![4, 8]];
    b.position(1);
    relocate(&mut b, Movement::Down, false, &q);
    assert_eq!(b.cursor(), 5);
    relocate(&mut b, Movement::Down, false, &q);
    assert_eq!(b.cursor(), 9);
    relocate(&mut b, Movement::Up, false, &q);
    assert_eq!(b.cursor(), 5);
    // Home/End: inicio/fim da linha VISUAL
    relocate(&mut b, Movement::LineStart, false, &q);
    assert_eq!(b.cursor(), 4);
    relocate(&mut b, Movement::LineEnd, false, &q);
    assert_eq!(b.cursor(), 7, "antes do espaco da quebra, ainda na mesma linha visual");
    // coluna alem do fim de uma linha quebrada nao pula para a seguinte
    b.position(12);
    relocate(&mut b, Movement::Up, false, &q);
    assert_eq!(b.cursor(), 7);
}

#[test]
fn shift_extends_the_selection() {
    let mut b = Buffer::new();
    typing(&mut b, "abcdefgh\nab\nabcdefgh");
    b.position(6);
    relocate(&mut b, Movement::Down, true, &[]);
    assert_eq!((b.selection(), b.inverted()), (6..11, false));
    relocate(&mut b, Movement::Up, true, &[]);
    relocate(&mut b, Movement::Up, true, &[]);
    assert_eq!((b.selection(), b.inverted(), b.cursor()), (0..6, true, 0), "a ancora fica no 6");
    relocate(&mut b, Movement::Right, true, &[]);
    assert_eq!((b.selection(), b.cursor()), (1..6, 1));
    relocate(&mut b, Movement::TextEnd, true, &[]);
    assert_eq!((b.selection(), b.inverted()), (6..20, false));
    relocate(&mut b, Movement::TextStart, false, &[]);
    assert_eq!(b.selection(), 0..0, "sem Shift: so o cursor");
    relocate(&mut b, Movement::LineEnd, true, &[]);
    assert_eq!(b.selection(), 0..8);
    // setas sem Shift com selecao: vao para a ponta dela
    relocate(&mut b, Movement::Left, false, &[]);
    assert_eq!(b.selection(), 0..0);
    b.select_all();
    relocate(&mut b, Movement::Right, false, &[]);
    assert_eq!(b.selection(), 20..20);
    // digitar substitui a selecao feita com Shift
    relocate(&mut b, Movement::BeforeWord, true, &[]);
    typing(&mut b, "xyz");
    assert_eq!(b.text(), "abcdefgh\nab\nxyz");
}

#[test]
fn option_walks_by_words() {
    let mut b = Buffer::new();
    typing(&mut b, "Maria da Conceição, filha");
    relocate(&mut b, Movement::BeforeWord, false, &[]);
    assert_eq!(b.cursor(), "Maria da Conceição, ".len());
    relocate(&mut b, Movement::BeforeWord, false, &[]);
    assert_eq!(b.cursor(), "Maria da ".len());
    relocate(&mut b, Movement::BeforeWord, false, &[]);
    relocate(&mut b, Movement::BeforeWord, false, &[]);
    assert_eq!(b.cursor(), 0);
    relocate(&mut b, Movement::BeforeWord, false, &[]);
    assert_eq!(b.cursor(), 0);
    relocate(&mut b, Movement::AfterWord, false, &[]);
    assert_eq!(b.cursor(), "Maria".len());
    relocate(&mut b, Movement::AfterWord, false, &[]);
    relocate(&mut b, Movement::AfterWord, false, &[]);
    assert_eq!(b.cursor(), "Maria da Conceição".len(), "para no fim da palavra, antes da virgula");
}

#[test]
fn movement_keys() {
    use Movement::*;
    let t = |k, shift, cmd, alt| key(k, shift, cmd, false, alt);
    assert_eq!(t("left", false, false, false), Some(Key::Relocate(Left, false)));
    assert_eq!(t("left", true, false, false), Some(Key::Relocate(Left, true)));
    assert_eq!(t("right", false, false, false), Some(Key::Relocate(Right, false)));
    assert_eq!(t("up", false, false, false), Some(Key::Relocate(Up, false)));
    assert_eq!(t("down", true, false, false), Some(Key::Relocate(Down, true)));
    assert_eq!(t("home", false, false, false), Some(Key::Relocate(LineStart, false)));
    assert_eq!(t("end", true, false, false), Some(Key::Relocate(LineEnd, true)));
    assert_eq!(t("left", false, true, false), Some(Key::Relocate(LineStart, false)));
    assert_eq!(t("right", true, true, false), Some(Key::Relocate(LineEnd, true)));
    assert_eq!(t("up", false, true, false), Some(Key::Relocate(TextStart, false)));
    assert_eq!(t("down", true, true, false), Some(Key::Relocate(TextEnd, true)));
    assert_eq!(t("left", false, false, true), Some(Key::Relocate(BeforeWord, false)));
    assert_eq!(t("right", true, false, true), Some(Key::Relocate(AfterWord, true)));
    assert_eq!(t("a", false, true, false), Some(Key::SelectAll));
}

#[test]
fn height_of_the_textbox_goes_of_1_the_8_lines() {
    // a quebra deixa uma folga para o cursor no fim de uma linha cheia
    assert_eq!(break_width(300.0), Some(300.0 - BREAK_SLACK));
    assert_eq!(break_width(1.5), Some(1.0), "caixa minuscula: 1 px");
    assert_eq!(break_width(f32::NAN), None);
    assert_eq!(break_width(f32::INFINITY), None);
    assert_eq!(visible_lines(0), 1);
    assert_eq!(visible_lines(1), 1);
    assert_eq!(visible_lines(5), 5);
    assert_eq!(visible_lines(8), 8);
    assert_eq!(visible_lines(9), MAX_LINES_VISIBLE);
    assert_eq!(visible_lines(500), 8);
    assert_eq!(textbox_height(1, 18.0), 18.0);
    assert_eq!(textbox_height(3, 18.0), 54.0);
    assert_eq!(textbox_height(40, 18.0), 8.0 * 18.0, "passa de 8 linhas: rola por dentro");
    for bad in [0.0, -3.0, f32::NAN, f32::INFINITY] {
        let a = textbox_height(3, bad);
        assert!(a.is_finite() && a > 0.0, "{bad} -> {a}");
    }
}

#[test]
fn scroll_keeps_the_cursor_visible() {
    // ate 8 linhas: nada rola
    assert_eq!(scroll(0, 4, 8, true), 0);
    assert_eq!(scroll(3, 4, 8, true), 0, "rolagem velha maior que o possivel volta ao topo");
    // 20 linhas, cursor na ultima: mostra as 8 ultimas
    assert_eq!(scroll(0, 19, 20, true), 12);
    // cursor sobe acima da janela: a janela sobe junto
    assert_eq!(scroll(12, 5, 20, true), 5);
    // cursor dentro da janela: fica
    assert_eq!(scroll(5, 9, 20, true), 5);
    // cursor logo abaixo da janela: desce uma
    assert_eq!(scroll(5, 13, 20, true), 6);
    // rolagem pela roda (sem seguir o cursor): so o limite vale
    assert_eq!(scroll(10, 0, 20, false), 10);
    assert_eq!(scroll(99, 0, 20, false), 12);
    // texto encolheu (apagou linhas)
    assert_eq!(scroll(12, 2, 3, true), 0);
}

#[test]
fn composition_crossing_breaks_of_line() {
    // Enter = nova linha nao pode estragar a composicao (tecla morta) na 2a linha
    let mut b = Buffer::new();
    typing(&mut b, "Joao");
    b.insert("\n");
    typing(&mut b, "Concei");
    b.replace_and_mark_utf_16(None, "´", None);
    assert_eq!(b.marked(), Some(11..13), "o acento marcado fica na 2a linha");
    assert_eq!(b.marked_utf_16(), Some(11..12));
    assert_eq!(b.line_and_column(b.cursor()), (1, 8));
    typing(&mut b, "ç");
    typing(&mut b, "ão");
    b.insert("\n");
    b.replace_and_mark_utf_16(None, "~", None);
    typing(&mut b, "ã");
    assert_eq!(b.text(), "Joao\nConceição\nã");
    assert_eq!(b.lines().len(), 3);
    assert_eq!(b.marked(), None);
    // o InputHandler pede trechos que atravessam o "\n" (UTF-16)
    let (t, r) = b.text_utf_16(3..7);
    assert_eq!((t.as_str(), r), ("o\nCo", 3..7));
    // e substitui uma faixa que comeca numa linha e termina na outra
    b.replace_utf_16(Some(4..5), " ");
    assert_eq!(b.text(), "Joao Conceição\nã");
}

#[test]
fn textbox_keys() {
    assert_eq!(key("escape", false, false, false, false), Some(Key::Exit));
    assert_eq!(key("backspace", false, false, false, false), Some(Key::EraseBefore));
    assert_eq!(key("delete", false, false, false, false), Some(Key::EraseAfter));
    assert_eq!(key("v", false, true, false, false), Some(Key::Paste));
    assert_eq!(key("v", true, true, false, false), None, "Cmd-Shift-V nao cola: e o seletor de vault");
    assert_eq!(key("a", false, true, false, false), Some(Key::SelectAll));
    assert_eq!(key("c", false, true, false, false), Some(Key::Copy));
    assert_eq!(key("x", false, true, false, false), Some(Key::Cut));
    // texto comum (inclusive `[`, `f`, `i` que sao atalhos na imagem) segue para o InputHandler
    for k in ["a", "v", "[", "f", "i", "space", "/"] {
        assert_eq!(key(k, false, false, false, false), None, "{k}");
    }
}

#[test]
fn selected_and_cut() {
    let mut b = Buffer::new();
    b.insert("João e Maria");
    assert_eq!(b.selected(), "", "so o cursor");
    b.select_all();
    assert_eq!(b.selected(), "João e Maria");
    b.sel = 0.."João".len();
    assert_eq!(b.selected(), "João");
    // recortar = copiar o selecionado e apagar
    assert_eq!(b.cut().as_deref(), Some("João"));
    assert_eq!(b.text(), " e Maria");
    assert_eq!(b.cut(), None, "nada selecionado: nada recortado");
    assert_eq!(b.text(), " e Maria");
}

#[test]
fn typing_erase_and_relocate() {
    let mut b = Buffer::new();
    typing(&mut b, "Maria");
    typing(&mut b, " herdeira");
    assert_eq!(b.text(), "Maria herdeira");
    b.erase_before();
    assert_eq!(b.text(), "Maria herdeir");
    b.line_start();
    assert_eq!(b.cursor(), 0);
    b.erase_after();
    assert_eq!(b.text(), "aria herdeir");
    b.right();
    b.right();
    assert_eq!(b.cursor(), 2);
    b.left();
    assert_eq!(b.cursor(), 1);
    b.line_end();
    assert_eq!(b.cursor(), b.text().len());
    b.erase_after(); // no fim: nada
    assert_eq!(b.text(), "aria herdeir");
    b.line_start();
    b.erase_before(); // no inicio: nada
    assert_eq!(b.text(), "aria herdeir");
}

#[test]
fn accents_count_as_one_character() {
    let mut b = Buffer::new();
    typing(&mut b, "Conceição");
    b.erase_before();
    assert_eq!(b.text(), "Conceiçã");
    b.left();
    b.erase_before();
    assert_eq!(b.text(), "Conceiã");
    // acento combinante (NFD) sai junto com a letra
    let mut b = Buffer::new();
    typing(&mut b, "Jose\u{301}");
    b.erase_before();
    assert_eq!(b.text(), "Jos");
    let mut b = Buffer::new();
    typing(&mut b, "a\u{301}b");
    b.line_start();
    b.right();
    assert_eq!(b.cursor(), "a\u{301}".len());
}

#[test]
fn key_dead_composes_accent() {
    // macOS: `´` (tecla morta) chega como setMarkedText, e `a` como insertText na faixa marcada
    let mut b = Buffer::new();
    typing(&mut b, "Jo");
    b.replace_and_mark_utf_16(None, "´", None);
    assert_eq!(b.text(), "Jo´");
    assert_eq!(b.marked(), Some(2..4));
    assert_eq!(b.marked_utf_16(), Some(2..3));
    typing(&mut b, "ã");
    assert_eq!(b.text(), "Joã");
    assert_eq!(b.marked(), None);
    typing(&mut b, "o");
    assert_eq!(b.text(), "João");
    // c cedilha e crase pelo mesmo caminho
    b.replace_and_mark_utf_16(None, "`", None);
    typing(&mut b, "à");
    b.replace_and_mark_utf_16(None, "´", None);
    typing(&mut b, "ç");
    assert_eq!(b.text(), "Joãoàç");
    // composicao cancelada (Esc no IME): marca vazia remove o acento pendente
    b.replace_and_mark_utf_16(None, "~", None);
    b.replace_and_mark_utf_16(None, "", None);
    assert_eq!(b.text(), "Joãoàç");
    assert_eq!(b.marked(), None);
    // unmarkText mantem o que estava marcado
    b.replace_and_mark_utf_16(None, "^", None);
    b.uncheck();
    assert_eq!(b.text(), "Joãoàç^");
}

#[test]
fn selection_inside_of_the_text_marked() {
    let mut b = Buffer::new();
    typing(&mut b, "ab");
    // IME de varias letras: marca "xyz" com o cursor depois do 1o caractere
    b.replace_and_mark_utf_16(None, "xyz", Some(1..1));
    assert_eq!(b.text(), "abxyz");
    assert_eq!(b.selection(), 3..3);
    assert_eq!(b.marked(), Some(2..5));
}

#[test]
fn clear_in_the_middle_of_the_composition_wait_ela_finish() {
    let mut b = Buffer::new();
    assert!(b.clear_or_postpone(), "sem composicao: limpa ja");
    typing(&mut b, "Ana ");
    b.replace_and_mark_utf_16(None, "´", None);
    assert!(!b.clear_or_postpone(), "com o acento marcado: adia");
    assert!(b.postponed_cleanup());
    assert_eq!(b.text(), "Ana ´", "a composicao segue intacta");
    b.replace_utf_16(None, "á");
    assert_eq!((b.text(), b.selection(), b.marked()), ("á", 2..2, None), "so o composto fica");
    assert!(!b.postponed_cleanup());
    // unmarkText: o marcado vira o texto da caixa nova
    typing(&mut b, "x");
    b.replace_and_mark_utf_16(None, "ˆ", None);
    b.clear_or_postpone();
    b.uncheck();
    assert_eq!(b.text(), "ˆ");
    // composicao cancelada: a caixa limpa
    b.replace_and_mark_utf_16(None, "´", None);
    b.clear_or_postpone();
    b.replace_and_mark_utf_16(None, "", None);
    assert!(b.empty() && !b.postponed_cleanup());
    // terminada por outra tecla
    typing(&mut b, "y");
    b.replace_and_mark_utf_16(None, "´", None);
    b.clear_or_postpone();
    b.insert("\n");
    assert!(b.conclude_cleanup_postponed() && b.empty());
}

#[test]
fn utf_16_forward_and_back() {
    let mut b = Buffer::new();
    typing(&mut b, "aç🙂b");
    // a=1, ç=1, 🙂=2 unidades UTF-16
    assert_eq!(b.to_utf_16(b.text().len()), 5);
    assert_eq!(b.of_utf_16(2), "aç".len());
    assert_eq!(b.of_utf_16(4), "aç🙂".len());
    assert_eq!(b.of_utf_16(3), "aç🙂".len(), "metade do par substituto arredonda para cima");
    assert_eq!(b.of_utf_16(99), b.text().len());
    let (t, r) = b.text_utf_16(1..4);
    assert_eq!(t, "ç🙂");
    assert_eq!(r, 1..4);
    assert_eq!(b.selection_utf_16(), 5..5);
    // substituir por faixa UTF-16 explicita (o sistema troca o 🙂)
    b.replace_utf_16(Some(2..4), "x");
    assert_eq!(b.text(), "açxb");
    assert_eq!(b.cursor(), "açx".len());
}

#[test]
fn break_of_line_paste_and_lines() {
    let mut b = Buffer::new();
    typing(&mut b, "linha 1");
    b.insert("\n");
    b.insert("colado\ncom quebra");
    assert_eq!(b.text(), "linha 1\ncolado\ncom quebra");
    assert_eq!(b.lines(), vec![0..7, 8..14, 15..25]);
    assert_eq!(b.line_and_column(b.cursor()), (2, 10));
    assert_eq!(b.line_and_column(8), (1, 0));
    assert_eq!(b.line_and_column(7), (0, 7));
    b.line_start();
    assert_eq!(b.cursor(), 15);
    b.left();
    b.line_start();
    assert_eq!(b.cursor(), 8);
    b.line_end();
    assert_eq!(b.cursor(), 14);
    assert_eq!(Buffer::new().lines(), vec![0..0]);
}

#[test]
fn selection_replaced_to_the_typing_and_clear() {
    let mut b = Buffer::new();
    typing(&mut b, "texto velho");
    b.select_all();
    assert_eq!(b.selection(), 0..11);
    typing(&mut b, "novo");
    assert_eq!(b.text(), "novo");
    b.select_all();
    b.erase_before();
    assert!(b.empty());
    typing(&mut b, "x");
    b.clear();
    assert_eq!(b, Buffer::new());
    b.position(99);
    assert_eq!(b.cursor(), 0);
    typing(&mut b, "ção");
    b.position(1); // meio do "ç"
    assert_eq!(b.cursor(), 0);
}

#[test]
fn so_digits_discards_the_that_not_and_digit() {
    assert_eq!(so_digits("123"), "123");
    assert_eq!(so_digits("a1b2 c3-4,5.6"), "123456");
    assert_eq!(so_digits("abc"), "");
    assert_eq!(so_digits("٣4"), "4", "algarismos de outras escritas nao valem");
    assert_eq!(so_digits(""), "");
}

#[test]
fn digits_of_the_total() {
    assert_eq!(digits(0), 1);
    assert_eq!(digits(9), 1);
    assert_eq!(digits(10), 2);
    assert_eq!(digits(99), 2);
    assert_eq!(digits(100), 3);
    assert_eq!(digits(500), 3);
    assert_eq!(digits(12345), 5);
}

#[test]
fn selection_by_mouse_word_line_and_extend() {
    let mut b = Buffer::new();
    typing(&mut b, "ola mundo\nsegunda 12");
    b.select_word(5);
    assert_eq!(b.selected(), "mundo");
    b.select_word(3);
    assert_eq!(b.selected(), "ola", "na borda direita da palavra");
    b.select_word(b.text().len());
    assert_eq!(b.selected(), "12", "no fim do texto");
    b.select_word(9);
    assert_eq!(b.selected(), "mundo");
    b.select_line(14);
    assert_eq!(b.selected(), "segunda 12");
    b.select_line(2);
    assert_eq!(b.selected(), "ola mundo");
    b.position(4);
    b.extend_ate(8);
    assert_eq!(b.selected(), "mund");
    b.extend_ate(1);
    assert_eq!(b.selected(), "la ", "estende para tras da ancora");
    let mut e = Buffer::new();
    typing(&mut e, "a  b");
    e.select_word(2);
    assert!(e.selected().is_empty(), "entre espacos: so o cursor");
    let mut c = Buffer::new();
    typing(&mut c, "açúcar doce");
    c.select_word(3);
    assert_eq!(c.selected(), "açúcar");
}
