//! Troca de vault, recarga, renomear e relocalizar fontes.

use super::*;

impl AppState {
    /// O vault ativo e `v` (o capturado quando um trabalho foi para o fundo).
    pub fn vault_same(&self, v: Option<&Vault>) -> bool {
        self.vault.as_ref().map(|x| &x.root) == v.map(|x| &x.root)
    }

    /// O que a nova localizacao de `name` precisa ter: o tipo do vault, as
    /// imagens citadas no vault e a imagem a reabrir (a que falhou, ou a atual).
    pub fn expected_relink(&self, name: &str, kind: Option<SourceKind>, cited: Vec<String>) -> Expected {
        let current = match self.pending.as_ref().filter(|p| p.name == name).and_then(|p| p.image_frame.as_ref()) {
            Some(q) => Some(q.image.file.clone()),
            None => (self.source_has() && self.current.image.source == name && !self.current.image.file.is_empty())
                .then(|| self.current.image.file.clone()),
        };
        Expected { kind, cited, current }
    }

    /// Prepara a nova localizacao `new` de `name`. Nao toca o disco: rode
    /// [`Relink::execute`] no fundo e entregue o resultado a [`AppState::conclude_relink`].
    pub fn prepare_relink(&mut self, name: &str, new: PathBuf, expected: Expected) -> Relink {
        self.openings += 1;
        let cache_dir = self.cache_dir.join(self.openings.to_string());
        Relink { name: name.to_string(), file_path: new, expected, cache_dir, vault: self.vault.clone() }
    }

    /// A nova localizacao foi validada: grava o mapa nome -> caminho nos dados
    /// do app (por maquina; o vault nao muda) e, se a fonte estava aberta ou
    /// havia um quadro pedido (nota, quadro que falhou), reabre esse quadro no
    /// mesmo enquadramento.
    ///
    /// Se o vault mudou desde o pedido, so o mapa do vault em que foi pedido muda
    /// (a fonte aberta e o aviso sao do vault novo e ficam como estao).
    pub fn conclude_relink(&mut self, r: &Relink, ready: Ready) -> Option<Request> {
        let name = r.name.as_str();
        let root_dir = r.vault.as_ref().map(|v| v.root.clone());
        let error_save = root_dir.and_then(|root_dir| self.write_path(&root_dir, name, &r.file_path).err());
        if !self.vault_same(r.vault.as_ref()) {
            std::thread::spawn(move || drop(ready));
            if error_save.is_some() {
                self.warning = error_save;
            }
            return None;
        }
        self.session_paths.insert(name.to_string(), r.file_path.clone());
        let pending = self.pending.take().filter(|p| p.name == name);
        let open = self.source_has() && self.current.image.source == name;
        let image_frame = pending.as_ref().and_then(|p| p.image_frame.clone()).or_else(|| {
            open.then(|| Frame { image: self.current.image.clone(), framing: Some(self.fr.clone()) })
        });
        let highlight = pending.as_ref().and_then(|p| p.highlight.clone()).or_else(|| if open { self.annot.emphasis.clone() } else { None });
        let note = pending.as_ref().and_then(|p| p.note.clone());
        self.confirmation = Some(t!("confirmation.source_relinkada", name = format!("{name:?}"), file_path = r.file_path.display()));
        let Some(q) = image_frame else {
            // Nada aberto desta fonte: so o mapa muda. Solta a fonte fora desta thread.
            std::thread::spawn(move || drop(ready));
            if error_save.is_some() {
                self.warning = error_save;
            }
            return None;
        };
        let img = q.image.clone();
        let request = self.adopt_source_ready(name, ready, Some(q));
        self.annot.emphasis = highlight;
        if let Some(id) = note {
            self.note_open = Some((id, img));
        }
        if error_save.is_some() {
            self.warning = error_save;
        }
        request
    }

    /// A nova localizacao foi recusada: o mapa fica como estava e o aviso diz por que.
    pub fn relink_failed(&mut self, e: String) {
        self.warning = Some(e);
    }

