# JS de produção: velocidade contra o `dart2js`

Rodada de 2026-10-28, noite (macOS). O `-O4` do `dartforge-jsprod` já saía
cerca de 8% menor que o `dart2js -O4` no corpus
(`docs/JS-PRODUCAO-TAMANHO.md` §21), mas ninguém tinha medido o **tempo de
execução**. Esta rodada mede, acha onde somos mais lentos e corrige o que dá
para corrigir sem crescer o arquivo de forma perceptível.

## 1. Método

* Máquina: Mac mini arm64 (10 núcleos), compartilhada com outros agentes —
  a carga variou de 3 a 60 durante a rodada.
* Node 24.21 (`/Volumes/ssd_dock/tools/rea/node-v24.21.0-darwin-arm64`) e
  Chrome headless (protocolo de depuração, `console` capturado) como
  segundo motor.
* `dart2js` do SDK 3.6.2 (`dart compile js -O4`, e o nível padrão, `-O1`,
  para comparar com o nosso `-O2`).
* Programas: `bench/performance/*.dart`, `bench/comparison/{bench,
  json_encode,nullable_equality,color_numeric,async_await}.dart` (o
  `trees` das rodadas de ARC é o de `bench/comparison/bench.dart`; outro
  `trees` está em `escaping_objects.dart`). Variantes para a web (no
  *scratchpad*, não versionadas): `collections.dart` e `color_numeric.dart`
  usam inteiros de 64 bits que o `dart2js` recusa (constantes trocadas por
  2^53 − 1 e `1 << 30`); o `main(List<String>)` do `json_encode.dart` vira
  `main()` (ver §6). `string_buffer.dart` usa `Isolate` e não roda na web
  (nem no `dart2js`).
* Tempo: o `Stopwatch` de cada programa, com `performance.now()` trocado,
  no Node, pelo tempo de CPU da *thread* principal
  (`process.threadCpuUsage()`; `window` definido para que os dois runtimes
  usem `performance.now`). Com a máquina carregada o relógio de parede
  variava 10×; o tempo de CPU da *thread*, bem menos. Programas sem
  cronômetro (`nullable_equality`, `color_numeric`): tempo de CPU do
  processo. No Chrome, relógio de parede (`performance.now`).
* Cada número é a mediana de 5 execuções (Node `-O4`), 3 (Node `-O2`,
  Chrome); em cada execução, a mediana das rodadas do kernel sem a
  primeira. As execuções dos três arquivos se alternam.
* A saída de cada programa (sem os tempos) é comparada entre os arquivos
  antes de medir. Diferem só os de semântica da web: `fresh_list_append`
  e `color_numeric` (`0.0 is int` é verdadeiro na web; o nosso e o
  `dart2js` divergem da VM em linhas diferentes, e o nosso não mudou
  nesta rodada).
* Perfis: `node --cpu-prof` no arquivo `--no-minify` (nomes legíveis) e no
  minificado (posição → trecho do código).

## 2. Onde era mais lento, e por quê

A medida inicial (`-O4`, Node) tinha 20 kernels acima de 1,2× o `dart2js`.
Os perfis apontaram sete causas:

1. **Protótipos de `Number` e `String` em modo dicionário.** O
   `_installProperties` do runtime copiava os membros de `Object` para o
   protótipo JavaScript e depois redefinia os sobrescritos (`toString`, o
   *getter* `runtimeType`) com outros atributos. O V8 transforma em
   dicionário um protótipo cuja propriedade é reconfigurada, e só um
   *inline cache* com receptor objeto o devolve ao modo rápido — os
   receptores de `Number.prototype` e `String.prototype` são primitivos.
   Toda chamada de método de extensão em número ou *string*
   (`i[$toDouble]()`, `s[$isEmpty]`, `n[$modulo](m)`) virava busca lenta:
   `point_sum` 10× mais lento, `strings/hashes` 3×, `json/utf8_bytes` 4×.
