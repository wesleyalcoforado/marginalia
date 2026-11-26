//! Agendador dos ajustes de imagem, sem gpui (regra R4; relogio injetado como
//! em `quick_note_session`: cada chamada recebe o `current_time`).
//!
//! A view conta o que o usuario faz ([`ScheduleAdjustment::changed`] a cada valor novo
//! da barra ou tecla, [`ScheduleAdjustment::dropped`] ao soltar a barra,
//! [`ScheduleAdjustment::window_changed`] quando a regiao visivel muda) e executa as
//! [`MenuAction`]s que [`ScheduleAdjustment::tick`] devolve:
//! - No maximo UM filtro em voo (tile ou base). Enquanto ele roda, as mudancas
//!   so trocam o pendente: o proximo pedido leva o ULTIMO valor e a ULTIMA
//!   janela, nunca os intermediarios. A view avisa o fim por
//!   [`ScheduleAdjustment::finished`] (so o `work` certo libera o lugar).
//! - Cada valor novo pede um tile (a janela visivel, na resolucao da tela) ja. A
//!   base (pagina inteira reduzida, so fundo) sai [`AFTER_BASE`] depois da ultima
//!   mudanca, ou ja no `dropped`, mas SO depois de um tile desse valor terminar sem
//!   ser interrompido: a base borrada nunca aparece na frente durante o arrasto
//!   (com o tile dos mesmos valores na tela, o estado a guarda so como fundo,
//!   `Received::Layer`). Ajuste neutro nao tem base: o tile sai como a textura
//!   cheia (o estado decide).
//! - Um valor novo NAO interrompe o tile em voo (regra R2: um tile e curto, e
//!   interrompe-lo a cada movimento congelaria a tela durante o arrasto): ele
//!   chega e mostra o valor dele, e o proximo leva so o ultimo valor. Um valor
//!   novo interrompe ja uma BASE em voo (fundo de um valor superado) e, na
//!   passagem de/para o neutro, tambem o tile ([`MenuAction::Interrupt`]: a view
//!   abre uma geracao nova, o filtro para no proximo bloco de linhas e o
//!   resultado cai); o interrompido ainda conta como em voo ate o `finished`.
//! - Janela nova: o tile sai [`AFTER_WINDOW`] depois da ULTIMA janela (pan/zoom
//!   continuos nao enfileiram pedidos nem gravam nada). Um TILE em voo da janela
//!   velha e interrompido ja (geracao nova; com um so filtro em voo nao ha base
//!   dos mesmos valores a derrubar). Uma base em voo segue: ela nao depende da
//!   janela, e o tile da janela nova sai quando ela terminar. Janela igual a
//!   vigente nao gera trabalho; sem ajuste na tela a janela so e guardada. A
//!   PRIMEIRA janela informada numa imagem pede o tile ja (sem os 60 ms).
//! - Base adiantada ([`ScheduleAdjustment::base_missing`]): janela nova com o tile na tela
//!   sem base (o tile de um valor novo tirou a base velha e a nova ainda nao
//!   veio). A base do valor atual sai assim que o filtro em voo terminar, antes
//!   do tile da janela nova e sem esperar um tile do valor ficar pronto; sem
//!   janela nova, vale a ordem tile-antes-da-base acima.
//! - A base que chega a tela com o valor atual ([`ScheduleAdjustment::screen_base`],
//!   p.ex. a da carga do quadro) dispensa a base que esperava o primeiro tile.
//! - Um tile que o estado derrubou por janela velha ([`ScheduleAdjustment::fell_tile`])
//!   e pedido de novo ja, com a janela vigente.
//!   So o tile da agenda que caiu conta como derrubado; a carga do quadro que
//!   cai so adianta o pedido.
//! - Um tile que termina com o valor, a janela e o olho atuais encerra o tile
//!   pendente igual (A -> B -> A no arrasto nao refiltra o mesmo tile).
//! - O olho ([`ScheduleAdjustment::refilter`]) interrompe o filtro em voo (levava o
//!   estado velho do olho) e pede o tile e, com o olho ligado, logo a base. A
//!   view avisa o olho ([`ScheduleAdjustment::set_eye`]); religado por um valor
//!   novo, a textura sem filtro em voo e interrompida.
//! - Renomear a fonte ([`ScheduleAdjustment::renamed_source`]) so troca a chave: a
//!   mesma imagem segue na tela e nada pendente cai.
//! - O appdata e gravado [`WRITE_AFTER`] depois da ultima mudanca, ou ja no
//!   `dropped` ([`MenuAction::Write`], no fundo). Cada gravacao leva a imagem para a
//!   qual o valor foi escolhido.
//! - Trocar de imagem/pagina ([`ScheduleAdjustment::switched_image`]) descarta o tile, a
//!   base e a janela pendentes; a gravacao pendente da imagem anterior sai na
//!   hora, com a chave DELA. Com ajuste guardado na imagem nova, a carga do
//!   quadro ja traz a base; se ela nao vier com o valor, a base sai depois do
//!   primeiro tile da agenda nela.
//! - Um valor igual ao ultimo nao gera trabalho.

