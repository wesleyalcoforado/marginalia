use super::*;
use crate::decode::DecodeError;

// Caracterizacao: o texto de cada erro e o que o usuario le na barra de status.
#[test]
fn message_of_each_error_not_changed() {
    let yaml = serde_yaml::from_str::<serde_yaml::Value>("a: [").unwrap_err();
    let yaml_text = yaml.to_string();
    let json = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
    let json_text = json.to_string();
    let io = std::io::Error::new(std::io::ErrorKind::Other, "falhou");
    let cases: Vec<(Error, String)> = vec![
        (Error::NoFrontmatter, "documento sem frontmatter".into()),
        (Error::Yaml(yaml), format!("yaml invalido: {yaml_text}")),
        (Error::DuplicateName("n".into()), "nome ja existe: n".into()),
        (Error::NotFound("x".into()), "nao encontrado: x".into()),
        (Error::StateInvalid("e".into()), "estado invalido: e".into()),
        (Error::InvalidName("a\"b".into()), "nome invalido: \"a\\\"b\"".into()),
        (Error::OutsideHighlight("g".into()), "grifo fora da imagem: g".into()),
        (Error::ColorOutsideOfThePalette("c".into()), "cor fora da paleta: c".into()),
        (Error::Json(json), format!("json invalido: {json_text}")),
        (Error::VersionNotSupported(7), "dados do app com versao 7 nao suportada".into()),
        (Error::Zip(zip::result::ZipError::FileNotFound), format!("zip invalido: {}", zip::result::ZipError::FileNotFound)),
        (Error::ZipEmpty, "zip invalido: zip vazio".into()),
        (Error::InputNotFound("a.jpg".into()), "entrada nao encontrada na fonte: a.jpg".into()),
        (Error::InputLarge("a.jpg".into()), "entrada grande demais na fonte: a.jpg".into()),
        (Error::InvalidPath("../x".into()), "caminho invalido: ../x".into()),
        (Error::Decode(DecodeError::Invalid("i".into())), "decodificacao: arquivo invalido: i".into()),
        (Error::Io(io), "io: falhou".into()),
        (
            Error::Document("/v/p.md".into(), Box::new(Error::NoFrontmatter)),
            "/v/p.md: documento sem frontmatter".into(),
        ),
        (
            Error::VaultFormatOld("/v".into()),
            "o vault /v esta no formato antigo (pasta pistas/): rode scripts/migrar_pistas_para_notas.py /v e abra de novo".into(),
        ),
    ];
    for (e, expected) in cases {
        assert_eq!(e.to_string(), expected);
    }
}

#[test]
fn message_of_each_error_of_decoding_not_changed() {
    let cases = [
        (DecodeError::Invalid("i".into()), "arquivo invalido: i"),
        (DecodeError::MissingPage { requested: 5, total: 3 }, "pagina 5 inexistente (o documento tem 3)"),
        (DecodeError::NotSupported("xyz".into()), "formato nao suportado: xyz"),
        (DecodeError::PdfiumUnavailable("p".into()), "PDFium indisponivel: p"),
        (DecodeError::Reading("l".into()), "fonte indisponivel: l"),
    ];
    for (e, expected) in cases {
        assert_eq!(e.to_string(), expected);
    }
}
