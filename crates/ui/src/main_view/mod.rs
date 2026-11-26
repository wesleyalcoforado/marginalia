//! View raiz (gpui), layout estilo VS Code (modelo em `layout`): barra de
//! atividades a esquerda (Notas, Fontes, Tarefas; embaixo ajuda e a engrenagem
//! das Configuracoes), o painel esquerdo (a gaveta aberta), a imagem, o painel de ajustes e a
//! barra fina da direita (icone de ajustes sempre visivel), a nota rapida em
//! largura total e a barra de status (abrir fonte, anterior/seguinte, vault ·
//! fonte · pagina · imagem; clicar abre o seletor de vault; a direita os avisos
//! e confirmacoes). Os paineis encaixam: a imagem encolhe, nada fica por cima
//! dela. Tambem o fluxo "Abrir fonte". A logica fica em `AppState` /
//! `open_source` / `annotate` / `quick_note` (sem gpui); aqui so se traduz
//! evento -> chamada e se roda o trabalho pesado no executor de fundo (nada de
//! NAS nem decodificacao na thread da interface).
//!
//! Nota rapida: Cmd-K (de qualquer foco), `/` (com a imagem em foco) ou um
//! clique poe o foco na caixa; Esc e Cmd-Shift-K voltam a imagem (`focus_state`; a
//! borda azul mostra quem tem o foco). A
//! logica fica em `quick_note_session` (salvamento automatico): a 1a letra cria o
//! rascunho (`[]` no inicio cria tarefa) com a imagem, a pagina, o
//! enquadramento e o grifo pendente atuais; o texto vai ao disco com debounce e
//! o id e fixado uma vez (10 s sem digitar, sair da caixa, outra imagem, outro
//! vault, Cmd+Enter, sair do app). Enter quebra a linha. Outra imagem/pagina/fonte ou outro vault
//! TERMINAM a nota (a caixa limpa; a proxima tecla comeca uma nota nova no
//! quadro novo). Cmd+Enter tambem limpa a caixa (quando o disco tem o texto) e
//! devolve o foco a imagem. Cada gravacao roda no executor de fundo, uma de
//! cada vez; se falhar, o texto fica na caixa, o erro aparece na barra e a
//! sessao tenta de novo com espera crescente. Ao sair, o texto ainda nao
//! seguro vai primeiro para o registro de recuperacao (dados do app) e o vault
//! tem ate 3 s; ao abrir, o registro volta para o vault (marcador no rodape
//! enquanto nao puder).
//!
//! Gavetas (Task 14, logica em `drawers`): Cmd-1 notas, Cmd-2 fontes, Cmd-3
//! tarefas (a ordem dos icones da barra de atividades) abrem/fecham uma gaveta de
//! [`drawers::WIDTH`] px no painel esquerdo; a area da
//! imagem encolhe (o visualizador mede a area a cada quadro, entao zoom, arrasto
//! e grifos seguem alinhados). As listas sao lidas no executor de fundo
//! ([`MainView::refresh`]) ao abrir o app, ao abrir uma gaveta e depois de cada
//! gravacao; a lista guardada fica na tela enquanto recarrega.
//!
//! Task 15 (logica em `vault_picker`, `relink` e `AppState`): seletor de vault
//! (Cmd-Shift-V; abre sozinho sem vault), aviso "Fonte indisponivel" /
//! "Fonte nao encontrada" com "Tentar de novo" e "Apontar nova localizacao...",
//! "Renomear" na gaveta de fontes (a caixa e o mesmo editor da nota rapida), e
//! recarga do vault no fundo quando a janela volta ao foco. Abrir pasta/zip,
//! conferir o nome, registrar a fonte, ler os grifos, validar um vault, apontar
//! uma nova localizacao e renomear rodam no executor de fundo.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    canvas, div, fill, point, prelude::*, px, rgb, size, AnyView, App, Bounds, Context, DispatchPhase, ElementId, Entity,
    ExternalPaths, FocusHandle, FontWeight, Hsla, KeyDownEvent, ModifiersChangedEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    PathBuilder, PathPromptOptions, Pixels, PromptLevel, SharedString, Subscription, Task, Window, WindowHandle,
};
use marginalia_core::highlights::{Highlight, PALETTE};
use marginalia_core::model::ImageRef;
use marginalia_core::filters::Adjustments;
use texts::t;

