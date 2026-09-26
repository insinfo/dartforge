# Alinhamento da inferência do emit_js com a inferência comum

Estado de 2026-09-26. O emissor JS (`crates/emit_js`) recebe de
`dartforge_types::infer_program_bodies` o `BodyTypes` — tipo estático e
elemento resolvido de cada expressão, o mesmo que o backend nativo consome
(`get_type`/`get_resolved`, `crates/emit_native/src/context.rs`) — e, até
aqui, não o lia: reinferia tudo com um sistema próprio (`ty.rs`, `Ty`,
`emit_expr` devolvendo `(Js, Ty)`, `emit_args_infer`). O objetivo é uma fonte
única de inferência. Este documento é o inventário do passo 1 (medir onde as
duas inferências divergem e quem está certo) e registra o começo do passo 2.

## Resumo

| | |
|---|---|
| Programas do `corpus/js` | 196 (3 rejeitados de propósito por erro de linguagem, mas conferidos) |
| Expressões conferidas (`emit_expr`, fora da emissão especulativa) | 44.221 |
| Alvos de atribuição composta/`++` (não conferidos, ver §3) | 314 |
| Expressões com tipo divergente | **336 (0,76 %)** |
| Identificadores com alvo conferido | 18.449 |
| Identificadores com alvo divergente | **2** (`this` em extensão, representação) |

Veredito das 336 divergências de tipo contra o `package:analyzer` 3.6.2
(`tools/oraculo_tipos`):

| Quem acerta | Divergências |
|---|---:|
| Inferência comum | 312 |
| Os dois (nós distintos do analyzer no mesmo intervalo) | 12 |
| Nenhum (constante de enum genérico) | 4 |
| Sem nó no analyzer (alvo de cascata `?..`, `const (e)` em padrão) | 8 |
| Emissor | **0** |

Antes das correções do §5 o emissor acertava 4 casos que a inferência comum
errava; foram corrigidos na inferência comum. Hoje, **em todo ponto em que as
duas divergem no corpus, a inferência comum está certa ou empata** — o
caminho do passo 3 (o emissor passar a ler `BodyTypes`) conserta tipos, não os
estraga. Vários dos erros do emissor são observáveis em execução (§4).

## 1. A conferência

`crates/emit_js/src/conferencia.rs`, ligada por variável de ambiente:

    DARTFORGE_JS_CONFERIR_TIPOS=1                  # resumo por categoria em stderr
    DARTFORGE_JS_CONFERIR_TIPOS=/tmp/div.tsv       # e uma linha por divergência

- **Tipo.** Em `emit_expr`, antes das coerções de contexto (`coerce_to`), o
  `Ty` que o emissor deduziu é comparado com o tipo comum
  (`UnitBodyTypes::get_type`), convertido para `Ty` por um conversor sem
  efeitos colaterais (`ty_da_tabela`: o `Ctx::ty_of` registra limites de
  parâmetros de tipo que a emissão consulta depois, e a conferência não pode
  mudar o JS). Vale a primeira emissão real de cada nó; a emissão
  especulativa (`type_of`) não conta.
- **Alvo.** Em identificadores lidos como valor (`emit_identifier`) e
  chamados (`f(…)` em `emit_call`), o alvo da busca própria do emissor
  (`resolve_ident` → `IdentTarget`) é comparado com o `Resolved` comum, pela
  identidade da declaração.
- Com a conferência ligada ou desligada, a saída JS do corpus é idêntica byte
  a byte à de antes da mudança (conferido com `emitir_corpus`, abaixo).

Linha do `.tsv`: `arquivo:linha, nó, tipo|alvo, categoria, texto, emissor,
comum, caminho, início, comprimento` (intervalo em bytes). Cada emissão
acrescenta antes uma linha `#totais`.

