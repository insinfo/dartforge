# Brief — sistema de tipos em `crates/types` (passo 3 da meta governante)

Você está no repositório DartForge (`D:\Projects\dartforge`). Você acabou de
entregar `crates/elements` (outline, namespaces, patches, hierarquia por
nome). O passo seguinte da trilha é o **sistema de tipos**, e este brief cobre
a **primeira metade** dele: representação, tabela hash-consed, resolução de
todas as anotações de tipo do outline, hierarquia instanciada e subtipagem.
A segunda metade (inferência de corpos, promoção de fluxo, constantes) vem
num brief separado, porque depende desta e o critério de aceite é outro.

Leia antes de tocar em código, nesta ordem:

1. `PLANO.md`, seção **"Meta governante"** (trilha de 5 passos, o passo 3) e
   **"Meta de projeto — cadeia de ferramentas própria"** (memória).
2. `docs/FRONTEND-ARQUITETURA.md` §2 (memória) e §4 (tipos) — o contrato.
3. `crates/elements/src/model.rs` — o `Program` que você consome. Você é o
   dono dele; pode acrescentar campos que o `types` precise (ex.: `TypeId`
   resolvido por elemento), mas não pode reintroduzir ponteiros nem `String`
   de identificador.
4. `crates/frontend/src/ast.rs` — `TypeAnnotation`, `TypeKind`, `TypeParameter`,
   `Parameter`, `FunctionType`, `RecordType`: o que as anotações escritas
   carregam.
5. A especificação: `references/dart-language/specification/` — seções de
   tipos (subtyping, `FutureOr`, function types, records, nulabilidade). E
   as regras normativas de subtipagem em
   `references/dart-language/resources/type-system/subtyping.md` — **é a
   lista que o seu `is_subtype` implementa, regra por regra, na ordem**.
   Onde o CFE do SDK divergir da especificação, o SDK vence (é o oráculo).

## Regra de coexistência — o que você NÃO toca

`crates/frontend` está sendo editado por outro agente (parser: 426/426 do
SDK, restam ~16 arquivos do corpus pub). Não altere nada lá. Se a AST não
expõe algo de que você precisa, registre em `docs/FRONTEND-ARQUITETURA.md`
§7 com o motivo e siga sem. Também não toca em `crates/semantic`,
`crates/parser`, `crates/syntax` (trilha antiga).

Aviso de API que já mudou e você vai ver ao compilar: `StringLit::
constant_value()` devolve `DartStr` (WTF-8, unidades UTF-16) em vez de
`String`, porque `'\uD800'` é literal válido em Dart. `as_str()` dá `&str`
quando não há surrogate solto; `utf16_len()` é o `length` do Dart.

## O que entregar

Crate novo `crates/types` (`dartforge-types`), dependendo de `elements`,
`frontend`, `intern`, `diagnostics`.

### A. Representação e `TypeTable` hash-consed

`TypeId(u32)` índice numa `TypeTable` por sessão; **um** `TypeId` por tipo
estruturalmente igual (`InterfaceType(List, [int])` existe uma vez; a tabela
faz `intern(Type) -> TypeId` com `HashMap<Type, TypeId>`). Nulabilidade faz
parte da identidade (`int` ≠ `int?`).

```
Type::Dynamic | Void | Never | Null
Type::Interface { class: ClassId, args: Box<[TypeId]>, nullable }
Type::Function { type_params: Box<[TypeParamId]>, ret: TypeId,
                 positional: Box<[TypeId]>, optional: Box<[TypeId]>,
                 named: Box<[(SymbolId, TypeId, required: bool)]>, nullable }
Type::Record { positional: Box<[TypeId]>, named: Box<[(SymbolId, TypeId)]>, nullable }
Type::TypeParameter { param: TypeParamId, nullable }   // bound na tabela de params
Type::FutureOr { arg: TypeId, nullable }
Type::ExtensionType { decl: ClassId, args, nullable }  // erasure sob demanda
```

`TypeParamId` indexa uma tabela de parâmetros de tipo (dono: classe, função,
typedef ou tipo de função genérico), com `bound: TypeId` (`Object?` quando
ausente) e `variance` (só `covariant`/`contravariant`/`invariant` explícitos
se o corpus usar; senão registrar como pendência).

`Object`, `int`, `num`, `String`, `bool`, `Future`, `Iterable`, `List`,
`Null`, `Function`, `Record` são procurados uma vez em `dart:core`/
`dart:async` do `Program` e guardados na tabela (`CoreTypes`).

### B. Resolução de anotações do outline

Para cada elemento do `Program` — classe, mixin, enum, extension, extension
type, typedef, função/método/getter/setter/construtor, variável/campo —
resolver toda `TypeAnnotation` escrita para `TypeId`: parâmetros (com
posicionais opcionais e nomeados `required`), retorno, tipo de campo,
parâmetros de tipo e bounds, supertipos com argumentos (`extends
Iterable<E>`), `typedef` (alias expandido no ponto de uso, com substituição
dos parâmetros), tipos de função escritos (`int Function(String)`), records.

Onde a anotação está ausente o outline não infere: retorno sem anotação é
`dynamic`, campo sem anotação fica marcado `inferred: None` para a segunda
metade (a inferência de campo depende do inicializador). Exceção: getter/
setter e parâmetro de método que sobrescreve herdam o tipo do supertipo
(regra de *override inference* da especificação) — implemente, porque o SDK
usa em todo lugar.

