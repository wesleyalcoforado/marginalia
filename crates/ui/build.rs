// No Windows o gpui chama `TaskDialogIndirect` (comctl32 v6), que so existe com
// o manifesto de Common Controls 6 embutido no executavel; sem ele o programa
// morre ao iniciar (STATUS_ENTRYPOINT_NOT_FOUND). O manifesto tambem declara o
// DPI por monitor. Embutimos pelo proprio linker do MSVC (sem rc.exe).
fn main() {
    println!("cargo:rerun-if-changed=windows/marginalia.manifest.xml");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/app/marginalia.ico");
    let target_windows_msvc = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if !target_windows_msvc {
        return;
    }
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let manifesto = std::path::Path::new(&dir).join("windows").join("marginalia.manifest.xml");
    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}", manifesto.display());
    embutir_icone(std::path::Path::new(&dir));
}

// Icone do executavel (o do Explorer e da barra de tarefas): monta um arquivo .res com o .ico e o entrega ao
// linker, sem rc.exe. O .res e uma sequencia de recursos (cabecalho de 32 bytes com tipo e nome numericos, dados,
// alinhamento em 4 bytes) e comeca por um recurso vazio. Cada imagem do .ico vira um RT_ICON e o diretorio vira
// um RT_GROUP_ICON de id 1 (o menor id de grupo e o icone do executavel).
fn embutir_icone(dir: &std::path::Path) {
    let ico = std::fs::read(dir.join("assets").join("app").join("marginalia.ico")).expect("marginalia.ico");
    let u16_em = |i: usize| u16::from_le_bytes([ico[i], ico[i + 1]]);
    let u32_em = |i: usize| u32::from_le_bytes([ico[i], ico[i + 1], ico[i + 2], ico[i + 3]]);
    let n = u16_em(4) as usize;

    fn recurso(res: &mut Vec<u8>, tipo: u16, id: u16, dados: &[u8]) {
        res.extend((dados.len() as u32).to_le_bytes());
        res.extend(32u32.to_le_bytes());
        res.extend([0xFF, 0xFF]);
        res.extend(tipo.to_le_bytes());
        res.extend([0xFF, 0xFF]);
        res.extend(id.to_le_bytes());
        res.extend(0u32.to_le_bytes()); // versao dos dados
        res.extend(0x1030u16.to_le_bytes()); // MOVEABLE | PURE | DISCARDABLE
        res.extend(0u16.to_le_bytes()); // idioma neutro
        res.extend(0u32.to_le_bytes()); // versao
        res.extend(0u32.to_le_bytes()); // caracteristicas
        res.extend(dados);
        while res.len() % 4 != 0 {
            res.push(0);
        }
    }

    let mut res = Vec::new();
    recurso(&mut res, 0, 0, &[]); // recurso vazio que abre todo .res
    let mut grupo = Vec::new();
    grupo.extend([0, 0, 1, 0]);
    grupo.extend((n as u16).to_le_bytes());
    for i in 0..n {
        let e = 6 + 16 * i;
        let tamanho = u32_em(e + 8) as usize;
        let inicio = u32_em(e + 12) as usize;
        recurso(&mut res, 3, (i + 1) as u16, &ico[inicio..inicio + tamanho]);
        grupo.extend(&ico[e..e + 8]); // largura, altura, cores, reservado, planos, bits
        grupo.extend((tamanho as u32).to_le_bytes());
        grupo.extend(((i + 1) as u16).to_le_bytes());
    }
    recurso(&mut res, 14, 1, &grupo);

    let saida = std::path::Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("icone.res");
    std::fs::write(&saida, res).expect("gravar icone.res");
    println!("cargo:rustc-link-arg-bins={}", saida.display());
}