use std::time::{Duration, Instant};

use marginalia_core::filters::Adjustments;
use marginalia_core::model::ImageRef;

use crate::load::ViewWindow;

/// Sem mudanca de valor por este tempo (e com o tile do valor pronto), a base e pedida.
pub const AFTER_BASE: Duration = Duration::from_millis(200);
/// Sem mudanca por este tempo, o appdata e gravado.
pub const WRITE_AFTER: Duration = Duration::from_millis(500);
/// Sem janela nova por este tempo, o tile da ultima janela e pedido.
pub const AFTER_WINDOW: Duration = Duration::from_millis(60);

/// O que a view faz depois de [`ScheduleAdjustment::tick`].
#[derive(Clone, Debug, PartialEq)]
pub enum MenuAction {
    /// Filtro na janela visivel (`view_window`: a ultima informada; `None` so na
    /// textura sem filtro: um tile filtrado espera a primeira janela). Ao terminar (ou se nao houver o que filtrar):
    /// [`ScheduleAdjustment::finished`] com `work`.
    RequestTile { work: u64, image_adjustments: Adjustments, view_window: Option<ViewWindow> },
    /// Filtro na pagina inteira reduzida (camada de fundo). Ao terminar:
    /// [`ScheduleAdjustment::finished`].
    RequestBase { work: u64, image_adjustments: Adjustments },
    /// O filtro em voo `work` ficou superado (valor novo, janela nova, olho):
    /// a view o torna obsoleto agora (geracao nova; o filtro para no proximo
    /// bloco de linhas). Ele continua contando como em voo ate o
    /// [`ScheduleAdjustment::finished`] dele.
    Interrupt {
        #[cfg_attr(not(test), allow(dead_code))]
        work: u64,
    },
    /// Gravar o appdata (no fundo, gravacao atomica): `image_adjustments` de `image` ja
    /// estao na memoria (a view os pos la a cada mudanca); os campos dizem o
    /// que esta gravacao leva (testes e depuracao), a view so precisa do momento.
    Write {
        #[cfg_attr(not(test), allow(dead_code))]
        image: ImageRef,
        #[cfg_attr(not(test), allow(dead_code))]
        image_adjustments: Adjustments,
    },
}

/// O tipo do filtro em voo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Flight {
    #[default]
    Tile,
    Base,
}

