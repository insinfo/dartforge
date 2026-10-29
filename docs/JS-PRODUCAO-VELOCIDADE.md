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
  `_Future`. A máquina de estados veio na rodada seguinte (§9.2): 2,2×;
  os caminhos do runtime, na terceira (§10.2): ~1,4×.
* **`list_sort` (2,3×).** O `JSArray.sort` do DDC é o `Sort.sort` do Dart
  (*quicksort* de dois pivôs em Dart), com `Comparable.compare(a as
  Comparable, b as Comparable)` checado a cada comparação; o do `dart2js`
  chama o `Array.prototype.sort` nativo com o comparador. Ver §6.
* **`map_int_object` (2,4×).** O `IdentityMap`/`LinkedMap` do runtime
  guardam em `Map` do JavaScript; o `JsLinkedHashMap` do `dart2js` guarda
  chaves inteiras num objeto (`_nums`) com células encadeadas. Medido na
  §9.5 e feito na §10.1.
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

Defeito encontrado e **não** corrigido nesta rodada: o `main(List<String>
args)` recebia `undefined` nos dois perfis (`m.main()` sem argumentos,
`bundle.rs` e `module.rs`); a VM e o `dart2js` passam uma lista vazia. O
`bench/comparison/json_encode.dart` caía em `for (… in arguments)`.
Corrigido na rodada seguinte (§9.1).

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

## 9. Segunda rodada: `main(args)` e `async` como máquina de estados

Rodada de 2026-10-28, fim da tarde. Mesma máquina, mesmos motores e o mesmo
método da §1 (mediana de 5 execuções no Node `-O4`, 3 no Chrome e no
`-O2`); a carga da máquina ficou entre 3 e 6. “Antes” é o `fb01240b`, o fim
da §3.

### 9.1 `main(List<String> args)`

O `dart2js` 3.6 chama `callMain([])`, e o `callMain` passa
`convertMainArgumentList(args)`: um `JSArray<String>` vazio e crescível —
**não** lê `process.argv` nem sob o Node (só um `dartMainRunner` global
mudaria isso). Um segundo parâmetro posicional recebe `undefined`, lido
como `null`. A VM sem argumentos passa uma lista vazia (não modificável).

Agora a entrada passa `_interceptors.JSArray.of(<rti de JSArray<String>>,
[])` e, se `main` tem dois parâmetros posicionais, `null` no segundo
(`emit_js/src/module.rs`, `main_arguments`; a produção lê a chamada do
`main.mjs`, `bundle::entry_from_mjs`). Vale nos dois perfis: no de
desenvolvimento muda só o `main.mjs` de um `main` com parâmetros, que
passa a importar `_interceptors` e `dart_rti`. O
`bench/comparison/json_encode.dart` roda sem modificação nos dois.

Testes: `emit_js/tests/programs/main_arguments.dart` e
`main_arguments_message.dart`, contra a VM no desenvolvimento
(`tests/basic.rs`) e em `-O1`, `-O2` e `-O4`
(`emit_js_production/tests/main_arguments.rs`).

### 9.2 `async` como máquina de estados

`emit_js/src/async_machine.rs`, só na produção, chave
`DARTFORGE_JSPROD_ASYNC_MACHINE` (`=0` volta ao gerador;
`DARTFORGE_JSPROD_ASYNC_MACHINE_DEBUG=1` diz quais funções ficaram com o
gerador e por quê).

O emissor continua escrevendo o corpo do gerador (cada `await` é um
`yield`); o texto é analisado pelo `oxc` (com o `oxc_semantic` para os
escopos) e reescrito por emenda, como o `dart2js` escreve o seu:

```js
mid(i) {
  let t$completer = async._makeAsyncAwaitCompleter(T), t$goto = 0, a, b;
  let t$body = async._wrapJsFunctionForAsync((t$code, t$res) => {
    if (t$code === 1) return async._asyncRethrow(t$res, t$completer);
    switch (t$goto) {
      case 0: t$goto = 1; return async._asyncAwait(leaf(i), t$body);
      case 1: a = t$res; t$goto = 2; return async._asyncAwait(leaf(i + 1), t$body);
      case 2: b = t$res; return async._asyncReturn(a + b, t$completer);
    }
  });
  return async._asyncStartSync(t$body, t$completer);
}
```