Nomes resolvem pelo escopo da biblioteca declarante via `Program::lookup`/
`lookup_prefixed`; parâmetros de tipo em escopo vencem nomes de biblioteca.
Nome que não resolve, ou resolve para algo que não é tipo, é diagnóstico
**acumulado** (não pare no primeiro).

Resultado: tabelas laterais indexadas por id de elemento (`Vec<TypeId>` por
`FunctionElementId`, etc.) — nunca campos mutáveis na AST.

### C. Hierarquia instanciada

Para cada classe, o conjunto de supertipos **instanciados** (`class Foo
extends Bar<int>` → `Bar<int>`, `Baz<List<int>>` se `Bar<T> implements
Baz<List<T>>`), fechado transitivamente, com aplicações de mixin `S with M`
e `on`. `supertype_of(ty: TypeId, target: ClassId) -> Option<TypeId>`:
`List<int>` visto como `Iterable` é `Iterable<int>`. `Object` no topo;
`Null` e `Never` conforme a especificação.

### D. Subtipagem e operações

`is_subtype(a, b) -> bool` seguindo `subtyping.md` regra por regra (topo
`Object?`/`dynamic`/`void`, `Never` no fundo, `Null`, nulabilidade,
`FutureOr` dos dois lados, tipos de função com contravariância em parâmetros
e nomeados por nome, records por forma, parâmetros de tipo pelo bound,
interfaces por hierarquia instanciada com covariância por posição).
`substitute(ty, params, args)`, `lub`/`glb` (a versão da especificação;
"standard upper bound" com a regra dos "pontos mais altos" para interfaces),
`nullable(ty)`/`non_nullable(ty)`, `erase_extension_type(ty)`,
`normalize(ty)` (`FutureOr<Never>` = `Future<Never>`, `Null?` = `Null`, etc.).

## Critério de aceite — testes que precisam existir e passar

`crates/types/tests/sdk.rs`, `#[ignore]` como em `crates/elements/tests/sdk.rs`
(mesmo `DARTFORGE_SDK_LIB`):

1. `outline_do_sdk_resolve`: para as 36 bibliotecas do SDK, toda anotação de
   tipo de todo elemento resolve sem diagnóstico. Imprima as contagens:
   elementos, anotações resolvidas, `TypeId` únicos na tabela (o número
   único é a prova do hash-consing: tem de ser muito menor que o de
   anotações).
2. `hierarquia_do_sdk_instancia`: para toda classe do SDK, todos os
   supertipos instanciados; afirmar casos conhecidos —
   `supertype_of(List<int>, Iterable) == Iterable<int>`,
   `supertype_of(Map<String,int>, Object) == Object`,
   `supertype_of(int, Comparable) == Comparable<num>`,
   `supertype_of(String, Pattern)` existe, `Never <: tudo`.
3. `subtipagem_conhecida`: uma tabela de pares (a, b, esperado) com pelo
   menos 60 casos cobrindo cada regra de `subtyping.md`, inclusive os que
   costumam sair errado: `int? <: int` falso, `Null <: int?` verdadeiro,
   `FutureOr<int> <: Object` verdadeiro, `Future<int> <: FutureOr<int>`,
   `int <: FutureOr<int>`, `FutureOr<int?> <: FutureOr<int>` falso,
   `void Function(int) <: void Function(num)` falso e o inverso verdadeiro,
   `({required int a}) -> void` contra `({int? a})`, `(int, {String b})`
   records, `T extends num` vs `num`, `List<Never> <: List<int>`,
   `dynamic <: Object?` e `Object? <: dynamic` ambos verdadeiros.
4. `sem_sdk`: testes unitários com fontes em `tempdir` para typedefs
   genéricos com substituição, override inference de getter/setter e
   parâmetro, mixin application com supertipo instanciado, extension type
   com erasure, diagnóstico acumulado de dois nomes de tipo inexistentes.

Enquanto os testes 1–2 rodam sobre o SDK, o `assert` separa "não resolvido
por falta de nome no `elements`" de "não resolvido pelo `types`" — só o
segundo é seu; reporte os dois números.

## Disciplina que o projeto exige

* Documentação Rust em **português**, com contrato e exemplo (`sdk.rs` e
  `CONTRIBUTING.md` são o modelo).
* `Vec` indexado por id; `Box<[T]>` dentro de `Type` é aceitável (é o
  payload do nó, não um grafo); nada de `Rc`/`RefCell`; identificadores por
  `SymbolId`.
* A `TypeTable` sobrevive à sessão inteira e **cresce**: nasce com contador
  de entradas e bytes (`payload_bytes`, como o `Interner`), e o teste 1
  imprime o número — é o que o teste de platô do LSP vai vigiar depois.
* Medir: `cargo test -p dartforge-types -- --ignored --nocapture` com tempo
  e contagens colados na resposta final; `cargo fmt`, `cargo clippy -p
  dartforge-types --all-targets` limpos.
* Não faça commit; o proprietário revisa. Atualize só a linha `types` da
  tabela §6 de `docs/FRONTEND-ARQUITETURA.md` com o estado medido.