2. **`t$R("receita")` reavaliado a cada uso.** O `_Universe.eval` passa por
   quatro chamadas até o `Map` que guarda a rti canônica; as receitas do
   runtime estão em caminhos quentes (cada `Future`, cada `await`, os
   `is`/`as` do SDK, o `sort` genérico). No `async_await`, 17% do tempo.
3. **`async` sem `await` passando por gerador.** Dois terços das chamadas
   do `async_await` são `leaf`, que não espera nada, e mesmo assim criava o
   gerador e o passo `next()`/`done`.
4. **`for-in` por `for…of`.** O protocolo de iterador do JavaScript sobre um
   *array* é várias vezes mais lento que o laço por índice que o `dart2js`
   escreve.
5. **`length`, `[]` e `[]=` por métodos.** `l[$length]`, `l[$_get](i)` e
   `l[$_set](i, v)` mesmo quando o mundo prova que o receptor é um *array*
   (§21.4 da especificação de tamanho já fazia a leitura em `-O4`).
6. **`null` antes dos campos de construtores `const`.** O construtor
   `const` gravava `null` em todos os campos (a ordem das propriedades é a
   da canonicalização); o V8 dá ao campo que nasce `null` a representação
   geral, e cada `double` gravado depois vira um objeto: o `Point`
   temporário de `temporary_objects` alocava um número por campo.
7. **Chamadas de identidade e máscaras.** `i.toDouble()` era uma chamada de
   método num número, e `x & 0xFF` levava o `>>> 0` do `JSNumber.&` mesmo
   com resultado já não negativo.

## 3. O que mudou

Todas as formas valem só na produção; o desenvolvimento (contrato do DDC)
não muda. Cada uma tem a sua chave de diagnóstico (`=0` desliga).

| mudança | onde | chave | níveis |
| --- | --- | --- | --- |
| membros de extensão reunidos num objeto novo e definidos **uma vez** no protótipo nativo (remendo do `_applyExtension`) | `emit_js/src/sdk_patches.rs` | `DARTFORGE_JSPROD_FAST_PROTOTYPES` | todos |
| `t$R` guarda o que avaliou num `Map` próprio (mesmos objetos canônicos, uma busca) | `emit_js_production/src/own_sdk.rs` | `DARTFORGE_JSPROD_RECIPE_CACHE` | todos |
| corpo `async` sem `yield` no texto vira função comum, chamada uma vez no mesmo passo registrado na zona (`_wrapJsFunctionForAsync`), com o mesmo *completer* e os mesmos `_asyncStartSync`/`_asyncReturn`/`_asyncRethrow` | `emit_js/src/body.rs` | `DARTFORGE_JSPROD_SYNC_ASYNC_BODIES` | todos |
| `length`/`isEmpty`/`isNotEmpty` de receptor provado primitiva indexável → `.length` | `emit_js/src/expr.rs` | `DARTFORGE_JSPROD_NATIVE_LENGTH` | todos |
| `for (x in l)` com `l` provado *array* (ou *typed array*) → laço por índice, lendo `length` a cada passo como o `for…of` que substitui | `emit_js/src/body.rs` | `DARTFORGE_JSPROD_INDEXED_LOOPS` | todos |
| `r[i]` checado com receptor e índice sem efeitos → `(r[i] ?? r[$_get](i))`: o método só para `undefined`/`null` (fora da faixa, onde lança o `RangeError`; ou elemento `null`, que devolve de novo) | `emit_js/src/expr.rs` | `DARTFORGE_JSPROD_CHECKED_READS` | `-O1`–`-O3` |
| `r[i] = v` com faixa confiada e sem checagem de parâmetro no SDK → escrita JavaScript; num `JSArray`, antes, a checagem de lista imutável (`immutable$list`, pelo próprio `checkMutable`); índice ou valor com efeitos vão a temporários na ordem do Dart | `emit_js/src/expr.rs` | `DARTFORGE_JSPROD_INDEX_SET` | `-O4` |
| `n.toDouble()` (número não anulável) e `i.toInt()` (`int`) → `n` | `emit_js/src/call.rs` | `DARTFORGE_JSPROD_NUMBER_IDENTITY` | todos |
| `x & k` com `k` literal de 0 a 2^31 − 1 sem `>>> 0` | `emit_js/src/expr.rs` | `DARTFORGE_JSPROD_MASK` | todos |
| construtor `const` cujas gravações já vêm na ordem da declaração (campos com inicializador, `this.x`, lista de inicializadores) sem os `null` iniciais; os outros os mantêm | `emit_js/src/module.rs` | `DARTFORGE_JSPROD_CONST_ORDER` | todos |

