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
| Inferência de corpos, fluxo, constantes | **funcional, com lacunas** | 120.055 expressões do SDK em 155 ms; 40/40 negativos do `analyzer`; ~17 mil avisos no `new_sali/core` (ver §2.1) |

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

### 1.5 Backend nativo — `crates/emit_native` (feature `nativo`)

Trilha nova → HIR própria → LLVM IR → Clang → executável, com o runtime
Rust (GC por tracing). `dartforge compile-native` (compile com
`cargo build -p dartforge-cli --features nativo`). Estado do corpus
nativo no fim da sessão: ~6/202 (strings, classes, interpolação, coleções
básicas, `StringBuffer`, `for-in`, runas). Ver `docs/NATIVO.md`.

### 1.6 Infraestrutura

* `crates/diferencial` — harness paralelo com cache dos oráculos;
  `corpus/js/` com 213 programas verificados na VM.
* `docs/CONTRATO-DDC.md` — Dart e JS do `dartdevc` lado a lado para os 212.
* `scripts/` — `gerar-dart-sdk.ps1`, `servir.ps1`/`fluxo.mjs` (Edge por
  CDP), `limitless-ui.ps1` (`-Preparar`/`-Montar`/`-Servir`/`-E2e`),
  `medir-lsp.ps1`.
* Cache do outline do SDK em `target/dartforge/sdk-<hash>.bin` (5 ms para
  ler, contra ~105 ms de reanálise).

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
* `crates/gerador_ng` — o compilador do ngdart em Rust, com os 284 arquivos
  do `build_runner` de oráculo
  (`cargo run -p dartforge-gerador-ng --example oraculo -- <projeto>`).
  Hoje: **127 de 300 arquivos gerados por nós, 116 iguais byte a byte, 0
  diferentes**. Cobre:
  - biblioteca sem nada de Angular (o arquivo trivial);
  - componente de template estático — elementos HTML, texto e atributos,
    com as regras do oficial (`appendDiv`/`appendSpan`/`appendElement<T>`,
    atributos em ordem alfabética, `updateChildClass`);
  - **injeção no construtor**, resolvida pelo banco semântico: carga em
    duas fases (carregar sem os gerados, gerar, recarregar com a geração),
    `injectorGet` por token e `debugInjectorWrap` sob `isDevMode`, com o
    import da biblioteca que **declara** o tipo e o caminho pela regra do
    `getImportModulePath` do ngcompiler.

  `DARTFORGE_GERADOS=ng` compila com ele, e o que falta continua vindo do
  `build_runner`.

---

## 2. O que falta

### 2.0 O compilador de visões do ngdart

Sem ele, `dartforge serve` ainda depende de o `build_runner` ter rodado
uma vez no projeto (os 174 arquivos pendentes vêm do disco). Medido no
new_sali/frontend, os motivos por que cada pendente não é nosso — com o
conjunto completo, não só o primeiro:

| forma | aparece em | destrava sozinha |
|---|---|---|
| ligação (`[x]`, `(x)`, `[(x)]`, `#ref`, `*ngIf`) | 151 | 2 |
| folha de estilo (`styleUrls`) | 138 | 1 |
| componente/diretiva no template | 119 | 3 |
| interpolação `{{ }}` | 110 | 1 |
| `style` em linha | 34 | 0 |
| `@Directive`/`@Pipe` no arquivo | 10 | 9 |
| `<ng-content>` | 5 | 2 |
| injeção (tipo não resolvido) | 3 | 0 |

A injeção saiu da lista: era o maior bloqueio (137 arquivos) e caiu para 3
quando o gerador passou a ler o tipo do campo nos parâmetros `this.x` —
que é como quase todo componente ngdart recebe as dependências.

O que falta, em ordem do que aparece mais:

1. **Interpolação e detecção de mudança** — `{{ }}` vira `TextBinding` com
   `detectChangesInternal` e `checkBinding`; é a base de toda ligação.
2. **Ligações de propriedade e evento** — `[x]`, `(x)`, `[(x)]`, `#ref`.
3. **Diretivas e componentes no template** — casar seletor, instanciar a
   visão-filha, passar `@Input`/`@Output`. Precisa resolver a lista
   `directives:` da anotação pelo banco semântico.
