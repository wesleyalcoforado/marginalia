//! Exibicao: ajustes, rotacao, janela, camadas na tela e a chegada das respostas (`receive`).

use super::*;

impl AppState {
    pub(super) fn new_generation(&mut self) -> u64 {
        self.generation.fresh()
    }

    /// Troca os ajustes do quadro atual (na memoria e nos dados do app, por
    /// imagem) e pede a reaplicacao dos filtros (sem reler a fonte se a pagina
    /// ja chegou). Nao grava o disco: isso e da [`WriteAppData`], no fundo.
    #[cfg_attr(not(test), allow(dead_code))] // a view passa pelo `schedule_adjustment`
    pub fn set_adjustments(&mut self, a: Adjustments) -> Option<Request> {
        self.set_adjustments_in(a, Quality::Tile)
    }

    /// A gravacao dos ajustes no fundo terminou: falha vira aviso; sucesso tira
    /// o aviso de falha anterior (so ele: outro aviso, mais novo, fica).
    pub fn wrote_adjustments(&mut self, r: Result<(), String>) {
        match r {
            Ok(()) if self.warning.as_deref().is_some_and(|a| a.starts_with(&warning_adjustments_start())) => self.warning = None,
            Ok(()) => {}
            Err(e) => self.warning = Some(t!("warning.image_adjustments", e)),
        }
    }

    /// O usuario escolheu `a` para o quadro atual: vale ja na memoria (painel,
    /// menus, a proxima tecla) e nos dados do app em memoria. Nao pede filtro
    /// nem grava o disco (ver `schedule_adjustment`). Sem quadro: nada.
    pub fn apply_adjustments(&mut self, a: Adjustments) {
        if self.prefetcher.is_none() || self.current.image.file.is_empty() {
            return;
        }
        self.app_data.set_adjustments(&self.current.image, a.clone());
        self.adjustments = a;
    }

    /// Como [`set_adjustments`](Self::set_adjustments), na qualidade pedida: a
    /// base (pagina inteira reduzida) ou o tile (a janela visivel, [`set_window`](Self::set_janela);
    /// sem janela informada o tile sai como base). Um pedido de base ou tile dos
    /// mesmos ajustes do ultimo pedido de base ou tile reusa a geracao dele
    /// enquanto o tile dessa geracao nao chegou (os dois valem; `receive` nunca
    /// deixa a base cobrir o tile da mesma geracao); outro valor ganha geracao
    /// nova. Sem ajuste nao ha base nem tile: o pedido sai como cheia; com
    /// ajuste, um pedido de cheia sai como tile.
    ///
    /// Com o olho desligado ([`set_show_adjustments`](Self::definir_mostrar_ajustes))
    /// `a` fica guardado do mesmo jeito, mas o pedido leva a imagem sem ajustes.
    pub fn set_adjustments_in(&mut self, a: Adjustments, quality: Quality) -> Option<Request> {
        let prefetcher = self.prefetcher.clone()?;
        if self.current.image.file.is_empty() {
            return None;
        }
        let screen = self.in_screen(&a);
        let quality = match quality {
            _ if screen.is_neutral() => Quality::Full,
            // Sem janela informada (a imagem ainda carrega) um tile seria a pagina
            // inteira em resolucao cheia: sai a base (rapida, a pagina toda).
            Quality::Full | Quality::Tile if self.view_window.is_none() => Quality::Base,
            Quality::Full => Quality::Tile,
            q => q,
        };
        let reuses = quality != Quality::Full
            && self.request_adjustment.as_ref().is_some_and(|(g, v)| *g == self.generation.current() && *v == screen)
            && self.displayed_tile != Some(self.generation.current());
        self.apply_adjustments(a);
        if self.error.is_some() {
            // Nada a filtrar; os ajustes ficam guardados para quando a imagem abrir.
            return None;
        }
        if !reuses {
            self.new_generation();
        }
        if quality != Quality::Full {
            self.request_adjustment = Some((self.generation.current(), screen.clone()));
        }
        self.loading = true;
        Some(Request {
            generation: self.generation.current(),
            target: self.current.target(),
            decoded: self.page.clone(),
            image_adjustments: screen,
            view_window: if quality == Quality::Tile { self.view_window } else { None },
            quality,
            // Se o quadro ainda nao chegou, este pedido o substitui: herda o enquadramento dele.
            frame: self.frame_pending.clone().unwrap_or(Reframe::Keep),
            rotation: self.rotation,
            prefetcher,
            effective: self.generation.effective(),
        })
    }

