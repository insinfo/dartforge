# Perfil de produção do backend JavaScript

O que o `dart2js` faz e nós ainda não fazíamos: pegar o programa inteiro,
provar o que dele é alcançável a partir de `main`, jogar fora o resto,
encurtar o que sobra e escrever **um arquivo**.

Este documento é o plano. Foi escrito **antes** do primeiro commit de
código, como manda `docs/BRIEF-JS-PRODUCAO.md`, e traz junto as medições
que o justificam — elas mudaram a ordem de duas etapas e mostraram que uma
terceira é maior do que parecia.

Referência estudada antes de escrever: `docs/PESQUISA-OTIMIZACAO.md` (a
bibliografia aplicada — RTA, Safe ICF, ThinLTO, Liška, HyFM),
`references/dart-sdk/pkg/compiler` (`universe/`, `js_backend/`,
`js_emitter/`, `deferred_load/`) e `references/oxc`. As citações de
arquivo:linha abaixo são desses fontes.

A regra que governa tudo o que vem a seguir está na primeira página da
pesquisa, e vale repetir aqui porque o perfil de produção é justamente
onde ela é fácil de quebrar:

> Compartilhar o texto, a árvore sintática ou a implementação de uma função
> **não é** a mesma coisa que transformar duas bibliotecas Dart em uma só.

Dois arquivos vendorizados byte a byte iguais têm estado global separado e
tipos de identidade distinta (`docs/PESQUISA-OTIMIZACAO.md` §3 traz o
contraexemplo pronto). O empacotador da §4 **concatena** módulos e nunca
funde bibliotecas: cada `var L$… = Object.create(dart.library)` continua
sendo uma biblioteca, com os seus símbolos privados e o seu cache de
constantes. Isso não é um detalhe de implementação, é o limite do que a
produção pode fazer.

---

## 0. O ponto de partida, medido

| o que | tamanho |
| --- | --- |
| `corpus/js/01_print.dart` pelo `dart2js -O4` | **34.923 B** |
| o mesmo pelo nosso perfil de desenvolvimento | 1.377 B de módulo **+ 7.087.858 B de `dart_sdk.js`** |

O módulo que emitimos já é menor que a saída do `dart2js`. **Todo** o
excesso é o `dart_sdk.js`, que hoje vai inteiro para o disco e inteiro para
o navegador. Qualquer conversa sobre tamanho no perfil de produção é, em
primeiro lugar, uma conversa sobre o `dart_sdk.js`.

Isso decide a ordem do trabalho e é a diferença estrutural entre nós e o
`dart2js`: **ele compila o SDK junto com o programa**. Confirmado na
referência — `dart:_js_helper`, `dart:_interceptors` e `dart:_rti` são
bibliotecas Dart comuns (`sdk/lib/_internal/js_runtime/lib/`, registradas
em `sdk/lib/libraries.json`) que passam pelo mesmo enqueuer e pelo mesmo
tree shaking do código do usuário; o que as mantém vivas são os "impactos"
declarados em `js_backend/backend_impact.dart:93`, não uma regra de
"sempre incluir". Nós ligamos contra um `dart_sdk.js` **já compilado pelo
DDC**, que é o que nos deu semântica exata de graça (`docs/EMISSAO-DDC.md`).
O preço é que o nosso mundo fechado tem de alcançar dentro de um artefato
JS pronto, não dentro de uma árvore de elementos Dart.

---

## 1. Mundo fechado

### 1.0 O algoritmo, na forma canônica: RTA

A referência mais antiga e mais direta é o **RTA** de Bacon (Berkeley,
1997), que `docs/PESQUISA-OTIMIZACAO.md` §5 põe em uma frase: **cruzar
métodos alcançáveis com classes efetivamente instanciadas, iterando até o
ponto fixo**. Tudo o que o `dart2js` faz em `universe/` é esse algoritmo
com precisão maior; tudo o que fazemos aqui é esse algoritmo sobre um
artefato JS. Os dois avisos que vêm junto são os que mais custam caro:

1. **"Não achei chamada direta" não prova que o método morreu.**
   `package:js`, tearoffs e despacho dinâmico exigem tratamento
   conservador. No nosso caso isso virou três regras concretas, e cada uma
   nasceu de um programa do corpus que quebrou: toda string que pareça um
   nome é tratada como seletor possível (porque `dart.dsend(o, "nome")`
   passa o seletor como dado); os nomes gerados pelo DDC com `#` e `|`
   contam como seletor (`C["_#new#tearOff"]`); e **operador nunca é podado
   por seletor**, porque o membro se declara `['+'](outro)` e quem chama
   escreve `p[$plus](x)` — o alias de `dartx['+']`, que não tem relação
   textual com `+`.
2. **Alcançabilidade de corpo executável, de dados e de informação de tipo
   são coisas separadas.** Uma classe pode precisar de identidade sem
   precisar de um construtor. A nossa classificação já reflete isso sem ter
   sido desenhada para: as entradas de `addRules` são alcançabilidade de
   **tipo**, as de `defineLazy(CT, …)` são de **dados** (constantes), e os
   membros de classe são de **corpo** — três condições diferentes sobre o
   mesmo símbolo.