#[derive(Debug, Default)]
pub struct ScheduleAdjustment {
    /// Imagem dos ajustes atuais (`None`: nenhuma ainda).
    image: Option<ImageRef>,
    /// Ultimo valor conhecido (o que o usuario escolheu por ultimo).
    current: Adjustments,
    /// Ultima janela visivel informada nesta imagem.
    view_window: Option<ViewWindow>,
    /// O tile de `current` na `view_window` foi pedido para este instante (ainda nao saiu).
    tile_at: Option<Instant>,
    /// A base de `current` foi pedida para este instante (sai so com `ready_tile`).
    base_at: Option<Instant>,
    /// Um tile de `current` terminou sem ser interrompido: a base pode sair.
    ready_tile: bool,
    /// Imagem nova com ajuste: a base sai logo depois do primeiro tile pronto.
    base_after_tile: bool,
    /// Pan com o tile sem base na tela ([`ScheduleAdjustment::base_missing`]): a base sai
    /// antes do tile e sem esperar `ready_tile`.
    base_ja: bool,
    /// A gravacao de `current` vence neste instante (ainda nao saiu).
    write_at: Option<Instant>,
    /// Numero do filtro em voo.
    in_flight: Option<u64>,
    /// O filtro em voo e desta imagem e ainda nao foi interrompido.
    in_flight_here: bool,
    /// O que esta em voo (so vale com `in_flight`).
    in_flight_kind: Flight,
    /// Os ajustes que o filtro em voo leva (so vale com `in_flight`).
    in_flight_adjustments: Adjustments,
    /// A janela que o filtro em voo leva (so vale com `in_flight`).
    in_flight_window: Option<ViewWindow>,
    /// O filtro em voo filtra (a tela mostrava ajustes quando ele saiu); `false`:
    /// a textura cheia sem filtro (ajuste neutro ou o olho desligado).
    in_flight_filters: bool,
    /// O olho do painel esta desligado (a tela mostra a imagem sem ajustes).
    disabled_eye: bool,
    /// Interrupcao a devolver no proximo `tick`.
    interrupt: Option<u64>,
    /// Ultimo numero de trabalho dado.
    jobs: u64,
}

impl ScheduleAdjustment {
    pub fn new() -> ScheduleAdjustment {
        ScheduleAdjustment::default()
    }

    /// Imagem a que os ajustes atuais pertencem.
    pub fn image(&self) -> Option<&ImageRef> {
        self.image.as_ref()
    }

    /// A imagem atual passou a ser `image`, com os ajustes guardados `value`.
    /// Na mesma imagem nada muda. Em outra: tile, base e janela pendentes caem
    /// (o filtro em voo, se houver, termina obsoleto e ainda conta como em voo)
    /// e a gravacao pendente da imagem anterior sai agora, com a chave dela.
    pub fn switched_image(&mut self, image: ImageRef, value: Adjustments) -> Vec<MenuAction> {
        if self.image.as_ref() == Some(&image) {
            return Vec::new();
        }
        let actions: Vec<MenuAction> = self.unload().into_iter().collect();
        // O estado ja invalidou o filtro da imagem anterior (geracao nova).
        self.in_flight_here = false;
        self.interrupt = None;
        self.view_window = None;
        self.ready_tile = false;
        self.base_ja = false;
        // A carga do quadro traz a base; se ela nao chegar com este valor
        // (`screen_base`), a base vem depois do primeiro tile da agenda.
        self.base_after_tile = !value.is_neutral();
        self.image = Some(image);
        self.current = value;
        actions
    }

    /// A fonte `old` foi renomeada para `new`: so a chave da imagem muda (a
    /// mesma imagem segue na tela), entao nada pendente nem em voo cai.
    pub fn renamed_source(&mut self, old: &str, new: &str) {
        if let Some(i) = self.image.as_mut().filter(|i| i.source == old) {
            i.source = new.to_string();
        }
    }

    /// O usuario escolheu `value` (arrastando a barra, tecla, botao). Devolve se
    /// ha trabalho novo (um valor igual ao ultimo nao gera nenhum).
    pub fn changed(&mut self, value: Adjustments, current_time: Instant) -> bool {
        if self.image.is_none() || value == self.current {
            return false;
        }
        // O tile sai ja; a base (nada a filtrar no neutro) depois de parar e do tile pronto.
        self.tile_at = Some(current_time);
        self.ready_tile = false;
        self.base_after_tile = false;
        self.base_ja = false;
        self.base_at = (!value.is_neutral()).then(|| current_time + AFTER_BASE);
        self.write_at = Some(current_time + WRITE_AFTER);
        // R2: um valor novo NAO interrompe o tile em voo (curto; interrompe-lo a
        // cada movimento congelaria a tela no arrasto): o proximo leva o ultimo
        // valor. A base em voo (fundo de um valor superado) para ja, e tambem o
        // que esta em voo quando o que a tela pede muda de tipo (textura cheia
        // sem filtro x tile filtrado: passar de/para o neutro, ou o olho religado).
        self.current = value;
        if self.in_flight_kind == Flight::Base || self.in_flight_filters != self.filters() {
            self.interrupt_in_flight();
        }
        true
    }

