# Compilador Nativo (LLVM) — Trilha Nova

O que o backend nativo **faz hoje**, lido no código. O que falta e a ordem em
que entra estão em `docs/NATIVO-PLANO.md` (§6 o contrato R/E/N/G; §7 a rodada 2:
decisões, passos P0–P9 e o mapa de módulos com os donos de cada arquivo). Onde
este documento diz "ainda não", o construto vira diagnóstico de compilação.

---

## 1. Arquitetura

O compilador nativo consome os artefatos semânticos da trilha nova:

- `Program` (`crates/elements/src/model.rs`);
- `OutlineTypes` (`crates/types/src/resolve.rs`);
- `BodyTypes` (`crates/types/src/resolved.rs`);
- `TypeTable` e `CoreTypes` (`crates/types/src/table.rs`).

Etapas (`crates/emit_native/src/lib.rs`, `emitir_ir` e `compilar`):

1. **Carregamento e inferência.** O programa e o SDK (seção `vm` do
   `libraries.json`, com os patches) viram um `Program`; os corpos **do
   programa** são inferidos. Os corpos do SDK não são compilados: os membros do
   SDK que o programa usa são casados pelo nome no lowering (ver §3).
2. **Lowering para a HIR própria** (`crates/emit_native/src/hir.rs`,
   `crates/emit_native/src/lower/`). Chamada a função do usuário é direta; a
   método de instância do usuário é um `switch` sobre a classe dinâmica
   (`dartforge_value_class`) entre as implementações (`lower/membros.rs`). Não
   há vtable, seletor nem `CallInterface`/`CallDynamic` emitidos. `??`, `??=`,
   `?.`, `for-in` sobre lista, `try`/`catch`/`finally` e `assert` são baixados;
   **não** são: closures, cascatas, `switch`, padrões, `async`/`await`,
   geradores, extensões, `super.m()` (a lista completa e a contagem por
   programa saem do relatório do harness, §4).
3. **Construto não suportado** vira diagnóstico com posição (N1). Se o módulo
   tem algum, `emitir_ir` e `compilar` devolvem `Err` com **todos** os
   diagnósticos (a primeira linha é a do primeiro, sem posição, para agrupar),
   e nenhum IR é emitido.
4. **Emissão de LLVM IR** textual (`crates/emit_native/src/llvm/`), ponteiros
   opacos.
5. **Clang e ligação** (`crates/emit_native/src/driver.rs`), com cache de
   objeto por hash do IR (`cache_objeto.rs`) e o runtime Rust compilado uma vez
   por conteúdo (`cache.rs`).

---

## 2. Modelo de objetos (runtime, `crates/runtime`)

O runtime é Rust, com heap preciso por **handles** (`heap.rs`): um `Ref` do
código gerado é um `i64` que indexa a tabela de handles; `0` é null. O código
fonte do runtime são os fragmentos `nucleo`, `gc_raizes`, `excecoes`, `saida`,
`strings`, `colecoes` e `closures` de `crates/runtime/src/`, concatenados na
ordem de `FRAGMENTOS` (`crates/runtime/build.rs`) — o mesmo texto para o AOT
(`rustc` avulso) e para o JIT (módulo `dartforge_runtime::abi`).

- **Objeto de classe do usuário:** `Value::Object { class_id, fields }`, cada
  campo `(bits: i64, is_ref: bool)` — a marcação `is_ref` é o que o coletor
  segue (E1). O `class_id` hoje é a posição da classe na carga + 1, e as
  classes de erro do SDK têm ids fixos 1000–1012 (`lower/mod.rs`); ids estáveis
  por `DeclId` entram em P2.
- **`int`:** `i64` com estouro modular, como a VM. **`double`:** `f64`.
  **`bool`:** `i1` no IR, `u8` na fronteira com o runtime. Numa posição `Ref`
  (`Object?`, `dynamic`) os três viram **caixa** no heap (R3); as coleções
  guardam o escalar com a tag, nunca a caixa (R8).
- **`String`:** unidades de código UTF-16 na forma da VM (`Texto`,
  `heap.rs`; NATIVO-PLANO §7, decisão 5): `_OneByteString` (toda unidade ≤
  0xFF, um byte cada, Latin-1) ou `_TwoByteString` (dois bytes), escolhido
  pelo conteúdo e canônico — um pedaço Latin-1 de um texto de dois bytes
  volta a um byte. `length`, índices, `codeUnitAt`, `substring`, busca,
  `split`, `padLeft` e comparação são por unidade, em O(1) por acesso; um
  surrogate solto é uma unidade como outra (`runes` o devolve como ele
  mesmo). Só o `print` troca o surrogate solto por U+FFFD, como a VM
  (`Utf8::Encode`, `runtime/vm/unicode.cc`). As constantes chegam do IR em
  UTF-8 e o runtime aceita WTF-8 (`dartforge_string_new`); `toString` e
  interpolação montam o texto por unidades (`TextoMut`), sem passar por
  `String` do Rust. `StringBuffer` guarda as unidades.
- **Listas, mapas, conjuntos:** valores do runtime com slots etiquetados
  `(bits, tag)`; os membros são externs `dartforge_list_*`, `dartforge_map_*`,
  `dartforge_set_*`.
- **Closures:** o runtime tem `Value::Closure`/`Environment`/`Cell`, mas o
  lowering **ainda não** produz closures (P1).
- **Exceções:** modelo por valor — exceção pendente no runtime e verificação
  depois de cada chamada (NATIVO-PLANO §1), com `try`/`on T`/`catch (e, s)`/
  `finally` e `rethrow`.
- **Estado:** heap, nomes de classe e exceção pendente em `thread_local!` do
  runtime; os globais Dart são `@dfg_<id>` do módulo LLVM (passam para a
  tabela do isolado em P8).

---

## 3. Membros do SDK

O runtime escrito à mão cobre **só** os membros do SDK que
`crates/emit_native/src/lower/sdk_por_nome.rs` casa **pelo nome**, sem olhar o
tipo do receptor (`length`, `add`, `substring`, `join`, `Exception(…)`,
`StringBuffer()`…). Esse mecanismo está **congelado**: nenhum caso novo; ele é
apagado em P5, quando `dart:core`, `dart:async`, `dart:collection`,
`dart:convert`, `dart:math` e `dart:_internal` vêm da fonte do SDK 3.6.2 com
uma camada fina de patches nossos e natives em Rust (NATIVO-PLANO §7).

---

## 4. Medição

O placar é o do harness (`crates/diferencial`), no CI:
`pwsh scripts/ci.ps1 -Frente <frente> -Acompanhar` e
`pwsh scripts/ci.ps1 -Placar <id>`. O relatório nativo termina com duas
seções: as falhas agrupadas pela primeira linha do stderr, e **"construtos
(todos os diagnósticos)"** — cada construto não suportado com o número de
programas que o usam e a lista dos programas bloqueados **só** por ele.
