//! Arvore fonte -> registro -> itens das gavetas de notas e tarefas, sem gpui.
//! Funcao pura: recebe os itens ja filtrados e os registros do vault e devolve
//! as linhas a desenhar e a ordem dos itens (a mesma da selecao por teclado).
//! Sem nenhum registro no vault nada muda: as linhas sao so os itens, na ordem
//! recebida.

use std::collections::{BTreeSet, HashSet};

use marginalia_core::model::{ImageRef, Position, Record};
use marginalia_core::records;
use marginalia_core::source::ImageEntry;
use texts::t;

/// Um item (nota ou tarefa) ja aceito pelos filtros.
pub struct Input<'a> {
    pub id: &'a str,
    /// Tarefa sem imagem: `None` (fica solta, depois das fontes).
    pub image: Option<&'a ImageRef>,
    /// O `record:` do frontmatter (so vale numa imagem de fronteira).
    pub record: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)] // poucas linhas por tela; boxar so complicaria o uso
pub enum TreeLine {
    /// Cabecalho de fonte (recolhivel; a chave de recolher e `source:<name>`);
    /// `""` e "sem fonte" (itens sem imagem).
    Source {
        name: String,
        /// Itens da fonte que passam nos filtros (tambem os escondidos por recolher).
        total: usize,
        collapsed: bool,
    },
    Record {
        record: Record,
        valid: bool,
        /// `a.jpg – z.jpg`, ou `a.jpg – …` enquanto sem fim.
        range: String,
        /// Itens do registro que passam nos filtros (tambem os escondidos por recolher).
        total: usize,
        collapsed: bool,
    },
    /// O k-esimo item visivel (indice em `Tree::order`), dentro de um registro.
    Item(usize),
    /// O k-esimo item visivel, fora de qualquer registro (a desenhar sem recuo).
    Drops(usize),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tree {
    pub lines: Vec<TreeLine>,
    /// `order[k]` e o indice, em `entries`, do k-esimo item visivel.
    pub order: Vec<usize>,
}

/// Um quadradinho das guias pontilhadas (como no explorador de arquivos): o que
/// desenhar em cada coluna de recuo de uma linha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guide {
    /// Nada (o ancestral desta coluna era o ultimo irmao).
    Empty,
    /// Vertical inteira (ainda ha irmaos abaixo).
    Continues,
    /// Vertical inteira mais o traco ate a linha (ha irmao abaixo).
    Branch,
    /// Vertical so ate o traco (ultimo irmao).
    Last,
}

fn depth(l: &TreeLine) -> usize {
    match l {
        TreeLine::Source { .. } => 0,
        TreeLine::Record { .. } | TreeLine::Drops(_) => 1,
        TreeLine::Item(_) => 2,
    }
}

/// As guias de cada linha: uma coluna por nivel de recuo (`Source`: nenhuma;
/// `Record`/`Drops`: 1; `Item`: 2). A ultima coluna e o ramo da propria linha;
/// as anteriores continuam (ou nao) o ramo dos ancestrais.
pub fn guides(lines: &[TreeLine]) -> Vec<Vec<Guide>> {
    let deep: Vec<usize> = lines.iter().map(depth).collect();
    // Ha irmao (mesma profundidade) depois de `i`, antes de sair do pai?
    let after_sibling = |i: usize, d: usize| {
        for &pj in &deep[i + 1..] {
            if pj < d {
                return false;
            }
            if pj == d {
                return true;
            }
        }
        false
    };
    (0..lines.len())
        .map(|i| {
            let d = deep[i];
            (0..d)
                .map(|k| {
                    let has = after_sibling(i, k + 1);
                    match (k + 1 == d, has) {
                        (true, true) => Guide::Branch,
                        (true, false) => Guide::Last,
                        (false, true) => Guide::Continues,
                        (false, false) => Guide::Empty,
                    }
                })
                .collect()
        })
        .collect()
}

/// `file` (sem o prefixo `source/`), e `p. N` quando a pagina nao e a primeira.
fn position_label(source: &str, p: &Position) -> String {
    let file = p.file.strip_prefix(&format!("{source}/")).unwrap_or(&p.file);
    if p.page > 1 {
        t!("record.page_position", file, p.page)
    } else {
        file.to_string()
    }
}

/// Onde `p` fica na fonte, como a barra de status diz: `img 12` (posicao do arquivo entre as
/// imagens da fonte) e, em PDF ou multipagina, `pag 5`. Arquivo que a fonte nao tem mais:
/// o caminho dele.
pub fn index_label(entries: &[ImageEntry], p: &Position) -> String {
    let Some(i) = entries.iter().position(|e| e.path == p.file) else {
        return p.file.clone();
    };
    let pdf = p.file.to_ascii_lowercase().ends_with(".pdf");
    let image = t!("record.index.image", i + 1);
    let page = t!("record.index.page", p.page);
    match (pdf || p.page > 1, pdf && entries.len() == 1) {
        (false, _) => image,
        (true, true) => page,
        (true, false) => t!("warning.join", image, page),
    }
}

/// O intervalo de `r` pelos indices (`img 3 – img 9`, ou `img 3 – …` sem fim).
pub fn range_by_index(entries: &[ImageEntry], r: &Record) -> String {
    let ini = index_label(entries, &r.start);
    match &r.end {
        Some(f) => t!("record.range", ini, index_label(entries, f)),
        None => t!("record.open_range", ini),
    }
}

