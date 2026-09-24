# Estado do DartForge — 2026-09-22

O que **funciona hoje, verificado por execução**, e o que **falta**, nesta
ordem. Tudo aqui é medido; nada é estimativa salvo onde está escrito
"estimado". Os números são do `main` deste commit, na máquina do
proprietário (8 núcleos, 8 GB).

Alvos reais usados como critério:
* `C:/MyDartProjects/new_sali` — ngdart 8.0.0-dev.4 (frontend) + angel3
  (backend) + core, 1.258 arquivos, 8,5 MiB de Dart;
* `references/limitless_ui` — biblioteca de componentes ngdart do
  proprietário, com `example/` (24 componentes) e suíte e2e em puppeteer.

---

## 1. O que funciona

### 1.1 Front-end (análise) — `crates/frontend`, `elements`, `types`

| Fase | Estado | Evidência |
| --- | --- | --- |
| Léxico + sintaxe de Dart 3.6 | **completo** | 426/426 arquivos do `lib/` do SDK 3.6.2, 1.969/1.969 do corpus pub (26 pacotes), 1.258/1.258 do `new_sali` — `cargo test -p dartforge-frontend --test corpus -- --ignored` |
| Modelo de elementos, imports/exports, `part`, patches do SDK | **completo** | 36 bibliotecas do SDK carregadas com os patches do DDC fundidos, 269/269 supertipos resolvidos — `crates/elements/tests/sdk.rs` |
| Tipos: representação, hierarquia, subtipagem | **completo** | 83/83 casos normativos de `subtyping.md`; 25.179 anotações do SDK em 10.396 `TypeId` (hash-consing) |
| Inferência de corpos, fluxo, constantes | **funcional, com lacunas** (motor reescrito pela especificação, `crates/types/src/inferencia`) | 40/40 negativos do `analyzer`; medido contra o oráculo `package:analyzer` (`tools/oraculo_tipos`): `new_sali/core` 15 avisos e 31 de 634.368 expressões divergentes, `frontend` 28 avisos (17 deles em templates gerados, que o analyzer também acusa) e 56 de 1.259.011; SDK da fonte (nativo) 18 diagnósticos, os mesmos do analyzer; corpus de conformidade 83/95 programas iguais ao oráculo (ver §2.1) |
| Versão de linguagem por biblioteca e recursos 3.7–3.13 | **3.7–3.13 de sintaxe completos**; inferência e fluxo 3.7–3.13 (P6) pendentes | ver §1.1.1 |

### 1.1.1 Dart 3.7–3.13 — `docs/VERSOES-LINGUAGEM.md`

3.6.2 é o **piso**, não o teto. Cada biblioteca tem a sua versão de
linguagem (marcador `// @dart = x.y` > `languageVersion` do pacote > a
corrente, **3.13**), resolvida uma vez no carregamento e consultada como bits
(`LibraryFeatures`); `dart:*` fica no piso (D1). O harness tem **dois SDKs de
oráculo** (3.6.2 e 3.13.4, cada um com o seu `dartdevc` e o seu
`dart_sdk.js`); os programas dizem a versão que exigem (`// requer-dart:`).

| recurso | versão | `corpus/moderno` (VM e DDC 3.13.4; dev e produção) |
| --- | --- | --- |
| curingas `_` | 3.7 | 300–303 passam (1 negativo; `// @dart=3.6` volta a ligar `_`) |
| elementos null-aware | 3.8 | 310–311 passam (1 negativo); chave nula não avalia o valor |
| nomeados privados `{this._x}` | 3.12 | 320–323 passam (3 negativos) |
| atalhos de ponto | 3.10 | 330–332 passam (2 negativos) |
| construtores primários, `new`/`factory`, corpo `;`, `var`/`final` | 3.13 | 340–346, 348–349 passam (4 negativos); 347 pendente (membros de extension type, lacuna da 3.3) |
| inferência por bounds, fluxo sólido, gerador | 3.7–3.10 | 350–352 **pendentes** (P6, com o dono de `types`) |

Placar no CI (Pesado 35904470774 e CI 35904470762, `ci/moderno` em 5a68e2d, os dois verdes): **22/26** em desenvolvimento
e em produção, **26/26** DDC×VM, 12 negativos recusados na mesma linha que o
CFE; os 4 que faltam estão em `corpus/moderno/PENDENTES`. Na mesma rodada:
`corpus/js` 223/223 em desenvolvimento e produção, determinismo idêntico
(produção e IR do nativo), nativo e JIT 82/223 (os do `main`) e o portão
**custo zero verde** — nada regrediu. O `corpus/js` compilado na 3.6 dá **JS idêntico
byte a byte** ao da base (222/222), e o parser continua aceitando 426/426 do
SDK e 1.969/1.969 do pub (cada pacote na versão do seu pubspec). Nenhum
recurso precisou de runtime novo. O nativo não roda o `corpus/moderno`:
curinga liga nome, entrada de mapa null-aware é recusada e atalho de ponto só sai quando é
construção. Macros e augmentations: contratos em `docs/MACROS-PROTOCOLO.md`
e `docs/AUGMENTATIONS.md` (executor nativo auto-hospedado), sem código.

Custo para projeto 3.6 (regra governante): o portão `custo zero (tempo)` do
Pesado passou (corpus JS 9.318 → 9.208 ms, edição de corpo 32 → 32 ms); no A/B
local contra o `main` 6583c2b, mínimo de 3 por programa nos 223 do
`corpus/js`, 11.783 ms × 11.806 ms (+0,2%). Números e método em
`docs/VERSOES-LINGUAGEM.md` §7.

### 1.1.2 Augmentations e macros — `docs/AUGMENTATIONS.md`, `docs/MACROS-PROTOCOLO.md`

* **Augmentations** (P7): `augment` em classe, mixin, membros e funções de
  topo; bibliotecas de augmentation da forma 3.6 (`import augment` +
  `augment library`, experimento `macros`) e *parts* com imports da forma
  atual (`augmentations,enhanced-parts`); a fusão no outline generaliza o
  `@patch` do SDK (`elements/src/augmentation.rs`) e o `emit_js` emite a
  classe com os membros da cadeia. `corpus/macros/400–405`: **6/6** em
  desenvolvimento e produção, 6/6 DDC×VM (3.6.2 e 3.13.4), CI verde (Pesado
  35918855910, CI 35918855886). Divergências medidas dos dois CFEs em
  AUGMENTATIONS.md §4.
* **API de macros reescrita** (`pacotes/macros`, pacote `macros`, Dart puro):
  a superfície do `package:macros` 0.1.3-main.0 e o lado do executor
  (modelo, introspecção, builders com o texto do CFE, serviço `macro.*`). O
  `json.dart` do `package:json` 0.20.4, sem mudança, analisa contra ela com
  zero problemas no analyzer 3.6.2.
* **Hospedeiro** (`crates/macros_host`): detecção com custo zero, ordem do
  CFE, as três fases com recarga, modelo e consultas, montagem byte a byte,
  o serviço `macro.*` do `dfexec/1`, `Indisponivel` no produto.
  `dartforge macros --materializar` grava a augmentation
  (docs/MACROS-COMPATIBILIDADE.md). Placar em MACROS-PROTOCOLO.md §8.
* **Espera o executor nativo**: executar macros no `compile-js` (hoje: erro
  claro na anotação; `410_json_codable` em `corpus/macros/PENDENTES`).
* CI da rodada (`ci/macros` em ab2ad4a, os dois verdes): Pesado 35931208832
  (`macros` 6/7 dev e produção + 1 pendente, 7/7 DDC×VM; `corpus/js`
  223/223; `moderno` 22/26 como no `main`; custo zero verde) e CI 35931208819
  (inclusive `vm_executa_a_macro_e_bate_com_o_cfe` e
  `sessao_gravada_reproduz_o_texto_do_cfe` nos ignorados).

### 1.2 Emissão JavaScript — `crates/emit_js`

Emite **módulos ES6 no contrato do DDC** e liga contra o `dart:*` oficial
(`runtime/ddc/dart_sdk.js`, gerado de `ddc_platform.dill` por
`scripts/gerar-dart-sdk.ps1`). O `dartdevc` é o oráculo do contrato.

* **Corpus diferencial: 213/213.** Cada programa é executado em `dart run
  --enable-asserts`, em `dartdevc`+Node e no DartForge+Node; stdout e
  código de saída comparados **byte a byte**. `cargo run -p
  dartforge-diferencial`.
* **`new_sali/core`: 7 de 14 testes reais** (`package:test`) rodam no Node
  com a mesma saída da VM. Os outros 7 dependem de `dart:io` (leitura de
  arquivo, fontes de PDF) — impossível no navegador por definição; é alvo
  do backend nativo.
* **`limitless_ui/example` (biblioteca de componentes ngdart do
  proprietário, 24 componentes, 483 módulos): a suíte e2e em puppeteer
  passa — **26/26**, o mesmo que a saída oficial do `dart2js`
  (`build_web_compilers --release`), em 4m11s contra 3m33s. Uma sonda que
  percorre as **53 rotas** da galeria recolhendo `onerror`/
  `unhandledrejection`/`console.error` dá **52/53** nos dois lados (a rota
  restante não tem o seletor que a sonda espera, e reprova igual no
  oficial). Foi essa sonda, não a suíte, que encontrou o último defeito
  corrigido (receita rti de tipo genérico cru: `raw|Caixa<@>`, nunca
  `raw|Caixa`). `scripts/limitless-ui.ps1`, `docs/LIMITLESS-UI.md`.