* **Os mesmos auxiliares do runtime**, na mesma ordem: o mesmo *completer*,
  o mesmo passo registrado na zona (`_wrapJsFunctionForAsync`), o mesmo
  `_asyncStartSync` (o primeiro passo síncrono, o `isSync` depois dele), o
  mesmo `_asyncAwait` (um `then` por `await`, o `await` de um não-*future*
  por um `_Future` já completo), o mesmo `_asyncReturn` (completa síncrono
  depois do primeiro `await`, como o `dart2js`). A ordem do laço de eventos
  é a do gerador, que já era a da VM: some só o objeto gerador, os
  resultados `{value, done}` e as molduras.
* **Código sem `yield` é copiado como está**, renomeando só as variáveis
  içadas; `return`, `break` e `continue` que saem da parte copiada viram
  saltos (`t$goto = N; break;`, ou `continue t$sm` de dentro de um laço ou
  `switch` copiado).
* **As declarações dos blocos que têm `yield` sobem para a função** (têm
  de sobreviver aos passos), renomeadas se o nome já existe (parâmetro,
  outra variável, nome livre); uma sem inicializador dentro de um laço
  volta a `undefined` a cada volta.
* **Ordem de avaliação**: numa expressão com `await`, o que é avaliado antes
  dele vai a um temporário, a menos que nada possa mudá-lo — literal,
  `this`, variável nunca reatribuída, nome de módulo, a rti de uma receita;
  um local de uma função envolvente (que uma *closure* compartilha) é
  sempre guardado. `x + await f()` lê `x` antes, como a VM.
* **Controle**: laços, `if`, `switch` (os testes em sequência, com `===`),
  rótulos, `&&`/`||`/`??` e `?:` com `await` viram estados. `try`/`catch`/
  `finally` seguem o esquema do `dart2js`: `t$handler` é o estado do
  tratador corrente (um erro lançado num passo volta pelo laço protegido do
  `_wrapJsFunctionForAsync` com o código 1 e vai a ele), e um `finally`
  continua nos estados empilhados em `t$next` (o fim normal, um `return`,
  um `break` que o atravessa, ou o relançamento do erro pendente).
* **A IIFE do emissor dentro de um gerador**
  (`yield* (function*() {…}).call(this)`, usada por coleções com `for`/`if`)
  é embutida: o seu `return` vira atribuição e salto.
* **Recusas** (fica o gerador): uma variável de laço capturada por uma
  *closure* (perderia a ligação por iteração), declaração por padrão,
  outro `yield*`, `yield` em *template*, em chave computada, em `delete`,
  em atribuição lógica ou em chamada opcional, `for-in` com `yield`,
  `arguments`, `new.target`, `return`/`break` que saem de um corpo de
  `finally` com estados.

Cobertura: no corpus, todos os corpos `async` com `await` viram máquina
(no programa de teste, só o da variável de laço capturada fica com o
gerador); no `new_sali`, 1.067 viram máquina e sobram os geradores de
`async*`/`sync*`. `async*` e `sync*` continuam com
o gerador: têm outro protocolo (`_asyncStarHelper`, `_IterationMarker`) e
nenhum dos *kernels* depende deles.

Testes: `emit_js/tests/programs/async_state_machine.dart` (laços,
`switch`, rótulos, `try`/`catch`/`finally` com `await` em cada parte,
`return` e `break` através de `finally`, erro antes do primeiro `await`,
`rethrow`, erro de `finally` substituindo o pendente, ordem de avaliação,
*closures*, parâmetro capturado mudado durante o `await`, microtarefas,
*timers*, `then`, `Completer`, `Future.wait`, `async*`, zona) contra a VM
no desenvolvimento e em `-O1`, `-O2`, `-O4`, com a chave ligada e
desligada (`emit_js_production/tests/async_state_machine.rs`); testes de
unidade da reescrita no próprio módulo.

### 9.3 As receitas repetidas do runtime avaliadas uma vez

Com a máquina de estados, o perfil do `async_await` passou a ter o `t$R`
no topo (15%): cada `await` avalia ~4 receitas do runtime (`~(@)`,
`|b(@,|B)`, `|X<@>`… no `_awaitOnObject`, no `_wrapJsFunctionForAsync`, nos
ouvintes do `_Future`), cada uma uma busca no `Map` do §3. As receitas
repetidas **do programa** já eram variáveis declaradas logo depois do
runtime (`shapes.rs`); agora as chamadas **dentro de funções do runtime**
também contam: uma função roda depois de o arquivo carregar. O corpo de
uma IIFE roda enquanto o runtime carrega e mantém o `t$R`. Chave
`DARTFORGE_JSPROD_SDK_FIXED_RECIPES`; vale onde o arquivo é minificado
(`-O2` em diante).