/// `a.jpg – z.jpg`, ou `a.jpg – …` enquanto o registro esta sem fim.
pub fn range(r: &Record) -> String {
    let ini = position_label(&r.source, &r.start);
    match &r.end {
        Some(f) => t!("record.range", ini, position_label(&r.source, f)),
        None => t!("record.open_range", ini),
    }
}

enum Block {
    Record { ext: usize, items: Vec<usize> },
    Drops(usize),
}

/// Monta a arvore. `show_empty`: registros sem itens tambem aparecem (sem
/// filtro de texto/etiqueta ligado); `source_ok`: o filtro de fonte.
pub fn build(entries: &[Input<'_>], all: &[Record], collapsed_all: &HashSet<String>, show_empty: bool, source_ok: &dyn Fn(&str) -> bool) -> Tree {
    let mut sources: BTreeSet<&str> = entries.iter().filter_map(|e| e.image.map(|i| i.source.as_str())).collect();
    if show_empty {
        sources.extend(all.iter().map(|r| r.source.as_str()).filter(|f| source_ok(f)));
    }
    let mut a = Tree::default();
    for f in sources {
        let records: Vec<Record> = all.iter().filter(|r| r.source == f).cloned().collect();
        let ext = records::extensions(&records);
        let from_source: Vec<usize> = (0..entries.len()).filter(|&i| entries[i].image.is_some_and(|m| m.source == f)).collect();
        let pos = |i: usize| entries[i].image.map(|m| (m.file.as_str(), m.page)).unwrap_or(("", 0));
        let mut blocks: Vec<((&str, u32), u8, &str, Block)> = Vec::new();
        let mut record_by: Vec<Vec<usize>> = vec![Vec::new(); ext.len()];
        for &i in &from_source {
            let img = entries[i].image.expect("filtrado por imagem");
            match records::record_of(&ext, img, entries[i].record).and_then(|r| ext.iter().position(|e| e.record.id == r.id)) {
                Some(k) => record_by[k].push(i),
                None => blocks.push((pos(i), 1, entries[i].id, Block::Drops(i))),
            }
        }
        for (k, e) in ext.iter().enumerate() {
            let items = std::mem::take(&mut record_by[k]);
            if items.is_empty() && !show_empty {
                continue;
            }
            let ini = (e.record.start.file.as_str(), e.record.start.page);
            blocks.push((ini, 0, e.record.id.as_str(), Block::Record { ext: k, items }));
        }
        if blocks.is_empty() {
            continue;
        }
        // Registros primeiro (pela posicao do inicio), depois as soltas (pela posicao da imagem).
        blocks.sort_by(|x, y| x.1.cmp(&y.1).then_with(|| records::compare(x.0, y.0)).then_with(|| x.2.cmp(y.2)));
        let collapsed_one = collapsed_all.contains(&format!("fonte:{f}"));
        a.lines.push(TreeLine::Source { name: f.to_string(), total: from_source.len(), collapsed: collapsed_one });
        if collapsed_one {
            continue;
        }
        for (_, _, _, b) in blocks {
            match b {
                Block::Drops(i) => {
                    a.lines.push(TreeLine::Drops(a.order.len()));
                    a.order.push(i);
                }
                Block::Record { ext: k, mut items } => {
                    items.sort_by(|&x, &y| records::compare(pos(x), pos(y)).then_with(|| entries[x].id.cmp(entries[y].id)));
                    let e: &records::Extension<'_> = &ext[k];
                    let collapsed = collapsed_all.contains(&e.record.id);
                    a.lines.push(TreeLine::Record {
                        record: e.record.clone(),
                        valid: e.valid,
                        range: range(e.record),
                        total: items.len(),
                        collapsed,
                    });
                    if !collapsed {
                        for i in items {
                            a.lines.push(TreeLine::Item(a.order.len()));
                            a.order.push(i);
                        }
                    }
                }
            }
        }
    }
    // Itens sem imagem (tarefa solta) ficam por ultimo, na ordem recebida.
    let no: Vec<usize> = (0..entries.len()).filter(|&i| entries[i].image.is_none()).collect();
    if !no.is_empty() {
        a.lines.push(TreeLine::Source { name: String::new(), total: no.len(), collapsed: false });
        for i in no {
            a.lines.push(TreeLine::Drops(a.order.len()));
            a.order.push(i);
        }
    }
    a
}

/// Os dois registros que dividem a imagem `img` da fonte, se ela e de fronteira.
/// `current` e o registro em que o item esta (override ou padrao) e `other` o
/// destino de "Mover para". `back`: o item ja esta no segundo.
#[derive(Clone, Debug, PartialEq)]
pub struct DestinationRelocate {
    /// O que passar a `relocate_to_record`: `None` volta ao padrao.
    pub destination: Option<String>,
    /// Nome do registro para onde o item iria.
    pub name: String,
    pub back: bool,
}

pub fn relocate_destination(all: &[Record], img: &ImageRef, override_id: Option<&str>) -> Option<DestinationRelocate> {
    let records: Vec<Record> = all.iter().filter(|r| r.source == img.source).cloned().collect();
    let ext = records::extensions(&records);
    let c = records::candidates(&ext, img);
    let [first, second] = c.as_slice() else { return None };
    let no_second = records::record_of(&ext, img, override_id).is_some_and(|r| r.id == second.id);
    Some(if no_second {
        DestinationRelocate { destination: None, name: first.name.clone(), back: true }
    } else {
        DestinationRelocate { destination: Some(second.id.clone()), name: second.name.clone(), back: false }
    })
}

#[cfg(test)]
#[path = "tests/tree.rs"]
mod tests;
