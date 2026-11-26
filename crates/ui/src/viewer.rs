//! View gpui do visualizador: desenha uma imagem com o `Framing` atual e
//! traduz roda/arrasto/teclas de zoom para as funcoes puras de `viewer_math`.
//! Grifos (decisao R11 + Task 9, logica em `annotate`): arrastar move; clicar
//! (sem arrastar mais que `annotate::MIN_DRAG`) seleciona o grifo sob o
//! ponteiro, sem mover a imagem ([`ViewerEvent::Select`]); Shift+arrastar
//! desenha um retangulo (com o elastico na tela); Option+arrastar desenha uma
//! seta da cauda (apertou) ate a ponta (soltou), com a previa na tela;
//! Option+clique poe a seta curta padrao. O grifo pronto sai como
//! [`ViewerEvent::Highlight`] para a view raiz gravar; os grifos salvos da imagem
//! sao desenhados por cima dela, cada um na SUA cor (`Highlight::color`): pendente,
//! destaque (nota aberta) e selecao mudam so o contorno, nunca a cor.
//! A imagem chega pronta (BGRA, ja reduzida para caber na textura) de
//! `load::execute`, que roda fora da thread da interface, e o visualizador
//! fica com os pixels (o estado guarda so o tamanho: nada de copia aqui).
//!
//! Camadas (`viewer_math::Layers`): sem ajuste, a textura cheia sozinha; com
//! ajuste, a base esticada sobre a pagina inteira e o tile por cima, na
//! sub-caixa da sua janela (`viewer_math::window_textbox`), as duas com a
//! mesma mascara e arredondadas para fora no pixel fisico
//! (`viewer_math::textbox_to_outside`). Grifos e setas usam sempre o tamanho da
//! pagina (`image_sz`), nunca o da textura de uma camada. A cada quadro o
//! visualizador calcula a janela visivel (`viewer_math::visible_window`) e, se
//! mudou (pan, zoom, roda, redimensionar, imagem nova), avisa a view raiz com
//! [`ViewerEvent::ViewWindow`].

use std::cell::{Cell, RefCell};

use texts::t;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    canvas, div, fill, point, prelude::*, px, quad, rgb, rgba, size, App, BorderStyle, Bounds, ContentMask, Context,
    Corners, EventEmitter, FocusHandle, Hsla, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    PathBuilder, Pixels, Point, RenderImage, ScrollWheelEvent, Window,
};
use image::Frame;
use marginalia_core::highlights::{new_highlight_id, Highlight};
use marginalia_core::model::{Framing, ImageRef};
use marginalia_core::rotation::Rotation;

use crate::annotate::{self, Tool, Shape, Gesture};
use crate::load::{Layer, Reframe, Display, ViewWindow};
use crate::menus;
use crate::viewer_math::{self, Layers, Size};

/// Fator de zoom por pixel de rolagem: `factor = exp(delta_y * ZOOM_POR_PIXEL)`,
/// com `delta_y` como o gpui entrega (`ScrollDelta`, linhas convertidas a
/// `PIXELS_BY_LINE`), limitado por evento (`viewer_math::runs_factor`).
const ZOOM_BY_PIXEL: f32 = 0.002;
/// Fator de cada toque em `+` / `-`.
const ZOOM_KEY: f32 = 1.25;
/// Linhas -> pixels para rodas de mouse que reportam linhas.
const PIXELS_BY_LINE: f32 = 20.0;

/// O que o visualizador pede a view raiz.
pub enum ViewerEvent {
    /// Um grifo desenhado (coordenadas ja limitadas a 0..=1) sobre `image`,
    /// a imagem que estava na tela quando o gesto comecou.
    Highlight { image: ImageRef, highlight: Highlight },
    /// Clique simples sobre `image`: selecionar o grifo `id` (`None`: o clique
    /// caiu no vazio, tirar a selecao).
    Select { image: ImageRef, id: Option<String> },
    /// A janela visivel de `image` passou a ser `view_window` (pan, zoom, roda,
    /// redimensionar, imagem nova). Nunca vazia; repetida so depois de uma
    /// imagem nova chegar (o estado e a agenda deduplicam o resto).
    ViewWindow { image: ImageRef, view_window: ViewWindow },
}

/// Uma textura na tela e o tamanho dela em pixels.
#[derive(Clone)]
struct Texture {
    img: Arc<RenderImage>,
    sz: (u32, u32),
}

/// Arrasto em andamento com o botao esquerdo.
enum Drag {
    /// Sem modificador, ainda abaixo do limiar: e um clique (seleciona ao
    /// soltar; a imagem nao se mexe) ate passar dele, quando vira `Relocate`.
    /// `image`: a pronta na tela ao apertar (sem ela o clique nao seleciona).
    Click { start: Point<Pixels>, image: Option<ImageRef> },
    /// Move a imagem; ultima posicao do ponteiro.
    Relocate(Point<Pixels>),
    /// Shift: desenha um retangulo de `start` ate `current` (coordenadas da janela).
    /// `tool_by`: veio da ferramenta ligada (sem modificador), entao um clique seleciona.
    Rectangle { start: Point<Pixels>, current: Point<Pixels>, image: ImageRef, tool_by: bool },
    /// Option: seta da cauda `start` ate a ponta `current` (coordenadas da janela).
    Arrow { start: Point<Pixels>, current: Point<Pixels>, image: ImageRef, tool_by: bool },
}