O filtro do mundo (`emit_js_production/src/filter.rs`) ganhou
`typed_array_class` (todas as instâncias abaixo da classe são *typed
arrays*: a escrita não precisa da checagem de imutabilidade).

Defeito corrigido no caminho: `parity_get` escrevia `(i & 1) === 0` com a
precedência da igualdade, mas uma condição sai do `emit_cond` como texto e
o `!` a toma como primária — `!i.isEven` virava `!(i & 1) === 0`, sempre
falso, em todos os níveis da produção. As formas de paridade (e as novas
de `isEmpty`) saem entre parênteses.

Testes: `emit_js_production/tests/native_members.rs` roda
`emit_js/tests/programs/native_members.dart` (cada forma acima, inclusive
escrita em lista `const`, ordem de avaliação de índice e valor, `async`
sem `await` com erro, zona e `this`) em `-O1`, `-O2` e `-O4`, com todas as
chaves ligadas e desligadas, contra a VM.

Nota de histórico: o *index* do repositório é compartilhado com outros
agentes, e o commit `d69f5cbd` (“Give the lean area test the descriptor
the runtime reads”), feito em paralelo, levou as mudanças de `length`,
`for-in` por índice e leitura/escrita indexada que estavam preparadas para
o commit desta série; o commit seguinte (`5f8b674a`) registra isso.

## 4. Resultado

### 4.1 Node, `-O4` contra `dart2js -O4` (ms; mediana de 5)

