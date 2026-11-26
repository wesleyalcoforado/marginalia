//! O [`Work`] (um passo no vault, fora da thread da interface) e as funcoes de disco que ele roda.
use super::*;

/// Um passo de gravacao/leitura no vault, para rodar FORA da thread da interface.
pub struct Work {
    pub(super) seq: u64,
    pub(super) vault: Vault,
    pub(super) kind: DocKind,
    pub(super) step: Step,
    pub(super) now: NaiveDateTime,
    pub(super) queue: Arc<Mutex<Queue>>,
    pub(super) delay: Option<Delay>,
}

pub(super) enum Step {
    /// Cria o rascunho com `note_body` (e fixa o id se `pin`).
    /// `search` (restaurado sem id): antes, procura o documento que um
    /// trabalho atrasado pode ter deixado (corpo em `known_all`, ou rascunho
    /// com um comeco do texto) e confere se o grifo ainda esta livre.
    Create { note_body: String, ctx: Context, pin: bool, search: bool, known_all: Vec<String> },
    /// Grava `note_body` (se houver) em `id` e fixa o id se `pin`. `expected`
    /// (restaurado): so grava se o disco tem um desses corpos (ou ja `note_body`).
    Write { id: String, note_body: Option<String>, pin: bool, expected: Option<Vec<String>> },
    /// Apaga o rascunho `id` se nada o cita (senao so esvazia o corpo).
    Erase { id: String },
    /// Termina uma fixacao parada (`finalizar_id_em(tipo, antigo)`).
    Resume { old: String },
    /// Le o corpo de `id` para editar (terminando antes uma fixacao parada).
    Load { id: String },
}

/// Resultado de um [`Work`] (volta por [`QuickNoteSession::conclude`]).
#[derive(Clone, Debug, Default)]
pub struct Done {
    pub(super) seq: u64,
    /// O trabalho foi cancelado antes de comecar (o `flush` cuidou dele).
    pub(super) cancelled: bool,
    pub(super) root_dir: PathBuf,
    pub(super) created: Option<Created>,
    /// Id do documento depois do passo.
    pub(super) id: Option<String>,
    /// Corpo que agora esta no disco.
    pub(super) written: Option<String>,
    pub(super) pinned: bool,
    /// O rascunho foi apagado (ou ja nao existia).
    pub(super) erased: bool,
    /// Pedido para apagar, mas citado (ou ja nao rascunho): so ficou com o corpo vazio.
    pub(super) emptied: bool,
    pub(super) renames: Vec<(String, String)>,
    pub(super) error: Option<String>,
    /// O documento sumiu do vault (nem a retomada o acha).
    pub(super) lost: bool,
    /// Corpo lido e se o documento ainda e rascunho.
    pub(super) loaded: Option<(String, bool)>,
    /// O disco tem um texto que nem a sessao nem o registro conhecem: nada foi gravado.
    pub(super) conflict: bool,
    /// O grifo do registro ja e de outra nota: criado sem ele.
    pub(super) no_highlight: bool,
    /// Restaurado sem id: adotou o documento que um trabalho atrasado deixou (`written` e o corpo dele).
    pub(super) adopted: bool,
}

/// Trabalho permitido agora e o resultado do ultimo que rodou.
#[derive(Default)]
pub(super) struct Queue {
    pub(super) valid: u64,
    pub(super) done: Option<Done>,
}

pub(super) fn lock(f: &Mutex<Queue>) -> MutexGuard<'_, Queue> {
    f.lock().unwrap_or_else(|e| e.into_inner())
}

impl Work {
    /// Roda o passo. BLOQUEIA (vault, talvez sincronizado ou no NAS): rode no
    /// executor de fundo. Um trabalho cancelado (pelo `flush`) nao faz nada.
    pub fn execute(self) -> Done {
        if let Some(a) = &self.delay {
            a();
        }
        let queue = Arc::clone(&self.queue);
        let mut g = lock(&queue);
        if g.valid != self.seq {
            return Done { seq: self.seq, cancelled: true, ..Done::default() };
        }
        let f = self.run();
        g.done = Some(f.clone());
        f
    }

