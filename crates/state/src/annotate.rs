//! Grifos desenhados no visualizador, sem gpui (regra R4): qual gesto cada
//! combinacao de mouse + modificador faz, o retangulo "elastico" de um arrasto
//! convertido para coordenadas relativas da imagem (normalizado e limitado a
//! `0..=1` ANTES de ir para `HighlightsFile::add`), o ponteiro de um clique, e a
//! forma de um grifo salvo na tela (para desenhar por cima da imagem).
//!
//! Mapeamento (decisao R11 + Task 9): arrastar sem modificador move a imagem e
//! um clique simples (sem arrastar) seleciona o grifo sob o ponteiro (no vazio,
//! tira a selecao); Shift+arrastar desenha um retangulo; Option(Alt)+arrastar
//! desenha uma seta da CAUDA (onde apertou) ate a PONTA (onde soltou);
//! Option+clique sem arrastar poe uma seta curta padrao ([`SHORT_ARROW`]) com a
//! ponta no clique. O limiar entre clique e arrasto e [`MIN_DRAG`].

use marginalia_core::highlights::{Highlight, DEFAULT_COLOR};
use marginalia_core::model::Framing;

use crate::viewer_math::{self, Size};

/// Movimento minimo (pixels de tela, em cada eixo) para um botao apertado
/// virar arrasto (retangulo, seta ou mover): menos que isso e um clique.
pub const MIN_DRAG: f32 = 4.0;

/// Ferramenta de grifo ligada pelos botoes da barra de ferramentas: com ela,
/// arrastar sem modificador desenha (Shift e Option continuam valendo e
/// prevalecem). Fica ligada ate o botao ser clicado de novo ou Esc.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    No,
    Textbox,
    Arrow,
}

impl Tool {
    /// Clique no botao de `target`: liga, troca de ferramenta, ou desliga se ja era a ligada.
    pub fn toggle(self, target: Tool) -> Tool {
        if self == target {
            Tool::No
        } else {
            target
        }
    }
}

/// O que um botao esquerdo apertado comeca a fazer (pelos modificadores).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    /// Sem modificador: arrastar move a imagem; clicar seleciona.
    Relocate,
    /// Shift: arrastar desenha um retangulo.
    Rectangle,
    /// Option/Alt: arrastar desenha uma seta (cauda -> ponta); clicar, a seta curta.
    Arrow,
    /// Ferramenta caixa ligada, sem modificador: arrastar desenha um retangulo; clicar seleciona.
    LinkedRectangle,
    /// Ferramenta seta ligada, sem modificador: arrastar desenha uma seta; clicar seleciona.
    LinkedArrow,
}

/// Gesto pelos modificadores do clique e pela ferramenta ligada. Shift vence
/// Alt (Shift+Alt desenha) e os dois vencem a ferramenta.
pub fn gesture(shift: bool, alt: bool, tool: Tool) -> Gesture {
    if shift {
        Gesture::Rectangle
    } else if alt {
        Gesture::Arrow
    } else {
        match tool {
            Tool::No => Gesture::Relocate,
            Tool::Textbox => Gesture::LinkedRectangle,
            Tool::Arrow => Gesture::LinkedArrow,
        }
    }
}

/// O que o gesto fez, decidido ao soltar o botao.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAction {
    /// Shift+arrasto: grifo retangular.
    Rectangle,
    /// Option+arrasto: seta da cauda (apertou) ate a ponta (soltou).
    Arrow,
    /// Option+clique: seta curta padrao com a ponta no clique.
    ShortArrow,
    /// Clique simples: seleciona o grifo sob o ponteiro (ou tira a selecao).
    Click,
    /// Arrasto simples: moveu a imagem.
    Pan,
    /// Shift+clique: nada (clique descuidado).
    Nothing,
}

