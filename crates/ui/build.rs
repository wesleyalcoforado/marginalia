// No Windows o gpui chama `TaskDialogIndirect` (comctl32 v6), que so existe com
// o manifesto de Common Controls 6 embutido no executavel; sem ele o programa
// morre ao iniciar (STATUS_ENTRYPOINT_NOT_FOUND). O manifesto tambem declara o
// DPI por monitor. Embutimos pelo proprio linker do MSVC (sem rc.exe).
fn main() {
    println!("cargo:rerun-if-changed=windows/marginalia.manifest.xml");
    println!("cargo:rerun-if-changed=build.rs");
    let target_windows_msvc = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if !target_windows_msvc {
        return;
    }
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let manifesto = std::path::Path::new(&dir).join("windows").join("marginalia.manifest.xml");
    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}", manifesto.display());
}