    pub(super) fn run(self) -> Done {
        let mut f = Done { seq: self.seq, root_dir: self.vault.root.clone(), ..Done::default() };
        let (v, kind, now) = (&self.vault, self.kind, self.now);
        if !v.root.join("notas").is_dir() || !v.root.join(folder(kind)).is_dir() {
            // Vault desmontado (NAS: o ponto de montagem vazio continua la) ou
            // apagado: nunca criar as pastas no disco local; tenta mais tarde.
            f.error = Some(t!("error.quick_note.unavailable_vault", v.root.display()));
            return f;
        }
        match self.step {
            Step::Create { note_body, mut ctx, pin, search, known_all } => {
                if search {
                    // Restaurado sem id: a criacao (talvez com um corpo mais velho)
                    // pode ter chegado ao disco depois do prazo, e ate fixado o id.
                    if let Some((id, no_disk, draft)) = doc_left(v, kind, &ctx, &note_body, &known_all) {
                        // O texto do registro e gravado (e o id fixado) no passo seguinte,
                        // que compara de novo com este corpo antes de gravar.
                        f.written = Some(no_disk);
                        f.adopted = true;
                        f.id = Some(id);
                        f.pinned = !draft;
                        return f;
                    }
                    if ctx.highlight.as_deref().is_some_and(|g| highlight_in_use(v, g)) {
                        ctx.highlight = None;
                        f.no_highlight = true;
                    }
                }
                let r = match kind {
                    DocKind::Task => v.create_task_draft(&note_body, Some(ctx.image), None, now).map(Created::Task),
                    _ => v.create_draft(&note_body, ctx.image, ctx.framing, ctx.highlight, now).map(Created::Note),
                };
                match r {
                    Ok(c) => {
                        let id = id_of(&c).to_string();
                        f.created = Some(c);
                        f.written = Some(note_body);
                        f.id = Some(id.clone());
                        if pin {
                            pin_at(v, kind, id, now, &mut f);
                        }
                    }
                    Err(e) => f.error = Some(e.to_string()),
                }
            }
            Step::Write { id, note_body, pin, expected } => {
                let mut id = id;
                if let (Some(c), Some(x)) = (&note_body, &expected) {
                    // Restaurado: compara antes de gravar. O disco mudou desde a saida
                    // (editado por fora)? Nao sobrescreve. Nunca grava as cegas.
                    match read_doc(v, kind, &id) {
                        Ok((_, current)) if current != *c && !x.contains(&current) => {
                            f.id = Some(id);
                            f.conflict = true;
                            return f;
                        }
                        Ok(_) | Err(Error::NotFound(_)) => {}
                        Err(e) => {
                            f.id = Some(id);
                            f.error = Some(t!("error.quick_note.read_before_of_write", e));
                            return f;
                        }
                    }
                }
                if let Some(c) = note_body {
                    let r = write(v, kind, &mut id, &c, now, &mut f);
                    f.id = Some(id.clone());
                    match r {
                        Ok(()) => f.written = Some(c),
                        Err(e) => {
                            f.error = Some(e.to_string());
                            return f;
                        }
                    }
                }
                f.id = Some(id.clone());
                if pin && !f.pinned {
                    pin_at(v, kind, id, now, &mut f);
                }
            }
            Step::Erase { id } => erase_at(v, kind, &id, &mut f),
            Step::Resume { old } => match v.finish_id_in(kind, &old, now) {
                Ok(n) => {
                    if n != old {
                        f.renames.push((old, n.clone()));
                    }
                    f.id = Some(n);
                    f.pinned = true;
                }
                // `old` ja nao existe: alguem (a propria sessao, outra janela)
                // terminou a renomeacao. Nada a fazer; a gaveta rele.
                Err(Error::NotFound(_)) => f.lost = true,
                Err(e) => f.error = Some(e.to_string()),
            },
            Step::Load { id } => load_at(v, kind, id, now, &mut f),
        }
        f
    }
}