    /// Rotacao de exibicao do quadro atual.
    pub fn rotation(&self) -> Rotation {
        self.rotation
    }

    /// O usuario girou o quadro atual para `fresh`: vale ja na memoria e nos dados
    /// do app (por imagem; a gravacao no fundo e da view). As camadas na tela (base
    /// e tile) estao na orientacao velha: saem, e a geracao nova derruba o que
    /// estava em voo. Nao pede nada: a view pede a cheia (sem ajuste na tela) ou
    /// refiltra pela agenda. `false`: sem quadro, ou a mesma rotacao.
    pub fn rotate(&mut self, fresh: Rotation) -> bool {
        if self.prefetcher.is_none() || self.current.image.file.is_empty() || fresh == self.rotation {
            return false;
        }
        self.rotation = fresh;
        self.app_data.set_rotation(&self.current.image, fresh);
        self.clear_layers();
        self.displayed_base = None;
        self.displayed_tile = None;
        self.request_adjustment = None;
        self.new_generation();
        true
    }

    /// O filtro em voo ficou superado (a agenda decide: uma janela nova para o
    /// tile, um valor novo para a base, o olho, a passagem de/para o neutro; um
    /// valor novo nao interrompe o tile, regra R2): ele fica obsoleto agora (o
    /// `stop` dele vira verdadeiro no proximo bloco de linhas) e o resultado,
    /// se chegar, e ignorado. O proximo pedido abre uma geracao nova.
    pub fn interrupt_filter(&mut self) {
        self.new_generation();
    }

    /// O olho do painel esta ligado (a tela mostra os ajustes).
    pub fn show_adjustments(&self) -> bool {
        self.show_adjustments
    }

    /// Liga/desliga o olho do painel (antes/depois). Nao apaga nem grava nada:
    /// so os proximos pedidos (este quadro refiltrado pela view, a proxima
    /// imagem) levam ou nao os ajustes.
    pub fn set_show_adjustments(&mut self, show: bool) {
        self.show_adjustments = show;
    }

    /// Os ajustes da imagem que esta na tela agora (`None`: nada na tela).
    pub fn screen_adjustments(&self) -> Option<&Adjustments> {
        self.screen_adjustments.as_ref()
    }

    /// Guarda a janela visivel do quadro atual (grade e retangulo em pixels da
    /// grade, ver [`ViewWindow`]); os proximos tiles a usam, e um tile de outra janela
    /// que chegar depois cai em `receive`. Com a pagina ja decodificada, a janela e
    /// grampeada nela. Janela vazia (area sem tamanho, fora da pagina) e ignorada.
    /// Nao pede nada nem muda a geracao (a agenda interrompe o tile em voo da
    /// janela velha). `true`: a janela mudou.
    pub fn set_window(&mut self, j: ViewWindow) -> bool {
        let j = match &self.page {
            Some(p) => j.normalized(p.decoded.image.dimensions()),
            None => j,
        };
        if j.empty() || self.view_window == Some(j) {
            return false;
        }
        self.view_window = Some(j);
        true
    }