/// Grifos salvos da imagem atual e quais destacar.
#[derive(Clone, Default, PartialEq)]
struct Marks {
    highlights: Vec<Highlight>,
    /// Grifo da nota aberta.
    emphasis: Option<String>,
    /// Grifo que vai na proxima nota.
    pending: Option<String>,
    /// Grifo selecionado (Delete apaga, a paleta recolore).
    selected: Option<String>,
}

pub struct Viewer {
    focus_handle: FocusHandle,
    /// As texturas na tela (cheia, ou base e tile), espelhando o estado.
    layers: Layers<Texture>,
    /// Ultima janela visivel avisada (`None`: nenhuma desde a ultima imagem).
    given_window: Option<ViewWindow>,
    /// Tamanho ORIGINAL da pagina, sem girar (a textura pode ter sido reduzida).
    image_sz: Size,
    /// Rotacao de exibicao: a pagina aparece girada assim. Tudo daqui para baixo (zoom,
    /// pan, `fr`, caixas, grifos na tela) vive no espaco de EXIBICAO, a pagina ja
    /// girada, de `display_sz()`; so a API publica, os eventos e o que vai para o vault
    /// ficam nas coordenadas da imagem (`Rotation::display_to` / `from_display`).
    rot: Rotation,
    error: Option<String>,
    loading: bool,
    /// Enquadramento no espaco de exibicao (o centro gira junto; o zoom nao).
    fr: Framing,
    /// Limites (coordenadas da janela) da area de desenho no ultimo quadro.
    area: Rc<Cell<Bounds<Pixels>>>,
    drag: Option<Drag>,
    /// Ferramenta de grifo ligada pela barra de ferramentas (nao persistida).
    tool: Tool,
    marks: Marks,
    /// Os grifos de `marks` levados para o espaco de exibicao (desenho e clique).
    display_highlights: Vec<Highlight>,
    /// Qual imagem (fonte/arquivo/pagina) esta na tela.
    image_ref: Option<ImageRef>,
    /// Vira `true` quando um quadro desenhou a imagem (ou o erro).
    painted: Rc<Cell<bool>>,
    /// Erro devolvido por `paint_image` (ex.: textura recusada pelo atlas).
    paint_failure: Rc<RefCell<Option<String>>>,
}

