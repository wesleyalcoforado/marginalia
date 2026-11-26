//! O documento em edicao ([`Item`]) e o que a sessao decide fazer com ele.
use super::*;

/// Documento em edicao.
pub(super) struct Item {
    pub(super) serial: u64,
    /// Vault em que o item comecou: TODA gravacao dele vai para ca.
    pub(super) vault: Vault,
    pub(super) kind: DocKind,
    /// `None`: ainda nao criado.
    pub(super) id: Option<String>,
    /// Para criar (itens novos).
    pub(super) ctx: Option<Context>,
    /// Criado pela caixa (corpo como `route_quick_note_input`); carregado: corpo = caixa.
    pub(super) new: bool,
    pub(super) textbox: String,
    /// Corpo que esta no disco.
    pub(super) written: Option<String>,
    pub(super) pinned: bool,
    /// Fixar o id assim que houver texto (sair da caixa).
    pub(super) pin: bool,
    /// Gravar ja, sem esperar o debounce.
    pub(super) urgent: bool,
    /// Terminar o item: gravar, fixar e soltar.
    pub(super) end: bool,
    /// A caixa foi esvaziada com o item ainda rascunho: apagar o rascunho.
    pub(super) erase: bool,
    pub(super) last_key: Instant,
    /// Desde quando a caixa tem texto que o disco nao tem (para a [`MAX_WAIT`]).
    pub(super) since_dirty: Option<Instant>,
    /// O documento sumiu do vault: nada mais e gravado nele.
    pub(super) lost: bool,
    pub(super) created: Option<Created>,
    pub(super) error: Option<String>,
    pub(super) attempt_at: Option<Instant>,
    pub(super) failures: u32,
    /// Esta no registro de recuperacao com esta chave.
    pub(super) key: Option<String>,
    /// Restaurado sem id: procurar um rascunho igual (e conferir o grifo) antes de criar.
    pub(super) search: bool,
    /// Restaurado: os corpos que o disco pode ter desde a saida (so grava por
    /// cima de um deles; lista vazia: so se o disco ja tem o texto).
    pub(super) expected: Option<Vec<String>>,
    /// Corpos de trabalhos enviados e ainda nao confirmados: um deles pode
    /// chegar ao disco depois de um `flush` estourado (vao para o registro).
    pub(super) in_flight_bodies: Vec<String>,
    /// O disco mudou por fora desde a saida: espera o usuario (recriar ou descartar).
    pub(super) conflict: bool,
}

/// O item espera o usuario: sumiu do vault, ou o disco mudou por fora.
pub(super) fn stopped(it: &Item) -> bool {
    it.lost || it.conflict
}

/// Nota do registro tentando de novo um vault que nao responde (talvez de vez):
/// o usuario pode recria-la no quadro atual ou descarta-la.
pub(super) fn vault_waiting(it: &Item) -> bool {
    !stopped(it) && it.key.is_some() && it.error.is_some()
}

impl Item {
    pub(super) fn note_body(&self) -> String {
        if self.new {
            new_body(self.kind, &self.textbox)
        } else {
            self.textbox.clone()
        }
    }

    pub(super) fn dirty(&self) -> bool {
        self.written.as_deref() != Some(self.note_body().as_str())
    }

    /// O texto ja esta no vault (o que falta, fixar o id, nao perde nada).
    pub(super) fn safe(&self) -> bool {
        self.erase || (self.id.is_some() && !self.lost && !self.dirty())
    }

    pub(super) fn record(&self, key: &str) -> Record {
        let ctx = self.ctx.as_ref();
        Record {
            key: key.to_string(),
            vault: self.vault.root.clone(),
            kind: kind_name(self.kind).to_string(),
            id: self.id.clone(),
            text: self.note_body(),
            image: ctx.map(|c| c.image.clone()),
            framing: ctx.map(|c| c.framing.clone()),
            highlight: ctx.and_then(|c| c.highlight.clone()),
            lost: self.lost,
            no_disk: self.no_disk(),
        }
    }

    /// Corpos que o disco pode ter: o confirmado (ou os do registro restaurado)
    /// e os dos trabalhos em voo.
    pub(super) fn no_disk(&self) -> Vec<String> {
        let mut v: Vec<String> = match &self.expected {
            Some(x) => x.clone(),
            None => self.written.iter().cloned().collect(),
        };
        for c in &self.in_flight_bodies {
            if !v.contains(c) {
                v.push(c.clone());
            }
        }
        v
    }