/// Grava `note_body` em `id`; resolve o id que mudou por fora (`NotFound`) e
/// a fixacao em curso de outro documento antes de gravar de novo.
pub(super) fn write(v: &Vault, kind: DocKind, id: &mut String, note_body: &str, now: NaiveDateTime, f: &mut Done) -> Result<(), Error> {
    match v.update_text_in(kind, id, note_body) {
        Err(Error::NotFound(_)) => match v.finish_id_in(kind, id, now) {
            // A fixacao de `id` terminou (ou parou depois de apagar o antigo): grava no novo.
            Ok(n) => {
                if n != *id {
                    f.renames.push((id.clone(), n.clone()));
                }
                *id = n;
                f.pinned = true;
                v.update_text_in(kind, id, note_body)
            }
            Err(Error::NotFound(m)) => {
                f.lost = true;
                Err(Error::NotFound(m))
            }
            Err(e) => Err(e),
        },
        Err(e) => match in_progress(&e) {
            Some(a) => {
                // `id` e a continuacao de `a`, que ainda esta no disco: termina `a`
                // (o conteudo dele vai para `id`) e so entao grava o texto novo.
                v.finish_id_in(kind, &a, now)?;
                v.update_text_in(kind, id, note_body)
            }
            None => Err(e),
        },
        ok => ok,
    }
}

/// `finalizar_id_em(tipo, id)`, anotando o id novo em `f`.
pub(super) fn pin_at(v: &Vault, kind: DocKind, id: String, now: NaiveDateTime, f: &mut Done) {
    match v.finish_id_in(kind, &id, now) {
        Ok(n) => {
            if n != id {
                f.renames.push((id, n.clone()));
            }
            f.id = Some(n);
            f.pinned = true;
        }
        Err(e) => {
            f.lost = matches!(e, Error::NotFound(_));
            f.error = Some(e.to_string());
        }
    }
}

/// Apaga o rascunho `id` (pela API com tipo). So apaga um documento que
/// ainda e rascunho e que nenhuma tarefa ou nota cita; senao so esvazia o corpo.
pub(super) fn erase_at(v: &Vault, kind: DocKind, id: &str, f: &mut Done) {
    f.id = Some(id.to_string());
    let extra = match read_doc(v, kind, id) {
        Ok((x, _)) => x,
        Err(Error::NotFound(_)) => {
            f.erased = true;
            return;
        }
        Err(e) => {
            f.error = Some(e.to_string());
            return;
        }
    };
    let draft = extra.get("draft").and_then(|x| x.as_bool()) == Some(true);
    if !draft || cited(v, id) {
        match v.update_text_in(kind, id, "") {
            Ok(()) => {
                f.written = Some(String::new());
                f.emptied = true;
            }
            Err(e) => f.error = Some(e.to_string()),
        }
        return;
    }
    let r = match kind {
        DocKind::Note => v.delete_note(id),
        DocKind::Task => v.delete_task(id),
    };
    match r {
        Ok(()) => f.erased = true,
        Err(e) => f.error = Some(e.to_string()),
    }
}

/// Alguma tarefa (`note`) cita `id`. Documento ilegivel e pulado.
pub(super) fn cited(v: &Vault, id: &str) -> bool {
    let docs = |folder: &str| -> Vec<String> {
        std::fs::read_dir(v.root.join(folder))
            .map(|d| d.flatten().filter_map(|e| std::fs::read_to_string(e.path()).ok()).collect())
            .unwrap_or_default()
    };
    docs("tarefas").iter().any(|d| parse::<Task>(d).is_ok_and(|(t, _)| t.note.as_deref() == Some(id)))
}