4. **`*ngIf`/`*ngFor`** — visões embutidas e `ViewContainer`.
5. **Folha de estilo** — gerar também o `<x>.css.shim.dart` (o
   compilador de folhas com `_ngcontent-%ID%`) e os `addShimC`/`addShimE`.
6. **`<ng-content>`** — projeção.

Nada disso é adivinhável: cada forma tem a sua regra no `ngcompiler` e o
arquivo oficial correspondente serve de teste byte a byte.

### 2.1 Correção (ordem de prioridade)

1. **~17 mil avisos de tipos** no `new_sali/core` e ~85 mil no `frontend`.
   Não impedem a execução (o emissor recua para despacho dinâmico, que é
   sempre correto), mas cada aviso é uma inferência que não aconteceu —
   custa tamanho e velocidade no JS. Os dez grupos mais frequentes estão
   listados em `docs/FRONTEND-NEW-SALI.md`; os maiores são argumento
   incompatível `dynamic`→`int`, `num`→`double`, condição sem tipo `bool`
   e nome indefinido em cadeias longas de genéricos.
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

Nada feito. O modo atual é o de desenvolvimento (um módulo por biblioteca,
sem otimização global). Falta, na ordem do PLANO: alcance por símbolos
(tree shaking), inferência global de tipos, desvirtualização,
especialização, `const` propagado, minificação, code splitting (com
`deferred` virando `import()`), e o agrupamento em poucos chunks. A
referência é dart2js e Scala.js; a qualidade-alvo do JS emitido é a do
ReScript.

### 2.4 LSP

Falta tudo além de diagnósticos: hover, ir para definição, referências,
completion, rename, code actions — e a semântica (`crates/types`) por trás
do `trait Analisador`, que hoje só tem a implementação sintática.

### 2.5 Backend nativo

~196 dos 202 programas do corpus. Falta a maior parte: exceções, `async` e
event loop, genéricos reificados, `dart:io`, isolates, o `dart:core` da
seção `vm` a partir da fonte, e o cache de objetos (o Clang/link domina o
tempo).

### 2.6 ngdart e geração de código

O compilador de templates próprio (Fase 5 do PLANO) não existe. Hoje
dependemos dos `.template.dart` que o `build_runner` gera; o carregador já
os sobrepõe (`PackageConfig::generated_root`). Enquanto isso, compilar um
projeto ngdart exige `dart run build_runner build` uma vez (2m59s no
`new_sali/frontend`).

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

---

## 3. Como verificar tudo isto

```powershell
pwsh scripts/gerar-dart-sdk.ps1              # dart_sdk.js (3 s, uma vez)
cargo build --release -p dartforge-cli -p dartforge-diferencial
cargo run --release -p dartforge-diferencial # 212/212

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
oráculos `dart run`) ~10 min, `target/release` ~5 min. Worktrees de
agentes têm cada uma o seu `target/` — removê-las (`git worktree remove`)
depois de integrar o trabalho é parte da limpeza.

## 4. Organização do repositório

* **Trilha nova (em uso)**: `frontend` → `elements` → `types` →
  `emit_js` | `emit_native`, mais `dev`, `lsp`, `intern`, `diagnostics`,
  `instrument`, `diferencial`, `cli`.
* **Trilha velha (só o que o `crates/jit` ainda usa)**: `lexer`, `syntax`,
  `parser`, `semantic`, `hir`, `codegen`, `linker`, `optimizer`,
  `packages`, `compiler`, `macros`. Sai quando o backend nativo novo
  substituir o caminho antigo. O restante (`web`, `ngdart`, `asmjit-jit`)
  já foi removido do `main`.
* **Branch `experimento-inicial`**: preserva a árvore inteira antes da
  limpeza, incluindo tudo o que foi removido.
* Decisões e contratos: `PLANO.md` (governante), `docs/EMISSAO-DDC.md`,
  `docs/FRONTEND-ARQUITETURA.md`, `docs/NATIVO.md`, `docs/LSP.md`,
  `docs/FRONTEND-NEW-SALI.md`, `docs/LIMITLESS-UI.md`,
  `docs/CONTRATO-DDC.md`.

**Armadilha registrada**: não rodar `compile-js` enquanto o `build_runner`
está rodando — o `--delete-conflicting-outputs` apaga a árvore de gerados
e o carregador falha em dezenas de `.template.dart`.
