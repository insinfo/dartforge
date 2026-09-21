# Subconjunto implementado

Base mínima de compatibilidade: Dart 3.6.2. A evolução inclui recursos posteriores e extensões experimentais, identificados abaixo.

## Suporte atual

- Uma entrada `void main()` ou `Future<void> main() async`, sem parâmetros.
- Funções de nível superior com retorno explícito `int`, `String`, `bool` ou `void`.
- Parâmetros posicionais obrigatórios, opcionais `[...]` e nomeados `{...}` com
  `required` e valores padrão; chamadas antecipadas e recursivas.
- `return` com valor ou vazio, conforme o tipo; `if/else` com blocos obrigatórios.
- Declarações locais inicializadas `var`/`final`/`int`/`String`/`bool`, atribuições e escopos.
- Literais inteiros i32, strings Unicode, booleanos e `print`.
- Operadores `+ - * == != < <= > >= && ||` e unários `- !`; parênteses.
- Comentários de linha e de bloco aninhados.

Validação: nomes, escopos, duplicatas, uso antes da declaração, autorreferência,
mutabilidade, tipos, aridade, argumentos, retorno e condições `bool`. Funções não `void`
precisam de retorno comprovável em todos os caminhos; a análise conservadora pode
rejeitar condições constantes ou retornos em laços que Dart aceitaria.
Parâmetros são mutáveis e podem ser sombreados por locais do corpo.

## Laços e atualizações

`while`, `do { ... } while (...);` e `for` clássico exigem blocos com chaves. As cláusulas
inicialização, condição e atualização do `for` são opcionais, inclusive em `for (;;) {}`.
A inicialização aceita uma declaração inicializada, atribuição, atualização ou chamada;
a atualização aceita atribuição, atualização ou chamada. Não há listas de cláusulas
separadas por vírgulas nem declarações na atualização.

`break` e `continue` atuam sobre o laço mais próximo, sem rótulos, e são rejeitados fora
de laços. `continue` no `for` executa a atualização antes do próximo teste; no `do/while`,
reavalia a condição. A variável declarada no cabeçalho do `for` tem escopo no próprio
laço. Locais declarados no corpo não ficam visíveis na condição ou na atualização.

`++` e `--` prefixados ou pós-fixados e as atribuições compostas `+=`, `-=` e `*=` são
aceitos somente sobre identificadores, como instruções ou cláusulas de `for`. Não
produzem valores utilizáveis em expressões: `print(i++)` e `var x = ++i` são rejeitados.
As atualizações respeitam tipos e `final`; `+=` também permite concatenar duas strings.

```dart
void main() {
  var total = 0;
  for (var i = 0; i < 4; i++) {
    if (i == 1) { continue; }
    total += i;
  }
  print(total);
}
```

## Strings

Strings de uma linha usam aspas simples ou duplas. São aceitos escapes de aspas,
barra invertida e dólar, controles `\b`, `\f`, `\n`, `\r`, `\t`, `\v`, escapes
hexadecimais `\xHH`, Unicode `\uHHHH` e `\u{H...}` com um a seis dígitos hexadecimais.
Pares UTF-16 válidos, como `\uD83D\uDE00`, são decodificados; surrogates isolados são
rejeitados explicitamente. Escapes desconhecidos perdem a barra invertida, conforme
Dart; por exemplo, `\q` produz `q`. Para NUL, use `\x00` ou `\u0000`.

Strings raw `r'...'` e `r"..."` preservam barras invertidas e dólares literalmente.
Um escape `\n` dentro de uma string de uma linha pode produzir uma quebra de linha
no valor; uma quebra de linha escrita na fonte exige aspas triplas.

### Interpolação, aspas triplas e literais adjacentes

Há interpolação: `'$nome'` interpola o identificador e `'${expressão}'` interpola
a expressão inteira. A distinção é léxica, então `'$obj.campo'` interpola apenas
`obj` e concatena `.campo` como texto, e `'$a$b'` são duas interpolações. Depois
de `$` é preciso vir um identificador que não seja palavra reservada, ou `{`.
`\$` e as strings raw continuam com o cifrão literal.

