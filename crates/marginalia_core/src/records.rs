//! Posicao, extensao e validacao de registros: funcoes puras, sem disco e sem
//! abrir a fonte. A posicao de uma imagem e a chave (arquivo, pagina) na mesma
//! ordem das entradas da fonte (`source::compare_paths`).

use crate::model::{ImageRef, Position, Record};
use crate::source::compare_paths;
use std::cmp::Ordering;

/// Ordem de duas posicoes `(arquivo, pagina)` da mesma fonte.
pub fn compare(a: (&str, u32), b: (&str, u32)) -> Ordering {
    compare_paths(a.0, b.0).then_with(|| a.1.cmp(&b.1))
}

fn cmp_pos(a: &Position, b: &Position) -> Ordering {
    compare((&a.file, a.page), (&b.file, b.page))
}

fn cmp_img(a: &ImageRef, b: &Position) -> Ordering {
    compare((&a.file, a.page), (&b.file, b.page))
}

/// Intervalo efetivo de um registro de uma fonte.
#[derive(Debug, Clone, PartialEq)]
pub struct Extension<'a> {
    pub record: &'a Record,
    /// O fim gravado, ou o inicio do proximo registro (inclusive), ou `None`
    /// (vai ate o fim da fonte).
    pub end: Option<Position>,
    /// `false` para fim antes do inicio, mesmo inicio de outro registro ou
    /// sobreposicao alem da imagem de fronteira: nao agrupa nada.
    pub valid: bool,
}

impl Extension<'_> {
    fn contains(&self, img: &ImageRef) -> bool {
        cmp_img(img, &self.record.start) != Ordering::Less
            && self.end.as_ref().is_none_or(|f| cmp_img(img, f) != Ordering::Greater)
    }
}

/// Extensoes dos registros de UMA fonte (o chamador filtra), por inicio e,
/// no empate, por id.
pub fn extensions(records: &[Record]) -> Vec<Extension<'_>> {
    let mut v: Vec<&Record> = records.iter().collect();
    v.sort_by(|a, b| cmp_pos(&a.start, &b.start).then_with(|| a.id.cmp(&b.id)));
    let mut ext: Vec<Extension<'_>> = (0..v.len())
        .map(|i| {
            let end = v[i].end.clone().or_else(|| v.get(i + 1).map(|n| n.start.clone()));
            let valid = end.as_ref().is_none_or(|f| cmp_pos(f, &v[i].start) != Ordering::Less);
            Extension { record: v[i], end, valid }
        })
        .collect();
    for i in 0..ext.len() {
        for j in i + 1..ext.len() {
            if !ext[i].valid && !ext[j].valid {
                continue;
            }
            let start_same = ext[i].record.start == ext[j].record.start;
            // so conta quem tem intervalo coerente; o de fim antes do inicio ja e invalido
            let overrides = match &ext[i].end {
                Some(f) => cmp_pos(&ext[j].record.start, f) == Ordering::Less,
                None => true,
            };
            if start_same || (overrides && range_ok(&ext[i], &ext[j])) {
                ext[i].valid = false;
                ext[j].valid = false;
            }
        }
    }
    ext
}

fn range_ok(a: &Extension<'_>, b: &Extension<'_>) -> bool {
    let consistent = |e: &Extension<'_>| e.end.as_ref().is_none_or(|f| cmp_pos(f, &e.record.start) != Ordering::Less);
    consistent(a) && consistent(b)
}

/// Registros validos cujo intervalo contem `img` (0, 1 ou 2; com 2, o
/// primeiro e o que termina ali).
pub fn candidates<'a>(ext: &[Extension<'a>], img: &ImageRef) -> Vec<&'a Record> {
    ext.iter().filter(|e| e.valid && e.contains(img)).map(|e| e.record).collect()
}

/// Registro invalido que "e" desta imagem (para o usuario poder corrigir ou
/// excluir): o que comeca nela ou cujo intervalo coerente a contem.
pub fn invalid_at<'a>(ext: &[Extension<'a>], img: &ImageRef) -> Option<&'a Record> {
    ext.iter()
        .find(|e| !e.valid && (cmp_img(img, &e.record.start) == Ordering::Equal || e.contains(img)))
        .map(|e| e.record)
}

/// Registro de uma nota/tarefa em `img`: o do `override_id` se ele for um dos
/// candidatos, senao o primeiro candidato.
pub fn record_of<'a>(ext: &[Extension<'a>], img: &ImageRef, override_id: Option<&str>) -> Option<&'a Record> {
    let c = candidates(ext, img);
    override_id.and_then(|id| c.iter().copied().find(|r| r.id == id)).or_else(|| c.first().copied())
}

/// `img` e o fim de um registro e o inicio de outro.
pub fn shared(ext: &[Extension<'_>], img: &ImageRef) -> bool {
    candidates(ext, img).len() == 2
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecordError {
    /// Nome do registro em que o inicio cairia.
    StartInsideOfRecord(String),
    EndBeforeOfTheStart,
    /// Nome do registro que o fim engoliria.
    EndSwallowsRecord(String),
}

/// Pode iniciar um registro em `pos`? Recusa dentro de um registro com fim
/// gravado (a imagem do fim vale: e a fronteira) e no inicio de outro.
pub fn can_mark_start(existing: &[Record], pos: &Position) -> Result<(), RecordError> {
    for r in existing {
        let ini = cmp_pos(pos, &r.start);
        if ini == Ordering::Equal {
            return Err(RecordError::StartInsideOfRecord(r.name.clone()));
        }
        if ini == Ordering::Greater && r.end.as_ref().is_some_and(|f| cmp_pos(pos, f) == Ordering::Less) {
            return Err(RecordError::StartInsideOfRecord(r.name.clone()));
        }
    }
    Ok(())
}

/// Pode fixar o fim do registro `id` em `pos`? Nao antes do inicio e sem
/// engolir o inicio de outro (a imagem de inicio de outro e a fronteira).
pub fn can_mark_end(existing: &[Record], id: &str, pos: &Position) -> Result<(), RecordError> {
    let Some(r) = existing.iter().find(|r| r.id == id) else {
        return Ok(()); // o vault recusa o id inexistente
    };
    if cmp_pos(pos, &r.start) == Ordering::Less {
        return Err(RecordError::EndBeforeOfTheStart);
    }
    for o in existing {
        if o.id != id && cmp_pos(&o.start, &r.start) == Ordering::Greater && cmp_pos(&o.start, pos) == Ordering::Less {
            return Err(RecordError::EndSwallowsRecord(o.name.clone()));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/records.rs"]
mod tests;