    /// Aponta `name` para `new` de uma vez. BLOQUEIA (le o vault e o novo
    /// caminho): a interface usa `prepare_relink` / `Relink::execute` no fundo /
    /// `conclude_relink`; isto e o mesmo fluxo numa chamada so.
    #[cfg_attr(not(test), allow(dead_code))] // fluxo inteiro numa chamada: testes e referencia
    pub fn relink_source(&mut self, name: &str, new: PathBuf) -> Result<Option<Request>, String> {
        let (kind, cited) = match &self.vault {
            Some(v) => {
                let kind = v.list_sources().ok().and_then(|fs| fs.into_iter().find(|f| f.name == name).map(|f| f.kind));
                let notes = v.list_notes().unwrap_or_default();
                let tasks = v.list_tasks().unwrap_or_default();
                let files = notes
                    .iter()
                    .map(|(p, _)| &p.image)
                    .chain(tasks.iter().filter_map(|(t, _)| t.image.as_ref()))
                    .filter(|i| i.source == name)
                    .map(|i| i.file.as_str());
                (kind, relink::cited(files))
            }
            None => (None, Vec::new()),
        };
        let expected = self.expected_relink(name, kind, cited);
        let r = self.prepare_relink(name, new, expected);
        // Numa chamada so, a confirmacao (nada citado) e dada por quem chama.
        match r.execute() {
            Ok(v) => Ok(self.conclude_relink(&r, v.ready)),
            Err(e) => {
                self.relink_failed(e.clone());
                Err(e)
            }
        }
    }

    /// Passa a usar o vault `is_open` (validado no fundo por
    /// `vault_picker::open_with_highlights`) e o lembra nos dados do app. A fonte
    /// aberta continua se o vault novo a conhece no mesmo caminho nesta
    /// maquina; senao e fechada. O grifo pendente era do vault anterior e cai.
    /// O texto da caixa de nota fica (esta na view).
    pub fn adopt_vault(&mut self, is_open: vault_picker::Open) -> Swap {
        let vault_picker::Open { vault, highlights } = is_open;
        let mut swap = Swap { discarded_highlight: self.annot.pending.is_some(), ..Swap::default() };
        let name = self.current.image.source.clone();
        let current_path = self.source_has().then(|| self.path_of(&name)).flatten();
        let stays = self.source_has() && current_path.is_some() && self.app_data.source_path(&vault.root, &name) == current_path;
        self.session_paths.clear();
        self.app_data.set_last_vault(vault.root.clone());
        // Salva tambem a posicao na fonte do vault que sai.
        self.warning = self.save_app_data().err().map(|e| t!("warning.remember_vault", e));
        self.vault = Some(vault);
        self.reload_records();
        self.annot.image_drop();
        self.pending = None;
        self.note_open = None;
        if stays {
            if let Some(c) = current_path {
                self.session_paths.insert(name.clone(), c);
            }
            match highlights {
                Some(g) => self.adopt_highlights(&name, g),
                None => self.load_highlights(&name),
            }
        } else if self.source_has() {
            self.close_source();
            swap.closed_source = Some(name);
        }
        swap
    }

    /// Troca para o vault em `root` (uma pasta que ja e vault). Falha (pasta
    /// inexistente, ilegivel, que nao e vault): o vault antigo continua e o
    /// aviso diz por que. BLOQUEIA: a interface valida no fundo
    /// (`vault_picker::open_with_highlights`) e chama [`AppState::adopt_vault`].
    #[cfg_attr(not(test), allow(dead_code))] // fluxo inteiro numa chamada: testes e referencia
    pub fn switch_vault(&mut self, root: &Path) -> Result<Swap, String> {
        let source = self.source_has().then(|| self.current.image.source.clone());
        match vault_picker::open_with_highlights(root, false, source.as_deref()) {
            Ok(a) => Ok(self.adopt_vault(a)),
            Err(e) => self.fail(e.to_string()),
        }
    }