Aspas triplas `'''...'''` e `"""..."""` aceitam quebras de linha e interpolação;
`r'''...'''` não interpola. A primeira linha depois da abertura é descartada
quando só tem espaços e tabulações, e CR e CRLF escritos na fonte viram LF.
Literais adjacentes (`'a' 'b' '$c'`) concatenam em tempo de compilação, em
qualquer combinação de formas.

O resultado é `String`. Cada expressão é avaliada exatamente uma vez, na ordem
escrita; `null` vira `"null"`; `int` usa a conversão do runtime, não `String(x)`
do JavaScript. Uma instância cuja classe — ou um ancestral — declara
`String toString()` interpola pelo texto que ela produz, escolhido pelo objeto
e não pelo tipo estático. Enums, funções, `Future`, `Duration`, `Timer` e
instâncias **sem** `toString` declarado continuam rejeitados; a instância tem
diagnóstico próprio, que nomeia a classe e registra a decisão de não emitir
`Instance of 'Nome'`. O contrato completo das strings está em
[STRINGS.md](STRINGS.md); o do protocolo `Object`, em [OBJETO.md](OBJETO.md).

## Tipos anuláveis e objetos

Veja os contratos de [null safety e fluxo](NULL-SAFETY.md) e [classes/herança](CLASSES.md).
A ferramenta compila [bibliotecas, reexports e filtros show/hide](MODULES.md), resolve [pacotes](PACKAGES.md) e aceita [extensions em uma unidade](EXTENSIONS.md).

## Protocolo `Object` e acessores de instância

Getters e setters de instância, inclusive o par de mesmo nome; `operator ==`
com as duas regras do Dart (`null` à esquerda nunca chama o operador; quem
decide é o lado esquerdo); `hashCode` como getter sobrescrito; `toString`
integrado a `print` e à interpolação; e `identical`, que continua sendo
identidade de referência. `Map` e `Set` deste subconjunto **não** usam
`hashCode`: comparam chaves por identidade. `dynamic` é recusado de propósito,
com a mensagem explicando que o subconjunto é estaticamente resolvido.
Contrato, diagnósticos exatos e limites em [OBJETO.md](OBJETO.md).

## Ainda não suportado

Prefixos de import, partes, funções locais nomeadas, parâmetros nomeados ou
opcionais em closures, extensions, genéricos e `@Native`,
overloads, `double`, classes/métodos genéricos, extensions importadas, `for-in`,
rótulos, surrogates isolados, interpolação em expressão constante, formas de Set/Map/const além das documentadas abaixo,
source maps e bibliotecas padrão completas. Atualizações compostas de campos, acesso a índices e atribuições
usadas como expressões também não são suportadas.
Identificadores são ASCII; alguns nomes contextuais válidos em Dart ficam reservados
neste parser. Os casos não implementados retornam erro explícito.

A HIR preserva a AST com IDs de classe e alvos de chamadas de extensions resolvidos.
Ainda não há uma IR completa com IDs para todos os locais, membros e tipos.
Números usam JavaScript Number; limitar literais a i32 não limita a faixa dos resultados.
Não há garantia de equivalência integral à VM nem implementação completa de `int` Dart.
Casos numéricos de borda são testes de regressão, não prova geral de conformidade.

O parser limita aninhamento a 64 níveis estruturais, 16 primárias ativas e expressões a 128 nós, inclusive argumentos de
chamadas. Esses são limites temporários do protótipo, não da linguagem Dart.
Não existe limite estático para recursão ou iterações executadas pelo programa gerado.

Há [cache da última saída](CACHE.md) validado pelo conteúdo integral do grafo.
Há fusão opcional e conservadora de funções top-level idênticas por `--merge-identical-functions`.
Não há recompilação incremental por unidade, servidor LSP, servidor HTTP ou compilador
ngdart completo. Também não há benchmark válido de vantagem sobre DDC/dart2js.

## Documentação e contribuições

[CONTRIBUTING.md](../CONTRIBUTING.md) define as verificações e a documentação Rust em
português: `//!` para módulos, `///` para funções, contratos, limitações e exemplos
executáveis nas APIs públicas. A aceitação de sintaxe deve vir acompanhada de testes
que preservem efeitos, ordem de avaliação e escopos.

## Alvo nativo LLVM

