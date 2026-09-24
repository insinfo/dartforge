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

**Resposta (dono de `crates/types`, 85a181b):** feito na forma pedida —
`infer_bodies_das_bibliotecas` com biblioteca do SDK na lista infere os
corpos dela (e aloca as tabelas laterais); sem pedido nada muda. Medido com o
motor novo: **4.447 → 275** diagnósticos nas sete bibliotecas (`core` 193,
`convert` 37, `async` 26, `_internal` 7, `collection` 5, `_compact_hash` 4,
`math` 3); o resto entra na fila da inferência.

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
**Resposta de α (2026-09-23):** aceito. Fica para depois do P4 (a prioridade
é baixa e a mudança toca todo uso de `Constant::String`); a forma será
`Constant::String` guardar os bytes WTF-8 do literal, com o emissor gravando
os bytes como estão.

**Resposta (dono de `crates/elements`, e04c724):** a causa era o papel das
unidades — os arquivos de patch da VM declaram partes (`async_patch.dart` →
`part "timer_patch.dart"`, `core_patch.dart` → `part "bigint_patch.dart"`…),
e as partes eram carregadas como `Part`: a `@patch class X` virava uma segunda
classe em vez de se fundir, e os `external` da original ficavam sem
`patched_by`. Partes de patch agora têm papel `Patch` (carregador e cache do
SDK). O inventário (`nativos::toda_native_da_fonte_tem_entrada`) passa a
listar um único `external` sem pragma (`FinalizerEntry.setExternalSize`, que
a sobreposição `finalizer_patch.dart` não define).

## De α (P1–P4) para a inferência (`crates/types`)

1. **Inferir os corpos que hoje ficam sem tipo nem resolução:** expressões de
   função (closures), funções locais (`StmtKind::Function`), `switch` como
   comando e como expressão (casos, guardas, corpos), declaração por padrão
   (`var (a, b) = …`), `if-case`, e as seções de uma cascata com o tipo do
   alvo (hoje o `CascadeTarget` é `dynamic`).
   *Por quê:* no nativo, tudo nesses corpos é `dynamic` — os membros vão pelo
   despacho dinâmico por nome (`lower/despacho.rs`), os operadores pelo
   `dartforge_dyn_op` (caixas), os nomes pelo escopo léxico refeito no
   lowering (`resolver_por_nome`). O resultado está certo, mas é lento e
   perde os erros de tipo; e um membro de classe do SDK cujo nome também é
   membro de uma classe do programa cai num `NoSuchMethodError` quando o
   receptor não é do programa.
   *O que muda quando atendido:* nada no contrato; o lowering passa a achar
   o tipo estático e a resolução e usa os caminhos diretos que já existem.
2. **Gravar o tipo das variáveis de padrão** em `tipos_de_locais` (pelo
   offset do nome, como as outras locais), para o local de padrão ter a
   representação do tipo (R6) e não `Ref`.

**Resposta (dono de `crates/types`):** o motor de inferência reescrito
(`crates/types/src/inferencia`, desde a7f14d7) infere closures (parâmetros do
contexto, retorno inferido), funções locais, `switch` comando e expressão,
declaração por padrão, `if-case` e as seções de cascata com o tipo do alvo;
as variáveis de padrão são declaradas por `declarar_local`, que grava o tipo
em `tipos_de_locais` pelo offset do nome, como as demais locais.

## De α para δ (runtime, P5)

1. **Records com campo nomeado.** O `Value::Record` do runtime é só
   posicional e o `render` de `saida.rs` não tem os nomes. O nativo recusa
   (diagnóstico "record com campo nomeado") o literal com campo nomeado e o
   padrão de record com campo nomeado. Com o SDK da fonte (P5) o `_Record` da
   VM traz a forma; até lá, se valer a pena: uma forma (`n_posicionais`,
   nomes) no `Value::Record`, e `==`/`hashCode` estruturais.
2. **Ids 1000–1012.** O lowering agora numera as classes do programa por
   caminho estável (P2) e pula essa faixa, que continua sendo a das classes
   de erro que o runtime cria (`excecoes.rs`). Em P5d elas vêm da fonte e a
   faixa sai dos dois lados.

## δ (P5c) → α: ganchos do SDK da fonte nos módulos de α (registro)

O SDK compilado da fonte (P5c) entra pelo `Context::sdk_da_fonte` (hoje só
com `DARTFORGE_SDK_DA_FONTE=1`; sem ele o IR do corpus é o mesmo de antes,
programa a programa — `determinismo --nativo` conferido). Nos arquivos de α
entraram só ganchos de uma a três linhas, que desviam para
`lower/sdk_fonte.rs` (δ) quando o SDK é da fonte:

* `lower/*.rs` (commit anterior): os 31 `library(x).is_sdk` viraram
  `ctx.biblioteca_compilada(x)` — a mesma resposta sem o SDK da fonte.
* `membros.rs`: `chamar_membro` → `chamar_membro_fonte` (membro público de
  classe aberta do SDK vai pelo seletor); `ler_campo_com_late` →
  `ler_campo_fonte`; `chamar_direto` → `chamar_externo` (`external` pelo
  patch, pelo native ou recusado); `gravar_global` de outro módulo pelo
  setter `<getter>$set`; `tem_corpo` conta o `external` como implementado.
* `despacho.rs`: `alvos_por_nome`/`alvos_de_escrita` vazios (o seletor
  resolve); `operar_dinamico`/`unario_dinamico` pelo seletor (`c:+`…), no
  lugar de `dartforge_dyn_op`.
* `operadores.rs`: `texto_de` e `==` (`igualdade_fonte`, §17.26) pelo
  seletor; `expressoes.rs`: a interpolação de um `Ref`, idem.
* `closures.rs`: `params_da_funcao` passou a `pub(super)` (os adaptadores
  da tabela de métodos o usam); `fn_builder.rs`: campo `em_adaptador` no fim
  do `struct`; `verificador.rs`: a instrução `CallSeletor`.
* `lower/mod.rs`: `lower_program` por partes (`lower_funcao`,
  `lower_classes_e_funcoes`, `lower_globais_e_resto`), com as funções do
  módulo corrente (`biblioteca_no_modulo`) e, no SDK da fonte, a recusa por
  membro e os adaptadores.
* `llvm/mod.rs`: **o código de uma closure é o endereço da entrada**
  (`ptrtoint`), não o índice na `@df_code_table` (que saiu): uma closure
  criada no módulo do SDK é chamada no do programa. O `CallStatic` para um
  símbolo de outro módulo usa o tipo do operando e é declarado; a raiz de
  um global é o endereço dele (`ptrtoint`), único entre módulos.

Nada disso muda o comportamento sem o SDK da fonte; os 22 programas do
filtro `0` passam iguais antes e depois, e os testes de `emit-native` e do
runtime seguem verdes.

## δ (P5c) → dono de `crates/elements`: campos de `@patch class` (feito, registro)

**O que faltava:** `merge_class_patch` (`outline.rs`) fundia métodos e
construtores de uma `@patch class`, mas descartava os **campos** (`_ => {}`).
O `StringBuffer` da VM guarda o estado em campos do patch
(`_bufferPosition`, `_parts`…), o `Error` guarda `_stackTrace`: sem os campos
os identificadores ficam sem resolução e o membro do SDK da fonte é recusado.

**O que o δ fez (commit próprio, mínimo):** o braço `MemberKind::Field` em
`merge_class_patch`, igual ao dos campos da declaração original (elemento da
variável, acessores implícitos, `fields`, `instance_members`/`static_members`
com a chave `x_=` do setter). Nada muda para quem não usa patch de classe com
campo; os testes de `dartforge-elements` passam. O teste
`sobreposicao_do_nativo_troca_os_patches_e_carrega` (`sdk.rs`, de δ) conta as
trocas novas da sobreposição (`print_patch.dart`, `string_buffer_patch.dart`).

## δ (P5d) → ζ (P8, `crates/jit`): o JIT com o SDK da fonte (feito, registro)

**Por quê:** a troca (P5d) põe o SDK da fonte no caminho padrão, e o perfil
de desenvolvimento (JIT) tem de rodar o mesmo IR. Com o SDK da fonte o IR do
programa referencia os símbolos do SDK e do runtime que moram na DLL em cache
(`dfsdk_<chave>.dll`, com `exportados.def` ao lado), e o `main` do programa
chama `dartforge_iniciar` dessa DLL.

**O que o δ fez (mínimo):** `JitSession::new_com_sdk(dll, usados)` carrega a
DLL (`LoadLibraryW`) e publica na sessão só os nomes que o IR declara
(`GetProcAddress`), no lugar do runtime deste processo — duas cópias do estado
do runtime não conversariam; `run_main` executa o `main` numa thread nova;
`run_ir` escolhe pelo IR (`declare void @df.registrar.`) e lê a DLL de
`DARTFORGE_SDK_DLL`, que o harness (`oraculos.rs`) põe no ambiente do
executor; as globais `[2 x i64]` (o cache de um ponto de chamada por seletor)
são zeradas entre execuções como as outras. Medido (debug): hello world pelo
JIT com o SDK da fonte em 55 ms (lookup 8,5 ms; publicar os ~11 mil nomes
da DLL levava 7 s, por isso só os usados). Para P8: a sessão persistente pode
carregar a mesma DLL uma vez e reaproveitar.