O efeito vai além do `async`: o `Sort.sort` (`Comparable.compare(a as
Comparable, b as Comparable)`) e a codificação de JSON avaliavam receitas
a cada passo.

### 9.4 Resultado

Node, `-O4` contra `dart2js -O4` (ms; mediana de 5). “Máquina” é o commit
da §9.2; “receitas”, o da §9.3:

| kernel | antes | máquina | receitas | `dart2js -O4` | receitas/`dart2js` |
| --- | ---: | ---: | ---: | ---: | ---: |
| `async_await` | 1165,5 | 672,0 | 538,5 | 245,0 | 2,20 |
| `collections/list_sort` | 321,6 | 316,3 | 225,5 | 120,0 | 1,88 |
| `json/encode_small` | 20,8 | 20,7 | 19,2 | 14,2 | 1,35 |
| `json/encode_large` | 64,4 | 65,3 | 61,3 | 49,9 | 1,23 |
| `json/decode_large` | 29,8 | 29,9 | 27,7 | 29,6 | 0,94 |
| `json/utf8_bytes` | 29,2 | 29,1 | 26,7 | 21,3 | 1,25 |
| `json_encode/ascii_json` (µs/op) | 27,9 | 27,9 | 25,3 | 17,8 | 1,42 |
| `json_encode/mixed_utf16_json` (µs/op) | 43,0 | 42,9 | 40,4 | 33,2 | 1,22 |
| `objects_in_collections/map_int_object` | 10,8 | 11,1 | 10,7 | 3,7 | 2,89 |

Os demais *kernels* da §4.1 ficaram no ruído (±5%). O `json_encode` desta
tabela ainda é a variante com `main()` (a mesma do §4, para comparar); o
original agora roda igual.

Chrome headless, `-O4` (ms; mediana de 3):

| kernel | antes | máquina | receitas | `dart2js -O4` |
| --- | ---: | ---: | ---: | ---: |
| `async_await` | 1020,5 | 597,0 | 522,0 | 226,0 |
| `collections/list_sort` | 283,9 | 283,3 | 213,5 | 128,3 |
| `json/encode_small` / `encode_large` | 11,2 / 62,1 | 11,1 / 60,8 | 10,2 / 59,6 | 6,8 / 48,3 |
| `json/utf8_bytes` | 22,2 | 21,8 | 20,8 | 16,8 |
| `objects_in_collections/map_int_object` | 9,6 | 9,4 | 9,5 | 2,7 |

Node, nosso `-O2` contra o `dart2js` padrão (`-O1`) (ms; mediana de 3):

| kernel | antes | depois | `dart2js` (`-O1`) | depois/`dart2js` |
| --- | ---: | ---: | ---: | ---: |
| `async_await` | 1500,5 | 845,0 | 754,5 | 1,12 |
| `collections/list_sort` | 535,6 | 404,7 | 224,2 | 1,81 |
| `collections/sieve` | 73,8 | 46,5 | 33,0 | 1,41 |
| `collections/list_add` | 19,7 | 12,0 | 9,1 | 1,32 |
| `json/encode_medium` | 35,2 | 32,4 | 26,1 | 1,24 |
| `json/utf8_bytes` | 35,1 | 32,0 | 24,6 | 1,30 |

(No `-O2` o `[]=` e o `add` de `JSArray` checam a covariância com uma
receita do runtime: o `sieve` e o `list_add` ganham com a §9.3.)

### 9.5 `map_int_object`: medido, não feito

O *kernel* faz 200 mil `m[k] = v` e 200 mil `m[i]` num `Map<int, Item>`.
O nosso `IdentityMap` guarda num `Map` do JavaScript; o `JsLinkedHashMap`
do `dart2js` guarda as chaves inteiras num objeto (`_nums`) apontando para
células encadeadas (a ordem de inserção fica na lista das células). No
mesmo padrão de acesso, em JavaScript puro (Node, ms):

| representação | ms |
| --- | ---: |
| `Map` (a nossa) | 10,3 |
| `Map` para a ordem + objeto `_nums` para a busca | 8,9 |
| objeto `_nums` + lista de células (a do `dart2js`) | 3,7 |

