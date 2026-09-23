# Pedidos entre os agentes do nativo (rodada 2)

Arquivos com dono (docs/NATIVO-PLANO.md §7.3) não são editados por outro
agente: quem precisa de uma mudança num deles a registra aqui, com o porquê e a
forma exata, e o dono a aplica (ou responde aqui por que não).

## δ → dono de `crates/types`: inferir os corpos do SDK quando pedidos (P5a)

**Onde:** `crates/types/src/infer.rs`, `BodyInferrer::infer_functions`, as
duas primeiras verificações do laço:

```rust
if self.program.library(func_elem.library).is_sdk { continue; }
if self.apenas_bibliotecas.as_ref().is_some_and(|s| !s.contains(&func_elem.library.0)) { continue; }
```

**Por quê:** P5 compila `dart:core`, `dart:collection`, `dart:_internal`,
`dart:math` e `dart:_compact_hash` **da fonte**; para isso o lowering precisa
dos tipos e da resolução dos corpos do SDK (seção `vm` com os patches e a
sobreposição `sdk_nativo/`). Hoje nenhum corpo do SDK é inferido, nem quando
pedido por `infer_bodies_das_bibliotecas`: o `is_sdk` pula antes de olhar
`apenas_bibliotecas`. O aceite de P5a é **zero** diagnósticos de inferência
nesses corpos, e a medição dele depende disto.

**Forma pedida (a menor):** o `is_sdk` só pula quando a biblioteca não foi
pedida explicitamente:

```rust
let pedida = self.apenas_bibliotecas.as_ref().map(|s| s.contains(&func_elem.library.0));
if pedida == Some(false) { continue; }
if pedida.is_none() && self.program.library(func_elem.library).is_sdk { continue; }
```

Nada muda para quem não pede biblioteca do SDK (o JS, o LSP, o nativo de
hoje). O mesmo vale para `infer_variable_initializers` (o `is_sdk &&
declared_ty.is_some()` pula também as pedidas).

**Medição local com essa mudança (não enviada):** ver NATIVO-PLANO §7.4.

## δ → α: texto dos literais sem perda (`lower/expressoes.rs`)

**Onde:** `lower/expressoes.rs`, braço `ExprKind::String`, as duas linhas
`String::from_utf8_lossy(text.as_bytes()).to_string()` (o literal constante e
cada `StringPart::Text`).

**Por quê:** o front-end guarda o texto do literal em WTF-8
(`frontend/src/text.rs`): um surrogate solto (`'\uD83D'`) é guardado em três
bytes. O `from_utf8_lossy` troca esses três bytes por três U+FFFD, então
`'\uD83D'.length` dá 3 no nativo e 1 na VM. Desde a decisão 5 o runtime já
aceita WTF-8 em `dartforge_string_new` (o `Texto::de_wtf8` de
`runtime/src/heap.rs`), então o que falta é o texto chegar inteiro ao IR.

**Forma pedida:** `Constant::String` carregar os bytes WTF-8 do literal (por
exemplo `Constant::String(DartStr)` ou um `Vec<u8>`), e o emissor
(`llvm/mod.rs`, `emit_string_constants`) gravar esses bytes como estão — ele já
escapa byte a byte. Nenhuma mudança no runtime.

**Prioridade:** baixa — nenhum programa do corpus tem literal com surrogate
solto (o programa 04 cria os soltos com `String.fromCharCode`, que já está
certo). Morre sozinho se P5d baixar os literais pelo `_OneByteString`/
`_TwoByteString` da fonte.

## δ → dono de `crates/elements`: `patched_by` dos membros de classe patcheados (P5d)

**O que foi medido:** `nativos::inventario` (emit_native) acha **43** `external`
das bibliotecas da fonte com `patched_by == None` embora o patch exista —
membros de classe com `@patch`: `Object.==`, `Object.hashCode`,
`Timer._createTimer`, `_AsyncRun._scheduleImmediate`, `String.fromCharCodes`,
`double.parse`, `identical`, `Expando.[]`… (a lista sai de
`cargo test -p dartforge-emit-native nativos -- --nocapture`).

**Por quê importa:** em P5d o lowering baixa a chamada ao `external` pelo corpo
do patch (`patched_by`); sem a ligação, o membro parece não ter
implementação.

**Pedido:** preencher `patched_by` também para métodos, getters, setters,
operadores e fábricas de classes com `@patch`, como já é feito para as
funções de topo.