    pub(super) fn new_item(serial: u64, vault: Vault, kind: DocKind, ctx: Option<Context>, textbox: String, current_time: Instant) -> Item {
        Item {
            serial,
            vault,
            kind,
            id: None,
            ctx,
            new: true,
            textbox,
            written: None,
            pinned: false,
            pin: false,
            urgent: false,
            end: false,
            erase: false,
            last_key: current_time,
            since_dirty: Some(current_time),
            lost: false,
            created: None,
            error: None,
            attempt_at: None,
            failures: 0,
            key: None,
            search: false,
            expected: None,
            in_flight_bodies: Vec::new(),
            conflict: false,
        }
    }
}

/// O corpo que este passo leva ao disco fica anotado no item ate o resultado
/// voltar (ultimos 3: so o trabalho que segurava o vault no prazo pode chegar).
pub(super) fn annotate_in_flight(it: &mut Item, p: &Step) {
    let note_body = match p {
        Step::Create { note_body, .. } | Step::Write { note_body: Some(note_body), .. } => note_body,
        _ => return,
    };
    if !it.in_flight_bodies.contains(note_body) {
        it.in_flight_bodies.push(note_body.clone());
        if it.in_flight_bodies.len() > 3 {
            it.in_flight_bodies.remove(0);
        }
    }
}

/// O que um item quer fazer agora.
pub(super) enum MenuAction {
    Work(Step),
    Finished,
    Wait(Duration),
    Nothing,
}

pub(super) fn menu_action(it: &mut Item, current_time: Instant, rhythm: QuickNoteRhythm) -> MenuAction {
    let (debounce, wait_max) = (rhythm_wait(rhythm), rhythm_max_wait(rhythm));
    if stopped(it) {
        return MenuAction::Nothing;
    }
    if let Some(t) = it.attempt_at.filter(|t| current_time < *t) {
        return MenuAction::Wait(t - current_time);
    }
    it.attempt_at = None;
    if it.erase {
        return match it.id.clone() {
            // Nunca chegou ao disco: nada a apagar.
            None => MenuAction::Finished,
            Some(id) => MenuAction::Work(Step::Erase { id }),
        };
    }
    let note_body = it.note_body();
    let dirty = it.dirty();
    let idle = current_time.saturating_duration_since(it.last_key);
    if it.pin && !text_has(&note_body) {
        // Nada a fixar ainda (o id nao mudaria): so grava.
        it.pin = false;
    }
    // Sem letra nem digito o id nao muda: um rascunho vazio (citado) fica rascunho.
    let pin = !it.pinned && text_has(&note_body) && (it.end || it.pin || idle >= PIN_AFTER);
    let max_wait = it.since_dirty.is_some_and(|d| current_time.saturating_duration_since(d) >= wait_max);
    let write = dirty && (it.end || pin || it.urgent || idle >= debounce || max_wait);
    match it.id.clone() {
        None => match it.ctx.clone() {
            Some(ctx) => MenuAction::Work(Step::Create {
                note_body,
                ctx,
                pin,
                search: it.search,
                known_all: it.expected.clone().unwrap_or_default(),
            }),
            None => MenuAction::Nothing,
        },
        Some(id) if write || pin => {
            let expected = it.expected.clone().filter(|_| dirty);
            MenuAction::Work(Step::Write { id, note_body: dirty.then_some(note_body), pin, expected })
        }
        Some(_) if it.end => MenuAction::Finished,
        Some(_) => {
            let mut deadlines = Vec::new();
            if dirty {
                deadlines.push(it.last_key + debounce);
                if let Some(d) = it.since_dirty {
                    deadlines.push(d + wait_max);
                }
            }
            if !it.pinned && text_has(&note_body) {
                deadlines.push(it.last_key + PIN_AFTER);
            }
            match deadlines.into_iter().min() {
                Some(p) => MenuAction::Wait(p.saturating_duration_since(current_time)),
                None => MenuAction::Nothing,
            }
        }
    }
}

pub(super) struct Resumed {
    pub(super) vault: Vault,
    pub(super) kind: DocKind,
    pub(super) old: String,
}

pub(super) struct Load {
    pub(super) vault: Vault,
    pub(super) kind: DocKind,
    pub(super) id: String,
    pub(super) generation: u64,
}

pub(super) enum InFlight {
    Item(u64),
    Resumed(Resumed),
    Load(Load),
}

/// Onde esta um item: o da caixa ou um dos que estao terminando.
#[derive(Clone, Copy)]
pub(super) enum Where {
    Current,
    Leaving(usize),
}