Só a representação do `dart2js` paga, e ela troca o `IdentityMap` inteiro:
iteração pelas células (as chaves de um objeto iteram em ordem numérica,
não na de inserção), `remove`, `clear`, `putIfAbsent`, os iteráveis de
`keys`/`values` e a checagem de modificação concorrente — todo mapa de
chave primitiva do programa passa por ela. Não coube nesta rodada; é o
próximo passo para este *kernel*.

### 9.6 Tamanho

Corpus `corpus/js`, como na §5 (`gzip -9`):

| nível | antes | máquina | receitas | `dart2js` do nível | depois/`dart2js` |
| --- | ---: | ---: | ---: | ---: | ---: |
| `-O2` | 10.362.665 (gz 3.509.656) | 10.365.671 (gz 3.512.315) | 10.341.152 (gz 3.519.114) | 12.416.485 (gz 4.047.873) | 0,833× (gz 0,869×) |
| `-O4` | 10.241.372 (gz 3.475.268) | 10.244.385 (gz 3.478.020) | 10.224.278 (gz 3.485.084) | 11.024.980 (gz 3.582.721) | 0,927× (gz 0,973×) |

A máquina custa 3 KB no corpus (gz +2,7 KB): poucos programas têm `async`
com `await`, e o texto da máquina substitui o do gerador. As receitas
encolhem o arquivo (−20 a −25 KB) e crescem o gzip (+7 KB, +0,2%): o nome
curto da variável comprime pior que a receita repetida.

Projetos (`-O2`, o padrão): `limitless_ui` 3.354.408 → 3.349.301 (gz
942.210 → 944.878); `new_sali` 7.155.083 → 7.178.083 (gz 1.912.077 →
1.937.907, +1,4%): é o projeto com mais `async` (1.067 funções), e cada
`await` custa ~18 bytes a mais minificado (`n=7;return Rr(x,a);case 7:`
contra `(yield x)`). Continua em 0,895× do `dart2js -O2` (gz 0,918×).

### 9.7 Validação

* A cada commit, numa exportação limpa (`git archive` mais o *patch*) com
  `CARGO_TARGET_DIR` próprio: *build*, `clippy -D warnings`, testes de
  unidade de `emit_js` e `emit_js_production`, os ignorados
  `optimization_levels`, `int_parity`, `printed_names`, `native_members`,
  `main_arguments` e `async_state_machine`.
* `dartforge-differential --production`: `corpus/js` 274/274 em `-O1` e
  `-O2`, 258/258 em `-O3` e 254/254 em `-O4` (os excluídos de
  `production_levels.tsv`); desenvolvimento 274/274; `corpus/modern` 26/26
  em `-O2` (desenvolvimento 24/26, os mesmos dois de antes).
* `limitless_ui`: e2e 15/15 e as 48 rotas sem erro no Chrome; `new_sali`
  carrega sem erro — os dois com a máquina e as receitas.
* O `crates/emit_js` mudou fora de `ctx.production()` só no `main.mjs` do
  desenvolvimento (§9.1); a máquina de estados e as suas dependências do
  `oxc` só mudam o texto da produção.

## 10. Terceira rodada: mapas de chave primitiva e o runtime do `await`

Rodada de 2026-10-28, noite, até 29 de madrugada. Mesmo método da §1, mas
com a máquina **muito** mais carregada (carga de 10 a 74, outros dois
agentes compilando): os tempos absolutos saíram inflados (o `async_await`
do `dart2js` variou de 245 a 440 ms entre execuções) e só as razões medidas
na mesma execução, com os três arquivos alternados, contam. “Antes” é o
`6eee45b3` (fim da §9).

Commits: `eb85abd4` (§10.1), `ab6ed67a` (§10.2) e `aae6630f` (correção
da §10.2, ver o fim dela).

### 10.1 `IdentityMap` em células

O `IdentityMap` do runtime (todo mapa de chave `String`, `int` ou outra
primitiva, os literais desses mapas e os mapas `identity`) guardava as
entradas num `Map` do JavaScript. Agora, remendo do SDK de produção
(`emit_js/src/sdk_patches.rs`, chave `DARTFORGE_JSPROD_IDENTITY_MAP_CELLS`),
como o `JsLinkedHashMap` do `dart2js`:

* cada entrada é uma célula `{k, v, n, p}` numa lista duplamente encadeada
  na ordem de inserção;
* a célula é achada por três tabelas criadas sob demanda: `_strings`
  (objeto sem protótipo) para as chaves `String`, `_nums` (outro objeto sem
  protótipo) para os inteiros `k` com `(k & 0x3fffffff) === k` — a busca
  vira leitura de elemento — e `_rest` (um `Map`) para o resto (inteiros
  negativos ou grandes, `double`, `bool`, `null`, objetos dos mapas
  `identity`);
* `[]=` decide a tabela uma vez (`_put`/`_putIn`/`_link`); `remove`
  desliga a célula; `clear` solta as tabelas;
* `keys`/`values` continuam no `_JSMapIterable` (o `_map` do `IdentityMap`
  devolve um adaptador com `keys()`/`values()` sobre as células), com a
  checagem de modificação concorrente antes de cada passo, como antes;
  `forEach` anda nas células e checa depois de cada chamada.

Regras da VM, agora exatas (antes três divergiam):

* `-0.0` e `0` são a mesma chave e a célula guarda a **primeira** inserida
  (`<num, int>{}..[-0.0] = 1..[0] = 2` tem a chave `-0.0`; o `Map` do
  JavaScript normalizava para `0`);
* uma chave `NaN` nunca é achada de novo (a célula não entra em tabela):
  dois `m[double.nan] = …` dão duas entradas e `m[double.nan]` é `null`;
* `addAll` só conta como modificação as chaves novas (antes uma chamada
  durante a iteração lançava `ConcurrentModificationError` mesmo sem chave
  nova);
* `putIfAbsent` cujo `ifAbsent` insere a própria chave termina como
  `this[key] = value`.

Medidos e descartados (JavaScript puro, padrão do *kernel*): chaves
`String` no `_rest` (1,9 contra 1,6 ms em 200 mil operações; mapas pequenos
de JSON 15% mais lentos que com `_strings`); tabela de índices com
*arrays* paralelos de chaves e valores (menos variação de GC, mas mapas
pequenos 50% mais lentos); células por construtor em vez de literal (igual
no mínimo, pior na mediana); o truque do `dart2js` de forçar modo dicionário
na criação da tabela (o `Object.create(null)` do V8 já nasce assim).

Teste: `emit_js/tests/programs/identity_maps.dart` (ordem, os três tipos de
chave, `__proto__`/`constructor`/`"0"`, `-0.0`, `NaN`, `null`, `bool`,
`remove`, `clear`, `putIfAbsent`, `addAll`, `update`, iteradores e
modificação concorrente, mapas `identity`, JSON) contra a VM em `-O1`,
`-O2` e `-O4` (`emit_js_production/tests/identity_maps.rs`).

### 10.2 O runtime do `await`

Com a máquina de estados (§9.2), o perfil do `async_await` era quase todo
runtime: avaliação de rti por ambiente (`instanceType(this)[_eval]("…")`,
um `Map` por rti) no `_Future`, *getters* preguiçosos e `t$R`. Mudou, só na
produção:

| mudança | onde | chave |
| --- | --- | --- |
| `_FutureListener.handleValue` de um futuro da zona raiz, rodando na zona raiz, chama o *callback* como o `_RootZone.runUnary` faz, sem avaliar os dois argumentos de tipo que ele ignora | remendo de `async/future_impl.dart` | `DARTFORGE_JSPROD_FUTURE_PATHS` |
| `_Future._asyncComplete` e `_AsyncAwaitCompleter.complete` testam `value is Future` antes de `value is Future<T>` (um valor que não é `Future` não é `Future<T>`): o caso comum não avalia `Future<T>` | `future_impl.dart`, `async_patch.dart` | a mesma |
| um `static` preguiçoso de **biblioteca** vira propriedade de dados depois de inicializado ou escrito (`_rootZone`), como o comentário do `defineLazyField` do DDC pretendia | remendo de `ddc_runtime/operations.dart` | `DARTFORGE_JSPROD_LAZY_DATA_PROPERTIES` |
| `const int` calculado de literais e de outras constantes com `<<`, `>>`, `|`, `&`, `^`, `+`, `-`, `*`, operandos e resultado em `0..2^30` (`_FutureListener.maskValue = 1 << 0`, `stateThenAwait = stateThenOnerror \| maskAwait`) vira o número no uso, em vez de *getter* preguiçoso (o mundo não guarda a variável) | `emit_js/src/own_sdk.rs`, `world/src/lib.rs` | `DARTFORGE_JSPROD_FOLD_CONST_INTS` |
| a receita de uma *closure* que uma função do runtime cria (`dart.fn(f, t$R("~(@)"))` no `_awaitOnObject`) é declarada uma vez mesmo escrita uma vez só (a §9.3 só declarava as repetidas) | `emit_js_production/src/shapes.rs` | `DARTFORGE_JSPROD_SDK_CLOSURE_RECIPES` |