| kernel | antes | depois | `dart2js -O4` | depois/`dart2js` |
| --- | ---: | ---: | ---: | ---: |
| `calls/shapes` | 2,5 | 1,4 | 3,9 | 0,36 |
| `calls/closures` | 6,7 | 6,0 | 4,8 | 1,25 |
| `calls/fib` | 5,1 | 4,8 | 4,5 | 1,07 |
| `escaping_objects/trees` | 16,4 | 16,5 | 16,4 | 1,01 |
| `escaping_objects/linked_list` | 8,9 | 8,9 | 8,7 | 1,02 |
| `fresh_list_append` | 25,6 | 25,2 | 24,7 | 1,02 |
| `json/decode_small` | 12,7 | 11,3 | 9,3 | 1,22 |
| `json/encode_small` | 22,1 | 20,6 | 14,3 | 1,44 |
| `json/decode_medium` | 16,9 | 14,3 | 12,8 | 1,12 |
| `json/encode_medium` | 33,5 | 31,5 | 23,6 | 1,33 |
| `json/decode_large` | 35,4 | 30,0 | 30,2 | 0,99 |
| `json/encode_large` | 68,6 | 65,7 | 50,3 | 1,31 |
| `json/indented` | 32,8 | 32,0 | 27,0 | 1,19 |
| `json/utf8_bytes` | 81,1 | 29,4 | 21,5 | 1,37 |
| `json/reviver` | 13,1 | 11,0 | 9,1 | 1,21 |
| `numeric/mandelbrot` | 5,1 | 5,5 | 5,4 | 1,02 |
| `numeric/collatz` | 187,3 | 192,9 | 127,7 | 1,51 |
| `objects_in_collections/object_list` | 7,1 | 5,0 | 3,5 | 1,43 |
| `objects_in_collections/object_sort` | 21,3 | 14,9 | 15,3 | 0,97 |
| `objects_in_collections/map_int_object` | 12,2 | 11,0 | 4,5 | 2,44 |
| `objects_in_collections/map_str_object` | 4,6 | 3,3 | 3,5 | 0,94 |
| `strings/build` | 16,0 | 14,2 | 12,4 | 1,15 |
| `strings/hashes` | 33,1 | 11,0 | 10,6 | 1,04 |
| `temporary_objects/points` | 21,2 | 11,4 | 9,0 | 1,27 |
| `temporary_objects/point_sum` | 33,6 | 3,4 | 3,5 | 0,97 |
| `typed_data/product_f64` | 3,5 | 3,5 | 3,4 | 1,03 |
| `typed_data/fnv_bytes` | 54,7 | 54,7 | 55,2 | 0,99 |
| `typed_data_widths/mixed_f32_i16` | 6,8 | 6,8 | 6,9 | 0,99 |
| `typed_data_widths/sum_i32` | 6,7 | 6,7 | 6,8 | 0,99 |
| `collections/sieve` | 21,9 | 7,9 | 8,5 | 0,93 |
| `collections/list_add` | 4,3 | 4,3 | 5,0 | 0,86 |
| `collections/list_read` | 6,8 | 6,8 | 6,8 | 1,00 |
| `collections/list_sort` | 578,0 | 324,1 | 138,6 | 2,34 |
| `collections/map` | 31,1 | 27,4 | 28,6 | 0,96 |
| `collections/set_str` | 9,0 | 9,1 | 16,4 | 0,55 |
| `bench/numeric` | 1408 | 1379 | 1184 | 1,16 |
| `bench/strings` | 41 | 40 | 64 | 0,63 |
| `bench/json` | 76 | 82 | 70 | 1,17 |
| `bench/fib`, `bench/trees` | 12, 17 | 12, 17 | 11, 17 | ≈1 |
| `async_await` | 2132 | 1154 | 250 | 4,63 |
| `json_encode/ascii_json` (µs/op) | 30,3 | 28,8 | 18,4 | 1,57 |
| `json_encode/latin1_json` (µs/op) | 29,9 | 27,9 | 18,2 | 1,53 |
| `json_encode/mixed_utf16_json` (µs/op) | 45,6 | 43,2 | 33,6 | 1,29 |
| `json_encode/*_string_buffer` | ≈ | ≈ | ≈ | ≈1 |
| `nullable_equality`, `color_numeric` (CPU do processo) | 26,5, 46,1 | 27,6, 50,1 | 25,6, 46,1 | ≈1 |

(`bench.dart` mede com `elapsedMilliseconds`: resolução de 1 ms.)

### 4.2 Chrome headless, `-O4` contra `dart2js -O4` (ms; mediana de 3)

| kernel | antes | depois | `dart2js -O4` |
| --- | ---: | ---: | ---: |
| `calls/shapes` / `closures` / `fib` | 2,2 / 6,6 / 5,1 | 1,4 / 5,4 / 5,1 | 1,4 / 3,4 / 5,1 |
| `temporary_objects/points` / `point_sum` | 16,5 / 29,2 | 10,2 / 3,4 | 9,0 / 3,4 |
| `objects_in_collections/object_list` / `object_sort` | 6,7 / 19,4 | 5,4 / 14,7 | 5,9 / 15,4 |
| `objects_in_collections/map_int_object` / `map_str_object` | 10,4 / 4,8 | 9,6 / 2,7 | 2,7 / 3,2 |
| `collections/sieve` / `list_sort` | 21,6 / 460,5 | 7,0 / 284,4 | 7,5 / 120,8 |
| `json/encode_small` / `decode_large` / `utf8_bytes` | 12,5 / 25,6 / 82,7 | 11,2 / 21,6 / 21,9 | 6,8 / 24,2 / 16,8 |
| `strings/build` / `hashes` | 14,6 / 28,1 | 12,9 / 10,2 | 12,6 / 9,8 |
| `async_await` | 1736 | 1034 | 224 |
| `bench/numeric` / `strings` / `json` | 335 / 53 / 76 | 335 / 51 / 76 | 305 / 44 / 70 |

O Chrome confirma o Node: os mesmos kernels melhoraram e os mesmos
continuam atrás.

### 4.3 Node, nosso `-O2` contra o `dart2js` padrão (`-O1`)