### 1.1 O algoritmo, como o `dart2js` o faz

O `dart2js` roda um ponto fixo entre **dois conjuntos que só crescem**
(`universe/resolution_world_builder.dart:196`):

* **I** — classes instanciadas (`_instantiationInfo`, `:200`);
* **S** — seletores dinâmicos chamados, com a restrição de receptor
  (`_invokedNames`/`_invokedGetters`/`_invokedSetters`, `:214-216`).

A regra de casamento é: **um membro `m` de `C` está vivo se `C ∈ I` e
existe `s ∈ S` que se aplica a `m`**. O ponto fixo é dirigido por três
índices invertidos de pendência por nome (`:232-240`) — membros de classes
já instanciadas que ainda não foram vistos sendo chamados — para que cada
lado só consulte o outro quando muda. O laço externo
(`resolution/enqueuer.dart:283-302`) não para quando a fila esvazia: ele
reexamina as classes recém-instanciadas, porque `noSuchMethod` e as
constantes podem reabastecer a fila.

Isso é seguro porque toda aresta possível de execução vira um `*Use` num
`WorldImpact` **local** ao membro (`universe/world_impact.dart:22`), e a
única aproximação é para o lado conservador: receptor `dynamic` vira
`isAll = true`, "qualquer receptor" (`universe/world_builder.dart:187-195`).
O `dart2js` confere a invariante num modo seco
(`checkEnqueuerConsistency`, `enqueue.dart:142-156`) que refaz a descoberta
e falha por assert se achar um uso não enfileirado. **Vamos copiar essa
verificação desde o começo** — é a única forma prática de depurar um erro
de soundness num ponto fixo.

### 1.2 Onde o nosso mundo fechado vive

Duas camadas, porque temos dois corpos de código com naturezas diferentes:

**(a) Código do usuário e dos pacotes — sobre a nossa trilha semântica.**
`Program` + `OutlineTypes` + `BodyTypes` já dão tudo o que o ponto fixo
precisa: classes instanciadas (`new C(...)`, literais, `const`), membros
chamados com tipo estático conhecido, e o conjunto de nomes usados em
despacho dinâmico (que hoje o emissor já calcula quando decide entre
chamada direta, `dartx` e `dart.dsend` — `crates/emit_js/src/call.rs`).

**(b) O `dart_sdk.js` — sobre o texto JS, porque ele já está compilado.**
Essa é a decisão de projeto que este perfil introduz, e ela foi validada
por medição antes de ser escrita aqui.

### 1.3 O mundo fechado sobre o `dart_sdk.js`

O `dart_sdk.js` do DDC não é JS arbitrário: é saída de compilador, com uma
gramática pequena e regular. A varredura por caracteres (respeitando
strings, comentários e profundidade de `{}`/`()`/`[]`) parte os 7.087.858 B
em **14.735 declarações de topo, sem perder um byte** (soma das fatias =
tamanho do arquivo). Cada uma cai em uma destas formas:

| forma | o que é | símbolo |
| --- | --- | --- |
| `var core = Object.create(dart.library);` | espaço de nomes da biblioteca | `core` |
| `core.Object = class Object { … };` | classe | `core.Object` |
| `(core.Error.new = function(…){…}).prototype = …;` | construtor | liga em `core.Error` |
| `dart.addRtiResources(core.Uri, […]);` | metadado | liga no 1º argumento |
| `dart.setLibraryUri(C, I[14]);`, `dart.setMethodSignature(C, …)`, `dart.defineExtensionMethods(C, […])` | idem | idem |
| `dart.registerExtension("XMLHttpRequest", html$.HttpRequest);` | despacho nativo | liga no **2º** argumento |
| `dart.applyMixin(V, M);`, `(V[dart.mixinNew] = …)` | aplicação de mixin | liga em `V` |
| `var $length = dartx.length = Symbol("dartx.length");` | símbolo de membro de interceptador | `dartx.length` |
| `dart.defineLazy(ALVO, { get x(){…}, … })` | estáticos preguiçosos | **um por entrada**: `ALVO.x` |
| `dart.copyProperties(io, { … })` | idem | **um por propriedade** |
| `dart_rti._Universe.addRules(u, JSON.parse('{"core|List":{…},…}'))` | regras de subtipagem | **uma por chave**: `core.List` |

As três últimas linhas são sub-divisões obrigatórias: sem elas, três
declarações (o `defineLazy(CT, …)` das 549 constantes com 124 KB, o
`addRules` com 315 KB, o `copyProperties(io, …)`) referenciam o programa
inteiro e derrubam o alcance. Com elas, sobram **19.947 unidades** e apenas
**20 unidades "sempre vivas"** (6 KB), que são o `trackLibraries`, a linha
de `export` e um punhado de registros de erro do `rti`.

**Arestas.** De cada unidade se extrai o conjunto de símbolos referenciados:
identificadores qualificados `lib.Nome`, acesso por string `lib['A|b']`,
índices de constante `C[42]`/`CT.C42` e — este é o que mais importa — os
**nomes dentro de receitas rti**: a string `"core|List<core|int>"` é uma
referência a `core.List` e a `core.int` que nenhum analisador de JS enxerga,
porque `dart_rti._Universe.eval` resolve o tipo por nome em execução. É o
mesmo fenômeno que o `dart2js` trata com `type_reference.dart`; aqui ele
vira uma regra de extração de referências.