O backend nativo tem um subconjunto separado: int64 modular, bool, void,
funções/recursão, variáveis, controle de fluxo e bibliotecas/pacotes. Suporta strings,
classes com construtor implícito, herança e despacho virtual, com GC de tracing preciso.
Não suporta extensions nativas. Aceita int?/bool?/String?/classes anuláveis, null, ?? e !, com
falha imediata da asserção em vez de exceção capturável. A validação da HIR rejeita os demais
recursos antes do driver, inclusive em código não executado. Os literais ainda são
limitados a i32 pelo parser compartilhado; os resultados das operações usam i64.
O alvo JS mantém sua representação Number. Ver [referências e plano AOT](AOT-REFERENCIAS.md).

Funções e métodos tipados aceitam corpos `=> expressão;`, inclusive `void` com
descarte do resultado. Strings nativas usam UTF-8 para concatenação, igualdade e
impressão do subconjunto. Não oferecem toda a API String/UTF-16. Referências
temporárias usam slots estáticos reutilizados por iteração; podem ficar retidas
até sobrescrita ou retorno, sem análise completa de vivacidade.

## Interfaces, classes abstratas e enums

JS e LLVM aceitam `abstract class`, `interface class`, `abstract interface class`,
`implements` múltiplo e herança simples. Classes concretas precisam satisfazer os
contratos transitivos de métodos; parâmetros são contravariantes e resultados
covariantes. Um contrato `void` permite descartar o resultado concreto.
`implements` não herda implementação. `interface class` só permite `extends` na
própria biblioteca. Contratos de campos/getters/setters em interfaces ainda são rejeitados.
Conflitos entre assinaturas herdadas exigem uma declaração explícita compatível.

Enums simples têm identidade nominal, valores canônicos, `.name`, `.index` e
nullabilidade. O JavaScript também aceita o subconjunto de enums avançadas descrito
abaixo; LLVM continua restrito a enums simples. `values` e impressão direta do enum
ainda não são suportados. Valores privados respeitam bibliotecas.

## Coleções e closures no JavaScript

List<T>, Iterable<T>, tipos de função, closures e um subconjunto de dart:core
estão descritos em [COLECOES-CLOSURES.md](COLECOES-CLOSURES.md).
As representações do runtime Rust existem, mas o lowering LLVM ainda rejeita essas construções.

## Conjuntos, espalhamentos, `?.` e operadores de bits

`Set<T>` tem literal (`{1, 2}`, `<int>{}`), ordem de inserção preservada, `add`,
`contains`, `length` e iteração. `{}` sem argumentos de tipo continua sendo o
mapa vazio, como em Dart; um literal formado só por espalhamentos exige `<T>` ou
`<K, V>`, porque a forma dependeria do tipo estático do operando.

Espalhamentos `...` e `...?` valem em lista, conjunto e mapa; `...` exige
operando não anulável, como nos SDKs 3.6.2 e 3.13.4. Elementos `if`, `if-else`
e `for` (clássico e `for-in`) valem em lista, conjunto e mapa, aninhados e
combinados com espalhamentos — em mapa, `if`/`for` exigem `<K, V>` explícito.

`a?.b`, `a?.b()` e `a?[i]` curto-circuitam a cadeia inteira: `a?.b.c` só avalia
`.c` quando `a` não é null, o receptor é avaliado uma única vez e o resultado é
anulável mesmo quando o seletor não é.

`|`, `&`, `^`, `~`, `<<`, `>>` e `>>>` aceitam apenas `int` e seguem a semântica
de inteiro do alvo web (32 bits sem sinal), conferida contra `dart compile js`
numa grade de 696 casos. A VM usa 64 bits com sinal e diverge quando o resultado
tem o bit de sinal ligado; a tabela completa da divergência, os diagnósticos
exatos e os limites estão em
[COLECOES-OPERADORES.md](COLECOES-OPERADORES.md).

## Incremento 13: genéricos e constantes

O backend JavaScript amplia enums com campos escalares finais, construtor const,
métodos e getters, além de switches com guardas. Funções genéricas top-level e
constantes locais/listas canônicas estão descritas em
[GENERICS-CONST-ENUMS.md](GENERICS-CONST-ENUMS.md). As restrições anteriores sobre
enums avançadas continuam aplicáveis ao backend LLVM, cujo lowering está pendente.

## Modificadores e mixins

