# Marginalia

Leitor de imagens para pesquisa genealógica (macOS e Windows). Abre digitalizações de livros paroquiais, inventários e
outros documentos — uma pasta ou um zip, normalmente num NAS montado por SMB — e deixa anotar sem tirar a imagem
de vista: grifos (retângulo ou seta, em 6 cores) sobre a imagem, uma nota rápida que vira **nota** ou **tarefa**
em markdown e se salva sozinha, e gavetas para rever, editar e excluir as notas (com `#etiquetas` no próprio texto). Tudo o que você produz fica num **vault** de arquivos `.md`/`.json`,
legível no Obsidian. As imagens originais nunca são alteradas nem copiadas para o vault.

Escrito em Rust com [gpui](https://gpui.rs) (o framework de interface do Zed). `crates/marginalia_core` tem a
lógica de dados (fontes, decodificação, filtros, vault, dados do app), `crates/state` o estado do app
(navegação, carga de imagens, ajustes, nota rápida) e `crates/ui` a janela.

## Compilar e rodar

Requisitos:

- **macOS** com Xcode e a toolchain Metal (`xcrun -f metal` precisa funcionar: o gpui compila shaders Metal); ou
  **Windows 10/11 (x64)** com as Build Tools do Visual Studio 2022 (workload "Desenvolvimento para desktop com C++"),
  Rust no alvo `x86_64-pc-windows-msvc` e Python 3 para baixar o PDFium;
- Rust 1.100.0 (stable);
- PDFium para abrir PDFs: a biblioteca nativa não fica no repositório. Rode `python3 scripts/baixar_pdfium.py` (ou aponte
  `PDFIUM_LIB_DIR`), como descrito em [`docs/pdfium.md`](docs/pdfium.md). Sem ela o app abre normalmente e só os
  PDFs mostram o erro com a instrução.

```sh
cargo run -p ui -- --vault <pasta-do-vault> [fonte]
```

- `fonte`: uma pasta, um zip (`--cache` copia o zip para um cache local, apagado ao sair, em vez de ler direto do
  NAS) ou uma imagem/PDF avulso. Sem `fonte` a janela abre vazia, com um quadro no meio convidando a abrir uma (botão "Abrir nova fonte", Cmd-O ou soltar o item na janela).
- Vault: `--vault <pasta>` ou, senão, a variável `MARGINALIA_VAULT`. Sem nenhum dos dois, abre antes uma
  **tela de escolha do vault** (janelinha própria, toda vez): os vaults usados antes (o mais recente primeiro; um
  cuja pasta sumiu aparece esmaecido), "Abrir pasta…" e "Criar vault aqui". Escolhido o vault, a janela principal
  abre (com a fonte da linha de comando, se houver); fechar a tela encerra o app, pois o vault é obrigatório.
- Testes: `cargo test --workspace`. Smoke sem cliques (abre, navega, anota, mexe nas gavetas e nos menus, simula
  a fonte sumindo, renomeia, troca de vault e sai com 0 ou 1):
  `MARGINALIA_SMOKE=1 MARGINALIA_APPDATA=/tmp/m cargo run -p ui -- <pasta-ou-zip> --vault <vault>`
  (detalhes em [`docs/gpui.md`](docs/gpui.md)). No PowerShell as variáveis de ambiente ficam definidas até fechar o
  terminal (e o app passaria a rodar o smoke sempre): use `powershell -ExecutionPolicy Bypass -File scripts\smoke.ps1 <pasta>`, que as limpa ao terminar (o `Bypass` vale só para essa execução: o Windows bloqueia scripts `.ps1` por padrão).
- **Pacote para distribuir** (o `cargo build --release` só gera o executável solto, que no macOS abre o Terminal):
  `scripts/empacotar_mac.sh` monta `dist/Marginalia.app` (PDFium em `Contents/Frameworks`, assinatura ad-hoc);
  `powershell -ExecutionPolicy Bypass -File scripts\empacotar_win.ps1` gera `dist\Marginalia-<versão>-win-x64.zip`
  (portátil: `marginalia.exe` + `pdfium.dll`). Ao empurrar uma tag `v*`, o workflow `release.yml` faz os dois e anexa os
  zips a uma release do GitHub. O ícone mestre é `crates/ui/assets/app/marginalia.png`; `scripts/gerar_icones.py` (precisa do ImageMagick) gera dele a versão sem margem para o `.icns` do macOS, os tamanhos da tela de abertura e do Sobre e o `marginalia.ico` do Windows (embutido no `.exe` pelo `build.rs`). Sem notarização/assinatura, o sistema avisa na primeira vez: no macOS, tente abrir e depois Ajustes do Sistema → Privacidade e Segurança → "Abrir Mesmo Assim" (ou `xattr -dr com.apple.quarantine Marginalia.app`); no Windows (SmartScreen), "Mais informações" → "Executar assim mesmo".

## A janela

A **barra de título** tem a mesma cor da barra de status (o título nativo do macOS é transparente e o app pinta a
faixa; os botões da janela ficam por cima, e arrastar a faixa ou dar duplo clique nela funciona como sempre). Em tela
cheia ela some. No **Windows** o sistema não desenha a barra: o app desenha os botões de minimizar, maximizar e fechar
(à direita), o título arrasta a janela e o duplo clique nele maximiza; logo abaixo vem a barra de menus (ver "Menus").

O layout é o do VS Code, da esquerda para a direita:

- **Barra de atividades** (esquerda): os ícones Notas, Fontes e Tarefas (Cmd-1/2/3, nessa ordem) abrem e fecham
  o painel lateral; um painel por vez, o mesmo ícone fecha. A engrenagem embaixo abre a tela de Configurações (Cmd-,).
- **Sobre** (menu Marginalia > Sobre o Marginalia): uma janela pequena do macOS, de tamanho fixo e no tema escolhido,
  com o ícone do app, a versão, o desenvolvedor (Wesley Alcoforado), o link para o site Genealogia do Ceará
  (<https://www.genealogiadoceara.net>, abre no navegador) e "Todos os direitos reservados" com o ano. Pedir o Sobre
  de novo traz a mesma janela para a frente; Esc ou o botão vermelho fecha.
- **Configurações** (engrenagem da barra de atividades, Cmd-, ou menu Marginalia; Esc ou clicar fora fecha): uma
  tela sobreposta com as preferências do app, cada uma com opções prontas (clicar escolhe e já vale, sem botão
  de salvar) e o botão "Restaurar padrões" no fim. Ficam nos dados do app. Por enquanto: **Reabrir a última
  fonte** (sim/não; ao abrir o app sem indicar uma fonte, abre a mais recente do vault que ainda existe),
  **Tema** (escuro, o de sempre; claro; sépia, tom de papel; vale para todas as janelas na hora, inclusive a de
  escolha do vault, e a cor dos grifos não muda com ele) e
  **Salvamento da nota** (rápido / normal / espaçado; ver "Nota rápida"), **Tamanho da letra da
  nota** (pequena 12 px / média 14 px, a de sempre / grande 16 px; só na caixa da nota rápida) e **Largura do painel**
  (estreito 260 / médio 320, o padrão / largo 460; arrastar a borda do painel continua valendo e deixa uma largura
  "personalizada", sem opção marcada).
- **Painel lateral** (as gavetas): encaixa ao lado da imagem (a imagem encolhe; nada fica por cima dela).
- **Barra de ferramentas** (opcional, no topo da coluna da imagem, entre as barras laterais; menu Exibir > Barra de ferramentas, começa visível e a
  escolha fica nos dados do app): grifo caixa e grifo seta | **cor dos grifos** (ícone de paleta, com uma bolinha da cor atual: abre um menuzinho com as 6 cores, amarelo é o padrão; a cor escolhida vale para os grifos novos e, com um grifo selecionado, muda a dele; fecha ao escolher, com Esc ou clicando fora) | girar à esquerda e à direita | zoom −, zoom + e
  ajustar à janela | **Ir para** (caixa numérica de uma linha: só aceita algarismos, no máximo tantos quanto o total (99 imagens: 2; 500: 3), também ao colar, com setinhas e ↑/↓ (Shift: 10) para mudar o número; digite e Enter; acima do total vai à última imagem, 0 ou
  negativo vai à primeira; a caixa mostra a imagem (ou página) atual e acompanha a navegação; ao digitar nela o número fica até Enter, Esc ou sair da caixa, e ao passar o mouse a dica diz o total. Num PDF, ou outro arquivo
  único com várias páginas, o número é a página; nas demais fontes, a posição da imagem na fonte). Cada botão faz o
  mesmo que o item de menu e tem dica com o atalho. Ligada, a ferramenta de
  grifo **continua ligada** depois de desenhar, até clicar no botão de novo ou apertar Esc (o cursor vira cruz
  e o botão fica azul): arrastar sem modificador desenha, um clique sem arrastar seleciona o grifo, Shift e
  Option continuam valendo e prevalecem, e para mover a imagem usa-se `W`/`A`/`S`/`D`. Ocultar a barra desliga
  a ferramenta.
- **Imagem** no centro: zoom, arrasto, grifos.
- **Painel de ajustes** (direita): o ícone de pincel na barra fina da direita fica sempre visível e abre/fecha o
  painel (ou Cmd-F). No modelo do painel Basic do Camera Raw: sob o cabeçalho "▾ Ajustes" (clicar recolhe), uma
  **barra** por ajuste, com o rótulo à esquerda, a caixa numérica editável à direita e a trilha fina embaixo, com
  um marcador triangular. Brilho e contraste vão de −100 a +100 (0 no meio é o neutro), nitidez e passa-alta de
  0 a 100, limiar de 0 a 255 (com a caixa de liga/desliga ao lado do rótulo; mexer na barra liga). Inverter e
  tons de cinza são interruptores; "Restaurar ajustes" fica no fim. O **olho** do
  cabeçalho liga e desliga a prévia dos ajustes: desligado, a imagem aparece sem ajustes (nada é apagado; vale
  para a sessão, também nas outras imagens) e mexer num ajuste o liga de novo.

  **Onde o filtro roda.** Com um ajuste ativo, só a parte visível da página é filtrada, na resolução da tela
  (pixels físicos: numa tela Retina, o dobro dos pontos): o **tile**. Por baixo dele fica a **base**, a página
  inteira reduzida (lado maior de até 2048 px, cópia feita uma vez por página) e filtrada com o mesmo valor, só
  como fundo para o que ainda não tem tile. O desfoque da nitidez e do passa-alta tem o raio na escala da tela
  (sigma de 2 px com a página em resolução cheia, proporcionalmente menor quando ela aparece reduzida), então o realce tem
  o mesmo alcance relativo à página em qualquer zoom; na base, reduzida, ele é proporcionalmente mais fraco. Uma
  imagem com ajuste guardado abre com a base e o tile vem assim que o visualizador informa a região visível. A
  cada valor novo da barra sai um tile; só um filtro roda por vez e o próximo leva sempre o último valor (os do
  meio do arrasto são pulados). A base do valor sai ao soltar (ou 200 ms parado), depois do tile dele. Um valor
  novo **não** interrompe o tile em andamento (ele chega e mostra o valor dele; o seguinte traz o último); só
  mudar a região visível (pan, zoom, redimensionar), a imagem, a página ou a fonte, ou o olho, interrompem um
  tile. Uma base em andamento é interrompida por um valor novo. Pan e zoom contínuos pedem um tile 60 ms depois
  de parar (o primeiro tile de uma imagem sai na hora) e nunca gravam nada. No pan, a área que aparece mostra a
  base por baixo. Logo depois de mudar o valor ela pode mostrar, por um instante, a base do valor anterior
  (enquanto o tile do valor novo não chegou, as camadas do anterior ficam juntas) ou o fundo escuro (o tile novo
  já tirou a base anterior e a do valor novo ainda não veio: ela é pedida na hora do pan, antes do tile da região
  nova). Sem ajuste, a imagem
  aparece inteira sem filtro, como sempre. Os ajustes vão para os dados do app **depois de soltar** (ou 500 ms
  parado), uma gravação por arrasto, no fundo.

  Tempos de um passo do arrasto (um tile) numa janela de 2560×1600 pixels físicos, Mac com 14 núcleos
  (`cargo test --release -p marginalia_core -- --ignored --nocapture medir_passo_do_arrasto`), páginas sintéticas
  de 6000×4000 e 12000×8000: com a página ajustada à janela, nitidez ou passa-alta ~8–12 ms e as duas juntas
  ~15–20 ms; no zoom em que a página aparece em resolução cheia (o tile mais caro), ~14–16 ms e ~22–25 ms. Isso
  com o recorte da região já guardado (só o valor mudou); depois de um pan ou zoom o recorte é refeito, +3 ms em
  resolução cheia e +8 a +18 ms com a página reduzida (até ~37 ms no total). O filtro antigo, da página inteira em
  resolução cheia, levava ~140 ms (uma das duas) e ~260 ms (as duas) na de 6000×4000, ~0,5–1,2 s na de
  12000×8000 (com o desfoque refeito nesta rodada, ~55/~105 ms e ~0,22/~0,41 s).
- **Rotação**: Cmd-L / Cmd-R giram a imagem de 90° em 90° (o ponto no centro da janela continua no centro e o zoom
  não muda). Fica guardada por imagem (e página) nos dados do app: reabrir a imagem a mostra girada. Com ajuste ativo, a
  base e o tile são refeitos na orientação nova (a janela fica escura por um instante até chegarem).
- **Nota rápida** em largura total embaixo da imagem (sem linhas de dicas: todos os comandos estão na tela de
  ajuda, `?`).
- **Barra de status**: botões Abrir fonte e Vault (a navegação entre imagens é pelo teclado: ← →), depois vault · fonte · página ·
  imagem (clicar abre o seletor de vault), à direita, avisos e confirmações.

A borda azul mostra quem tem o foco do teclado: a imagem, a nota rápida ou a gaveta. Os botões só com ícone têm
dica (tooltip) com o nome e o atalho.

## Nota rápida e salvamento automático

- Não há botão "salvar". A **1ª letra** digitada cria a nota no vault, com a imagem, a página, o enquadramento e o
  grifo pendente daquele momento, com um id provisório (`AAAAMMDD-HHMM-sem-texto`). Começar com `[]` cria uma
  **tarefa** no lugar da nota (o arquivo só nasce quando há algo além de `[`).
- Cada tecla só guarda o texto; o disco o recebe meio segundo depois da última tecla (ou a cada 2 s digitando sem
  parar). Em Configurações, "Salvamento da nota" muda essa espera: *Rápido* (¼ de segundo, ou a
  cada 1 s digitando sem parar), *Normal* (o padrão, acima) ou *Espaçado* (2 s, ou a cada 5 s digitando sem parar).
  A regra dos 10 s abaixo não muda.
- **Regra dos 10 s**: o id definitivo (data + o começo do texto, `20261113-1432-batismo-de-ana`) é fixado e o
  arquivo renomeado **uma única vez**, depois de **10 s sem digitar**, ao sair da caixa, ao mudar de
  imagem/página/fonte/vault, no Cmd-Enter ou ao sair do app. Depois disso, editar não renomeia mais.
- Mudar de imagem, página, fonte ou vault **termina** a nota: o texto vai para a nota da imagem em que ela
  começou, a caixa limpa ("nota salva: <id>") e a próxima tecla cria uma nota nova no quadro novo.
- **Etiquetas**: escreva `#palavra` em qualquer ponto do texto da nota e ela vira uma etiqueta (a mesma regra do
  Obsidian, que também as mostra: letras com acento, dígitos, `_` ou `-`, com ao menos um não-dígito; não vale
  colada a uma letra, como em `a#b`, nem como título `# Título`). A gaveta de notas filtra por elas.
- Cmd-Enter finaliza no lugar: fixa o id, limpa a caixa (quando o disco já tem o texto) e devolve o foco à
  imagem, pronta para a próxima nota.
- Clicar numa nota ou tarefa da gaveta (ou Enter nela) abre a imagem dela, carrega o texto na caixa e dá
  o foco à caixa para continuar editando (transcrições longas de várias páginas se fazem assim). A nota que
  estava na caixa é salva antes.
- Se uma gravação falha, o texto fica na caixa, o erro aparece na barra e o app tenta de novo com espera
  crescente. Ao sair (Cmd-Q, fechar a janela) o que ainda não estava seguro vai primeiro para um registro de
  recuperação nos dados do app e o vault tem até 3 s; na próxima abertura o registro volta para o vault.
- O texto gravado é o da caixa sem espaços e linhas em branco nas pontas (o recuo e as linhas em branco do meio
  ficam); numa tarefa sai também o `[]` do início.

## Atalhos

As teclas desta tabela e dos textos do app estão como no macOS. No **Windows** (e no Linux) o Cmd é o **Ctrl** (Cmd-O é
Ctrl+O), o Option é o **Alt** e o Shift é o Shift; o app mostra as teclas já na forma do sistema (`Ctrl+Shift+V`).

| Tecla | Ação |
|---|---|
| Cmd-O | Abrir fonte (pasta ou zip; para zip pergunta "Abrir direto" ou "Copiar para cache") |
| Arrastar e soltar na janela | Abre como o Cmd-O (mesma pergunta para zip): um item só, pasta, zip, imagem (jpg/png/tiff) ou PDF; a imagem/PDF solto abre sozinho como fonte de uma imagem. Recusado com aviso na barra sem vault, durante uma renomeação, troca de vault ou nova localização ("aguarde"), com vários itens ou outro tipo de arquivo |
| Cmd-Shift-V | Abrir vault… (o seletor: vaults conhecidos, o ativo marcado; "Abrir pasta…", "Criar vault aqui") |
| Cmd-Q | Sair (apaga o cache de zips); vale de qualquer foco. Esc não sai do app |
| Seta esquerda / Seta direita, PageUp/PageDown | Quadro anterior / seguinte (página seguinte ou próximo arquivo) |
| Roda/trackpad, arrastar | Zoom em torno do ponteiro; mover a imagem |
| `W` / `A` / `S` / `D` (imagem em foco) | Mostrar o que está acima / à esquerda / abaixo / à direita (segurar repete); com Shift/Option/Cmd não fazem nada. Nunca mexem na gaveta (nem com ela em foco) |
| `+` (ou `=`), `-` (ou `_`), `0` | Mais zoom, menos zoom, ajustar à janela |
| Cmd-F | Mostrar/esconder o painel de ajustes |
| Cmd-, | Abrir/fechar a tela de Configurações (também pela engrenagem da barra de atividades e pelo menu Marginalia); Esc também fecha |
| `?` | Abrir/fechar a tela de ajuda com todos os comandos (também pelo ícone `?` da barra de atividades e pelo menu Ajuda); Esc também fecha. Digitada na nota rápida, escreve `?` |
| Cmd-[ / Cmd-] | Brilho −/+ (move a barra de 5 em 5; vale de qualquer foco, com uma fonte aberta) |
| Cmd-Shift-[ / Cmd-Shift-] | Contraste −/+ (move a barra) |
| Cmd-I, Cmd-G, Cmd-T | Inverter, tons de cinza, limiar (liga/desliga) |
| Cmd-Shift-R | Restaurar os ajustes neutros |
| Cmd-L / Cmd-R | Girar a imagem 90° para a esquerda / para a direita (menu Imagem; vale de qualquer foco, com uma fonte aberta). A rotação fica guardada por imagem nos dados do app |
| Clique, arrastar, duplo clique (barra do painel de ajustes) | Clicar na trilha pula para o ponto; arrastar ajusta (a parte visível acompanha na resolução da tela; a base da página inteira vem ao soltar); duplo clique na trilha (o 1º clique já leva o marcador ao ponteiro) ou no rótulo volta ao neutro |
| Setas cima/baixo, Shift, Enter, Esc (caixa numérica de uma barra) | Clicar na caixa e digitar o valor; Enter confirma (fora da faixa vai ao limite; texto que não é número não muda nada), Esc devolve o valor e o foco à imagem; as setas mudam de 1 em 1, com Shift de 10 em 10 |
| Shift+arrastar (ou arrastar com a ferramenta caixa ligada) | Grifo retangular (a próxima nota fica ligada a ele) |
| Option+arrastar (ou arrastar com a ferramenta seta ligada), Option+clique | Grifo seta: da cauda (onde apertou) até a ponta (onde soltou); o clique faz uma seta curta com a ponta ali |
| Clique num grifo | Seleciona o grifo (clique fora tira a seleção) |
| Delete / Backspace (imagem em foco, grifo selecionado) | Apaga o grifo selecionado |
| Esc (imagem em foco, ferramenta de grifo ligada) | Desliga a ferramenta (a seleção do grifo fica) |
| Esc (imagem em foco, grifo selecionado) | Tira a seleção do grifo |
| Cmd-Z | Desfazer: com a imagem em foco devolve o último grifo apagado (mesmo id e cor). Na nota rápida não faz nada (a caixa não tem desfazer) |
| Cmd-K (ou `/` com a imagem em foco) | Foco na nota rápida (clicar na caixa também); a caixa ganha a borda azul |
| Cmd-Shift-K, Esc (na nota) | Foco de volta na imagem (borda azul na imagem), sem apagar o texto |
| (digitar na nota) | Salva sozinho (ver "Nota rápida e salvamento automático"); `[]` no início cria tarefa |
| Enter, Shift-Enter (na nota) | Nova linha (a caixa cresce até 8 linhas e depois rola). Linha longa quebra sozinha na largura da caixa (só na tela: o arquivo guarda a linha inteira) |
| Cmd-Enter (na nota) | Finaliza: fixa o id, limpa a caixa e devolve o foco à imagem (pronta para a próxima nota) |
| Setas, Home/End (na nota) | Andar pelo texto; cima/baixo seguem as linhas da tela (também as quebradas) e lembram a coluna; Home/End e Cmd-←/→: início/fim da linha da tela; Cmd-↑/↓: início/fim do texto; Option-←/→: palavra. Com Shift, seleciona |
| Backspace, Delete, Cmd-A, Cmd-X, Cmd-C, Cmd-V (na nota) | Edição do texto (selecionar tudo, recortar, copiar, colar) |
| Cmd-1 / Cmd-2 / Cmd-3 | Abrir/fechar a gaveta de notas / fontes / tarefas (a ordem dos ícones; de qualquer foco). Abrir dá o foco à gaveta (borda azul) |
| Clique numa nota ou tarefa (gaveta) | Abre a imagem, carrega o texto na caixa e dá o foco à caixa para editar (a nota que estava na caixa é salva antes; até o texto chegar a caixa fica vazia e desligada, a barra diz "carregando nota…" e o foco espera na gaveta) |
| Setas cima/baixo, Enter (gaveta em foco) | Escolher a linha; Enter faz o mesmo que o clique (na de fontes, abre a fonte) |
| Backspace (notas/tarefas, gaveta em foco) | Excluir a linha escolhida (nota ou tarefa): abre a confirmação |
| Tab ou Setas, Enter ou espaço, Esc (confirmação de excluir) | Tab e as setas para os lados trocam o botão em foco (abre em "Cancelar"); Enter ou espaço aciona o botão em foco, então só exclui com o foco em "Excluir"; Esc cancela. Clicar em "Excluir" exclui |
| `x` ou espaço (tarefas, gaveta em foco) | Marcar feita / reabrir |
| Esc (gaveta em foco) | Fecha a gaveta (o foco volta à imagem) |
| Setas cima/baixo, Enter, Esc, `o`, `n` (seletor de vault) | Escolher, abrir o vault, fechar, "Abrir pasta…", "Criar vault aqui" |
| Enter / Esc (caixa "Renomear" da gaveta de fontes) | Renomear a fonte / cancelar |

As teclas de uma letra só (`x`, `w`…) não valem enquanto a nota rápida, a caixa
"Renomear" ou a caixa numérica de uma barra tem o foco: lá elas são texto. As teclas da gaveta (setas, Enter, `x`, Backspace) só valem
com a gaveta em foco (abrir pelo ícone ou Cmd-1/2/3, ou clicar dentro dela); com a imagem em foco não mexem na
gaveta. Com a confirmação de excluir aberta, ela fica com as teclas. Clicar na
imagem, Cmd-Shift-K ou Esc (na nota) voltam à imagem; Cmd-K vai à nota. Cmd-Q, Cmd-O, Cmd-Shift-V, Cmd-K,
Cmd-Shift-K e Cmd-1/2/3 valem de qualquer foco; Cmd-Z e Backspace dependem do foco (acima).

Esta tabela é conferida por teste (`menus::tests::readme_lista_toda_tecla_registrada`): todo atalho registrado no
app (os `KeyBinding`, as teclas simples dos menus e as teclas da gaveta, da nota, do seletor, da imagem e da
confirmação) precisa aparecer aqui.

## Menus

Tudo o que os botões, as barras e as teclas fazem também está na barra de menus (a do macOS; no Windows, uma barra no alto da janela, com os mesmos itens, desligados pelos mesmos critérios, mas sem o menu com o nome do app: Configurações e Sair ficam no fim de Arquivo e Sobre no fim de Ajuda; clicar fora a fecha, e com um menu aberto, passar o mouse em outro título troca de menu; pelo teclado, como no Windows: segure o **Alt** e aperte a letra do menu (as letras de acesso aparecem sublinhadas enquanto o Alt está apertado); com o menu aberto, **← →** trocam de menu, **↑ ↓** de item, **Enter** aciona, uma **letra** aciona o item de mesma letra e **Esc** volta um nível) (o mesmo código: o item,
o botão e a tecla chamam o mesmo método; os itens de brilho, contraste, nitidez, passa-alta e limiar movem a barra
pelos passos de sempre). Os itens que não se aplicam ficam desligados (sem fonte aberta, sem grifo selecionado,
nada a desfazer, nenhuma linha escolhida na gaveta). Teclas de uma letra aparecem no rótulo ("Ajustes de imagem
(F)") porque só valem com o foco certo; os atalhos com Cmd aparecem à direita, como de costume.

- **Marginalia**: Sobre o Marginalia (janela própria com o ícone, a versão, o desenvolvedor, o link para a Genealogia do Ceará e os direitos reservados; Esc fecha), Configurações… (Cmd-,), Sair do Marginalia (Cmd-Q).
- **Arquivo**: Abrir fonte… (Cmd-O), Abrir vault… (Cmd-Shift-V), Fechar fonte.
- **Editar**: Desfazer (Cmd-Z); Recortar, Copiar, Colar, Selecionar tudo (Cmd-X/C/V/A, na nota rápida ou na caixa
  "Renomear"); Excluir grifo (Backspace); Excluir nota/tarefa… (com a gaveta em foco; sem atalho no menu, a tecla é Backspace).
- **Exibir**: Notas, Fontes, Tarefas (Cmd-1/2/3); Ajustes de imagem (Cmd-F); Barra de ferramentas; Aumentar zoom (`+`), Diminuir zoom
  (`-`), Ajustar à janela (`0`).
- **Navegar**: Imagem anterior (←), Próxima imagem (→); Focar nota (Cmd-K), Focar imagem (Cmd-Shift-K).
- **Imagem**: Girar à esquerda/à direita (Cmd-L/Cmd-R); Grifo caixa e Grifo seta (ligam e desligam a
  ferramenta, sem atalho); Cor dos grifos (abre o menu de cores, sem atalho); aumentar/diminuir brilho (Cmd-]/Cmd-[), contraste (Cmd-Shift-]/Cmd-Shift-[), nitidez, passa-alta e limiar; Inverter (Cmd-I),
  Tons de cinza (Cmd-G), Limiar (Cmd-T); Restaurar ajustes (Cmd-Shift-R).

- **Ajuda**: Ajuda (`?`), a tela com todos os comandos por assunto; Buscar atualizações… (checa agora se há versão nova, veja abaixo).

"Fechar fonte" guarda a posição (reabrir a fonte volta ao mesmo quadro) e termina a nota em edição, que fica salva.

## Aviso de versão nova

Ao abrir, o app consulta em segundo plano (no máximo uma vez a cada 24 h) a última **release publicada** do repositório no
GitHub. Se houver versão maior que a instalada, abre uma janelinha com **Baixar** (abre a página da release no navegador),
**Agora não** e **Pular esta versão** (só volta a avisar de uma versão ainda maior). O app nunca baixa nem instala nada
sozinho. Nada seu é enviado: é um GET anônimo, só com o nome e a versão do app no `User-Agent`. Sem internet, a checagem
automática fica calada. **Ajuda → Buscar atualizações…** checa na hora e sempre responde (há versão nova, você já está na
última, ou não foi possível verificar). Para desligar a checagem automática: Configurações → "Avisar sobre versão nova ao
abrir". Rascunhos de release não contam: o aviso só aparece depois que a release é publicada.

**Para quem publica:** suba `version` em `crates/ui/Cargo.toml`, crie a tag `v` + a mesma versão (`v0.2.0`; o workflow
recusa se divergirem), espere o workflow gerar o **rascunho** da release, teste os zips e clique em **Publish release**.

## Registros

Uma fonte (caixa, livro, PDF) costuma guardar vários documentos. Um **registro** é um intervalo de imagens da fonte com um
nome ("Inventário de João Silva"); serve para agrupar o que você anotou sobre ele. É opcional: sem nenhum registro tudo
fica como sempre foi.

- **Marcar**: uma faixa no alto da imagem diz onde você está, citando o intervalo do registro pela posição das imagens
  na fonte, como a barra de status (`img 3 – img 9`; em PDF, `pág 5 – pág 9`), e não pelo nome dos arquivos. Os botões
  da faixa são só ícones, com dica ao passar o mouse, sempre nos mesmos lugares (apagados quando não valem para a
  imagem atual): início, fim, estender o registro anterior até aqui (seta até a barra), lápis (renomear) e lixeira
  (cancelar/excluir). Fora de qualquer registro ela oferece "Marcar início de
  registro aqui": pede o nome e o registro já fica salvo, sem fim ("sem fim ainda"). Mais adiante, "Marcar fim aqui".
  Depois de marcado o fim, o botão passa a ser "Ajustar fim aqui" (marque de novo em outra imagem); numa imagem
  depois do fim, a faixa oferece "Estender <registro> até aqui". "Renomear" troca só o nome. "Cancelar" descarta o registro
  que você acabou de criar (ainda sem fim); qualquer outro registro tem "Excluir", que pede confirmação. Mudar o *início* de
  um registro ainda não tem botão: exclua e marque de novo (ou edite o arquivo em `registros/`). Excluir um registro **nunca** apaga notas ou tarefas.
- **Imagem de fronteira**: o fim de um registro e o início do seguinte podem ser a mesma imagem (comum em documentos
  antigos). Uma nota ou tarefa nessa imagem fica, por padrão, no registro que termina ali; o botão "Mover para
  "<registro>"" na linha dela a leva para o outro ("Voltar para…" desfaz). Fora disso nada é gravado na nota: o
  pertencimento é sempre calculado pela posição da imagem.
- **Regras**: registros não se sobrepõem nem se aninham (só dividem a imagem de fronteira). Marcar um início dentro de
  outro registro, ou um fim antes do início ou por cima de outro registro, é recusado com um aviso. Um registro sem fim
  vale até o início do próximo (ou o fim da fonte). Um vault editado à mão com intervalos inválidos abre normalmente: o
  registro aparece "intervalo inválido" e não agrupa nada até você corrigi-lo ou excluí-lo.
- **Gavetas**: Notas e Tarefas são uma árvore: cada fonte é um cabeçalho (▾/▸ recolhe e mostra a contagem), com os
  registros dela primeiro (cada um com o intervalo e recolhível) e, depois, as notas que não estão em nenhum registro,
  na ordem das imagens (na mesma imagem, na ordem de criação). Por isso o card de uma nota mostra só `arquivo · p. N`:
  a fonte é o cabeçalho. Tarefas sem imagem ficam num grupo "Sem fonte" no fim. Os filtros valem dentro da árvore.

## Gavetas

O painel das gavetas se redimensiona arrastando a borda direita dele (entre 220 e 700 px; a imagem cede o mesmo tanto). A largura fica guardada nos dados do app e volta igual na próxima vez; numa janela estreita o painel cede espaço sozinho.

- **Notas**: três filtros que se combinam (todos têm de valer), um por linha, no alto da gaveta: a **busca** no texto inteiro da nota (sem diferenciar maiúsculas; o ✕ limpa; Enter ou Esc devolvem o foco à lista), as **fontes** (um menu com caixas de marcar: valem as notas de qualquer fonte marcada; nenhuma marcada = todas) e as **etiquetas** (os `#botões` ligam e desligam; ligando várias, aparecem só as notas que têm *todas*). O título mostra o contador ("2 de 4" com filtro, "4 notas" sem) e, sem resultado, a gaveta diz "Nenhuma nota com esses filtros". Trocar de vault zera os três. As listas de notas e de tarefas mostram uma barra de rolagem fina quando passam da altura da gaveta. Editar o texto de uma nota (inclusive acrescentar `#etiqueta`) atualiza a lista na hora. Cada linha tem só "Excluir" (ícone de lixeira); clicar na linha abre a nota para editar. A linha da nota aberta tem a borda laranja.
- **Tarefas**: filtros abertas/feitas; o quadrado à esquerda do texto (vazio = pendente, com visto = feita) marca feita ou reabre; a lixeira (e "Mover para…", quando há) aparece ao passar o mouse.
  Uma tarefa cuja nota foi excluída continua, marcada "nota excluída".
- **Fontes**: clicar na linha abre a fonte. Um ícone antes do nome diz o que ela é (imagem avulsa, pasta de
  imagens, PDF ou zip). Dois botões de ícone na linha: "Renomear" (lápis; o vault inteiro é reescrito: notas,
  tarefas, notas e grifos seguem o nome novo; se parar no meio, nada se perde e renomear de novo com o mesmo nome
  termina) e "Apontar nova localização…" (seta circular; vale também com o caminho existente). Troca de vault, nova localização e renomear são um de cada vez: pedir outro no meio mostra
  "aguarde".
- **Excluir** (lixeira da linha ou Backspace com a gaveta em foco) apaga o arquivo de verdade e não tem desfazer:
  por isso pede confirmação, num quadro ("Excluir esta nota?" ou "esta tarefa?", com o começo do texto) com "Cancelar" e "Excluir" que diz o que fica para trás (tarefas
  que citam a nota continuam; o grifo ligado permanece na imagem). O foco abre em "Cancelar", e Enter só exclui
  com o foco em "Excluir".

## Onde ficam os dados

- **Vault** (o resultado da pesquisa, a pasta que você escolhe; pode estar no git, no Drive ou no Synology):
  `fontes/` (uma `.md` por fonte: nome e tipo), `notas/` e `tarefas/` (markdown com frontmatter YAML; as `#etiquetas` ficam no
  próprio texto), `registros/` (uma `.md` por registro: nome, fonte, início e fim; a pasta só nasce no primeiro registro) e `grifos/` (um `.json` por fonte, coordenadas relativas 0..1,
  com a cor de cada grifo). Pode ter vários vaults; um fica ativo por vez.
  **Vault do formato antigo** (com a pasta `pistas/`, de quando as notas se chamavam "pistas"): o app se recusa a abrir e
  manda rodar uma vez `python3 scripts/migrar_pistas_para_notas.py <vault>` (com `--simular` só mostra o que faria): a
  pasta `pistas/` vira `notas/` e a chave `pista:` das tarefas vira `nota:`; o resto não muda. É um script avulso, fora do
  app; faça uma cópia do vault antes. (Para um vault ainda mais antigo, com `estado:` nas pistas e notas promovidas, use o
  `scripts/migrar_vault_antigo.py` do commit `5272d78`, antes desta migração.)
  **Chaves dentro dos arquivos:** o frontmatter, os `.json` de grifos e o `appdata.json` usam chaves em inglês
  (`image`, `source`, `state: open|done`, `kind`, `highlight`, `"kind": "rectangle"|"pointer"`...). Arquivos gravados com as
  chaves antigas em português continuam sendo lidos, e passam para as novas chaves quando o app os regrava. Os nomes das
  pastas (`fontes/`, `notas/`, `tarefas/`, `registros/`, `grifos/`) continuam em português.
- **Dados do app** (por máquina, nunca no vault): `~/Library/Application Support/Marginalia/appdata.json` (macOS) ou `%APPDATA%\Marginalia\appdata.json` (Windows) — o
  caminho de cada fonte nesta máquina (por vault), os ajustes de imagem e a rotação por imagem, a cor dos grifos, o último
  vault, a lista dos vaults conhecidos e o que o aviso de versão guarda (ligado ou não, quando checou, versão pulada) — e `recuperacao.json` (texto da nota ainda não gravado no vault). A cópia
  de zips em cache fica em `.../Marginalia/cache/` e é apagada ao sair. Apagar os dados do app não perde nada da
  pesquisa já gravada; `MARGINALIA_APPDATA` troca a pasta (o smoke usa uma temporária).

Se o caminho de uma fonte não existir (NAS montado em outro lugar, zip movido) ou a leitura falhar no meio da
sessão, o app mostra "Fonte indisponível" / "Fonte não encontrada" com "Tentar de novo" e "Apontar nova
localização…". A nova localização só é aceita se for do mesmo tipo (pasta ou zip) e tiver a imagem que vai
reabrir e pelo menos metade das imagens que o vault cita nessa fonte (de uma amostra de até 200); se nada no vault
cita a fonte, o app pergunta, com o caminho e quantas imagens ele tem, antes de usar. O vault não muda. O vault é relido
no fundo quando a janela volta ao foco (edições feitas no Obsidian aparecem).

## Limites conhecidos

- TIFF CCITT Group 3/4 (fax) e TIFF com paleta não abrem (mostram o erro "formato não suportado"); ver
  [`docs/formatos.md`](docs/formatos.md) para a lista completa.
- A orientação EXIF de JPEG/TIFF é ignorada: a foto aparece como foi gravada (de lado, se a câmera girou); gire-a
  com Cmd-L/Cmd-R. Os grifos, as notas e os enquadramentos guardados são sempre relativos à imagem como foi
  decodificada, sem a rotação (o vault não muda ao girar): girar uma imagem leva os grifos junto e uma nota abre
  no mesmo ponto da imagem.
- Parte da E/S do vault roda na thread da interface (gravar grifos, excluir, marcar tarefa):
  num NAS lento a janela pode parar um instante nessas ações. A abertura de fontes, a leitura das gavetas, a
  decodificação e as gravações da nota rápida rodam no fundo.
- Tags YAML (`!foo valor`) no frontmatter não são preservadas: tornam aquela nota, tarefa ou fonte ilegível
  para o app (ela aparece como ilegível na gaveta); o resto do vault segue normal.
- A nota rápida não tem desfazer de texto (Cmd-Z não faz nada nela); recortar, copiar, colar, selecionar e
  quebra de linha funcionam.
- Os menus do macOS, as dicas (tooltips), o arrastar e soltar a partir do Finder e o desenho do quadro de
  confirmação foram conferidos por testes e pelo smoke, não visualmente na tela.
- As chaves `draft` e `renaming_from` no frontmatter de notas e tarefas são reservadas: uma nota escrita à mão
  com `draft: true` é tratada como rascunho (o id dela é renomeado pelo texto na primeira finalização).
- Sem zoom por pinça (use a roda/trackpad de rolagem ou `+`/`-`).
- Ajustes: o tile e a base podem ficar até 1 pixel da tela fora de alinhamento um do outro (a base é outra
  reamostragem da página); isso nunca foi conferido na tela. O desenho de verdade das duas camadas (a base por
  baixo, o tile no lugar certo, sem piscar) foi conferido por testes e pelo smoke (tamanhos, regiões e camadas),
  não visualmente.
- PDFs são renderizados numa resolução fixa: aproximar o zoom amplia esses pixels e não traz mais detalhe.
- Cada gaveta desenha no máximo 300 linhas ("Mostrando 300 de N — use os filtros").
- Ao abrir uma fonte o nome é o da pasta/zip (com " (2)", " (3)"… se já existir outra com esse nome); não há campo
  para digitar o nome na abertura — use "Renomear" na gaveta de fontes depois.

## Créditos

Ícones: [Unicons](https://github.com/Iconscout/unicons) da Iconscout, licença Apache-2.0 (texto em
`crates/ui/assets/icons/LICENSE-unicons.txt`).

## Licença

O código é distribuído sob a [Apache-2.0](LICENSE). Pode ser usado, modificado e redistribuído, inclusive em projetos
comerciais, desde que se **mantenha a atribuição**: o aviso de copyright, o texto da licença, a indicação das alterações
feitas e o arquivo [`NOTICE`](NOTICE) (nome do autor e site) em toda redistribuição, original ou derivada. A PDFium que
acompanha o app tem a licença própria dela (`LICENSE-pdfium.txt`, dentro do pacote).