    /// A tela mostra ajustes para o valor atual (o proximo pedido filtra).
    fn filters(&self) -> bool {
        !self.disabled_eye && !self.current.is_neutral()
    }

    /// O tile pendente filtra mas a imagem ainda nao tem janela informada (ela
    /// carrega): ele espera o primeiro `window_changed` (sem janela seria a pagina
    /// inteira em resolucao cheia). A textura sem filtro nao espera.
    fn tile_wait_window(&self) -> bool {
        self.view_window.is_none() && self.filters()
    }

    /// O prazo do tile pendente, se ele ja pode sair (nao espera a janela).
    fn tile_deadline(&self) -> Option<Instant> {
        self.tile_at.filter(|_| !self.tile_wait_window())
    }

    /// O olho do painel passou a `enabled` (a view avisa sempre que o muda, tambem
    /// quando mexer num ajuste o religa). So guarda: o refiltro e
    /// [`ScheduleAdjustment::refilter`], e o valor novo que o religou ([`ScheduleAdjustment::changed`])
    /// interrompe a textura sem filtro que estiver em voo.
    pub fn set_eye(&mut self, enabled: bool) {
        self.disabled_eye = !enabled;
    }

    /// O estado grampeou a janela guardada na pagina (ela chegou depois da
    /// janela, ou a janela ficou vazia nela e foi esquecida): a agenda passa a
    /// guardar a mesma, para a mesma janela informada de novo nao gerar trabalho.
    pub fn normalized_window(&mut self, view_window: Option<ViewWindow>) {
        if self.image.is_none() || self.view_window == view_window {
            return;
        }
        if self.in_flight.is_some() && self.in_flight_window == self.view_window {
            self.in_flight_window = view_window;
        }
        self.view_window = view_window;
    }

    /// A regiao visivel passou a ser `view_window` (`filter`: a tela mostra ajustes;
    /// sem eles a janela so e guardada para os proximos tiles). Devolve se ha
    /// trabalho novo.
    pub fn window_changed(&mut self, view_window: ViewWindow, current_time: Instant, filter: bool) -> bool {
        if self.image.is_none() || self.view_window == Some(view_window) {
            return false;
        }
        // A primeira janela informada nesta imagem: nenhum tile dela na tela ainda.
        let first = self.view_window.is_none();
        self.view_window = Some(view_window);
        if !filter || self.current.is_neutral() {
            return false;
        }
        // O tile em voo e da janela velha (a base em voo nao depende da janela).
        if self.in_flight_kind == Flight::Tile {
            self.interrupt_in_flight();
        }
        // So a ultima janela conta: o prazo recomeca a cada uma. A primeira da
        // imagem sai ja (nao ha rajada a esperar e a tela so tem a base).
        self.tile_at = Some(if first { current_time } else { current_time + AFTER_WINDOW });
        true
    }

    /// Chamar logo depois de um [`ScheduleAdjustment::window_changed`] que devolveu
    /// `true` quando o tile na tela esta sem base (o tile de um valor novo tirou a
    /// base do valor velho e a dele ainda nao veio): o pan expoe area que ficaria
    /// escura ate a base. A base do valor atual sai ja, ANTES do tile da janela
    /// nova e sem esperar um tile do valor ficar pronto (a janela mudou: o tile em
    /// voo era da janela velha e ja foi interrompido). Ela ainda espera o filtro em
    /// voo terminar (um por vez), e uma base do valor atual ja em voo basta. Um
    /// valor novo, o olho ou outra imagem desfazem o adiantamento (volta a ordem
    /// tile-antes-da-base). Sem ajuste na tela: nada.
    pub fn base_missing(&mut self, current_time: Instant) {
        if self.image.is_none() || !self.filters() {
            return;
        }
        if self.in_flight.is_some() && self.in_flight_here && self.in_flight_kind == Flight::Base && self.in_flight_adjustments == self.current {
            return;
        }
        self.base_ja = true;
        self.base_at = Some(self.base_at.map_or(current_time, |p| p.min(current_time)));
    }

