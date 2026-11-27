# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Marginalia é um leitor de imagens para pesquisa genealógica (macOS e Windows, Rust + [gpui](https://gpui.rs)). Abre pastas/zips/PDFs de
digitalizações e deixa anotar (grifos, notas rápidas que viram notas/tarefas em markdown) sem alterar as imagens. Código,
comentários, testes e documentação são em português. O `README.md` é só o básico para humanos (compilar, licença, créditos); a referência do comportamento (atalhos, menus, nota rápida,
dados) é `docs/comportamento.md`: leia-a antes de mexer em qualquer coisa visível.

## Comandos

Requisitos: Rust 1.100.0 e, no macOS, a toolchain Metal (`xcrun -f metal` tem de funcionar: o build do gpui compila shaders); no Windows (x64), as Build Tools do Visual Studio 2022 com C++ e Python 3 (PDFium).
PDFs precisam da biblioteca PDFium em `vendor/pdfium/` (não versionada; `python3 scripts/baixar_pdfium.py`, ver `docs/pdfium.md`); sem ela só os PDFs falham.

```sh
cargo run -p ui -- <pasta|zip|imagem> --vault <pasta-do-vault>   # sem fonte a janela abre vazia, com o convite para abrir uma
cargo test --workspace
cargo test -p state --lib <filtro>                  # um teste do estado (app_state, carga, quick_note_session...)
cargo test -p ui --bin marginalia <filtro>   # um teste da janela (é um crate bin: não existe --lib)
cargo test -p marginalia_core --lib -- <filtro>              # um teste do nucleo; testes de integração: --test vault|decode|...
cargo test --release -p marginalia_core -- --ignored --nocapture medir_passo_do_arrasto   # medição de desempenho dos filtros
```

**Smoke** (abre uma janela real, mexe com cliques/teclas sintéticos e sai com 0 ou 1; o que os testes sem janela não pegam):

```sh
MARGINALIA_SMOKE=1 MARGINALIA_APPDATA=<pasta-temp> cargo run -p ui -- <pasta> --vault <vault-temp>
```

No Windows use `powershell -ExecutionPolicy Bypass -File scripts\smoke.ps1 <pasta>` (as variáveis de ambiente do PowerShell ficam até fechar o terminal, e o app passaria a rodar o smoke sempre). Use sempre `MARGINALIA_APPDATA` e um vault temporários (senão toca nos dados do usuário). Com vault a fonte precisa de **3 ou mais
imagens**. Os passos estão em `main.rs` (`rodar_smoke` e as `smoke_*`; a doc do topo do arquivo lista o que cada um confere).
Mudou geometria da imagem, camadas/tile, grifos, menus ou layout: rode o smoke, os testes não bastam.

`gpui` está fixado em `=0.2.2` (API muda rápido; atualizar é decisão explícita, rodar o smoke). Não rode `cargo update` geral.
O código-fonte do gpui para consulta fica em `~/.cargo/registry/src/*/gpui-0.2.2/`. `cargo clippy` tem avisos antigos em
`marginalia_core/src/decode.rs` (`chunks_exact`); não são de quem está mexendo.

## Distribuição e releases

**Pacote para distribuir** (o `cargo build --release` só gera o executável solto, que no macOS abre o Terminal):
  `scripts/empacotar_mac.sh` monta `dist/Marginalia.app` (PDFium em `Contents/Frameworks`, assinatura ad-hoc);
  `powershell -ExecutionPolicy Bypass -File scripts\empacotar_win.ps1` gera `dist\Marginalia-<versão>-win-x64.zip`
  (portátil: `marginalia.exe` + `pdfium.dll`). Ao empurrar uma tag `v*`, o workflow `release.yml` faz os dois e anexa os
  zips a uma release do GitHub. O ícone mestre é `crates/ui/assets/app/marginalia.png`; `scripts/gerar_icones.py` (precisa do ImageMagick) gera dele a versão sem margem para o `.icns` do macOS, os tamanhos da tela de abertura e do Sobre e o `marginalia.ico` do Windows (embutido no `.exe` pelo `build.rs`). Sem notarização/assinatura, o sistema avisa na primeira vez: no macOS, tente abrir e depois Ajustes do Sistema → Privacidade e Segurança → "Abrir Mesmo Assim" (ou `xattr -dr com.apple.quarantine Marginalia.app`); no Windows (SmartScreen), "Mais informações" → "Executar assim mesmo".