* **`new_sali/frontend` (ngdart + `dart:html` + `package:js`): a aplicação
  roda no navegador.** 616 módulos; `node --check` 616/616; no Edge
  headless os **11 passos do fluxo** (carga, carrossel, erro do IdP,
  submeter login, rotas públicas, guarda de rota, callback OIDC, sessão
  forjada) ficaram **idênticos à saída oficial do `build_web_compilers`**,
  inclusive as 7 chamadas HTTP que a aplicação faz. `scripts/servir.ps1
  -Fluxo` (dirige o Edge por CDP, separa erro da aplicação de erro de
  ambiente).

Construtos cobertos: classes (construtores de todo tipo, `const`
canonicalizado, estáticos, operadores, mixins, enums com membros,
genéricas, `noSuchMethod`, `late`), coleções e literais tipados com rti,
exceções, `async`/`await`/`async*`/`sync*`/`await for`, `dynamic` por
`dsend`, records e padrões, extensions, `typedef`, bibliotecas múltiplas
com prefixos/`show`/`hide`/`part`, interop (`package:js`,
`dart:js_interop`, `@JSName`, `@anonymous`), `dart:html`.

### 1.2.1 Perfil de **produção** — `crates/emit_js_producao`

`dartforge-jsprod <entrada.dart> -o <saida.js>` escreve **um arquivo**, com
o runtime embutido e podado pelo mundo fechado. O plano, a referência
estudada e o que ele ainda não faz estão em `docs/JS-PRODUCAO.md`.

* **Corpus diferencial: 214/214**, com o **mesmo stdout da VM**, byte a
  byte — o mesmo placar do perfil de desenvolvimento.
  `cargo run --release -p dartforge-diferencial -- --producao` compara os
  três (VM × nosso desenvolvimento × nossa produção) e agrupa as falhas em
  duas listas, porque as causas são diferentes: falha do desenvolvimento é
  construto que falta no emissor, falha da produção é poda ou montagem.
* **Poda do `dart_sdk.js`**: a varredura parte os 7.087.858 B em 14.735
  declarações de topo **sem perder um byte**, sub-divide as três que
  sozinhas referenciam o programa inteiro (as 549 constantes do `CT`, os
  315 KB de regras de subtipagem do `addRules`, o `copyProperties`), extrai
  as referências — inclusive os nomes de classe dentro das **receitas
  rti**, que nenhum analisador de JS enxerga — e roda o ponto fixo.
  No `01_print`: **6.922 KB → 1.474 KB**, 8.450 de 44.965 unidades vivas.
* **Empacotamento**: uma IIFE por módulo, em ordem topológica, com os
  `var L$…` içados. **Bibliotecas nunca são fundidas** — dois arquivos
  vendorizados byte a byte iguais continuam com estado global separado e
  tipos de identidade distinta (`docs/PESQUISA-OTIMIZACAO.md` §3).
* **Determinismo** por construção, com teste de unidade (duas montagens das
  mesmas entradas dão o mesmo arquivo byte a byte) e pelo modo
  `dartforge-diferencial determinismo --producao --trabalhadores 1,4,8`,
  que o backend nativo trouxe e que o perfil de produção passa.

Contra o oficial (`dart compile js -O4`, SDK 3.6.2), amostra do corpus por
`pwsh scripts/medir-js-producao.ps1`:

| programa | dart2js | jsprod | dart2js | jsprod |
| --- | ---: | ---: | ---: | ---: |
| `01_print` | 34,1 KB | 1.527 KB | 2,58 s | **0,88 s** |
| `40_classes_basico` | 34,4 KB | 1.538 KB | 2,32 s | **0,60 s** |
| `60_list_basico` | 45,1 KB | 1.533 KB | 3,98 s | **0,76 s** |
| `80_async_await_basico` | 48,3 KB | 1.534 KB | 3,32 s | **0,68 s** |
| `120_convert_json` | 57,2 KB | 1.728 KB | 1,81 s | **0,41 s** |

**Somos 3 a 5× mais rápidos e 30 a 45× maiores**, e o tamanho tem piso fixo
de ~1,5 MB qualquer que seja o programa: é o que resta do `dart_sdk.js` do
DDC depois da poda. Ver §2.3 — fechar esse buraco é compilar o SDK pela
nossa trilha, não otimizar mais.

**Mundo fechado sobre a nossa trilha** (etapa 5, `crates/mundo`,
`docs/JS-PRODUCAO.md` §1.7). O código do usuário e dos pacotes é podado
**antes** da emissão por um RTA sobre o modelo de elementos: seletor por
nome, três níveis de classe e a fronteira com o `dart_sdk.js` por regra.
Um verificador sempre ligado confere o texto emitido e fecha o ponto fixo
mundo+texto. O modo stub (`--verificar-stub`) denuncia na execução qualquer
chamada a código podado. Sem filtro, o emissor é byte a byte o de antes
(teste `identidade.rs` e o corpus inteiro contra o binário de `main`).

**Projeto real**: `new_sali/core`, `test/arvore_processo_item_test.dart`
(`package:test`, 351 módulos): **32.003 KB → 4.463 KB** e compilação de
4,3 s → 1,4 s, com a **mesma saída do `dart run`**. Os 7 testes do core
que rodam na web ficam entre 3,0 e 4,8 MB, todos iguais à VM e sem stub
executado. O custo do verificador é de 28-36 ms. O `limitless_ui/example`
só cai 6,5% (48,2 → 45,1 MB): a galeria alcança quase tudo, e seletor só por
nome é o limite (`docs/JS-PRODUCAO.md` §6.0).

### 1.3 Latência e memória — `crates/dev`

`dartforge dev` mantém a sessão viva e recompila o mínimo.

| Cenário (`new_sali/core`, 2.302 unidades, 352 módulos) | Tempo |
| --- | --- |
| compilação completa (`compile-js`, 2ª vez) | **1,72 s** |
| **edição de corpo** (sessão viva) | **227 ms** — 1 unidade reanalisada, 1 módulo escrito |
| edição de API pública | 288 ms — 82 bibliotecas reemitidas, 3 módulos |

`new_sali/frontend` (3.183 unidades, 616 módulos): **6,5 s a frio**
(cache do SDK construído, saída vazia), 4,3 s morno, edição de corpo
**335 ms**. Contra a toolchain oficial no mesmo projeto e máquina:
`build_runner build` **2 min 59 s** e `dart2js` de produção **~4 min**
(esta última não é comparação de igual para igual — ver PLANO.md,
"Latência medida contra a toolchain oficial").

**Memória — a propriedade que o LSP do Dart não tem**: teste de platô com
20 edições sucessivas, `live_bytes` **+0,00 MB** (core 204,63 MB, pico
278 MB; frontend 438,15 MB, pico 568 MB). No mesmo projeto o `webdev`
chega a ~10 GB e o LSP do Dart a ~6 GB, crescendo a cada edição.

Trajetória medida no mesmo arquivo do `core` ao longo do dia: 12,4 s →
4,6 → 2,87 → 1,72 s (completa); 1,70 s → 227 ms (incremental).

### 1.4 LSP — `crates/lsp` + `editors/vscode`

Transporte JSON-RPC por stdio escrito aqui, `didChange` **incremental**
com conversão UTF-16 correta, `$/cancelRequest`, `publishDiagnostics` pelo
parser novo, `DocumentStore` como dono por documento. Extensão VS Code
(cliente TypeScript fino). Medição no `new_sali`, mesma sequência de 1.462
mensagens: **DartForge pico 21,3 MiB / platô 19,0 MiB; `dart
language-server` pico 640,5 MiB / platô 633,7 MiB**.

**Diagnósticos com paridade (plano A1/A2) — `crates/diagnostics`,
`crates/paridade`, `dartforge analyze`.** O `Diagnostic` tem código,
severidade e argumentos; a mensagem de um diagnóstico com código é o molde
oficial em inglês renderizado como o `formatList` do analyzer. A tabela
(`diagnostics/src/codigos_g.rs`) é gerada do `analyzer-6.11.0` da cache do
pub: 1.030 códigos — 542 `CompileTimeErrorCode`, 7 `StaticWarningCode`,
144 `WarningCode`, 8 `HintCode`, 48 `FfiCode`, 265 `ParserErrorCode`, 12
`ScannerErrorCode`, 4 `TodoCode`. `dartforge analyze [--format=json]` emite o
mesmo JSON v1 do `dart analyze` (offset UTF-16, ordem do dartdev, códigos de
saída 3/2/0) e publica pela regra do plano §2.3: sintaxe sempre, semântica só
os códigos de `crates/paridade/verificados.txt` — **vazia hoje**.

* **Aceite de A2**: dados código, argumentos e intervalo, a sonda de 7 erros
  sai **byte a byte** igual ao JSON gravado do SDK 3.6.2
  (`crates/paridade/tests/sonda.rs`). Pela análise de hoje (inferência pela
  especificação, main 27c31d0), 3 dos 7 batem em posição e mensagem:
  `non_bool_condition`, `not_assigned_potentially_non_nullable_local_variable`
  e `argument_type_not_assignable`. Os outros 4 dependem do pedido T1 a
  `types`: `undefined_function` sai como `undefined_identifier`;
  `return_of_invalid_type` cobre o comando inteiro, e não a expressão;
  `unchecked_use_of_nullable_value` não existe; e `unused_local_variable` é
  do A3. Há 1 falso positivo.