impl Viewer {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            layers: Layers::default(),
            given_window: None,
            image_sz: (0.0, 0.0),
            rot: Rotation::R0,
            error: None,
            loading: false,
            fr: viewer_math::default_framing(),
            area: Rc::new(Cell::new(Bounds::default())),
            drag: None,
            tool: Tool::No,
            marks: Marks::default(),
            display_highlights: Vec::new(),
            image_ref: None,
            painted: Rc::new(Cell::new(false)),
            paint_failure: Rc::new(RefCell::new(None)),
        }
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    /// Sinal compartilhado que vira `true` depois do primeiro quadro desenhado.
    pub fn painted(&self) -> Rc<Cell<bool>> {
        self.painted.clone()
    }

    /// Rotacao de exibicao da imagem na tela.
    pub fn rotation(&self) -> Rotation {
        self.rot
    }

    /// Tamanho da pagina no espaco de exibicao (os lados trocam em 90 e 270 graus).
    fn display_sz(&self) -> Size {
        viewer_math::display_sz(self.image_sz, self.rot)
    }

    /// Enquadramento da imagem (`e`, nas coordenadas dela) -> o do espaco de exibicao.
    fn framing_to_display(&self, e: Framing) -> Framing {
        let e = viewer_math::clamp(e);
        let (cx, cy) = self.rot.display_to((e.cx, e.cy));
        Framing { zoom: e.zoom, cx, cy }
    }

    /// O marcador de erro esta na tela (erro do quadro ou da pintura).
    pub fn error_shows(&self) -> bool {
        self.error.is_some() || self.paint_failure.borrow().is_some()
    }

    /// Area de desenho medida no ultimo quadro (coordenadas da janela):
    /// `(x, y, largura, altura)`. Muda quando uma gaveta abre ou fecha.
    pub fn area(&self) -> (f32, f32, f32, f32) {
        let b = self.area.get();
        (f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height))
    }

    pub fn image_has(&self) -> bool {
        self.layers.has()
    }

    /// Tamanho das texturas na tela, por camada: `(cheia, base, tile e a janela
    /// dele)` (para o smoke).
    #[allow(clippy::type_complexity)]
    pub fn screen_layers(&self) -> (Option<(u32, u32)>, Option<(u32, u32)>, Option<((u32, u32), ViewWindow)>) {
        let c = &self.layers;
        (c.full.as_ref().map(|t| t.sz), c.base.as_ref().map(|t| t.sz), c.tile.as_ref().map(|(t, j)| (t.sz, *j)))
    }

    /// A ultima janela visivel avisada a view raiz (para o smoke).
    pub fn given_window(&self) -> Option<ViewWindow> {
        self.given_window
    }

    /// A imagem das camadas na tela (para o smoke: nada de outra imagem sobra).
    pub fn screen_image(&self) -> Option<&ImageRef> {
        self.image_ref.as_ref()
    }

    /// Os pixels (BGRA) viram textura sem copia: `Received` traz a unica
    /// referencia a eles (o estado guarda so o tamanho).
    fn texture(ex: Display) -> Texture {
        let sz = ex.bgra.dimensions();
        Texture { img: Arc::new(RenderImage::new([Frame::new(Arc::unwrap_or_clone(ex.bgra))])), sz }
    }

    /// Poe a imagem na `layer` (ver `viewer_math::Layers::receive`;
    /// `keep_other`: o estado ainda tem a outra camada de ajuste). Imagem
    /// nova (outra pagina): todas as texturas anteriores saem. As texturas
    /// substituidas saem do atlas.
    #[allow(clippy::too_many_arguments)]
    pub fn set_image(
        &mut self,
        ex: Display,
        frame: Reframe,
        image: ImageRef,
        layer: Layer,
        keep_other: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.image_ref.as_ref() != Some(&image) {
            self.discard_texture(window, cx);
        }
        self.image_ref = Some(image);
        self.image_sz = (ex.sz_original.0 as f32, ex.sz_original.1 as f32);
        // A textura manda na orientacao: pixels de outra rotacao nunca ficam na tela.
        self.adopt_rotation(ex.rot, window, cx);
        let t = Self::texture(ex);
        for old in self.layers.receive(layer, t, keep_other) {
            cx.drop_image(old.img, Some(window));
        }
        self.error = None;
        self.loading = false;
        *self.paint_failure.borrow_mut() = None;
        match frame {
            Reframe::Keep => {}
            Reframe::Adjust => self.fr = viewer_math::default_framing(),
            Reframe::Adopt(e) => self.fr = self.framing_to_display(e),
        }
        // Imagem (ou camada) nova: a janela vigente e avisada de novo no proximo
        // quadro (o estado pode te-la esquecido; ele ignora a repetida).
        self.given_window = None;
        cx.notify();
    }

    /// So a base de fundo mudou (mesmos valores do tile na tela): o tile e o
    /// enquadramento ficam.
    pub fn set_background(&mut self, ex: Display, window: &mut Window, cx: &mut Context<Self>) {
        // Uma base na orientacao velha (girou no meio do caminho) nunca entra.
        if self.image_ref.is_none() || ex.rot != self.rot {
            return;
        }
        let t = Self::texture(ex);
        for old in self.layers.receive(Layer::Base, t, true) {
            cx.drop_image(old.img, Some(window));
        }
        self.loading = false;
        cx.notify();
    }

    /// A janela visivel que o proximo quadro avisaria (`None`: nada a avisar,
    /// ja avisada, sem imagem, com erro ou com a area sem tamanho). O smoke
    /// espera ela sair antes de dar a tela por pronta.
    pub fn window_by_inform(&self, window: &Window) -> Option<ViewWindow> {
        if !self.layers.has() {
            return None;
        }
        let j = self.current_window(window)?;
        (self.given_window != Some(j)).then_some(j)
    }

    /// A janela visivel agora, na grade da pagina SEM girar (onde o tile e recortado),
    /// com ou sem textura na tela (`None`: sem imagem, com erro ou area sem tamanho).
    fn current_window(&self, window: &Window) -> Option<ViewWindow> {
        if self.image_ref.is_none() || self.error_shows() {
            return None;
        }
        let (area, _) = self.area_and_origin();
        viewer_math::window_visible_rotated(&self.fr, area, self.image_sz, self.rot, window.scale_factor())
    }

    /// Passa a exibir a pagina girada em `rot` (`R0`..`R270`). A imagem fica no mesmo
    /// ponto (o centro acompanha a rotacao, o zoom nao muda) e os grifos giram junto;
    /// as texturas na tela eram da orientacao velha e saem: o estado pede as novas.
    /// Devolve a janela visivel NOVA (ja marcada como avisada: quem chama a entrega ao
    /// estado e a agenda, sem esperar o proximo quadro). `None`: nada mudou, ou sem
    /// janela a avisar.
    pub fn set_rotation(&mut self, rot: Rotation, window: &mut Window, cx: &mut Context<Self>) -> Option<ViewWindow> {
        if rot == self.rot {
            return None;
        }
        self.adopt_rotation(rot, window, cx);
        let j = self.current_window(window);
        self.given_window = j;
        cx.notify();
        j
    }

    /// `rot` passa a ser a rotacao da tela. Mudou: as texturas (orientacao velha) saem, o
    /// centro e os grifos acompanham, e um gesto em andamento (feito na orientacao
    /// velha) e abandonado.
    fn adopt_rotation(&mut self, rot: Rotation, window: &mut Window, cx: &mut Context<Self>) {
        if rot == self.rot {
            return;
        }
        let center = self.rot.from_display((self.fr.cx, self.fr.cy));
        self.discard_texture(window, cx);
        self.rot = rot;
        let (ncx, ncy) = rot.display_to(center);
        self.fr = viewer_math::clamp(Framing { zoom: self.fr.zoom, cx: ncx, cy: ncy });
        self.drag = None;
        self.recompute_highlights_display();
    }

    fn recompute_highlights_display(&mut self) {
        self.display_highlights = self.marks.highlights.iter().map(|g| g.display_to(self.rot)).collect();
    }

    /// Avisa a view raiz se a janela visivel mudou (chamado a cada quadro, com
    /// a area medida no anterior; um redimensionar pede mais um quadro).
    fn inform_window(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(j) = self.window_by_inform(window) else { return };
        let Some(image) = self.image_ref.clone() else { return };
        self.given_window = Some(j);
        cx.emit(ViewerEvent::ViewWindow { image, view_window: j });
    }

    /// Mostra o marcador de erro no lugar da imagem.
    pub fn set_error(&mut self, msg: impl Into<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.discard_texture(window, cx);
        self.image_ref = None;
        self.error = Some(msg.into());
        self.loading = false;
        cx.notify();
    }

    /// Nada na tela (a fonte foi fechada ao trocar de vault).
    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.discard_texture(window, cx);
        self.image_ref = None;
        self.error = None;
        self.loading = false;
        self.marks = Marks::default();
        self.display_highlights.clear();
        *self.paint_failure.borrow_mut() = None;
        cx.notify();
    }

    pub fn loading(&self) -> bool {
        self.loading
    }

    /// Marca "carregando": a imagem anterior fica na tela ate a nova chegar.
    /// Comecar a carregar tira o marcador de erro do quadro anterior.
    pub fn set_loading(&mut self, yes: bool, cx: &mut Context<Self>) {
        if yes && self.error.is_some() {
            self.error = None;
            cx.notify();
        }
        if self.loading != yes {
            self.loading = yes;
            cx.notify();
        }
    }

    fn discard_texture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for old in self.layers.clear() {
            cx.drop_image(old.img, Some(window));
        }
        self.given_window = None;
    }

    /// O enquadramento nas coordenadas da IMAGEM (o que as notas guardam): o centro
    /// volta da exibicao, o zoom e o mesmo.
    pub fn framing(&self) -> Framing {
        let (cx, cy) = self.rot.from_display((self.fr.cx, self.fr.cy));
        Framing { zoom: self.fr.zoom, cx, cy }
    }

    /// Troca o enquadramento (nas coordenadas da imagem; o smoke usa para simular zoom/arrasto).
    pub fn set_framing(&mut self, e: Framing, cx: &mut Context<Self>) {
        self.fr = self.framing_to_display(e);
        cx.notify();
    }

    /// Grifos salvos da imagem atual, o destacado (nota aberta), o pendente e o selecionado.
    pub fn set_highlights(
        &mut self,
        highlights: &[Highlight],
        emphasis: Option<&str>,
        pending: Option<&str>,
        selected: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let new = Marks {
            highlights: highlights.to_vec(),
            emphasis: emphasis.map(str::to_string),
            pending: pending.map(str::to_string),
            selected: selected.map(str::to_string),
        };
        if new != self.marks {
            self.marks = new;
            self.recompute_highlights_display();
            cx.notify();
        }
    }

    /// Id do grifo destacado na tela (para o smoke).
    pub fn emphasis(&self) -> Option<&str> {
        self.marks.emphasis.as_deref()
    }

    /// Id do grifo desenhado como selecionado (para o smoke).
    pub fn selected(&self) -> Option<&str> {
        self.marks.selected.as_deref()
    }

    /// `(id, cor 0xrrggbb)` de cada grifo como e desenhado (para o smoke: a cor
    /// vem so do grifo, igual em qualquer estado).
    pub fn screen_colors(&self) -> Vec<(String, u32)> {
        self.marks.highlights.iter().map(|g| (g.id().to_string(), annotate::color_rgb(g.color()))).collect()
    }

    /// Quantos grifos estao desenhados sobre a imagem (para o smoke).
    pub fn screen_highlights(&self) -> usize {
        if self.layers.has() { self.marks.highlights.len() } else { 0 }
    }

    /// Ponto da janela (pixels logicos) -> ponto relativo da imagem (0..=1).
    pub fn screen_to_image(&self, p: Point<f32>) -> (f32, f32) {
        let (area, origin) = self.area_and_origin();
        let in_display = viewer_math::screen_to_image(&self.fr, area, self.display_sz(), (p.x - origin.0, p.y - origin.1));
        self.rot.from_display(in_display)
    }

    /// Ponto relativo da imagem (0..=1) -> ponto da janela (pixels logicos).
    pub fn image_to_screen(&self, p: (f32, f32)) -> Point<f32> {
        let (area, origin) = self.area_and_origin();
        let (x, y) = viewer_math::image_to_screen(&self.fr, area, self.display_sz(), self.rot.display_to(p));
        point(x + origin.0, y + origin.1)
    }

    fn area_and_origin(&self) -> (Size, (f32, f32)) {
        let b = self.area.get();
        (
            (f32::from(b.size.width), f32::from(b.size.height)),
            (f32::from(b.origin.x), f32::from(b.origin.y)),
        )
    }

    /// Posicao do evento relativa a area de desenho.
    fn relative(&self, p: Point<Pixels>) -> (f32, f32) {
        let (_, origin) = self.area_and_origin();
        (f32::from(p.x) - origin.0, f32::from(p.y) - origin.1)
    }

    fn zoom(&mut self, factor: f32, anchor: Option<(f32, f32)>, cx: &mut Context<Self>) {
        let (area, _) = self.area_and_origin();
        let anchor = anchor.unwrap_or((area.0 / 2.0, area.1 / 2.0));
        self.fr = viewer_math::zoom_at(&self.fr, area, self.display_sz(), factor, anchor);
        cx.notify();
    }

    fn on_scroll(&mut self, ev: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let delta = ev.delta.pixel_delta(px(PIXELS_BY_LINE));
        let factor = viewer_math::runs_factor(f32::from(delta.y), ZOOM_BY_PIXEL);
        let anchor = self.relative(ev.position);
        self.zoom(factor, Some(anchor), cx);
    }

    fn new_id() -> String {
        new_highlight_id(chrono::Local::now().naive_local())
    }

    pub fn on_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window);
        // Grifos so sobre a imagem pronta na tela: nada durante a carga (a tela
        // ainda mostra a imagem anterior) nem sobre o marcador de erro.
        let ready = match &self.image_ref {
            Some(i) if self.layers.has() && !self.loading && !self.error_shows() => Some(i.clone()),
            _ => None,
        };
        let gesture = annotate::gesture(ev.modifiers.shift, ev.modifiers.alt, self.tool);
        let start = ev.position;
        self.drag = match (gesture, ready) {
            (Gesture::Rectangle, Some(image)) => Some(Drag::Rectangle { start, current: start, image, tool_by: false }),
            (Gesture::LinkedRectangle, Some(image)) => Some(Drag::Rectangle { start, current: start, image, tool_by: true }),
            (Gesture::Arrow, Some(image)) => Some(Drag::Arrow { start, current: start, image, tool_by: false }),
            (Gesture::LinkedArrow, Some(image)) => Some(Drag::Arrow { start, current: start, image, tool_by: true }),
            // Shift/Option/ferramenta durante a carga ou sobre o erro: nao desenha nem move.
            (Gesture::Rectangle | Gesture::Arrow | Gesture::LinkedRectangle | Gesture::LinkedArrow, None) => None,
            // Sem modificador: clique ate passar do limiar (a imagem nao pula).
            (Gesture::Relocate, image) => Some(Drag::Click { start, image }),
        };
        cx.notify();
    }

    pub fn on_move(&mut self, ev: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.drag.is_none() {
            return;
        }
        if ev.pressed_button != Some(MouseButton::Left) {
            self.drag = None;
            cx.notify();
            return;
        }
        let (area, _) = self.area_and_origin();
        match &mut self.drag {
            Some(Drag::Click { start, .. }) => {
                let (a, b) = (*start, ev.position);
                let (ra, rb) = ((f32::from(a.x), f32::from(a.y)), (f32::from(b.x), f32::from(b.y)));
                if annotate::sort(Gesture::Relocate, ra, rb) == annotate::MenuAction::Pan {
                    // Passou do limiar: vira arrasto e alcanca o ponteiro de uma vez.
                    self.fr = viewer_math::pan_by(&self.fr, area, self.display_sz(), rb.0 - ra.0, rb.1 - ra.1);
                    self.drag = Some(Drag::Relocate(b));
                }
            }
            Some(Drag::Relocate(before)) => {
                let (dx, dy) = (f32::from(ev.position.x - before.x), f32::from(ev.position.y - before.y));
                *before = ev.position;
                self.fr = viewer_math::pan_by(&self.fr, area, self.display_sz(), dx, dy);
            }
            Some(Drag::Rectangle { current, .. } | Drag::Arrow { current, .. }) => *current = ev.position,
            None => {}
        }
        cx.notify();
    }

    pub fn on_up(&mut self, ev: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let (area, _) = self.area_and_origin();
        let end = self.relative(ev.position);
        // A view raiz recusa um grifo se o quadro atual ja for outro (`commit_highlight_in`).
        match self.drag.take() {
            Some(Drag::Rectangle { start, image, tool_by, .. } | Drag::Arrow { start, image, tool_by, .. })
                if tool_by && annotate::is_click(self.relative(start), end) =>
            {
                // Ferramenta ligada e so um clique: seleciona o grifo sob o ponteiro, como sem ela.
                let id = annotate::screen_highlight(&self.display_highlights, &self.fr, area, self.display_sz(), end);
                cx.emit(ViewerEvent::Select { image, id });
            }
            Some(Drag::Rectangle { start, image, .. }) => {
                // Clique (< annotate::ARRASTO_MINIMO px) ou retangulo fora da imagem: nada.
                let a = self.relative(start);
                if let Some(highlight) = annotate::rectangle(&self.fr, area, self.display_sz(), a, end, Self::new_id()) {
                    // Desenhado sobre a pagina girada: o vault guarda na imagem.
                    cx.emit(ViewerEvent::Highlight { image, highlight: highlight.from_display(self.rot) });
                }
            }
            Some(Drag::Arrow { start, image, .. }) => {
                // Arrasto: cauda -> ponta; clique: seta curta padrao; apertou no fundo: nada.
                let a = self.relative(start);
                if let Some(highlight) = annotate::gesture_arrow(&self.fr, area, self.display_sz(), a, end, Self::new_id()) {
                    cx.emit(ViewerEvent::Highlight { image, highlight: highlight.from_display(self.rot) });
                }
            }
            Some(Drag::Click { start, image: Some(image) }) => {
                let a = self.relative(start);
                if annotate::sort(Gesture::Relocate, a, end) == annotate::MenuAction::Click {
                    let id = annotate::screen_highlight(&self.display_highlights, &self.fr, area, self.display_sz(), end);
                    cx.emit(ViewerEvent::Select { image, id });
                }
            }
            Some(Drag::Click { image: None, .. } | Drag::Relocate(_)) | None => {}
        }
        cx.notify();
    }

    pub fn tool(&self) -> Tool {
        self.tool
    }

    /// Botao/menu de uma ferramenta de grifo: liga, troca ou desliga (`Tool::toggle`).
    pub fn toggle_tool(&mut self, target: Tool, cx: &mut Context<Self>) {
        self.tool = self.tool.toggle(target);
        self.drag = None;
        cx.notify();
    }

    pub fn disable_tool(&mut self, cx: &mut Context<Self>) {
        if self.tool != Tool::No {
            self.tool = Tool::No;
            self.drag = None;
            cx.notify();
        }
    }

    /// W/A/S/D (decididas em `focus_state::pan_key` pela view raiz): move a imagem
    /// `dir` passos de `viewer_math::pan_step` pixels de tela, como um arrasto.
    pub fn steps_pan(&mut self, dir: (f32, f32), cx: &mut Context<Self>) {
        let (area, _) = self.area_and_origin();
        let step = viewer_math::pan_step(self.fr.zoom, area);
        self.fr = viewer_math::pan_by(&self.fr, area, self.display_sz(), dir.0 * step, dir.1 * step);
        cx.notify();
    }

    fn on_key(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        let ch = k.key_char.as_deref().unwrap_or(k.key.as_str());
        if k.modifiers.platform || k.modifiers.control || k.modifiers.alt {
            return;
        }
        // `+`/`=`, `-`/`_`, `0` (`menus::KEYS`); os itens do menu Exibir chamam o mesmo.
        if let Some(a) = menus::key_action(ch).filter(|a| a.e_zoom()) {
            self.zoom_by(a, cx);
        }
    }

    /// Zoom pelas teclas e pelo menu Exibir: mais / menos (em torno do centro) ou ajustar a janela.
    pub fn zoom_by(&mut self, a: menus::MenuAction, cx: &mut Context<Self>) {
        match a {
            menus::MenuAction::ZoomUp => self.zoom(ZOOM_KEY, None, cx),
            menus::MenuAction::ZoomDown => self.zoom(1.0 / ZOOM_KEY, None, cx),
            menus::MenuAction::Adjust => {
                self.fr = viewer_math::default_framing();
                cx.notify();
            }
            _ => {}
        }
    }
}