/// Classifica o gesto `g` de `start` a `end` (pixels de tela) pelo limiar
/// [`MIN_DRAG`] (ver [`is_click`]).
pub fn sort(g: Gesture, start: (f32, f32), end: (f32, f32)) -> MenuAction {
    match (g, is_click(start, end)) {
        (Gesture::Rectangle, false) => MenuAction::Rectangle,
        (Gesture::Rectangle, true) => MenuAction::Nothing,
        (Gesture::Arrow, false) => MenuAction::Arrow,
        (Gesture::Arrow, true) => MenuAction::ShortArrow,
        (Gesture::Relocate | Gesture::LinkedRectangle | Gesture::LinkedArrow, true) => MenuAction::Click,
        (Gesture::LinkedRectangle, false) => MenuAction::Rectangle,
        (Gesture::LinkedArrow, false) => MenuAction::Arrow,
        (Gesture::Relocate, false) => MenuAction::Pan,
    }
}

/// Limita `v` a `0..=1`; NaN vira 0.
fn unit(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

/// Maior `w <= b - a` com `a + w <= 1.0` exato em f32 (`HighlightsFile` valida `x + w <= 1`).
fn width(a: f32, b: f32) -> f32 {
    let mut w = b - a;
    while w > 0.0 && a + w > 1.0 {
        w = f32::from_bits(w.to_bits() - 1);
    }
    w.max(0.0)
}

/// Retangulo relativo `(x, y, w, h)` entre dois pontos de imagem quaisquer:
/// normalizado (x, y no canto superior esquerdo, w, h >= 0) e limitado a
/// `0..=1`. `None` se a parte dentro da imagem tiver largura ou altura zero
/// (arrasto todo fora da imagem, ou numa linha).
pub fn relative_rectangle(a: (f32, f32), b: (f32, f32)) -> Option<(f32, f32, f32, f32)> {
    let (x0, x1) = (unit(a.0.min(b.0)), unit(a.0.max(b.0)));
    let (y0, y1) = (unit(a.1.min(b.1)), unit(a.1.max(b.1)));
    let (w, h) = (width(x0, x1), width(y0, y1));
    (w > 0.0 && h > 0.0).then_some((x0, y0, w, h))
}

/// O arrasto de `start` a `end` (pixels relativos a area de desenho) e so um
/// clique (menos de [`MIN_DRAG`] nos dois eixos).
pub fn is_click(start: (f32, f32), end: (f32, f32)) -> bool {
    let (dx, dy) = ((end.0 - start.0).abs(), (end.1 - start.1).abs());
    !(dx >= MIN_DRAG || dy >= MIN_DRAG) || !dx.is_finite() || !dy.is_finite()
}

/// Grifo retangular de um Shift+arrasto de `start` a `end` (pixels relativos
/// a area). `None` para um clique (arrasto menor que [`MIN_DRAG`]) ou
/// um retangulo todo fora da imagem.
pub fn rectangle(
    fr: &Framing,
    area: Size,
    img: Size,
    start: (f32, f32),
    end: (f32, f32),
    id: String,
) -> Option<Highlight> {
    if is_click(start, end) {
        return None;
    }
    let a = viewer_math::screen_to_image(fr, area, img, start);
    let b = viewer_math::screen_to_image(fr, area, img, end);
    let (x, y, w, h) = relative_rectangle(a, b)?;
    Some(Highlight::Rectangle { id, x, y, w, h, color: None })
}

/// Ponteiro (ponto, sem cauda) em `p` (pixels relativos a area): o ultimo
/// recurso da seta curta ([`gesture_arrow`]). `None` se `p` caiu fora da
/// imagem (no fundo).
pub fn pointer(fr: &Framing, area: Size, img: Size, p: (f32, f32), id: String) -> Option<Highlight> {
    let (x, y) = viewer_math::screen_to_image(fr, area, img, p);
    let inside = (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y);
    inside.then_some(Highlight::Pointer { id, x, y, tail: None, color: None })
}

/// Comprimento minimo (unidades relativas) de uma seta: so o que o core precisa
/// (cauda diferente da ponta, com folga para arredondamento). O que separa
/// clique de arrasto e o limiar de TELA ([`MIN_DRAG`]), aplicado por
/// [`gesture_arrow`]: com zoom 32 um arrasto de 6 px ainda e uma seta.
pub const ARROW_MINIMA: f32 = 1e-5;

/// Cauda convencional de um ponteiro ANTIGO (sem `tail`): vai da ponta ate
/// `ponta + CAUDA_PADRAO`, isto e, para cima e para a esquerda. So vale para o
/// teste de acerto: o ponteiro antigo e, na pratica, um ponto, e a cauda curta
/// so da um pouco de area para clicar.
pub const DEFAULT_TAIL: (f32, f32) = (-0.03, -0.03);

/// Seta de `tail` a `tip` (coordenadas relativas da imagem, ja convertidas):
/// ambos os pontos sao limitados a `0..=1`. `color`: `None` deixa a decisao para
/// `AppState::commit_highlight` (a cor atual, e a padrao fica sem `color` no arquivo).
/// `None` para seta degenerada (ponta a menos de [`ARROW_MINIMA`] da cauda,
/// depois de limitar) ou com coordenada NaN.
pub fn arrow(tail: (f32, f32), tip: (f32, f32), color: Option<&str>, id: String) -> Option<Highlight> {
    if [tail.0, tail.1, tip.0, tip.1].iter().any(|v| v.is_nan()) {
        return None;
    }
    let c = (unit(tail.0), unit(tail.1));
    let p = (unit(tip.0), unit(tip.1));
    let (dx, dy) = (p.0 - c.0, p.1 - c.1);
    if (dx * dx + dy * dy).sqrt() < ARROW_MINIMA {
        return None;
    }
    Some(Highlight::Pointer { id, x: p.0, y: p.1, tail: Some(c), color: color.map(str::to_string) })
}

/// Seta curta padrao do Option+clique: a cauda fica `SHORT_ARROW` pixels DE TELA
/// a partir da ponta (o clique), isto e, acima e a esquerda (o mesmo lado de
/// [`DEFAULT_TAIL`]) e a seta aponta para baixo e para a direita, com o mesmo
/// tamanho na tela em qualquer zoom. Se a cauda cair em cima da ponta depois de
/// limitar a `0..=1` (clique no canto superior esquerdo), usa o lado oposto;
/// so se ainda fosse degenerada ([`ARROW_MINIMA`]) viraria um ponto (ponteiro
/// sem cauda), nunca um grifo invalido. Em qualquer zoom (1..32) e uma seta.
pub const SHORT_ARROW: (f32, f32) = (-36.0, -36.0);

/// O ponto de tela `p` cai dentro da imagem?
fn image_inside(fr: &Framing, area: Size, img: Size, p: (f32, f32)) -> bool {
    let (x, y) = viewer_math::screen_to_image(fr, area, img, p);
    (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)
}

/// Grifo de um Option+gesto de `start` (apertou) a `end` (soltou), em pixels
/// relativos a area. Arrasto ([`MenuAction::Arrow`]): seta da cauda `start` ate a
/// ponta `end`, as duas limitadas a `0..=1` (soltar fora da imagem para na
/// borda); uma seta curta demais em unidades relativas vira um ponto na ponta.
/// Clique ([`MenuAction::ShortArrow`]): a seta curta padrao ([`SHORT_ARROW`]).
/// `None` se apertou fora da imagem (no fundo). A cor fica `None`
/// (`AppState::commit_highlight` decide pela cor atual).
pub fn gesture_arrow(
    fr: &Framing,
    area: Size,
    img: Size,
    start: (f32, f32),
    end: (f32, f32),
    id: String,
) -> Option<Highlight> {
    if !image_inside(fr, area, img, start) {
        return None;
    }
    let rel = |p: (f32, f32)| viewer_math::screen_to_image(fr, area, img, p);
    let image_point = |p: (f32, f32), id: String| {
        let (x, y) = rel(p);
        Some(Highlight::Pointer { id, x: unit(x), y: unit(y), tail: None, color: None })
    };
    match sort(Gesture::Arrow, start, end) {
        MenuAction::ShortArrow => {
            let tip = rel(start);
            let side = |s: f32| rel((start.0 + s * SHORT_ARROW.0, start.1 + s * SHORT_ARROW.1));
            arrow(side(1.0), tip, None, id.clone())
                .or_else(|| arrow(side(-1.0), tip, None, id.clone()))
                .or_else(|| pointer(fr, area, img, start, id))
        }
        _ => {
            // Depois de limitar a imagem a seta ainda tem pelo menos o limiar na
            // TELA? (arrasto na borda para fora achata): senao, um ponto na ponta.
            let limit = |p: (f32, f32)| {
                let (x, y) = rel(p);
                viewer_math::image_to_screen(fr, area, img, (unit(x), unit(y)))
            };
            let (a, b) = (limit(start), limit(end));
            if ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt() < MIN_DRAG {
                return image_point(end, id);
            }
            arrow(rel(start), rel(end), None, id.clone()).or_else(|| image_point(end, id))
        }
    }
}

/// Seta provisoria (Option+arrasto em andamento) na tela: as pontas limitadas
/// a imagem como a seta gravada ficara. `None` enquanto ainda e clique ou se
/// apertou fora da imagem.
pub fn arrow_preview(fr: &Framing, area: Size, img: Size, start: (f32, f32), current: (f32, f32)) -> Option<Shape> {
    if sort(Gesture::Arrow, start, current) != MenuAction::Arrow || !image_inside(fr, area, img, start) {
        return None;
    }
    let screen = |p: (f32, f32)| {
        let (x, y) = viewer_math::screen_to_image(fr, area, img, p);
        viewer_math::image_to_screen(fr, area, img, (unit(x), unit(y)))
    };
    Some(Shape::Arrow { tail: screen(start), tip: screen(current) })
}

/// Tolerancia de acerto de `tol_px` pixels de tela em unidades relativas da
/// imagem, por eixo (`tol_px / largura da imagem na tela`, `tol_px / altura`),
/// para [`fix`]. `None` com area ou imagem degenerada.
pub fn tolerance(fr: &Framing, area: Size, img: Size, tol_px: f32) -> Option<(f32, f32)> {
    let (w, h) = viewer_math::pixels_by_unit(fr, area, img)?;
    Some((tol_px / w, tol_px / h))
}

/// Raio de acerto de um clique, em pixels de tela.
pub const TOLERANCE_PX: f32 = 8.0;

/// O grifo sob o clique em `p` (pixels relativos a area), com
/// [`TOLERANCE_PX`] convertida por eixo ([`tolerance`]) e a prioridade de
/// [`fix`] (setas e pontos, depois retangulos menores). `None`: vazio.
pub fn screen_highlight(highlights: &[Highlight], fr: &Framing, area: Size, img: Size, p: (f32, f32)) -> Option<String> {
    let tol = tolerance(fr, area, img, TOLERANCE_PX)?;
    fix(highlights, viewer_math::screen_to_image(fr, area, img, p), tol)
}

/// Distancia de `p` ao segmento `a`-`b`.
fn distance_to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (vx, vy) = (b.0 - a.0, b.1 - a.1);
    let (wx, wy) = (p.0 - a.0, p.1 - a.1);
    let len2 = vx * vx + vy * vy;
    let t = if len2 > 0.0 { ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let (qx, qy) = (a.0 + t * vx, a.1 + t * vy);
    ((p.0 - qx).powi(2) + (p.1 - qy).powi(2)).sqrt()
}

/// Distancia de `p` ao retangulo cheio (0 dentro dele).
fn distance_to_rectangle(p: (f32, f32), x: f32, y: f32, w: f32, h: f32) -> f32 {
    let dx = (x - p.0).max(0.0).max(p.0 - (x + w));
    let dy = (y - p.1).max(0.0).max(p.1 - (y + h));
    (dx * dx + dy * dy).sqrt()
}

/// O grifo sob o ponto `p` (coordenadas relativas da imagem), se houver.
/// Retangulo: acerta dentro da area ou a ate `tolerance` da borda. Seta:
/// distancia ao segmento cauda -> ponta ate `tolerance`; ponteiro antigo sem
/// cauda usa [`DEFAULT_TAIL`].
///
/// `tolerance` e `(x, y)` em unidades relativas da imagem, um valor por eixo:
/// quem chama converte pixels usando a largura e a altura da imagem NA TELA,
/// separadas (a regiao de acerto e uma elipse de semieixos `tolerance`).
/// Tolerancia zero vale como um valor minimo; NaN em `p` ou na tolerancia: `None`.
///
/// Prioridade quando varios acertam: setas e pontos antes dos retangulos (os
/// pequenos nao ficam escondidos); entre retangulos, o de menor area; em
/// empate (areas dentro de `AREA_TIE`), o ultimo da lista (o desenhado por cima).
pub fn fix(highlights: &[Highlight], p: (f32, f32), tolerance: (f32, f32)) -> Option<String> {
    if [p.0, p.1, tolerance.0, tolerance.1].iter().any(|v| v.is_nan()) {
        return None; // `f32::max` engole NaN: sem isto um clique NaN acertaria tudo
    }
    let tol = (tolerance.0.max(TOLERANCE_MINIMA), tolerance.1.max(TOLERANCE_MINIMA));
    // (rank, area) menor vence; areas quase iguais empatam e o ultimo da lista ganha.
    let mut best: Option<((u8, f32), &Highlight)> = None;
    for g in highlights {
        let Some(key) = fixes(g, p, tol) else { continue };
        if best.is_none_or(|(m, _)| wins(key, m)) {
            best = Some((key, g));
        }
    }
    best.map(|(_, g)| g.id().to_string())
}

/// Diferenca relativa de area abaixo da qual dois retangulos contam como do mesmo
/// tamanho: o ruido de arredondamento de `f32` (ex.: o mesmo retangulo gravado por
/// rotacoes diferentes) nao pode decidir qual fica "por baixo".
const AREA_TIE: f32 = 1e-3;

/// `fresh` ganha de `current` (que ja era a melhor)? Rank menor vence; no mesmo rank, area
/// menor vence, e area praticamente igual e empate, que a `fresh` (a ultima da lista) ganha.
fn wins(fresh: (u8, f32), current: (u8, f32)) -> bool {
    if fresh.0 != current.0 {
        return fresh.0 < current.0;
    }
    let (a, b) = (fresh.1, current.1);
    (a - b).abs() <= AREA_TIE * a.max(b) || a < b
}

/// Menor tolerancia por eixo (evita dividir por zero).
const TOLERANCE_MINIMA: f32 = 1e-6;

/// `Some((rank, area))` se `g` esta sob `p`: rank 0 setas/pontos, 1 retangulos.
fn fixes(g: &Highlight, p: (f32, f32), tol: (f32, f32)) -> Option<(u8, f32)> {
    // Escala cada eixo pela tolerancia: o acerto vira "distancia <= 1".
    let es = |q: (f32, f32)| (q.0 / tol.0, q.1 / tol.1);
    let (key, d) = match *g {
        Highlight::Rectangle { x, y, w, h, .. } => {
            let (a, b) = (es((x, y)), es((w, h)));
            ((1, w * h), distance_to_rectangle(es(p), a.0, a.1, b.0, b.1))
        }
        Highlight::Pointer { x, y, tail, .. } => {
            let tail = tail.unwrap_or((x + DEFAULT_TAIL.0, y + DEFAULT_TAIL.1));
            ((0, 0.0), distance_to_segment(es(p), es(tail), es((x, y))))
        }
    };
    (d <= 1.0).then_some(key)
}

/// Um grifo na tela, em pixels relativos a area de desenho.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Rectangle { x: f32, y: f32, w: f32, h: f32 },
    /// Seta: haste da cauda ate a ponta, cabeca na ponta.
    Arrow { tail: (f32, f32), tip: (f32, f32) },
    /// Ponteiro antigo (sem cauda): um ponto. Nenhuma cauda e inventada.
    ImagePoint { x: f32, y: f32 },
}