* **Placar no corpus** (`corpus/diagnosticos`, oráculo gravado em disco):
  9.441 arquivos em 5 grupos — `tests/language` (1.180), trechos de
  `pkg/analyzer/test/src/diagnostics` (8.253), os de `@dart` 3.7+ com o
  oráculo 3.13.4 (7) e a sonda — e 26.133 diagnósticos do oráculo.
  **1.935 na posição exata (7,4%), 1.731 com mensagem igual**; posição
  errada 1.638; FP 2.696; FN 22.560. Antes da inferência reescrita o placar
  era 1.268 (4,9%) e FP 4.660.
  * Códigos com 100%: `illegal_character` e `unnecessary_cast`, de 593 com
    casos.
  * FN maiores: `unused_local_variable` 2.315 (A3), `duplicate_definition`
    1.317, `expected_executable` 1.240, `type_argument_not_matching_bounds`
    1.175 e `missing_const_final_var_or_type` 912. Dos 1.724
    `expected_token`, a recuperação do parser difere da do fasta.
  * FP maiores: `expected_token` 410, `undefined_identifier` 377,
    `undefined_method` 368, `undefined_class` 359 e `undefined_getter` 202.
  * 2 pânicos isolados: `part/self_test.dart` esgota a memória, e um caso de
    `recursive_interface_inheritance` não termina. Cada lote roda num
    processo filho com teto de memória e de tempo.
  * 290 atribuições ambíguas: `types` ainda não diz a unidade do
    diagnóstico.
* **Projetos reais**: o oráculo 3.6.2 dá **0** no `new_sali/core` (36 s),
  no `new_sali/frontend` (24 s) e no `limitless_ui` (233 s). O nosso lado
  tem, internamente, **53, 29 e 216** diagnósticos, todos falsos positivos
  (antes da inferência reescrita: 3.746, 3.029 e 2.045). Por código:
  `undefined_identifier` 127, `uri_has_not_been_generated` 70,
  `const_initialized_with_non_constant_value` 47,
  `missing_required_argument` 34 e `argument_type_not_assignable` 13.
  **Publicados pela regra: 0.** Nenhum chega ao editor.
* **Mutações** dos três projetos: 45 mutantes (`nome`, `import`, `tipo`,
  `!` e `await`), com o oráculo regravado sobre cada um. Oráculo 190;
  **acertos 75**; posição errada 9; FP 58; FN 106.* **CI**: job `analise` do `pesado.yml`, contra o oráculo gravado (o runner
  não roda `dart analyze`), com o relatório idêntico em 1, 4 e 8
  trabalhadores (`determinismo`).
### 1.5 Backend nativo — `crates/emit_native` (feature `nativo`)

Trilha nova → HIR própria → LLVM IR → Clang → executável, com o runtime
Rust (GC por tracing). `dartforge compile-native` (compile com
`cargo build -p dartforge-cli --features nativo`); `dartforge aot` é o
apelido de produção do mesmo caminho.

**Rodada 2, P6 + RTI: corpus nativo 91/223 (Pesado 35904857766, CI
35904857783), JIT 91/223 sem divergência, os 91 também sob `--gc-stress`
(job novo do `pesado.yml`, que reprova se um programa só falha com
estresse).** `async`/`await` como máquina de estados com o quadro no heap,
sobre o `dart:async` **compilado da fonte** (com `dart:_internal` e a
`Duration`; só para quem usa `dart:async`), laço de eventos no runtime
(microtarefas antes de timers, timers na ordem da VM); tipos em tempo de
execução no desenho do dart2js (receitas, universo canônico, regras de
supertipo; `is`/`as`/padrões genéricos, o cast inteiro, `Type`); `super`
dentro de mixin. Um defeito do carregador corrigido: a parte de um arquivo
de patch do SDK era carregada como parte comum. Detalhes em
`docs/NATIVO-PLANO.md` §7.7–§7.8. Antes disto:

**Rodada 2, P1–P4 (α): corpus nativo 81/223 (Pesado 35871381320), JIT
81/223 com zero divergências JIT × AOT; os 81 passam também sob
`--gc-stress`.** Closures com captura em célula e a convenção uniforme de
chamada de valor função; símbolos estáveis pelo caminho da declaração
(`df.<biblioteca>.<dono>.<membro>`, teste T-ID); despacho por nome para
receptor sem tipo e operadores sobre `num`/`dynamic`; `switch` (comando e
expressão), padrões, enums, records com campo nomeado e `const` canônico;
cascata, `super`, mixins pela linearização, extensões. Detalhes e decisões
em `docs/NATIVO-PLANO.md` §7.4–§7.5. Antes disto:

**Corpus nativo: 50/222 (CI, run 35823269758), antes 7/214; sob
`--gc-stress`, os mesmos 50/222 (run 35823275126).** O backend passou a ter um
**contrato de representação e raízes** (`docs/NATIVO-PLANO.md` §6 — R, E,
N, G), implementado em quatro passos medidos no CI: 7/214 → 26/214 (N:
nada de `0` como substituto; construtor, atribuição e membros de verdade)
→ 48/214 (R: representação pelo tipo, caixas, locais tipados) → 48/222
(E + G: arestas pela representação, verificador da HIR, raízes do código
gerado — com o corpus crescido para 222 no `main`) → 50/222 (o
`--gc-stress` achou `isEven` testando a paridade do handle da caixa). Dos 71 `panic` de
*handle* do começo, **nenhum sobrou**; também sumiram os `RefCell already
borrowed`, os tempos esgotados e os estouros do teto de heap do corpus. O
que falha hoje falha como **erro de compilação com diagnóstico** ("não
suportado no backend nativo: <construto>"), não como escalar usado como
handle. Dois achados que mudaram o desenho: o backend rodava **sem
`dart:core`** (a seção `vm` do `libraries.json` usa `include` e o
`SdkLayout` não o seguia — todo `int` era `dynamic`), e a inferência não
tinha braço para `for-in`. O harness diferencial roda o corpus pelo
backend nativo comparando com a VM byte a byte:

```powershell
cargo build --release -p dartforge-diferencial
target\release\dartforge-diferencial.exe --nativo
```

Limites ligados por padrão, porque sem eles o corpus nativo em paralelo
tomou a memória da máquina: heap do runtime com teto de 256 MiB
(`DARTFORGE_HEAP_MAX_MB`), `--jobs 2` no modo nativo, `--limite-exec` de
5 s para executar o binário, e 4 MiB de saída capturada por processo.

O número saiu de 3/214 nesta sessão; o que mudou de fato foi a natureza do
gargalo — as falhas de "o Clang recusa o módulo" caíram de 172 para 13, e
o que sobra é programa que roda e imprime outra coisa. Ver
`docs/NATIVO.md` e `docs/NATIVO-PLANO.md`.

**Rodada 2, δ (NATIVO-PLANO §7.4):** strings com semântica UTF-16 na forma
da VM (`_OneByteString`/`_TwoByteString`) e `Ref` com `Smi` etiquetado
(R10) entraram sem regressão — nativo 50/223, JIT 50/223 com 0 divergências,
os mesmos 50 sob `--gc-stress` (Pesado 35849542217 e 35852784596). P5a: a
sobreposição `sdk_nativo/` carrega; os corpos das sete bibliotecas da fonte
têm 4 447 diagnósticos de inferência (medidos com um pedido a
`crates/types`). P5b: tabela dos 137 natives, 48 já no runtime. P5c/P5d
esperam P1–P4 (o lowering de closures, despacho e RTI para compilar o SDK).

O **lowering de exceções** existe: `throw`, `try`/`catch`/`finally`,
`rethrow`, com `finally` como sub-rotina e discriminador de razão (normal,
`return`, exceção, `break`, `continue`). O mecanismo escolhido é
**exceção pendente com verificação depois da chamada**, e não landing
pads: SEH/funclets no MSVC seria um segundo lowering, o runtime em Rust
não desenrola através de `extern "C"` — e é de lá que vem a maior parte
das exceções reais —, e exceção que atravessa `await` não pode depender da
pilha nativa. A justificativa e o custo no caminho feliz estão em
`docs/NATIVO-PLANO.md`.

**Determinismo verificado no corpus inteiro**: `dartforge-diferencial
determinismo --nativo [--trabalhadores 1,4,8]` emite o LLVM IR de cada
programa, sem Clang, ligação nem execução, e exige o mesmo resumo (FNV-1a
de 128 bits) — ou o mesmo erro — programa a programa. Passa com 1, 4 e 8
trabalhadores nos 214 programas (184 com IR, 30 com erro de carga), em
**0,2–0,5 s por passada**. `--executar` mantém o caminho antigo (compilar,
executar e comparar o relatório; o IR só com `DARTFORGE_KEEP_IR=1`). Ver §3.2.

**Cache de objeto**: o `.obj` de cada programa fica em
`native_cache/obj/` pela chave do IR + `clang --version` + bandeiras
(`crates/emit_native/src/cache_objeto.rs`); o mesmo IR dá o mesmo objeto
byte a byte. `compile-native --emit-ir` grava só o IR, `--resumo` imprime o
resumo.

### 1.5.1 JIT — `dartforge run` e `dartforge reload` (R0)

O `crates/jit` (ORCv2) executa o **mesmo LLVM IR** que o AOT entrega ao
Clang (`emitir_ir`). Compile com `cargo build -p dartforge-cli --features jit`;
o executável passa a exigir a `LLVM-C.dll` da distribuição completa no
`PATH` (`docs/JIT.md`).

* `dartforge run <entrada.dart> [--timings]` emite o IR e o executa neste
  processo, sem trampolim: chamadas diretas, como no AOT.
* `dartforge reload <entrada.dart>` é **reinício a quente (R0)**: a cada
  edição recompila tudo e recomeça do `main` num processo novo. **O estado
  NÃO é preservado**, e a saída diz isso. Edição que não compila mantém a
  geração em execução. Recarga com estado é o R1 (`docs/PESQUISA-HOT-RELOAD.md`).
* `dartforge-diferencial --jit` compara o JIT com a VM (mesmo placar do
  `--nativo`); `--jit-aot` passa o MESMO IR também pelo AOT, lista todo
  `JIT≠AOT` (é defeito) e mede os tempos. No CI é o job `jit` do `pesado.yml`.

O runtime é **fonte única**: o mesmo `runtime_main.rs` vira a `.lib` do AOT e
o módulo `dartforge_runtime::abi` do JIT, com a tabela de símbolos gerada
(`crates/runtime/build.rs`). No corpus inteiro, JIT e AOT deram **222/222
idênticos** a partir do mesmo IR. O JIT até executar leva 43 ms por programa, e
o Clang + ligação 165 ms (medianas, Pesado 35827208951, placar 50/222 nos dois; `docs/JIT.md`). A
biblioteca `crates/jit` já serve de executor persistente: `compile_module`
para o cache, `add_compiled_module`, e várias execuções com estado limpo.

### 1.5.2 Runtime Dart — `crates/runtime`

O runtime em Rust que o executável nativo carrega: heap com **GC por
tracing preciso** (cada campo marca se é referência), `String` em UTF-16
para casar com a indexação do SDK, `int` de 64 bits com estouro modular
como na VM, e a fronteira de símbolos com o IR emitido.

Ganhou nesta sessão um **teto duro de heap** (256 MiB por padrão,
`DARTFORGE_HEAP_MAX_MB` ajusta, `0` desliga), separado do gatilho de
coleta e contando também a tabela de slots. Ao estourar, sai com uma linha
legível e código 255, sem `panic` atravessando `extern "C"`. Não é
detalhe de teste: sem ele, um programa do corpus em laço ia a 1,9 GB e
travava a máquina.

Com o contrato de raízes (G, `docs/NATIVO-PLANO.md` §6.5) o heap **coleta
em qualquer alocação** — o portão "só com frame aberto" saiu, porque o
código gerado agora abre um quadro de raízes por função e enraíza cada
valor `Ref` (SSA e locais). A exceção pendente, o rastro corrente e os
globais `Ref` são raízes; as tabelas laterais por handle (coleções
imutáveis, iterações ativas) moram no `Heap` e são purgadas a cada coleta;
toda extern que aloca mais de uma vez enraíza os temporários. `int`,
`double` e `bool` numa posição de referência são **caixas**
(`BoxedInt`/`BoxedDouble`/dois singletons de `bool`) que as coleções
normalizam para o escalar na entrada, com `==`/`identical` por valor como
na VM. `Heap::get` distingue as quatro falhas de handle (null, negativo,
além da tabela, já coletado) e `DARTFORGE_GC_OFF=1` desliga a coleta para
diagnóstico. `DARTFORGE_GC_STRESS=1` (e `--gc-stress` no harness) coleta
antes de toda alocação.

### 1.6 Infraestrutura

* `crates/diferencial` — harness paralelo com cache dos oráculos;
  `corpus/js/` com 213 programas verificados na VM.
* `docs/CONTRATO-DDC.md` — Dart e JS do `dartdevc` lado a lado para os 212.
* Determinismo (`docs/PESQUISA-OTIMIZACAO.md` §11): `dartforge-diferencial
  determinismo [--nativo] [--trabalhadores 1,4,8]` exige relatório idêntico
  com qualquer número de trabalhadores; no nativo, o mesmo LLVM IR de cada
  programa — se a ordem de conclusão mudar o IR, o cache de objeto por hash
  erra e o Clang roda à toa. Verificado idêntico com 1, 4 e 8; no nativo,
  no corpus inteiro e sem precisar de `DARTFORGE_KEEP_IR` (§3.2).
* `scripts/` — `gerar-dart-sdk.ps1`, `servir.ps1`/`fluxo.mjs` (Edge por
  CDP), `limitless-ui.ps1` (`-Preparar`/`-Montar`/`-Servir`/`-E2e`),
  `medir-lsp.ps1`.
* Cache do outline do SDK em `target/dartforge/sdk-<hash>.bin` (5 ms para
  ler, contra ~105 ms de reanálise).
* **CI no GitHub Actions** (§3.0): `ci.yml` (rápido, todo push) e
  `pesado.yml` (corpus JS, produção, nativo em fragmentos, determinismo).
  Fora do `ci.yml`, declarado no cabeçalho dele:
  * **`cargo fmt --check` e `clippy -D warnings`** — o workspace nunca foi
    formatado inteiro nem está limpo no clippy (o primeiro crate,
    `dartforge-elements`, já para com 3 erros). Voltam num commit **só de
    formatação/lint**, depois que as branches em andamento entrarem;
    fazê-lo antes conflita com todas elas.
  * **Ubuntu**: a trilha nova falha no Linux em `dartforge-dev` (`hashes`,
    `plato`), `dartforge-emit-js` (`basico`) e `dartforge-elements`
    (`sdk_cache`), e passa no Windows.
  * Sem exclusões de crate: a trilha velha, o `cranelift-jit` e o driver
    `native` saíram do workspace (§4), e com eles os 16 testes que falhavam
    e a falha registrada de `real_executable_prints_ints_and_bools`. Os
    quatro exemplos de memória têm nomes distintos (`memoria_<crate>`), e o
    CI roda `cargo test --workspace` com todos os alvos e os doctests.

### 1.7 `dartforge serve` e o gerador do ngdart

* `dartforge serve <entrada> -o <dir> [--web <dir>] [--porta N]` — servidor
  HTTP próprio, cliente de recarga injetado na resposta do `index.html` (o
  arquivo no disco não é tocado) e canal WebSocket (RFC 6455) como o
  auto-refresh do webdev. Protocolo versionado em JSON (`ola`,
  `recarregar`, `erro`), separado do transporte. Verificado no
  `new_sali/frontend` com Edge headless por CDP
  (`scripts/verificar-recarga.mjs`): editar um componente recarrega o
  navegador em **1,5 s**, contra **1m20s** do `build_runner` para a mesma
  edição.
* Fontes geradas em memória (`crates/elements/src/gerado.rs`): os
  `.template.dart` do ngdart podem vir de uma tabela indexada pelo caminho
  natural, sem passar pelo disco. Geração imutável e trocada inteira — o
  navegador nunca vê meio estado. `DARTFORGE_GERADOS=build_runner` prova o
  encanamento: 284 templates lidos da memória dão 616 módulos byte a byte
  iguais aos da compilação que lê do disco.
* `crates/gerador_ng` — o compilador do ngdart em Rust, com dois oráculos:
  o corpus próprio (`corpus/ngdart/`, uma forma por arquivo, com o
  `.template.dart` oficial ao lado) e os 945 arquivos que o `build_runner`
  gerou no `new_sali/frontend`
  (`cargo run -p dartforge-gerador-ng --example oraculo -- <projeto>`).

  **new_sali/frontend: 186 arquivos gerados por nós (212 iguais byte a
  byte contando os `.css.shim.dart`), 0 diferentes, 114 pendentes. Corpus:
  99 de 106 oráculos conferidos, os 7 restantes recusados de propósito.**
  (Medido em 2026-09-23, rodada 3; antes dela: 162 gerados, corpus 94/100;
  antes da rodada 2: 145, corpus 58/66.)

  Cobre hoje:
  - biblioteca sem Angular, `@Directive` e `@Pipe` (o arquivo trivial);
  - template estático: elementos, texto, atributos, `<ng-content>` (com e
    sem `select`);
  - interpolação tipada como o `_TypeResolver` (chamada com o retorno do
    método, ternário/`!x`/`x!`/`??`/índice/binário `dynamic`), atributo
    interpolado (`interpolateString1`, `interpolate2`), locais de visão
    ancestral pela cadeia de `parentView`;
  - imutabilidade do `isImmutable` oficial (getter explícito, `a.b`, `!x` e
    chamada são mutáveis) e a guarda de diretivas: nó que casa um seletor
    de diretiva conhecida e não emitida é recusado;
  - ligações `[x]`, `[class.x]`, `[attr.x]`, `[style.x]` (constantes no
    `if (firstCheck)` compartilhado) e eventos gerais: tear-off, handler
    complexo (`_handleEvent_N`), atribuição, `$event`, evento próprio pelo
    `eventManager`;
  - pipes puros `$pipe.nome(..)` (instância na visão do componente,
    `pureProxyN` por chamada, também nas embutidas);
  - `@HostListener` em componente (forma simples), `@HostBinding('class.x')`
    em diretiva (o `XNgCd`), `#ref` em elemento HTML com `@ViewChild`
    estático, `providers: []` e `pipes:` sem uso;
  - componente filho completo: `@Input` (literal, constante, dinâmica,
    ordem de declaração), `@Output` (`subscription_N`), ciclo de vida e
    `onPush` do filho, injeção no construtor dele (nó, `ChangeDetectorRef`,
    serviços pela cadeia de injetores), `#ref` no filho com `@ViewChild`
    (`queryChangeDetectorRefs`), filho com `*`, projeção por seletor e
    consulta de conteúdo sem resultado;
  - diretivas de atributo por **metadados lidos do programa**
    (`metadados.rs`, o `_ComponentVisitor` do ngcompiler sobre o banco
    semântico: seletor, `providers` avaliados como constante, `@Input`/
    `@Output`/`@HostListener` herdados, ganchos, dependências com
    `@Inject`/`@Optional`/`@Self`/`@Host`) — sem catálogo escrito à mão:
    `ngforms` (`NgForm`, `NgModel`, acessores de valor, `select`/`option`,
    validadores) e as diretivas do próprio projeto, com os provedores na
    ordem das dependências, dependência de elemento acima (`@Host`, também
    de visão ancestral), `ngOnDestroy`, ouvintes do mesmo evento agrupados e
    o `injectorGetInternal`;
  - componente de seletor composto, diretiva em tag que não é HTML
    (`addShimE`), conteúdo que nenhuma projeção recebe e `@ContentChildren`
    do filho preenchido (o page header do new_sali);
  - injeção no construtor, ciclo de vida (os sete ganchos), `onPush`;
  - folhas de estilo: **Sass** (o subconjunto que os projetos usam) e o
    shim `_ngcontent-%ID%` na forma do `csslib` compacto, gerando também o
    `.css.shim.dart`.

  Duas coisas sustentam o "0 diferentes": o gerador **recusa** toda forma
  que não sabe traduzir (e o placar conta por motivo, para saber o que
  atacar), e o conversor de expressões reusa o parser Dart da trilha nova
  em vez de aproximar texto.

  `DARTFORGE_GERADOS=ng` compila com ele, fazendo antes uma carga de
  resolução (o equivalente ao `BuildStep.resolver`, lendo o nosso banco
  semântico); o que falta continua vindo do `build_runner`.

### 1.8 Motor de geração de código — `crates/build` (substituto do `build_runner`)

Contrato: `docs/BUILD-MOTOR.md`; protocolo do executor Dart:
`docs/BUILD-PROTOCOLO.md`; plano e fases: `docs/BUILD-RUST.md` §0. O motor
lê a configuração que o `build_runner` 2.4.15 leria, monta **o mesmo plano
de fases**, e para cada ação decide pela impressão digital do que ela
consultou se executa ou reaproveita; publica numa `Geracao` em memória.
Executores, nesta ordem: **nativo** (Rust), **Dart** (`dfexec/1`, hoje
indisponível), **apoio** (o que o `build_runner` deixou no disco, com aviso
quando está mais velho que a fonte; erro com `dartforge build --estrito`).

* **Plano igual ao oficial**: a forma canônica de `dartforge build --plano`
  é igual à do `.dart_tool/build/entrypoint/build.dart` (lido, nunca
  gerado) nos 9 casos do `corpus/builders` com builders, no
  `new_sali/frontend` e no `limitless_ui/example` (20 aplicações cada).
* **`corpus/builders`** (10 casos, oráculo do `build_runner` oficial,
  `scripts/corpus-builders.ps1`): todas as saídas dos manifestos são
  previstas pelo grafo, menos uma escrita por pós-processador; placar
  **0 iguais / 61 pendentes / 0 diferentes** (sem executor Dart nem apoio
  no corpus limpo); determinismo 1/4/8 idêntico; incremental = do zero nas
  12 edições (`crates/build/tests/corpus.rs`, `#[ignore]`, no `ci.yml` depois
  de `pub get`).
* **ngdart pelo motor** (estágio A: uma ação de pacote, `gerar_com_apoio`
  sobre o `Program` da sessão, pela API pública do `gerador_ng`):
  `crates/build/tests/ng_transparencia.rs` — 60 saídas pelo motor iguais às
  do gerador chamado direto, 58 conferidas com o oráculo de `corpus/ngdart`,
  **0 diferentes**; incremental = do zero com edição de template e arquivo
  novo. No `new_sali/frontend`, `dartforge build --comparar` (placar contra o
  apoio do `build_runner`): **151 iguais / 1.410 pendentes / 0 diferentes**
  (145 `.template.dart` e 6 `.css.shim.dart` nativos), e as contagens de
  saídas esperadas iguais às do `.dart_tool/build/generated` (773
  `.template.dart`, 192 `.css`, 192 `.css.map`, 202 `.css.shim.dart`, 202
  `.css.dart`). O Sass nativo é **não verificado** (porta de igualdade: só
  mede, publica o apoio): 7 de 181 `.css` sairiam iguais.
* **`compile-js` do `new_sali/frontend` com o motor** (padrão quando há
  `build_runner`; o `emit_js` não depende mais do `gerador_ng`): 575 módulos,
  **574 byte a byte iguais** aos do `DARTFORGE_GERADOS=ng` de antes (579
  módulos); o do `limitless_ui` difere só por absorver os 4 `.css.dart` que o
  fluxo antigo lia do `.dart_tool/build/generated` (caminho fora do pub
  cache, então módulo próprio) e o motor publica no caminho natural, como os
  demais gerados do pacote.
* **Latência de edição no `new_sali/frontend`** (`scripts/medir-geracao.ps1`,
  sessão viva, 3 repetições aplicadas e revertidas, apoio = o
  `.dart_tool/build/generated` que já existe, máquina carregada):

  | edição | motor | nativo (estágio A) | recarga das geradas | ações | saídas alteradas | unidades / módulos |
  |---|---|---|---|---|---|---|
  | texto no `.html` de um componente coberto | 0,68–0,91 s | 0,65–0,88 s | 0,28–0,37 s | 454 | 1 | 1 / 1 |
  | propriedade no `.scss` do componente | 0,66–0,72 s | 0,64–0,69 s | 0,28–0,32 s | 454 | 1 (`.css.shim.dart`) | 1 / 1 |
  | corpo no `.dart` do componente, fora do template | 0,67–0,91 s | 0,64–0,87 s | — | 454 | **0** (corte pela saída) | 1 / 1 |
  | corpo em `.dart` sem Angular | **< 1 ms** | — | — | **0** | 0 | 1 / 1 |

  O estágio A reexecuta o pacote inteiro (`gerar_com_apoio` ≈ 0,52 s +
  consultas ≈ 0,14 s) a cada edição de arquivo Angular: **acima dos 500 ms**
  da meta; o estágio B (ação por componente) depende do
  `docs/BUILD-PEDIDOS-GERADOR-NG.md`. O que o usuário espera ainda é
  dominado pela escrita do módulo de 41 MB em que o ciclo de imports do app
  funde as bibliotecas (3–13 s nesta máquina, com ou sem motor); sem o motor,
  a mesma sessão leva 0,42–0,47 s fora a escrita numa edição de corpo.
  `@Input` novo num filho não foi medido (só faz sentido no estágio B).
* **Custo zero** (regra governante, PLANO.md): portão estrutural
  `crates/dev/tests/custo_zero.rs` verde — num projeto sem `build_runner`,
  nenhum motor construído (`instancias() == 0`), relatório sem motor e a
  **mesma contagem de alocações** por edição que uma sessão construída sem
  etapas; portão de tempo no `pesado.yml` (job `custo-zero`, commit × main,
  5 rodadas alternadas, tolerância 3%), verde nas duas rodadas:
  35847625760 — corpus JS inteiro **11,36 × 11,56 s** (razão 1,017), edição
  de corpo numa sessão sintética de 300 bibliotecas **34 × 35 ms** (1,029);
  35850781174 (depois do merge do `main`) — **11,53 × 11,62 s** (1,008) e
  **35 × 34 ms** (0,971). O ruído entre rodadas do mesmo binário chegou a 14%
  (11,1–12,7 s): a mediana de 5 cabe nos 3%, mas por pouco; se o portão
  oscilar, a tolerância sobe com a medição registrada aqui. Local no
  `new_sali/core` (`scripts/medir-custo-zero.ps1`, média do platô de 20
  edições de corpo, 5 rodadas alternadas, máquina carregada por outros
  agentes): base **536 ms** × atual **465 ms** de mediana (0,87; faixas
  419–714 e 393–613 ms) — sem piora; os 227 ms do §1.3 são de máquina
  livre.

---

## 2. O que falta

### 2.0 O compilador de visões do ngdart

Sem ele, `dartforge serve` ainda depende de o `build_runner` ter rodado
uma vez no projeto (os 114 arquivos pendentes vêm do disco). Medido no
new_sali/frontend em 2026-09-23 (`oraculo --listar`, fim da rodada 3),
com o conjunto completo de recusas de cada arquivo e cada uma contada pela
sub-forma:

| motivo | aparece em | destrava sozinho |
|---|---|---|
| ligação no template | 65 | 3 |
| diretiva casada por seletor | 40 | 4 |
| interpolação | 37 | 1 |
| `@ViewChild` em visão embutida / `@ViewChildren` | 34 | 3 |
| ligação em componente filho | 31 | 3 |
| `@ViewChild` de componente ou diretiva | 31 | 4 |
| folha de estilo | 27 | 5 |
| evento | 21 | 0 |
| `providers: [..]` | 11 | 2 |

As sub-formas mais frequentes:

| sub-forma | aparece em | destrava sozinha |
|---|---|---|
| interpolação: tipo desconhecido de propriedade | 31 | 0 |
| `@ViewChild` de `#ref` em componente filho | 27 | 4 |
| Sass ou CSS fora do subconjunto | 25 | 5 |
| local de `*ngFor` sem o tipo do elemento | 23 | 1 |
| `@ViewChild` sem `#ref` / de `#ref` em visão embutida | 20 / 19 | 3 / 0 |
| nome fora do componente (ligação / evento) | 17 / 17 | 0 / 0 |
| `#ref` usado em expressão / em visão embutida | 15 / 14 | 0 / 0 |
| dependência de diretiva de fora do nó (injetor) | 13 | 2 |
| `<template>` escrito no template | 11 | 0 |
| `providers:` no próprio componente | 11 | 2 |
| `#ref` com valor (`#f="ngForm"`) | 10 | 0 |

O que a rodada 3 destravou, sobre os 162 de antes: metadados lidos do
programa no lugar do catálogo +0 (a mesma saída, 0 diferentes); page
header (seletor composto, diretiva em tag não HTML, `@ContentChildren`
preenchido) +8; `select`/`option` e diretivas do projeto (`@Host`,
`OnDestroy`, ouvintes agrupados) +4; `NgModel` em componente, provedor
preguiçoso, `XNgCd` de validador usado, `style`/`tabindex` escritos +12.
As sub-formas "diretiva X" que dominavam (page header 47/43,
`CustomSelectControlValueAccessor` 28, `CustomNgSelectOption` 20,
`NgModel` em componente 28, `MaxLengthValidator` 17, `style` 34) saíram
da lista. O que domina agora é tipagem de expressão (membro de tipo
desconhecido, local de `*ngFor`) e `#ref`/`@ViewChild` fora da visão raiz.
Nada disso é adivinhável: cada forma tem a sua regra no `ngcompiler` e o
arquivo oficial correspondente serve de teste byte a byte.

### 2.1 Correção (ordem de prioridade)

1. **Lacunas de inferência de tipos** (o `dart analyze` oficial dá 0
   diagnósticos nos projetos: todo aviso nosso é falso positivo). Medido
   em 2026-09-23 com o oráculo (`tools/oraculo_tipos`, método em
   `docs/FRONTEND-NEW-SALI.md`): `new_sali/core` 13.431 → **15** avisos,
   `frontend` 57.883 → **28** (17 em templates gerados, legítimos);
   divergências de tipo estático por expressão 137.428 → 31 (core) e
   316.756 → 56 (frontend); corpus `corpus/inferencia` 83/95 programas
   iguais ao oráculo (teste no CI). Os
   grupos restantes, por causa, estão em `docs/FRONTEND-NEW-SALI.md`.
   O contrato é `docs/INFERENCIA-ESPECIFICACAO.md` (regras do analyzer
   6.11 com arquivo:linha, e o que muda até 3.14), com o corpus de
   conformidade `corpus/inferencia/` (95 programas, tipos gravados pelo
   oráculo) e a fila de lacunas do motor em `docs/INFERENCIA-LACUNAS.md`.
2. **Escrita em disco** no `limitless_ui`: 233 s para 484 arquivos, contra
   10,5 s de compilação. É I/O do Windows com antivírus, não compilador —
   o `dartforge dev` já contorna (reescreveu 58 arquivos na recompilação),
   mas o `compile-js` de projeto grande ainda sofre.
3. **Tamanho do JS**: 48 MB (dev, sem tree shaking) contra 4,5 MB do
   `main.dart.js` do dart2js em release. A comparação só será válida
   contra o DDC em modo de desenvolvimento, ou depois do modo de produção
   (§2.3). Medição pendente.
4. **`new_sali/backend`** (angel3): não compila; usa `dart:io`,
   `dart:isolate`, `dart:ffi`. Alvo do backend nativo.
5. Divergências web×VM declaradas (7 programas do corpus): são do próprio
   DDC (bits de 32 bits, `1.0` imprimindo `1`, `-0.0`), não defeitos.

### 2.2 Latência (o caminho está medido, não é chute)

1. **`Program` e outline reconstruídos a cada compilação** — 160 ms no
   core, 277 ms no frontend, 55–83% do que resta numa edição. Precisa de
   **ids estáveis por biblioteca** (arenas de elementos por biblioteca)
   para reaproveitar o outline das bibliotecas intactas. Com isso a edição
   de corpo cai para a faixa de 60–80 ms (estimado a partir das fases já
   medidas).
2. **Camada de texto da emissão**: 9,76 M alocações por compilação
   completa, ~7,3 M delas em `String` por nó de expressão. A correção é o
   **buffer único por módulo** (`emit_expr` escreve direto e devolve a
   faixa, em vez de devolver `Js`). Estimado: −6 M alocações.
3. Emissão paralela por módulo (hoje é série).

### 2.3 Modo de produção

**Existe, e é o de §1.2.1**, com o mundo fechado sobre a nossa trilha. O
que ainda falta, na ordem do `docs/JS-PRODUCAO.md` §6:

* precisão do mundo: restrição pelo tipo do receptor e espécie de seletor,
  o que o `limitless_ui` pede;
* suíte e2e do `limitless_ui` com o bundle de produção;
* despacho direto por alvo único;
* minificação;
* deduplicação de funções na trilha tipada;
* code splitting (`deferred` virando `import()`).

Achado de passagem: a ordem dos encaminhadores de `noSuchMethod`
(`Ctx::unimplemented_abstract`, iteração de `HashMap`) muda entre execuções
do mesmo binário. É um não determinismo do emissor anterior a este trabalho.

E o limite estrutural, medido e registrado: **enquanto o runtime for o
`dart_sdk.js` do DDC, o piso é da ordem de 1 MB, não os 35 KB do
`dart2js`**. Ele chega lá porque compila o SDK do fonte com inferência
global; o nosso vem pré-compilado, com tabela de assinaturas, receitas rti
e métodos de extensão emitidos para qualquer uso possível — dá para apagar
a classe ou o membro inteiro, nunca a metade do metadado que sobra. Fechar
esses 30× é o passo 4 do PLANO (compilar o SDK pela nossa trilha), não uma
otimização a mais.

### 2.4 LSP

Falta tudo além de diagnósticos: hover, ir para definição, referências,
completion, rename, code actions — e a semântica (`crates/types`) por trás
do `trait Analisador`, que hoje só tem a implementação sintática.

Diagnósticos semânticos (plano A1/A2 feitos; ver §1.4): a lista de
verificados está vazia. Nenhum código semântico tem 100% no corpus com 0 FP
nos projetos, então o editor só recebe sintaxe. O caminho até lá:

1. **T1 em `types`**: emitir `code`/`args` (hoje a `paridade/src/ponte.rs`
   reconhece os moldes em português) e dizer a unidade de cada diagnóstico
   (hoje uma passada de inferência por biblioteca e casamento de intervalo:
   290 atribuições ambíguas no corpus), e acertar posição e argumentos
   (`undefined_function`, `return_of_invalid_type_from_*`).
2. **Parser com `ParserErrorCode`** (métrica separada, não bloqueia):
   a recuperação difere da do fasta em `expected_token`.
3. **A3** (warnings: `unused_local_variable` 2.315 FN, `unused_import`,
   `unused_element`, `dead_code`) e o `ErrorVerifier` (`duplicate_definition`,
   `type_argument_not_matching_bounds`...).
4. Os FP dos projetos reais: 298 internos (53 + 29 + 216). O portão dos
   códigos que dependem de tipo é o oráculo de tipos de `types`
   (`examples/comparar_tipos`): um código desses só entra na lista quando as
   divergências de tipo nas bibliotecas do corpus zerarem.

### 2.5 Backend nativo

**Depois de P1–P4 (Pesado 35871381320): 142 dos 223 falham.** Quase todos
por membro do SDK sem implementação no runtime (`where`, `map`, `fold`,
`toStringAsFixed`, `sort`, `List.filled`/`List.generate`, `parse`,
`hashCode`…, que o P5 resolve com o SDK da fonte), `await`/`yield` (P6/P7)
e o `toString()` de objeto do programa dentro de uma coleção impressa (o
runtime não chama código Dart). Da linguagem, faltam os genéricos em tempo
de execução (RTI: `is List<int>` é diagnóstico) e `super` dentro de mixin.
A tabela abaixo é a de antes da rodada 2.

172 dos 222 programas do corpus (CI, run 35823269758), agrupados pelo
relatório do harness (`--nativo`). A família de "handle" (71 de 214 no
começo — escalar usado como handle, null desreferenciado, raiz faltando)
**zerou**, e a natureza do que falta mudou: quase tudo agora é construto
que o backend declara não suportar, com o nome do construto no placar.

| falhas | causa |
| --- | --- |
| 164 | **erro de compilação: não suportado** — em 85 grupos; os maiores: chamada de valor de função/closure (`f`, `cb`, `callback`, 9 + 6), `switch` (expressão 8, comando 6), aritmética sobre `num`/`dynamic` (8), `super` como valor (6), `await`/`yield` (5 + 4), cascata (4), constante de enum (4), `hashCode`/`Object.hash` (4), `int.parse`/`double.parse` (4), e membros do SDK sem implementação no runtime (`clear`, `sort`, `addAll`, `toStringAsFixed`, `abs`, `where`…) |
| 4 | roda, mas imprime diferente da VM (`27_operadores_logicos_curto_circuito`, `66_colecoes_literais_spread_if_for`, `156_antigo_generics_constants`, `178_antigo_nativo_mixins`) |
| 3 | nem carrega: `dart:js`, `dart:js_util`, `dart:js_interop` (não existem na seção `vm`) |
| 1 | o Clang ainda recusa o IR |

Os mesmos 50 passam sob `--gc-stress` (coleta antes de toda alocação; run
35823275126). A lista do que fazer agora é a coluna de construtos acima,
do maior grupo para o menor; closures (com captura em célula) e `switch`
desbloqueiam mais que qualquer outro item. Depois disso, o passo 6 do
contrato (raízes só onde há ponto de coleta, pela tabela de efeitos das
externs; raiz como `store` num quadro em memória) — só com medição.

O lowering de exceções existe (`throw`/`try`/`catch`/`finally`/`rethrow`,
com o `finally` como sub-rotina e discriminador de razão); falta acertar os
textos de `toString` dos erros do `dart:core`, que o corpus compara byte a
byte. Continuam faltando `async` e event loop, genéricos reificados,
`dart:io`, isolates e o `dart:core` da seção `vm` a partir da fonte. O
cache de objeto **por programa** existe (§3.2); **por módulo**, com o SDK
compartilhado entre programas (o resumo por biblioteca de
`docs/PESQUISA-OTIMIZACAO.md` §6), depende de separar o SDK em módulo
próprio com símbolos e ids estáveis — hoje não há o que separar: 0% do IR
é corpo de função do SDK (§3.2).

### 2.6 ngdart e geração de código

O compilador de templates próprio **existe e cobre 134 dos 300 arquivos**
do `new_sali/frontend` (§1.7). O que falta dele está na tabela do §2.0.
Enquanto não fecha, compilar um projeto ngdart ainda exige
`dart run build_runner build` uma vez (2m59s no `new_sali/frontend`) para
os arquivos pendentes — o motor de build (§1.8) usa o que ele deixou no
disco como **apoio** e avisa quando o apoio está mais velho que a fonte.

O que falta do motor (§1.8), em ordem: o **estágio B** do ngdart (uma ação
por componente, consultas finas), que depende dos acréscimos públicos
pedidos ao `gerador_ng` em `docs/BUILD-PEDIDOS-GERADOR-NG.md` e é o que
leva a edição de componente para baixo de 500 ms; o Sass byte a byte do
`sass_builder` (mesmo documento, item 4), para o `.css` servido sair do
nativo; o executor Dart (`dfexec/1`, `docs/BUILD-PROTOCOLO.md`), que é o
executor nativo compartilhado com as macros e o que tira os 61 pendentes do
`corpus/builders`; e o `go_router_builder` no corpus (D-B4, exige Flutter).

Plano para substituir o `build_runner` por um motor em Rust:
`docs/BUILD-RUST.md`. O dado que o orienta: dos 9.879 artefatos que o
`build_runner` gera nesse projeto, **934 `.ddc.js`/`.ddc.dill` e ~7.980
de bookkeeping são do `build_web_compilers`** — o compilador que já
substituímos. Nos dois projetos do proprietário sobram três builders
(`ngdart`, `i18n`, `sass_builder`).

**Regra governante (PLANO.md): tudo nosso, compatível com o
ecossistema.** A VM oficial não faz parte do produto; `json_serializable`,
`freezed`, `drift`, `mockito`, `source_gen` e os demais têm de funcionar
**no nosso runtime**. Isso ordena as prioridades do backend nativo: o
alvo dominante é compilar e executar o `package:analyzer` (438 arquivos,
227.252 linhas, `dart:io`/`isolate`/`ffi`/`typed_data`), de onde esses
geradores dependem. Geradores nativos em Rust são aceleração opcional,
com saída byte a byte igual verificada por `corpus/builders/` (ainda não
existe). O `dart` oficial fica só como oráculo de comparação.

### 2.7 JIT

O R0 funciona (§1.5.1): `run` e `reload` sobre o mesmo IR do `emit_native`.
Falta:

* **recarga com estado (R1)** — `docs/PESQUISA-HOT-RELOAD.md`. Os cenários
  estão em `crates/jit/testes-pendentes/hot_reload.rs`, escritos contra a
  trilha velha (não compilam; ficam fora de `tests/` como a lista do que
  migrar para o IR do `emit_native`);
* preservar o contrato de `crates/jit/tests/execucao.rs` — o mesmo programa
  compila uma vez e executa pelos dois caminhos (ORCv2 e AOT) exigindo saída
  idêntica. Divergir em tempo de compilação é esperado; em resultado, é
  defeito;
* a recomendação de `docs/historico/CRANELIFT.md` continua: **não adotar**
  Cranelift. O experimento foi feito e medido (o crate saiu do workspace e
  está na branch `exploracao-inicial`); quem for mexer em JIT lê isso antes
  de repetir.

---

## 3. Como verificar tudo isto

### 3.0 No GitHub Actions — o caminho normal para o que é pesado

A máquina de desenvolvimento (8 GB, compartilhada por vários agentes) não
roda mais corpus nenhum: o repositório é público, os minutos de Actions são
gratuitos, e os runners Windows têm 4 núcleos e 16 GB. Dois workflows:

* **`ci.yml`** — todo push no `main` e em `ci/**`, todo PR; ~2,5 min.
  `cargo test --workspace` (todos os alvos e os doctests), os
  `#[ignore]` que o runner satisfaz (SDK Dart 3.6.2, Clang
  22.1.8, rustc) e `cargo doc -D warnings`. O que fica de fora e por quê
  está no cabeçalho dele e em §1.6.
* **`pesado.yml`** — push em `ci/**`, `workflow_dispatch` (suíte e número de
  fragmentos) e todo dia às 03:17 (Brasília) no `main`. Medido na rodada de
  2026-09-23 (`ci/infra`, 214 programas):

| job | o que roda | placar | tempo do job (harness) |
| --- | --- | --- | --- |
| compilar | release de `dartforge-diferencial`, `dartforge`, `dartforge-jsprod`; completa o cache dos oráculos (`verificar`) | — | 2,2 min |
| js (desenvolvimento) | `dartforge-diferencial --jobs 4` | **214/214** | 1,2 min (45 s) |
| js (produção) | `--producao --jobs 4` | **214/214** e 214/214 | 2,3 min (1 min 48 s) |
| nativo K/2 | `--nativo --jobs 4 --fragmento K/2` | 4/107 e 3/107 | 1,5 e 1,7 min (28 e 30 s) |
| nativo (placar consolidado) | soma os fragmentos e funde o agrupamento de falhas | **7/214** | 0,5 min |
| determinismo (produção) | `determinismo --producao --trabalhadores 1,4,8` | idêntico | 7,5 min (7 min) |
| determinismo (nativo, IR) | `determinismo --nativo --trabalhadores 1,4,8` | idêntico, 184 com IR | 0,6 min (5 s) |
| moderno (3.7–3.13) | `--corpus corpus/moderno --producao --jobs 4`, com o SDK 3.13.4 instalado por zip (cache pela versão); `PENDENTES` pode falhar, pendente que passa reprova | ver §1.1.1 | — |
| **rodada inteira** | | | **9,8 min** |

**Critério de cada job.** JS desenvolvimento e produção: código de saída do
harness 0, isto é, **todos os programas do corpus** passam — qualquer que
seja o tamanho do corpus, sem número fixo. Nativo: placar abaixo do total
é trabalho em andamento e não reprova; reprova se o harness quebrar
(código fora de {0, 1} ou relatório sem a linha de placar). Determinismo:
código 0. Cada job escreve placar, tempo, memória livre mínima e o
agrupamento de falhas no resumo da rodada (`$GITHUB_STEP_SUMMARY`), deixa
o placar numa anotação (aparece em `gh run view`) e sobe o relatório
completo como artefato `relatorio-<job>` (14 dias).

**Convenção: uma branch `ci/<frente>` por frente de trabalho** (`ci/nativo`,
`ci/ngdart`, `ci/verificacao`, `ci/infra`…). Empurrar para ela roda o
`pesado.yml` inteiro e o `ci.yml` sobre aquele commit, sem mexer no `main`.
Rodadas de branches diferentes **nunca se cancelam** (o grupo de
concorrência inclui a ref); uma rodada nova na **mesma** branch cancela a
anterior. As branches `ci/**` são descartáveis: o push é forçado.

```powershell
pwsh scripts/ci.ps1 -Frente nativo -Acompanhar   # HEAD -> ci/nativo, acompanha até o fim
pwsh scripts/ci.ps1 -Listar                      # última rodada de cada ci/** e do main, com placar
pwsh scripts/ci.ps1 -Placar <run-id>             # placar de cada job + relatórios baixados
pwsh scripts/ci.ps1 -Suite nativo -Ref ci/nativo -Fragmentos 4   # workflow_dispatch
gh run view <run-id> --log-failed                # o log do que falhou
gh run download <run-id> -p 'relatorio-*'        # relatórios completos
```

`-Suite` (workflow_dispatch) só funciona depois que o `pesado.yml` estiver
no `main`; até lá, `-Frente`.

**Limites do plano gratuito, e o dimensionamento.**

* **20 jobs simultâneos na conta inteira** (Linux e Windows juntos); o
  excedente espera na fila, não falha. Uma rodada `todos` com N fragmentos
  tem pico de 4 + N jobs: com o **N = 2 padrão**, 6 — cabem três frentes ao
  mesmo tempo. N sai da medição (§3.2): o corpus nativo inteiro custa ~1 min
  de harness, e cada job gasta ~1,3 min só preparando o ambiente. Subir N
  (`-Fragmentos`, ou `gh variable set FRAGMENTOS_NATIVO --body N`) quando um
  fragmento passar de ~20 min.
* **10 GB de cache por repositório**, e o que foi usado há mais tempo sai
  primeiro. Um branch só lê o próprio cache e o do `main`; por isso: o
  `rust-cache` só é **gravado no `main`** (as `ci/**` restauram o do main e
  não multiplicam entradas); o Clang 22.1.8 tem chave fixa pela versão
  (`llvm-22.1.8-windows-x64-clang-v2`); os oráculos (`dart run`,
  `dartdevc`+Node) têm chave pelo hash de `corpus/js/**` e o `restore-keys`
  traz o anterior, de modo que só programas alterados são recalculados
  (~55 KB). O SDK Dart não é cacheado: o `setup-dart` o baixa em ~10 s. O
  agendamento diário no `main` é o que mantém esses caches no escopo que
  todas as `ci/**` leem.

### Na máquina local

```powershell
pwsh scripts/gerar-dart-sdk.ps1              # dart_sdk.js (3 s, uma vez)
cargo build --release -p dartforge-cli -p dartforge-diferencial
cargo run --release -p dartforge-diferencial # 214/214

# projeto real
cargo run --release -p dartforge-cli -- compile-js `
  C:/MyDartProjects/new_sali/frontend/web/main.dart -o saida `
  --packages C:/MyDartProjects/new_sali/frontend/.dart_tool/package_config.json
pwsh scripts/servir.ps1 -Dir saida -Web C:/MyDartProjects/new_sali/frontend/web -Fluxo

# sessão residente
cargo run --release -p dartforge-cli -- dev <entrada.dart> -o saida --packages <cfg>
```

## 3.1 Espaço em disco — vigiar

O `target/` do Cargo **não se limpa sozinho**: num dia de trabalho com
vários agentes chegou a **38 GB** (21 GB em `debug/deps`, 8 GB de
compilação incremental), tudo recriável e nada de código. Rode

```powershell
pwsh scripts/limpar.ps1            # relata o que ocupa espaço
pwsh scripts/limpar.ps1 -Limpar    # apaga o lixo seguro (debug, target-*, incremental)
pwsh scripts/limpar.ps1 -Limpar -Tudo  # também release e o cache de oráculos
```

Custo de recriar: `target/debug` ~10 min, `target/diferencial` (cache dos
oráculos `dart run`) ~10 min, `target/release` ~5 min. Sem custo, e
apagados sempre: `target/diferencial/nativo` (executáveis e `.ll` do corpus
nativo; o harness já apaga cada `.exe` depois de executar, salvo
`DARTFORGE_KEEP_EXE`) e as `.lib` do runtime em `target/native_cache/` fora
as 2 mais recentes (o próprio cache também poda). Só com `-Tudo`:
`target/native_cache/obj`, o cache de objeto, que se poda sozinho no teto de
`DARTFORGE_CACHE_OBJ_MB` (256 MB). `DARTFORGE_CACHE_NATIVO` muda o
diretório; sem ele, `$CARGO_TARGET_DIR/native_cache`. Worktrees de
agentes têm cada uma o seu `target/` — removê-las (`git worktree remove`)
depois de integrar o trabalho é parte da limpeza.

**Regra dos temporários: no D:, nunca no C:.** O C: tem pouco espaço e o
`%TEMP%` chegou a 10,5 GB de sobras. Temporários grandes vão para
`target/tmp-*` ou `target/scratch-*` do repositório (os scripts de
navegador, `medir-lsp.ps1`, `verificar-poda-js.ps1` e `ci.ps1 -Placar`
gravam lá); agentes e sessões longas definem antes
`$env:TEMP='<repo>\target\tmp-<frente>'; $env:TMP=$env:TEMP` e apagam a
pasta no fim. Todo teste ou comando que cria um temporário o apaga também
no caminho de erro, pânico ou tempo esgotado (`tempfile::tempdir`, ou um
guarda com `Drop` como `OutputDir` em `crates/jit/tests/execucao.rs` e
`DiretorioGeracoes` no `dartforge reload`). `limpar.ps1 -Limpar` apaga
`target/tmp-*`, `target/scratch-*` e as sobras antigas no `%TEMP%`
(`dartforge-*`, `dfserve*`, `df-recarga`, `lsp-stdout-*`).

### 3.2 Verificação rápida do backend nativo — medido

O corpus nativo foi registrado aqui como **10 programas em 9 minutos**
(~3 h por passada, ~10 h para o determinismo), e o determinismo só tinha
sido verificado num subconjunto. As três saídas propostas estão feitas,
seguindo `docs/PESQUISA-OTIMIZACAO.md` §11 (determinismo com 1, 4 e 8
trabalhadores) e §6 (resumo e cache por módulo):

1. **Determinismo não executa.** `dartforge-diferencial determinismo
   --nativo` emite o LLVM IR de cada programa dentro do processo
   (`emitir_ir`, sem Clang, ligação nem execução) e compara, programa a
   programa, o resumo FNV-1a de 128 bits — ou a mensagem inteira do erro.
   Divergência lista todos os programas e grava o primeiro, emitido sozinho
   e `n` vezes em paralelo, em `target/diferencial/determinismo/`.
   `--executar` mantém o caminho antigo. O relatório (uma linha
   `<hash> <bytes> <nome>` por programa) não tem tempos nem caminhos: um
   `diff` de dois relatórios diz quais programas mudaram de IR.
2. **Cache de objeto por programa** (`cache_objeto.rs`): mesma entrada,
   mesmo hash, mesmo `.obj`. O cache do runtime ganhou chave estável
   (versão do `rustc` + bandeiras + fonte), publicação atômica e uma
   compilação por processo — antes, um trabalhador podia ligar contra uma
   `.lib` pela metade.
3. A **amostra estratificada** não foi feita: com os números abaixo, a
   passada de determinismo inteira custa menos que escolher a amostra.

Números (release, máquina de 8 núcleos e 7,7 GB compartilhados):

| medida | valor |
| --- | --- |
| emissão de um programa (front-end + HIR + LLVM IR) | 1–40 ms; HIR e LLVM IR < 1 ms |
| LLVM IR por programa | 13–120 KB, média 30 KB (184 programas) |
| fração do IR que é corpo de função do SDK | **0%** |
| pico de memória de `compile-native` | 6 MB |
| determinismo IR, 214 programas, 1/4/8 trabalhadores | **0,5 / 0,2 / 0,2 s** por passada; 3,8 s o processo inteiro |
| pico do harness no determinismo IR, 8 emissões simultâneas | 10 MB |
| Clang `-O0` de um programa, morno | 33–84 ms |
| Clang com acerto no cache de objeto | 21 ms (é o `clang --version`) |
| ligação, morna | ~95 ms |
| runtime (`rustc -O`), uma vez por conteúdo | ~10 s |

Por que é tão barato: a seção `vm` do `libraries.json` só declara
`dart:cli`, e o `include` de `vm_common` ainda não é seguido — o programa é
compilado sem o SDK, e é por isso também que 30 programas nem carregam
(`dart:math`, `dart:async`, `dart:collection`…). Quando o SDK entrar, cada
emissão analisa o SDK inteiro (no JS isso custa centenas de MB, §1.3); por
isso o harness separa emissões simultâneas de trabalhadores:
`DARTFORGE_IR_PARALELO_MAX` (padrão 2) limita as emissões físicas, e a
ordem de conclusão continua variando, que é o que o teste precisa. O
cache por **módulo** — o SDK compilado uma vez e compartilhado entre
programas — só paga depois disso, e exige símbolos e ids de classe estáveis
(hoje são índices globais dependentes da ordem de carga).

Reprodutibilidade do objeto, medida: o Clang gravava o `TimeDateStamp` no
cabeçalho COFF, e o mesmo IR dava objetos diferentes no byte 4;
`-mno-incremental-linker-compatible` zera, e agora dois Clang sobre o mesmo
IR dão o mesmo `.obj`. O Clang também roda no diretório do objeto com o
nome relativo (o hash), para o `source_filename` não levar o caminho de
quem compilou.

**A passada executada completa roda no CI, em fragmentos, e foi medida**
(`pesado.yml`, §3.0; runner Windows de 4 núcleos e 16 GB, `--jobs 4`,
`DARTFORGE_HEAP_MAX_MB=256`): **7/214**, o mesmo placar da máquina local,
em **28 e 30 s de harness** para os dois fragmentos de 107 programas —
cada job inteiro, com a preparação, 1,5–1,7 min. Com 8 fragmentos, cada um
levou 5–10 s. A memória livre do runner nunca caiu abaixo de 12,8 de 16 GB.
Ou seja: os **9 minutos para 10 programas** eram da máquina local, não do
backend — sem oráculos em cache, disputando memória e disco com os outros
agentes. O tempo desta passada vai crescer quando mais programas rodarem
até o fim (hoje a maioria falha cedo); é aí que N sobe.

Medido também no CI, antes do cache do runtime com `OnceLock` e publicação
atômica chegar ao `main`: num runner novo, **62 dos 214 programas** caíam
em "compilação do runtime nativo com rustc falhou" — os trabalhadores
compilavam a mesma `.lib` ao mesmo tempo. Na máquina local o cache já
existia e a corrida não aparecia. Com a correção, a rodada seguinte, sem
aquecimento nenhum, deu 7/214 sem nenhuma dessas falhas.

## 4. Organização do repositório

* **Crates (todos em uso)**: `frontend` → `elements` → `types` →
  `emit_js` | `emit_native`, mais `emit_js_producao` e `mundo` (produção
  JS), `gerador_ng` (ngdart), `runtime` e `jit` (nativo), `abi`
  (`abi-info`), `dev`, `lsp`, `intern`, `diagnostics`, `instrument`,
  `diferencial` e `cli`. O workspace é `crates/*`, sem exclusões.
* **A trilha velha foi removida** em 2026-09-23: `lexer`, `syntax`,
  `parser`, `semantic`, `hir`, `codegen`, `linker`, `optimizer`,
  `packages`, `compiler`, `macros`, `llvm`, o driver `native` e o
  `cranelift-jit` (~65 mil linhas), com `tests/` da raiz (fixtures de
  conformidade, já convertidos em `corpus/js/*_antigo_*`) e os scripts
  `conformance*.ps1`/`benchmark-process.ps1`. Antes disso já tinham saído
  `web`, `ngdart` e `asmjit-jit`.
* **Branch `exploracao-inicial`** (`origin`, db75b90): preserva a árvore
  inteira antes da remoção, incluindo tudo o que foi removido.
* **Documentação**: `docs/` tem só o vigente; o que descreve a exploração
  inicial (incrementos 01–25, contratos do subconjunto antigo, CRANELIFT,
  ASMJIT, relatórios JSON e briefs de agentes já cumpridos) está em
  `docs/historico/`, com um README.
* Decisões e contratos: `PLANO.md` (governante), `docs/EMISSAO-DDC.md`,
  `docs/FRONTEND-ARQUITETURA.md`, `docs/NATIVO.md`, `docs/NATIVO-PLANO.md`,
  `docs/JIT.md`, `docs/JS-PRODUCAO.md`, `docs/LSP.md`,
  `docs/FRONTEND-NEW-SALI.md`, `docs/LIMITLESS-UI.md`,
  `docs/CONTRATO-DDC.md`, `docs/BUILD-RUST.md`, `docs/GERADOR-NG.md`.

**Armadilha registrada**: não rodar `compile-js` enquanto o `build_runner`
está rodando — o `--delete-conflicting-outputs` apaga a árvore de gerados
e o carregador falha em dezenas de `.template.dart`.