/// Estados de um grifo na tela. Mudam so o contorno (tracejado, anel, alcas),
/// nunca a cor, que vem de `Highlight::color`.
#[derive(Clone, Copy, Default)]
struct QuickNoteStyle {
    /// Vai na proxima nota: contorno tracejado e mais grosso.
    pending: bool,
    /// Grifo da nota aberta: anel ciano por fora.
    emphasis: bool,
    /// Selecionado: anel branco e alcas (cantos, cauda e ponta).
    selected: bool,
}

/// Raio do ponto (ponteiro antigo, sem cauda) na tela.
const POINT_RADIUS: f32 = 7.0;
/// Comprimento (e largura da base) da cabeca da seta, em pixels de tela.
const ARROW_HEAD: f32 = 16.0;
/// Espessura da haste da seta.
const SHAFT: f32 = 3.0;
/// Anel do destaque (nota aberta).
const CYAN: u32 = 0x00e5ff;
/// Anel e alcas da selecao.
const WHITE: u32 = 0xffffff;
const HANDLE_BLUE: u32 = 0x1e88e5;
/// Lado das alcas da selecao.
const HANDLE: f32 = 8.0;
/// Quanto um grifo desenha alem da forma (anel ciano, halos, alcas, cabeca).
const DRAWING_MARGIN: f32 = 24.0;