/// Onde desenhar `g` com o enquadramento atual.
pub fn in_screen(fr: &Framing, area: Size, img: Size, g: &Highlight) -> Shape {
    let screen = |p| viewer_math::image_to_screen(fr, area, img, p);
    match *g {
        Highlight::Rectangle { x, y, w, h, .. } => {
            let (a, b) = (screen((x, y)), screen((x + w, y + h)));
            Shape::Rectangle { x: a.0, y: a.1, w: b.0 - a.0, h: b.1 - a.1 }
        }
        Highlight::Pointer { x, y, tail: Some(c), .. } => Shape::Arrow { tail: screen(c), tip: screen((x, y)) },
        Highlight::Pointer { x, y, tail: None, .. } => {
            let (x, y) = screen((x, y));
            Shape::ImagePoint { x, y }
        }
    }
}

/// Onde vao as alcas do grifo selecionado: os 4 cantos do retangulo (sup.
/// esq., sup. dir., inf. esq., inf. dir.), a cauda e a ponta da seta, o centro
/// do ponto.
pub fn handles(f: Shape) -> Vec<(f32, f32)> {
    match f {
        Shape::Rectangle { x, y, w, h } => vec![(x, y), (x + w, y), (x, y + h), (x + w, y + h)],
        Shape::Arrow { tail, tip } => vec![tail, tip],
        Shape::ImagePoint { x, y } => vec![(x, y)],
    }
}