use crate::app_state::{
    sort_failure_rename, step, renaming_follower, OpenNote, Opening, AppState, Control, Input, OpeningFailure,
    FailureRename, RotationAfter, Frame, QuickNoteError, ScheduleStep, Received, ZipMode,
};
use crate::schedule_adjustment::ScheduleAdjustment;
use crate::load::{self, ViewWindow, Request, Quality, Response};
use crate::help;
use crate::annotate;
use crate::drawers::{
    self, DeletionAction, DrawerAction, DrawerSourceKind, TasksFilter, LineSource, Drawer, Drawers, NoteLine, DrawerSourceStatus,
    LineTask,
};
use crate::focus_state::{self, Command, Focus, Transition};
use crate::icons::{icon, Icon};
use crate::layout::{self, Layout, StatusMessage, StatusInfo};
use crate::menus;
use crate::quick_note_session::{Effect, QuickNoteSession};
use crate::quick_note_view::{QuickNoteBox, QuickNoteEvent};
use crate::open_source;
use crate::relink::{self, Reason};
use crate::slider::{self, Bar};
use crate::vault_picker::{self, PickerAction, VaultError, Picker};
use crate::viewer::{Viewer, ViewerEvent};

mod image_adjustments;
mod barras;
mod commands;
mod source;
mod drawer_views;
mod quick_note;
mod records;
mod help_screen;
mod screen_config;
mod about_screen;
mod vault;
mod widgets;

use self::{source::*, drawer_views::*, widgets::*};

/// Quanto tempo "nota criada: <id>" fica na barra.
const CONFIRMATION: Duration = Duration::from_millis(2500);

/// Algarismos que a caixa "Ir para" aceita (cabem na caixa sem quebrar a linha).
const MAX_DIGITS_GO_TO: usize = 6;

