# Brief — compilador nativo (LLVM) sobre a trilha nova

Você trabalha no DartForge (`D:\Projects\dartforge`), na worktree
`D:/Projects/dartforge-nativo`, branch `nativo-llvm` (criada com
`git worktree add -b nativo-llvm D:/Projects/dartforge-nativo main`).
Commite só nela; sem push; integração em `main` é feita por rebase e
fast-forward quando `cargo build --workspace` passa. Regra do proprietário
(Princípio 9 do PLANO.md): foco estrito em funcionar; nada de formatação,
estilo ou clippy; documentação Rust curta em português; commit ao fim de
cada bloco verificado; números medidos, não adjetivos.

## Regra de coexistência — o que você NÃO toca

Outros agentes estão no compilador JavaScript. Você **não edita**
`crates/frontend`, `crates/elements`, `crates/types`, `crates/emit_js`,
`crates/lsp`, `crates/diferencial`, `corpus/js`, `docs/EMISSAO-DDC.md`. Se
precisar de algo dessas fases (uma consulta que o `types` não expõe, um
campo no `Program`), escreva o pedido em `docs/NATIVO.md` §"Pendências
para outras fases" com o motivo e siga sem — nunca duplique a análise. No
`crates/cli/src/main.rs` acrescente **uma linha** de despacho para
`crates/cli/src/nativo.rs`, que é seu. Você é dono de `crates/llvm`,
`crates/native`, `crates/runtime`, `crates/jit`, `crates/cranelift-jit`,
`crates/abi`, do novo `crates/emit_native` e de `corpus/nativo/`.

## Contexto — leia antes de escrever código

1. `PLANO.md`: "Meta governante" (a trilha nova e o que já está pronto),
   "Incremento 28 — perfis de execução" (decisões: uma HIR própria, o
   backend consome a HIR e não assembly à mão, runtime nativo compartilhado,
   `DevJit`/`ReleaseAot` são perfis), "Regra de projeto — equivalência
   semântica" e "Meta de projeto — cadeia de ferramentas própria".
2. `docs/EMISSAO-DDC.md` — como o lado JS consome a trilha nova; você
   consome exatamente as mesmas estruturas: `Program` (`crates/elements/
   src/model.rs`), `OutlineTypes` (`crates/types/src/resolve.rs`),
   `BodyTypes` com `static_types`/`resolved` por `ExprId`
   (`crates/types/src/resolved.rs`), `TypeTable`/`CoreTypes`
   (`crates/types/src/table.rs`). `crates/emit_js/src/lib.rs::compilar` é
   o modelo de como dirigir o pipeline até as tabelas.
3. O backend nativo **antigo** (`crates/llvm`, `crates/native`,
   `crates/runtime`, `crates/hir`) compila o subconjunto antigo a partir
   da HIR antiga: LLVM IR textual → Clang → link com o runtime Rust (que
   já tem heap com GC por tracing, `heap.rs`, protocolo de raízes em
   `runtime_main.rs`, strings, closures, listas). Reaproveite o **runtime**
   e a **infraestrutura de emissão de IR e link** (`crates/llvm`,
   `crates/native`); não reaproveite a HIR antiga nem o `semantic` antigo.
   `docs/ABI-FFI-WASM.md` e `docs/IMPLEMENTACAO-*.md` documentam o que existe.
4. O oráculo de semântica é a VM: `dart run --enable-asserts x.dart`
   (Dart 3.6.2 no PATH). Para o nativo os `int` são de 64 bits com
   estouro modular — a semântica da VM, não a da web.
5. O `dart:core` da VM: `C:/tools/dartsdk-3.6.2/lib/libraries.json` seção
   `vm`, patches em `lib/_internal/vm/lib/*_patch.dart` e `vm_shared`. Os
   membros `external` ali têm `@pragma("vm:external-name", "Nome")` e são
   implementados em C++ na VM (`references/dart-sdk/runtime/lib/*.cc`).
   `SdkLayout::load(lib, "vm")` carrega essa seção (o `elements` já faz
   isso para "dartdevc"; a seção "vm" é a mesma API).

## Arquitetura — decisões fechadas

* **Entrada única**: `Program + OutlineTypes + BodyTypes + TypeTable`.
  Nenhuma reanálise, nenhuma tabela de `dart:core` escrita à mão.
* **HIR própria em `crates/emit_native/src/hir.rs`** (a "IR média" do
  PLANO): toda chamada resolvida como estática, de interface (por
  seletor) ou dinâmica; casts implícitos explícitos; açúcar removido
  (cascatas, null-aware, `??=`, `for-in` sobre iterador, padrões
  desugarados em testes e leituras, `async` em máquina de estados sobre o
  runtime). É esta HIR que o LLVM consome, e que um JIT (Cranelift) pode
  consumir depois — o PLANO proíbe backend consumindo IR de outro backend.
