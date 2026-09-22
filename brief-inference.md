# Brief — inferência de corpos em `crates/types` (passo 3, segunda metade)

Você está no repositório DartForge (`D:\Projects\dartforge`). A primeira
metade do sistema de tipos está entregue e verificada (`TypeTable`
hash-consed — 25.179 anotações do SDK em 10.396 `TypeId`, 1,65 MB —,
hierarquia instanciada, subtipagem normativa 83/83, resolução do outline).
Esta segunda metade tipa os **corpos**: toda expressão do SDK, do corpus e
do projeto de referência recebe um tipo estático, todo nome dentro de um
corpo resolve para uma declaração, e o resultado sai em tabelas laterais
por unidade — o que a fase `lowering` vai consumir.

Leia antes de tocar em código:

1. `PLANO.md` — "Meta governante" (passo 3), "Meta de projeto — cadeia de
   ferramentas própria" e, no fim do item 10, o **alvo de referência
   `C:/MyDartProjects/new_sali`** com os números do front-end: cada fase
   nova é medida nele com a mesma disciplina (memória vivos/pico, tempo).
2. `docs/FRONTEND-ARQUITETURA.md` §2 e §4.
3. O que você mesmo escreveu em `crates/types/src/` — `resolve.rs` é o
   ponto de partida: as tabelas laterais dos corpos seguem o mesmo padrão
   de `OutlineTypes`.
4. `crates/frontend/src/ast.rs` — `ExprKind` inteiro, `StmtKind`,
   `PatternKind`, `Function`, `Parameter`, `CollectionElement` (inclusive
   `NullAwareExpression`/`MapEntry` com `null_aware_*`), `StringLit`
   (`DartStr`, WTF-8).
5. Especificação: `references/dart-language/specification/` — inferência
   de tipos (seção "Type inference"), promoção
   (`resources/type-system/flow-analysis.md`), inferência de argumentos de
   tipo (`resources/type-system/inference.md`), `subtyping.md` que você já
   usa. O CFE em `references/dart-sdk/pkg/front_end/lib/src/type_inference/`
   é a implementação de referência quando a especificação for omissa.

## Regra de coexistência — o que você NÃO toca

`crates/frontend` continua com outro agente (próximo trabalho lá: reduzir o
tamanho dos nós da AST — `Expr` 120 bytes, `Member` 248 — sem mudar a API
pública dos `ExprKind`/`StmtKind`, então o que você consome não muda de
forma; se mudar, você é avisado). `crates/semantic`, `crates/parser`,
`crates/syntax` são a trilha antiga. `crates/elements` e `crates/types`
são seus.

## O que entregar

### A. Escopos e resolução de nomes em corpos

Um resolvedor de escopos léxicos por corpo de função: parâmetros, variáveis
locais (com a regra de que uma local só é visível após a declaração, e que
usar antes é erro), funções locais, parâmetros de tipo da função e da
classe, `this`/`super`, membros da classe (inclusive herdados e de mixins),
escopo da biblioteca (`Program::lookup`), prefixos de import, e os membros
estáticos por `Classe.nome`. Toda referência de identificador no corpo vira
uma entrada na tabela lateral `resolved: Vec<Option<Resolved>>` indexada
por `ExprId`:

```
Resolved::Local(LocalId) | Parameter(..) | TypeParameter(TypeParamId)
Resolved::Element(Element)             // topo, classe, função, variável
Resolved::Member { class: ClassId, member: FunctionElementId | VariableId, via_super: bool }
Resolved::Prefix(LibraryId) | Resolved::Dynamic /* receptor dynamic */
```

Acessos `a.b` resolvem pelo **tipo estático de `a`** (interface, extension
type, `dynamic`, tipo de função com `call`), com busca de membro na
hierarquia instanciada e em **extensions** aplicáveis no escopo (regra de
especificidade da especificação quando há mais de uma). `?.`, cascatas,
`super.x`, tearoffs, getters/setters implícitos de campos, `operator`s
(`a + b` é `a.operator+(b)`, `a[i]` é `operator[]`, `-a` é `unary-`).

### B. Inferência de expressões e statements

`static_type: Vec<TypeId>` por `ExprId`. Regras da especificação, na ordem
em que doem: literais (`int`/`double` por contexto — `double x = 1;` é
`1.0`), strings e interpolação, listas/sets/maps com argumento de tipo
inferido dos elementos **e** do contexto (`List<num> x = [1]` é
`List<num>`), records, condicionais com LUB, `??`, `is`/`as`, chamadas de
função e método com **inferência de argumentos de tipo** por restrições
(`inference.md`: coleta de restrições, resolução com bounds, contexto de
retorno), closures com tipo de retorno inferido do corpo (inclusive
`async`/`async*`/`sync*` — `Future<T>`, `Stream<T>`, `Iterable<T>` — e
com parâmetros sem anotação tipados pelo contexto), `await` desembrulhando
`FutureOr`, `throw` como `Never`, `switch` como expressão com padrões,
atribuições compostas por operador, `late`, `const`.

Declarações locais sem anotação inferem do inicializador; campos sem
anotação (marcados `inferred: None` na primeira metade) inferem aqui do
inicializador, e a dependência cíclica entre campos é erro, como no SDK.

### C. Promoção por fluxo