A ordem do laço de eventos não muda: os mesmos `scheduleMicrotask`, os
mesmos ouvintes, a mesma ordem; os atalhos só pulam avaliações de tipo cujo
resultado não era usado.

Defeito encontrado e corrigido no caminho (`aae6630f`): a primeira versão
convertia também os `static` preguiçosos de **classe**; o V8 transforma em
dicionário uma função cujo acessor é redefinido, e todo `static` da classe
ficava mais lento — o `Stopwatch` do `json_encode` levou o
`mixed_utf16_json` de 68 a 100 µs/op. Os de classe (`Zone._current`)
continuam com o *getter*; converter todos valia ~8% no `async_await`, e
só os de biblioteca vale parte disso (dentro do ruído desta rodada).

`async*` e `sync*` continuam com o gerador: nenhum *kernel* os usa e o
protocolo é outro (`_asyncStarHelper`, `_IterationMarker`); não medimos
ganho que justificasse a máquina deles nesta rodada.

Teste: `emit_js/tests/programs/runtime_paths.dart` (estáticos preguiçosos
lidos, escritos, falhando e repetidos; `const` dobrados; futuros completados
com valor, com outro futuro, com `Future<Object>`, `Completer.sync`, erro,
microtarefas e *timers* na ordem da VM; zona com `runUnary` interceptado)
contra a VM em `-O1`, `-O2` e `-O4`, com as chaves ligadas e desligadas
(`emit_js_production/tests/runtime_paths.rs`).

Perfil do `async_await` depois: `Map.get` das avaliações de rti caiu de 15%
para 6% do tempo e o `t$R` saiu do topo; sobra o próprio `_Future`
(`_propagateToListeners`, as duas *closures* e o `_thenAwait` de cada
`await`, a criação do *completer*), que o `dart2js` também executa.

### 10.3 Resultado

Node, `-O4` contra `dart2js -O4` (ms, tempo de CPU da *thread*; mediana de
5 a 7 execuções alternadas). Duas execuções com carga diferente, para mostrar
que a razão se mantém:

| kernel | antes | depois | `dart2js -O4` | depois/`dart2js` |
| --- | ---: | ---: | ---: | ---: |
| `objects_in_collections/map_int_object` (carga ~10) | 12,6 | 5,2 | 4,3 | 1,21 |
| `objects_in_collections/map_int_object` (carga ~25) | 20,0 | 7,9 | 7,2 | 1,10 |
| `objects_in_collections/map_str_object` | 3,4 | 3,4 | 3,7 | 0,92 |
| `collections/map` (`Map<int,int>`, 500 mil) | 57,7 | 48,2 | 44,7 | 1,08 |
| `async_await` (carga ~12) | 571,5 | 333,5 | 247,5 | 1,35 |
| `async_await` (carga ~25) | 620,5 | 392,0 | 258,5 | 1,52 |
| `json/*`, `json_encode/*` | — | ≈ antes | — | ruído (±10%) |

No `map_int_object` as rodadas sem GC empatam com o `dart2js`; a mediana
depende de quantas rodadas pegam uma coleta maior (as células são 200 mil
objetos vivos por rodada, nos dois).

Chrome headless, `-O4` (ms, relógio de parede; mediana de 3 a 5):

| kernel | antes | depois | `dart2js -O4` |
| --- | ---: | ---: | ---: |
| `objects_in_collections/map_int_object` | 10,6 | 7,5 | 2,7 |
| `objects_in_collections/map_str_object` | 2,8 | 2,8 | 3,3 |
| `collections/map` | 29,3 | 16,1 | 30,7 |
| `async_await` | 490,5 | 316,5 | 221,5 |
| `json/encode_large` / `decode_large` | 61,8 / 21,0 | 58,2 / 21,0 | 52,7 / 25,8 |