**Aviso de versão nova.** `marginalia_core::update` (lógica pura) + `state::update_check` (HTTP com `ureq`, bloqueante, no executor de fundo) + `ui::update_notice`/`main_view/update_screen.rs` (janela). O app lê `releases/latest` do repositório no GitHub (sem token): o GitHub ignora rascunhos, então publicar a release é o gatilho do aviso. A versão do app é a de `crates/ui/Cargo.toml` (`update::parse_own(env!("CARGO_PKG_VERSION"))` na `ui`; a de `state`/`core` não vale). `MARGINALIA_UPDATE_URL` troca o endereço (o smoke usa um servidor local; no smoke a checagem automática só roda com essa variável). Campos novos em `AppData`: `auto_update_check` (padrão sim), `last_update_check`, `skipped_version`. Nunca baixa nem instala nada.

**Para quem publica:** suba `version` em `crates/ui/Cargo.toml`, crie a tag `v` + a mesma versão (`v0.2.0`; o workflow
recusa se divergirem), espere o workflow gerar o **rascunho** da release, teste os zips e clique em **Publish release**.

## Arquitetura

**Quatro crates, dependência só para baixo: `ui` → `state` → `marginalia_core` → `texts`.** `marginalia_core` (fontes pasta/zip/PDF, decodificação,
filtros de imagem, vault, dados do app, grifos, rotação, `records`: posição, extensão e validação dos intervalos de imagens) e `state` (`app_state`, `load`, `schedule_adjustment`, `quick_note_session`,
`quick_note`, `annotate`, `drawers`, `layout`, `open_source`, `relink`, `image_frame`, `vault_picker`, `viewer_math`, `tree`: navegação, carga de
imagens, ajustes, nota rápida, gavetas, grifos) não dependem de gpui. `ui` é a janela: `main.rs`, `main_view.rs`,
`viewer.rs`, `note_view.rs` e `icons.rs` usam gpui; `menus`, `help`, `config` (a tela de Configurações: seções, linhas e o que cada opção faz nos dados do app), `focus_state` e `slider` não usam, mas só descrevem a tela
(menus, atalhos, foco do teclado, barra deslizante). **Regra R4 (citada nos comentários): lógica que dá para testar sem
janela fica em `marginalia_core`/`state`.** O teste `crates/ui/integration/layers.rs` falha se outro módulo da `ui`
importar gpui ou se `marginalia_core`/`state` o fizerem: módulo novo com gpui entra de propósito na lista `WITH_GPUI` do teste. Dentro do `state` os módulos também não podem formar ciclos (`crates/state/integration/ciclos.rs`): o que dois módulos
compartilham mora no mais baixo (`image_frame`, `open_source`); `app_state` e `quick_note_session` ficam no topo, e `app_state` reexporta os tipos
que a interface usa.

**Textos de usuário.** Todo texto que o usuário lê (menus, ajuda, gavetas, avisos, confirmações, erros do `marginalia_core`, diálogos, botões, dicas)
vem do catálogo `crates/texts/languages/pt.txt` (`chave = texto`, `#` comentário, `\n` quebra de linha, `{0}`/`{nome}` valores, `{{`/`}}`
chaves literais) por `t!("chave")`, `t!("chave", a, b)` ou `t!("chave", nome = valor)`: devolve `String`; chave ausente devolve a própria chave.
A chave é sempre um **literal na mesma linha do `t!(`** (um `match` com um `t!` por variante, nunca `format!` montando a chave), senão
`crates/texts/integration/chaves.rs` não a enxerga. Esse teste falha se uma chave usada não existe, se uma chave do catálogo fica sem uso
ou se um texto cita tecla do Mac fora de marcador. **Nunca escreva tecla num texto** (`⌘ ⇧ ⌥ ⌃ ⌫ ⌦ ↵`, `Cmd`, `Ctrl`, `Shift`, `Alt`, `Option`):
use `{shortcut:<id>}` (o `id` é `Acao::id()` ou uma entrada de `menus::ATALHOS_SEM_MENU`); a `ui` registra o resolvedor
(`menus::registrar_atalhos_do_app()`, na partida) e testes do `state`/`marginalia_core` usam `app_state::teste::registrar_atalhos_falsos()`.
Esc, Enter, Tab e setas podem ficar no texto. Mensagens só de diagnóstico (`eprintln!`, `log`) ficam no código. Sem plural: os textos usam "(s)".

**Dois lugares de dados, nunca misturados.** O *vault* (pasta do usuário, markdown com frontmatter YAML + `grifos/<fonte>.json`) guarda
o resultado da pesquisa e é lido por humanos/Obsidian; as imagens originais nunca são copiadas nem alteradas. Os *dados do app*
(`appdata.json`, por máquina: caminho de cada fonte, ajustes e rotação por imagem, cor dos grifos, vaults conhecidos) nunca vão para o
vault. Structs persistidas sem `#[serde(default)]` quebram arquivos antigos ao ganhar campo: use o default em campos novos.