Base/final/sealed e mixin/mixin class são descritos em
[CLASS-MODIFIERS.md](CLASS-MODIFIERS.md). With em classes é expandido antes da análise
para aplicações abstratas com despacho e armazenamento por instância. Os backends
JS e LLVM compartilham essas aplicações; switches sealed e padrões C() permanecem
no JavaScript. Part/part of, on, super e aplicações de mixins em enums continuam pendentes.

## Imports e exports condicionais

Diretivas com if e igualdade de strings selecionam a primeira alternativa verdadeira
por perfil Dart 3.6.2. Veja [o incremento 15](IMPLEMENTACAO-15.md). JS e Native AOT
usam seus próprios perfis; Wasm só permite inspeção do grafo. Bibliotecas SDK
anunciadas pelas condições não estão automaticamente implementadas no DartForge.

## Anotações e funções nativas

O [incremento 16](IMPLEMENTACAO-16.md) admite @override/@deprecated/@Deprecated
nas declarações descritas no contrato e @Native em funções external top-level.
Import dart:ffi é aceito no perfil Native AOT para Int32/Int64/Void, com ligação
estática de objetos. Isso substitui a limitação anterior de import FFI inteiramente
indisponível, sem habilitar o restante da biblioteca.

## this e construtores generativos

O [incremento 17](IMPLEMENTACAO-17.md) implementa o subconjunto de construtores
posicionais descrito em [THIS-CONSTRUCTORS.md](THIS-CONSTRUCTORS.md), nos backends
JavaScript e LLVM. Isso substitui as restrições anteriores de construtores apenas
implícitos ou exclusivos de enums, mantendo os demais limites explícitos.

## Recursos de produção no backend nativo AOT

Variáveis e constantes de nível superior, membros estáticos de classe,
construtores nomeados com lista de inicialização e `super` explícito, o operador
ternário e o apagamento de genéricos com bound nominal passaram a ter lowering
LLVM. Isso substitui as recusas anteriores desses recursos no alvo nativo; o
contrato, os símbolos emitidos e a mensagem exata de cada limite estão em
[NATIVO-PRODUCAO.md](NATIVO-PRODUCAO.md).

Estáticos e variáveis de topo são inicializados **na carga**, antes de `main`, na
mesma ordem do backend JavaScript: campos estáticos por classe em ordem de
herança, depois as variáveis de topo na ordem escrita. O Dart 3.6.2 e o 3.13.4
inicializam preguiçosamente, no primeiro acesso, então um programa cujo efeito
colateral de inicializador seja observável imprime em outra ordem nos SDKs. A
divergência é dos dois backends do DartForge juntos, está registrada em
NATIVO-PRODUCAO.md e é afirmada por teste. Ler um estático antes de seu
inicializador é recusado em compilação no alvo nativo, porque o JavaScript
emitido também falha nesses casos, com `ReferenceError`.

`static` não participa de herança nem de despacho dinâmico nos dois backends:
`C.v` resolve na declaração escrita e uma subclasse não o recebe como membro.
Fábricas nomeadas, getters estáticos, escrita em campo estático e reificação de
genéricos (`is`/`as` sobre parâmetros de tipo, funções genéricas) continuam
recusados no alvo nativo, com diagnóstico próprio.

## Genéricos reificados no JavaScript

Funções genéricas top-level admitem limites superiores e `T?`; o limite padrão
de `<T>` é `Object?`. `is`, `is!` e `as` usam descritores em execução, incluindo
tipos nominais, funções, List e Iterable. A representação preserva argumentos de
tipo em chamadas aninhadas e closures. Covariância de listas exige verificações
nas escritas, conservando o tipo real dos elementos.

Métodos genéricos e bounds recursivos permanecem fora do subconjunto. **Classes**
genéricas e herança parametrizada são aceitas com **apagamento** (erasure): `T`
resolve para o seu bound, então `C<int>` e `C<String>` compartilham uma
representação e `is C<int>` só distingue a classe crua. Os argumentos escritos são
validados em quantidade e contra o bound antes de serem apagados, de modo que
`C<String>` com `T extends int` é recusado em vez de apagado em silêncio. Contrato
e mensagens em [TIPOS.md](TIPOS.md).

