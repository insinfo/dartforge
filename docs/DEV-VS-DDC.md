# Compilador de desenvolvimento: DartForge (`dartforge serve`) × DDC (`webdev serve`)

Medido em 2026-10-13 num Apple M4 (10 núcleos, 16 GB), macOS 27.2, **com a
máquina carregada por outros agentes** (carga de 1 min entre 25 e 220 durante
as medições; cada linha das tabelas traz a carga). Pergunta do proprietário:
"o nosso compilador de desenvolvimento estilo DDC é mais rápido e mais
eficiente que o DDC oficial?".

Reproduzir: `scripts/compare-dev-ddc.mjs` (comandos `cold`, `edits`, `node`,
`report`; o cabeçalho do script documenta as opções). Os resultados brutos
desta rodada ficaram em JSON lines no scratchpad da sessão; as tabelas abaixo
saem de `node scripts/compare-dev-ddc.mjs report`.

## Método

* **Ferramentas.** Do nosso lado, `dartforge serve web/main.dart -o … --web web`
  (crates/dev + crates/emit_js, binário release do commit `747cb59c`, motor de
  build nativo para os templates do ngdart). Do lado oficial, `dart run webdev
  serve web:PORTA --auto refresh` (webdev 3.7.1, build_runner 2.4.15,
  build_web_compilers 4.1.1 — daemon do build_runner + workers persistentes de
  `dartdevc` e `kernel_worker` com `--use-incremental-compiler` e
  `--reuse-compiler-result`; o webdev ainda acrescenta
  `ddc=generate-full-dill=true`). Dart SDK 3.6.2, Chrome 155 headless, Node 22.11.
* **Projetos.** `limitless_ui/example` (galeria; 186 arquivos em `lib/` mais o
  pacote `limitless_ui` por `path: ..`) e `new_sali/frontend` (284 arquivos,
  ngdart 8.0.0-dev.4, 2.970 bibliotecas no fecho). Cada ferramenta trabalha
  numa **cópia própria** do projeto (os originais não foram tocados). Na cópia
  do `new_sali` foi preciso acrescentar `webdev: 3.7.1` às dev_dependencies
  (`dart pub add --offline`), o que trocou `mime`, `shelf_web_socket` e
  `vm_service` de versão; a cópia do DartForge recebeu o mesmo
  `package_config.json`, de modo que as duas compilam os mesmos pacotes.
* **Edições** (a mesma sequência nas duas cópias, ferramentas alternadas a cada
  edição para repartir a variação de carga): *corpo* — muda um `print('marca')`
  no construtor do componente raiz/da página inicial; *API pública* — adiciona
  uma função pública nova (`benchApiN`) numa biblioteca exportada pelo barrel
  que quase todo o projeto importa (`currency_input_formatter.dart` do
  limitless_ui; `mem_cache.dart` do new_sali) e passa a chamá-la do
  construtor; *template* — insere um `<span>` com a marca no `.html` do
  componente. O tempo medido vai da escrita do arquivo até a **marca nova
  aparecer na página recarregada** (console por CDP, ou o DOM), ou seja, código
  novo de fato executando — não só "o servidor terminou".
* **Memória.** Pegada física (`footprint`, `phys_footprint` e o pico de vida
  `phys_footprint_peak`) somada sobre **todos** os processos da ferramenta: o
  nosso é um processo só; o do webdev é o cliente webdev, o daemon do
  build_runner (que se desacopla) e os workers (que também ficam com pai 1) —
  achados por linha de comando entre os processos criados depois da linha de
  base. RSS está registrado mas não é usado: no macOS sob pressão ele chegou a
  marcar 16 MB para um daemon com 1,1 GB de pegada.
* **CPU por edição.** Soma do tempo de CPU (`ps`) dos processos da ferramenta
  antes e depois da edição — o número mais estável com a máquina carregada.
* **JS servido.** Os scripts que o Chrome baixou numa carga sem cache,
  rebaixados pelo script e comprimidos com gzip nível 6.

## Resultados

### 1. Início a frio

Não houve tempo de rodar o comando `cold` completo (intercalado, várias
repetições) antes do fim do dia; os números abaixo são de rodadas isoladas e
servem de ordem de grandeza, com a carga ao lado.

