# ABI de host v1 do DartForge para `wasm32-linear`

Data: 21/10/2026. **Estado: W0, W2 e W3** (execução, saída, término, relógios,
entropia, fuso e espera de timers), mais o interop JS (`dartforge_js`, W4). O que
já está aqui não muda de significado.

A ABI é o único contrato entre um módulo `wasm32-linear` e quem o executa. O mesmo
arquivo `.wasm`, byte a byte, roda nos dois adaptadores:

| adaptador | onde | motor |
|---|---|---|
| executor independente | `crates/wasm_host` (`dartforge-wasm-run`) | Wasmtime 50.0.0 (Cranelift), só as features do perfil |
| navegador e Node | `runtime/wasm/dartforge_host_v1.mjs` | `WebAssembly` do motor JS |

## Perfil do módulo

Todo módulo passa pelo portão (`crates/wasm_gate`, `dartforge-wasm-gate`) antes de
executar; o executor recusa o que o portão recusa.

- Validação com `wasmparser` 0.262.0 restrito ao perfil `wasm32-linear` do
  [plano](PLANO-WASM-MVP.md) §3.1: WebAssembly 2.0 (sign-extension, conversões sem
  trap, bulk memory, multivalue, reference types, SIMD128) mais tail calls,
  extended constant expressions e exceções com `exnref`. Recusados: WasmGC, typed
  function references, threads e memória compartilhada, EH legado, relaxed SIMD,
  memory64 e multi-memory.
- Imports só de funções e só do módulo `dartforge_host_v1`. Memória, tabela e
  globais importadas são recusadas (nada de WASI, `env` ou libc).
- Nenhuma global exportada: a ABI exporta funções e a memória.
- No máximo uma memória e uma tabela; páginas iniciais limitadas pela política.
- A seção `target_features` é só informativa: a prova é a validação.

## Exports exigidos

| export | tipo | contrato |
|---|---|---|
| `memory` | memória | a memória linear do módulo, definida por ele |
| `dartforge_abi_version` | `() -> i32` | devolve `1`; outro valor faz o host recusar o módulo sem executar |
| `dartforge_run` | `() -> i32` | executa o programa e devolve o status de saída, ou `-1` enquanto ele espera um timer |
| `dartforge_delay_us` | `() -> f64` | depois de um `-1`: microssegundos até o próximo timer (`inf` se distante demais) |
| `dartforge_resume` | `() -> i32` | continua o programa depois do atraso; status ou `-1` de novo |

O host chama `dartforge_abi_version` e, se for `1`, `dartforge_run` uma única vez.
O módulo nunca bloqueia esperando um timer: com só timers futuros, o laço de
eventos devolve `-1`, o host espera `dartforge_delay_us` (no navegador com
`setTimeout`, sem travar a página) e chama `dartforge_resume`, até receber o
status. Uma porta viva sem timers encerra o programa, como no dart2wasm.
O status devolvido é o código de saída do programa (0, ou 255 numa exceção não
tratada, como a VM); um `exit` do módulo encerra a execução por dentro com o seu
status. Os dois exports vêm do runtime (`crates/runtime/src/wasm_entry.rs`). Uma *trap* do motor
(`unreachable`, acesso fora da memória, divisão por zero, pilha esgotada) é
resultado separado do status: o host a reporta como falha do runtime, nunca como
exceção Dart, e não encerra o próprio processo por causa dela. O executor de linha
de comando sai com 70 numa trap e 2 quando o módulo nem chegou a executar.

## Imports (`dartforge_host_v1`)

### `write(channel: i32, offset: i32, length: i32) -> i32`

Copia `length` bytes da memória linear a partir de `offset` para o canal, de forma
síncrona; o módulo pode reutilizar o buffer assim que a chamada voltar. Os bytes
são UTF-8, mas o host não os decodifica nem valida.

| status | significado |
|---|---|
| `0` | copiado |
| `-1` | faixa fora da memória: `offset` ou `length` negativo, ou `offset + length` (sem overflow) maior que o tamanho atual |
| `-2` | faixa válida, mas canal diferente de `1` (stdout) e `2` (stderr) |

O host lê a memória atual a cada chamada, então `memory.grow` entre chamadas é
seguro. `write` não reentra no módulo.

As faixas de memória de todos os imports seguem a mesma regra de `write`: fora da
memória atual o import devolve `-1` e não escreve nada.

### `exit(status: i32)`

Encerra a execução do módulo com `status`, sem voltar: o host desenrola os quadros
do módulo (no JS, lançando um erro que o adaptador captura; no Wasmtime, um erro do
import) e reporta `status` como resultado de `dartforge_run`, como o `proc_exit`
do WASI. O runtime o usa para `exit` de erros fatais e para memória esgotada
(`Out of memory: …` no stderr e status 255).

### `clock_realtime_us() -> f64`

Microssegundos desde a época Unix (UTC), o relógio civil de `DateTime.now`. O
navegador devolve `Date.now() * 1000`, a mesma resolução de milissegundo do
dart2wasm; o executor devolve microssegundos, como a VM.

### `clock_monotonic_us() -> f64`

Microssegundos desde uma origem do host, nunca decrescente: `Stopwatch`, timers e
estatísticas do coletor. Navegador: `performance.now() * 1000`.