* **Modelo de objetos** (documentar em `docs/NATIVO.md` antes de emitir
  a primeira classe): cabeçalho com id de classe + ponteiro de metadados;
  campos por índice; **despacho por seletor** (tabela global de seletores
  → índice, vtable por classe, `noSuchMethod` na entrada faltante);
  interfaces por consulta na hierarquia instanciada do `types`; genéricos
  **reificados** (argumentos de tipo no objeto para `is`/`as`, como o
  runtime de tipos do dart2js: descritores de tipo canônicos no runtime);
  `int` i64, `double` f64, `bool` i1, `String` UTF-16 no runtime
  (`DartStr` já é WTF-8/UTF-16 no front-end), `null` tagged; closures com
  ambiente capturado por célula; exceções por unwinding do LLVM
  (`invoke`/`landingpad` com personalidade própria) ou, se medir mais
  simples e correto, `setjmp`/`longjmp` no runtime — decida, meça e
  documente; `StackTrace` por lista de frames capturada no `throw`.
* **`dart:core` a partir da fonte do SDK (seção `vm`)**, compilado pela
  mesma pipeline; cada `external` vira uma chamada a uma função Rust do
  runtime registrada pelo nome do `vm:external-name`. Nativo não
  implementado **não** é erro de compilação: gera um stub que lança
  `UnimplementedError("nativo X")` em execução, para que a cobertura
  cresça medida pelo corpus e não por lista.
* **Event loop e `async`** no runtime (fila de microtarefas, timers,
  `Future`/`Stream` do próprio SDK compilado; só os nativos de
  `dart:async` e `Timer` em Rust). `dart:io` mínimo: `File`, `Directory`,
  `stdout`, `Platform`, `Process` básico — é o que os testes do alvo usam.

## Alvos, na ordem, e o critério de aceite de cada um

1. **Corpus JS como corpus nativo**: `cargo run -p dartforge-diferencial`
   já roda os 202 programas de `corpus/js/` contra `dart run`. Crie
   `crates/emit_native/tests/diferencial.rs` (ou um subcomando `nativo`
   no seu próprio harness em `crates/emit_native/src/bin/`; não edite
   `crates/diferencial`) que compila cada programa com `dartforge
   compile-native x.dart -o x.exe`, executa e compara stdout + código com
   `dart run --enable-asserts` byte a byte (os 5 marcados
   `// diverge-ddc:` **batem** no nativo — é semântica da VM; para o
   nativo não há divergência declarada). Meta desta fase: **202/202**.
   Ordem que o corpus impõe: strings/aritmética → controle/funções →
   classes → coleções → exceções → async → dynamic/records/padrões →
   extensions/bibliotecas → SDK (`convert`, `collection`, `typed_data`,
   `DateTime`, `RegExp`, `Uri`, `math`). Reporte N/202 a cada commit.
2. **Os 7 testes do `new_sali/core` que dependem de `dart:io`**
   (`C:/MyDartProjects/new_sali/core/test/`: `ferias_real_delta_pdf_test`,
   `dart2js_flags_regression`, `pdf_generation_regression`,
   `quill_pdf_conversion`, `quill_table_better_pdf`,
   `sali_page_setup_pdf`, `sali_pdf_font_families`; `package_config` em
   `core/.dart_tool/`; `package:test` roda os testes em processo). Eles
   leem fontes por `File(...)` e geram PDF com `pdf_plus`. Aceite: mesma
   saída de `dart run --enable-asserts`.
3. **`new_sali/backend`** (angel3, `dart:io` servidor, `dart:isolate`,
   `dart:ffi`): compilar `bin/` e responder a uma requisição HTTP local
   igual à VM. É a meta; chegue a ela pelos dois anteriores.

## Medição obrigatória

Tempo por fase (`--timings`: front-end, HIR, LLVM IR, Clang, link) e pico
de memória (`crates/instrument`, modelo em
`crates/frontend/examples/memoria.rs`) no maior programa que compilar; o
custo do Clang/link é o que o PLANO já mediu como dominante — cache de
objetos por hash de módulo (runtime e SDK compilados uma vez) é parte do
trabalho, não opcional. Registre os números em `docs/NATIVO.md`.

## Disciplina

O backend nativo do CLI está atrás do feature `nativo`
(`crates/cli/src/nativo.rs`; `cargo build -p dartforge-cli --features
nativo`) — o build padrão do CLI não puxa `llvm-sys`, para que o caminho
JavaScript não pague o LLVM. Seu `compile-native` entra nesse módulo, sob o
mesmo feature. `cargo build -p dartforge-emit-native -p dartforge-cli
--features nativo` e `cargo test -p dartforge-emit-native` verdes; o `clang` está em `D:/LLVM/22.1.8/bin`
(perfil Windows x64 já validado em `crates/abi`). Commit por bloco
verificado com o N/202 na mensagem. Na resposta final: N/202, quais dos 7
testes do core rodam, tempos, e o hash.

Nota de eficiência (proprietário): quando o passo seguinte é executar o binário, use `cargo build` direto — `cargo check` seguido de `build` tipa duas vezes e não reaproveita nada. `check` só quando for corrigir erros sem executar. Faça `git merge main` para receber `.cargo/config.toml` com `target/` compartilhado (D:/Projects/dartforge/target): a máquina tem 8 GB e os builds das worktrees se enfileiram em vez de recompilar tudo.

**Princípio 10 (proprietário, 2026-09-22):** referência antes de código — localize a regra na referência (DDC/dartdevc, CONTRATO-DDC.md, especificação/CFE, runtime/lib/*.cc, analyzer), leia o código próprio envolvido inteiro, corrija todas as ocorrências de uma vez e compile uma vez para confirmar. Tentativa-e-erro compilando/executando para descobrir o que fazer é proibido.