| Projeto | Ferramenta | Situação | Pronto para servir | Pegada após pronto | Carga |
|---|---|---|---|---|---|
| limitless_ui | webdev | `.dart_tool/build` apagado (script de build já pré-compilado) | **103 s** (build: 1 min 27 s, 5.787 saídas) | — | 50–80 |
| limitless_ui | DartForge | saída vazia, cache do SDK pronto, `dev --once` | **4,9 s** (4,2 s CPU; pico 590 MB) | 563 MB | ~50 |
| new_sali | webdev | `.dart_tool/build` apagado | **340 s** (+6 s até a página renderizar) | **7,0 GB em 21 processos** | 34–45 |
| new_sali | DartForge | saída vazia, cache do SDK pronto, `dev --once` | **19,4 s** (8,2 s CPU; pico 1,08 GB) | — | ~33 |

Ressalva importante: o DartForge **ainda depende de um `build_runner build`
prévio** para ~10–16 ações por projeto que o motor nativo não produz (o
`sass_builder`, cuja saída "ainda não foi verificada byte a byte", e algumas
saídas do ngdart): sem `.dart_tool/build/generated` a sessão falha com
`could not read …/app_component.template.dart`. Nas medições a cópia do
DartForge recebeu só os `.dart`/`.css` gerados (13–17 MB). Num checkout limpo,
portanto, o primeiro início do DartForge hoje custa um `build_runner build`
(da ordem do próprio webdev a frio) — até o Sass nativo ser verificado.

### 2. Edição → página recarregada com o código novo

`limitless_ui`, 7 edições de cada tipo por ferramenta (21 no total), mediana
[mín–máx]:

| Edição | Ferramenta | Edição → código novo visível | Edição → recarga começa | CPU por edição | Carga |
|---|---|---|---|---|---|
| corpo | DartForge | 5,3 s [2,0–7,8] | 4,4 s | **1,44 s** | 44 |
| corpo | webdev | 5,3 s [2,8–8,0] | 4,3 s | 4,91 s | 44 |
| API pública | DartForge | **10,6 s** [8,1–14,7] | 9,9 s | **3,48 s** | 45 |
| API pública | webdev | 40,5 s [19,5–74,8] | 38,1 s | 40,9 s | 44 |
| template | DartForge | **4,0 s** [2,4–5,7] | 3,3 s | **1,34 s** | 42 |
| template | webdev | 7,0 s [3,5–8,1] | 5,6 s | 4,32 s | 43 |

`new_sali` (poucas edições — a máquina chegou a carga 220 e cada edição do
webdev levou minutos):

| Edição | Ferramenta | Edição → build pronto | Edição → código novo visível | CPU | Carga |
|---|---|---|---|---|---|
| corpo | DartForge | 4,3 s | **5,2 s** | 2,6 s | 41 |
| corpo | webdev | 24,2 s | nunca (ver abaixo) | 35,2 s | 37 |
| corpo | DartForge | 26,0 s | 28,6 s | 3,4 s | 72 |
| corpo | webdev | 48,5 s | nunca | 79,4 s | 49–107 |
| API / template | — | não medido (interrompido no fim do dia) | | | |

Um corpo editado no `new_sali` faz o build_runner recompilar
`lib/new_sali_frontend.ddc.module` — o barrel e tudo o que forma ciclo com ele
viram **um único módulo DDC enorme** — e a primeira edição depois de abrir o
servidor levou 3 min 28 s (1.374 saídas). **O live reload do webdev não
funciona neste app**: o roteador troca a URL para `/login` (roteamento por
caminho) e, ao recarregar, o servidor do webdev responde `404 Not Found` para
`/login` — não tem fallback para `index.html`. O nosso servidor tem esse
fallback (só para navegação, `Accept: text/html`) e recarrega certo. No
webdev o desenvolvedor teria de navegar de volta à raiz à mão.

