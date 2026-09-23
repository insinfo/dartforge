# Brief — carregador de programa e outline em `crates/elements`

Você está no repositório DartForge (`D:\Projects\dartforge`), um compilador
Dart → JavaScript em Rust. Leia antes de tocar em código, nesta ordem:

1. `PLANO.md`, seção **"Meta governante — qualquer projeto Dart 3.6 válido no
   dart2js/DDC compila"** (a trilha de 5 passos) e a seção **"Meta de projeto
   — cadeia de ferramentas própria, sem memória gerenciada"**.
2. `docs/FRONTEND-ARQUITETURA.md` inteiro — §2 (memória) e §3 (modelo de
   elementos e SDK a partir da fonte) são o contrato do seu trabalho.
3. `crates/elements/src/model.rs` e `crates/elements/src/sdk.rs` — o esqueleto
   que já existe (tipos do `Program`, `SdkLayout::load` de `libraries.json`).
4. `crates/frontend/src/ast.rs` — a árvore que você consome (`CompilationUnit`,
   `Directive`, `DirectiveKind`, `Decl`, `DeclKind`, `Member`).
5. `crates/packages/src/lib.rs` — o resolvedor antigo de `package_config.json`
   e diretivas condicionais; é referência de regras, não de código a reutilizar
   (ele empresta `&str` da fonte, o que a trilha nova proíbe).

## Regra de coexistência — o que você NÃO toca

Outro agente está editando **agora** `crates/frontend/src/parser/*.rs` e
`crates/frontend/src/lexer.rs` para levar o parser a 100% do SDK. Você não
altera nada em `crates/frontend`. Se precisar de algo que a AST não expõe,
registre em `docs/FRONTEND-ARQUITETURA.md` §7 ("O que ainda não está decidido")
como pendência com o motivo, e siga sem. Também não toca em `crates/parser`,
`crates/syntax`, `crates/semantic`, `crates/packages` — são a trilha antiga, que
continua compilando o que compila até a nova cobri-la.

## O que entregar — passo 2 da trilha, "Modelo de elementos e resolução"

Em `crates/elements`, sobre o esqueleto existente:

### A. `load.rs` — carregador do fecho transitivo de bibliotecas

`pub fn load(entry: &Path, sdk: &SdkLayout, package_config: Option<&Path>,
interner: &mut Interner) -> Result<Program, Vec<Diagnostic>>`.

* Lê o arquivo de entrada, faz `dartforge_frontend::parser::parse`, percorre as
  diretivas e carrega recursivamente: `dart:x` via `SdkLayout` (biblioteca de
  origem **mais os patch files, na ordem do `libraries.json`**, cada um como
  `Unit` com `UnitRole::Patch` da mesma `LibraryId`); `package:x/y.dart` via
  `.dart_tool/package_config.json` v2 (`packageUri`, `rootUri`,
  `languageVersion`); URIs relativas resolvidas contra a unidade que importa.
* `dart:core` é importado implicitamente por toda biblioteca; `Program::core`
  aponta para ela.
* `part 'x.dart'` entra na mesma `LibraryId` com `UnitRole::Part`; verificar
  que o `part of` corresponde (por URI ou por nome de biblioteca).
* Diretivas condicionais `if (dart.library.io) 'x'`: primeira correspondência
  vence; o ambiente é o conjunto de `dart:x` com `supported: true` no
  `SdkLayout` — `dart.library.io` é falso no DDC porque `dart:io` está
  `supported: false`.
* Ciclos de import são normais em Dart (e no SDK): use uma fila com mapa
  URI → `LibraryId` alocado antes de analisar a unidade.
* Cada unidade vira `Unit { uri, path, source, ast, unit, library, role }`;
  a `String` da fonte é dona, a AST não empresta nada (regra §2).
* Erros de leitura ou de sintaxe viram `Diagnostic` **acumulados** — o
  carregador não para no primeiro erro (pré-requisito do LSP, PLANO.md).

### B. `outline.rs` — construção dos elementos e namespaces

Depois de todas as unidades carregadas, uma passada por biblioteca:

* Para cada `Decl` de topo (em todas as unidades da biblioteca, patches
  incluídos), criar o elemento correspondente em `Program::classes` /
  `extensions` / `typedefs` / `functions` / `variables` e inserir em
  `Library::declared`. Getter e setter de topo com o mesmo nome são o mesmo
  `Binding` com `getter`/`setter` distintos.
* **Aplicação de patch**: `@patch class X` no patch funde com a classe `X` da
  origem — os membros do patch substituem os `external` da origem e os demais
  são acrescentados. `@patch` em função/getter/setter de topo idem. Classe ou
  membro só no patch (sem `@patch`) é declaração nova da biblioteca.
