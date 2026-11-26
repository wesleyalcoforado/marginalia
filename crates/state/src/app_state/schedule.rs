//! O que a agenda de ajustes (`schedule_adjustment`) faz a cada resposta que chega e
//! a cada quadro novo: a ligacao entre o estado e a agenda, sem gpui.

use std::time::Instant;

use super::*;
use crate::schedule_adjustment::{MenuAction as AdjustmentAction, ScheduleAdjustment};

/// O que a view precisa fazer depois que a agenda seguiu o quadro atual.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Followed {
    /// A agenda passou a outra imagem/pagina: um arrasto de barra em curso nao
    /// atravessa a troca (o proximo movimento levaria o valor para a imagem nova).
    pub switched: bool,
    /// A troca deixou uma gravacao dos ajustes da imagem anterior para sair ja.
    pub write: bool,
}

/// O que a view faz depois de uma chamada que mexeu na agenda.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScheduleAfter {
    pub followed: Followed,
    /// A agenda tem trabalho para ja: a view roda `schedule_tick`.
    pub tick: bool,
}

/// O que a view faz depois de girar o quadro e o visualizador informar a janela nova.
pub enum RotationAfter {
    /// Com ajuste na tela: a agenda refiltra (`tick` diz se ha o que pedir ja).
    Refilter { tick: bool },
    /// Sem ajuste na tela: a textura cheia girada sai direto, sem a agenda.
    Full(Request),
    /// Nada a pedir.
    Nothing,
}

/// Um passo que a view executa depois de [`AppState::schedule_tick`].
#[derive(Clone, Debug, PartialEq)]
pub enum ScheduleStep {
    /// Filtrar no fundo: [`AppState::request_schedule`] com estes valores (a view
    /// roda o pedido e avisa a agenda do fim com `ScheduleAdjustment::finished`).
    Filter { work: u64, image_adjustments: Adjustments, quality: Quality },
    /// Gravar os dados do app no fundo.
    Write,
}

impl AppState {
    /// Ha um quadro (fonte aberta e arquivo escolhido) para ajustar, girar, filtrar.
    pub fn frame_has(&self) -> bool {
        self.source_has() && !self.current.image.file.is_empty()
    }

    /// A camada de ajuste que o visualizador mantem quando chega uma imagem em
    /// `c`: so a que o estado ainda tem na tela (nunca uma base de outro valor
    /// sob o tile novo, nem um tile velho sobre a base nova).
    pub fn keep_other_layer(&self, c: Layer) -> bool {
        match c {
            Layer::Tile(_) => self.base_display().is_some(),
            Layer::Base => self.tile_display().is_some(),
            Layer::Full => false,
        }
    }

    /// A agenda segue o quadro atual: em outra imagem/pagina (ou a fonte
    /// renomeada) os pendentes da anterior caem e a gravacao dela sai ja, com a
    /// chave dela. Roda antes de qualquer tick: um filtro da imagem anterior que
    /// termina depois da troca nunca pede o valor antigo na imagem nova.
    pub fn schedule_follow_image(&self, schedule: &mut ScheduleAdjustment) -> Followed {
        if !self.frame_has() || schedule.image() == Some(&self.current.image) {
            return Followed::default();
        }
        let actions = schedule.switched_image(self.current.image.clone(), self.adjustments.clone());
        Followed { switched: true, write: actions.iter().any(|a| matches!(a, AdjustmentAction::Write { .. })) }
    }

    /// A agenda depois de uma resposta (`received`, que o estado ja aceitou).
    /// `work`: o filtro da agenda que a trouxe (`None`: a carga do quadro).
    /// Uma base na tela segue a imagem e registra os valores dela; a janela
    /// guardada antes da pagina e grampeada; o tile derrubado pela janela volta
    /// para a agenda pedir de novo.
    pub fn schedule_after_receive(&self, schedule: &mut ScheduleAdjustment, received: &Received, work: Option<u64>, current_time: Instant) -> Followed {
        let mut followed = Followed::default();
        if matches!(received, Received::Image(_, _, Layer::Base) | Received::Layer(_)) {
            followed = self.schedule_follow_image(schedule);
            if let Some(a) = self.base_adjustments() {
                schedule.screen_base(a);
            }
        }
        // A pagina chegou: a janela guardada antes dela foi grampeada (ou esquecida).
        if schedule.image() == Some(&self.current.image) {
            schedule.normalized_window(self.view_window());
        }
        if matches!(received, Received::WindowChanged) {
            schedule.fell_tile(current_time, work);
        }
        followed
    }

    /// O que a agenda pede em `current_time`. A janela do tile e a interrupcao do
    /// filtro superado (geracao nova: o resultado dele cai em `receive`) ja
    /// foram aplicadas aqui; sobram o filtro e a gravacao, para a view rodar.
    pub fn schedule_tick(&mut self, schedule: &mut ScheduleAdjustment, current_time: Instant) -> Vec<ScheduleStep> {
        let mut steps = Vec::new();
        for a in schedule.tick(current_time) {
            match a {
                AdjustmentAction::RequestTile { work, image_adjustments, view_window } => {
                    if let Some(j) = view_window {
                        // A mesma que a view ja deu ao estado (no-op); defesa.
                        self.set_window(j);
                    }
                    steps.push(ScheduleStep::Filter { work, image_adjustments, quality: Quality::Tile });
                }
                AdjustmentAction::RequestBase { work, image_adjustments } => {
                    steps.push(ScheduleStep::Filter { work, image_adjustments, quality: Quality::Base });
                }
                AdjustmentAction::Write { .. } => steps.push(ScheduleStep::Write),
                AdjustmentAction::Interrupt { .. } => self.interrupt_filter(),
            }
        }
        steps
    }