/// Documentos legiveis de `<folder>/` (texto cru), pulando temporarios ocultos.
pub(super) fn docs_of(v: &Vault, folder: &str) -> Vec<String> {
    std::fs::read_dir(v.root.join(folder))
        .map(|d| {
            d.flatten()
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .filter_map(|e| std::fs::read_to_string(e.path()).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// O documento desta imagem que um trabalho da sessao anterior deixou para
/// `text`: corpo igual a um dos `known_all` (enviados e talvez chegados
/// depois do prazo, mesmo com o id ja fixado), ou um rascunho (marca ou id
/// provisorio) cujo corpo e um comeco do texto. Devolve id, corpo e se e rascunho.
///
/// So adota o que e provavelmente a propria nota: mesma imagem e (notas)
/// mesmo grifo e mesmo enquadramento; um documento com id fixado so se o
/// corpo e conhecido E o id saiu desse corpo (tarefa fixada nunca). Mais de um
/// candidato: nenhum (cria nova, nunca sobrescreve a nota de outra).
pub(super) fn doc_left(v: &Vault, kind: DocKind, ctx: &Context, text: &str, known_all: &[String]) -> Option<(String, String, bool)> {
    let is_draft = |x: &Extras, id: &str| x.get("draft").and_then(|b| b.as_bool()) == Some(true) || id.contains("-sem-texto");
    let known = |c: &str| known_all.iter().any(|k| k == c);
    let serves = |id: &str, x: &Extras, c: &str, pinned_can: bool| {
        if is_draft(x, id) {
            known(c) || (!c.trim().is_empty() && text.starts_with(c))
        } else {
            pinned_can && known(c) && id_left_of(id, c)
        }
    };
    let candidates: Vec<(String, String, bool)> = docs_of(v, folder(kind))
        .iter()
        .filter_map(|d| match kind {
            DocKind::Task => parse::<Task>(d)
                .ok()
                .filter(|(t, c)| t.image.as_ref() == Some(&ctx.image) && serves(&t.id, &t.extra, c, false))
                .map(|(t, c)| (t.id.clone(), c, true)),
            _ => parse::<Note>(d)
                .ok()
                .filter(|(p, c)| {
                    p.image == ctx.image && p.highlight == ctx.highlight && p.framing == ctx.framing && serves(&p.id, &p.extra, c, true)
                })
                .map(|(p, c)| (p.id.clone(), c.clone(), is_draft(&p.extra, &p.id))),
        })
        .collect();
    match <[_; 1]>::try_from(candidates) {
        Ok([um]) => Some(um),
        Err(_) => None,
    }
}

/// O id `AAAAMMDD-HHMM-<slug>[-n]` foi feito deste corpo.
pub(super) fn id_left_of(id: &str, note_body: &str) -> bool {
    let s = marginalia_core::ids::slug(note_body);
    let Some(rest) = id.get(14..) else { return false };
    rest == s || rest.strip_prefix(s.as_str()).and_then(|r| r.strip_prefix('-')).is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()))
}

/// Alguma nota ja esta ligada ao grifo `g`.
pub(super) fn highlight_in_use(v: &Vault, g: &str) -> bool {
    docs_of(v, "notas").iter().any(|d| parse::<Note>(d).is_ok_and(|(p, _)| p.highlight.as_deref() == Some(g)))
}

/// O erro e "renomeacao em curso; finalize A": devolve `A`.
pub(super) fn in_progress(e: &Error) -> Option<String> {
    match e {
        Error::StateInvalid(m) => m.strip_prefix(t!("error.vault.renaming_in_progress", "").as_str()).map(str::to_string),
        _ => None,
    }
}

pub(super) fn folder(kind: DocKind) -> &'static str {
    match kind {
        DocKind::Note => "notas",
        DocKind::Task => "tarefas",
    }
}

pub(super) fn kind_of(s: &str) -> Option<DocKind> {
    match s {
        // "pista" e o nome antigo, de registros de recuperacao gravados antes do renome
        "note" | "nota" | "pista" => Some(DocKind::Note),
        "task" | "tarefa" => Some(DocKind::Task),
        _ => None,
    }
}

pub(super) fn kind_name(t: DocKind) -> &'static str {
    match t {
        DocKind::Note => "note",
        DocKind::Task => "task",
    }
}

