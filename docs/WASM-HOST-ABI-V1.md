# ABI de host v1 do DartForge para `wasm32-mvp`

Data: 21/10/2026. **Estado: subconjunto do W0** (saída e execução). As demais
operações da §3.4 do [plano](PLANO-WASM-MVP.md) (relógios, entropia, agendamento,
recursos opcionais) entram nesta mesma versão antes do W5; o que já está aqui não
muda de significado.

A ABI é o único contrato entre um módulo `wasm32-mvp` e quem o executa. O mesmo
arquivo `.wasm`, byte a byte, roda nos dois adaptadores:

| adaptador | onde | motor |
|---|---|---|
| executor independente | `crates/wasm_host` (`dartforge-wasm-run`) | Wasmi 2.0.0, só recursos MVP |
| navegador e Node | `runtime/wasm/dartforge_host_v1.mjs` | `WebAssembly` do motor JS |

## Perfil do módulo

Todo módulo passa pelo portão estrito (`crates/wasm_gate`, `dartforge-wasm-gate`)
antes de executar; o executor recusa o que o portão recusa.

- Validação com `wasmparser` 0.262.0 e `WasmFeatures::MVP`: nada de sign-extension,
  conversões saturantes, bulk memory, multivalue, reference types, SIMD, threads,
  EH, tail calls, memory64, multi-memory nem WasmGC.
- Imports só de funções e só do módulo `dartforge_host_v1`. Memória, tabela e
  globais importadas são recusadas (nada de WASI, `env` ou libc).
- Nenhuma global exportada (a extensão `mutable-globals` fica fora do perfil
  estrito, mesmo que o alvo Rust `wasm32v1-none` a declare em `target_features`).
- No máximo uma memória e uma tabela; páginas iniciais limitadas pela política.
- A seção `target_features` é só informativa: a prova é a validação.

## Exports exigidos

| export | tipo | contrato |
|---|---|---|
| `memory` | memória | a memória linear do módulo, definida por ele |
| `dartforge_abi_version` | `() -> i32` | devolve `1`; outro valor faz o host recusar o módulo sem executar |
| `dartforge_run` | `() -> i32` | executa o programa uma vez e devolve o status de saída |

O host chama `dartforge_abi_version` e, se for `1`, `dartforge_run` uma única vez.
O status devolvido é o código de saída do programa. Uma *trap* do motor
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

## Versionamento

Mudar assinatura ou significado de qualquer item acima exige `dartforge_host_v2`
e outra versão em `dartforge_abi_version`. Acrescentar imports à v1 é permitido
enquanto o plano não declarar a v1 fechada (W5); um módulo só importa o que usa.

## Evidência

`scripts/wasm-w0.sh` gera um módulo com LLVM 22.1.8 (`-mcpu=mvp`) e um fragmento
Rust `no_std` (`wasm32v1-none`, `target-cpu=mvp`), passa no portão e executa o mesmo
arquivo no Wasmi, no Node 24 (V8) e no Chrome headless. Resultados em
`bench/results/2026-10-21-wasm-w0/`.