    /// O pedido de carga do filtro `work` da agenda. `None`: nada a filtrar
    /// (a agenda nao esta nesta imagem, ou o estado nao tem o que pedir) e o
    /// trabalho ja foi dado como terminado na agenda.
    pub fn request_schedule(&mut self, schedule: &mut ScheduleAdjustment, work: u64, image_adjustments: Adjustments, q: Quality) -> Option<Request> {
        // Nunca o valor de uma imagem em outra (a agenda ja seguiu o quadro; defesa).
        let request = (schedule.image() == Some(&self.current.image)).then(|| self.set_adjustments_in(image_adjustments, q)).flatten();
        if request.is_none() {
            schedule.finished(work);
        }
        request
    }

    /// A regiao visivel do quadro passou a ser `j` (pan, zoom, roda, redimensionar,
    /// imagem nova). O estado a guarda (um tile de outra janela que chegar cai); a
    /// agenda interrompe o tile em voo da janela velha e pede o da nova depois de
    /// a janela parar.
    pub fn schedule_window_changed(&mut self, schedule: &mut ScheduleAdjustment, j: ViewWindow, current_time: Instant) -> ScheduleAfter {
        let followed = self.schedule_follow_image(schedule);
        let mut r = ScheduleAfter { followed, tick: false };
        if !self.frame_has() || !self.set_window(j) {
            return r;
        }
        let Some(j) = self.view_window() else { return r };
        if schedule.window_changed(j, current_time, self.screen_filters()) {
            if self.tile_no_base() {
                // O pan exporia area escura ate a base do valor: ela sai ja.
                schedule.base_missing(current_time);
            }
            r.tick = true;
        }
        r
    }

    /// O usuario escolheu `a` para o quadro: vale ja na memoria (painel, menus); o
    /// tile sai pela agenda, a base e a gravacao depois de parar. Mexer num ajuste
    /// com o olho desligado volta a mostrar os ajustes (o "antes" e so para comparar).
    pub fn schedule_adjustment_changed(&mut self, schedule: &mut ScheduleAdjustment, a: Adjustments, current_time: Instant) -> ScheduleAfter {
        let followed = self.schedule_follow_image(schedule);
        let mut r = ScheduleAfter { followed, tick: false };
        if !self.frame_has() {
            return r;
        }
        if !self.show_adjustments() && a != self.adjustments {
            self.set_show_adjustments(true);
            schedule.set_eye(true);
        }
        self.apply_adjustments(a.clone());
        r.tick = schedule.changed(a, current_time);
        r
    }

    /// O olho do cabecalho "Ajustes": liga/desliga a previa dos ajustes. Nada e
    /// apagado nem gravado; o quadro atual e refiltrado pela agenda.
    pub fn schedule_toggle_preview(&mut self, schedule: &mut ScheduleAdjustment, current_time: Instant) -> ScheduleAfter {
        let show = !self.show_adjustments();
        self.set_show_adjustments(show);
        schedule.set_eye(show);
        let followed = self.schedule_follow_image(schedule);
        let tick = self.frame_has() && !self.adjustments.is_neutral() && schedule.refilter(current_time, show);
        ScheduleAfter { followed, tick }
    }

    /// Gira o quadro atual 90 graus (`clockwise`: para a direita). `Some(fresh)`: o
    /// estado girou (memoria e dados do app, camadas fora) e a view gira o
    /// visualizador e chama `schedule_after_rotate`; `None`: sem quadro, com
    /// erro, ou nada mudou.
    pub fn rotate_frame(&mut self, schedule: &mut ScheduleAdjustment, clockwise: bool) -> (Followed, Option<Rotation>) {
        let followed = self.schedule_follow_image(schedule);
        if !self.frame_has() || self.error.is_some() {
            return (followed, None);
        }
        let current = self.rotation();
        let fresh = if clockwise { current.clockwise() } else { current.anti_clockwise() };
        (followed, self.rotate(fresh).then_some(fresh))
    }

    /// O visualizador girou e informou a janela visivel nova (`view_window`): ela ja
    /// vale para o estado e a agenda antes do pedido. Com ajuste na tela a agenda
    /// refiltra (a janela nova primeiro, como no pan); sem, a textura cheia girada.
    pub fn schedule_after_rotate(&mut self, schedule: &mut ScheduleAdjustment, view_window: Option<ViewWindow>, current_time: Instant) -> RotationAfter {
        if let Some(j) = view_window {
            self.set_window(j);
        }
        let filters = self.screen_filters();
        if let Some(j) = self.view_window() {
            schedule.window_changed(j, current_time, filters);
        }
        if filters {
            return RotationAfter::Refilter { tick: schedule.refilter(current_time, true) };
        }
        match self.set_adjustments_in(self.adjustments.clone(), Quality::Full) {
            Some(p) => RotationAfter::Full(p),
            None => RotationAfter::Nothing,
        }
    }
}