Leitura: com a carga desta máquina o **tempo de parede** de uma edição de corpo
ficou empatado no limitless_ui (os dois disputam CPU com ~40 processos), mas
o **trabalho** não: o DartForge gasta 3,4× menos CPU numa edição de corpo,
3,2× menos num template e **12× menos numa mudança de API pública**, onde o
DDC recompila em cascata os módulos que dependem do resumo (`.ddc.dill`)
alterado. Em máquina ociosa a diferença de parede tende à de CPU (na rodada de
teste, carga ~30: corpo 1,9 s × 9,1 s; API 5,0 s × 22,1 s; template 1,7 s ×
1,9 s).

### 3. Memória

| Projeto | Ferramenta | Após iniciar | Ao longo das edições (mediana [mín–máx]) | Fim | Soma dos picos | Processos |
|---|---|---|---|---|---|---|
| limitless_ui | DartForge | 476 MB | 533 MB [477–552] (21 edições) | 520 MB | 587 MB | 1 |
| limitless_ui | webdev | 5.151 MB | 3.927 MB [2.920–5.398] | **5.398 MB** | **7.129 MB** | 22 |
| new_sali | DartForge | 0,84–1,06 GB | 0,75–0,95 GB | — | 1,09 GB | 1 |
| new_sali | webdev | 5,2–6,0 GB | 4,9–7,0 GB | 7,0 GB | 7,6 GB (a frio) | 19–21 |

O DartForge fica num platô (≈ +50 MB nas primeiras edições, depois estável);
o webdev cresce de 2,9 para 5,4 GB em 21 edições do limitless_ui, em 22
processos (≈ 8 workers `dartdevc`/`kernel_worker` de 300–450 MB cada, o daemon
com 0,8–1,2 GB). **10× menos memória**, num processo só.

### 4. JavaScript servido (carga fria da página)

| Projeto | Ferramenta | Scripts | Bruto | gzip |
|---|---|---|---|---|
| limitless_ui | DartForge | 455 | 51,4 MB | **4,17 MB** |
| limitless_ui | webdev | 333 | 47,5 MB | 5,24 MB |
| new_sali | DartForge | 585 | 93,0 MB | **7,84 MB** |
| new_sali | webdev | 382 | 83,1 MB | 9,94 MB |

O total bruto é ~8–12% maior no nosso (um módulo ES por biblioteca: mais
arquivos; o mesmo `dart_sdk.js` do DDC de 7 MB nos dois), mas comprime 20%
melhor. As duas saídas usam o `dart_sdk.js` gerado pelo DDC 3.6.2, então a
diferença é só o código do usuário e dos pacotes.

### 5. Eficiência em tempo de execução

Carga da página no Chrome headless (5 cargas alternadas, ms desde a navegação
até o app renderizado):

| Projeto | Ferramenta | Sem cache HTTP | Com cache HTTP | Tempo de script | Heap JS |
|---|---|---|---|---|---|
| limitless_ui | DartForge | **666** [451–2046] | **606** | 94 ms | 40 MB |
| limitless_ui | webdev | 2.863 [2092–4940] | 2.594 | 174 ms | 55 MB |
| new_sali | DartForge | **1.063** [1051–1669] | **1.025** | 133 ms | 71–92 MB |
| new_sali | webdev | 4.171 [3945–35600] | 3.262 | 254 ms | 211 MB |

O app fica pronto **3–4× mais cedo** com o nosso: módulos ES nativos carregados
em paralelo pelo navegador contra o carregamento sequencial do RequireJS +
injeção de metadados de depuração do dwds. A troca de rota (24 rotas × 3
voltas no limitless_ui) cabe em dois quadros nas duas saídas (≈33 ms por rota);
tempo de tarefas no thread principal 858 ms (nosso) × 916 ms (DDC) — empate.

Programas de `bench/performance` compilados pelos dois e executados no Node
(mediana das rodadas 2–6, 3 execuções alternadas). **Os resultados impressos
são idênticos aos do DDC em todos os núcleos** (onde diferem da VM é por
semântica web legítima — `String.hashCode`, formatação de `double` — nas duas
saídas igualmente):