    /// Recarga do vault (listas e grifos da fonte aberta) para rodar no fundo.
    /// `None` sem vault. Barata na thread da interface (so clones).
    pub fn reload(&mut self) -> Option<Reload> {
        let vault = self.vault.clone()?;
        self.reload += 1;
        Some(Reload {
            generation: self.reload,
            vault,
            app_data: self.app_data.clone(),
            source: self.source_has().then(|| self.current.image.source.clone()),
            highlights_version: self.annot.highlights_version,
        })
    }

    /// Entra com o que a recarga leu. Os grifos so trocam se a recarga e a mais
    /// nova, do mesmo vault e da mesma fonte, e nada mudou os grifos aqui
    /// enquanto ela lia. Quadro aberto, destaque e nota aberta ficam. Devolve as
    /// listas para as gavetas (que descartam por conta propria uma recarga velha).
    pub fn receive_reload(&mut self, r: Reread) -> Lists {
        let same = r.generation == self.reload
            && self.vault.as_ref().is_some_and(|v| v.root == r.root_dir)
            && r.highlights_version == self.annot.highlights_version
            && r.source.as_deref() == self.source_has().then_some(self.current.image.source.as_str());
        if let (true, Some(Some(g)), Some(f)) = (same, r.highlights.as_ref().map(|g| g.as_ref().ok()), r.source.as_deref()) {
            if *g != self.annot.highlights {
                self.annot.highlights = g.clone();
                self.annot.error_highlights = None;
                self.annot.highlights_version += 1;
                // Grifo pendente apagado por fora nao vai para a proxima nota.
                if self.annot.pending.as_ref().is_some_and(|(img, gp)| self.annot.highlights.find(img, gp.id()).is_none()) {
                    self.annot.pending = None;
                }
                // Selecao de grifo apagado por fora some.
                let current = self.current_image().cloned();
                if let (Some(s), Some(img)) = (&self.annot.selected, current) {
                    if self.annot.highlights.find(&img, s).is_none() {
                        self.annot.selected = None;
                    }
                }
                let _ = f;
            }
        }
        let mut lists = r.lists;
        if !self.vault.as_ref().is_some_and(|v| v.root == r.root_dir) {
            // Renomeacoes paradas de outro vault: nao sao deste (a sessao nao as retoma).
            lists.resume.clear();
        } else {
            // A faixa do leitor usa os mesmos registros que as gavetas acabaram de ler.
            self.records = lists.records.clone();
        }
        lists
    }

    /// Renomeia a fonte `old` para `new`. BLOQUEIA (reescreve as notas,
    /// tarefas e notas que a citam): a interface valida o nome
    /// (`relink::validate_name`), roda `Vault::rename_source` no fundo e chama
    /// [`AppState::conclude_rename`].
    #[cfg_attr(not(test), allow(dead_code))] // fluxo inteiro numa chamada: testes e referencia
    pub fn rename_source(&mut self, old: &str, new: &str) -> Result<String, String> {
        let vault = self.vault_or_error()?;
        let existing: Vec<String> = match vault.list_sources() {
            Ok(fs) => self.busy_names(old, fs.into_iter().map(|f| f.name).collect()),
            Err(e) => return self.fail(t!("error.source.list_read", e)),
        };
        if !existing.iter().any(|n| n == old) {
            return self.fail(t!("error.source.not_exists", old = format!("{old:?}")));
        }
        let new = match relink::validate_name(new, old, &existing) {
            Ok(n) => n,
            Err(e) => return self.fail(e),
        };
        if let Err(e) = vault.rename_source(old, &new) {
            let failure = sort_failure_rename(&vault, old, &new, &e);
            let m = self.rename_failed(&vault.root, old, &new, &e.to_string(), failure);
            return Err(m);
        }
        self.conclude_rename(&vault.root, old, &new);
        Ok(new)
    }