No Chrome o `map_int_object` tem rodadas de 2,7 ms nos dois arquivos e
rodadas de 8–11 ms (GC); com 5 rodadas, a mediana do nosso caiu nas lentas
e a do `dart2js` nas rápidas (`9,5 2,7 9,7 2,7 7,9` contra
`8,6 2,6 11,6 2,7 2,6`).

Node, nosso `-O2` contra o `dart2js` padrão (`-O1`) (ms; mediana de 5,
carga alta):

| kernel | antes | depois | `dart2js` (`-O1`) |
| --- | ---: | ---: | ---: |
| `objects_in_collections/map_int_object` | 40,4 | 19,4 | 19,8 |
| `collections/map` | 132,4 | 65,7 | 72,9 |
| `async_await` | 1508,5 | 1137,0 | 1341,0 |

### 10.4 Tamanho

Corpus `corpus/js`, como na §5 (`gzip -9`):

| nível | antes | depois | diferença | `dart2js` do nível | depois/`dart2js` |
| --- | ---: | ---: | ---: | ---: | ---: |
| `-O2` | 10.341.152 (gz 3.519.114) | 10.424.048 (gz 3.549.897) | +82.896 (+0,80%; gz +0,87%) | 12.416.485 (gz 4.047.873) | 0,840× (gz 0,877×) |
| `-O4` | 10.224.278 (gz 3.485.084) | 10.306.823 (gz 3.515.294) | +82.545 (+0,81%; gz +0,87%) | 11.024.980 (gz 3.582.721) | 0,935× (gz 0,981×) |

Quase tudo é o `IdentityMap` (`-O4`: +65 KB, gz +26 KB; +700 a +980 bytes
em cada um dos 72 programas que usam mapa de chave primitiva): a classe
nova é maior que a que delegava ao `Map`. O restante (§10.2) soma +17 KB
(gz +4 KB): +60 a +100 bytes do `defineLazyField` em todo programa, e os
programas com `async` *encolhem* ~500 bytes (as máscaras do
`_FutureListener` dobradas). Tentativas de encolher o `IdentityMap`: os
iteradores próprios viraram o adaptador do `_JSMapIterable` (−16 KB).

Projetos (`-O2`): `limitless_ui` 3.349.301 → 3.349.816 (gz 944.878 →
945.125); `new_sali` 7.178.083 → 7.176.651 (gz 1.937.907 → 1.937.748).

### 10.5 Validação

A cada commit, numa exportação limpa (`git archive` do HEAD mais os
arquivos do commit, com `CARGO_TARGET_DIR` próprio e os arquivos tocados
para recompilar): *build*, `clippy -D warnings` (`emit_js`,
`emit_js_production`, `world`), testes de unidade, os ignorados
`optimization_levels`, `int_parity`, `printed_names`, `native_members`,
`main_arguments`, `async_state_machine`, `identity_maps` e
`runtime_paths`; `dartforge-differential --production` em `corpus/js`
274/274 em `-O1` e `-O2`, 258/258 em `-O3`, 254/254 em `-O4`
(`production_levels.tsv`), desenvolvimento 274/274, `corpus/modern` 26/26
em `-O2`; `limitless_ui` e2e 15/15 e as 48 rotas sem erro no Chrome;
`new_sali` carrega sem erro. O `crates/emit_js` mudou só em remendos do SDK
de produção e no dobramento de constantes do modo SDK
(`own_sdk::inline_constant`, só com `ctx.sdk`): o texto do desenvolvimento
é o mesmo.

### 10.6 O que continua

* **`async_await` (~1,4×).** O que sobra é o `_Future` do DDC, o mesmo
  código do `dart2js` sem a inferência dele: a criação do *completer* (duas
  avaliações de rti por chamada `async`), o `_thenAwait` (uma avaliação e um
  `bind` por `await`), o `Zone._current` de classe pelo *getter*
  preguiçoso. Próximos passos medidos como possíveis: o emissor passar a rti
  do *completer* pronta, e mover o `Zone._current` para uma variável de
  biblioteca.
* **`map_int_object`**: igual ao `dart2js` fora do GC; a variação é das
  coletas.
* **Tamanho**: a margem no gzip do `-O4` caiu de 0,973× para 0,981×.
* Os demais itens da §7 (`list_sort`, `collatz`, JSON, `closures`) não
  foram tocados.