* Membros de classe: `ClassElement` com membros por nome (métodos, getters,
  setters, operadores, campos com getter/setter implícitos, construtores
  inclusive o implícito sem nome e os sintéticos de enum: `values`, `index`,
  `name`), supertipos **por nome** (`extends`, `with`, `implements`, `on`) —
  a resolução do nome para `ClassId` acontece no passo D, depois que todos os
  namespaces existem.
* Privacidade: `_x` entra em `declared`, nunca em `exported`.

### C. Namespaces importados e exportados

* `Library::exported` = `declared` sem privados, mais o que cada `export`
  traz, com `show`/`hide` aplicados **em sequência** na ordem escrita.
  Reexports são transitivos e podem ter ciclo: itere até ponto fixo (o CFE faz
  isso; um `export` que nada acrescenta na iteração N termina).
* Namespace importado de cada biblioteca: para cada `import`, o `exported` da
  alvo filtrado pelos combinadores; sem prefixo cai no escopo da biblioteca,
  com prefixo `p` vira `Element::Prefix` e o conteúdo fica acessível por
  `lookup_prefixed`. Dois imports com o mesmo nome apontando para elementos
  diferentes marcam `Binding::ambiguous = true` — o import não é erro, o uso é
  (regra do Dart). Declaração local **sempre** vence import.
* `dart:core` implícito é o último na precedência (qualquer import explícito
  sombreia `core`).

### D. Resolução de supertipos e hierarquia

Com todos os namespaces prontos, resolver cada nome de supertipo pelo escopo
da biblioteca que declara a classe (`lookup` / `lookup_prefixed`) para um
`ClassId`. Detectar ciclo de herança e reportar. Isso fecha o critério de
aceite do passo 2 para supertipos; referências de nome em **corpos** não são
resolvidas aqui (dependem de tipos — passo 3).

## Critério de aceite — teste que precisa existir e passar

`crates/elements/tests/sdk.rs`, `#[ignore]` como em
`crates/frontend/tests/corpus.rs` (depende de `C:/tools/dartsdk-3.6.2/lib`,
sobrescrevível por `DARTFORGE_SDK_LIB`):

1. `todo_dart_x_carrega`: para cada biblioteca de `libraries.json` seção
   `dartdevc`, `load` de um arquivo sintético `import 'dart:x'; void main() {}`
   termina sem diagnóstico e com os patches aplicados (afirme que `dart:core`
   tem a classe `int` com o membro `parse` vindo do patch e sem `external`).
2. `supertipos_do_sdk_resolvem`: toda classe de todas as bibliotecas do SDK
   tem cada supertipo resolvido para um `ClassId`; a lista de falhas sai no
   `assert` como o corpus do frontend faz.
3. `namespaces_do_sdk_sem_ambiguidade_em_uso`: nenhum nome usado em `extends`/
   `implements`/`with` do SDK cai num `Binding::ambiguous`.
4. Um teste unitário sem SDK, com fontes em `tempdir`, cobrindo: `part`,
   prefixo, `show` seguido de `hide`, reexport cíclico entre duas bibliotecas,
   declaração local sombreando import, dois imports ambíguos, diretiva
   condicional.

Enquanto o parser do outro agente não aceitar 100% do SDK, os testes 1–3 podem
reprovar em arquivos que **ainda não parseiam**; nesse caso o `assert` precisa
mostrar separadamente "recusados pelo parser" e "recusados pelo elements", e só
o segundo grupo é responsabilidade sua. Registre o número dos dois grupos na
resposta final.

## Disciplina que o projeto exige

* Documentação Rust em **português**, com contrato (o que faz, o que devolve,
  quando erra) e exemplo quando couber — veja `sdk.rs` e `CONTRIBUTING.md`.
* Sem `Rc`/`Box` por nó, sem `String` para identificador (use `SymbolId`), sem
  empréstimo da fonte na saída. Tudo em `Vec` indexado por id.
* Sem cache sem teto: se criar qualquer mapa que sobreviva a uma chamada de
  `load`, ele nasce com limite e contadores (`hits`/`misses`/`evictions`).
* `cargo fmt`, `cargo clippy -p dartforge-elements --all-targets` limpo,
  `cargo test -p dartforge-elements` verde, e os `#[ignore]` rodados com
  `cargo test -p dartforge-elements -- --ignored` com o resultado colado.
* Não faça commit; o proprietário revisa antes. Não altere `PLANO.md` além de
  marcar, na tabela de §6 de `docs/FRONTEND-ARQUITETURA.md`, o estado real
  medido da fase `elements`.