O backend LLVM aceita o apagamento quando o bound é **nominal**: `T extends Base`
chega ao alvo nativo já como `Base` e vira o handle dessa classe, o que é seguro
porque nenhuma operação do subconjunto nativo observa o argumento de tipo. As
formas que o observariam — `is`, `as`, funções genéricas, chamadas com argumentos
de tipo escritos e bound `Object`/`Object?` — são recusadas em vez de apagadas,
com a mensagem exata de cada caso listada em
[NATIVO-PRODUCAO.md](NATIVO-PRODUCAO.md).
Não há reflexão `runtimeType` completa. Detalhes e referências em
[GENERICS-REIFIED-REFERENCIAS.md](GENERICS-REIFIED-REFERENCIAS.md).

## Records no JavaScript

Records posicionais/nomeados, campos imutáveis, igualdade estrutural e tipos
reificados estão integrados a genéricos e nulabilidade. Declarações var/final
podem desestruturar um record sem padrões aninhados. Veja
[contrato e limites do incremento 19](IMPLEMENTACAO-19.md). LLVM, extension types
e macros não ganham suporte por essa implementação.

## Cascatas no JavaScript

Cascatas `..` e `?..` avaliam o receptor uma vez e preservam sua identidade.
Seções admitem chamadas, seletores e atribuições simples a campos/índices.
Null na cascata `?..` impede todos os efeitos das seções. Atribuições compostas
nas seções e LLVM permanecem diagnosticados. Veja o
[contrato e a matriz de versões](IMPLEMENTACAO-20.md).

## Metaprogramação experimental

`@JsonCodable()` incorporada em Rust expande classes simples com campos escalares
em construtor, fábrica fromJson e método toJson. A geração acontece antes da
análise normal. Mapas com chaves String, acesso/atribuição por índice e fábricas
nomeadas com corpo de expressão estão disponíveis no JavaScript.
Veja [contrato, referências e limites](IMPLEMENTACAO-21.md).
A base 3.6.2 não impede recursos mais recentes; ainda não há execução de macros
arbitrárias escritas em Dart nem resolução do antigo package:json.

## Tree shaking e async

Tree shaking JavaScript opcional (`--tree-shake`), desativado por padrão.
Funções/classes inacessíveis são removidas após a análise estática completa.
O passe preserva métodos de classes vivas e descritores reificados.

Async/await, Future.value/delayed, Duration, scheduleMicrotask e Timer one-shot
possuem implementação JS. Consultar [o contrato e limites](IMPLEMENTACAO-23.md),
incluindo APIs de async ainda ausentes e rejeição explícita no backend LLVM.

Macros incorporadas possuem [fases e cache de planos](IMPLEMENTACAO-22.md).

## Parâmetros nomeados, opcionais e padrões

Funções de topo, métodos, métodos abstratos, construtores generativos e fábricas
nomeadas aceitam `{...}` de nomeados — com `required` e com valor padrão — e
`[...]` de posicionais opcionais. Um initializing formal nomeado pode ter nome
privado (`this._apiKey`, chamado por `apiKey:`), conforme Dart 3.12.

```dart
class Cliente {
  final String host;
  final int porta;
  Cliente({required this.host, this.porta = 80});
}

void registrar(String m, [int nivel = 0, String? tag]) => print('$m $nivel $tag');

void main() {
  registrar('a');
  print(Cliente(porta: 8080, host: 'b').host);
}
```

Regras: um único grupo opcional por assinatura; opcional sem padrão exige tipo
anulável; o padrão precisa ser uma constante escalar; os nomeados vêm depois dos
posicionais na chamada, em qualquer ordem entre si, e são avaliados na ordem
escrita. Closures, métodos de extension, funções genéricas, `@Native` e o backend
LLVM continuam restritos a posicionais obrigatórios, com diagnóstico próprio.

O contrato completo, a emissão JavaScript e a mensagem exata de cada limite estão
em [PARAMETROS.md](PARAMETROS.md).

## Tipos escritos, `late` e `typedef`

Anotações genéricas escritas são aceitas em toda posição, com aninhamento e
nulabilidade: `List<int>`, `Map<String, List<int>>`, `List<List<List<int>>>`,
`Set<Map<String, int>>`, `Iterable<T>`, `Future<T>` e `List<int>?`. `typedef`
funciona nas duas formas — `typedef F = int Function(int);` e
`typedef int G(int x);` — resolvendo para o tipo subjacente no parse.