Mediana de 3; a carga da máquina subiu durante esta medida (o
`collections/map` variou 2× entre execuções do mesmo arquivo), então só as
diferenças grandes contam.

| kernel | antes | depois | `dart2js` (`-O1`) | depois/`dart2js` |
| --- | ---: | ---: | ---: | ---: |
| `calls/shapes` | 2,6 | 1,6 | 9,1 | 0,18 |
| `json/decode_medium` | 29,0 | 21,5 | 23,6 | 0,91 |
| `json/encode_medium` | 62,8 | 51,8 | 43,5 | 1,19 |
| `json/utf8_bytes` | 146,1 | 57,4 | 39,0 | 1,47 |
| `objects_in_collections/object_list` | 11,8 | 8,8 | 5,6 | 1,57 |
| `objects_in_collections/object_sort` | 81,8 | 74,7 | 31,1 | 2,40 |
| `strings/hashes` | 34,6 | 11,3 | 10,8 | 1,05 |
| `temporary_objects/points` | 21,2 | 11,4 | 9,0 | 1,27 |
| `temporary_objects/point_sum` | 34,8 | 3,4 | 3,4 | 1,00 |
| `collections/sieve` | 67,5 | 69,2 | 49,7 | 1,39 |
| `collections/list_add` | 18,5 | 19,3 | 12,5 | 1,54 |
| `collections/list_read` | 29,7 | 6,9 | 7,0 | 0,99 |
| `collections/list_sort` | 843,7 | 723,1 | 374,9 | 1,93 |
| `async_await` | 3460 | 2302 | 1076 | 2,14 |
| `json_encode/mixed_utf16_json` (µs/op) | 210,2 | 82,3 | 50,9 | 1,62 |

Em `-O2` ficam o `[]=` e o `add` checados no método (§7).

## 5. Tamanho

Corpus `corpus/js` (273 programas), como na §21.7 da especificação de
tamanho (sem `--enable-asserts`, `gzip -9`):

| nível | antes | depois | diferença | `dart2js` do nível | depois/`dart2js` |
| --- | ---: | ---: | ---: | ---: | ---: |
| `-O2` | 10.305.846 (gz 3.492.719) | 10.362.665 (gz 3.509.656) | +56.819 (+0,55%; gz +0,48%) | 12.416.485 (gz 4.047.873) | 0,835× (gz 0,867×) |
| `-O4` | 10.175.736 (gz 3.457.004) | 10.241.372 (gz 3.475.268) | +65.636 (+0,65%; gz +0,53%) | 11.024.980 (gz 3.582.721) | 0,929× (gz 0,970×) |

Por mudança (`-O4`, com `--enable-asserts`, chave desligada contra todas
ligadas): `length` nativo +20 KB (gz −1,5 KB: `.length` repete mais que os
símbolos minificados), `for-in` por índice +21 KB (gz +7,7 KB), escrita
indexada +21 KB (gz +6,2 KB); o `async` sem gerador, as identidades, as
máscaras e a ordem dos `const` encolhem. Os projetos (`limitless_ui`,
`new_sali`) não foram medidos nesta rodada.

## 6. Medidos e descartados

* **`add` de `JSArray` como `push` no lugar** (`fixed$length` e o
  `checkGrowable` do próprio `JSArray`, `-O3` em diante): depois dos
  protótipos rápidos, `list_add` e `object_list` não mudaram além do
  ruído, e o corpus crescia 50 KB (o SDK tem muitos `add`). Fora.
* **Tabela preguiçosa de receitas** (`t$E(k)` com a receita numa tabela e
  o resultado num *array*): tão rápida quanto o cache em `Map`, mas o gzip
  do corpus crescia 0,6% (os índices comprimem pior que as receitas
  repetidas). Trocada pelo cache dentro do `t$R`.
* **Ordenação pelo `Array.prototype.sort`** (o que o `dart2js` 3.6 faz no
  `JSArray.sort`): é estável e o `Sort.sort` do Dart não é; com elementos
  iguais distinguíveis a saída deixaria de ser a da VM em `-O1`/`-O2`.
  Não tentado.

