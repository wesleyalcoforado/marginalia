//! Registros da fonte aberta: marcar inicio e fim na imagem atual, cancelar,
//! renomear, mover uma nota entre os dois registros de uma imagem de fronteira.
//! A regra (posicao, extensao, validacao) mora em `marginalia_core::records`; aqui
//! so ha o estado que a faixa do leitor mostra e as gravacoes no vault.

use super::*;
use chrono::NaiveDateTime;
use marginalia_core::model::{Position, Record};
use marginalia_core::records::{self, RecordError};

/// O que a faixa do leitor diz da imagem atual.
#[derive(Clone, Debug, PartialEq)]
pub enum RecordStrip {
    /// Fora de qualquer registro ("Marcar inicio de registro aqui").
    Outside,
    /// Dentro de um registro valido. `registro.fim == None`: "sem fim ainda"
    /// (`end` e entao a extensao calculada, so informativa).
    Em { record: Record, end: Option<Position>, shared: bool },
    /// Um registro quebrado (editado a mao) que e desta imagem: so da para exclui-lo.
    Invalid { record: Record },
}

fn from_source(all: &[Record], source: &str) -> Vec<Record> {
    all.iter().filter(|r| r.source == source).cloned().collect()
}

impl AppState {
    /// O intervalo de `r` pelos indices das imagens da fonte aberta (`img 3 – img 9`).
    pub fn record_range(&self, r: &Record) -> String {
        match self.source.as_ref() {
            Some(s) => crate::tree::range_by_index(s.entries(), r),
            None => crate::tree::range(r),
        }
    }

    /// Onde `p` fica na fonte aberta (`img 3`, `pág 5`).
    pub fn position_label(&self, p: &Position) -> String {
        match self.source.as_ref() {
            Some(s) => crate::tree::index_label(s.entries(), p),
            None => p.file.clone(),
        }
    }

    /// Registros do vault como a faixa os ve (a recarga das gavetas e as
    /// proprias operacoes daqui o atualizam).
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// Troca a lista de registros que a faixa usa (vinda de uma recarga).
    pub fn adopt_records(&mut self, records: Vec<Record>) {
        self.records = records;
    }

    /// Le `records/` do vault de novo. Um registro ilegivel avisa e deixa a
    /// lista como estava.
    pub(super) fn reload_records(&mut self) {
        let Some(v) = &self.vault else {
            self.records.clear();
            return;
        };
        match v.list_records_readable() {
            Ok(l) => self.records = l.into_iter().map(|(r, _)| r).collect(),
            Err(e) => self.warning = Some(t!("record.error.write", e)),
        }
    }

    /// A faixa da imagem atual; `None` sem vault ou sem imagem aberta.
    pub fn record_strip(&self) -> Option<RecordStrip> {
        self.vault.as_ref()?;
        let img = self.current_image()?;
        let hurts = from_source(&self.records, &img.source);
        let ext = records::extensions(&hurts);
        if let Some(r) = records::record_of(&ext, img, None) {
            let end = ext.iter().find(|e| e.record.id == r.id).and_then(|e| e.end.clone());
            return Some(RecordStrip::Em { record: r.clone(), end, shared: records::shared(&ext, img) });
        }
        if let Some(r) = records::invalid_at(&ext, img) {
            return Some(RecordStrip::Invalid { record: r.clone() });
        }
        Some(RecordStrip::Outside)
    }

    /// O registro mais proximo que terminou antes da imagem atual: "Estender
    /// ate aqui" (a faixa so oferece "Ajustar fim" dentro do registro).
    pub fn record_to_extend(&self) -> Option<Record> {
        let img = self.current_image()?;
        let here = Position::de(img);
        let hurts = from_source(&self.records, &img.source);
        let ext = records::extensions(&hurts);
        ext.iter()
            .filter(|e| e.valid && e.record.end.as_ref().is_some_and(|f| records::compare((&f.file, f.page), (&here.file, here.page)).is_lt()))
            .max_by(|a, b| {
                let (x, y) = (a.record.end.as_ref().unwrap(), b.record.end.as_ref().unwrap());
                records::compare((&x.file, x.page), (&y.file, y.page))
            })
            .map(|e| e.record.clone())
    }

    /// Contexto de uma operacao: vault, imagem atual e os registros da fonte, lidos agora.
    fn record_context(&mut self) -> Result<(Vault, ImageRef, Vec<Record>), String> {
        let vault = self.vault_or_error()?;
        let Some(img) = self.current_image().cloned() else {
            return self.fail(QuickNoteError::NoImage.to_string());
        };
        match vault.list_records_readable() {
            Ok(l) => Ok((vault, img.clone(), from_source(&l.into_iter().map(|(r, _)| r).collect::<Vec<_>>(), &img.source))),
            Err(e) => self.fail(t!("record.error.write", e)),
        }
    }