`flow-analysis.md`: promoção de locais (nunca de campos, exceto os
`final` privados sem sobrescrita — regra do Dart 3.2) por `is`, `!= null`,
`== null` com ramos, `&&`/`||`/`!`, `if`/`while`/`for`/`switch`/`try`,
atribuição definitiva (`late` e `final` sem inicializador), `Never`
interrompendo o fluxo (`throw`, `return`, chamada de função que devolve
`Never`), e a **despromoção** por atribuição e por captura em closure. O
resultado é observável: a tabela `static_type` de um `ExprId` de
identificador reflete o tipo promovido naquele ponto.

### D. Verificação de tipos e diagnósticos

Erros de atribuição, argumento, retorno, operador inexistente, membro
inexistente em tipo não-`dynamic`, argumento de tipo fora do bound, uso de
variável não definitivamente atribuída, `await` fora de `async`, `yield`
fora de gerador — com o **mesmo critério do `analyzer`**: o que o SDK aceita
não pode ser recusado, e os testes negativos dele reprovam aqui. Todos
acumulados, nunca parando no primeiro (o LSP precisa de todos).

### E. Avaliador de constantes

`const` de aritmética, strings (`DartStr`, concatenação por unidades
UTF-16), `bool`, `identical`, coleções const, construtores const com campos
`final`, `String.fromEnvironment`/`int.fromEnvironment`/`bool.
fromEnvironment` com ambiente vazio, e canonicalização (duas `const A(1)`
são a mesma constante). Necessário para `switch` clássico, anotações e
valores padrão de parâmetros.

## Critério de aceite

`crates/types/tests/bodies.rs`, `#[ignore]` sobre o SDK (mesmo
`DARTFORGE_SDK_LIB`):

1. `corpos_do_sdk_tipam`: para as 36 bibliotecas, toda expressão de todo
   corpo tem tipo estático e todo identificador resolve, sem diagnóstico.
   Imprima: expressões tipadas, identificadores resolvidos, closures com
   retorno inferido, chamadas genéricas com argumentos inferidos, `TypeId`
   únicos depois dos corpos (o hash-consing precisa continuar valendo:
   espere dezenas de milhares, não centenas de milhares), `payload_bytes`
   da tabela, tempo. Enquanto houver recusa, o `assert` separa por
   categoria de construção (a mensagem do diagnóstico serve de chave) e
   imprime as 20 mais frequentes com arquivo:linha — é assim que o
   trabalho se prioriza, como foi feito no parser.
2. `corpus_pub_tipa`: idem para `references/pub` (os pacotes têm
   `package_config`? Se não, o `elements` resolve `package:` pelo layout
   `references/pub/<nome>-<versão>/lib` — implemente esse fallback no
   `elements`, que é seu).
3. `new_sali_tipa`: `C:/MyDartProjects/new_sali/frontend` e `/backend`
   pelos `package_config.json` deles (se não existirem, rode `dart pub get`
   em cada um; o SDK 3.6.2 está no PATH). Imprima também **memória**: use
   o alocador contador como em `crates/frontend/examples/memoria.rs` num
   exemplo `crates/types/examples/memoria.rs` — vivos com tudo retido,
   pico, razão sobre a fonte, tempo. O front-end sozinho deu 132 MiB /
   254 ms para 8,5 MiB; a fase de tipos tem de declarar o seu acréscimo.
4. `promocao_e_inferencia`: unitários sem SDK cobrindo cada regra de C e
   os casos que costumam sair errado — promoção desfeita por atribuição
   dentro de closure, `late final` sem inicializador, `??=` promovendo,
   `switch` exaustivo em `sealed`, `List<num> x = [1]`, closure `async`
   inferindo `Future<int>`, inferência de `T` com bound `Comparable<T>`,
   `double x = 1`.
5. `negativos_do_analyzer`: pelo menos 40 programas inválidos (traduzidos
   de `references/dart-sdk/pkg/analyzer/test/src/diagnostics/` — cite o
   arquivo de origem em cada um) que precisam produzir o diagnóstico
   correspondente, com o código do erro do SDK como identificador
   (`argument_type_not_assignable`, `undefined_getter`, …).

## Disciplina

* Documentação Rust em português com contrato e exemplo.
* Tabelas laterais em `Vec` indexado por `ExprId`, nunca campos na AST;
  nada de `Rc`/`RefCell`; escopos como pilha de `Vec`, não mapas
  encadeados por ponteiro.
* A `TypeTable` cresce com os corpos: reporte antes/depois; se a tabela
  passar de ordem de grandeza, é sinal de tipo não canonicalizado
  (ex.: tipos de função com nomes de parâmetro na identidade).
* `cargo fmt`, `cargo clippy -p dartforge-types --all-targets` limpos;
  `cargo test -p dartforge-types -- --ignored --nocapture` com a saída
  colada na resposta final.
* **Commit ao fim de cada bloco verificado** (`git add` dos seus arquivos,
  mensagem em português, sem push) — instrução do proprietário para não
  perder trabalho. Atualize só a linha `types` da tabela §6 de
  `docs/FRONTEND-ARQUITETURA.md` com o estado medido.

Nota de eficiência (proprietário): quando o passo seguinte é executar o binário, use `cargo build` direto — `cargo check` seguido de `build` tipa duas vezes e não reaproveita nada. `check` só quando for corrigir erros sem executar. Faça `git merge main` para receber `.cargo/config.toml` com `target/` compartilhado (D:/Projects/dartforge/target): a máquina tem 8 GB e os builds das worktrees se enfileiram em vez de recompilar tudo.