**Dois espaços de coordenadas (rotação).** Grifos, notas e `Enquadramento` ficam sempre nas coordenadas relativas (0..1) da imagem
*decodificada, sem girar*. A rotação (90° em 90°, `core::rotacao`) é só de exibição: o gpui não gira textura, então `load` gira os
pixels depois do filtro, e o `Viewer` trabalha no espaço de *exibição* (página já girada, `tam_exib()`), convertendo só nas bordas:
API pública (`enquadramento`, `screen_to_image`...), janela do tile, grifos criados/desenhados. A janela/grade do tile é sempre na
página sem girar. Ao tocar nisso, `viewer_math::{janela_visivel_girada, caixa_da_janela_girada}` e `Grifo::{para,da}_exibicao`
são os pontos de conversão.

**Pipeline de uma imagem.** `AppState` (estado puro) produz um `Pedido` (quadro, ajustes, qualidade, janela, rotação) →
`load::run` roda no executor de fundo (leitura do NAS, decodificação, filtros, BGRA, giro) → `Resposta` volta para
`AppState::receber`, que só aceita a *geração* vigente (resposta de geração velha vira `Ignorado`) → `Recebido` → `MainView`
entrega ao `Viewer`. Sem ajuste: uma textura *cheia*. Com ajuste: uma *base* (página inteira reduzida) por baixo e um *tile* (só a
janela visível, na resolução da tela) por cima; `schedule_adjustment` decide quando pedir cada um (um filtro em voo por vez, o próximo
leva o último valor). Mudou um valor/rotação/quadro: `nova_geracao()` e limpar camadas, senão aparece textura de outro estado.

**Um ponto de entrada para comandos.** Menu, atalho, botão e tecla chamam o mesmo `MainView::comando(Acao)`. Uma ação nova toca:
`menus.rs` (`Acao`, `TODAS` com o tamanho certo, `id`, `atalho`, `ATALHOS`/`TECLAS`, `tree`, `disponivel`), a macro
`acoes_do_menu!` em `main.rs` e o `match` de `comando`. Atalhos com Cmd que dependem de foco usam `Contexto::SoMenu`. O teste
O teste `help::tests::help_list_whole_shortcut_of_the_menus` (e o vizinho das teclas da gaveta, nota e seletor) falha se um atalho novo não estiver na tela de Ajuda (`help.rs`); descreva o comportamento da tecla também em `docs/comportamento.md`.

**Ferramentas e ajuda.** A ferramenta de grifo ligada (`annotate::Ferramenta`, estado só no `Viewer`) entra em `annotate::gesto`; a barra de ferramentas e o menu chamam as mesmas `Acao` (`GrifoCaixa`, `GrifoSeta`, `Ferramentas`). A tela de ajuda (`help.rs`, sem gpui) é uma tabela estática que testes cruzam com `menus::ATALHOS`/`TECLAS` e as teclas da gaveta, da nota e do seletor: atalho novo sem linha lá derruba o teste.

**Etiquetas e vault antigo.** `marginalia_core/src/tags.rs` extrai `#etiquetas` do texto (regra do Obsidian); a gaveta de notas filtra por busca, fonte e etiquetas (`drawers::FiltrosNotes`). Não há estados nem promoção de nota; um vault com a pasta `pistas/` (o nome antigo de `notas/`) o app se recusa a abrir e manda rodar `scripts/migrar_pistas_para_notas.py`, fora do app (o app não tem código de migração).

**Nota rápida.** `quick_note_session.rs` é a máquina de estados do salvamento automático (1ª letra cria o arquivo, debounce de gravação,
id definitivo fixado uma vez só após 10 s sem digitar / ao trocar de imagem, recuperação ao sair). Os testes dela estão em
`quick_note_session_testes.rs`, com relógio injetado (mesma ideia em `schedule_adjustment`: cada chamada recebe o `agora`).

**Temas.** As cores do app são por *papel* (`crates/ui/src/tema.rs`: `Paleta` com `fundo`, `barra`, `borda`, `texto`, `acento`, `aviso`...; três paletas, `ESCURO`, `CLARO`, `SEPIA`). A interface lê `crate::theme::atual().<papel>` na hora de desenhar; **nunca escreva `rgb(0x...)` com cor de tema fora de `theme.rs`** (o teste `sem_cor_solta_fora_do_tema` falha e lista a linha; o que não muda com o tema, como a seleção dos grifos e as sombras, entra de propósito na lista `fixas()` do teste). Cor nova = papel novo nas três paletas; `tests/tema.rs` confere o contraste de cada uma. Escolha do usuário: `AppData::tema()`, aplicada por `MainView::aplicar_config`.