### Como reproduzir

    # 1. emitir o corpus com a conferência (e guardar o JS para comparar)
    cargo build --release -p dartforge-emit-js --example emitir_corpus
    DARTFORGE_JS_CONFERIR_TIPOS=/tmp/div.tsv \
        target/release/examples/emitir_corpus /tmp/js corpus/js/*.dart

    # 2. oráculo do analyzer (num diretório com pubspec que dependa de
    #    `analyzer` e `path`, depois de `dart pub get`)
    ls -d $PWD/corpus/js/*.dart > /tmp/arquivos.txt
    DART_SDK=<sdk 3.6.2> dart --packages=<dir>/.dart_tool/package_config.json \
        tools/oraculo_tipos/oraculo.dart $PWD/corpus/js /tmp/arquivos.txt /tmp/oraculo.tsv

    # 3. juiz: quem acerta, por categoria
    node scripts/conferir-tipos-js.mjs /tmp/div.tsv /tmp/oraculo.tsv --exemplos 3

`emitir_corpus` grava um arquivo por programa (todos os módulos em ordem):
`diff -r` entre duas execuções mostra qualquer mudança na emissão. Com
`EMITIR_CORPUS_EXECUTAVEL=1` também escreve cada programa pronto para o Node
(`dart_sdk.js` de `DARTFORGE_DART_SDK_JS`, gerado por
`scripts/gerar-dart-sdk.ps1`).

## 2. Normalizações (representação, não semântica)

A comparação trata como iguais:

| Emissor | Comum | Por quê |
|---|---|---|
| `Iface { FutureOr, [T] }` | `Ty::FutureOr { T }` | o emissor usa as duas formas |
| `Iface { Null }`, `Never?` | `Null` | idem |
| `FutureOr<T?>?` | `FutureOr<T?>` | `?` redundante |
| `Param` com id próprio | `Param` com id da `TypeTable`, mesmo nome | o emissor cria ids novos para funções genéricas locais e tipos de função |
| parâmetros ligados de `Fn` | idem, ids diferentes | comparados por posição (alfa-equivalência) |
| `X` | `X & B` (`Intersection`) | em execução é `X` |
| campo (`Element::Variable`) | acessor implícito (`Function` com `variable`) | a identidade é a variável |
| `IdentTarget::Unknown` | sem resolução | `dynamic`/`Never` como valor, nome indefinido |

E conta à parte (sem conferir):

- **Alvos de atribuição composta e de `++`/`--`** (314). A inferência comum
  registra o tipo de leitura no nó da atribuição e não tipa o alvo — como o
  analyzer, que dá `readType`/`writeType` à `AssignmentExpression` e nenhum
  `staticType` ao identificador alvo. O emissor lê o alvo com `emit_expr`.
  Antes desta regra, 324 divergências "comum dynamic" eram só isso.
- Alvos de identificador sem identidade estável no emissor (membro achado por
  nome no `$this` de extensão, extensão aplicável a `this`).

## 3. Categorias de divergência

Contagem por forma da diferença (emissor × comum), com o veredito do
analyzer:

| Categoria | N | Veredito |
|---|---:|---|
| mesma classe, argumentos de tipo diferentes | 102 | comum 101, sem oráculo 1 |
| classes diferentes | 75 | comum 75 |
| só a nulabilidade de topo | 67 | comum 61, sem oráculo 6 |
| emissor `dynamic`, comum preciso | 37 | comum 33, nenhum 4 |
| tipos de função diferentes | 30 | comum 30 |
| tear-off genérico: comum já instanciado | 12 | os dois (nós distintos) |
| comum `dynamic`, emissor preciso | 11 | comum 11 |
| parâmetro de tipo × outro | 1 | comum 1 |
| espécies diferentes | 1 | sem oráculo 1 |

### Por causa

Agrupamento das 336 pela causa (heurística conferida à mão). `E` = erro do
emissor, `R` = representação, `X` = erro dos dois / fora do emissor.

| Causa | N | Exemplo (emissor → comum = analyzer) |
|---|---:|---|
| E1 tipo cru instanciado com `Object?` em vez de `dynamic` | 61 | `jsonDecode(s) as List`: `List<Object?>` → `List<dynamic>`; `<Comparable>[…]`, `Caixa c` cru |
| E5 promoção que o emissor não faz | 57 | `int? v = 3; v.triple()`: `int?` → `int` (promoção na inicialização); `m!` promove `m`; `(a, b) = (1, 's')` promove; `var s = …; s!` |
| E13 literal de coleção/closure sem o contexto certo | 47 | `[[1], [], [3]]`: `List<List<int>>` → `List<List<dynamic>>` (e o que se lê dele); `{(1, 2), (a: 1, b: 2)}`: `Set<Object>` → `Set<Record>`; `[Caixa(1), Caixa('x')]`: `Caixa<Object>` → `Caixa<int>`; `List.generate(n, (_) => …)`: parâmetro `dynamic` → `int` |
| E7 inferência descendente: `T` fixado pelo contexto | 31 | `print(id(5))`: `int` → `Object?`; `print(primeiro([7, 8]))`: `List<int>` → `List<Object?>`; `print(math.max(1, 2))`: `int` → `num` |
| E8 tipo de variável: `as`, `is`, padrão, `catch` | 28 | `on RangeError catch (e)`: `Object` → `RangeError`; `switch (e) { Num() => e }`: `Expr` → `Num`; `Ok(:final valor)`: `Object?` → `int` |
| E3 tipagem especial de `int` | 21 | `15.clamp(0, 10)`: `num` → `int`; `-7.remainder(2)`: `num` → `int` |
| E12 emissor sem tipo onde há tipo estático | 19 | `soma5(10)` (chamada de objeto com `call`); `dynamic a; if (a is int) a > 1` (promoção de `dynamic`); `(e as int) * e`; `e.invalidValue` em `on RangeError catch (e)` |
| E11 parâmetro de tipo vazado no resultado | 18 | `xs.expand((x) => [x, -x])`: `List<T>` → `List<int>`; `mapear<String>((x) => …)` dentro de `extension E<T>`: `x` `String` → `T` |
| E2 `await` de `Null`/`Never` | 13 | `await null`: `dynamic` → `Null` |
| R1 tear-off genérico | 12 | `ints.reduce(math.max)`: emissor `T Function<T extends num>(T, T)` (instancia depois, na coerção), comum `int Function(int, int)`; o analyzer tem os dois (identificador genérico + `FunctionReference` instanciado) |
| E9 closure sem `return` retorna `Null` | 8 | `Future(() { … })`: `Future<void>` → `Future<Null>` |
| R2 alvo de cascata `?..` | 7 | `nulo?..a()..b()`: alvo `Pedido?` no emissor, `Pedido` na comum (a seção só roda com não nulo); o analyzer não tem esse nó |
| X2 constante de enum genérico | 4 | `enum Valor<T> { inteiro<int>(7) … }`, `Valor.inteiro.get()`: emissor `dynamic`, comum `T` (vazado), analyzer `int` |
| E6 promoção mantida onde o Dart desfaz | 3 | `Box? s = null; for (…) { s = b; } s!.x`: `Box` → `Box?` (atribuição no laço) |
| E10 closure que só lança | 3 | `Future.microtask(() => throw …)`: `Future<dynamic>` → `Future<Never>` |
| E4 literal `int` em contexto `T extends num` | 2 | `math.max(1.5, 2)`: o `2` sai `double` → `int` |
| R3 fim de cadeia `?.` | 1 | `a?.b?.c.length`: o nó da propriedade é `int?` na comum (null-shorting registrado no nó), `int` no emissor (o `?` vem das guardas) |
| X1 `const (e)` em padrão vira record | 1 | `case const (base * 2)`: o parser gera `Record { const_: true }` com um posicional; o emissor o trata como parênteses (`int`), a comum como record `(int,)` |

### Alvos

Dos 18.449 identificadores, só 2 divergem: `this` dentro de extensão, que o
parser dá como `Identifier` e o emissor liga ao `$this` local, e a inferência
comum não resolve. Antes da normalização do acessor implícito (§2), 418
leituras de campo pareciam divergir (o emissor aponta o campo, a comum o
getter implícito); e 7 nomes sem declaração (`dynamic`, `Never`, nome
indefinido) eram "desconhecido × sem resolução" dos dois lados.

## 4. Efeito observável dos erros do emissor

Os tipos do emissor viram argumentos de tipo reificados (rti) no JS. Um
programa com os casos de E7, E13, E9 e E10 imprime, no Node, tipos diferentes
dos do `dart run`:

| Dart 3.6.2 (VM) | emit_js |
|---|---|
| `primeiro recebeu List<Object?>` | `primeiro recebeu List<int>` |
| `singleton Object?` (`first(singleton('x'))`) | `singleton String` |
| `[Caixa<int>, Caixa<String>]` | `[Caixa<Object>, Caixa<Object>]` |
| `m[0].runtimeType` de `[[1], [2, [3]]]`: `List<int>` | `List<Object>` |
| `_Map<String, String>` | `IdentityMap<String, Object>` (o nome da classe é do DDC; o argumento, não) |
| `Future.microtask(() => throw …)`: `Future<Never>` | `_Future<dynamic>` |
| `Future(() { … })`: `Future<Null>` | `_Future<void>` |
| `List.unmodifiable(_itens)` em `Pilha<int>`: `List<int>` | `List<dynamic>` |
| `{(1, 2), (a: 1, b: 2)}`: `_Set<Record>` | `LinkedSet<Object>` |
| `<Comparable>[3]`: `List<Comparable<dynamic>>` | `List<Comparable<Object?>>` |
| `print(id(5))`, `id<T>` imprime `T`: `Object?` | `int` |
| `print(maior(3, 9))`: `Comparable<Object>` | `int` |
| `tipo(15.clamp(0, 10))`: `int` | `num` |
| `Base? b = null; b = Child(); tipo(b)`: `Base` | `Child` |

Nenhum programa do `corpus/js` depende disso na saída (por isso o corpus
passa), mas qualquer `is`/`as`/`runtimeType` sobre esses valores diverge.

## 5. Correções feitas na inferência comum

Cada uma com teste em `crates/types/tests/inferencia_grupos.rs`, conferida
contra o analyzer:

1. **Escrutinado do `switch` sombreado pela variável do padrão**
   (`padroes::caso`). Em `switch (e) { Neg(:final e) => … }` o alvo da
   promoção era resolvido depois de o padrão declarar o novo `e`: a variável
   do padrão (tipo do campo, `Expr`) saía promovida para `Neg`. Agora o
   escrutinado é resolvido antes, no escopo de fora.
2. **`Never` como valor** (`expr::resolver_nome`). Não há declaração de
   `Never` no `dart:core`; caía em nome indefinido (`dynamic`). Agora é
   pseudotipo embutido como `dynamic`, de tipo `Type`.
3. **Coerção por `call` numa chamada genérica** (`chamadas::invocar`,
   `tipos::tipo_do_call_implicito`). `[1, 2, 3].map(somador)`, com
   `Somador.call(int) → int`, restringia `T` pelo tipo da interface e `T`
   saía `dynamic`; a coerção (*implicit call tearoff*) vem antes da restrição.
   Resolveu também três divergências conhecidas de `corpus/inferencia`
   (`mem11_call_generico`, catraca atualizada).

Os testes de `crates/types` e `crates/emit_js` passam; a saída JS do corpus
não muda (o emissor não lia `BodyTypes`).

## 6. Passo 2 começado: elemento de topo vem de `BodyTypes`

`FnEmitter::alvo_do_identificador` (`expr.rs`): para identificador lido
como valor ou chamado, se a resolução comum é `Resolved::Element` de um
elemento de topo (função/variável sem classe nem extensão, classe, typedef,
extensão), o emissor usa esse alvo; o resto (locais, membros, parâmetros de
tipo, prefixos, campo estático de extensão — que a comum também dá como
`Element`) continua pela busca própria, assim como nó sem resolução comum.
Cerca de metade das resoluções de identificador do corpus já vem da
inferência comum; a saída JS dos 196 programas é idêntica byte a byte.

Efeito semântico (teste diferencial `p9_escopo_lexico`): a busca própria
preferia um membro **herdado** a uma declaração de topo homônima; o Dart
segue o escopo léxico (membro herdado não está nele) e usa a de topo. O
emissor imprimia `herdado` onde a VM imprime `topo`.

## 7. Pendências

- **X2, constante de enum genérico** (`resolve.rs`, `VariableRef::EnumConstant`):
  o outline dá o tipo cru `Valor` (sem argumentos) a `inteiro<int>(7)`, e o
  acesso a membro vaza `T`. O certo é `Valor<int>` — explícito nos argumentos
  de tipo da constante ou inferido do construtor. Não corrigi porque muda o
  tipo que o backend nativo vê para essas constantes e não há como validá-lo
  aqui (o emissor JS também erra: `dynamic`).
- **X1, `const (e)` em padrão** (`parser/expressions.rs`,
  `parse_parenthesized_or_record` com `const_`): o parser perde se houve
  vírgula e gera record de um posicional; o emissor contorna tratando todo
  `const (x)` de um posicional como parênteses — o que erra `const (1,)`. O
  certo é o parser distinguir (`const (e)` é padrão constante com
  parênteses; `const (e,)` é record).
- **`this` em extensão** como `Identifier` sem resolução comum.
- **Cobertura**: só nós emitidos por `emit_expr` têm o tipo conferido.
  Receptores estáticos (`C.m`, `p.x`), identificadores chamados (só o alvo) e
  argumentos de tipo inferidos de chamadas genéricas que não aparecem no tipo
  de nenhum nó não são conferidos diretamente.

## 8. Recomendações

**Passo 2 (resolução).** A resolução comum e a do emissor concordam em
todos os identificadores do corpus. Seguir, na ordem de menor risco:

1. membros por `this` implícito (`Resolved::Member` → `IdentTarget::ThisMember`
   / `Static`; a substituição `Member::subst` ainda sai de `lookup_member`);
2. acessos com receptor (`a.b`, `a.m()`), onde a comum registra o membro no
   nó da propriedade/chamada — a conferência precisa ganhar um gancho em
   `emit_member_get`/`emit_method_call` antes;
3. extensões (`Resolved::ExtensionMember`) e construtores
   (`Resolved::Constructor`, hoje resolvidos à parte em `emit_instance_creation`).

A cada passo, a conferência (alvo da busca própria × comum) com o corpus
limpo é a condição para trocar; `emitir_corpus` + `diff -r` confere que a
saída não muda onde não deve.

**Passo 3 (tipos).** O inventário diz que trocar o `Ty` deduzido pelo tipo
comum corrige, não regride. Mas a troca muda o JS (rti de literais e de
chamadas genéricas, despacho direto × dinâmico) exatamente nos 336 pontos, e
isso tem de ser medido pelo diferencial (VM × Node), não só pelo texto.
Sugestões:

- começar pelos tipos que só decidem forma de emissão local e não viram rti:
  `await`, `catch`, promoções (E2, E5, E6, E8, E12) — o `Ty` do emissor pode
  ser substituído pelo comum lido em `emit_expr` para identificadores;
- depois os argumentos de tipo inferidos (E7, E11, E13), que mudam o rti e
  corrigem o §4 — isso pede ler também os argumentos de tipo instanciados de
  cada chamada/literal; hoje `BodyTypes` só guarda o tipo do nó, e a
  instanciação escolhida (`chamadas::invocar` devolve `inst`) não é
  registrada. Registrar a instanciação por nó de chamada é o pré-requisito
  do `emit_args_infer` sair;
- E1 (tipo cru → `Object?`) nasce em `resolve_type` do emissor
  (`body.rs`), que completa os argumentos que faltam com o limite declarado
  do parâmetro — `Object?` quando não há limite —, onde a instanciação para
  os limites do Dart dá `dynamic` (é o que `instanciar_para_limites` da
  inferência comum faz). Pode ser corrigido direto no emissor antes da
  troca, com o diferencial;
- manter a conferência ligada no CI do corpus (resumo em stderr) enquanto as
  duas inferências coexistirem, com catraca sobre o número de divergências.