fn window_point(origin: Point<Pixels>, p: (f32, f32)) -> Point<Pixels> {
    point(origin.x + px(p.0), origin.y + px(p.1))
}

/// Traco (aberto ou fechado) de `width` px, tracejado se `strokes`.
fn stroke(window: &mut Window, pts: &[Point<Pixels>], closed: bool, width: f32, strokes: Option<&[Pixels]>, color: Hsla) {
    let mut b = PathBuilder::stroke(px(width));
    if let Some(d) = strokes {
        b = b.dash_array(d);
    }
    b.add_polygon(pts, closed);
    if let Ok(p) = b.build() {
        window.paint_path(p, color);
    }
}

fn fill_in(window: &mut Window, pts: &[Point<Pixels>], color: Hsla) {
    let mut b = PathBuilder::fill();
    b.add_polygon(pts, true);
    if let Ok(p) = b.build() {
        window.paint_path(p, color);
    }
}

/// Quadrado/circulo de `side` centrado em `c` (borda `border` px).
fn mark(window: &mut Window, c: Point<Pixels>, side: f32, radius: f32, background: Hsla, border: f32, border_color: Hsla, style: BorderStyle) {
    let b = Bounds::new(point(c.x - px(side / 2.0), c.y - px(side / 2.0)), size(px(side), px(side)));
    window.paint_quad(quad(b, px(radius), background, px(border), border_color, style));
}