    /// Os nomes que um novo nome para `old` nao pode ter: `existing` sem o
    /// destino de uma renomeacao de `old` que parou no meio neste vault
    /// (repetir o mesmo nome termina; `Vault::rename_source` recusa um nome que
    /// e de fato de outra fonte).
    pub fn busy_names(&self, old: &str, mut existing: Vec<String>) -> Vec<String> {
        if let (Some((root_dir, a, n)), Some(v)) = (&self.interrupted_renaming, &self.vault) {
            if *root_dir == v.root && a == old {
                existing.retain(|x| x != n);
            }
        }
        existing
    }

    /// `Vault::rename_source` falhou ([`sort_failure_rename`], no fundo).
    /// Parou no meio: o caminho nesta maquina passa a valer para os DOIS nomes
    /// (nada se perde, seja qual for o nome que cada nota ainda tem). Comum
    /// (nome de outra fonte, vault ilegivel): nenhum mapa muda. Parou no fim
    /// (`PartialNoOld`): conta como concluida ([`AppState::conclude_rename`]).
    /// Devolve e mostra o aviso.
    pub fn rename_failed(&mut self, root_dir: &Path, old: &str, new: &str, error: &str, failure: FailureRename) -> String {
        if failure == FailureRename::PartialNoOld {
            // `sources/<old>.md` ja saiu e as referencias ja foram reescritas:
            // so faltou a limpeza. O nome novo vale (notas e grifos novos o usam).
            self.conclude_rename(root_dir, old, new);
            self.confirmation = None;
            let m = t!("warning.rename.parou_in_the_end", old = format!("{old:?}"), new = format!("{new:?}"), error = error);
            self.warning = Some(m.clone());
            return m;
        }
        let m = if failure != FailureRename::Common {
            self.interrupted_renaming = Some((root_dir.to_path_buf(), old.to_string(), new.to_string()));
            if let Some(c) = self.app_data.source_path(root_dir, old) {
                if self.app_data.source_path(root_dir, new).is_none() {
                    if let Err(e) = self.write_path(root_dir, new, &c) {
                        self.warning = Some(e);
                    }
                }
            }
            if let Some(c) = self.session_paths.get(old).cloned() {
                self.session_paths.entry(new.to_string()).or_insert(c);
            }
            t!("warning.rename.parou_in_the_middle", old = format!("{old:?}"), new = format!("{new:?}"), error = error)
        } else {
            t!("error.rename", old = format!("{old:?}"), error = error)
        };
        self.warning = Some(m.clone());
        m
    }

    /// O vault ja foi reescrito (`Vault::rename_source`): o caminho nesta
    /// maquina e os ajustes seguem o nome novo (dados do app, salvos), e a
    /// fonte aberta, o grifo pendente, a nota aberta e o aviso passam a usar o
    /// nome novo. O quadro aberto continua (o prefetcher nao usa o nome).
    ///
    /// `root_dir`: o vault em que a renomeacao foi pedida. Se o vault ativo agora e
    /// outro, so os dados do app daquele vault mudam.
    pub fn conclude_rename(&mut self, root_dir: &Path, old: &str, new: &str) {
        self.interrupted_renaming = None;
        self.app_data.rename_source(root_dir, old, new);
        if let Err(e) = self.save_app_data() {
            self.warning = Some(t!("warning.store_name_new", e));
        }
        if self.vault.as_ref().is_none_or(|v| v.root != root_dir) {
            return;
        }
        if let Some(c) = self.session_paths.remove(old) {
            self.session_paths.insert(new.to_string(), c);
        }
        let swap = |i: &mut ImageRef| {
            if i.source == old {
                i.source = new.to_string();
            }
        };
        swap(&mut self.current.image);
        if let Some((img, _)) = self.annot.pending.as_mut() {
            swap(img);
        }
        if let Some((_, img)) = self.note_open.as_mut() {
            swap(img);
        }
        if let Some(p) = self.pending.as_mut() {
            if p.name == old {
                p.name = new.to_string();
            }
            if let Some(q) = p.image_frame.as_mut() {
                swap(&mut q.image);
            }
        }
        self.confirmation = Some(t!("confirmation.renamed_source", old = format!("{old:?}"), new = format!("{new:?}")));
    }
}