pub struct MainView {
    state: AppState,
    viewer: Entity<Viewer>,
    /// Painel de ajustes encaixado a direita (o esquerdo e a gaveta aberta; ver `layout`).
    open_adjustments: bool,
    /// Cabecalho "Ajustes" do painel recolhido (so as barras somem; a paleta fica).
    collapsed_adjustments: bool,
    /// A janela Sobre (propria do macOS), se aberta; uma so por vez.
    about_window: Option<WindowHandle<about_screen::ViewAbout>>,
    /// Tela de ajuda (sobreposta) aberta: `?`, o icone da barra de atividades ou o menu Ajuda.
    open_help: bool,
    /// Menuzinho de cores dos grifos (barra de ferramentas) aberto.
    open_colors: bool,
    /// Barra de menus (fora do macOS): o titulo em destaque, o menu aberto e o item marcado.
    menu: crate::menu_bar::MenuBar,
    /// `Alt` apertado agora: a barra de menus mostra as letras de acesso sublinhadas.
    alt_held: bool,
    /// Onde o menu aberto foi desenhado (coordenadas da janela), para o smoke conferir a posicao.
    limits_menu: Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
    /// Tela de Configuracoes (sobreposta) aberta: Cmd-, , a engrenagem ou o menu Marginalia.
    config_open: bool,
    /// Caixas numericas das barras (na ordem de `slider::ALL`): o editor da
    /// nota rapida em uma linha (Enter confirma, Esc devolve).
    textboxes: Vec<Entity<QuickNoteBox>>,
    /// Caixas com texto digitado ainda sem Enter/Esc (as outras seguem o valor).
    dirty_all: slider::DirtyTextboxes,
    /// Barra sendo arrastada (do aperto na trilha ate soltar o mouse).
    bar_drag: Option<Bar>,
    /// A borda do painel das gavetas esta sendo arrastada (redimensionar).
    drawer_drag: bool,
    /// A barra de rolagem de uma lista esta sendo arrastada (a lista que ela rola).
    scroll_drag: Option<gpui::ScrollHandle>,
    /// Onde cada trilha foi pintada no ultimo quadro `(esquerda, largura)`:
    /// ponteiro -> valor (`slider::Bar::point_value`).
    tracks: Rc<RefCell<[Option<(f32, f32)>; 5]>>,
    /// Carga em andamento. Substituir cancela a anterior (vale a mais nova).
    task: Option<Task<()>>,
    /// Tile/base/gravacao dos ajustes (logica em `schedule_adjustment`).
    schedule: ScheduleAdjustment,
    /// O filtro em voo da agenda (um so; separado da carga do quadro, para a
    /// navegacao nao engolir o aviso de fim que libera a agenda).
    filter: Option<Task<()>>,
    /// Proximo tick da agenda (base depois de parar, tile depois de a janela parar, gravacao adiada).
    schedule_tick: Option<Task<()>>,
    /// Gravacoes do appdata rodando no fundo.
    writes: usize,
    opening: Option<Task<()>>,
    quick_note: Entity<QuickNoteBox>,
    /// Salvamento automatico da nota rapida (logica em `quick_note_session`).
    session: QuickNoteSession,
    /// Gravacao da nota rodando no fundo (uma de cada vez: a sessao garante).
    work_quick_note: Option<Task<()>>,
    /// Proximo tick da sessao (debounce, 10 s para fixar o id, nova tentativa).
    quick_note_tick: Option<Task<()>>,
    /// Geracao da confirmacao na barra (so a mais nova se apaga sozinha).
    generation_confirmation: u64,
    /// Gavetas laterais (listas guardadas, filtros, selecao, menu de promocao).
    drawers: Drawers,
    /// Leitura das listas em andamento (substituir cancela a anterior).
    reload: Option<Task<()>>,
    /// Seletor de vault (Cmd-Shift-V).
    picker: Picker,
    /// Caixa de "Renomear fonte" (o editor da nota rapida) e a fonte em renomeacao.
    rename: Entity<QuickNoteBox>,
    /// Busca no texto da gaveta de notas (uma linha).
    search: Entity<QuickNoteBox>,
    /// A lista suspensa de fontes da gaveta de notas esta aberta.
    sources_menu: bool,
    scroll_notes: gpui::ScrollHandle,
    tasks_scroll: gpui::ScrollHandle,
    renaming: Option<String>,
    /// Caixa de nome da faixa de registro (marcar inicio / renomear) e o que ela esta fazendo.
    record_name: Entity<QuickNoteBox>,
    /// "Ir para" da barra de ferramentas (numero da imagem ou pagina).
    ir_to: Entity<QuickNoteBox>,
    /// O usuario esta digitando na caixa "Ir para": ela guarda o texto ate Enter/Esc/sair
    /// (fora disso, mostra a imagem atual e acompanha a navegacao).
    go_to_dirty: bool,
    record_editing: Option<records::RecordEditing>,
    /// Registro cuja exclusao (com fim marcado) espera confirmacao na faixa.
    record_deleting: Option<String>,
    /// Registro criado nesta sessao e ainda sem fim: o unico que "Cancelar" descarta sem pedir confirmacao.
    created_just: Option<String>,
    /// Troca de vault, nova localizacao ou renomeacao rodando no fundo, e o que mostrar.
    work: Option<Task<()>>,
    /// Troca de vault, nova localizacao e renomeacao: um de cada vez.
    exclusive: relink::Exclusive,
    /// Foco do teclado da gaveta aberta (`Focus::Drawer`): as teclas dela so valem com ele.
    drawer_focus: FocusHandle,
    /// Clique/Enter numa linha: o foco vai a caixa quando o texto chegar (R26).
    focus_on_load: focus_state::FocusToTheLoad,
    /// O texto chegou: o proximo quadro da o foco a caixa (aqui nao ha `Window`).
    focus_textbox_now: bool,
    _signature_list: Vec<Subscription>,
}

