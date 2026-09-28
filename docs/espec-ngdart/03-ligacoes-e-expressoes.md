# 03 — Ligações e expressões do template

Especificação derivada da leitura do `ngcompiler-3.0.0-dev.3` (caminhos relativos a
`lib/v1/src/`) e do runtime `ngdart-8.0.0-dev.4`. Cobre: a gramática das expressões, a
conversão para Dart, as propriedades semânticas que escolhem a forma do código
(`isImmutable`, `canBeNull`, tipo estático), a interpolação, os alvos de ligação e o
*update statement* de cada um, os eventos e os pipes.

Abreviações usadas nas citações (`arquivo:linha`):

| Abreviação | Arquivo |
|---|---|
| `EP` | `compiler/expression_parser/parser.dart` |
| `AP` | `compiler/expression_parser/analyzer_parser.dart` |
| `AST` | `compiler/expression_parser/ast.dart` |
| `ATP` | `compiler/template_parser/ast_template_parser.dart` |
| `TP` | `compiler/template_parser.dart` |
| `BC` | `compiler/semantic_analysis/binding_converter.dart` |
| `EC` | `compiler/semantic_analysis/element_converter.dart` |
| `MDC` | `compiler/semantic_analysis/matched_directive_converter.dart` |
| `ME` | `compiler/optimize_ir/merge_events.dart` |
| `IR` | `compiler/ir/model.dart` |
| `AC` | `compiler/analyzed_class.dart` |
| `TO` | `compiler/template_optimize.dart` |
| `XC` | `compiler/view_compiler/expression_converter.dart` |
| `BVC` | `compiler/view_compiler/bound_value_converter.dart` |
| `USV` | `compiler/view_compiler/update_statement_visitor.dart` |
| `PB` | `compiler/view_compiler/property_binder.dart` |
| `EB` | `compiler/view_compiler/event_binder.dart` |
| `VNR` | `compiler/view_compiler/view_name_resolver.dart` |
| `PU` | `compiler/view_compiler/parse_utils.dart` |
| `CP` | `compiler/view_compiler/compile_pipe.dart` |
| `CV` | `compiler/view_compiler/compile_view.dart` |
| `VBd` | `compiler/view_compiler/view_builder.dart` |
| `VBi` | `compiler/view_compiler/view_binder.dart` |
| `VU` | `compiler/view_compiler/view_compiler_utils.dart` |
| `CMt` | `compiler/view_compiler/compile_method.dart` |
| `CT` | `compiler/view_compiler/constants.dart` |
| `DT` | `compiler/view_compiler/devtools.dart` |
| `ID` | `compiler/identifiers.dart` |
| `CH` | `compiler/chars.dart` |
| `OA` | `compiler/output/output_ast.dart` |
| `AE` | `compiler/output/abstract_emitter.dart` |
| `DE` | `compiler/output/dart_emitter.dart` |
| `FC` | `source_gen/template_compiler/find_components.dart` |
| `RT:x` | runtime `ngdart` (`lib/src/runtime/x.dart`) |

Porte Rust: `crates/gerador_ng/src/expr.rs` (`X`) e `crates/gerador_ng/src/visao.rs` (`V`),
citados por função. Legenda de cobertura: **✅ coberto**, **⚠️ parcial/divergente**,
**❌ falta** (o porte recusa a forma ou não a gera).

Os exemplos mostram a saída **depois** do `DartFormatter` que o builder roda (página
"infinita": nada quebra, só a vírgula final de argumento nomeado força quebra). O emissor
bruto escreve `(c? a: b)`, `(a?? b)`, `x/* REF:… */;`; o formatador normaliza para
`(c ? a : b)`, `(a ?? b)`, `x /* REF:… */;`. Os exemplos marcados **(oráculo)** estão
literalmente em `corpus/ngdart/oraculo/*.template.dart`; os outros são **(derivado)**, deduzidos
só do código. Aliases `importN` são ilustrativos.

---

## 1. Visão geral do caminho de uma expressão

1. O `ngast` entrega o texto cru de cada `[x]="…"`, `(e)="…"`, `{{…}}` e atributo com `{{ }}`.
2. `ATP` chama o `ExpressionParser` (`parseBinding`, `parseAction`, `parseInterpolation`),
   que embrulha o texto num arquivo Dart, parseia com o `package:analyzer` e converte o
   subconjunto aceito para o AST próprio (`AST`) — `AP:71-141`.
3. `_PipeValidator` confere nomes e aridade dos pipes (`ATP:1515-1597`).
4. `BC` converte cada nó de template em `ir.Binding(source, target)`; eventos passam por
   `rewriteTearOff` e são classificados simples/complexos (`BC:308-344`); eventos do mesmo
   nome são fundidos (`ME:7-24`).
5. `VBi` percorre a visão e chama `bindRenderText`, `bindRenderInputs`, `bindRenderOutputs`,
   `bindDirectiveInputs`, `bindDirectiveHostProps`, `bindDirectiveOutputs` (`VBi:65-130`).
6. `PB`/`EB` convertem a fonte com `BoundValueConverter` → `convertCdExpressionToIr`
   (`XC:38-53`) e o alvo com `bindingToUpdateStatements` (`USV:15-38`).
7. O emissor (`AE`/`DE`) escreve o texto; o `DartFormatter` normaliza espaços.

---

## 2. Gramática das expressões (parse)

### G1 — O parser é o do Dart, filtrado
`parseExpression` embrulha a entrada em `void __EXPRESSION__() => $input;`, parseia com
`parseString` (feature `non-nullable`), exige zero diagnósticos e exatamente uma declaração,
e visita a expressão do corpo com `_AngularSubsetVisitor` (`AP:72-121`). Todo nó do analyzer
que não tem `visitX` sobrescrito cai em `visitNode`/`visitExpression` e é **rejeitado**
(`AP:256-271`): `ParseException("<Tipo>: Not a subset of supported Dart expressions.")`.

Consequência: listas/mapas/sets literais, `is`/`as`, `this`, `new`/`const`, funções
anônimas, `throw`, cascata (`AP:465-467`), strings com interpolação Dart (`'a$b'` é
`StringInterpolation`) e strings adjacentes (`'a' 'b'`) são erros.

- Rust: `X:analisar` usa `var _e = <expr>;` com o parser do dartforge e recusa diagnóstico;
  `X:Conversor::expr_do_no` recusa lista/mapa, função, `is`/`as` e o resto por padrão. **✅**

### G2 — Entrada vazia
`input.isEmpty` → `EmptyExpr` (`AP:79-81`). Em `parseAction`, `input == null` é erro
"Blank expressions are not allowed in event bindings." (`EP:43-49`), mas `""` passa como
`EmptyExpr`. Em interpolação, `{{ }}` só com espaços é erro
"Blank expressions are not allowed in interpolated strings" (`EP:147-156`).
- Rust: `""` gera `var _e = ;` → erro de parse → recusa. **❌** (ver C3 para a saída.)