/// A forma (mais `margin` px para halos, aneis e alcas) fica toda fora da area
/// `(0, 0)..area`: nao vale montar caminhos para ela. NaN conta como fora.
pub fn outside_area(f: Shape, area: Size, margin: f32) -> bool {
    let (x0, y0, x1, y1) = match f {
        Shape::Rectangle { x, y, w, h } => (x, y, x + w, y + h),
        Shape::Arrow { tail, tip } => (tail.0.min(tip.0), tail.1.min(tip.1), tail.0.max(tip.0), tail.1.max(tip.1)),
        Shape::ImagePoint { x, y } => (x, y, x, y),
    };
    if [x0, y0, x1, y1].iter().any(|v| v.is_nan()) {
        return true;
    }
    x1 + margin < 0.0 || y1 + margin < 0.0 || x0 - margin > area.0 || y0 - margin > area.1
}

/// Geometria de desenho de uma seta na tela.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArrowGeometry {
    /// Onde a haste (que comeca na cauda) termina: a base da cabeca, para o
    /// traco grosso nao passar da ponta.
    pub end_shaft: (f32, f32),
    /// Triangulo da cabeca: a ponta e os dois cantos da base.
    pub head: [(f32, f32); 3],
}

/// Cabeca triangular de `size` pixels (comprimento; a base tem a mesma
/// largura) na ponta da seta `tail` -> `tip`. Numa seta curta a cabeca
/// encolhe para no maximo 60% do comprimento (continua parecendo uma seta).
/// `None` para seta degenerada (comprimento < 0.5 px), tamanho nao positivo
/// ou NaN: quem desenha cai no ponto.
pub fn arrow_geometry(tail: (f32, f32), tip: (f32, f32), size: f32) -> Option<ArrowGeometry> {
    let (dx, dy) = (tip.0 - tail.0, tip.1 - tail.1);
    let len = (dx * dx + dy * dy).sqrt();
    if !len.is_finite() || len < 0.5 || !size.is_finite() || size <= 0.0 {
        return None;
    }
    let (ux, uy) = (dx / len, dy / len);
    let t = size.min(len * 0.6);
    let base = (tip.0 - ux * t, tip.1 - uy * t);
    // a esquerda de quem anda da cauda para a ponta (tela, y para baixo): (uy, -ux); meia largura t / 2
    let (nx, ny) = (uy * t / 2.0, -ux * t / 2.0);
    Some(ArrowGeometry { end_shaft: base, head: [tip, (base.0 + nx, base.1 + ny), (base.0 - nx, base.1 - ny)] })
}

/// Cor `#rrggbb` do grifo (de [`Highlight::color`]) em `0xrrggbb`. Texto estranho
/// (JSON editado a mao) vira a cor padrao, nunca um panico. A cor desenhada
/// vem SO daqui: nunca do estado (pendente, destaque, selecao) nem de um hash.
pub fn color_rgb(color: &str) -> u32 {
    let read_file = |c: &str| {
        let h = c.strip_prefix('#')?;
        (h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit())).then(|| u32::from_str_radix(h, 16).ok()).flatten()
    };
    read_file(color).or_else(|| read_file(DEFAULT_COLOR)).unwrap_or(0xffc400)
}

/// Retangulo elastico (enquanto arrasta) na tela: normalizado, sem limitar.
pub fn elastic(start: (f32, f32), end: (f32, f32)) -> Shape {
    Shape::Rectangle {
        x: start.0.min(end.0),
        y: start.1.min(end.1),
        w: (end.0 - start.0).abs(),
        h: (end.1 - start.1).abs(),
    }
}

#[cfg(test)]
#[path = "tests/annotate.rs"]
mod tests;
