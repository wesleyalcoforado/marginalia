# PDFium

PDFs sao renderizados com PDFium (crate `pdfium-render` =0.8.30, feature `pdfium_latest` = pdfium 6996). A biblioteca nativa nao fica no repositorio: `vendor/` nao e versionado.

## Onde a biblioteca e procurada (nesta ordem)

1. variavel de ambiente `PDFIUM_LIB_DIR` (pasta que contem a biblioteca);
2. a pasta do executavel e, ao lado dela, `../Frameworks` (app instalado: o pacote leva a biblioteca junto);
3. `<raiz do workspace>/vendor/pdfium/lib/` (desenvolvimento);
4. caminho de bibliotecas do sistema.

A biblioteca se chama `libpdfium.dylib` (macOS), `pdfium.dll` (Windows) ou `libpdfium.so` (Linux). Se nada for encontrado,
`decode_page` devolve `DecodeError::PdfiumIndisponivel` com a instrucao de instalacao (nunca entra em panico).

## Baixar (desenvolvimento, CI e empacotamento)

```sh
python3 scripts/baixar_pdfium.py                       # plataforma desta maquina
python3 scripts/baixar_pdfium.py --plataforma win-x64  # outra plataforma (mac-arm64, mac-x64, win-x64, linux-x64)
```

O script (so biblioteca padrao do Python) baixa o pacote da release `chromium/6996` de
https://github.com/bblanchon/pdfium-binaries, **so o usa se o SHA-256 bater com o fixado no proprio script** (os hashes
conferem com `pdfium-sha256sum.txt` publicado na release) e deixa em `vendor/pdfium/`:

```
vendor/pdfium/lib/<biblioteca>
vendor/pdfium/LICENSE
```

No Windows a DLL vem em `bin/` dentro do pacote; o script a coloca em `lib/` para o caminho ser o mesmo nas tres plataformas.
Trocar de versao: atualize `RELEASE`, `URL_BASE` e os hashes no script juntos, e a feature `pdfium_latest` do `pdfium-render`
tem de corresponder a essa versao.

Se o macOS bloquear o carregamento por quarentena: `xattr -d com.apple.quarantine vendor/pdfium/lib/libpdfium.dylib`.

## Empacotar

O usuario final nao baixa nada: o pacote (zip portatil ou instalador) leva a biblioteca junto, ao lado do executavel
(Windows e Linux) ou em `Contents/Frameworks` do `.app` (macOS). Rode o script acima na etapa de empacotamento e copie
`vendor/pdfium/lib/<biblioteca>` e `vendor/pdfium/LICENSE` (a licenca do PDFium deve acompanhar a redistribuicao).

## Fixtures

`crates/marginalia_core/integration/fixtures/` e gerado por
`cargo test -p marginalia_core --test decode -- --ignored gerar_fixtures`.