    /// Uma base de `value` chegou a tela (a carga do quadro tambem traz uma): se
    /// e o valor atual, a base que esperava o primeiro tile da imagem (ou a
    /// adiantada pelo pan) nao e pedida de novo.
    pub fn screen_base(&mut self, value: &Adjustments) {
        if self.image.is_some() && *value == self.current {
            self.base_after_tile = false;
            if std::mem::take(&mut self.base_ja) {
                self.base_at = None;
            }
        }
    }

    /// O estado derrubou um tile que chegou porque a janela dele deixou de valer
    /// (`Received::WindowChanged`): o da janela vigente sai ja. `work`: o tile
    /// da agenda que caiu (ele nao conta como pronto para a base); `None`: a carga
    /// do quadro (navegacao), que a agenda nao pediu e que nao toca o tile em voo.
    pub fn fell_tile(&mut self, current_time: Instant, work: Option<u64>) {
        if self.image.is_none() {
            return;
        }
        if work.is_some() && self.in_flight == work && self.in_flight_kind == Flight::Tile {
            self.in_flight_here = false;
        }
        self.tile_at = Some(self.tile_at.map_or(current_time, |p| p.min(current_time)));
    }

    /// Interrompe o filtro em voo desta imagem (uma vez): sai no proximo `tick`.
    fn interrupt_in_flight(&mut self) {
        if std::mem::take(&mut self.in_flight_here) {
            self.interrupt = self.in_flight;
        }
    }

    /// Refiltrar o valor atual sem mudar nada (o olho do painel liga/desliga
    /// os ajustes na tela): o tile e, com `preview` (a tela vai mostrar ajustes),
    /// logo a base. O filtro em voo levava o estado velho do olho: e
    /// interrompido. Nunca grava. Sem imagem: `false`.
    pub fn refilter(&mut self, current_time: Instant, preview: bool) -> bool {
        if self.image.is_none() {
            return false;
        }
        self.disabled_eye = !preview;
        self.interrupt_in_flight();
        self.tile_at = Some(current_time);
        self.ready_tile = false;
        self.base_after_tile = false;
        self.base_ja = false;
        self.base_at = (preview && !self.current.is_neutral()).then_some(current_time);
        true
    }

    /// O usuario soltou a barra (ou a tecla ja e um passo inteiro): a base e a
    /// gravacao pendentes vencem agora (a base ainda espera o tile do valor).
    pub fn dropped(&mut self, current_time: Instant) {
        for deadline in [&mut self.base_at, &mut self.write_at] {
            if let Some(p) = deadline.as_mut() {
                *p = (*p).min(current_time);
            }
        }
    }

    /// Prazo da base, se ela ja pode sair (o tile do valor esta pronto).
    fn base_deadline(&self, current_time: Instant) -> Option<Instant> {
        if self.base_ja {
            return self.base_at;
        }
        if !self.ready_tile {
            return None;
        }
        match (self.base_at, self.base_after_tile) {
            (Some(p), _) => Some(p),
            (None, true) => Some(current_time),
            (None, false) => None,
        }
    }

    /// O que fazer em `current_time`: no maximo um pedido de filtro (se nada esta em
    /// voo; o tile antes da base) e a gravacao vencida.
    pub fn tick(&mut self, current_time: Instant) -> Vec<MenuAction> {
        let mut actions = Vec::new();
        if let Some(work) = self.interrupt.take() {
            actions.push(MenuAction::Interrupt { work });
        }
        if self.in_flight.is_none() {
            let image_adjustments = self.current.clone();
            let base = self.base_deadline(current_time).is_some_and(|p| p <= current_time);
            let tile = self.tile_deadline().is_some_and(|p| p <= current_time);
            // O tile antes da base, salvo a base adiantada pelo pan (`base_missing`).
            if tile && !(base && self.base_ja) {
                self.tile_at = None;
                let work = self.new_work(Flight::Tile);
                actions.push(MenuAction::RequestTile { work, image_adjustments, view_window: self.view_window });
            } else if base {
                self.base_at = None;
                self.base_after_tile = false;
                self.base_ja = false;
                actions.push(MenuAction::RequestBase { work: self.new_work(Flight::Base), image_adjustments });
            }
        }
        if self.write_at.is_some_and(|p| p <= current_time) {
            actions.extend(self.unload_write());
        }
        actions
    }

