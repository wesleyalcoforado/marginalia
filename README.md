# Marginalia

Leitor de imagens para pesquisa genealógica (macOS e Windows). Abre digitalizações de livros paroquiais, inventários e
outros documentos (uma pasta, um zip ou um PDF) e deixa anotar sem alterar as imagens: grifos sobre a imagem e uma
nota rápida que vira **nota** ou **tarefa** em markdown, num **vault** de arquivos legível no Obsidian.

Escrito em Rust com [gpui](https://gpui.rs). Atalhos, menus, gavetas e onde ficam os dados estão em
[`docs/comportamento.md`](docs/comportamento.md).

## Compilar e rodar

- **macOS** com Xcode e a toolchain Metal (`xcrun -f metal` precisa funcionar); ou **Windows 10/11 (x64)** com as Build
  Tools do Visual Studio 2022 (C++), Rust no alvo `x86_64-pc-windows-msvc` e Python 3;
- Rust 1.100.0 (stable);
- PDFium para abrir PDFs: `python3 scripts/baixar_pdfium.py` (veja [`docs/pdfium.md`](docs/pdfium.md)). Sem ela só os PDFs
  falham.

```sh
cargo run -p ui -- --vault <pasta-do-vault> [pasta|zip|imagem|pdf]
cargo test --workspace
```

Sem `--vault` (nem `MARGINALIA_VAULT`) o app abre uma tela para escolher ou criar o vault.

Os binários prontos ficam nas [Releases](https://github.com/wesleyalcoforado/marginalia/releases). Como o app não é assinado, o sistema avisa na primeira vez:
no macOS, tente abrir e depois Ajustes do Sistema → Privacidade e Segurança → "Abrir Mesmo Assim"; no Windows
(SmartScreen), "Mais informações" → "Executar assim mesmo".

## Créditos

Ícones: [Unicons](https://github.com/Iconscout/unicons) da Iconscout, licença Apache-2.0 (texto em
`crates/ui/assets/icons/LICENSE-unicons.txt`).

## Licença

O código é distribuído sob a [Apache-2.0](LICENSE). Pode ser usado, modificado e redistribuído, inclusive em projetos
comerciais, desde que se **mantenha a atribuição**: o aviso de copyright, o texto da licença, a indicação das alterações
feitas e o arquivo [`NOTICE`](NOTICE) (nome do autor e site) em toda redistribuição, original ou derivada. A PDFium que
acompanha o app tem a licença própria dela (`LICENSE-pdfium.txt`, dentro do pacote).