| Programa | Núcleo | DDC (ms) | DartForge (ms) | DartForge/DDC |
|---|---|---|---|---|
| calls | shapes / closures / fib | 4 / 17 / 13 | 2 / **28** / 5 | 0,50 / **1,65** / 0,38 |
| escaping_objects | trees / linked_list | 44 / 30 | 50 / 39 | 1,14 / **1,30** |
| json | 9 núcleos | 17–165 | 19–171 | 0,93–1,27 (decode_medium 1,27) |
| numeric | mandelbrot / collatz | 6 / 1080 | 6 / 996 | 1,00 / 0,92 |
| objects_in_collections | 4 núcleos | 39–171 | 37–168 | 0,95–1,12 |
| strings | build / hashes | 57 / 24 | 59 / 24 | 1,04 / 1,00 |
| temporary_objects | points / point_sum | 60 / 48 | 59 / 45 | 0,98 / 0,94 |
| typed_data | product_f64 / fnv_bytes | 297 / 485 | 289 / 477 | 0,97 / 0,98 |

`collections.dart` não compila no DDC (literal `9223372036854775807` não é
representável em JS) e ficou fora. No geral, empate (mediana ≈1,0), com dois
pontos fracos nossos: **closures (1,65×)** e **listas encadeadas/árvores
(1,14–1,30×)**, e dois fortes: chamadas diretas e recursão (`fib` 0,38×,
`shapes` 0,50×).

## Veredito

* **Edição (trabalho):** o DartForge faz a mesma recarga com 3–12× menos CPU;
  na mudança de API pública é 4× mais rápido mesmo na parede (10,6 s × 40,5 s
  sob carga 45). Em edição de corpo, sob esta carga, a parede empatou — e o
  nosso custo de corpo (1,4–2,6 s de CPU) é bem maior do que deveria (ver
  melhorias 1).
* **Memória:** 0,5 GB num processo contra 4–7 GB em ~20 processos — 10×.
* **Início a frio:** 20–25× mais rápido (4,9 s × 103 s; 19 s × 340 s), **com a
  ressalva** de ainda precisar dos gerados do build_runner para o Sass e
  algumas saídas do ngdart.
* **Página:** app pronto 3–4× mais cedo; JS 20% menor em gzip, ~10% maior bruto.
* **Execução:** equivalente ao DDC (mesma saída em todos os benchmarks;
  desempenho ≈1,0×, com closures 1,65× mais lentas).
* **Robustez:** o live reload do webdev quebra no new_sali (404 em `/login`);
  o nosso recarrega.

## O que melhorar no nosso compilador de desenvolvimento (por impacto)

1. **A edição de corpo dispara o motor de build inteiro.** No new_sali, um
   `print` no construtor: total 4,2 s, dos quais **2,8 s em "generation
   stages"** — 454 ações executadas (442 nativas do ngdart, 208 unidades
   regeneradas, 882 consultas reavaliadas) **para 0 saídas alteradas** — e só
   ~0,65 s de compilação de fato (outline 255 ms, corpos 224 ms, emissão
   152 ms). A consulta `LibrarySource` usa o hash do texto inteiro da
   biblioteca; os templates que dependem dela via barrel deviam depender só do
   hash de API + diretivas/anotações (o ramo `api` que já existe). Corte
   antecipado aqui deve levar a edição de corpo para < 1 s.
2. **`load` refaz o outline do programa todo a cada edição** (declarações +
   escopos ≈ 210 ms com 1 unidade alterada em 3.361) e a recarga das unidades
   geradas custa 0,66 s mesmo sem saída alterada.
3. **Servidor sem cache HTTP:** `Cache-Control: no-store` e `Connection: close`
   em cada resposta — cada recarga baixa de novo os 455–585 módulos (51–93 MB)
   em conexões novas. ETag/`If-None-Match` (304) ou URLs com hash do conteúdo +
   `immutable`, e keep-alive, cortam a fase "recarga → código novo" (hoje
   0,4–1,2 s) e o tráfego. Barato.
4. **Independência do build_runner:** verificar o `sass_builder` nativo e
   cobrir as saídas do ngdart que ainda vêm do fallback (10 no limitless_ui,
   16 no new_sali), para que o primeiro início num checkout limpo não precise
   do build_runner.
5. **Closures 1,65× mais lentas que o DDC** e alocação de objetos encadeados
   1,1–1,3× (ver o JS emitido para fechamentos/tear-offs em `emit_js`).
