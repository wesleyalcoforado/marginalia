# gpui

A interface (`crates/ui`) usa o [gpui](https://gpui.rs), o framework de UI com GPU do Zed.
`nucleo` nao depende de gpui.

## Dependencia escolhida

`gpui = { version = "=0.2.2", default-features = false, features = ["font-kit"] }`, do crates.io.

- Por que crates.io e nao git: a 0.2.2 publicada compila neste Mac (arm64, Rust 1.100.0) e ja tem o que o
  visualizador precisa (`canvas`, `Window::paint_image` com `RenderImage`, `with_content_mask`, eventos de
  roda, mouse e teclado). Uma dependencia git do repositorio do Zed nao foi necessaria.
- Versao fixada com `=`: a API do gpui muda depressa; atualizar e uma decisao explicita (rodar o smoke abaixo).
- `default-features = false` tira `wayland`, `x11` e `windows-manifest` (so Linux/Windows); `font-kit` fica
  porque o texto no macOS usa o sistema de fontes.
- gpui usa o crate `image` 0.25 (o mesmo do core): `RenderImage::new` recebe `image::Frame` em **BGRA**, por
  isso `Viewer::set_image` troca os canais R e B.
- Documentacao usada: o proprio fonte do crate em `~/.cargo/registry/src/*/gpui-0.2.2/` (`src/elements/canvas.rs`,
  `src/window.rs`, `src/interactive.rs`, `examples/`) e https://docs.rs/gpui/0.2.2.

### Ajustes no core para resolver as dependencias

Com gpui no workspace o resolvedor exige `wasm-bindgen >= 0.2.100` (via `zed-reqwest` -> `js-sys`), e
`pdfium-render` 0.8.28 limitava `wasm-bindgen <= 0.2.95`. Esses pinos existiam so por causa do Rust 1.81 antigo:

- `pdfium-render` `=0.8.28` -> `=0.8.30` (mesma `pdfium_latest` = pdfium 6996, mesma biblioteca nativa);
- dev-dependency `tempfile` `=3.12.0` -> `3.20` (gpui exige `^3.20`).

Nenhum `cargo update` geral: o `Cargo.lock` foi apenas estendido pelo resolvedor.

## Requisitos do sistema (macOS)

- Xcode com a toolchain Metal (`xcrun -f metal` precisa funcionar): o build do gpui compila os shaders Metal.
- Rust 1.100.0 (stable).

## Compilar e rodar

```sh
cargo run -p ui                          # abre a amostra crates/marginalia_core/integration/fixtures/amostra.jpg
cargo run -p ui -- caminho/pasta         # pasta (le direto)
cargo run -p ui -- caminho/livro.zip     # zip direto; com --cache copia para o cache local
cargo run -p ui -- caminho/doc.tif       # imagem/PDF avulso (PDF precisa do PDFium, ver docs/pdfium.md)
cargo run -p ui -- caminho/pasta --vault caminho/vault   # com vault: grifos e nota rapida
cargo test -p ui                         # logica sem janela (app_state, carga, open_source, viewer_math, annotate, quick_note)
```

Smoke (sem clique; sai com 0 ou 1):

```sh
MARGINALIA_SMOKE=1 MARGINALIA_APPDATA=/tmp/m cargo run -p ui -- caminho/pasta-ou-zip [--cache]
```

Abre a fonte, aplica e desfaz "inverter", percorre TODOS os quadros com `go_next` conferindo que a tela bate
com o estado (imagem na tela, ou marcador de erro no quadro corrompido), abre o 1o quadro com um enquadramento
guardado e sai. `MARGINALIA_SMOKE_ERROS=n` exige exatamente `n` quadros com erro. `MARGINALIA_APPDATA` troca a
pasta dos dados do app (padrao `~/Library/Application Support/Marginalia`), para o smoke nao tocar a do usuario.

Com vault, a fonte do smoke precisa de **pelo menos 3 imagens**: as corridas da carga (Shift+arrasto e nota
enviada logo depois de `go_next`) voltam ao 1o quadro e exigem que `go_next` mude de imagem de verdade. Com menos,
o smoke falha logo com "smoke precisa de pelo menos 3 imagens na fonte (tem N)".

Task 9 (`smoke_grifos_tela`): retangulo, seta por Option+arrasto e seta curta por Option+clique pelos handlers do
visualizador, cor pela paleta (com e sem selecao), clique que seleciona sem mover a imagem, Delete/Backspace apagam e
Cmd+Z devolve (teclas pela janela; com a caixa em foco nao tocam nos grifos), Esc tira a selecao, e confere
`grifos/<fonte>.json` (tipo/cor/cauda) a cada passo e as mesmas cores depois de reler dados do app e grifos do disco
(a "reabertura" rele do disco no MESMO processo: `AppData::load`, `GrifosFile::load` e `MainView::refresh`; nao abre
um processo novo). A amostra de cor passa por `MainView::clicar_amostra`, o mesmo corpo do listener do clique (o smoke
nao sintetiza o clique na amostra); logo depois Delete e Cmd+Z ainda valem (a amostra nao tem `track_focus`).
`MARGINALIA_SMOKE_PAUSA=ms` para nesse ponto (seta selecionada, retangulo, paleta) para uma captura da janela.
Desenho: cada grifo na cor de `Grifo::cor()` (retangulo translucido + borda; seta = haste + cabeca triangular com
sombra escura; ponteiro antigo sem cauda continua um ponto); retangulos e pontos tem um contorno escuro fino por fora
(amarelo e branco legiveis em papel claro); pendente = tracejado/mais grosso, destaque da nota = anel ciano, selecao =
anel branco entre duas linhas escuras + alcas, desenhado por ultimo; nenhum estado muda a cor. Grifos fora da area
(zoom) nao sao desenhados. Com um grifo selecionado a linha de dicas mostra "⌫ apaga · Esc solta · cor: F".

Com um vault aberto o smoke continua: desenha um retangulo com Shift+arrasto (e confere que um Shift+clique de
2 px nao cria grifo e que o arrasto simples so move), digita uma nota com `/` + teclas + uma composicao de acento
(`~` marcado + `ã`, como o macOS entrega uma tecla morta) + Cmd+Enter, confere a nota no vault (imagem, pagina,
enquadramento, grifo) e o foco de volta na imagem, cria uma tarefa com `[] ...` (o `[` nao vira atalho de brilho),
mais duas notas (uma com ponteiro por Option+clique), reabre a primeira nota de outra imagem (quadro,
enquadramento e grifo destacado), confere que grifos e notas feitos com a imagem seguinte ainda carregando nao
usam a imagem/enquadramento anterior, que Esc nao sai do app e que Cmd-Q (com a caixa em foco) sai.
Task 8: Cmd-K / Esc / Cmd-Shift-K levam o foco entre imagem e nota; 12 linhas com Enter (e um acento composto na 2a)
ficam no rascunho, a caixa rola ate o cursor, Cmd+Enter fixa o id e limpa; W/A/S/D mudam o enquadramento no sentido
certo so com a imagem em foco (Shift/Option ignorados; com a nota em foco `w` e texto). Fix round 1: Cmd-1 abre a
gaveta de notas com o foco nela; W/A/S/D com a gaveta em foco nao mexem em nada; com a imagem em foco
Backspace/`p`/`e`/`x`/Cmd-Backspace nao tocam a gaveta; Enter na linha (gaveta em foco) abre a imagem, carrega o
texto na caixa e da o foco a caixa; uma linha longa com acentos (e um acento composto) ocupa mais de uma linha
visual da caixa e o arquivo guarda a linha inteira. Antes do Esc/Cmd-Q, as gavetas: Cmd-1/2/3 (notas/fontes/tarefas) abrem e fecham cada gaveta e o smoke confere que a area da imagem
encolhe exatamente `drawers::LARGURA` (320 px) e volta, que a imagem foi desenhada de novo e que a ida e volta
janela -> imagem fecha; pela gaveta de notas abre a primeira nota (Enter; o grifo destacado cai dentro da area
menor; o texto vai para a caixa com o foco), filtra as notas (busca digitada, etiqueta, contador), pede a exclusao com Backspace e cancela, marca a tarefa como feita (`x`), abre a imagem dela (Enter, o texto na caixa), confere a fonte
com status ok e fecha com Esc. Depois (Task 15, tambem sem vault) abre uma pasta
temporaria com 12 copias da amostra, MOVE a pasta com a fonte aberta, abre um quadro fora do pre-carregamento e confere o
aviso "Fonte indisponivel" e o marcador de erro, aperta 3 setas (nada carrega, o quadro fica), aponta a nova localizacao
(`MainView::relink`, o que o botao faz depois do painel do sistema) e confere o mesmo quadro no mesmo enquadramento, o mapa
nos dados do app e `fontes/` intocado; com vault renomeia a fonte digitando na caixa da gaveta de fontes, tenta trocar para
um vault inexistente (o ativo fica), cria um vault novo com um rascunho na nota (o rascunho fica, a fonte de outro vault
fecha), abre/fecha o seletor com Cmd-Shift-V e volta ao vault do smoke com uma fonte abrindo no vault novo e uma segunda
troca pedida no meio (recusada com "aguarde"; a abertura que chega depois da troca e descartada e o viewer nao fica
"carregando"). `esperar` pede uma pintura a cada ~200 ms e, no
tempo esgotado, diz qual condicao faltou (ocioso / tem_fonte / pintou). Sem `MARGINALIA_APPDATA`, o smoke usa uma pasta
temporaria para os dados do app (nunca a do usuario), diz qual na saida e a apaga ao sair (como as pastas temporarias das fontes). Os eventos de mouse vao direto aos handlers do visualizador
(`Window::dispatch_event` devolve um tipo privado no gpui 0.2.2); as teclas passam por `Window::dispatch_keystroke`.

### Vault e seletor (Task 15)

Ao abrir: `--vault pasta`, senao `MARGINALIA_VAULT`, senao o ultimo vault dos dados do app (se a pasta ainda existir).
Sem nenhum, o seletor de vault abre sozinho (e a caixa de nota diz "Cmd-Shift-V"). O seletor (Cmd-Shift-V, ou o botao
"Vault: ..." na barra) lista os vaults conhecidos dos dados do app, mais recente primeiro, com o ativo marcado, e tem
"Abrir pasta..." (pasta que ja e vault; se nao for, pergunta se deve criar) e "Criar vault aqui" (cria `fontes/`,
`notas/`, `tarefas/`, `grifos/`). A pasta e validada no fundo (`vault_picker::abrir_com_grifos`): inexistente,
ilegivel ou que nao e vault vira aviso e o vault ativo continua. Trocar: as gavetas passam a mostrar o vault novo (filtro
por fonte zerado), o vault vira o "ultimo", a fonte aberta continua so se o vault novo a conhece no mesmo caminho nesta
maquina (senao fecha), o grifo pendente cai (era do vault anterior; a barra avisa) e o texto da caixa de nota fica.

### Fonte ausente, indisponivel, nova localizacao, renomear (Task 15)

- Abrir uma fonte sem caminho nesta maquina ou com caminho que nao existe nao abre nada: `ErroFonte::FonteAusente` e o
  aviso "Fonte nao encontrada"/"sem caminho nesta maquina" abaixo da barra. A thread da interface so le o mapa
  (`AppData::source_path`); se o caminho existe e conferido no fundo (`Abertura::executar_pronta` ->
  `FalhaAbertura::Ausente`), e `AppState::entrar_fonte` transforma isso no mesmo `FonteAusente`.
- Todo resultado que volta do fundo (abrir, nota de outra fonte, "Tentar de novo", nova localizacao, renomear) leva o
  vault de quando comecou: se o vault ativo mudou, `entrar_fonte` descarta a fonte (sem gravar caminho) e
  `concluir_relink`/`concluir_renomear` so mudam o mapa do vault em que foram pedidos. Trocar de vault cancela a
  abertura em andamento e tira o "carregando". Troca de vault, nova localizacao e renomear sao um de cada vez
  (`relink::Exclusivo`): pedir outro no meio mostra "aguarde: ...".
- Erro de E/S ao ler uma imagem no meio da sessao (NAS desmontado; `DecodeError::Leitura`, nunca em cache) mostra o
  marcador de erro e o aviso "Fonte indisponivel: <nome>"; imagem corrompida continua sendo so o marcador. Durante a queda
  as setas nao carregam nada (nada se acumula num NAS preso) e "Tentar de novo" recusa enquanto uma tentativa carrega.
- "Tentar de novo" reabre o quadro que falhou (ou resolve a fonte de novo); "Apontar nova localizacao..." abre o painel do
  sistema (pasta para fonte-pasta, arquivo para zip), confere no fundo que o caminho tem as imagens da fonte (o tipo, a
  imagem a reabrir e pelo menos metade, no minimo 1, das citadas nas gavetas, de uma amostra de ate 200;
  `relink::conferir`) e so entao grava o mapa nome -> caminho nos dados do app (o vault nao muda) e reabre o mesmo quadro
  no mesmo enquadramento. Nada citado e nenhuma imagem a reabrir: pergunta (`window.prompt`, com o caminho e quantas
  imagens tem) antes de gravar. Recusado: o aviso diz por que e o mapa fica. Na gaveta de fontes, fontes sem caminho valido tem o mesmo botao.
- "Renomear" (gaveta de fontes) poe na linha uma caixa (o editor da nota rapida) com o nome selecionado; Enter confere o
  nome (`relink::validar_nome`: nao vazio, sem `/ \ :`, sem ponto no inicio, sem repetir outra fonte sem distinguir
  maiusculas), reescreve o vault no fundo (`Vault::rename_fonte`: notas, tarefas, notas e `grifos/<nome>.json`) e leva o
  caminho e os ajustes (copiados) para o nome novo nos dados do app; a fonte aberta e as gavetas mudam de nome e o quadro
  aberto continua. Esc cancela. A reescrita e retomavel: primeiro `fontes/<novo>.md` (com a marca `renaming_from`) e a
  copia dos grifos, depois cada documento (arquivo temporario + rename), e so no fim os arquivos antigos saem. Se parar
  no meio, o caminho nesta maquina passa a valer para os dois nomes (`AppState::renomear_falhou`), as gavetas sao relidas
  e renomear de novo com o mesmo nome termina; um nome que e de fato de outra fonte continua recusado.
- Recarga: quando a janela volta ao foco (`observe_window_activation`) e depois de gravar, trocar de vault, renomear ou
  apontar uma fonte, `AppState::reload` le no fundo as listas e os grifos da fonte aberta. Quadro aberto, destaque, nota
  aberta, filtros, linha escolhida (pelo id) e rolagem ficam; arquivo que sumiu e pulado calado, ilegivel e contado; uma
  recarga velha (ou que comecou antes de um grifo novo) nao troca os grifos.

### Controles

| Tecla | Acao |
|---|---|
| Setas esquerda/direita, PageUp/PageDown | quadro anterior / seguinte (pagina seguinte ou proximo arquivo) |
| Cmd-O | Abrir fonte: pasta ou zip; para zip pergunta "Abrir direto" ou "Copiar para cache" |
| Cmd-Shift-V | Seletor de vault (setas, Enter abre, Esc fecha, `o` "Abrir pasta...", `n` "Criar vault aqui") |
| `F` (ou o icone da barra da direita) | mostra/esconde o painel de ajustes encaixado a direita (cada controle tem botoes `-`/`+` ou "alternar") |
| `[` / `]` | brilho -/+ (passo 0.05) |
| `{` / `}` | contraste -/+ (passo 0.1) |
| `i`, `g`, `t` | inverter, tons de cinza, limiar (liga em 128; passo 8 no painel) |
| `r` | restaurar ajustes neutros |
| roda/trackpad, arrastar, `+`/`-`/`0` | zoom em torno do ponteiro (fator por evento limitado a 0.5..2.0), mover, ajustar |
| Shift+arrastar | desenha um retangulo (grifo); menos de 4 px e ignorado; nada enquanto a imagem carrega ou com erro |
| Option+arrastar | desenha uma seta (grifo): cauda onde apertou, ponta onde soltou (limitadas a imagem; apertar no fundo nao cria nada), com previa na tela |
| Option+clique (menos de 4 px) | seta curta padrao: ponta no clique, cauda 36 px de tela acima e a esquerda (`annotate::SETA_CURTA`, sempre 36 px NA TELA, em qualquer zoom; no canto vira para o outro lado). O limiar entre clique e arrasto e so o de tela (4 px): com zoom 32 um Option+arrasto de 6 px e uma seta; `SETA_MINIMA` (1e-5) so garante cauda != ponta. Arrasto que o limite da imagem achata para menos de 4 px vira um ponto |
| clique simples (menos de 4 px, sem modificador) | seleciona o grifo sob o ponteiro (8 px de tolerancia; setas/pontos antes de retangulos menores antes de maiores) sem mover a imagem; no vazio tira a selecao. Passou de 4 px: arrasta a imagem |
| Delete / Backspace (so com a imagem em foco e um grifo selecionado) | apaga o grifo; `Cmd+Z` (so com a imagem em foco) o devolve com o mesmo id e cor, selecionado (uma posicao, so na mesma imagem); sem grifo a desfazer, Cmd+Z nao faz nada. Esc com selecao tira a selecao |
| amostras de cor (painel de ajustes, `F`) | 6 cores de `PALETA` (amarelo padrao); a atual tem contorno. Define a cor dos novos grifos (dados do app) e recolore o selecionado (no JSON) |
| `W`/`A`/`S`/`D` (so com a imagem em foco, sem modificador) | mostra o que esta acima/a esquerda/abaixo/a direita (`foco::tecla_pan` + `viewer_math::passo_pan`: 10% do menor lado da area, 4..400 px de tela, igual em qualquer zoom); nunca sao teclas da gaveta, em nenhum foco (`foco::tecla_gaveta`) |
| Cmd-K (ou `/` com a imagem em foco) | foco na caixa de nota rapida (rodape); clicar na caixa tambem. Caixa desligada: o foco fica e a barra diz por que |
| Cmd-Shift-K, Esc (na caixa), clique na imagem | foco de volta na imagem (o texto fica). Borda `0x5b8def` em quem tem o foco, `0x444444` sem foco (imagem e caixa); a gaveta em foco ganha a borda azul por cima |
| digitar (na caixa) | salvamento automatico (`nota_sessao`): a 1a letra cria a nota/tarefa (`[]` no inicio) com imagem, pagina, enquadramento e o ultimo grifo. Vai ao disco o texto sem espacos/linhas em branco nas pontas (o meio fica) |
| Enter, Shift+Enter (na caixa) | nova linha; linha longa quebra sozinha na largura da caixa (`shape_text` -> `WrappedLine`, so na tela). A caixa cresce ate 8 linhas VISUAIS e depois rola por dentro seguindo o cursor (roda do mouse rola livre) |
| setas, Home/End (na caixa) | cima/baixo pelas linhas visuais com coluna-alvo; Home/End e Cmd-←/→: inicio/fim da linha visual; Cmd-↑/↓: inicio/fim do texto; Option-←/→: palavra; Shift estende a selecao (`quick_note::Buffer::mover_cursor`) |
| Cmd+Enter (na caixa) | finaliza: grava, fixa o id, limpa a caixa ("nota criada/salva: <id>") e devolve o foco a imagem |
| Cmd-1 / Cmd-2 / Cmd-3 | abre/fecha a gaveta de notas / fontes / tarefas (a ordem dos icones) no painel esquerdo (de qualquer foco, como Cmd-Q); abrir da o foco a gaveta (`Foco::Gaveta`), fechar a gaveta em foco devolve-o a imagem; a imagem fica com o resto da janela |
| clique numa linha de nota/tarefa, ou Enter nela (gaveta em foco) | abre a imagem, carrega o texto na caixa (`NotaSessao::carregar`; a nota da caixa termina antes; de outra fonte, depois que ela entra; ate o texto chegar a caixa fica limpa e desligada e a barra mostra "carregando nota…") e, QUANDO o texto chega, da o foco a caixa (R26, `foco::FocoAoCarregar`; antes disso o foco fica na gaveta, para nenhuma tecla sumir numa caixa desligada). Se a carga falhar ou nao acontecer (a fonte nao abriu, outra abertura, troca de vault), o foco fica na gaveta e a barra diz. Tecla segurada so repete as setas da lista (`foco::aceita_repeticao`): Backspace segurado pede a exclusao uma vez so |
| setas cima/baixo, Enter (gaveta em foco) | escolher a linha; Enter na de fontes abre a fonte |
| Backspace (gaveta de notas ou tarefas em foco) | pede a exclusao da linha escolhida (confirmacao; nao ha desfazer) |
| `x` ou espaco (gaveta de tarefas em foco) | marca feita / reabre (`state: done` / `open`); a caixa `[ ]` na linha faz o mesmo |
| Esc (gaveta em foco) | fecha o menu de promocao, senao a gaveta |
| Cmd-Q | sair (apaga o cache de zips). `Esc` nao sai do app |
| Enter / Esc (caixa "Renomear" da gaveta de fontes) | renomear a fonte / cancelar (a caixa e o mesmo editor em modo de uma linha) |

Cmd-Q, Cmd-O, Cmd-Shift-V, Cmd-K e Cmd-Shift-K sao acoes do app (`actions!` + `KeyBinding` sem contexto): as
teclas casam antes dos listeners das views, entao valem com a caixa de nota em foco e mesmo sem nenhum foco.

### Menus do macOS (Task 10)

`menus.rs` (sem gpui) descreve a barra: `arvore() -> Vec<MenuDesc>` (Marginalia, Arquivo, Editar, Exibir, Navegar,
Imagem), cada `ItemDesc::Acao { rotulo, acao_id, atalho }` com o `acao_id` de uma `menus::Acao`; `ATALHOS` (atalhos
com Cmd -> `KeyBinding`), `TECLAS` (teclas simples das views) e `disponivel(acao, Estado)`. `main.rs` tem a macro
`acoes_do_menu!`, que de uma lista `TipoGpui => Acao` gera o `actions!`, `acao_gpui`, o inverso, a `KeyBinding` de
um atalho e `registrar_na_vista`; `menus_gpui()` monta os `gpui::Menu` (recortar/copiar/colar/selecionar tudo e
desfazer com `OsAction`).

- Um metodo so: botao, tecla e item de menu chamam `MainView::comando(Acao)` (os botoes da barra tambem usam
  `menus::dica(Acao)` como dica, o texto do menu). O zoom e `Viewer::zoom_por`; recortar/copiar/colar/selecionar
  tudo, `NoteBox::editar`.
- Ligado/desligado: o `validateMenuItem` do gpui pergunta `App::is_action_available`. As acoes da view so sao
  registradas (`on_action` na raiz) quando `menus::disponivel` diz que se aplicam (sem fonte: fechar, navegar, zoom
  e ajustes desligados; Excluir grifo so com selecao; Desfazer so com `foco::alvo_desfazer`; Excluir nota/nota…
  so com a gaveta em foco e uma linha). As de texto so existem na `NoteBox` (contexto `NotaRapida`): fora da caixa
  ficam desligadas. As do app (globais) estao sempre ligadas.
- Teclas: Cmd-Q/O/Shift-V/K/Shift-K sao `KeyBinding` globais; Cmd-X/C/V/A, no contexto `NotaRapida`. Cmd-1/2/3,
  Cmd-Z e Cmd-Backspace dependem do foco e nao repetem segurados, entao continuam no `on_key` da view; a
  `KeyBinding` deles fica no contexto `MarginaliaSoMenu` (nenhum elemento o declara) so para o menu mostrar o atalho.
  Uma tecla dessas que a view nao trata (Cmd-Z na nota) segue para o menu pelo macOS, e o item esta desligado.
  Teclas simples (`F`, `[`, setas, `+`...) nao viram `KeyBinding` (no macOS uma tecla sem modificador no menu
  dispararia digitando): aparecem no rotulo, "Ajustes de imagem (F)".
- Cmd+Z: imagem em foco com grifo apagado -> o grifo; na
  nota, nada (a caixa nao tem desfazer, e Cmd+Z nunca desfaz um grifo enquanto se digita).
- "Fechar fonte" (`AppState::fechar_fonte_aberta`): guarda a posicao, cancela abertura/carga, limpa a imagem,
  termina a nota em edicao (gravada) e tira o foco da caixa desligada.
- Testes (`menus::tests`, `main::tests`): todo `acao_id` resolve e vira uma acao do gpui propria; nenhum `acao_id`
  nem atalho repetido; atalhos com Cmd da arvore == `ATALHOS` (e vice-versa); teclas simples == `TECLAS` /
  `foco::tecla_grifo`; dicas das gavetas e da barra == as do menu; rotulos em portugues, maiuscula so no inicio,
  `…`; o README lista todo atalho dos menus; a barra do gpui espelha a arvore.
- Smoke (`smoke_menus`, com vault): confere a barra posta (`App::get_menus`), escolhe itens pela janela
  (`Window::dispatch_action`, o caminho do macOS) e compara com as teclas: Notas = Cmd-1 (gaveta, foco, largura),
  Excluir nota/nota… abre a confirmacao (Esc cancela), Aumentar zoom = `+`, Ajustar = `0`, Ajustes = `F`,
  Inverter = `i`, Focar nota/imagem = Cmd-K/Cmd-Shift-K (Copiar ligado e Desfazer desligado na nota), Excluir grifo
  e Desfazer (o mesmo grifo, selecionado), Fechar fonte (itens da imagem desligados) e reabrir no mesmo quadro.
  A barra de menus do macOS nao entra numa captura de janela: a aparencia dos menus nao foi conferida a olho.

Ajustes sao gravados por imagem nos dados do app e voltam ao reabrir a imagem. Navegar volta a ajustar a janela
(zoom 1.0), salvo quando o quadro abre com um enquadramento guardado (`open_frame`).

## Organizacao (regra R4)

Toda logica testavel sem janela fica em modulos sem gpui: `app_state.rs` (fonte aberta, quadro atual, ajustes,
erro; navegacao), `carga.rs` (ler/decodificar/filtrar/reduzir/BGRA), `open_source.rs` (nome padrao, registro
no vault e nos dados do app, construcao da fonte), `viewer_math.rs` (geometria), `annotate.rs` (gestos, retangulo
normalizado e limitado a 0..1, ponteiro, grifo na tela) e `quick_note.rs` (texto da caixa de nota: cursor, UTF-16,
composicao, teclas) e `drawers.rs` (gavetas: listas, filtros por fonte e estado, ordem, rotulos, selecao, menu de
promocao, teclas), `vault_picker.rs` (vaults conhecidos, ativo, selecao, validar/abrir uma pasta) e `relink.rs` (aviso de
fonte ausente/indisponivel, conferencia da nova localizacao, nome novo de fonte) e `menus.rs` (arvore dos menus,
atalhos, teclas simples, itens ligados). Tipos gpui aparecem so em `viewer.rs`, `note_view.rs`, `main_view.rs` e `main.rs`.

### Caixa de nota e acentos

O gpui 0.2.2 nao tem campo de texto: `note_view.rs` e um editor minimo com `EntityInputHandler`. No macOS a tecla
vai primeiro aos listeners e, se ninguem parar a propagacao, ao `NSTextInputContext`; tecla morta (agudo, til, circunflexo, crase)
chega como `setMarkedText` (o acento fica sublinhado) e a letra seguinte como `insertText` ja composta (`á ã ç`).
Por isso o listener da caixa so trata Enter/Cmd+Enter/Esc/setas/apagar/Cmd-V/Cmd-A e os atalhos da imagem nao rodam
com a caixa em foco. Com uma composicao em curso o sistema recebe a tecla antes: Enter conclui o acento, nao quebra a
linha.

Quebra automatica: cada linha logica e moldada com `text_system().shape_text(.., Some(largura), ..)`; as quebras
(`wrap_boundaries` -> byte do glifo) viram as linhas visuais de `quick_note::Visual` (sem gpui, testada). Altura,
rolagem, cursor, selecao, clique (`indice_no_ponto`) e a posicao da janela de candidatos (`bounds_for_range`) usam as
linhas visuais. A altura do layout usa a largura da pintura anterior; se a quebra mudar na pintura, outro quadro.

Foco da gaveta: `MainView::foco_gaveta` (um `FocusHandle` no painel esquerdo, `track_focus` +
`capture_any_mouse_down`): qualquer clique dentro dela a foca antes dos filhos; o clique numa linha depois leva o
foco a caixa. As teclas da gaveta passam por `foco::tecla_gaveta` (so com `Foco::Gaveta`; W/A/S/D nunca).

### Gravacoes no vault

Notas, tarefas, `grifos/<fonte>.json`, excluir/marcar tarefa sao gravados na thread da interface: poucos bytes
numa pasta local (o Synology Drive sincroniza depois). Falha de gravacao vira aviso na barra e o texto fica na caixa.
No fundo (Task 15): ler `fontes/` para conferir o nome e registrar a fonte ao abrir (`open_source::abrir_no_fundo`), o
nome livre do Cmd-O, os grifos ao abrir uma fonte (`Abertura::executar_pronta`), validar/criar um vault, conferir a nova
localizacao e renomear uma fonte. `appdata.json` (pasta local) e salvo na thread da interface.

### Gavetas

Layout estilo VS Code (v1.1, modelo sem gpui em `layout.rs`): barra de atividades a esquerda (48 px; icones Notas,
Fontes, Tarefas e a engrenagem embaixo, ainda sem acao), o painel esquerdo com a gaveta aberta (320 px; o mesmo icone
fecha, outro troca, um por vez; Cmd-1/2/3 alternam o mesmo estado), a imagem, o painel de ajustes (280 px) e a barra
fina da direita (40 px, icone `paint-tool` sempre visivel; `F` faz o mesmo). Embaixo, em largura total, a nota rapida
e a barra de status: abrir fonte (`folder-open`), quadro anterior/seguinte (`angle-left`/`angle-right`), o texto
`vault: X · fonte: Y · pág 3/12 · img 42/300 · arquivo.jpg` (`layout::status_texto`; clicar abre o seletor de vault)
e, a direita, abrindo/carregando, trabalho exclusivo, confirmacao, aviso e avisos. A
barra superior antiga saiu. Os paineis encaixam ao lado da imagem (nao por cima): `layout::larguras` divide a janela
(a imagem fica com pelo menos 200 px quando cabe; numa janela estreita o painel esquerdo cede primeiro, depois o de
ajustes; nada fica negativo) e, como o `canvas` mede a area a cada quadro, zoom, arrasto e grifos continuam alinhados
(enquadramento e grifos sao relativos, 0..1).

As listas (todas as `.md` de `notas/`, `tarefas/` e `fontes/`, mais o status do caminho de
cada fonte, que pode estar no NAS) sao lidas por `drawers::carregar` no executor de fundo, nunca no `render`. A view
guarda a ultima lista e a mostra enquanto recarrega ("atualizando..."); so a recarga mais nova entra. Recarrega ao abrir
o app, ao abrir uma gaveta e depois de cada gravacao (nota, excluir, tarefa, fonte nova); a linha
gravada muda na hora (marcacao otimista). `MainView::refresh` tambem roda quando a janela volta ao foco (Task 15).
Arquivo ilegivel e pulado e contado no rodape da gaveta.

### Thread da interface

Nada de NAS nem decodificacao na thread da interface. Navegar so troca o quadro, chama `Prefetcher::hint` e
devolve um `Pedido`; a view roda `carga::executar` (que chama `Prefetcher::get`, `filters::apply`, reduz e
converte para BGRA) no executor de fundo do gpui e entrega a `Resposta` ao `AppState`. Cada pedido tem uma
geracao: a resposta de um pedido superado e descartada, e um pedido superado desiste antes de ler/filtrar
(vale o mais novo). Mudancas de ajuste esperam 60 ms (debounce) e reaplicam os filtros sobre a pagina ja
decodificada, sem reler a fonte. Abrir fonte (percorrer a pasta, copiar o zip) tambem roda no fundo, e o
`Prefetcher` antigo e solto numa thread propria (o `Drop` junta as threads, que podem estar presas no NAS).

### Limite de textura

O atlas Metal do gpui recusa texturas com mais de 16384 px de lado. `carga` reduz a imagem para caber (antes
dos filtros) e guarda o tamanho original, que e o usado na geometria (enquadramento e grifos sao relativos,
0..1). Se `paint_image` ainda falhar, o visualizador mostra o erro em vez de uma janela em branco. A textura
anterior sai do atlas a cada troca de imagem (`drop_image`).

## Aviso conhecido

O cargo mostra `the following packages contain code that will be rejected by a future version of Rust:
block v0.1.6, proc-macro-error2 v2.0.1`. Sao dependencias transitivas do gpui (nao do nosso codigo);
`.cargo/config.toml` desliga esse relatorio (`[future-incompat-report] frequency = "never"`).

Com imagens muito pequenas ampliadas (a amostra tem 40x30 px), as bordas aparecem com um leve degrade
escuro: e a amostragem linear do atlas do gpui na borda da textura. Irrelevante para digitalizacoes reais.