**Windows.** O código é o mesmo; só a casca muda, e ela decide em tempo de execução (`cfg!(target_os)`), para compilar e ser testada nos dois sistemas. O sistema não desenha a barra de título (`appears_transparent`): `barras.rs` desenha minimizar/maximizar/fechar com `WindowControlArea` (a área de arrasto é irmã dos botões, nunca a pai: o gpui devolve a primeira área sob o mouse) e `crates/ui/build.rs` embute o manifesto de Common Controls v6 (sem ele o programa morre ao iniciar com `STATUS_ENTRYPOINT_NOT_FOUND`). O `set_menus` do gpui não desenha nada no Windows: `MainView::barra_menu` desenha a barra de menus a partir de `menus::arvore_pc()` (a mesma `arvore()` sem o menu do app: Configurações e Sair vão para Arquivo, Sobre para Ajuda; nenhum item se perde). O estado e o teclado dela (Alt+letra, setas, letras de acesso, Esc) moram em `menu_barra.rs`, sem gpui e testado; `MainView::on_key_menu` o liga às teclas na fase de captura (antes da caixa de nota, que engoliria as setas) e `modificadores_mudaram` só marca o Alt apertado (sublinha as letras de acesso). Tocar no Alt sozinho não faz nada, de propósito. `MARGINALIA_DEBUG_KEYS=1` escreve no terminal cada tecla e cada mudança de modificadores como o gpui as entrega, para diagnosticar um atalho que não responde no teclado real (o smoke injeta teclas por dentro do gpui e não pega isso). O passo `smoke_barra_menu` mede a posição real do menu (`menu_limites`, registrada por um `canvas`) e roda o teclado pelo caminho real; `Window::dispatch_event` não é utilizável de fora, então ele chama os mesmos métodos que o handler de mouse chama. Os atalhos continuam escritos como `cmd-…` em todo o app; só `menus::para_gpui` os troca por `secondary-…` ao criar a `KeyBinding` (Cmd no macOS, Ctrl no resto), `main::normalizar_modificadores` faz o Ctrl valer como "comando" nos eventos de tecla, e `menus::simbolo` mostra `Ctrl+Shift+V` fora do macOS (nos testes sempre o estilo do macOS, que os arquivos dourados gravam). Testes de caminho comparam chaves com `/` (`MAIN_SEPARATOR`), e os de inode/permissão são `#[cfg(unix)]`.

**Janela.** O título nativo é transparente (`TitlebarOptions` em `main.rs`); a faixa de título é desenhada por
`MainView::barra_titulo` na mesma cor da barra de status (`COR_BARRA`), com `layout::ALTURA_TITULO` e `TRAFFIC_LIGHT` (`main.rs`)
alinhados entre si.

## Convenções

- Identificadores (crates, módulos, tipos, funções, campos, nomes de teste, chaves do catálogo e ids de ação) em **inglês**; comentários de código em português **sem acentos** (ASCII), enquanto
  textos de interface, README e docs levam acento.
- **Testes ficam fora do código de produção**: `foo.rs` só declara `#[cfg(test)] #[path = "tests/foo.rs"] mod testes;` e os testes
  moram em `tests/foo.rs`, na pasta `tests/` ao lado do arquivo (`src/tests/foo.rs`; para `src/app_state/x.rs` é
  `src/app_state/tests/x.rs`; `main.rs`, `lib.rs` e `mod.rs` usam `tests/main.rs`, `tests/lib.rs` e `tests/<pasta>.rs`). O módulo
  `testes` é filho de `foo`, então `use super::*;` enxerga também o que é privado (por isso o `#[path]`: um `mod tests;` solto na
  raiz viraria irmão e só veria o público). Testes de integração (só a API pública) ficam em `integracao/` na raiz do crate, fora de
  `src/`, declarados como `[[test]]` no `Cargo.toml` (ver `marginalia_core/integration`); assim `tests/` quer dizer sempre "testes de módulo". O teste de camadas trata `tests/foo.rs` com a mesma permissão de gpui de `foo.rs`.
- Teste de um comportamento visível: faça no módulo sem gpui quando der, e o smoke para o que só existe com a janela.
- `docs/superpowers/{specs,plans}` (só locais, não versionados: estão no `.gitignore`) guardam o design e os planos históricos (o `R2`, `R11`... dos comentários vêm do design); não são
  a fonte da verdade do comportamento atual, `docs/comportamento.md` e o código são.