**Medição do alcance a partir de `core.print`** (o `01_print.dart`):

| granularidade | vivo |
| --- | --- |
| nada (hoje) | 6.922 KB |
| por declaração/classe | **1.546 KB** |
| por membro (protótipo, conservador) | **1.070 KB** |
| `dart2js -O4`, para referência | 35 KB |

### 1.4 Granularidade por membro, e por que ela é a mesma regra do dart2js

Na granularidade por declaração, uma classe é uma unidade só: `int` fica
vivo, logo todo o `_interceptors.JSNumber` fica vivo, logo
`core._BigIntImpl` (64 KB) entra porque algum método de `JSNumber` o usa.
Foi assim que o protótipo achou o maior desperdício — não por adivinhação,
por rastreio do caminho.

A correção é exatamente a regra da §1.1, escrita sobre o texto:

> uma unidade de membro vive se **(a classe vive)** **e** **(o seletor vive)**.

Um seletor fica vivo quando aparece numa unidade viva, em qualquer das
formas que o contrato do DDC usa: `x.nome`, `x[$nome]`, `dart.dsend(x,
"nome")`, `dart.dload`/`dput`/`dindex`. É um ponto fixo de duas condições
— a unidade só acende quando as duas acendem —, que é o mesmo mecanismo
dos `potential*` decrescentes de `universe/member_usage.dart:315-325`.

**Limite honesto**: o protótipo trata *toda string literal* de uma unidade
viva como possível seletor, o que é conservador demais e explica boa parte
dos 1.070 KB. Refinar isso é trabalho de precisão, não de arquitetura.

### 1.5 O que este plano NÃO promete

Com o `dart_sdk.js` do DDC como runtime, **o piso é da ordem de 1 MB**, não
de 35 KB. O `dart2js` chega a 35 KB porque compila o SDK do fonte com
inferência global: sem isso, cada classe do `dart_sdk.js` carrega tabela de
assinaturas, receitas rti e tabela de métodos de extensão que o DDC emite
para *qualquer* uso possível, e nós só podemos apagar a classe inteira ou
o membro inteiro — nunca a metade do metadado que sobra.

Fechar esses 30× exige compilar `dart:core` e companhia pela nossa própria
trilha (passo 4 do `PLANO.md`), o que está fora deste trabalho. **Isto é
uma conclusão do plano, não uma desculpa**: o brief pede que um passo maior
do que parecia seja dito, e este é ele. O que este perfil entrega é 6,5×
sobre o estado atual, com o mesmo `stdout`, e a arquitetura pronta para o
dia em que o SDK vier do nosso front-end — nesse dia a mesma máquina de
alcance passa a operar sobre elementos, com granularidade de membro real.

---

### 1.6 Resumos, e não o mundo na memória (ThinLTO)

Hoje cada compilação de produção lê os 7 MB do `dart_sdk.js`, classifica as
14.735 declarações em ~45 mil unidades e roda o ponto fixo. Num programa
pequeno isso domina o tempo; num projeto grande domina a memória. A
resposta está em `docs/PESQUISA-OTIMIZACAO.md` §6: **análise global por
resumos**, como o ThinLTO (CGO 2017) — não usar LLVM no caminho Dart→JS,
usar a arquitetura.

Duas aplicações concretas, nesta ordem:

1. **Índice do `dart_sdk.js` em cache.** A classificação do runtime só
   depende do arquivo, não do programa: é um índice (símbolo → unidades,
   unidade → referências e seletores) que deveria ser construído uma vez e
   lido do disco, como já se faz com o outline do SDK
   (`target/dartforge/sdk-<hash>.bin`, 5 ms para ler contra ~105 ms de
   reanálise — `ESTADO.md` §1.6). O ponto fixo em cima do índice é barato;
   construir o índice é que não é.
2. **Resumo por biblioteca do usuário**, para a etapa 5: declarações e
   assinaturas, referências, classes instanciadas, chamadas e efeitos,
   constantes relevantes. O índice global decide o que é alcançável e quais
   corpos merecem ser carregados. Com 7,7 GB de RAM na máquina do
   proprietário, isso deixa de ser elegância e vira requisito — e casa com
   a propriedade que o `dartforge dev` já tem (platô de memória em +0,00 MB
   depois de 20 edições).

A regra de higiene que vem junto (§2.1 da pesquisa): **nenhum cache cresce
sem política de descarte explícita.**

## 2. Despacho

Hoje o emissor decide entre três formas (`crates/emit_js/src/call.rs`):
chamada direta quando o tipo estático do receptor é uma classe Dart, o
símbolo `dartx` quando é um tipo nativo (`int`, `String`, `List`…), e
`dart.dsend`/`dload`/`dput` quando o tipo é desconhecido. A terceira é
sempre correta e sempre a pior.

Com mundo fechado, a regra do `dart2js` é
`locateSingleMember` (`inferrer/typemasks/flat_type_mask.dart:762-801`), e
ela tem **três guardas, todas necessárias**:

1. o seletor não pode ser `call` de closure (não é modelável por membro);
2. o conjunto de alvos tem de ter **exatamente um** elemento;
3. o alvo tem de estar implementado em **todas** as classes do cone do
   receptor (`everySubtypeIsSubclassOfOrMixinUseOf` / `isSubclassOf`).

E há uma quarta no consumidor (`ssa/optimize.dart:1059-1062`): a aridade da
chamada tem de bater **exatamente** com a assinatura, porque a chamada
direta não aplica valores padrão de parâmetros opcionais.

A elegância que vale copiar: `noSuchMethod` **não é caso especial**. Ele
entra no conjunto de alvos como qualquer outro membro
(`universe/function_set.dart:81-90`), então um sítio de chamada que pudesse
cair em `noSuchMethod` já tem ≥ 2 alvos e a guarda (2) o rejeita sozinha.

**O que faremos, e quando.** O conjunto de alvos de um seletor é
exatamente o que o mundo fechado da §1 calcula. A troca de `dart.dsend(o,
"foo", a)` por `o.foo(a)` quando `foo` tem alvo único é, portanto, uma
consequência do mundo fechado, não um trabalho separado — e é por isso que
ela vem **depois** dele na ordem da §6. Quando o receptor é `dynamic` e o
seletor tem mais de um alvo, fica `dart.dsend`: correto, mais lento, e é o
que o perfil de desenvolvimento já faz.