    fn rule_error(e: RecordError) -> String {
        match e {
            RecordError::StartInsideOfRecord(n) => t!("record.error.inside_start", n),
            RecordError::EndBeforeOfTheStart => t!("record.error.before_end"),
            RecordError::EndSwallowsRecord(n) => t!("record.error.swallows_end", n),
        }
    }

    /// Cria um registro sem fim comecando na imagem atual. Recusa nome vazio e
    /// inicio dentro de outro registro (aviso na barra; nada e gravado).
    pub fn mark_start_record(&mut self, name: &str, current_time: NaiveDateTime) -> Result<Record, String> {
        let name = name.trim();
        if name.is_empty() {
            return self.fail(t!("record.error.empty_name"));
        }
        let (vault, img, existing) = self.record_context()?;
        let pos = Position::de(&img);
        if let Err(e) = records::can_mark_start(&existing, &pos) {
            return self.fail(Self::rule_error(e));
        }
        match vault.create_record(name, &img.source, pos, current_time) {
            Ok(r) => {
                self.reload_records();
                self.confirmation = Some(t!("record.confirmation.start", r.name));
                Ok(r)
            }
            Err(e) => self.fail(t!("record.error.write", e)),
        }
    }

    /// Fixa (ou ajusta) o fim do registro `id` na imagem atual.
    pub fn mark_end_record(&mut self, id: &str) -> Result<(), String> {
        let (vault, img, existing) = self.record_context()?;
        let pos = Position::de(&img);
        if let Err(e) = records::can_mark_end(&existing, id, &pos) {
            return self.fail(Self::rule_error(e));
        }
        match vault.update_record(id, |r| r.end = Some(pos)) {
            Ok(r) => {
                self.reload_records();
                self.confirmation = Some(t!("record.confirmation.end", r.name));
                Ok(())
            }
            Err(e) => self.fail(t!("record.error.write", e)),
        }
    }

    /// Apaga o registro (Cancelar sem fim, Excluir com fim). Notas e tarefas ficam.
    pub fn discard_record(&mut self, id: &str) -> Result<(), String> {
        let vault = self.vault_or_error()?;
        let name = self.records.iter().find(|r| r.id == id).map(|r| r.name.clone()).unwrap_or_else(|| id.to_string());
        match vault.delete_record(id) {
            Ok(()) => {
                self.reload_records();
                self.confirmation = Some(t!("record.confirmation.discarded", name));
                Ok(())
            }
            Err(e) => self.fail(t!("record.error.write", e)),
        }
    }

    /// Troca so o nome (o id e o arquivo ficam).
    pub fn rename_record(&mut self, id: &str, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return self.fail(t!("record.error.empty_name"));
        }
        let vault = self.vault_or_error()?;
        match vault.update_record(id, |r| r.name = name.to_string()) {
            Ok(r) => {
                self.reload_records();
                self.confirmation = Some(t!("record.confirmation.renamed", r.name));
                Ok(())
            }
            Err(e) => self.fail(t!("record.error.write", e)),
        }
    }

    /// Move uma nota ou tarefa que esta numa imagem de fronteira para o outro
    /// registro (`destino: Some(id)`) ou de volta ao padrao (`None`).
    pub fn relocate_to_record(&mut self, kind: DocKind, id: &str, destination: Option<&str>) -> Result<(), String> {
        let vault = self.vault_or_error()?;
        let image = match (kind, vault.find_by_id(id)) {
            (DocKind::Note, Ok(Some(Found::Note(p, _)))) => Some(p.image),
            (DocKind::Task, Ok(Some(Found::Task(t, _)))) => match t.image {
                Some(i) => Some(i),
                None => return self.fail(t!("record.error.no_image_item")),
            },
            (_, Err(e)) => return self.fail(t!("record.error.write", e)),
            _ => None,
        };
        let Some(image) = image else {
            return self.fail(marginalia_core::Error::NotFound(id.to_string()).to_string());
        };
        let all = match vault.list_records_readable() {
            Ok(l) => l.into_iter().map(|(r, _)| r).collect::<Vec<_>>(),
            Err(e) => return self.fail(t!("record.error.write", e)),
        };
        let hurts = from_source(&all, &image.source);
        let ext = records::extensions(&hurts);
        if !records::shared(&ext, &image) {
            return self.fail(t!("record.error.not_shared"));
        }
        let candidates = records::candidates(&ext, &image);
        let destination_name = match destination {
            Some(d) => match candidates.iter().find(|r| r.id == d) {
                Some(r) => Some(r.name.clone()),
                None => return self.fail(t!("record.error.destination")),
            },
            None => None,
        };
        match vault.set_record_of(kind, id, destination) {
            Ok(()) => {
                self.confirmation = Some(match destination_name {
                    Some(n) => t!("record.confirmation.moved", n),
                    None => t!("record.confirmation.moved_of_back"),
                });
                Ok(())
            }
            Err(e) => self.fail(t!("record.error.write", e)),
        }
    }
}

#[cfg(test)]
#[path = "tests/records.rs"]
mod tests;