## 7. O que continua mais lento, e por quê

* **`async_await` (4,6×).** O contrato do DDC compila `async` com geradores
  JavaScript (`function*` e um passo `next`/`throw` por `await`); o
  `dart2js` escreve uma máquina de estados com `switch`. Sobra o custo do
  gerador em cada `await`, o GC das molduras dos geradores (19% do tempo) e
  as avaliações de rti por ambiente (`T[_eval]("…")`, um `Map` por rti) do
  `_Future`. O próximo passo seria a máquina de estados na produção.
* **`list_sort` (2,3×).** O `JSArray.sort` do DDC é o `Sort.sort` do Dart
  (*quicksort* de dois pivôs em Dart), com `Comparable.compare(a as
  Comparable, b as Comparable)` checado a cada comparação; o do `dart2js`
  chama o `Array.prototype.sort` nativo com o comparador. Ver §6.
* **`map_int_object` (2,4×).** O `IdentityMap`/`LinkedMap` do runtime
  guardam em `Map` do JavaScript; o `JsLinkedHashMap` do `dart2js` guarda
  chaves inteiras num objeto (`_nums`) com células encadeadas.
* **`collatz` (1,5×).** `x >> 1` com `x` de sinal desconhecido é
  `x > 0 ? x >>> 1 : x >> 1 >>> 0` (`dart.shrCount`); a inferência do
  `dart2js` prova `x` não negativo e escreve `x >>> 1`. Precisaria de uma
  análise de faixa por variável local.
* **JSON (codificar 1,3–1,6×, decodificar 1,0–1,2×).** O `_JsonStringifier`
  do runtime aloca um `List.filled` por mapa (com a rti do ambiente
  avaliada a cada vez) e escreve no `StringBuffer` por chamadas de método;
  o do `dart2js` é o mesmo código, mas com *inlining* e a inferência de
  tipos que o nosso mundo fechado não tem.
* **`closures` (1,25×).** Uma *closure* Dart aqui é uma função JavaScript
  com a rti numa propriedade; no `dart2js`, um objeto de classe com
  `call$1`, que o V8 trata como chamada monomórfica.
* **`-O2`**: as leituras indexadas checadas ficaram no `??`, mas o `add` e o
  `[]=` de `JSArray` continuam com a checagem de covariância do elemento
  no próprio método (`instanceType(this)[_eval]("…")[_as](v)`): o contrato
  do DDC checa no chamado, e o `dart2js` só no chamador quando o tipo
  estático não é exato.

Defeito encontrado e **não** corrigido: o `main(List<String> args)` recebe
`undefined` nos dois perfis (`m.main()` sem argumentos, `bundle.rs` e
`module.rs`); a VM e o `dart2js` passam uma lista vazia. O
`bench/comparison/json_encode.dart` cai em `for (… in arguments)`.

## 8. Validação

* `dartforge-differential --production` em `corpus/js`: a cada commit,
  274/274 em `-O2` e 254/254 em `-O4` (20 excluídos, nenhum passando);
  nos três últimos estados e no final também 274/274 em `-O1` e 258/258
  em `-O3` (16 excluídos); desenvolvimento 274/274; `corpus/modern` 26/26 em
  produção (`-O2` e `-O4`; desenvolvimento 24/26, os mesmos dois de antes,
  340 e 341, rejeitados pelo *frontend*).
* Testes de `emit_js` e `emit_js_production`, inclusive os ignorados, com
  Node, Dart e `DARTFORGE_SDK_LIB`: só o `tests/identity.rs` de
  `p3_classes` falha, como antes; o novo `native_members` passa.
* `clippy -D warnings` e `rustfmt` nos arquivos tocados. O `crates/emit_js`
  mudou só em caminhos guardados por `ctx.production()` e no remendo do SDK
  de produção: o texto do perfil de desenvolvimento é o mesmo.
* Cada commit foi validado numa exportação limpa (`git archive` do HEAD
  mais o *patch*) com `CARGO_TARGET_DIR` próprio.