    /// O tile da janela vigente com os ajustes atuais (depois de
    /// [`Received::WindowChanged`]); reusa a geracao se os valores sao os mesmos do
    /// ultimo pedido (nao derruba a base em voo). `None`: sem ajuste na tela (nada
    /// a filtrar), sem quadro ou quadro com erro. A view pede pela agenda
    /// (`ScheduleAdjustment::fell_tile`), que mantem um filtro em voo por vez.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn re_request_tile(&mut self) -> Option<Request> {
        if self.in_screen(&self.adjustments).is_neutral() {
            return None;
        }
        self.set_adjustments_in(self.adjustments.clone(), Quality::Tile)
    }

    /// A tela mostra ajustes no quadro atual (ajuste ativo e o olho ligado): ha
    /// tile e base a filtrar quando a janela muda.
    pub fn screen_filters(&self) -> bool {
        !self.in_screen(&self.adjustments).is_neutral()
    }

    /// Tamanho da pagina decodificada do quadro atual (`None`: ainda nao chegou).
    pub fn page_sz(&self) -> Option<(u32, u32)> {
        self.page.as_ref().map(|p| p.decoded.image.dimensions())
    }

    /// A janela visivel informada para o quadro atual (`None`: ainda nenhuma).
    pub fn view_window(&self) -> Option<ViewWindow> {
        self.view_window
    }

    /// O tile na tela (a textura e a janela que cobre), se o quadro tem ajuste.
    /// O visualizador mantem a camada dele so enquanto esta e `Some`.
    pub fn tile_display(&self) -> Option<(LayerInTheScreen, ViewWindow)> {
        self.tile.as_ref().map(|(c, j, _)| (*c, *j))
    }

    /// A base na tela (pagina inteira reduzida), se o quadro tem ajuste. Sempre
    /// dos mesmos ajustes do tile, quando os dois estao na tela.
    pub fn base_display(&self) -> Option<LayerInTheScreen> {
        self.base.as_ref().map(|(c, _)| *c)
    }

    /// Os ajustes da base na tela (`None`: nenhuma base).
    pub fn base_adjustments(&self) -> Option<&Adjustments> {
        self.base.as_ref().map(|(_, a)| a)
    }

    /// Ha um tile na tela sem base dos mesmos valores por baixo (o tile de um
    /// valor novo tirou a base velha): um pan exporia area sem nada.
    pub fn tile_no_base(&self) -> bool {
        self.tile.as_ref().is_some_and(|(_, _, t)| self.base.as_ref().is_none_or(|(_, b)| b != t))
    }

    /// Numera a camada que vai para a tela com os pixels de `e`.
    pub(super) fn new_layer(&mut self, e: &Display) -> LayerInTheScreen {
        self.layers_total += 1;
        LayerInTheScreen { sz: e.bgra.dimensions(), number: self.layers_total }
    }

    pub(super) fn clear_layers(&mut self) {
        self.tile = None;
        self.base = None;
    }

    /// As ultimas imagens aceitas para a tela (a mais nova no fim) e quantas
    /// foram aceitas na sessao.
    pub fn displays(&self) -> (&std::collections::VecDeque<Displayed>, u64) {
        (&self.displays, self.displays_total)
    }

    /// O que a tela pede para `a`: ele mesmo com o olho ligado, nada com ele desligado.
    pub(super) fn in_screen(&self, a: &Adjustments) -> Adjustments {
        if self.show_adjustments { a.clone() } else { Adjustments::default() }
    }

    /// Aplica a resposta se ela for a mais recente. Uma base so entra se a
    /// geracao dela ainda vale e o tile dessa geracao ainda nao chegou; um tile
    /// so entra se a geracao E a janela dele ainda valem (a informada, ou a
    /// pagina inteira enquanto nenhuma foi informada). Tile e base na tela ficam
    /// sempre dos mesmos ajustes: a camada de outro valor sai.
    pub fn receive(&mut self, r: Response) -> Received {
        if r.generation != self.generation.current() {
            return Received::Ignored;
        }
        match r.outcome {
            Outcome::Stale => Received::Ignored,
            Outcome::Ready { page, display } => {
                let sz = page.decoded.image.dimensions();
                // Uma janela informada antes de a pagina decodificar e grampeada nela agora.
                self.view_window = self.view_window.map(|j| j.normalized(sz)).filter(|j| !j.empty());
                let current_window = self.view_window.unwrap_or_else(|| ViewWindow::whole_page(sz));
                if r.quality == Quality::Tile && r.view_window != Some(current_window) {
                    // Nada mais vem para este pedido: o quadro nao fica "carregando".
                    self.loading = false;
                    return Received::WindowChanged;
                }
                let equal_tile = self.tile.as_ref().is_some_and(|(_, _, a)| *a == r.image_adjustments);
                if r.quality == Quality::Base && (self.displayed_tile == Some(r.generation) || equal_tile) {
                    // Um tile dos mesmos valores ja esta na tela (desta geracao, ou de
                    // uma anterior quando a base foi pedida depois dele): a base so vai
                    // para o fundo, sem tocar a tela, o tile nem o enquadramento. Nada
                    // mais vem para este pedido: o quadro nao fica "carregando".
                    self.loading = false;
                    self.base = Some((self.new_layer(&display), r.image_adjustments));
                    if !self.page.as_ref().is_some_and(|p| Arc::ptr_eq(&p.decoded, &page.decoded)) {
                        self.page = Some(page);
                    }
                    return Received::Layer(display);
                }
                if self.pending.as_ref().is_some_and(|p| p.name == self.current.image.source) {
                    self.pending = None;
                }
                let frame = match r.quality {
                    Quality::Base => {
                        self.displayed_base = Some(r.generation);
                        r.frame
                    }
                    Quality::Tile | Quality::Full => {
                        self.displayed_tile = Some(r.generation);
                        // A base desta geracao ja enquadrou: nao desfazer o zoom de agora.
                        if self.displayed_base == Some(r.generation) { Reframe::Keep } else { r.frame }
                    }
                };
                let layer = match r.quality {
                    Quality::Base => {
                        if self.tile.as_ref().is_some_and(|(_, _, a)| *a != r.image_adjustments) {
                            self.tile = None;
                        }
                        self.base = Some((self.new_layer(&display), r.image_adjustments.clone()));
                        Layer::Base
                    }
                    Quality::Tile => {
                        if self.base.as_ref().is_some_and(|(_, a)| *a != r.image_adjustments) {
                            self.base = None;
                        }
                        self.tile = Some((self.new_layer(&display), current_window, r.image_adjustments.clone()));
                        Layer::Tile(current_window)
                    }
                    // Sem ajuste: so a textura cheia, sem camadas.
                    Quality::Full => {
                        self.clear_layers();
                        Layer::Full
                    }
                };
                self.total_pages = Some(page.page_count());
                // Base ou tile da mesma pagina decodificada: fica a `Page` guardada,
                // que ja pode ter a copia reduzida da base e o recorte do tile (nao refazer).
                let same = |p: &Arc<Page>| Arc::ptr_eq(&p.decoded, &page.decoded);
                if !self.page.as_ref().is_some_and(same) {
                    self.page = Some(page);
                }
                self.error = None;
                self.displayed = true;
                self.loading = false;
                self.frame_pending = None;
                self.displays_total += 1;
                if self.displays.len() == MAX_DISPLAYS {
                    self.displays.pop_front();
                }
                self.displays.push_back(Displayed { number: self.displays_total, quality: r.quality, image_adjustments: r.image_adjustments.clone() });
                self.screen_adjustments = Some(r.image_adjustments);
                Received::Image(display, frame, layer)
            }
            Outcome::Error(m) => {
                self.page = None;
                self.clear_layers();
                self.displayed = false;
                self.error = Some(m.clone());
                self.loading = false;
                self.frame_pending = None;
                Received::Error(m)
            }
            Outcome::Unavailable(m) => {
                self.page = None;
                self.clear_layers();
                self.displayed = false;
                self.error = Some(m.clone());
                self.loading = false;
                self.frame_pending = None;
                self.nav_pending = None;
                let name = self.current.image.source.clone();
                let file_path = self.path_of(&name);
                let mut p = Pending::fresh(&name, file_path, Reason::Unavailable);
                p.detail = Some(m.clone());
                // O quadro que falhou, no enquadramento que ele ia ter.
                p.image_frame = Some(Frame { image: self.current.image.clone(), framing: Some(self.fr.clone()) });
                p.highlight = self.annot.emphasis.clone();
                p.note = self.note_open.as_ref().map(|(id, _)| id.clone());
                self.pending = Some(p);
                Received::Unavailable(m)
            }
        }
    }

}