impl MainView {
    pub fn new(state: AppState, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let viewer = cx.new(Viewer::new);
        let quick_note = cx.new(QuickNoteBox::new);
        let rename = cx.new(|cx| QuickNoteBox::new(cx).single_line());
        let record_name = cx.new(|cx| QuickNoteBox::new(cx).single_line());
        let ir_to = cx.new(|cx| QuickNoteBox::new(cx).single_line().so_digits(MAX_DIGITS_GO_TO));
        let search = cx.new(|cx| {
            let mut b = QuickNoteBox::new(cx).single_line();
            b.configure(true, t!("drawer.search.placeholder"), cx);
            b
        });
        let quick_note_focus = quick_note.read(cx).focus_handle().clone();
        let drawer_focus = cx.focus_handle();
        let focus_go_to = ir_to.read(cx).focus_handle().clone();
        let textboxes: Vec<Entity<QuickNoteBox>> = slider::ALL.iter().map(|_| cx.new(|cx| QuickNoteBox::new(cx).single_line())).collect();
        let mut signatures = Vec::new();
        for (b, textbox) in slider::ALL.into_iter().zip(&textboxes) {
            signatures.push(cx.subscribe_in(textbox, window, move |v, _, ev: &QuickNoteEvent, window, cx| match ev {
                QuickNoteEvent::Finish(t) => {
                    v.confirm_number(b, t, window, cx);
                }
                QuickNoteEvent::Exit => v.cancel_number(b, window, cx),
                // O usuario digitou: a caixa guarda o texto ate Enter/Esc.
                QuickNoteEvent::Changed(_) => v.dirty_all.changed(b),
            }));
            // Saiu da caixa sem Enter: o texto cai e a caixa volta a mostrar o valor atual (no proximo quadro).
            let focus_state = textbox.read(cx).focus_handle().clone();
            signatures.push(cx.on_blur(&focus_state, window, move |v, _, cx| {
                v.dirty_all.clear(b);
                cx.notify()
            }));
        }
        signatures.extend([
            // O foco saiu da gaveta (clique na imagem, Cmd-K, outra janela...): a
            // confirmacao de excluir, que so ouve as teclas na gaveta, cai.
            cx.on_blur(&drawer_focus, window, |v, window, cx| {
                let in_drawer = v.current_focus(window, cx) == Focus::Drawer;
                if v.drawers.deletion_confirming().is_some() && !in_drawer {
                    v.drawers.focus_changed(false);
                    cx.notify();
                }
            }),
            cx.subscribe_in(&rename, window, |v, _, ev: &QuickNoteEvent, window, cx| match ev {
                QuickNoteEvent::Finish(t) => v.confirm_rename(t, window, cx),
                QuickNoteEvent::Exit => v.cancel_rename(window, cx),
                QuickNoteEvent::Changed(_) => {}
            }),
            cx.subscribe_in(&record_name, window, |v, _, ev: &QuickNoteEvent, window, cx| match ev {
                QuickNoteEvent::Finish(t) => v.confirm_name_record(t, window, cx),
                QuickNoteEvent::Exit => v.cancel_name_record(window, cx),
                QuickNoteEvent::Changed(_) => {}
            }),
            cx.subscribe_in(&ir_to, window, |v, _, ev: &QuickNoteEvent, window, cx| match ev {
                QuickNoteEvent::Finish(t) => v.confirm_go_to(t, window, cx),
                QuickNoteEvent::Exit => v.cancel_go_to(window, cx),
                QuickNoteEvent::Changed(_) => v.go_to_dirty = true,
            }),
            // A busca das notas filtra a cada letra; Enter e Esc devolvem o foco a lista.
            cx.subscribe_in(&search, window, |v, _, ev: &QuickNoteEvent, window, cx| match ev {
                QuickNoteEvent::Changed(t) => {
                    v.drawers.set_search(t);
                    cx.notify();
                }
                QuickNoteEvent::Finish(_) | QuickNoteEvent::Exit => v.focus_drawer(window, cx),
            }),
            // Saiu da caixa (Esc, clique na imagem, outra janela): grava e fixa o id.
            cx.on_blur(&quick_note_focus, window, |v, _, cx| v.left_quick_note(cx)),
            // "Ir para": saiu da caixa sem Enter, o texto cai e ela volta a mostrar a imagem atual.
            cx.on_blur(&focus_go_to, window, |v, _, cx| {
                v.go_to_dirty = false;
                cx.notify()
            }),
            // Voltou ao app (talvez depois de editar o vault no Obsidian): rele no fundo.
            // Com uma recarga ainda lendo (vault num NAS lento), nao empilha outra.
            cx.observe_window_activation(window, |v, window, cx| {
                if window.is_window_active() && v.reload.is_none() {
                    v.refresh(cx);
                }
            }),
            cx.subscribe_in(&viewer, window, |v, _, ev: &ViewerEvent, window, cx| match ev {
                ViewerEvent::Highlight { image, highlight } => v.add_highlight(image, highlight.clone(), cx),
                ViewerEvent::Select { image, id } => v.select_highlight(image, id.clone(), cx),
                // A janela de uma imagem que ja nao e a atual (a nova ainda carrega) nao vale.
                ViewerEvent::ViewWindow { image, view_window } if *image == v.state.current.image => {
                    v.window_changed(*view_window, window, cx)
                }
                ViewerEvent::ViewWindow { .. } => {}
            }),
            cx.subscribe_in(&quick_note, window, |v, _, ev: &QuickNoteEvent, window, cx| match ev {
                QuickNoteEvent::Finish(t) => v.finish_quick_note(t, window, cx),
                QuickNoteEvent::Exit => v.focus_command(Command::Esc, window, cx),
                QuickNoteEvent::Changed(t) => v.quick_note_changed(t, cx),
            }),
            // Janela fechada (a view e solta): grava a nota em edicao e guarda a
            // ultima posicao aberta. O `on_app_quit` do main faz o mesmo na saida;
            // quem vier depois acha tudo salvo e nao grava de novo.
            cx.on_release(|v: &mut MainView, _| {
                v.flush_quick_note();
                v.save_position()
            }),
        ]);
        let mut v = MainView {
            state,
            viewer,
            open_adjustments: false,
            collapsed_adjustments: false,
            about_window: None,
            open_help: false,
            open_colors: false,
            menu: Default::default(),
            alt_held: false,
            limits_menu: Rc::new(std::cell::Cell::new(None)),
            config_open: false,
            textboxes,
            dirty_all: slider::DirtyTextboxes::default(),
            bar_drag: None,
            drawer_drag: false,
            scroll_drag: None,
            tracks: Rc::new(RefCell::new([None; 5])),
            task: None,
            schedule: ScheduleAdjustment::new(),
            filter: None,
            schedule_tick: None,
            writes: 0,
            opening: None,
            quick_note,
            session: QuickNoteSession::new(),
            work_quick_note: None,
            quick_note_tick: None,
            generation_confirmation: 0,
            drawers: Drawers::default(),
            reload: None,
            picker: Picker::default(),
            rename,
            search,
            sources_menu: false,
            scroll_notes: gpui::ScrollHandle::new(),
            tasks_scroll: gpui::ScrollHandle::new(),
            renaming: None,
            record_name,
            ir_to,
            go_to_dirty: false,
            record_editing: None,
            record_deleting: None,
            created_just: None,
            work: None,
            exclusive: relink::Exclusive::default(),
            drawer_focus,
            focus_on_load: focus_state::FocusToTheLoad::default(),
            focus_textbox_now: false,
            _signature_list: signatures,
        };
        v.apply_config(cx);
        if let Some(l) = v.state.app_data.drawers_width() {
            v.drawers.set_width(l);
        }
        if v.state.vault.is_none() {
            // Primeira vez (sem --vault, MARGINALIA_VAULT nem ultimo vault): o seletor.
            v.open_picker(cx);
        }
        v.sync(cx);
        v.restore_quick_note(cx);
        v.refresh(cx);
        v
    }

