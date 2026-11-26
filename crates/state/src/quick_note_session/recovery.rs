//! Arquivo de recuperacao: o texto que ainda nao esta seguro no vault.
use super::*;

/// Registro de recuperacao, na pasta dos dados do app.
pub const RECOVERY_FILE: &str = "recuperacao.json";


/// Uma nota cujo texto ainda nao esta seguro no vault.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    #[serde(rename = "key", alias = "chave")]
    pub key: String,
    /// Vault em que a nota comecou (so vai para ele).
    pub vault: PathBuf,
    /// `note`, `task` ou `quick_note`.
    #[serde(rename = "kind", alias = "tipo")]
    pub kind: String,
    /// `None`: o rascunho ainda nao tinha sido criado.
    #[serde(default)]
    pub id: Option<String>,
    /// O corpo a gravar.
    #[serde(rename = "text", alias = "texto")]
    pub text: String,
    #[serde(default)]
    #[serde(rename = "image", alias = "imagem")]
    pub image: Option<ImageRef>,
    #[serde(default)]
    #[serde(rename = "framing", alias = "enquadramento")]
    pub framing: Option<Framing>,
    #[serde(default)]
    #[serde(rename = "highlight", alias = "grifo")]
    pub highlight: Option<String>,
    /// O documento sumiu do vault: a nota espera o usuario recriar.
    #[serde(default)]
    #[serde(rename = "lost", alias = "perdido")]
    pub lost: bool,
    /// Corpos que podem estar no disco desta nota: o ultimo confirmado e o de um
    /// trabalho que estava em voo na saida (pode ter chegado depois). Ao restaurar,
    /// um disco com outro texto (editado por fora) nao e sobrescrito.
    #[serde(default, deserialize_with = "one_or_several")]
    #[serde(rename = "on_disk", alias = "no_disco")]
    pub no_disk: Vec<String>,
}

/// `no_disk` foi um texto so (registros antigos) e agora e uma lista.
pub(super) fn one_or_several<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrSeveral {
        Nothing(()),
        Um(String),
        Several(Vec<String>),
    }
    Ok(match OneOrSeveral::deserialize(d)? {
        OneOrSeveral::Nothing(()) => Vec::new(),
        OneOrSeveral::Um(s) => vec![s],
        OneOrSeveral::Several(v) => v,
    })
}

#[derive(Default, Serialize, Deserialize)]
pub(super) struct RecoveryFile {
    #[serde(rename = "version", alias = "versao")]
    pub(super) version: u32,
    #[serde(rename = "notes", alias = "notas")]
    pub(super) notes: Vec<Record>,
}

/// Temporario da gravacao atomica do registro.
/// Um nome por processo: duas instancias nunca escrevem no mesmo temporario.
pub fn temporary_file(fname: &Path) -> PathBuf {
    fname.with_extension(format!("json.{}.tmp", std::process::id()))
}

/// Tira do caminho um registro ilegivel: `recovery.json.corrupt-<data>`
/// (nunca e sobrescrito nem apagado). Devolve para onde foi.
pub fn isolate_recovery(fname: &Path, current_time: NaiveDateTime) -> Result<PathBuf, String> {
    let base = format!("{}.corrompido-{}", RECOVERY_FILE, current_time.format("%Y%m%d-%H%M%S"));
    let mut destination = fname.with_file_name(&base);
    let mut n = 1;
    while destination.exists() {
        n += 1;
        destination = fname.with_file_name(format!("{base}-{n}"));
    }
    std::fs::rename(fname, &destination)
        .map(|()| destination)
        .map_err(|e| t!("error.recovery.isolate", fname.display(), e))
}

/// Caminho do registro na pasta dos dados do app.
pub fn recovery_file(base: &Path) -> PathBuf {
    base.join(RECOVERY_FILE)
}

/// Le o registro (ausente: vazio).
pub fn recovery_read(fname: &Path) -> Result<Vec<Record>, String> {
    match std::fs::read_to_string(fname) {
        Ok(t) => serde_json::from_str::<RecoveryFile>(&t)
            .map(|a| a.notes)
            .map_err(|e| t!("error.recovery.unreadable", fname.display(), e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(t!("error.recovery.read_file", fname.display(), e)),
    }
}

/// Grava o registro de forma atomica (temporario + `rename`); vazio apaga o arquivo.
pub fn write_recovery(fname: &Path, records: &[Record]) -> Result<(), String> {
    let err = |e: std::io::Error| t!("error.recovery.write", fname.display(), e);
    if records.is_empty() {
        return match std::fs::remove_file(fname) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(err(e)),
            _ => {
                sync_folder(fname);
                Ok(())
            }
        };
    }
    if let Some(p) = fname.parent() {
        std::fs::create_dir_all(p).map_err(err)?;
    }
    let json = serde_json::to_string_pretty(&RecoveryFile { version: 1, notes: records.to_vec() })
        .map_err(|e| t!("error.recovery.serializar", e))?;
    let tmp = temporary_file(fname);
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp).map_err(err)?;
        f.write_all(json.as_bytes()).map_err(err)?;
        f.sync_all().map_err(err)?;
    }
    std::fs::rename(&tmp, fname).map_err(err)?;
    sync_folder(fname);
    Ok(())
}

/// fsync da pasta (o `rename` e a remocao ficam no disco). Falha so avisa nada:
/// nem todo sistema de arquivos aceita.
pub(super) fn sync_folder(fname: &Path) {
    if let Some(p) = fname.parent() {
        if let Ok(d) = std::fs::File::open(p) {
            let _ = d.sync_all();
        }
    }
}