6. Volume bruto 8–12% maior: muitos módulos pequenos; agrupar bibliotecas
   pequenas de pacote por pacote reduziria requisições.

## Ressalvas

* Máquina compartilhada e muito carregada (25–220); os tempos de parede têm
  dispersão grande — leia as colunas de CPU e as medianas, e a carga ao lado.
* O `new_sali` teve só 2 edições de corpo por ferramenta; API e template não
  foram medidos nele. O comando `cold` completo (intercalado, ≥3 repetições,
  com `build_runner build` e os inícios mornos) **não foi executado** — a
  tabela de início a frio vem de rodadas isoladas.
* A "primeira edição" do webdev inclui subir os workers; está nas medianas.
* A tour de rotas mede quase só cadência de quadros: não é um benchmark de CPU
  pesado; a comparação de CPU de execução é a do Node.
* O DDC compilou com `generate-full-dill=true` (padrão do `webdev serve`);
  `build_runner build` puro, sem esse dill, seria um pouco mais leve.

## Rodada de 2026-10-31: checkout limpo e worker residente

Mac mini M4 (16 GB), macOS 27.2, Dart SDK 3.6.2, webdev 3.7.1, Chrome
headless; máquina compartilhada (carga 6–45, anotada em cada linha bruta).
Binário do DartForge: `dartforge` release da árvore de `6fb7115d` (o perfil
de desenvolvimento não muda desde `e510672a`). Script: o mesmo
`scripts/compare-dev-ddc.mjs` (`cold` com uma repetição por variante;
`edits` com 3 rodadas no `limitless_ui` e 2 no `new_sali`); resultados
brutos no *scratchpad* da sessão.

**Checkout limpo de verdade.** Cada ferramenta recebeu uma cópia própria
dos projetos **sem** `.dart_tool/`, `build/` nem saídas geradas (`rsync`
que exclui só isso), mais `dart pub get --offline`; o `new_sali` ganhou
`webdev: 3.7.1` nas `dev_dependencies` nas duas cópias (mesmo
`pubspec.lock`).

### 1. O DartForge ainda não sobe num checkout limpo

`dartforge dev`/`serve` falha nos dois projetos antes de compilar:
`could not read …/app_component.template.dart` (`limitless_ui`, 1 erro;
`new_sali`, 83). Com o executor de builders Dart na VM
(`DARTFORGE_BUILD_DART=dart`) também. O `dartforge build` sozinho mostra o
motivo: sem a VM, 545 ações sem produtor (7 saídas do `ngx_dart`, 533 de
builders Dart, 5 do Sass "ainda não verificado byte a byte"); com
`--dart`, o motor roda as 2.119 ações em **16,6 s** no `limitless_ui`
(contra 2 min 9 s do `build_runner build`), mas a sessão do `dev` não usa
essas saídas e falha do mesmo jeito. Hoje, portanto, o caminho do
DartForge num checkout limpo é `dart run build_runner build` + `dartforge
serve`.

### 2. Início

| projeto | caminho | pronto para servir | CPU | pico de memória | processos |
| --- | --- | ---: | ---: | ---: | ---: |
| `limitless_ui` | webdev, checkout limpo | 114,4 s | 140,6 s | 4,89 GB | 19 |
| | DartForge: `build_runner build` (limpo) + `serve` | 156,4 s + 10,2 s = **166,6 s** | 129,7 + 5,2 s | 1,24 GB / 0,58 GB | 1 |
| | webdev, morno (`.dart_tool/build` pronto) | 10,7 s | 8,2 s | 0,68 GB | 3 |
| | DartForge, com os gerados, sem saída anterior | **10,2 s** | 5,2 s | 0,58 GB | 1 |
| | DartForge, morno (saída anterior no disco) | **8,0 s** | 5,2 s | 0,58 GB | 1 |
| `new_sali` | webdev, checkout limpo | 120,7 s | 192,1 s | 6,71 GB | 19 |
| | DartForge: `build_runner build` (limpo) + `serve` | 129,2 s + 14,1 s = **143,3 s** | 192,6 + 8,2 s | 1,83 GB / 0,99 GB | 1 |
| | webdev, morno | 10,6 s | 9,9 s | 0,77 GB | 3 |
| | DartForge, com os gerados | **14,1 s** | 8,2 s | 0,99 GB | 1 |
| | DartForge, morno | **7,7 s** | 7,9 s | 1,05 GB | 1 |