    fn new_work(&mut self, kind: Flight) -> u64 {
        self.jobs += 1;
        self.in_flight = Some(self.jobs);
        self.in_flight_here = true;
        self.in_flight_kind = kind;
        self.in_flight_adjustments = self.current.clone();
        self.in_flight_window = self.view_window;
        self.in_flight_filters = self.filters();
        self.jobs
    }

    fn unload_write(&mut self) -> Option<MenuAction> {
        self.write_at.take()?;
        let image = self.image.clone()?;
        Some(MenuAction::Write { image, image_adjustments: self.current.clone() })
    }

    /// O filtro `work` terminou (chegou, foi descartado ou nem saiu). Um
    /// numero que nao e o do filtro em voo nao muda nada. Um tile do valor atual
    /// que terminou sem ser interrompido (a janela dele ainda vale): libera a
    /// base, e um tile pendente igual a ele (A -> B -> A no arrasto, mesma janela)
    /// cai: seria o mesmo filtro de novo. Um tile de um valor anterior (o arrasto
    /// seguiu) nao libera.
    pub fn finished(&mut self, work: u64) {
        if self.in_flight == Some(work) {
            if self.in_flight_here && self.in_flight_kind == Flight::Tile && self.in_flight_adjustments == self.current {
                self.ready_tile = true;
                if self.in_flight_window == self.view_window && self.in_flight_filters == self.filters() {
                    self.tile_at = None;
                }
            }
            if self.in_flight_here && self.in_flight_kind == Flight::Base && self.in_flight_adjustments == self.current {
                // A base do valor atual veio: nada mais a adiantar.
                self.base_ja = false;
            }
            self.in_flight = None;
            self.in_flight_here = false;
        }
    }

    /// Daqui a quanto chamar [`ScheduleAdjustment::tick`] de novo (`None`: so quando
    /// algo acontecer: mudanca, soltura, janela ou fim do filtro em voo).
    pub fn next_tick(&self, current_time: Instant) -> Option<Duration> {
        if self.interrupt.is_some() {
            return Some(Duration::ZERO);
        }
        let filter = match self.in_flight {
            Some(_) => None,
            None => [self.tile_deadline(), self.base_deadline(current_time)].into_iter().flatten().min(),
        };
        [filter, self.write_at].into_iter().flatten().min().map(|p| p.saturating_duration_since(current_time))
    }

    /// Ha um tile desta imagem em voo, ainda valendo (R2: um valor novo nao o
    /// interrompe; uma janela nova, o olho ou a troca de imagem sim).
    pub fn tile_in_flight(&self) -> bool {
        self.in_flight.is_some() && self.in_flight_here && self.in_flight_kind == Flight::Tile
    }

    /// Nada pendente nem em voo (a base que so espera o primeiro tile de uma
    /// imagem nova e o tile que espera a primeira janela nao contam: nada os pede
    /// ainda; a janela os pede quando o visualizador a informar).
    pub fn idle(&self) -> bool {
        // A base do valor sai so depois de um tile dele: com o tile esperando a
        // janela, ela tambem espera.
        let blocked_base = self.tile_wait_window() && !self.ready_tile && !self.base_ja;
        let base = self.base_at.is_some() && !blocked_base;
        self.in_flight.is_none() && self.tile_deadline().is_none() && !base && self.write_at.is_none()
    }

    /// Sair do app / fechar a janela: a gravacao pendente sai agora (a view a
    /// executa na hora, sem esperar o fundo). Tile e base pendentes caem.
    pub fn unload(&mut self) -> Option<MenuAction> {
        self.tile_at = None;
        self.base_at = None;
        self.base_after_tile = false;
        self.base_ja = false;
        self.unload_write()
    }
}

#[cfg(test)]
#[path = "tests/schedule_adjustment.rs"]
mod tests;