`late` sem inicializador é aceito em local, campo e variável de topo. A
declaração nasce num sentinela exclusivo e a leitura antes da escrita **lança em
execução**, com a mensagem do SDK: `LateInitializationError: Local 'x' has not
been initialized.` para um local e `Field 'x' ...` para campo e variável de topo.
`late final` aceita exatamente uma atribuição e prova em execução que não houve
uma segunda. O lado direito é avaliado antes do lançamento, e o receptor de
`receptor.campo = valor` é avaliado uma única vez.

Dois limites explícitos: `late T x = init;` é **recusado**, porque em Dart o
inicializador roda na primeira leitura e uma escrita anterior o cancela — a
célula preguiçosa não é emitida; e os casos que o Dart recusa em compilação por
atribuição definida (ler um local definitivamente não escrito, reatribuir um
`late final` definitivamente atribuído) aparecem aqui em execução, não em
compilação.

A célula de `late` é emissão do backend JavaScript: LLVM AOT e os dois JIT a
recusam explicitamente (`LLVM AOT ainda não suporta late`) em vez de tratá-la
como `null`, para que os backends não discordem em silêncio.

O contrato completo, cada mensagem exata e a medição de desempenho estão em
[TIPOS.md](TIPOS.md).

## Lacunas fechadas contra o pacote `pdf` 3.13.1

Sete construções que a aferição de [CORPUS-REAL.md](CORPUS-REAL.md) contou
contra 179 arquivos de Dart de produção passaram a ser aceitas.

`const` e `static const` **sem anotação de tipo** tomam o tipo do inicializador
constante, decidido sintaticamente: literais, operadores constantes sobre eles,
literais de coleção com tipo de elemento escrito ou uniforme, invocações de
construtor e referências a `const` declarados antes. O tipo deduzido é um tipo de
verdade, não `dynamic`.

**Anotações em parâmetro** são aceitas com a mesma lista das declarações:
`Deprecated` e os metadados sem efeito semântico de `package:meta`. `@pragma`
continua com diagnóstico próprio, porque dirige o compilador e aceitá-la em
silêncio prometeria honrar uma diretiva que não é lida. `factory` também aceita
essa lista, o que substitui a recusa anterior de qualquer metadado em fábrica.

**`mixin M on Base`** é aceito: dentro do corpo os membros de `Base` estão
visíveis, `M` é subtipo de `Base` e a aplicação só vale onde `Base` está na
cadeia de superclasses — `implements Base` não satisfaz a restrição. Uma única
restrição por mixin; várias exigiriam um tipo de interseção sintetizado.

**`assert` na lista de inicialização** roda antes do corpo e antes de `super`, e
divide a lista com as entradas `campo = valor` na ordem escrita. A mensagem
diverge da do SDK — `Failed assertion: is not true.` contra a do Dart, com
arquivo, linha e texto da condição — e a asserção é sempre emitida, enquanto
`dart run` só a executa com `--enable-asserts`.

**Construtores redirecionadores** existem nas duas formas, `C.nomeado() :
this(0);` e `factory C.x() = Outra;`. Um redirecionador delega inteiramente: não
executa corpo próprio, não inicializa campo algum, não chama `super` e não aceita
formal `this.campo`. `const` redirecionador permanece recusado, e é o único
desses limites que o Dart não tem.

**Incremento e decremento em posição de expressão** — `a[i++]`, `x = y++`,
`a[--d]`, `a[e++] = 99` — valem sobre um nome simples de tipo numérico e
gravável. `a[i++]` avalia `i` uma única vez e indexa com o valor **anterior**.
Alvo composto, `final`, `late` e propriedade só com setter são recusados com a
alternativa escrita na mensagem.

O limite de complexidade de expressão foi **separado em dois**: `MAX_DEPTH`
continua em 64 e é o que protege a pilha, contando a profundidade da árvore
inclusive nas cadeias associativas à esquerda; `MAX_EXPR_NODES` é só um teto de
tamanho e subiu de 128 para 65.536, porque `const List<double>` com 255 elementos
é código real, largo e de profundidade 1.

O contrato completo, cada mensagem exata e os limites que permanecem estão em
[LACUNAS.md](LACUNAS.md).