/// Extras e corpo de `<pasta de tipo>/<id>.md`.
pub(super) fn read_doc(v: &Vault, kind: DocKind, id: &str) -> Result<(Extras, String), Error> {
    if id.is_empty() || id.contains(['/', '\\']) {
        return Err(Error::NotFound(id.to_string()));
    }
    let path = v.root.join(folder(kind)).join(format!("{id}.md"));
    let doc = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Error::NotFound(id.to_string())),
        Err(e) => return Err(Error::Io(e)),
    };
    let r = match kind {
        DocKind::Note => parse::<Note>(&doc).map(|(m, b)| (m.extra, b)),
        DocKind::Task => parse::<Task>(&doc).map(|(m, b)| (m.extra, b)),
    };
    r.map_err(|e| Error::Document(path, Box::new(e)))
}

/// Le `id` para editar. Documento que e a continuacao de uma fixacao parada
/// (`renomeando_de: A`): termina a de A antes (nunca se edita N antes disso).
pub(super) fn load_at(v: &Vault, kind: DocKind, id: String, now: NaiveDateTime, f: &mut Done) {
    let mut id = id;
    let (mut extra, mut note_body) = match read_doc(v, kind, &id) {
        Ok(x) => x,
        Err(e) => {
            f.error = Some(e.to_string());
            return;
        }
    };
    let old = extra.get("renaming_from").and_then(|x| x.as_str()).filter(|a| *a != id).map(str::to_string);
    if let Some(a) = old {
        match v.finish_id_in(kind, &a, now) {
            Ok(n) => {
                f.renames.push((a, n.clone()));
                id = n;
                match read_doc(v, kind, &id) {
                    Ok((e2, c2)) => (extra, note_body) = (e2, c2),
                    Err(e) => {
                        f.error = Some(e.to_string());
                        return;
                    }
                }
            }
            Err(e) => {
                f.error = Some(t!("error.quick_note.stopped_renaming", a, e));
                return;
            }
        }
    }
    let draft = extra.get("draft").and_then(|x| x.as_bool()) == Some(true);
    f.id = Some(id);
    f.loaded = Some((note_body, draft));
}

pub(super) fn id_of(c: &Created) -> &str {
    match c {
        Created::Note(p) => &p.id,
        Created::Task(t) => &t.id,
    }
}

/// O id fixado vale tambem no que o Cmd+Enter devolve (sem a marca de rascunho).
pub(super) fn with_id(c: &mut Created, id: &str, pinned: bool) {
    let extra = match c {
        Created::Note(p) => {
            p.id = id.to_string();
            &mut p.extra
        }
        Created::Task(t) => {
            t.id = id.to_string();
            &mut t.extra
        }
    };
    if pinned {
        extra.shift_remove("draft");
    }
}

/// Tipo do item que `text` cria, ou `None` se ainda nao ha o que criar
/// (vazio, so espacos, so `[`, so `[]`).
pub(super) fn new_kind(text: &str) -> Option<DocKind> {
    let t = text.trim();
    if t.is_empty() || t == "[" {
        return None;
    }
    match t.strip_prefix("[]") {
        Some(r) if r.trim().is_empty() => None,
        Some(_) => Some(DocKind::Task),
        None => Some(DocKind::Note),
    }
}

/// Corpo de um item criado pela caixa (como `route_quick_note_input`).
pub(super) fn new_body(kind: DocKind, text: &str) -> String {
    let t = text.trim();
    match kind {
        DocKind::Task => t.strip_prefix("[]").unwrap_or(t).trim().to_string(),
        _ => t.to_string(),
    }
}

/// O corpo tem letra ou digito (sem isso o id nao muda: nada a fixar ainda).
pub(super) fn text_has(s: &str) -> bool {
    s.chars().any(char::is_alphanumeric)
}

/// Espera antes da tentativa seguinte depois de `failures` falhas seguidas: 1, 2, 4... 30 s.
pub(super) fn wait(failures: u32) -> Duration {
    let n = failures.saturating_sub(1).min(5);
    (RETRY * (1u32 << n)).min(RETRY_MAX)
}
