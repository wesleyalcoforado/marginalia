//! Os modulos do `state` formam um grafo sem ciclos: quem e mais baixo
//! (`load`, `image_frame`, `open_source`...) nao importa quem e mais alto (`app_state`,
//! `quick_note_session`). Le os `crate::<modulo>` do codigo de producao (comentarios e a
//! pasta `tests/` ficam de fora) e falha mostrando o ciclo, se aparecer um.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// Os modulos de topo declarados em `lib.rs` (`pub mod x;`).
fn modules(src: &Path) -> Vec<String> {
    fs::read_to_string(src.join("lib.rs"))
        .unwrap()
        .lines()
        .filter_map(|l| l.trim().strip_prefix("pub mod ")?.strip_suffix(';').map(str::to_string))
        .collect()
}

/// Codigo de `m` sem comentarios de linha: `m.rs` e, se houver pasta, `m/*.rs` (sem `tests/`).
fn code(src: &Path, m: &str) -> String {
    let mut files = vec![src.join(format!("{m}.rs"))];
    if let Ok(dir) = fs::read_dir(src.join(m)) {
        files.extend(dir.map(|e| e.unwrap().path()).filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "rs")));
    }
    files
        .iter()
        .filter_map(|p| fs::read_to_string(p).ok())
        .flat_map(|t| t.lines().map(|l| l.split("//").next().unwrap_or("").to_string()).collect::<Vec<_>>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn graph(src: &Path) -> BTreeMap<String, BTreeSet<String>> {
    let mods = modules(src);
    let mut g = BTreeMap::new();
    for m in &mods {
        let t = code(src, m);
        let uses = mods.iter().filter(|d| *d != m && mentioned(&t, d)).cloned().collect();
        g.insert(m.clone(), uses);
    }
    g
}

/// `crate::<modulo>` como palavra inteira.
fn mentioned(text: &str, modulo: &str) -> bool {
    let target = format!("crate::{modulo}");
    text.match_indices(&target).any(|(i, _)| {
        !text[i + target.len()..].chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

/// Um ciclo do grafo (a lista de modulos em ordem), se houver.
fn find_cycle(g: &BTreeMap<String, BTreeSet<String>>) -> Option<Vec<String>> {
    fn visit(g: &BTreeMap<String, BTreeSet<String>>, v: &str, track: &mut Vec<String>, done: &mut BTreeSet<String>) -> Option<Vec<String>> {
        if let Some(i) = track.iter().position(|x| x == v) {
            let mut c = track[i..].to_vec();
            c.push(v.to_string());
            return Some(c);
        }
        if done.contains(v) {
            return None;
        }
        track.push(v.to_string());
        for w in g.get(v).into_iter().flatten() {
            if let Some(c) = visit(g, w, track, done) {
                return Some(c);
            }
        }
        track.pop();
        done.insert(v.to_string());
        None
    }
    let mut done = BTreeSet::new();
    g.keys().find_map(|v| visit(g, v, &mut Vec::new(), &mut done))
}

#[test]
fn os_modules_of_the_state_not_has_cycles() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let g = graph(&src);
    assert!(g.len() >= 10, "achou so {} modulos em lib.rs", g.len());
    if let Some(c) = find_cycle(&g) {
        panic!("ciclo entre modulos do estado: {}\nmova o tipo ou a funcao compartilhada para o modulo mais baixo", c.join(" -> "));
    }
}

#[test]
fn the_detector_finds_cycle_and_accepts_graph_no_cycle() {
    let mk = |edges: &[(&str, &str)]| {
        let mut g: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (a, b) in edges {
            g.entry(a.to_string()).or_default().insert(b.to_string());
            g.entry(b.to_string()).or_default();
        }
        g
    };
    assert_eq!(find_cycle(&mk(&[("a", "b"), ("b", "c"), ("c", "a")])).map(|c| c.len()), Some(4));
    assert!(find_cycle(&mk(&[("a", "b"), ("a", "c"), ("b", "c")])).is_none());
    assert!(mentioned("use crate::carga::X;", "carga"));
    assert!(!mentioned("use crate::carga_extra::X;", "carga"));
}