/// Desenha um grifo salvo (pixels relativos a area, deslocados por `origin`)
/// na cor `color` (`0xrrggbb`, de `annotate::color_rgb(g.color())`).
fn paint_highlight(window: &mut Window, origin: Point<Pixels>, f: Shape, color: u32, style: QuickNoteStyle) {
    let solid: Hsla = rgb(color).into();
    let translucent: Hsla = rgba((color << 8) | 0x26).into();
    let shadow: Hsla = rgba(0x0000008c).into();
    match f {
        Shape::Rectangle { x, y, w, h } => {
            // Anel de `width` px que ocupa de `folga - largura` a `slack` px por fora
            // da borda (a borda do gpui e desenhada para dentro do retangulo).
            let ring = |window: &mut Window, slack: f32, width: f32, c: Hsla| {
                let b = Bounds::new(window_point(origin, (x - slack, y - slack)), size(px(w + 2.0 * slack), px(h + 2.0 * slack)));
                window.paint_quad(quad(b, px(0.0), rgba(0x00000000), px(width), c, BorderStyle::Solid));
            };
            if style.emphasis {
                ring(window, 8.0, 3.0, rgb(CYAN).into());
            }
            if style.selected {
                // branco entre duas linhas escuras: visivel no papel claro e no fundo escuro
                ring(window, 5.0, 1.0, shadow);
                ring(window, 4.0, 2.0, rgb(WHITE).into());
                ring(window, 2.0, 1.0, shadow);
            }
            // Contorno escuro fino logo por fora da borda colorida: amarelo e branco
            // continuam legiveis numa digitalizacao clara.
            ring(window, 1.0, 1.0, shadow);
            let (width, border) = if style.pending { (3.0, BorderStyle::Dashed) } else { (2.0, BorderStyle::Solid) };
            let b = Bounds::new(window_point(origin, (x, y)), size(px(w), px(h)));
            window.paint_quad(quad(b, px(0.0), translucent, px(width), solid, border));
        }
        Shape::Arrow { tail, tip } => match annotate::arrow_geometry(tail, tip, ARROW_HEAD) {
            Some(g) => {
                let shaft = [window_point(origin, tail), window_point(origin, g.end_shaft)];
                let head = g.head.map(|p| window_point(origin, p));
                // Halos de dentro para fora pintados de fora para dentro: ciano
                // (destaque), branco (selecao), sombra escura (contraste no papel).
                let halo = |window: &mut Window, k: f32, c: Hsla| {
                    stroke(window, &shaft, false, SHAFT + 2.0 * k, None, c);
                    stroke(window, &head, true, 2.0 * k, None, c);
                };
                if style.emphasis {
                    halo(window, 6.0, rgb(CYAN).into());
                }
                if style.selected {
                    // branco com uma borda escura por fora (e a sombra por dentro)
                    halo(window, 4.5, shadow);
                    halo(window, 3.5, rgb(WHITE).into());
                }
                halo(window, 1.2, shadow);
                let (width, strokes) = if style.pending { (SHAFT + 1.0, Some([px(9.0), px(5.0)])) } else { (SHAFT, None) };
                stroke(window, &shaft, false, width, strokes.as_ref().map(|t| t.as_slice()), solid);
                fill_in(window, &head, solid);
            }
            // Ponta em cima da cauda na tela (zoom muito pequeno): um ponto.
            None => paint_highlight(window, origin, Shape::ImagePoint { x: tip.0, y: tip.1 }, color, style),
        },
        Shape::ImagePoint { x, y } => {
            let c = window_point(origin, (x, y));
            let r = POINT_RADIUS;
            if style.emphasis {
                mark(window, c, 2.0 * (r + 6.0), r + 6.0, rgba(0x00000000).into(), 3.0, rgb(CYAN).into(), BorderStyle::Solid);
            }
            if style.selected {
                mark(window, c, 2.0 * (r + 5.0), r + 5.0, rgba(0x00000000).into(), 1.0, shadow, BorderStyle::Solid);
                mark(window, c, 2.0 * (r + 4.0), r + 4.0, rgba(0x00000000).into(), 2.0, rgb(WHITE).into(), BorderStyle::Solid);
                mark(window, c, 2.0 * (r + 2.0), r + 2.0, rgba(0x00000000).into(), 1.0, shadow, BorderStyle::Solid);
            }
            let border = if style.pending { 3.0 } else { 2.0 };
            mark(window, c, 2.0 * r, r, solid, border, rgb(0x000000).into(), BorderStyle::Solid);
        }
    }
    if style.selected {
        for p in annotate::handles(f) {
            mark(window, window_point(origin, p), HANDLE, 1.0, rgb(WHITE).into(), 1.5, rgb(HANDLE_BLUE).into(), BorderStyle::Solid);
        }
    }
}