Leitura: **do checkout limpo o DartForge é mais lento que o webdev**
(1,46× no `limitless_ui`, 1,19× no `new_sali`), porque paga um
`build_runner build` inteiro antes; o "20×" de 13/10 media o DartForge
com os gerados já no disco contra o webdev do zero. Com os gerados
prontos, o início fica empatado com o webdev morno (10,2 × 10,7 s;
14,1 × 10,6 s) e o morno do DartForge é o mais rápido (8,0 e 7,7 s).

### 3. Edição → código novo na página (worker residente)

Os dois servidores no ar, edições alternadas; mediana [valores] das
rodadas, CPU somada dos processos da ferramenta.

| projeto | edição | DartForge | CPU | webdev | CPU |
| --- | --- | ---: | ---: | ---: | ---: |
| `limitless_ui` | corpo | **1,74 s** [2,59 1,74 1,47] | 1,1 s | 3,70 s [17,6 3,33 3,70] | 3,4 s |
| | API pública | **5,52 s** [5,92 4,02 5,52] | 3,0 s | 18,0 s [23,9 18,0 16,9] | 41,2 s |
| | template | 3,06 s [2,75 3,06 3,06] | 1,1 s | **2,16 s** [2,16 2,17 2,15] | 2,3 s |
| `new_sali` | corpo | **2,98 s** [3,79 2,16] | 2,0 s | nunca (ver abaixo) | 24–83 s |
| | API pública | **5,12 s** [7,28 2,96] | 2,9 s | nunca | 30–40 s |
| | template | **2,80 s** [2,81 2,79] | 1,4 s | nunca | 22–29 s |

O webdev continua sem mostrar o código novo no `new_sali`: a recarga cai
em `/login`, que o servidor dele responde com 404 (o mesmo de 13/10); a
CPU por edição é a do `build_runner` reconstruindo o módulo do *barrel*.
No `limitless_ui`, a edição de template é o único caso em que o webdev
chega antes (2,2 × 3,1 s de parede, com o dobro da CPU): o nosso gasta ~1 s
de CPU e espera o resto (motor de build, escrita e recarga).

### 4. Memória, JavaScript servido e página

| projeto | medida | DartForge | webdev |
| --- | --- | ---: | ---: |
| `limitless_ui` | pegada ao fim das edições (pico) | **0,51 GB (0,62)**, 1 processo | 3,90 GB (5,33), 19 processos |
| | JS servido: *scripts*, bruto, gzip | 447, 56,4 MB, **4,42 MB** | 324, 48,8 MB, 5,39 MB |
| | página pronta, sem / com cache HTTP | **364–810 / 369–554 ms** | 969–1.269 / 566–676 ms |
| `new_sali` | pegada durante as edições (pico ao fim) | **0,85–1,19 GB (1,19)**, 1 processo | 5,0–6,0 GB (7,90), 19 processos |
| | JS servido: *scripts*, bruto, gzip | 586, 104,4 MB, **8,52 MB** | 382, 87,1 MB, 10,42 MB |
| | página pronta, sem / com cache HTTP | **678 / 784 ms** | 1.350 / 923 ms |

### 5. O que fazer, por impacto

1. **Subir do checkout limpo sem `build_runner`.** A sessão do `dev` com
   `DARTFORGE_BUILD_DART` não materializa os `.template.dart` que o
   `dartforge build --dart` gera em 16,6 s; ligar as duas coisas tiraria
   ~2 min do primeiro início dos dois projetos (o resto é o Sass ainda
   não verificado e as 7 saídas do `ngx_dart` sem produtor nativo).
2. **Edição de template no `limitless_ui`** (3,1 s de parede para ~1 s de
   CPU): medir onde a sessão espera.
3. Os itens 2, 3, 5 e 6 da lista de 13/10 continuam.