    pub fn drawers(&self) -> &Drawers {
        &self.drawers
    }

    pub fn drawers_mut(&mut self) -> &mut Drawers {
        &mut self.drawers
    }

    /// Rele o vault no executor de fundo (`AppState::reload`): notas,
    /// tarefas, fontes (com o status do caminho, que pode estar no
    /// NAS) e os grifos da fonte aberta. Nunca na thread da interface. A lista
    /// guardada fica na tela ate a nova chegar (filtros, linha escolhida, rolagem
    /// e quadro aberto ficam); uma recarga superada e descartada. Roda ao abrir o
    /// app, quando a janela volta ao foco, ao abrir uma gaveta e depois de cada
    /// gravacao, troca de vault, renomeacao ou nova localizacao.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(r) = self.state.reload() else { return };
        let g = self.drawers.request_reload();
        self.reload = Some(cx.spawn(async move |this, cx| {
            let reread = cx.background_executor().spawn(async move { r.execute() }).await;
            let _ = this.update(cx, |v, cx| {
                let l = v.state.receive_reload(reread);
                let resume = l.resume.clone();
                if v.drawers.receive(g, l) {
                    v.reload = None;
                }
                // Fixacoes de id paradas no meio: terminam na fila da nota rapida,
                // antes de qualquer edicao do documento novo (que fica escondido).
                let mut e = Effect::default();
                for (kind, old) in resume {
                    e.join(v.session.resume(&mut v.state, kind, &old, Instant::now()));
                }
                v.apply_quick_note(e, cx);
                v.sync(cx);
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Paineis encaixados agora (o esquerdo e a gaveta aberta).
    pub fn layout(&self) -> Layout {
        Layout::de(&self.drawers, self.open_adjustments)
    }

    pub fn viewer(&self) -> &Entity<Viewer> {
        &self.viewer
    }

    pub fn quick_note(&self) -> &Entity<QuickNoteBox> {
        &self.quick_note
    }

    /// Leva ao visualizador e a caixa o que o estado tem de grifos e de vault, e
    /// deixa a nota rapida perceber troca de imagem/vault (fixa o id; uma nota
    /// do vault anterior termina la).
    fn sync(&mut self, cx: &mut Context<Self>) {
        self.schedule_follow_image(cx);
        self.pause_quick_note();
        let e = self.session.observe(&mut self.state, Instant::now());
        self.apply_quick_note(e, cx);
    }

    fn sync_screen(&mut self, cx: &mut Context<Self>) {
        let e = &self.state;
        let (highlights, emphasis) = (e.image_highlights().to_vec(), e.emphasis().map(str::to_string));
        let pending = e.pending_highlight().map(|g| g.id().to_string());
        let selected = e.annot.selected.clone();
        self.viewer.update(cx, |v, cx| {
            v.set_highlights(&highlights, emphasis.as_deref(), pending.as_deref(), selected.as_deref(), cx)
        });
        // Entre o clique numa linha e o texto dela chegar, a caixa fica limpa e
        // desligada: o que se digitasse ali iria para a nota errada.
        let loading = self.quick_note_loading();
        // Nada mais carregando e o texto nao veio (falhou, cancelada): o pedido de foco cai.
        self.focus_on_load.after(false, loading);
        let e = &self.state;
        let (enabled, hint) = match (&e.vault, e.source_has()) {
            (None, _) => (false, t!("quick_note.hint.no_vault")),
            (Some(_), _) if loading => (false, t!("quick_note.hint.loading")),
            (Some(_), false) => (false, t!("quick_note.hint.no_source")),
            (Some(_), true) => (true, t!("quick_note.hint.quick")),
        };
        self.quick_note.update(cx, |n, cx| n.configure(enabled, hint, cx));
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Nao ha carga, abertura, troca de vault, nova localizacao, renomeacao,
    /// filtro (tile ou base) nem gravacao de ajustes em andamento ou pendente.
    pub fn idle(&self) -> bool {
        self.opening.is_none()
            && !self.state.loading()
            && self.work.is_none()
            && !self.session.busy()
            && self.schedule.idle()
            && self.writes == 0
    }

    /// O que impede [`MainView::idle`], numa linha (o smoke a mostra quando a
    /// tela nao fica pronta no prazo).
    pub fn debug(&self) -> String {
        format!(
            "abrindo {} carregando {} trabalho {} sessao {} gravacoes {} agenda {:?}",
            self.opening.is_some(),
            self.state.loading(),
            self.work.is_some(),
            self.session.busy(),
            self.writes,
            self.schedule
        )
    }

    /// Um tile (a janela visivel filtrada) dos ajustes esta em voo (o smoke muda o
    /// valor nesse instante e confere que ele chega, regra R2).
    pub fn tile_in_flight(&self) -> bool {
        self.schedule.tile_in_flight()
    }
}

impl Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Layout VS Code: atividades | painel esquerdo | imagem | ajustes | barra da
        // direita. Os paineis encaixam (larguras de `layout::widths`, recalculadas a
        // cada quadro com a largura da janela) e a imagem fica com o resto; o
        // visualizador mede a area a cada quadro (canvas), entao enquadramento e
        // grifos seguem alinhados.
        // Rede de seguranca do on_blur da gaveta: com o foco fora dela, a
        // confirmacao de excluir (que so ouve teclas la) nao fica na tela.
        if self.drawers.deletion_confirming().is_some() {
            let in_drawer = self.current_focus(window, cx) == Focus::Drawer;
            self.drawers.focus_changed(in_drawer);
        }
        if std::mem::take(&mut self.focus_textbox_now) {
            // O texto pedido por um clique/Enter chegou (fora de um quadro, sem `Window`).
            cx.defer_in(window, |v, w, cx| {
                v.quick_note.read(cx).focus_handle().focus(w);
                cx.notify();
            });
        }
        self.sync_textboxes(cx);
        let layout = self.layout();
        let view_window = f32::from(window.viewport_size().width);
        let widths = layout::widths_with(view_window, &layout, self.drawers.width());
        // A barra de ferramentas fica so na coluna da imagem (os paineis laterais sobem ate o titulo).
        let no_source = !self.state.source_has() && self.opening.is_none();
        let area_image = div()
            .flex_1()
            .min_h(px(0.0))
            .w_full()
            .relative()
            .overflow_hidden()
            .child(self.viewer.clone())
            .when(no_source, |d| d.child(self.invitation_open_source(cx)))
            .when(self.open_colors, |d| d.child(self.colors_background(cx)));
        let mut image = div().flex_1().min_w(px(0.0)).h_full().flex().flex_col().overflow_hidden();
        if self.visible_tools() && self.state.source_has() {
            image = image.child(self.tools_bar(cx));
        }
        if let Some(f) = self.record_strip(cx) {
            image = image.child(f);
        }
        let image = image.child(area_image);
        let mut middle = div().flex_1().min_h(px(0.0)).flex().flex_row().overflow_hidden().child(self.activities_bar(cx));
        if let Some(g) = layout.left {
            middle = middle.child(self.drawer(g, widths.left, window, cx));
        }
        middle = middle.child(image);
        if layout.open_adjustments {
            middle = middle.child(self.panel(widths.image_adjustments, cx));
        }
        middle = middle.child(self.right_bar(cx));
        let root_dir = div()
            .size_full()
            .flex()
            .flex_col()
            .relative()
            .bg(rgb(crate::theme::current().background))
            .text_color(rgb(crate::theme::current().text))
            .on_key_down(cx.listener(Self::on_key))
            // Barra de menus (fora do macOS): as teclas dela entram na fase de captura, antes da
            // caixa de nota (que engoliria as setas); o `Alt` apertado mostra as letras de acesso.
            .capture_key_down(cx.listener(Self::on_key_menu))
            .on_modifiers_changed(cx.listener(Self::changed_modifiers))
            .capture_any_mouse_down(cx.listener(|v, _: &MouseDownEvent, _, cx| {
                // So a barra em destaque (sem menu aberto): clicar em qualquer lugar a deixa.
                if v.menu.active() && v.menu.open_at().is_none() {
                    v.menu.close();
                    cx.notify();
                }
            }))
            // A janela inteira aceita soltar uma fonte (Finder): realce durante o
            // arrasto (borda por cima de tudo, sem mudar o layout) e o Cmd-O ao soltar.
            .drag_over::<ExternalPaths>(|s, _, _, _| s.bg(rgb(crate::theme::current().drop_background)))
            .on_drop(cx.listener(|v, p: &ExternalPaths, window, cx| {
                let _ = v.drop_item(p.paths(), window, cx);
            }));
        // Itens de menu da view: so os que se aplicam agora ficam ligados.
        let state_menu = self.state_menu(window, cx);
        let mut root_dir = crate::register_in_view(root_dir, &state_menu, cx);
        if !window.is_fullscreen() {
            root_dir = root_dir.child(self.title_bar(window));
        }
        if !cfg!(target_os = "macos") {
            root_dir = root_dir.child(self.render_menu_bar(window, cx));
            if self.menu.open_at().is_some() {
                root_dir = root_dir.child(self.menu_background(window.is_fullscreen(), cx));
            }
        }
        if self.state.pending().is_some() {
            root_dir = root_dir.child(self.source_warning(cx));
        }
        root_dir = root_dir.child(middle).child(self.footer(cx)).child(self.status_bar(view_window, cx));
        if self.open_help {
            root_dir = root_dir.child(self.help_screen(cx));
        }
        if self.config_open {
            root_dir = root_dir.child(self.screen_config(cx));
        }
        if self.picker.is_open() {
            // Ancorado na janela (nao na area da imagem, que pode ser estreita).
            root_dir = root_dir.child(self.vault_picker(view_window, cx));
        }
        if cx.has_active_drag() {
            root_dir = root_dir.child(drop_highlight());
        }
        if self.drawer_drag {
            root_dir = root_dir.child(self.drawer_drag_thread(cx));
        }
        if self.scroll_drag.is_some() {
            root_dir = root_dir.child(self.scroll_drag_thread(cx));
        }
        if self.bar_drag.is_some() {
            // Fora do painel: soltar o mouse vale mesmo se o painel fechar no meio (F).
            root_dir = root_dir.child(self.drag_thread(cx));
        }
        root_dir
    }
}