### `random_fill(offset: i32, length: i32) -> i32`

Preenche a faixa com o gerador criptográfico do host (`crypto.getRandomValues` no
navegador, o do sistema operacional no executor): `Random()` sem semente e
`Random.secure`. Devolve `0`, `-1` (faixa inválida) ou `-3` (host sem gerador); o
runtime trata `-3` como falha, nunca recorre a uma fonte fraca.

### `time_zone(epoch_seconds: f64, offset_out: i32, name_offset: i32, name_capacity: i32) -> i32`

O fuso local no instante dado: escreve o deslocamento em segundos a leste de UTC
(`i32` little-endian) em `offset_out` e até `name_capacity` bytes do nome em UTF-8
em `name_offset`; devolve o comprimento completo do nome (`-1` com faixa
inválida). O navegador responde como o `DateTime` do dart2wasm (`getTimezoneOffset`
e o nome entre parênteses de `Date.prototype.toString`); o executor, como a VM
nativa (`localtime_r`; no Windows, as regras da zona para o ano).

### `option(name_offset: i32, name_length: i32, out_offset: i32, out_capacity: i32) -> i32`

Uma opção de execução do runtime (`DARTFORGE_GC_STRESS`, `DARTFORGE_GC_STATS`…),
não o `Platform.environment` do Dart: copia até `out_capacity` bytes do valor e
devolve o comprimento completo, ou `-1` quando a opção não existe. O executor
repassa só as variáveis `DARTFORGE_*` do próprio processo; o adaptador JS, o
objeto `options` de `runModule` (o runner do Node passa as `DARTFORGE_*`).

## Interop JS (`dartforge_js`)

Um módulo que usa `dart:js_interop` importa também do módulo `dartforge_js`, que
só um host JavaScript tem: o adaptador (`Interop` em
`runtime/wasm/dartforge_host_v1.mjs`) e o `.mjs` companheiro que o `compile-wasm`
escreve ao lado do `.wasm` (`<app>.wasm` → `<app>.mjs`). O executor independente
recusa o módulo antes de executar, dizendo que ele precisa de um host JS.

Um valor JavaScript é um *handle* `i32` da tabela do adaptador; o handle 0 é
`null`/`undefined`, que o Dart vê como `null`. Cada `JSValue` do Dart é dono de
um handle e o devolve à tabela quando morre (finalizador nativo do coletor).

| import | tipo | contrato |
|---|---|---|
| `release` | `(handle: i32)` | devolve o handle à tabela |
| `string_from_latin1` | `(offset, length) -> i32` | uma string JS com os `length` bytes (unidades Latin-1) da faixa; handle novo |
| `string_from_utf16` | `(offset, length) -> i32` | uma string JS com as `length` unidades UTF-16 da faixa (alinhada a 2), surrogates soltos preservados |
| `string_length` | `(handle) -> i32` | o comprimento `n` da string, ou `-n - 1` quando alguma unidade passa de `0xFF` |
| `string_read` | `(handle, offset, two_bytes)` | escreve as unidades da string na faixa: bytes, ou `u16` com `two_bytes = 1` |
| `0`, `1`, … | os do trecho | os trechos do programa, definidos no companheiro |

Um **trecho** é a função JavaScript de um membro de interop ou de um `external`
do SDK com `@pragma('dartforge:js', '<código>')`. No companheiro, cada import
converte os argumentos (handle → valor; string → valor, consumindo o handle;
`bool` de membro → booleano JS), chama o trecho e converte o resultado (valor →
handle novo; número e inteiro pelo `ToNumber`/`ToInt32` da fronteira). Se o
trecho lança, o import chama o export **`dartforge_js_throw`** e devolve 0: ao
voltar, o código Dart encontra a exceção pendente, um `_JavaScriptError` como o
do dart2wasm. O módulo exporta `dartforge_js_throw` sempre que tem trechos.

O companheiro exporta `abi = 1` e `snippets(r)`, que devolve as funções dos
imports por nome; `r` é o `Interop` do adaptador (`get`, `take`, `ref`, `fail`).
Não há `eval`: o arquivo é JavaScript comum, compatível com CSP. Os nomes curtos
(`0`, `1`…) são dados depois do link; nos objetos o import é `s:<hex>` da
assinatura e do código, de modo que trechos iguais de objetos diferentes viram
um só import.

## Versionamento

Mudar assinatura ou significado de qualquer item acima exige `dartforge_host_v2`
e outra versão em `dartforge_abi_version`. Acrescentar imports à v1 é permitido
enquanto o plano não declarar a v1 fechada (W5); um módulo só importa o que usa.

## Evidência

`scripts/wasm-w2.sh` liga o runtime real do DartForge para wasm32 a um programa
de prova e executa o mesmo arquivo no Wasmtime, no Node e no Chrome, com OOM e
duas instâncias (`bench/results/2026-10-21-wasm-w2/`).

`scripts/wasm-w0.sh` gera um módulo com LLVM 22.1.8 e um fragmento Rust com `std`
(`wasm32-unknown-unknown`), com as features do perfil, passa no portão e executa o
mesmo arquivo no Wasmtime, no Node 24 (V8) e no Chrome headless. Resultados em
`bench/results/2026-10-21-wasm-w0/`.