/// Retangulo elastico ou seta provisoria (gesto em andamento): branco, fino.
fn paint_preview(window: &mut Window, origin: Point<Pixels>, f: Shape) {
    match f {
        Shape::Rectangle { x, y, w, h } => {
            let b = Bounds::new(window_point(origin, (x, y)), size(px(w), px(h)));
            window.paint_quad(quad(b, px(0.0), rgba(0xffffff1a), px(1.0), rgb(WHITE), BorderStyle::Dashed));
        }
        Shape::Arrow { tail, tip } => {
            if let Some(g) = annotate::arrow_geometry(tail, tip, ARROW_HEAD) {
                let shaft = [window_point(origin, tail), window_point(origin, g.end_shaft)];
                let head = g.head.map(|p| window_point(origin, p));
                stroke(window, &shaft, false, 4.0, None, rgba(0x0000008c).into());
                stroke(window, &shaft, false, 2.0, Some(&[px(6.0), px(4.0)]), rgb(WHITE).into());
                fill_in(window, &head, rgb(WHITE).into());
            }
        }
        Shape::ImagePoint { .. } => {}
    }
}

/// Caixa vermelha com a mensagem: o marcador de erro do quadro.
fn error_marker(msg: String) -> impl IntoElement {
    div()
        .max_w(px(720.0))
        .p_4()
        .rounded_md()
        .border_2()
        .border_color(rgb(crate::theme::current().border_error))
        .bg(rgb(crate::theme::current().background_error))
        .text_color(rgb(crate::theme::current().text_error))
        .flex()
        .flex_col()
        .gap_1()
        .child(div().font_weight(gpui::FontWeight::BOLD).child(t!("error.open_image.title")))
        .child(msg)
        .child(div().text_sm().text_color(rgb(crate::theme::current().text_error)).child(t!("error.open_image.hint")))
}

impl EventEmitter<ViewerEvent> for Viewer {}

/// Borda da imagem em foco (as teclas da imagem e W/A/S/D valem) ou nao. Por
/// cima da area, sem ocupar espaco: o canvas continua medindo a area inteira.
fn focus_border(focused: bool) -> impl IntoElement {
    let color = if focused { crate::theme::current().accent } else { crate::theme::current().strong_border };
    div().absolute().top_0().left_0().size_full().border_2().border_color(rgb(color))
}