Vale registrar o que já sabemos: no `limitless_ui/example` a compilação
emite **74.332 avisos de inferência de corpos**, e cada um é um sítio que
recuou para despacho dinâmico (`docs/EMISSAO-DDC.md`, "Pedido ao
`crates/types`"). O maior ganho de despacho do perfil de produção não vem
da desvirtualização — vem de fechar essas lacunas de inferência.

---

## 3. Nomes

### 3.1 O esquema, e a invariante que o torna possível

Minificar membros só funciona por causa de uma propriedade do esquema de
nomes do `dart2js` (`js_backend/namer.dart:703-737`): a chave de
disambiguação é

```
<chaveDaBiblioteca>@<nomeOriginal>@<sufixoDeAridade>@<nomeados…>
```

com `chaveDaBiblioteca` vazia para nomes públicos. Logo **todo `foo`
público do programa inteiro, com a mesma aridade, recebe o mesmo nome
curto** — e é isso que faz `x.foo()` continuar funcionando sem saber o tipo
de `x`. Definição e sítio de chamada convergem *por construção*, sem tabela.
`@` é o separador porque não pode aparecer num identificador Dart.

Nomes anotados (`get$x`, `set$x`, `$isC`, `call$2`) são **concatenação em
tempo de execução** dentro do `js_helper`, então o prefixo não pode ser
minificado em separado — só encurtado globalmente (`get$`→`g`, `set$`→`s`,
`$is`→`$i`). Daí a regra `hasBannedMinifiedPrefix` (`namer.dart:2712-2715`):
**nenhum nome de instância minificado pode começar por `g` ou `s`**.

Dois conjuntos de reservados, e a assimetria importa: propriedades usam o
conjunto pequeno (`jsReserved` = keywords + `__proto__`/`prototype`/
`constructor`/`call`/`eval`/`arguments`, `namer.dart:2699`); variáveis locais
usam a união de cinco listas (`namer.dart:2002`). `x.if` é ilegal, `x.Array`
não é.

### 3.2 A lista de fugas — nomes que não podem ser minificados

| fuga | por quê | critério de segurança |
| --- | --- | --- |
| **`@JS` / `dart:js_interop` / `@staticInterop`** | o nome existe num objeto JS que não controlamos | `NativeData.isJsInteropMember`, `native_data.dart:848-867`; o nome entra como **texto do template**, nunca como nome gerenciado (`ssa/codegen.dart:2902-2960`). No nosso emissor já é o que `Ctx::is_js_member`/`js_interop_member_name` decidem (`crates/emit_js/src/ctx.rs`). |
| **`@Native` / `@JSName` / tags de dispatch** | propriedade real do DOM | `native_data.dart:437,805-806`. Nós já temos `Ctx::native_set` e `native_member_names` (`module.rs:1611`). |
| **campos de classe que estende nativa** | um objeto nativo pode ter qualquer propriedade | `namer.dart:545-552,588-591`: o campo recebe `<Classe>_<campo>` ou prefixo `$$`. |
| **`noSuchMethod` / `Invocation.memberName`** | o nome de origem é observável | **não existe tabela**: o `dart2js` embute a string fonte literal em cada stub (`class_stub_generator.dart:185`), e passa o nome minificado só como segundo argumento. Custo em bytes, não em corretude. |
| **`#foo` / `const Symbol`** | mesma codificação do anterior | `constants/constant_system.dart:253-264` — o campo `_name` é a string fonte (`"_foo@<uri>"` se privado). Por isso `invocation.memberName == #foo` funciona sem tradução. |
| **propriedades nativas curtas** | `x.at()` num `String` chamaria `String.prototype.at` **em silêncio** em vez de lançar `NoSuchMethodError` | a lista de ~250 nomes de `minify_namer.dart:72-104`, queimada no escopo **antes** da primeira alocação. Exceção deliberada: **campos** podem reusá-las (`field_naming_mixin.dart:62-80`), porque um campo só é acessado de receptor estaticamente conhecido. |
| **`Type.toString()`** | — | o `dart2js` **degrada de propósito**: só 10 classes (`int`, `double`, `num`, `String`, `bool`, `Null`, `List`, `Object`, `Map`, `JSObject`) vão para `MANGLED_GLOBAL_NAMES` (`fragment_emitter.dart:1956-1990`); o resto vira `"minified:xY"`. |
| **`dart:mirrors`** | — | **não implementar nada.** Não existe mais: `MirrorsData` foi removido do `pkg/compiler`, e `MANGLED_NAMES` é emitido **vazio** (`fragment_emitter.dart:2043-2052`), com o comentário de que a tabela custava tamanho demais. |

A decisão de arquitetura a copiar: o namer **nunca** verifica um nome
depois de gerá-lo. Reservados, prefixos banidos, sugestões e nomes nativos
são pré-carregados no escopo antes da primeira alocação
(`reserveBackendNames`, `frequency_namer.dart:21-34`) — o que torna
impossível esquecer uma verificação num caminho novo.

### 3.3 O que o `oxc` resolve e o que não resolve

`references/oxc` tem `oxc_parser`, `oxc_codegen`, `oxc_minifier`
(`Compressor`, peephole, DCE), `oxc_mangler` e — o achado útil —
`oxc_minifier::PropertyMangler`
(`crates/oxc_minifier/src/property_mangler.rs`), com API de três fases
(`collect` → `assign` → `rewrite`) feita justamente para **consistência
entre vários arquivos**, e um `ManglePropertyCache` que permite **fixar**
mapeamentos.

O que ele **não** resolve, e é o essencial: ele é puramente sintático.
Colapsa por string (não sabe que `A.foo` e `B.foo` são membros distintos,
nem que dois membros que não colidem na hierarquia poderiam dividir o mesmo
nome curto), escolhe o que minificar por *regex*, e não enxerga acesso
dinâmico, interop nem `noSuchMethod`. Essa inteligência é a do mundo
fechado, e tem de vir de nós.

**Decisão**: não escrever um minificador do zero **e** não deixar o `oxc`
escolher nomes. O mapeamento nome→nome é calculado por nós (onde temos a
hierarquia e a lista de fugas) e injetado via `ManglePropertyCache`; o
`oxc` entra como **reescritor** (`PropertyMangler::rewrite`, que não é
idempotente — exatamente uma aplicação por programa) e como impressor. O
mangler de escopo (`oxc_mangler`) cuida dos locais, onde não há fuga
possível.

---

### 3.4 Emissão pensada para a engine

`docs/PESQUISA-OTIMIZACAO.md` §8 acrescenta três coisas que não são sobre
tamanho, e sim sobre o que o V8 faz com o que emitimos:

* **Shapes / hidden classes** — inicializar **sempre as mesmas propriedades
  na mesma ordem, dentro do construtor**. Atribuição condicional fora dele
  degrada o inline cache de monomórfico para megamórfico. Vale conferir
  contra o que o nosso `emit_constructor` faz hoje: os campos são
  inicializados no construtor (`emit_field_inits`), o que é o certo, mas
  `late` e campos de mixin merecem ser medidos.
* **Despacho plano** para o que sobrar polimórfico, em vez de cadeias
  longas de protótipo.
* **Minificação por frequência** — os símbolos mais frequentes recebem os
  nomes mais curtos. É o `FrequencyBasedNamer` do dart2js
  (`frequency_namer.dart:72-86`), que descarta os nomes de refcount zero
  **antes** de alocar; a pesquisa confirma a regra por outro caminho.

## 3.5 Deduplicação de funções

Fica para depois da minificação, e **não se faz sobre o texto JavaScript**.
Essa é a lição do SalSSA (`docs/PESQUISA-OTIMIZACAO.md` §13): não
simplificar um passe destruindo informação que os seguintes terão de
reconstruir. No texto JS já se perdeu o acesso direto a símbolos, tipos e
efeitos — que é exatamente o que a verificação de equivalência precisa. A
comparação e a fusão são na trilha tipada.

Vale marcar a fronteira, porque o resto deste documento faz o contrário: a
poda do `dart_sdk.js` opera sobre texto **porque não há alternativa** — o
runtime já vem compilado pelo DDC e não temos IR dele. É uma concessão
delimitada a um artefato de terceiros, não um método.

`docs/PESQUISA-OTIMIZACAO.md` §4 e §12 dão o algoritmo que evita o erro
caro:

1. **candidatos baratos primeiro** — agrupar por assinatura, estrutura de
   controle e sequência de operações normalizada; com 100 mil funções, todos
   contra todos seriam ~5 bilhões de pares;
2. **normalizar nomes locais, nunca identidades externas** —
   `proximo() => ++contador` só é equivalente a outra se o `contador`
   resolver para o **mesmo** armazenamento;
3. **refinamento de partição até estabilizar**, como o ICF do `gold`/`mold`:
   duas funções idênticas que chamam símbolos diferentes só são
   equivalentes se os alvos também forem — e recursão mútua exige tratar
   componentes fortemente conexos, que hash recursivo ingênuo não resolve;
4. **hash igual é candidato, não prova** — depois do hash, comparação
   estrutural completa;
5. **preservar identidade observável** (Safe ICF, 2010): se a referência da
   função escapa para comparação (`identical`, `==`), manter declarações
   separadas com **corpo compartilhado**.

E as quatro identidades que têm de ficar distintas
(`docs/PESQUISA-OTIMIZACAO.md` §12) — duas declarações podem compartilhar
`BodyId` sem compartilhar o resto:

```text
DefinitionId → a declaração
TypeId       → identidade e representação do tipo
StorageId    → o armazenamento de estado
BodyId       → a implementação executável
```

Equivalência é **no grafo, não no arquivo**: `validar_publicado` chamando
`normalizar_publicado` e `validar_vendor` chamando `normalizar_vendor` são
equivalentes se as duas versões de `normalizar` também forem. Comparar
incluindo os identificadores dos alvos rejeita cedo demais.

Fusão de funções *parecidas* (Sequence Alignment, CGO 2019; HyFM, LCTES
2021) é modo orientado a tamanho, com **orçamento explícito de tempo e
memória e desistência antecipada** (§14): filtro barato → candidatos
compatíveis → estimativa de economia → análise detalhada só dos
promissores → transformação → verificação. O modelo de custo é nosso:
**bytes emitidos, bytes comprimidos, adaptadores necessários e execução no
navegador** — nunca instruções removidas de uma IR. A memória se mede em
quatro categorias (programa carregado, caches, temporário do passe, pico do
processo), e não se cita o número do artigo como se fosse do compilador
inteiro: os 48 MB do HyFM são do passe avaliado.

**Fora deste plano, e registrado para não se perder tempo**: o BOLT (§15)
não otimiza `.js` — ele otimizaria o nosso `dartforge.exe`, e só aceita ELF
x86-64/AArch64, o que exclui o `.exe` de Windows. Acelerar o compilador é
PGO do `rustc`, e é outro trabalho.

## 4. Saída

**Um arquivo**, sem `import`/`export`, sem `dart_sdk.js` ao lado.

O `dart2js` monta o seu com um preâmbulo de ~350 linhas escritas à mão
(`js_emitter/startup_emitter/fragment_emitter.dart:36`, `_mainBoilerplate`)
e uma ordem de fases fixa (`:387-441`): holders → protótipos → tear-offs →
herança → regras de tipo → constantes → estáticos → `invokeMain`. Os
"holders" não são conceito do modelo: são resolvidos tarde, por um
finalizador (`js_backend/deferred_holder_expression.dart:394`), e nomeados
**por frequência de referência** (`:702-740`) — daí os `A`, `B`, `C` da
saída minificada.

Nós não precisamos de holders nem de preâmbulo próprio: o preâmbulo já é o
`dart_sdk.js` (a parte dele que sobreviver à §1), e a ordem de fases já
está resolvida pela ordem topológica dos módulos que o emissor calcula.
O bundle é:

```
[ dart_sdk.js podado, sem a linha `export { … }` ]
[ preâmbulo (globalThis.self) ]
var L$a = Object.create(dart.library);      // um por biblioteca, no topo
var L$b = Object.create(dart.library);
(function () { …corpo do módulo A, sem import/export… })();   // ordem topológica
(function () { …corpo do módulo B… })();
L$main.main();
```

**Por que IIFE por módulo, e não concatenação.** Cada módulo declara no
preâmbulo `var`s locais com nomes gerados (`var _foo$ = dart.privateName(L$x,
"_foo")`, os aliases `dartx`, `_is`/`_as`/`_eval`). Esses nomes são únicos
**dentro** do módulo, não entre módulos: concatenar faria a segunda
declaração de `_foo$` sobrescrever a primeira, e dois `_foo` de bibliotecas
diferentes são símbolos **diferentes** — seria um defeito silencioso de
privacidade. Renomear por prefixo seria reescrita de texto sobre código que
usa esses nomes como chaves computadas: arriscado e desnecessário. A IIFE
resolve por escopo, custa 20 bytes por módulo, e mantém no topo exatamente
o que precisa ser compartilhado (os `L$…`, que o emissor já declara na
primeira linha de cada módulo).

Carregamento diferido fica **fora deste plano**. O `dart2js` o faz
particionando por `ImportSet` (`deferred_load/deferred_load.dart:5-60`) e
emitindo `.part.js` que se registram num global; é uma etapa inteira, e o
brief não a pede.

---

## 5. Verificação

Nada entra sem passar por aqui.

1. **`corpus/js` inteiro, mesmo `stdout` byte a byte.** `crates/diferencial`
   ganha `--producao`: um quarto executor, ao lado de `dart run`,
   `dartdevc`+Node e do nosso desenvolvimento. O relatório compara
   **VM × desenvolvimento × produção** e falha se qualquer um divergir. O
   placar de produção vai para o `ESTADO.md`.
2. **`node --check`** no arquivo único antes de executá-lo: um bundle que
   nem parseia tem de falhar com mensagem própria, não com um erro de
   execução a dez quadros de profundidade.
3. **Tamanho e tempo contra o oficial**, nos dois projetos reais
   (`new_sali/frontend` e `limitless_ui/example`), com
   `dart compile js -O4` do SDK 3.6.2 como régua. O `limitless_ui` tem
   suíte e2e (26/26) e a sonda das 53 rotas: o bundle de produção tem de
   passar nas duas, como a saída oficial passa.
4. **Auto-teste do alcance**, no espírito de `checkEnqueuerConsistency`
   (`enqueue.dart:142-156`): um modo que, depois do ponto fixo, refaz a
   descoberta a seco e falha se achar unidade alcançável não marcada. Sem
   isso, um erro de soundness aparece como "um programa do corpus quebrou"
   três etapas depois.

5. **Determinismo, desde já** (`docs/PESQUISA-OTIMIZACAO.md` §11). Mesmas
   entradas e mesma configuração têm de dar **o mesmo arquivo** com 1, 4 e
   8 trabalhadores — inclusive nomes gerados, ordem de diagnósticos e hash
   do módulo. A razão é prática e visível: o `dartforge serve` recarrega
   por geração, e geração que muda à toa é um defeito que o usuário vê.

   Onde estamos: o perfil de produção hoje é determinístico por
   construção — a classificação é sequencial, as unidades são reemitidas na
   ordem do arquivo e nenhuma decisão depende de endereço ou de ordem de
   conclusão. Isso **é uma propriedade a preservar**, não um acaso. Duas
   consequências para o que vem:
   * quando a classificação for paralelizada, o paralelismo é **dentro** da
     etapa, com as etapas em sequência, cada trabalhador publicando
     resultado local — nunca um `Mutex` sobre o índice disputado por todos;
   * a minificação por frequência precisa de **desempate canônico e
     estável** (o dart2js desempata pela chave do seletor,
     `frequency_namer.dart:72-86`); sem isso, dois builds iguais geram
     nomes diferentes.

   O teste entra no `crates/diferencial`: compilar o mesmo programa com
   1, 4 e 8 trabalhadores e comparar o arquivo byte a byte.

6. **Os cenários de `docs/PESQUISA-OTIMIZACAO.md` §9**, que são mais do que
   o corpus mede. Os que este perfil tem de responder:

   | cenário | o que medir |
   | --- | --- |
   | compilação de produção sem cache | tempo por fase, pico de RAM, alocações |
   | pacote copiado 1, 2, 4 e 8 vezes | crescimento do JS emitido — é o teste que prova que a deduplicação funciona **e** que a identidade de biblioteca foi preservada |
   | identidade de tipos e estado global | dois arquivos vendorizados iguais continuam sendo duas bibliotecas (o contraexemplo do §3 da pesquisa vira caso de corpus) |

   Para deduplicação o teste é de **comportamento**, não de texto:
   identidade de tipos, estado independente, funções como valores,
   inicialização, exceções e referências vindas de código gerado. Onde o
   Dart admite mais de um resultado válido (identidade de closures), o
   teste aceita o conjunto, não uma implementação.

7. **Relatório de poda, não só um número.** `docs/PESQUISA-OTIMIZACAO.md`
   §10 pede que a saída explique o resultado. O `--sem-poda`/`--sem-membros`
   e o `DARTFORGE_JSPROD_QUEM`/`_CAMINHO`/`_GATILHO` já são a metade
   diagnóstica disso; falta a metade narrativa:

   ```text
   runtime 6.922 KB -> 1.474 KB (8.450 de 44.965 unidades)
   maiores sobreviventes: core._BigIntImpl 64 KB, async.Stream 47 KB
   core._BigIntImpl vivo por: _interceptors.JSNumber -> sel:toRadixString
   ```

Regra que não muda: **compilar é confirmação, não método de descoberta.**
Cada etapa abaixo tem o placar do corpus como critério de saída.

---

## 6. Ordem de trabalho

Por quanto cada passo destrava, com o corpus como placar em cada um.

| # | etapa | destrava | placar-alvo |
| --- | --- | --- | --- |
| 1 | **Bundle de arquivo único** — remover `import`/`export`, IIFE por módulo, ordem topológica, `dart_sdk.js` embutido, `main()` no fim. | tudo o que vem depois: passa a existir **um** programa para analisar. Sozinho já troca N+1 arquivos por 1. | corpus 214/214, tamanho ainda ~7 MB |
| 2 | **Alcance no `dart_sdk.js` por declaração** — segmentação, classificação, referências (inclusive receitas rti), ponto fixo, poda. | o único eixo de tamanho que existe hoje. | corpus 214/214, ~1,5 MB |
| 3 | **`--producao` no `crates/diferencial`** — VM × desenvolvimento × produção, e o auto-teste de alcance. | torna 1 e 2 verificáveis em vez de plausíveis. Vem aqui, e não no fim, porque as etapas seguintes sem ele são tentativa e erro. | o próprio placar |
| 4 | **Granularidade por membro** — classe partida por membro, seletores vivos, ponto fixo de duas condições. | ~30% a mais de poda; e é a mesma regra que o mundo fechado sobre elementos vai usar. | corpus 214/214, ~1,0 MB |
| 5 | **Mundo fechado sobre a nossa trilha** — o ponto fixo da §1.1 sobre `Program`/`OutlineTypes`, podando classes e membros do usuário e dos pacotes antes de emitir. | pouco no corpus (programas pequenos); **muito** nos projetos reais, onde hoje se emitem 48 MB. | corpus 214/214 + `limitless_ui` 26/26 |
| 6 | **Despacho direto por alvo único** — as três guardas de `locateSingleMember` mais a de aridade. | velocidade, e tamanho pela queda dos sítios `dart.dsend`. | corpus 214/214 |
| 7 | **Minificação** — mapeamento calculado por nós com a lista de fugas da §3.2, aplicado pelo `PropertyMangler` do `oxc`; locais pelo `oxc_mangler`. | tamanho; entra por último porque é a etapa cujo erro é mais difícil de ver (um nome errado só aparece num caminho raro). | corpus 214/214 |

Fora deste plano, e registrado como tal: carregamento diferido (`deferred`
→ `import()`), divisão em chunks, e compilar o SDK pelo nosso front-end —
que é o único caminho para fechar os 30× que faltam para o `dart2js` (§1.5).

### 6.2 Como isto se encaixa na ordem geral (`PESQUISA-OTIMIZACAO.md` §17)

A pesquisa dá cinco etapas para o projeto inteiro. As sete deste documento
são o recorte do backend JS, e caem assim:

| etapa geral (§17) | o que deste plano a cumpre |
| --- | --- |
| 1. medição e determinismo | §0 (a medição que ordena o trabalho) e §5.5 (o teste de 1/4/8 trabalhadores) |
| 2. representações e incrementalidade | §1.6 — índice do `dart_sdk.js` em cache e resumo por biblioteca |
| 3. alcançabilidade e fusão conservadora | etapas 1–5 aqui (mundo fechado, poda, granularidade por membro) |
| 4. fusão por grafo e código semelhante | §3.5, depois da minificação (etapa 7) |
| 5. PGO do compilador | fora deste plano (§3.5, nota sobre BOLT) |

E os dois modos que a pesquisa exige que fiquem separados já existem: o
`dartforge serve` é o de incrementalidade e previsibilidade; o
`dartforge-jsprod` é o de trabalho global. Nenhuma otimização de produção
entra no caminho do `serve`.

---

## 6.1 O que já está implementado

Etapas 1 a 4 do quadro acima, em `crates/emit_js_producao`
(`dartforge-jsprod`). `crates/emit_js` não foi tocado: a produção
**consome** a emissão de desenvolvimento.

| módulo | o que faz |
| --- | --- |
| `varredura.rs` | parte JS de compilador em declarações de topo, entradas de objeto e membros de classe. Não é parser: é um leitor de caracteres que respeita strings, comentários e profundidade. O teste exige **conservação**: a concatenação das fatias é byte a byte a entrada. |
| `bundle.rs` | separa cada módulo em namespaces içáveis, dependências e corpo; ordena topologicamente pelo grafo de `import` lido do próprio texto; monta o arquivo único com uma IIFE por módulo. |
| `sdk.rs` | classifica as 14.735 declarações do `dart_sdk.js`, extrai referências (inclusive as das receitas rti) e seletores, e reemite as unidades vivas na ordem do arquivo, recolocando as vírgulas dos grupos. |
| `alcance.rs` | o ponto fixo: uma unidade acende quando **algum** gatilho está vivo e **todos** os requisitos estão. A conjunção é o que dá granularidade de membro. |

Três defeitos que a medição pegou — e que o diagnóstico embutido
(`DARTFORGE_JSPROD_QUEM`, `_CAMINHO`, `_GATILHO`) localizou em vez de
adivinhar. Cada um virou teste de regressão:

1. **`dart.applyMixin(V, M);` lida como declaração de `dart.applyMixin`.**
   Esse símbolo só é referenciado pelos próprios sítios de chamada: um
   laço que se apaga sozinho. As 93 aplicações de mixin sumiam, e
   `defineExtensionAccessors` depois procurava no protótipo um getter que
   não existia mais. Correção: `lib.Nome` só é declaração quando vem `=`
   ou `[` depois.
2. **`dart.defineLazy(html$.Event, {…})` com o alvo lido como `html$`.**
   O espaço de nomes está sempre vivo e a cabeça do grupo cita a classe,
   então `dart:html` inteiro entrava num programa que só faz `print`:
   4.845 KB contra 1.650 KB.
3. **Classes ligadas por `registerExtension` a tipos embutidos do JS.**
   `"olá"` é um `String` do JS e só vira `core.String` porque
   `registerExtension("String", _interceptors.JSString)` rodou — nenhum
   nome do programa cita a classe. São raiz, como os impactos de
   `js_backend/backend_impact.dart:93` declaram no dart2js.

## 7. Território

Crate novo `crates/emit_js_producao`, com o binário `dartforge-jsprod`
(`crates/cli/src/main.rs` é de outro agente e não é tocado). Em
`crates/emit_js` **só se acrescenta**, porque o `dartforge serve` depende
do que já existe. `crates/diferencial` ganha o modo `--producao`.
