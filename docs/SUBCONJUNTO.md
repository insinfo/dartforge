# Subconjunto implementado

Alvo: Dart 3.6.2, igual ao SDK instalado e ao configurado no CI.

## Suporte atual

- Uma entrada `void main()` sem parâmetros.
- Funções de nível superior com retorno explícito `int`, `String`, `bool` ou `void`.
- Parâmetros posicionais obrigatórios e tipados; chamadas antecipadas e recursivas.
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
Não há interpolação, strings de aspas triplas ou strings multilinha. Um escape `\n`
dentro de uma string de uma linha pode produzir uma quebra de linha no valor.

## Tipos anuláveis e objetos

Veja os contratos de [null safety e fluxo](NULL-SAFETY.md) e [classes/herança](CLASSES.md).
A ferramenta compila [bibliotecas, reexports e filtros show/hide](MODULES.md), resolve [pacotes](PACKAGES.md) e aceita [extensions em uma unidade](EXTENSIONS.md).

## Ainda não suportado

Prefixos de import, partes, funções locais nomeadas, parâmetros nomeados ou
opcionais, overloads, `double`, genéricos definidos pelo usuário, extensions importadas, async, `for-in`, `switch`,
rótulos, interpolação, strings triplas, surrogates isolados, Set/Map, Dart `const`,
source maps e bibliotecas padrão completas. Atualizações compostas de campos, acesso a índices e atribuições
usadas como expressões também não são suportadas.
Identificadores são ASCII; alguns nomes contextuais válidos em Dart ficam reservados
neste parser. Os casos não implementados retornam erro explícito.

A HIR preserva a AST com IDs de classe e alvos de chamadas de extensions resolvidos.
Ainda não há uma IR completa com IDs para todos os locais, membros e tipos.
Números usam JavaScript Number; limitar literais a i32 não limita a faixa dos resultados.
Não há garantia de equivalência integral à VM nem implementação completa de `int` Dart.
Casos numéricos de borda são testes de regressão, não prova geral de conformidade.

O parser limita aninhamento a 64 níveis e expressões a 128 nós, inclusive argumentos de
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

## Genéricos reificados no JavaScript

Funções genéricas top-level admitem limites superiores e `T?`; o limite padrão
de `<T>` é `Object?`. `is`, `is!` e `as` usam descritores em execução, incluindo
tipos nominais, funções, List e Iterable. A representação preserva argumentos de
tipo em chamadas aninhadas e closures. Covariância de listas exige verificações
nas escritas, conservando o tipo real dos elementos.

Classes e métodos genéricos, herança parametrizada e bounds recursivos permanecem
fora do subconjunto. LLVM rejeita os novos testes/casts e genéricos explicitamente.
Não há reflexão `runtimeType` completa. Detalhes e referências em
[GENERICS-REIFIED-REFERENCIAS.md](GENERICS-REIFIED-REFERENCIAS.md).

## Records no JavaScript

Records posicionais/nomeados, campos imutáveis, igualdade estrutural e tipos
reificados estão integrados a genéricos e nulabilidade. Declarações var/final
podem desestruturar um record sem padrões aninhados. Veja
[contrato e limites do incremento 19](IMPLEMENTACAO-19.md). LLVM, extension types
e macros não ganham suporte por essa implementação.