impl Render for Viewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.inform_window(window, cx);
        let focused = self.focus_handle.is_focused(window);
        let root_dir = div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key))
            .size_full()
            .relative()
            .bg(rgb(crate::theme::current().background))
            .text_color(rgb(crate::theme::current().text));

        let error = self.error.clone().or_else(|| {
            self.paint_failure.borrow().as_ref().map(|m| t!("error.image_not_drawn", m))
        });
        if let Some(error) = error {
            let painted = self.painted.clone();
            return root_dir
                .flex()
                .items_center()
                .justify_center()
                .p_4()
                .child(error_marker(error))
                .child(canvas(|_, _, _| {}, move |_, _, _, _| painted.set(true)).size_0())
                .child(focus_border(focused))
                .into_any_element();
        }

        let full = self.layers.full.as_ref().map(|t| t.img.clone());
        let base = self.layers.base.as_ref().map(|t| t.img.clone());
        let tile = self.layers.tile.as_ref().map(|(t, j)| (t.img.clone(), *j));
        let fr = self.fr.clone();
        let sz = self.display_sz();
        let sz_original = self.image_sz;
        let rot = self.rot;
        let highlights = self.display_highlights.clone();
        let area_cell = self.area.clone();
        let painted = self.painted.clone();
        let failure = self.paint_failure.clone();
        let marks = self.marks.clone();
        let preview = match &self.drag {
            Some(Drag::Rectangle { start, current, .. }) => Some(annotate::elastic(self.relative(*start), self.relative(*current))),
            Some(Drag::Arrow { start, current, .. }) => {
                let (area, _) = self.area_and_origin();
                annotate::arrow_preview(&fr, area, sz, self.relative(*start), self.relative(*current))
            }
            _ => None,
        };
        let area = canvas(
            move |bounds, window, _| {
                if area_cell.get() != bounds {
                    // Area nova (redimensionar, gaveta): mais um quadro, que avisa a janela visivel dela.
                    area_cell.set(bounds);
                    window.request_animation_frame();
                }
            },
            move |bounds: Bounds<Pixels>, (), window: &mut Window, _: &mut App| {
                if full.is_none() && base.is_none() && tile.is_none() {
                    return;
                }
                let area = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                let (ox, oy) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
                let factor = window.scale_factor();
                // Caixa relativa a area -> caixa da janela arredondada para fora no pixel fisico.
                let outside_to = |c: (f32, f32, f32, f32)| {
                    let ((x, y), (w, h)) = viewer_math::textbox_to_outside((c.0 + ox, c.1 + oy, c.2 + ox, c.3 + oy), factor);
                    Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
                };
                let page = viewer_math::page_textbox(&fr, area, sz);
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    window.paint_quad(fill(bounds, rgb(crate::theme::current().background)));
                    let mut paints: Vec<(Bounds<Pixels>, Arc<RenderImage>)> = Vec::new();
                    if let Some(img) = full {
                        // Sem ajuste: a textura cheia sozinha, como sempre foi.
                        let destination = Bounds::new(
                            point(bounds.origin.x + px(page.0), bounds.origin.y + px(page.1)),
                            size(px(page.2 - page.0), px(page.3 - page.1)),
                        );
                        paints.push((destination, img));
                    }
                    if let Some(img) = base {
                        // A base cobre a pagina inteira (por baixo do tile).
                        paints.push((outside_to(page), img));
                    }
                    if let Some((img, j)) = tile {
                        paints.push((outside_to(viewer_math::rotated_window_textbox(&fr, area, sz_original, rot, &j)), img));
                    }
                    for (destination, img) in paints {
                        if let Err(e) = window.paint_image(destination, Corners::default(), img, 0, false) {
                            // Nunca uma janela em branco: o proximo quadro mostra o erro.
                            *failure.borrow_mut() = Some(e.to_string());
                            window.request_animation_frame();
                            return;
                        }
                    }
                    painted.set(true);
                    let e = |id: &Option<String>, g: &Highlight| id.as_deref() == Some(g.id());
                    // O selecionado por ultimo: anel e alcas nunca ficam cobertos.
                    let (sel, rest): (Vec<&Highlight>, Vec<&Highlight>) = highlights.iter().partition(|g| e(&marks.selected, g));
                    for g in rest.into_iter().chain(sel) {
                        let shape = annotate::in_screen(&fr, area, sz, g);
                        if annotate::outside_area(shape, area, DRAWING_MARGIN) {
                            continue; // fora do quadro (zoom): nada de montar caminhos
                        }
                        let style = QuickNoteStyle {
                            pending: e(&marks.pending, g),
                            emphasis: e(&marks.emphasis, g),
                            selected: e(&marks.selected, g),
                        };
                        paint_highlight(window, bounds.origin, shape, annotate::color_rgb(g.color()), style);
                    }
                    if let Some(f) = preview {
                        paint_preview(window, bounds.origin, f);
                    }
                });
            },
        )
        .size_full();

        let mut root_dir = if self.tool == Tool::No { root_dir } else { root_dir.cursor_crosshair() };
        root_dir = root_dir
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_down))
            .on_mouse_move(cx.listener(Self::on_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_up))
            .child(area);
        if self.loading {
            root_dir = root_dir.child(
                div()
                    .absolute()
                    .bottom_2()
                    .left_2()
                    .px_2()
                    .rounded_md()
                    .bg(rgb(0x000000))
                    .opacity(0.7)
                    .text_sm()
                    .child(t!("status.loading_viewer")),
            );
        }
        root_dir.child(focus_border(focused)).into_any_element()
    }
}