### G3 — Interpolação proibida onde se espera expressão
`parseBinding` e `parseAction` rejeitam texto com `{{…}}` ("Got interpolation ({{}}) where
expression was expected", `EP:161-170`). A regex de corte é `{{([\s\S]*?)}}` (`EP:9`),
aplicada com `jsSplit` (partes pares = texto, ímpares = expressão; `EP:135-159`).
- Rust: `V:partes_da_interpolacao` usa a mesma regra não gulosa (`find("}}")`). Diferença:
  `{{` sem `}}` é texto para o oficial (a regex não casa) e `None` (recusa) para o Rust. **⚠️**

### G4 — Identificador simples
`visitSimpleIdentifier`: primeiro `_matchExport(nome)`; se não for export,
`PropertyRead(ImplicitReceiver(), nome)` (`AP:435-455`).
`_matchExport` (`AP:222-251`): nome não prefixado de `exports:` → `StaticRead(id)`; com
segundo nome → `PropertyRead(StaticRead(id), nome2)`; prefixo de biblioteca → `StaticRead`
do export `prefixo.nome`.
**A classe do próprio componente é um export implícito** com `analyzedClass`
(`FC:846-850`); exports que são classes recebem `analyzedClass`, os outros não
(`FC:870-903`).

Importante: o export é resolvido **no parse**, antes de locais de template e membros do
componente — um nome de `exports:` sombreia um `let-x`/`#ref` de mesmo nome.

- Rust: `X:expr_do_no` (ramo `Identifier`) resolve `$event` → **locais** → exports →
  métodos → membros. Ordem divergente (local vence export). **⚠️**
- Export implícito da própria classe (`MeuComp.estatico` no template) e exports com prefixo
  (`V`, montagem de `Exportados`, filtra `!n.contains('.')`) não existem no porte. **❌**

### G5 — `a.b` e `a?.b`
`PrefixedIdentifier` (`a.b`): export `a.b` ou `PropertyRead(PropertyRead(implícito,a), b)`
(`AP:442-451`). `PropertyAccess`: `SafePropertyRead` se `?.`, senão `PropertyRead`
(`AP:463-475`).
- Rust: ramo `Property` de `X:expr_do_no` escreve `alvo.nome`/`alvo?.nome`. **✅**

### G6 — Chamadas
`visitMethodInvocation` (`AP:290-330`):
- `alvo` identificador e `_matchExport(alvo, metodo)` casa → `FunctionCall(receptor_export, args)`.
- `alvo` qualquer → `MethodCall(alvo, nome, args)` ou `SafeMethodCall` se `?.` (`AP:410-425`).
- sem alvo: se `metodo` resolve para `StaticRead` (função de `exports:`) →
  `FunctionCall(StaticRead, args)`; senão `MethodCall(ImplicitReceiver(), nome, args)`.
- `FunctionExpressionInvocation` (`f()(x)`) → `FunctionCall(alvo, args)` **com pipes proibidos**
  nos argumentos (`AP:273-287`).
- Argumento de tipo (`f<int>()`) é erro "Generic type arguments not supported." (`AP:384-386`).
- Argumentos nomeados viram `NamedExpr` (`AP:407-409`).
- Rust: ramo `Call` de `X:expr_do_no`; recusa argumento de tipo. **✅**

### G7 — Pipes: só `$pipe.nome(entrada, args…)`
Não existe a sintaxe `x | pipe:arg` nesta versão: `a | b` é `BinaryExpression` com `BAR`,
fora da lista de G9, e é rejeitado. Pipe é a chamada de método cujo receptor é
`PropertyRead` de nome `$pipe` (`AP:397-404`) — **qualquer** receptor com esse nome, então
`x.$pipe.f(v)` também é pipe (o `x` é descartado). Regras de `_createPipeOrThrow`
(`AP:332-357`): proibido em ação ("Pipes are not allowed in this context"), sem argumento
nomeado, com ao menos um posicional. Resultado: `BindingPipe(exp: args[0], name, args[1..])`
(`AP:359-371`).
- Rust: `X:Conversor::pipe`, reconhecido só quando o alvo é o identificador `$pipe`;
  recusa em evento, nomeado e sem argumento; `a | b` é recusado com motivo "pipe". **✅**
  (`x.$pipe.f()` **⚠️**, forma patológica.)

### G8 — Literais
`true/false`, `double`, `int` (valor numérico — `0xFF` vira `255`), `null`, string simples
(`stringValue`, escapes já resolvidos) → `LiteralPrimitive(valor)` (`AP:485-508`).
- Rust: literal inteiro só decimal, double só na forma canônica (sem expoente). **⚠️**
  (hex/expoente recusados; o oficial escreve `'$valor'` do Dart.)

### G9 — Binários e `??`
Aceitos: `+ - * / == != && || % < <= > >=` → `Binary(lexema, e, d)`; `??` → `IfNull(e, d)`;
qualquer outro (`~/`, `&`, `|`, `^`, `<<`, `>>`, `>>>`) cai em `super` e é rejeitado
(`AP:510-539`).
- Rust: `X:operador_do_template` tem exatamente o mesmo conjunto. **✅**

### G10 — Prefixo e pós-fixo
`!x` → `PrefixNot(x)`; **`-x` → `Binary('-', LiteralPrimitive(0), x)`** (`AP:601-616`);
outro prefixo é erro. `x!` → `PostfixNotNull(x)`; outro pós-fixo (`x++`) é erro
(`AP:618-630`).
- Rust: `!` e `x!` **✅**; `-x` é recusado (`"`-x`"`). **❌**

### G11 — Ternário, índice, parênteses
`c ? a : b` → `Conditional` (`AP:592-599`). `a[i]` → `KeyedRead` — **o `?` de `a?[i]` é
ignorado** (`AP:477-483`). Parênteses são descartados (`AP:457-461`).
- Rust: ternário e índice **✅**; `a?[i]` recusado **⚠️**; parênteses descartados **✅**.

### G12 — Atribuição (só em evento)
`allowAssignments` só em `parseAction` (`AP:17-30`). Fora dela: "Assignment (x = y)
expressions are only valid in an event binding." (`AP:543-548`). Formas (`AP:549-590`):
- `x = v` → `PropertyWrite(ImplicitReceiver, x, v)`;
- `a.b = v` / `a.b.c = v` → `PropertyWrite(alvo, nome, v)`;
- `a?.b = v` → erro "Null-aware property assignment is not supported";
- `a[i] = v` → `KeyedWrite(a, i, v)`;
- o **operador não é conferido**: `x += 1` vira `PropertyWrite(x, 1)` (sai `x = 1`).
- Rust: `X:Conversor::atribuicao`: `x = v` e `a.b = v` **✅**; `a[i] = v` recusado **❌**;
  `+=` recusado de propósito (o oficial geraria código errado) **⚠️**.

### G13 — `$event`
Não é sintaxe: é `PropertyRead(implícito, '$event')`, resolvido na conversão por
`getLocal('$event')` → variável `$event` (`VNR:40-43`). Não há erro de parse fora de evento;
em ligação ele só não compila.
- Rust: `$event` fora de evento é recusado. **✅** (mais estrito.)

### G14 — Validação de pipes (`_PipeValidator`)
Para cada `BoundText`, propriedade de diretiva/elemento e evento, coleta
`nome → [nº de args à direita]` e reporta "The pipe 'x' could not be found." ou
"… was invoked with too many arguments: N expected, but M found." com
`N = transformType.paramTypes.length - 1` (`ATP:1535-1561`, `ATP:1598-1611`).
- Rust: `V:pipes_do_template` (pipe fora de `pipes:` e argumentos demais). **✅**

---

## 3. AST de template (nós e visita)

| Nó (`AST`) | Campos | Origem |
|---|---|---|
| `EmptyExpr` | — | texto vazio |
| `StaticRead` | `id: CompileIdentifierMetadata` | export |
| `VariableRead` | `name` | só interno (G/§8.4) |
| `ImplicitReceiver` | — | raiz de nome livre |
| `Conditional` | `condition, trueExp, falseExp` | `?:` |
| `IfNull` | `condition, nullExp` | `??` |
| `PropertyRead` / `SafePropertyRead` | `receiver, name` | `a.b` / `a?.b` |
| `KeyedRead` / `KeyedWrite` | `receiver, key[, value]` | `a[i]` / `a[i]=v` |
| `PropertyWrite` | `receiver, name, value` | `a.b = v` |
| `BindingPipe` | `exp, name, args` | `$pipe.n(…)` |
| `LiteralPrimitive` | `value` | literal |
| `Interpolation` | `strings, expressions` | `{{ }}` |
| `Binary` | `operator, left, right` | binário |
| `PrefixNot` / `PostfixNotNull` | `expression` | `!x` / `x!` |
| `MethodCall` / `SafeMethodCall` | `receiver, name, args, namedArgs` | `r.m()` / `r?.m()` |
| `FunctionCall` | `target, args, namedArgs` | `f()()`, função exportada |
| `NamedExpr` | `name, expression` | `n: v` |

`ASTWithSource` guarda `ast`, `source` (texto como escrito) e `location` (`AST:526-550`). O
`location` do template é `sourceSpan.sourceUrl.toString()` (`ATP:1168-1176`); de
`@HostListener`/host é `''` (`ATP:532`, `VBd:960`).

---

## 4. Propriedades semânticas (`analyzed_class.dart`)

`BoundExpression` delega: `isImmutable → isImmutable(ast, analyzedClass)`,
`isNullable → canBeNull(ast)`, `isString/isBool/isNumber/isDouble/isInt → getExpressionType`
(`IR:593-641`). `StringLiteral` e `BoundI18nMessage`: imutáveis, não nulos, `isString`
(`IR:512-570`). `EventHandler`: tudo `false` (`IR:644-665`).

### S1 — `isImmutable` (regra exata, `AC:115-152`)
```
imutável(e) =
  LiteralPrimitive | StaticRead | EmptyExpr        → true
  IfNull(c, n)                                     → imutável(c) && imutável(n)
  Binary(_, l, r)                                  → imutável(l) && imutável(r)
  Interpolation(_, es)                             → todos imutáveis
  PropertyRead(rec, nome):
     analyzedClass == null                         → false
     rec é ImplicitReceiver, ou StaticRead com id.analyzedClass != null:
        classe = (StaticRead ? id.analyzedClass : analyzedClass)
        campo = classe.lookUpGetter(nome)?.variable
        campo != null → !campo.isSynthetic && (campo.isFinal || campo.isConst)
        classe.lookUpMethod(nome) != null → true   // "methods are immutable"
     senão                                         → false
  qualquer outro (MethodCall, Conditional, PrefixNot, KeyedRead,
                  SafePropertyRead, BindingPipe, FunctionCall, …)  → false
```
Observações:
- getter explícito: a `variable` é sintética → **false**; campo `final`/`const` → true;
  `lookUpGetter` do analyzer 6.x sobe a hierarquia e acha estáticos também.
- `StaticRead` é **sempre** imutável: um export de variável de topo **mutável**, de getter
  de topo ou de classe é imutável para o oficial.
- `-x` (`0 - x`) herda de `x`; `-1` é imutável.
- Nome que é local de template: consulta-se o campo **do componente** de mesmo nome (a
  regra não conhece locais); sem campo → false.
- Rust: `X:expr_do_no` preenche `Convertida.imutavel` com a mesma regra para literal,
  binário, `??`, membro `final`/`const`, método como valor. Divergências **⚠️**:
  export `Variavel { imutavel }` e `Getter` usam a mutabilidade real (oficial: sempre
  imutável); `Classe.metodo` estático é mutável no Rust (oficial: método → imutável);
  local com o nome de um campo `final` do componente é mutável no Rust (oficial: imutável).

### S2 — `canBeNull` (`AC:210-221`)
```
podeSerNulo(e) =
  LiteralPrimitive (inclusive null) | EmptyExpr | Interpolation → false
  IfNull(c, n) → podeSerNulo(c) ? podeSerNulo(n) : false
  resto        → true
```
- Rust: `pode_ser_nulo` com o mesmo cálculo (`??`: `a && b`). **✅**
  (`atributo_interpolado` força `false`, correto para `Interpolation`.)

### S3 — Tipo estático (`_TypeResolver`, `AC:232-377`)
Receptor implícito = `classElement.thisType`; `_variables` = `analyzedClass.locals`.

| Nó | Tipo |
|---|---|
| `Binary` | `String` se `op == '+'` e os dois lados `== String` (igualdade de `DartType`: `String?` **não** conta); senão `dynamic` |
| `Conditional`, `EmptyExpr`, `FunctionCall`, `IfNull`, `KeyedRead/Write`, `NamedExpr`, `BindingPipe`, `PrefixNot`, `PostfixNotNull`, `PropertyWrite`, `VariableRead` | `dynamic` |
| `ImplicitReceiver` | `thisType` do componente |
| `Interpolation` | 1 expressão de tipo primitivo (`bool/double/int/num`) → esse tipo; senão `String` |
| `LiteralPrimitive` | `String` se o valor é `String`; senão `dynamic` (int/bool também!) |
| `MethodCall`, `SafeMethodCall` | retorno de `lookUpMethod2(nome)` no tipo do receptor (`InterfaceType`), senão `dynamic` |
| `PropertyRead` | se receptor é o implícito e `nome ∈ locals` → `locals[nome] ?? dynamic`; senão retorno de `lookUpGetter2(nome)` no tipo do receptor, senão `dynamic` |
| `SafePropertyRead` | retorno de `lookUpGetter2`, sem consultar locais |
| `StaticRead` | `id.analyzedClass?.thisType`, senão `dynamic` |

Os `locals` só têm tipo para locais de `*ngFor` (tipados por `_typeNgForLocals`) e `let-x`
com `dartType` (`TO:18-40`, `VBd:438-442`). `MethodCall` com receptor implícito procura o
**método do componente** mesmo quando o nome é um local chamado como função.

Predicados (`AC:52-80`): `isBool/isDouble/isInt/isNumber/isString` = `isDartCoreX` do tipo
(que vale também para a versão anulável: `int?` é `isDartCoreInt`). **`isNumber` é só
`num`**, não `int`/`double`.

- Rust: `Convertida.tipo` segue a tabela (literal não-string `dynamic`, `!x` `dynamic`,
  `String + String`, retorno de método, local chamado como função consulta o método). Usa
  o banco semântico para cadeias (`Resolucao::tipo_do_membro`). **✅** Tipo desconhecido é
  recusado (o oficial cairia em `dynamic`) — conservador.

### S4 — Estáticos e setters (`AC:154-163`)
`isStaticGetterOrMethod(nome)`: `classElement.getGetter(nome) ?? getMethod(nome)` **declarado
na própria classe** e `isStatic`. `isStaticSetter(nome)`: `getSetter(nome)?.isStatic`.
- Rust: `Membro.estatico` → `{classe}.{nome}`. **✅** (Método estático chamado — ramo `Call`
  usa `_ctx.` se o nome estiver em `metodos`; não verificado se `metodos` exclui estáticos. **⚠️**)

---

## 5. Conversão para Dart (`expression_converter.dart`)

Entrada: `convertCdExpressionToIr(nameResolver, implicitReceiver, ast, span, metadata,
boundType)` (`XC:38-53`). Numa visão, `implicitReceiver = _ctx` (`BVC:171-176`,
`CT:51`); num `DirectiveChangeDetector`, `this.instance` (`directive_compiler.dart:20`).
O receptor implícito do AST vira a sentinela `#implicit` (`XC:12`, `XC:145-146`), trocada
em cada uso.

### C1 — Leitura de nome (`visitPropertyRead`, `XC:274-285`)
receptor convertido; se for a sentinela: `getLocal(nome)` → `local_<nome>`; senão
`_getImplicitOrStaticReceiver(nome, isStaticGetterOrMethod)` → `importX.Comp` se estático,
senão `_ctx` (`XC:342-349`). Depois `.nome`.

| Template | Dart |
|---|---|
| `nome` | `_ctx.nome` |
| `item` (local de `*ngFor`) | `local_item` |
| `constanteEstatica` | `import1.MeuComp.constanteEstatica` |
| `a.b` | `_ctx.a.b` |
| `a?.b` | `_ctx.a?.b` |

- Rust: ramo `Identifier`/`Property`. **✅**

### C2 — Chamada de método (`visitMethodCall`, `XC:248-264`)
**Converte os argumentos antes do receptor** (ordem que decide a ordem dos `local_x` e a
numeração dos pipes). Receptor sentinela: `getLocal(nome)` → `local_f(args)`
(`InvokeFunctionExpr`); senão receptor implícito/estático e `.nome(args, nomeados)`.
`visitSafeMethodCall` (`XC:310-315`): receptor **antes** dos args, `?.nome(args)` e
**descarta os argumentos nomeados**. `visitFunctionCall` (`XC:130-136`): alvo, depois args.

| Template | Dart |
|---|---|
| `titulo()` | `_ctx.titulo()` |
| `a.m(x, n: 1)` | `_ctx.a.m(_ctx.x, n: 1,)` → formatado em várias linhas |
| `a?.m(x)` | `_ctx.a?.m(_ctx.x)` |
| `fnExportada(1)` | `import2.fnExportada(1)` |

- Rust: ramo `Call` (args antes do alvo, quebra com nomeado). **✅** `?.m()` com nomeado
  mantém o nomeado e ordena args antes do receptor. **⚠️**

### C3 — Vazio (`XC:116-120`)
`EmptyExpr` → `true` se `boundType` é `bool` (`o.boolType` ou `ExternalType` de nome
`bool`, `XC:352-359`), senão `''`. `boundType` só existe para `InputBinding` com tipo
(`BC:163-173`; `NgIf.ngIf` forçado a `bool`). Ex.: `<template [ngIf]="">` →
`this._NgIf_1_9.ngIf = true;` **(derivado)**; `[title]=""` → `setProperty(el, 'title', '')`.
- Rust: **❌** (G2).

### C4 — Operadores (`XC:91-114`, `XC:138-142`, `XC:266-272`)
`Binary` → `(l op r)`; `Conditional` → `(c ? t : f)`; `IfNull` → `(c ?? n)`;
`PrefixNot` → `(!x)`; `PostfixNotNull` → `(x!)` (ou `(x/*!*/)` sem null-safety,
`AE:442-447`). Cada nó põe os próprios parênteses (`AE:409-446`, `AE:464-474`).

| Template | Dart |
|---|---|
| `!ativo` | `(!_ctx.ativo)` **(oráculo)** |
| `ativo ? 'sim' : 'não'` | `(_ctx.ativo ? 'sim' : 'não')` **(oráculo)** |
| `talvez ?? 'x'` | `(_ctx.talvez ?? 'x')` **(oráculo)** |
| `talvez!` | `(_ctx.talvez!)` **(oráculo)** |
| `-x` | `(0 - _ctx.x)` **(derivado)** |
| `a + b * c` | `(_ctx.a + (_ctx.b * _ctx.c))` |

- Rust: **✅** exceto `-x` **❌**.

### C5 — Índice e escrita (`XC:231-242`, `XC:287-302`)
`KeyedRead` → `r[k]`; `KeyedWrite` → `r[k] = v`; `PropertyWrite` com receptor sentinela:
local → erro "Cannot assign to a reference or variable \"x\""; senão receptor implícito ou
estático (`isStaticSetter`). Escrita no começo da instrução sai sem parênteses; dentro de
expressão, entre parênteses (`lineWasEmpty`, `AE:198-283`).
- `(click)="valor = $event.target.value"` → `_ctx.valor = $event.target.value;` **(oráculo)**
- `(click)="f(x = 1)"` → `_ctx.f((_ctx.x = 1));` **(derivado)**
- Rust: `X:Conversor::atribuicao` e ramo `Assign` (parênteses dentro). **✅**; `KeyedWrite` **❌**.

### C6 — Estático e export (`XC:317-319`)
`StaticRead` → `importExpr(id)` → `importN.Nome`. Ex.: `exports: [Cores]`,
`{{Cores.vermelho}}` → `importN.Cores.vermelho`.
- Rust: `Exportado::*` com `tardio_q`. **✅** (ver G4 para prefixo/implícito.)

### C7 — Literais (`XC:244-246`, `AE:392-404`, `AE:612-630`)
String → aspas simples, escapando `'`, `\`, `\n`, `\r` e **`$` → `\$`** (o `DartEmitter`
liga `escapeDollar`, `DE:115`). Outros valores: `'$valor'` do Dart (`null`, `true`, `1.5`).
- Rust: `V:literal`. **✅**

### C8 — `getLocal` e declarações (`VNR:40-71`)
`$event` → variável `$event`. Senão procura `locals[nome]` na visão e nos ancestrais
(`declarationElement.view`), lê pela cadeia de `parentView` (`getPropertyInView`), aplica
`unsafeCast<T>` se o tipo do local não é `null`/`dynamic`, e cacheia
`final local_<nome> = <expr>;` no estado compartilhado da visão; cada escopo lembra quais
locais usou (`_localsInScope`, ordem de primeiro uso) e `getLocalDeclarations` devolve as
declarações nessa ordem. Ex. **(oráculo)**:
`final local_item = import7.unsafeCast<String>(this.locals['\$implicit']);`
- Rust: `Convertida.locais` + `V:declaracao_do_local`; local de visão ancestral lido em
  visão aninhada tem recusas parciais. **✅/⚠️**

---

## 6. Interpolação (`visitInterpolation`, `XC:168-229`)

### I1 — Onde nasce
- Texto: o `ngast` separa cada `{{e}}` num `InterpolationAst`; `ATP:641-658` parseia
  `'{{' + valor + '}}'`, então **todo `BoundText` tem `strings == ['', '']` e uma expressão**.
- Atributo `x="a {{b}} c"`: `_createPropertyForAttribute` (`ATP:348-378`) parseia o valor
  inteiro e cria uma ligação de propriedade `x` com fonte `Interpolation`; essas ligações
  vão **depois** das `[x]` do elemento (`ATP:330-346`).

### I2 — Família e aridade
`expressionsAreString = todas isString` → `interpolateString{0,1,2}`, senão
`interpolate{0,1,2}` (`ID:92-124`, `RT:interpolate`).
- **1 expressão** (`XC:178-207`):
  1. se `_isPrimitiveCheck(e)` = `!isImmutable(e) && (isBool||isNumber||isDouble||isInt)`
     (`XC:361-368`): devolve **a expressão crua, descartando os textos**;
  2. senão, textos (comprimidos, I3) ambos vazios: `LiteralPrimitive` → `o.literal('${valor}')`
     (`null` → `''`); outra → `interpolate[String]0(e)`;
  3. senão `interpolate[String]1('antes', e, 'depois')`.
- **2 expressões**: `interpolate[String]2(t0, e0, t1, e1, t2)`.
- **3+ expressões**: `interpolateN([t0, e0, t1, …, tn])` — sempre `interpolateN`, mesmo
  se tudo for `String` (`XC:219-227`).
- Textos intermediários só passam por `replaceNgSpace`; o primeiro por
  `_compressWhitespacePreceding`, o último por `_compressWhitespaceFollowing` (`XC:210-218`).

### I3 — `_compressWhitespace*` (`XC:148-166`, `CH:1-5`)
```
preceding(v)  = se preserveWhitespace || v contém ' ' || v contém '' || v não contém '\n':
                   replaceNgSpace(v)
                senão replaceNgSpace(v.replaceAll('\n','').trimLeft())
following(v)  = idem com trimRight()
replaceNgSpace(v) = v.replaceAll('', ' ')
```
- Rust: `V:comprimir_antes`/`comprimir_depois` sem o teste de `preserveWhitespace` do
  componente. **⚠️** (e `trim_start` do Rust não apara U+FEFF, que o `trimLeft` do Dart apara.)

### I4 — Exemplos
| Entrada | Dart |
|---|---|
| `{{mensagem}}` (`String`) | `import9.interpolateString0(_ctx.mensagem)` **(oráculo)** |
| `{{quantidade}}` (`int` mutável) | `_ctx.quantidade` (vai para `updateTextWithPrimitive`) **(oráculo)** |
| `{{ativo ? 'sim' : 'não'}}` | `import9.interpolate0((_ctx.ativo ? 'sim' : 'não'))` **(oráculo)** |
| `title="n={{n}}"` (`int`) | checagem `_ctx.n`; ação `import8.interpolate1('n=', currVal_4, '')` **(oráculo)** |
| `x="{{a}}-{{n}}"` | `import8.interpolate2('', _ctx.a, '-', _ctx.n, '')` **(oráculo)** |
| `x="{{a}}{{b}}{{c}}"` | `importN.interpolateN(['', _ctx.a, '', _ctx.b, '', _ctx.c, ''])` **(derivado)** |
| `{{'fixo'}}` | `'fixo'` (texto imutável, `appendText`) |

- Rust: `V:interpolacao` (texto) e `V:valor_interpolado` (atributo, diretiva). 0/1/2
  expressões **✅**; 3+ recusado **❌**; atributo com entidade HTML (`&`) recusado **⚠️**.

---

## 7. Alvos de ligação (`BindingTarget`) — `binding_converter.dart`

### T1 — Tabela de conversão
| Origem | Alvo IR | Regra |
|---|---|---|
| `TextAst` | `TextBinding` + `StringLiteral` | `BC:82-83` |
| `BoundTextAst` (`{{ }}`) | `TextBinding` + `BoundExpression` | `BC:91-99` |
| `[className]`, `[class]` (mapeado para `className`) | `ClassBinding()` | `BC:127-130`, `TP:52-53` |
| `[attr.class]` | `ClassBinding()` | `BC:133-135` |
| `[x]` (propriedade) | `PropertyBinding(nomeMapeado, securityContext)` | `BC:131`, `TP:52-71` |
| `[attr.x]`, `[attr.x.if]`, `[attr.ns:x]` | `AttributeBinding(x, namespace, isConditional: unit=='if')` | `BC:132-139`, `TP:73-99` |
| `[class.x]` | `ClassBinding(name: x)` | `BC:140-141` |
| `[style.x]`, `[style.x.u]` | `StyleBinding(x, u)` | `BC:142-143`, `TP:104-108` |
| entrada de diretiva | `InputBinding(memberName, templateName, tipo)`, `isDirect` só `NgIf.ngIf` | `BC:150-184` |
| `(e)` | `NativeEvent(e)` se `isNativeHtmlEvent(e)`, senão `CustomEvent(e)` | `BC:223-230` |
| saída de diretiva | `DirectiveOutput(memberName)` | `BC:201-208` |
| atributo de host (`@HostBinding` literal / `host:`) | `_attributeName`: `@ns:x`, sufixo `.if`, `class` → `ClassBinding`, `tabindex`/`tabIndex` → `TabIndexBinding`, resto `AttributeBinding` (`securityContext.none`) | `BC:53-59`, `BC:275-306` |
| `@HostListener` | `NativeEvent`/`CustomEvent` com handler | `BC:65-77` |

Namespace de atributo passa por `namespaceUris` (`xlink`, `svg`, `xhtml`; outro vira
`null`, `IR:381-399`, `VU:21-25`). `attr.x.u` com `u != 'if'` é erro "Invalid attribute
unit"; `attr.onX` é erro de segurança (`TP:75-89`). Contexto de segurança:
propriedade/atributo pelo esquema; `class.x` `none`; `style.x` `style` (`TP:100-108`).

### T2 — Saneamento (`_sanitizedValue`, `USV:252-277`)
`none` → valor; `html` → `sanitizeHtml(v)`; `style` → `sanitizeStyle(v)`; `url` →
`sanitizeUrl(v)`; `resourceUrl` → `sanitizeResourceUrl(v)`. Aplicado ao `renderValue`
passado ao visitor. `StyleBinding` **ignora** o `renderValue` e usa `currValExpr`: estilo
por nome nunca é saneado (`USV:131-133`).
- Ex. **(oráculo)**: `import7.setProperty(this._el_1, 'href', import9.sanitizeUrl(currVal_1))`.
- Rust: `V:saneador` + `V:acao`. **✅**

---

## 8. *Update statement* de cada alvo (`update_statement_visitor.dart`)

`bindingToUpdateStatements(binding, appViewInstance, renderNode, isHtmlElement, currVal)`
(`USV:15-38`): `renderValue = _sanitizedValue(ctx, currVal)`; o visitor gera **uma**
instrução; se a fonte é `BoundExpression`, ela recebe `sourceReference`; antes vem o
`devToolsBindingStatement` (só `InputBinding`, `DT:17-30`).

### U1 — Comentário `/* REF:url:ini:fim */` (`AE:108-126`, `IR:572-590`)
Instrução-expressão com `sourceReference` sai `expr/* REF:<sourceUrl>:<ini>:<fim> */;`
(formatado: `expr /* REF:… */;`). `ini/fim = sourceSpan.start/end.offset + templateOffset`
(offset do template inline no `.dart`; 0 para `templateUrl`). Só para fonte
`BoundExpression` com `sourceSpan` e `sourceUrl`: eventos, literais, i18n e ligações de host
(sem span) não têm REF. O span é o do atributo inteiro (`[x]="…"`) ou do nó `{{ }}`.
- Rust: formatado à mão em `V:propriedade`, `V:atributo_interpolado`, `V:interpolacao`. **✅**

### U2 — `PropertyBinding` (`USV:121-129`)
`importN.setProperty(<nó>, '<nome>', <renderValue>)`.
**(oráculo)** `import7.setProperty(this._el_3, 'title', import8.interpolate1('n=', currVal_4, ''))`.
- Rust: `V:acao` (ramo sem prefixo); `innerHtml` → `innerHTML`; recusa nomes renomeados pelo
  esquema (`readonly`, `tabindex`, …) **⚠️** (o oficial usaria `getMappedPropName`).

### U3 — `AttributeBinding` (`USV:56-95`)
```
se isConditional: renderValue = (renderValue ? '' : null); usarSet = false
se namespace:     updateAttributeNS(nó, '<uri>', '<nome>', renderValue)
senão:            (usarSet && !fonte.isNullable) ? setAttribute(nó, '<nome>', v)
                                                 : updateAttribute(nó, '<nome>', v)
```
**(oráculo)** `import7.updateAttribute(this._el_0, 'aria-label', currVal_1)`; atributo
interpolado (não nulo) → `setAttribute`.
- Rust: `V:acao` (ramo `attr.`): `setAttribute`/`updateAttribute` por `pode_ser_nulo` **✅**;
  `.if` e namespace recusados **❌**.

### U4 — `ClassBinding` (`USV:97-119`)
- sem nome (`[class]`, `[className]`, `[attr.class]`):
  `<appViewInstance>.updateChildClass(nó, v)` (HTML) ou `updateChildClassNonHtml`;
  `appViewInstance` é `this`, ou a visão do componente quando o elemento é um componente
  (`PB:90-92`).
- com nome: `importN.updateClassBinding(nó, 'x', v)` / `updateClassBindingNonHtml`.

**(oráculo)** `import7.updateClassBinding(this._el_0, 'ativo', currVal_0)`.
- Rust: `V:acao`. **✅**

### U5 — `StyleBinding` (`USV:131-162`)
```
s = fonte.isString ? currVal : currVal.toString()
com unidade:  valor = ((currVal == null) ? null : (s + '<u>'))
sem unidade:  valor = fonte.isString ? currVal : currVal?.toString() (se isNullable) / currVal.toString()
<nó>.style.setProperty('<nome>', valor)
```
**(oráculo)**:
`this._el_0.style.setProperty('width', ((currVal_0 == null) ? null : (currVal_0.toString() + 'px')))`;
`this._el_0.style.setProperty('opacity', currVal_0?.toString())`.
- Rust: `V:acao` (ramo `style.`), com `isString` que aceita `String?`. **✅**
  (`style.a.b.c` recusado; o oficial ignora a 4ª parte **⚠️**.)

### U6 — `TabIndexBinding` (`USV:164-191`)
Só vem de atributo literal/host (`BC:291-294`): `renderValue` literal → `int.tryParse`, erro
"The \"tabindex\" attribute expects an integer value, but got: \"…\"" se falhar; sai
`<nó>.tabIndex = N;` **(oráculo)** `this._el_3.tabIndex = 2;`. Não literal: `<nó>.tabIndex = v;`.
`[tabIndex]="e"` é `PropertyBinding('tabIndex')` → `setProperty`.
- Rust: literal em `V` (atributos estáticos) **✅**; `[tabindex]` ligado **❌**.

### U7 — `TextBinding` (`USV:193-205`, `CV:199-209`)
Fonte `isBool||isNumber||isDouble||isInt` → `this._textBinding_N.updateTextWithPrimitive(v)`,
senão `this._textBinding_N.updateText(v)`. (Tipos da `Interpolation`, S3.)
- Rust: `V:interpolacao`. **✅**

### U8 — `InputBinding` (`USV:213-219`, `DT:17-44`)
```
if (importN.isDevToolsEnabled) {
  importN.Inspector.instance.recordInput(<instância>, '<templateName>', <valor>);
}
<instância>.<propertyName> = <valor>;
```
**(oráculo)** `import11.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', _ctx.mostrar);`
- Rust: `V:entradas_de` e caminho do `NgIf`. **✅**

### U9 — Eventos (`USV:221-249`)
- `NativeEvent`: `(<nó> ?? <appViewInstance>).addEventListener('<nome>', handler)`.
- `CustomEvent`: `importN.appViewUtils.eventManager.addEventListener(<nó ou appView>, '<nome>', handler)`.
- `DirectiveOutput`: `final subscription_K = <instância>.<saída>.listen(handler);`
  (`subscribeObservable` → `listen`, `DE:410-411`; `subscription_K` com
  `K = subscriptions.length`, `CV:1080-1085`).
- `HtmlBinding`: erro.

**(oráculo)**:
`import8.appViewUtils.eventManager.addEventListener(_el_0, 'keyup.enter', this.eventHandler0(_ctx.enviar));`
`final subscription_0 = this._D06FilhoSaida_0_5.salvo.listen(this.eventHandler1(_ctx.guardar));`
- Rust: `V:ouvinte`, `V:evento_nativo`, saídas de filho. **✅**

---

## 9. Checagem, numeração e primeira checagem (`property_binder.dart`)

### B1 — `bindAndWriteToRenderer` (`PB:25-70`)
Para cada ligação, em ordem: `isDirect` → `_directBinding` no método das constantes (se
`source.isImmutable`) ou dos dinâmicos; senão `_checkBinding`. No fim, as constantes vão
por `addStmtsIfFirstCheck` (reaproveita o `if (firstCheck)` se ele é a **última** instrução
do método, `CMt:23-36`) e as dinâmicas depois, na ordem.
- Rust: `V:escrever_ligacoes` + `V:na_primeira_checagem`. **✅**

### B2 — Numeração `_expr_N`/`currVal_N`
`createUniqueBindIndex()` = `bindingCount++` do estado **compartilhado da visão** (todos os
escopos), começando em 0 (`VNR:9-19`, `VNR:93`). Consome índice **toda** chamada de
`_checkBinding` — também a imutável, a de componente-hospedeiro e a que não gera campo.
Não consomem: ligação direta (`NgIf`), texto (`bindRenderText`), eventos. A ordem é a do
`bindView`: por elemento, entradas do elemento (`[x]` na ordem do fonte, depois atributos
interpolados), depois cada diretiva (entradas) (`VBi:89-114`). Cada visão embutida tem
contador próprio. Nomes: `ReadClassMemberExpr('_expr_$i')`, `variable('currVal_$i')`
(`PB:290-294`).
- Rust: `proxima_ligacao` em `V:propriedade`/`V:valor_interpolado`. **✅**

### B3 — Ligação mutável (`_bind`, `PB:301-348`)
```
final currVal_N = <checkExpression>;
if (importC.checkBinding(this._expr_N, currVal_N, '<source>', '<location>')) {
  <update statements, com currVal_N (ou a interpolação otimizada, B5)>
  [changed = true;]                        // só com calcChanged (B6)
  this._expr_N = currVal_N;
}
```
e o campo `Object? _expr_N;` (privado, `objectType.asNullable()`). `checkBinding`
(`ID:138-141`, `RT:check_binding`) recebe `ASTWithSource.source` e `.location` para
`BoundExpression`; para `StringLiteral`/`BoundI18nMessage` só os dois primeiros
(`PB:237-288`). `checkExpression == null` → nada (`PB:324-327`).
**(oráculo)**:
```dart
final currVal_0 = _ctx.escondido;
if (import8.checkBinding(this._expr_0, currVal_0, 'escondido', 'package:corpus_ngdart/src/a07_ligacao_de_propriedade.html')) {
  import7.setProperty(this._el_0, 'hidden', currVal_0) /* REF:package:corpus_ngdart/src/a07_ligacao_de_propriedade.html:5:25 */;
  this._expr_0 = currVal_0;
}
```
- Rust: `V:propriedade`, `V:atributo_interpolado`, `V:entradas_de`. **✅**

### B4 — Ligação imutável (`_bindLiteral`, `PB:357-384`)
Se `isImmutable`: nada para componente-hospedeiro; se a expressão convertida é o literal
`null` → **nenhuma instrução** (o índice já foi consumido); senão as ações com `currVal_N`
trocado pela expressão e `_expr_N` por `null` (`replaceVarInStatement`); se
`isNullable`, dentro de `if ((<expr> != null)) { … }`. Tudo no `if (firstCheck)`.
**(oráculo)**:
```dart
if (firstCheck) {
  if ((_ctx.rotulo != null)) {
    import8.setProperty(this._el_4, 'title', _ctx.rotulo) /* REF:…:84:100 */;
  }
}
```
Com literal: `[title]="'a'"` → `setProperty(el, 'title', 'a') /* REF */;` sem `if`.
- Rust: `V:propriedade` **✅**; literal `null` é recusado em vez de omitido **⚠️**.

### B5 — Interpolação primitiva otimizada (`PB:449-478`)
`_shouldInterpolateAfterCheck` = fonte é `Interpolation` && tipo primitivo (1 expressão
primitiva) && `!isImmutable`. Então `checkExpression` = a expressão crua (via
`_isPrimitiveCheck`, I2) e a ação usa `Interpolation(strings, [VariableRead('currVal_N')])`
convertida — `VariableRead` é `dynamic`, logo sempre a família `interpolate`
(`interpolate0(currVal_N)` ou `interpolate1('a', currVal_N, 'b')`).
- Rust: `V:valor_interpolado` (`checagem`/`na_acao`). **✅**

### B6 — Entradas de diretiva e `changed` (`PB:108-146`)
`calcChanged = (componente && onPush) || AfterChanges`. Se `calcChanged` e não hospedeiro:
`changed = false;` antes; `changed = true;` depois de cada update; componente `onPush`:
`if (changed) { this._compView_N.markAsCheckOnce(); }`. `NgIf.ngIf` é direta
(`isDirect`, `BC:175-184`): sem `checkBinding`.
- Rust: `V:entradas_de`. **✅**

### B7 — Host props de diretiva (`PB:389-435`)
Componente com `@HostBinding` → `this._compView_N.detectHostChanges(firstCheck);`; diretiva
→ `<instância>.detectHostChanges(<compView ou this>, <nó>);` (o corpo fica no
`XNgCd`, `BoundValueConverter.forDirective` com receptor `this.instance`).
- Rust: `V` (`detectHostChanges`) e `V:classe_ngcd`. **✅**

---

## 10. Nós de texto (`bindRenderText`, `createTextBinding`)

### X1 — Imutável
`bindRenderText` não faz nada (`PB:72-86`); no `build()` o valor é calculado uma vez com
`_textValue` (receptor `_ctx`): com pai → `final _text_N = importD.appendText(<pai>, <valor>);`;
sem pai → `createText(<valor>)` (`CV:606-671`). Nó `_text_N` (`CV:97-104`).
**(oráculo)** `final _text_6 = import8.appendText(_el_5, import9.interpolate0((_ctx.numero ?? 1)));`
(literal `1` e `_ctx.numero` final → imutável; `??` não é `String` → `interpolate0`).

### X2 — Mutável
Campo `final importT.TextBinding _textBinding_N = importT.TextBinding();` (`CV:114-118`),
`<pai>.append(this._textBinding_N.element)` no `build()`; no `detectChangesInternal`,
`_directBinding` **sem** `checkBinding` e sem índice:
`this._textBinding_N.updateText(<interp>) /* REF */;` ou `updateTextWithPrimitive(<expr>)`.
**(oráculo)** `this._textBinding_1.updateTextWithPrimitive(_ctx.quantidade) /* REF:…:5:19 */;`
- Rust: `V:interpolacao`. **✅**

---

## 11. Eventos

### E1 — Coleta e ordem
Eventos do elemento = eventos do template (na ordem do fonte) **seguidos** dos
`@HostListener` das diretivas não-componente que casam o elemento, na ordem das diretivas
(`ATP:386-396`, `ATP:1072-1096`). O que casa uma `@Output` de diretiva vira
`BoundDirectiveEventAst` e sai da lista. Dois eventos do template de mesmo nome no mesmo
elemento: erro "Found multiple events with the same name" (`ATP:1500-1512`).
`@HostListener('e', args)`: fonte `'metodo(args.join(', '))'`, com `$event` inferido quando
`args` vazio e o método tem exatamente 1 parâmetro; dois `@HostListener` do mesmo evento na
mesma classe: o último sobrescreve (`FC:559-568`).

### E2 — `rewriteTearOff` (`BC:330-344`, `AC:172-207`)
Se o AST do handler é `PropertyRead` (qualquer receptor): procura `nome` com
`thisType.lookUpMethod2` na classe do componente — ou da diretiva do `@HostListener`. Achou:
sem parâmetro posicional → `MethodCall(rec, nome, [])`; com → `MethodCall(rec, nome,
[PropertyRead(implícito, '$event')])`. Não achou: fica `PropertyRead` (instrução inútil
`_ctx.campo;`). Note que `(click)="filho.fechar"` usa o método `fechar` **do componente**
para decidir, mas mantém o receptor `filho`.
- Rust: `X:Conversor::acao` (aridades, `metodo_herdado`; campo cai no complexo). **✅**

### E3 — Classificação (`handlerTypeFromExpression`, `PU:19-45`)
Simples = `MethodCall` com receptor `ImplicitReceiver`, sem nomeados, e args `[]`
(`simpleNoArgs`) ou `[$event]` exato (`simpleOneArg`). Resto → `ComplexEventHandler`.

### E4 — Fusão (`mergeEvents`, `ME:7-24`; `IR:672-726`)
Por nome, na ordem da primeira ocorrência: `Simple.merge(h)` → `Complex([this, h])`;
`Complex.merge(h)` → acrescenta. Aplicado às saídas do elemento e às de cada diretiva
(`EC:21-27`, `MDC:~60-66`).

### E5 — Emissão do handler (`BVC:78-137`)
- **Simples**: converte a chamada; exige `InvokeMethodExpr` (local chamado como função dá
  `InvokeFunctionExpr` → erro "Expected method for event binding."); vira tear-off
  `ReadPropExpr(receptor, nome)` e `this.eventHandler<numArgs>(<tear-off>)`. O receptor é a
  instância da diretiva quando o handler vem de `@HostListener` (`directiveInstance`),
  senão `_ctx`; método estático do componente → `importN.Comp.m`.
- **Complexo**: cada `SimpleEventHandler` → `<expr>;` (achatando complexos aninhados), método
  `_handleEvent_K` da visão (`K = _eventHandlerCount++`, `CV:723-735`) e
  `this.eventHandler1(this._handleEvent_K)`. Corpo (`CV:737-750`): declarações `local_x`
  do escopo, depois `final _ctx = this.ctx;` se `_ctx` é lido (`VU:607-620`), depois as
  instruções. Parâmetro `$event` sem tipo.
- Cada saída tem escopo de nomes novo (`scopeNamespace`, `EB:8-35`).

**(oráculo)**:
```dart
_el_0.addEventListener('click', this.eventHandler0(_ctx.clicou));
_el_0.addEventListener('click', this.eventHandler1(this._handleEvent_0));

void _handleEvent_0($event) {
  final local_g = import7.unsafeCast<import1.Grupo>(…locals['\$implicit']);
  final local_item = import7.unsafeCast<String>(this.locals['\$implicit']);
  final _ctx = this.ctx;
  _ctx.escolher(local_g, local_item);
}
```
- Rust: `V:handler`/`V:handler_com` e `X:Acao`. **✅** Divergências **⚠️**: handler simples em
  método estático é recusado (oficial: `this.eventHandler0(importN.Comp.m)`); função de
  `exports:` como handler é recusada (oficial: complexo `importN.f();`).

### E6 — Onde os ouvintes saem
`addEventListener` acrescenta ao método `build()` no momento do `bindView` (`CV:1087-1095`),
ou seja, **depois** da criação de todos os nós, na ordem: por elemento, saídas do elemento,
depois saídas de cada diretiva. `@HostListener` do **componente** saem no `build()` da
visão do componente, com `rootEl` como nó (`VBd:934-956`).
- Rust: `V:ouvinte` acumula em `ouvintes`. **✅**

---

## 12. Pipes (`compile_pipe.dart`)

### P1 — Instâncias
Puro: uma instância **por nome**, na visão do componente (`compView.purePipes`), criada no
primeiro uso; impuro: uma instância **por chamada**, na visão da chamada (`CP:20-43`).
Nome do campo: `_pipe_<nome>_<pipeCount++ da visão dona>` (`CP:45-47`). Meta: o **último**
de `pipes:` com o nome (`CP:95-109`). Campo `late final <Tipo> _pipe_x_N;` e no `build()`:
`this._pipe_x_N = importP.Tipo(<deps>);` — `ChangeDetectorRef` vira `this`; outras deps
via injetor com `debugInjectorEnter/Leave` (`CV:1285-1317`).

### P2 — Chamada
Puro: um `_PurePipeProxy` por chamada, campo **da visão que chama**:
`_pipe_<nome>_<n>_<k>` com `k` = nº de proxies já criados para essa instância (em todas as
visões); a chamada é `this._pipe_x_n_k(entrada, args…)` (`CP:75-84`). No `create()`:
`this._pipe_x_n_k = importQ.pureProxy<argCount>(<instância vista daqui>.transform);`
com tipo `late final R Function(P0..P(argCount-1))` (`CP:49-73`, `CV:1319-1343`).
`argCount` = entrada + argumentos; `Identifiers.pureProxies` vai até 10, mas o runtime só
define `pureProxy1..6` (`RT:proxies`). Impuro: `<instância>.transform(entrada, args…)`.
**(oráculo)**:
`this._pipe_date_0_0 = import18.pureProxy2(this._pipe_date_0.transform);`
`final currVal_0 = import19.interpolate0(this._pipe_date_0_0(_ctx.quando, 'dd/MM/yyyy'));`
`this._pipe_async_0.transform(_ctx.fluxo)`.

### P3 — Ordem
`visitPipe` converte a entrada, depois os argumentos, depois chama `callPipe`
(`XC:122-128`): pipe dentro do argumento de outro ganha o proxy antes. A ordem global é a
da conversão no `bindView`.
- Rust: `V:PipesDoTemplate`, `V:pipes_do_template`, `V:trocar_pipes` (ordena as marcas pelo
  fim da chamada), `V:conferir_pipes`. **✅** Limite de 6 argumentos e tipos só do
  `dart:core`: recusa **⚠️**; deps além de `ChangeDetectorRef` (`p.fora`) **⚠️**.

---

## Lacunas do porte (priorizadas)

1. **`isImmutable` de `StaticRead`** (**alta**, muda a forma do código): export de variável
   mutável, getter de topo ou classe deve ser imutável (`AC:116-118`); o Rust usa a
   mutabilidade real (`X:expr_do_no`, `Exportado::Variavel/Getter`). Idem `Classe.metodo`
   estático (oficial: imutável). Hoje sai `checkBinding` onde o oficial escreve uma vez no
   `firstCheck`.
2. **Precedência de `exports:` sobre locais** (**alta** quando ocorre): o oficial resolve
   export no parse, antes de `let-x`/`#ref` (`AP:435-440`); o Rust consulta locais primeiro.
3. **Export implícito da própria classe e exports com prefixo** (**média**): `MeuComp.X` e
   `prefixo.Nome` no template (`FC:846-903`) são recusados.
4. **`-x` → `(0 - x)`** (**média**, fácil): `AP:607-609`, `XC:91-105`; imutável se `x` é.
5. **`interpolateN([...])` para 3+ expressões** (**média**): `XC:219-227`; atributo com
   3+ `{{ }}` é recusado.
6. **`EmptyExpr`** (**média**): `[x]=""` → `''`, entrada `bool` → `true` (`XC:116-120`),
   `(e)=""` → handler complexo com `'';`.
7. **`KeyedWrite` em evento** (`a[i] = v`, `XC:236-242`) (**média**).
8. **`preserveWhitespace` no `_compressWhitespace*`** (**média**): com o componente em
   `preserveWhitespace: true` os textos não são comprimidos (`XC:151`, `XC:161`).
9. **Atributo condicional `[attr.x.if]` e com namespace** (`updateAttributeNS`,
   `USV:61-82`) (**média**).
10. **Propriedades renomeadas pelo esquema** (`[tabindex]`, `[readonly]`, `for`… →
    `setProperty(el, 'tabIndex', …)`) (**média**).
11. **Ligação imutável `null`** (**baixa**): o oficial não emite nada (`PB:364-369`); o
    Rust recusa.
12. **Handler simples estático e função de `exports:` como handler** (**baixa**): E5.
13. **`SafeMethodCall`** (**baixa**): receptor antes dos args e nomeados descartados
    (`XC:310-315`).
14. **Literais não canônicos** (hex, expoente, int grande) e **`a?[i]`** (o `?` some,
    `AP:477-483`) (**baixa**).
15. **`x.$pipe.f(v)`**, **`{{` sem `}}`**, **U+FEFF no `trimLeft`**, **`style.a.b.c`** (**muito
    baixa**, formas patológicas).
