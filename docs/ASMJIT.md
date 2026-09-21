# Backend de JIT por montador: AsmJit, UJIT e dynasm-rs

Terceiro experimento de backend do perfil de desenvolvimento, ao lado de
`crates/jit` (LLVM ORCv2, `docs/JIT.md`) e `crates/cranelift-jit`
(`docs/CRANELIFT.md`). Ele ocupa o extremo oposto do espectro: **sem IR e sem
otimização**. A HIR é percorrida uma vez e cada nó vira instrução de máquina no
ato.

O crate é `crates/asmjit-jit`. O nome preserva o do experimento, que nasceu
como "AsmJit"; a implementação adotada é `dynasm-rs`.

## Resumo executivo

| Pergunta | Resposta |
| --- | --- |
| Qual biblioteca foi adotada | `dynasm-rs` 5.1.0 (`dynasmrt`), MPL-2.0 |
| Estado da fatia mínima | Verde: 21 testes, inclusive o diferencial contra o executável AOT |
| O UJIT da AsmJit resolve a objeção do PLANO.md? | **Sim no nível de API, não no nível de arquitetura** — ver [O UJIT](#o-ujit-da-asmjit) |
| O UJIT foi adotado? | **Não.** Recomendação fundamentada de não adotar |
| Motivo principal da recusa | Zero API em C + o único recurso da nossa fatia que o UJIT **não** cobre de forma independente de alvo é justamente `call` |
| Eixo 1, geração de código | **ganha**: 5× a 18× mais rápido que o Cranelift sem otimizar, ~8× no maior programa |
| Eixo 2, execução do gerado | **perde**: 3,8× para o Cranelift e ~85× para o LLVM `-O2` no laço pesado; só 1,35× em código dominado por chamadas |
| Eixo 5, arquiteturas por emissor | `dynasm-rs` **1**; UJIT **2**; Cranelift e LLVM, todas |

## A fatia mínima, implementada em dynasm-rs

### O que está coberto

Funções de topo com até quatro parâmetros e retorno `int`/`bool`/`void`,
variáveis locais com sombreamento por escopo, aritmética (`+`, `-`, `*`, `-`
unário), as seis comparações, `&&`, `||`, `!`, `if`/`else`, `while`,
`do`/`while`, `for`, `break`, `continue`, `return`, chamadas entre funções do
programa — inclusive recursivas e mutuamente recursivas — e `print` de inteiros
e booleanos pelo runtime.

`int` é i64 com estouro modular e a ordem de avaliação é a do Dart, igual ao
backend LLVM. Tudo o mais é recusado com diagnóstico e span da AST, inclusive em
trechos inalcançáveis, com o prefixo `asmjit JIT ainda não suporta `.

### O modelo de execução, e por que ele é tão simples

Um montador não traz alocador de registradores. A escolha é entre escrever um ou
não precisar de um, e este tradutor não precisa: **toda posição viva mora na
pilha** e `RAX` é o único acumulador.

* cada parâmetro e cada declaração de variável recebe uma posição fixa de 8
  bytes relativa a `RBP`, calculada **antes** do prólogo por `src/quadro.rs`;
* uma expressão deixa seu valor em `RAX`;
* numa operação binária o operando esquerdo é derramado numa posição temporária
  enquanto o direito é avaliado, e depois relido em `RAX` com o direito em `RCX`.

O prólogo precisa saber de quanta pilha a função vai precisar antes de conhecer o
corpo. Há duas saídas: emitir um `sub rsp, imm32` remendável e corrigi-lo no fim,
ou percorrer o corpo antes e descobrir o tamanho. `src/quadro.rs` faz a segunda,
que é determinística e testável sem depender do mecanismo de *patch* da
biblioteca. Posições não são reaproveitadas entre escopos irmãos: gasta-se pilha
para não precisar de análise de tempo de vida, que é exatamente o tipo de
trabalho que um montador não faz.

O código resultante é volumoso e cheio de idas à pilha. É esse o preço que o
eixo 2 mede.

### A ABI é responsabilidade do backend

`src/abi.rs` isola as três decisões que o `dynasm-rs` não toma: onde cada
argumento chega, quanto de pilha reservar e o alinhamento no ponto de chamada.

A fatia para em **quatro parâmetros** porque a Win64 usa `RCX, RDX, R8, R9` e a
System V AMD64 usa `RDI, RSI, RDX, RCX` e aceitaria seis. Passar o quinto
argumento é justamente onde as duas ABIs mais divergem, e uma divergência não
testada num backend é pior do que uma recusa explícita.

O *shadow space* de 32 bytes da Win64 é reservado **uma vez no prólogo**, não a
cada chamada. É isso que permite manter `RSP` fixo durante todo o corpo e
dispensar ajuste de pilha por chamada.

Nada disso aparece no `dynasm-rs` e nada disso apareceria no Cranelift ou no
LLVM, que geram prólogo e epílogo a partir de uma assinatura. **É o primeiro
custo concreto de um montador, e ele é por arquitetura.**

### O que o teste diferencial pegou

O contrato central do projeto é que um backend que discorda do AOT no mesmo
programa está errado. No montador esse teste pega o que nada mais pega:
convenção de chamada, alinhamento de pilha, *shadow space*, largura de
registrador e a ordem em que os argumentos vão para os registradores.

O caso que mais exercita o modelo é chamada aninhada em posição de argumento:

```dart
print(quatro(quatro(1, 1, 1, 1), 2, 3, quatro(0, 1, 1, 0)));
```

Os argumentos externos já avaliados **não podem** morar em registrador enquanto a
chamada interna acontece, porque a chamada interna destrói todo registrador de
argumento. O tradutor resolve derramando cada argumento numa posição temporária e
só carregando os registradores depois que todos foram avaliados
(`src/tradutor.rs`, `fn chamada`). Está coberto por
`chamadas_aninhadas_e_sombreamento_de_escopo`.

### Correção de medição encontrada no caminho

O contador de bytes de código estava lendo `ExecutableBuffer::size()`, que
devolve o tamanho do **mapeamento**, arredondado para páginas: 4096 bytes para
qualquer programa da fatia — um contador constante, e portanto inútil. Passou a
ser o deslocamento final do montador, que é o número real de bytes emitidos. O
tamanho mapeado continua disponível em `ProgramaCompilado::bytes()`, onde
interessa: ele é o custo de memória por recarga do eixo 4.

## O UJIT da AsmJit

A AsmJit em C++ tem um emissor chamado **UJIT** — "Universal JIT API", descrito
pelo projeto como *"experimental target-independent backend with opt-in to target
dependent code generation for maximum performance"*.

Se isso funcionasse como o nome promete, resolveria a objeção registrada no
`PLANO.md`: escrever assembly à mão foi descartado como fundação porque exigiria
**um emissor por arquitetura**, e x64 e aarch64 são ambos necessários.

O levantamento abaixo é de leitura do código-fonte, não do README. Referência
registrada em `docs/references-manifest.json`: `github.com/asmjit/asmjit`, commit
`dffd8b164f228abbc47246c9881f107d656952b2`, 2026-09-15, licença **Zlib**,
biblioteca versão 1.23.0 (`asmjit/core/build_defs.h:19`).

### 1. O UJIT é de fato independente de alvo?

**Para a nossa fatia, sim — e a evidência é forte.**

A classe do consumidor é uma só, `ujit::UniCompiler` (`asmjit/ujit/uni_compiler.h:22`).
O consumidor **nunca nomeia um registrador de máquina**: aloca registradores
virtuais com `new_gp32`/`new_gp64`/`new_gpz`/`new_gp_ptr`
(`uni_compiler.h:774-800`), e a alocação de registradores é feita pelo passe de
RA da AsmJit.

O dado decisivo: a seção **"Emit - General Purpose Instructions"** vai de
`uni_compiler.h:991` a `1193` e **não contém uma única diretiva de arquitetura**.
A primeira aparece na linha 1446, já dentro da seção de vetores. Todo o
`uni_condition.h` (293 linhas) também é livre de guardas. Ou seja: toda a
superfície escalar — `mov`, `load*`, `store*`, `add`, `sub`, `mul`, `neg`,
deslocamentos, `cmov`, `select`, `j` — é idêntica nos dois alvos.

Os arquivos `uni_compiler_x86.cpp` (7.620 linhas) e `uni_compiler_a64.cpp`
(4.405 linhas) são **implementações internas da mesma API pública**, com
assinaturas idênticas — não APIs divergentes. Exemplo do que fica escondido:
`umod` não existe em AArch64 e é sintetizado com `udiv`/`mul`/`sub`
(`uni_compiler_a64.cpp:972-978`).

Prólogo, epílogo e quadro de pilha: o UJIT **não os emite**, e não precisa. Ele
delega ao Compiler da AsmJit, cujo inseridor de prólogo/epílogo trabalha a partir
do `FuncNode` (`asmjit/core/compiler.h:479-487`). O consumidor escreve
`add_func(sig)` … `ret(r)` … `end_func()` e nunca toca em pilha. **Todo o
trabalho de `src/abi.rs` e `src/quadro.rs` deste crate desapareceria.** Essa é a
parte em que o UJIT entrega exatamente o que promete.

O mecanismo de "opt-in a código dependente de alvo" existe e é o membro público
`cc` (`uni_compiler.h:135-136`), com o `#if` escrito **pelo consumidor** —
documentado com exemplo em `asmjit/core.h:2238-2259`.

**Mas há um limite de arquitetura que o nome esconde.** `asmjit/core/build_defs.h:324-332`
é uma cadeia `#if / #elif / #else`: exatamente um de `ASMJIT_UJIT_X86` e
`ASMJIT_UJIT_AARCH64` é definido, pelo arco do **alvo de compilação**, e se
nenhum casar o UJIT se autodesabilita. Consequência: o UJIT dá **portabilidade de
código-fonte entre alvos de build**, não emissão multi-alvo em tempo de execução.
Um processo x86-64 não consegue emitir AArch64 pelo `UniCompiler`, embora o
`a64::Assembler` cru da AsmJit consiga.

Para o eixo 5 isso ainda é uma vitória: **um emissor, um código-fonte, duas
arquiteturas**. Só não é o mesmo que "um binário emite para as duas".

Onde a independência vaza, fora da nossa fatia: o cabeçalho público tem 34
guardas de arquitetura, e a pior delas é o enum `GPExt`, que existe nas duas
arquiteturas **com o mesmo nome e significados diferentes**
(`uni_compiler.h:38-88` contra `:90-128`). Qualquer consumidor que consulte
recurso de CPU escreve código por arquitetura. `new_vec256`/`new_vec512` são só
x86; a forma base+índice de `mem_ptr` perde o deslocamento em AArch64
(`ujit_base.h:38-48`).

### 2. O UJIT cobre o que a nossa fatia precisa?

| Recurso da fatia | Situação | Evidência |
| --- | --- | --- |
| Inteiro de 64 bits: add, sub, mul, neg | **Sim** | `uni_compiler.h:1096-1112`, `:1051` |
| Comparações com sinal de 64 bits | **Sim** | `uni_condition.h:202-227` |
| Salto condicional e incondicional, rótulos, `bind` | **Sim** | `uni_compiler.h:1178-1180`, `:488`, `:537` |
| Acesso a memória de 64 bits, posição de pilha | **Sim** | `uni_compiler.h:1008-1027`, `new_stack` em `:932` |
| Prólogo, epílogo, quadro, ABI | **Sim, e melhor que o nosso** | `core/compiler.h:479-487`; `add_func`/`ret`/`end_func` em `uni_compiler.h:681-728` |
| **Chamada de função** | **Parcial — o furo** | ver abaixo |
| Booleano a partir de comparação (`setcc`/`cset`) | **Não existe**; via `cmov`/`select` | `uni_compiler.h:1042-1046` |
| Divisão e módulo **com sinal** | **Não existem** | `uni_op.h:104-105` só tem `kUDiv`/`kUMod` |

O furo das chamadas é o achado mais importante desta investigação. A seção
inteira de invocação do UJIT são 14 linhas (`uni_compiler.h:742-755`) e expõe
apenas `new_invoke_node` / `add_invoke_node`, ambas exigindo um `InstId` — **que é
específico de arquitetura**: `x86::Inst::kIdCall` contra `a64::Inst::kIdBl`. Não
existe `UniCompiler::invoke()` nem enumerador `UniOp*` para "call".

A saída prática é `uc.cc->invoke(...)`, que compila nas duas arquiteturas porque
`x86::Compiler::invoke` e `a64::Compiler::invoke` têm assinaturas iguais
(`x86/x86_compiler.h:768-776`, `arm/a64_compiler.h:318-326`) — e isso cobre tanto
chamar função JIT por rótulo quanto **função C nativa por endereço absoluto**,
que é o que o nosso `print` faz. Mas essa portabilidade é **coincidência de
assinatura, não contrato documentado do UJIT**.

E o indício mais eloquente: o próprio teste do UJIT,
`asmjit-testing/tests/asmjit_test_ujit.cpp`, tem **5.952 linhas e zero usos de
`InvokeNode` ou `invoke`**. O UJIT não exercita chamadas a partir do código
gerado. Para um backend de linguagem, em que chamada é a operação central, isso é
território sem teste a montante.

Um dado de proporção que enquadra tudo: o UJIT é, por qualquer medida, uma
**abstração de vetor/SIMD** com escalar em segundo plano. Em `uni_op.h`, os enums
escalares ocupam 100 das 819 linhas e 67 dos 617 enumeradores — **~11%**. Em
`uni_compiler.h`, a seção escalar tem 203 linhas contra ~870 de vetor, e há 627
pontos de entrada de operação vetorial gerados por macro contra ~40 operações
escalares. Existe um pool de constantes vetoriais obrigatório
(`vec_const_table.h`, `_ct_ref` em `uni_compiler.h:141`) e nenhum equivalente
escalar. O consumidor principal da AsmJit é o Blend2D, um rasterizador 2D; a
forma do UJIT reflete isso.

### 3. O custo: FFI, CMake e shim em C

**Não existe API em C na AsmJit.** Uma varredura por `extern "C"` na árvore
inteira devolve exatamente duas linhas, ambas em
`asmjit/core/virt_mem.cpp:124-145`, e são a **importação** de `mach_vm_remap` da
Apple — não uma API exportada. Nenhum arquivo `c_api`/`capi` existe.

Isso significa que usar o UJIT do Rust obriga a escrever um shim `extern "C"` em
C++ cobrindo **toda** a superfície necessária, e nada dela é representável em C:

* `Gp`, `Vec`, `Mem` são classes C++ com métodos `constexpr`;
* `UniCondition` guarda `Operand` por valor;
* as operações vetoriais são *templates* de função (`uni_compiler.h:1233-1283`);
* a ABI é carregada por `FuncSignature::build<Ret, Args...>()`, variádico de
  template.

O shim teria de envolver `UniCompiler`, `FuncNode`, `Label`, `Gp`, `Mem`,
`UniCondition`, `CodeHolder` e `JitRuntime`.

Exigências de ambiente, medidas no `CMakeLists.txt` do commit registrado:

| Item | Valor | Evidência |
| --- | --- | --- |
| CMake mínimo | **3.24** | `CMakeLists.txt:46` |
| Padrão C++ | **C++20**, e `PUBLIC` (propaga ao consumidor) | `CMakeLists.txt:430` |
| Linguagens | `CXX` apenas | `CMakeLists.txt:54-58` |
| UJIT é padrão? | **Sim, é opt-out**: `ASMJIT_NO_UJIT` default `OFF` | `CMakeLists.txt:90-97` |
| Dá para enxugar? | **Não muito**: UJIT exige Builder + Introspection + Compiler | `CMakeLists.txt:81-97` |

Volume de código a compilar, contado com `wc -l` (sem testes): `core` 39.939
linhas, `x86` 28.555, `arm` 17.421, `ujit` 16.476, `axl` 7.359 — **112.343 linhas
em 179 arquivos, 58 unidades de tradução**. Um build mínimo apenas para o host
x86-64 com UJIT fica em **~94.922 linhas e 48 unidades de tradução**
(`ASMJIT_NO_FOREIGN=ON` zera o outro backend via `build_defs.h:306-314`, embora as
11 unidades AArch64 continuem sendo invocadas pelo CMake e pré-processem para
nada).

Comparação direta com o que já está integrado:

| | dynasm-rs (adotado) | UJIT por FFI |
| --- | --- | --- |
| Toolchain extra | **nenhuma** — é um crate | CMake ≥ 3.24 + compilador C++20 |
| Código de terceiros compilado | `dynasm` + `dynasmrt`, Rust | ~95.000 linhas de C++, 48 TUs |
| Camada escrita à mão só para atravessar | **nenhuma** | shim C++ `extern "C"` sobre 8 tipos |
| `cargo build` do workspace continua suficiente? | **sim** | não — passa a exigir `build.rs` + CMake |
| Licença | MPL-2.0 (copyleft fraco por arquivo) | Zlib (permissiva) |
| Depuração de erro de codificação | falha em tempo de compilação do Rust | atravessa a fronteira de FFI |

A licença é o único eixo em que o UJIT ganha: Zlib é mais permissiva que
MPL-2.0. Na prática isso não decide nada, porque consumir `dynasmrt` sem
modificá-lo não afeta a licença MIT do DartForge — a MPL-2.0 é copyleft **por
arquivo**, e nenhum arquivo dela é modificado.

### 4. O UJIT é declarado experimental pelo próprio projeto

Sim, e a forma como isso está registrado é em si um dado. Uma varredura
insensível a caso por `experimental|unstable|subject to change|may change|API
break` na árvore toda devolve **um único aviso relevante**,
`asmjit/core.h:2106-2107`:

> UJIT is still in an experimental phase, expect minor API breaks in the future
> especially towards API stabilization.

Nada em `asmjit/ujit/` — nem em `ujit.h`, `ujit_base.h`, `uni_compiler.h`,
`uni_condition.h`, `uni_op.h`. **Um consumidor que inclua `<asmjit/ujit.h>` não vê
aviso nenhum.** O `README.md` também não menciona estabilidade do UJIT; sua única
referência é a linha de organização do projeto.

Para um experimento, "minor API breaks" é aceitável. O que não é aceitável é
pagar o custo de um shim de FFI sobre uma API que quebra, porque cada quebra
atravessa a fronteira: o shim é código nosso, em C++, que ninguém a montante
testa.

### Recomendação: não adotar o UJIT

O UJIT **resolve de verdade** a objeção técnica do `PLANO.md` — um emissor, um
código-fonte, x64 e aarch64 — e resolve de lambuja a parte mais chata deste
crate, a ABI e o quadro de pilha. Isso é um resultado real e vale registrar:
**a objeção "um emissor por arquitetura" não é mais verdadeira para montadores em
geral**, só para os montadores que estamos em posição de usar do Rust.

E é aí que a conta não fecha, por três motivos que se reforçam:

1. **Zero API em C.** O custo não é "escrever um `build.rs`": é manter um shim
   C++ sobre oito tipos com `constexpr`, templates variádicos e operandos por
   valor, para um workspace que hoje se constrói com `cargo build` e nada mais.
2. **O furo é exatamente onde dói.** De tudo que a nossa fatia precisa, o único
   item que o UJIT não cobre de forma independente de alvo é `call`. E o próprio
   teste do UJIT, com 5.952 linhas, nunca chama uma função. Adotaríamos uma
   camada de portabilidade cuja única lacuna é a operação central de uma
   linguagem.
3. **Nem `~/` nem `%` com sinal existem.** `uni_op.h:104-105` tem só as variantes
   sem sinal. Os dois operadores estão fora da fatia atual, mas estão dentro do
   Dart, e cada um deles voltaria a ser código por arquitetura — o custo que se
   pretendia eliminar.

Um "não adotar" com essa evidência é mais útil do que uma integração pela metade.
A segunda implementação da fatia mínima **não foi escrita**, deliberadamente: o
critério de "duas implementações medidas lado a lado valem mais que uma opinião"
pressupõe que as duas sejam viáveis de manter, e o item 1 já decide isso antes de
qualquer medição.

O que muda no `PLANO.md`, se algo mudar: o motivo 2 da decisão ("assembly à mão
exige dois emissores mantidos em paralelo") vale para `dynasm-rs`, que é o que
temos, e **não** vale como afirmação geral sobre montadores. A objeção que
sobrevive intacta é a motivo 1 e a motivo 3 — genéricos reificados, null safety,
raízes de GC e exceções atravessando chamadas são lowering de verdade, e nem o
UJIT ajuda nisso.

## Os cinco eixos

Metodologia de `docs/DESEMPENHO.md`: mediana e p95 de 20 amostras após 3
aquecimentos, mais contadores que não dependem da carga da máquina. O arnês é
`crates/asmjit-jit/examples/experimento.rs`:

```text
cargo run --release --example experimento -p dartforge-asmjit-jit
```

> **Aviso de contaminação.** As medições de tempo desta seção foram tomadas com
> outros três agentes compilando no mesmo host, e o relógio está comprovadamente
> ruidoso: um `cargo build` de um crate de dependência única passou de dez
> minutos durante a coleta. **Os tempos absolutos não são limpos e não devem ser
> citados como linha de base.** O que é confiável aqui são (a) os contadores de
> instruções e bytes, que não dependem de carga, e (b) as ordens de grandeza
> entre caminhos medidos na mesma execução. `docs/DESEMPENHO.md` exige dizer isso
> em vez de apresentar o número como limpo.

### Eixo 1 — tempo de geração de código em memória, da HIR ao executável

Duas fases, e não três: um montador **codifica cada instrução no ato**. Não há IR
intermediária para otimizar depois, e no `dynasm-rs` a codificação já foi resolvida
quando o próprio Rust compilou — `dynasm!` é macro procedural e em tempo de
execução só restam os operandos variáveis. A primeira fase mede HIR → bytes já
codificados, com a validação da fatia; a segunda mede só a resolução dos saltos
pendentes e a publicação das páginas executáveis. Fingir três fases aqui daria uma
tabela comparável com a do Cranelift e falsa.

| programa | tradução p50 (ms) | p95 | publicação p50 (ms) | p95 | total p50 (ms) | p95 | instruções x86-64 | bytes de código | bytes mapeados |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| trivial | 0,000 | 0,001 | 0,003 | 0,004 | 0,009 | 0,012 | 11 | 44 | 4096 |
| fib(32) | 0,002 | 0,002 | 0,003 | 0,004 | 0,010 | 0,010 | 58 | 288 | 4096 |
| somatorio(1e8) | 0,002 | 0,003 | 0,003 | 0,003 | 0,010 | 0,011 | 52 | 272 | 4096 |
| dez-funcoes | 0,016 | 0,033 | 0,008 | 0,015 | 0,040 | 0,061 | 242 | 1234 | 4096 |

A mesma HIR, na mesma execução, pelos outros dois caminhos:

| programa | Cranelift sem otimizar, total p50 (ms) | p95 | bytes | LLVM IR textual p50 (ms) | p95 | bytes de IR |
| --- | --- | --- | --- | --- | --- | --- |
| trivial | 0,044 | 0,121 | 32 | 0,001 | 0,001 | 870 |
| fib(32) | 0,176 | 0,271 | 132 | 0,007 | 0,008 | 1356 |
| somatorio(1e8) | 0,065 | 0,157 | 97 | 0,008 | 0,008 | 1480 |
| dez-funcoes | 0,319 | 0,504 | 472 | 0,040 | 0,045 | 3655 |

**O montador ganha o eixo 1 com folga: 0,040 ms contra 0,319 ms do Cranelift no
programa de dez funções, cerca de 8× mais rápido.** A razão varia de ~5× em
`trivial` a ~18× em `fib(32)` e não acompanha o tamanho do programa — ela depende
de quanto o Cranelift tem a construir de IR e quanto o seu custo fixo por função
pesa, e com quatro programas não dá para afirmar mais que "entre 5× e 18×, ~8× no
maior". Era o resultado esperado — não há IR para construir, nem
passe, nem alocador de registradores — e é a única razão pela qual um montador
entra nesta comparação.

O contador que não depende de carga mostra o outro lado da mesma moeda: para
`dez-funcoes` o montador emite **1.234 bytes contra 472 do Cranelift, 2,6× mais
código** para o mesmo programa. É o modelo de pilha sem alocador de registradores
aparecendo como volume. Esse número prevê o eixo 2.

### Eixo 2 — tempo de execução do código gerado, com laço pesado

Os dois JITs executam na thread do processo de teste. O AOT mede **o processo
inteiro**, incluindo criá-lo, então a linha `trivial` serve de piso: ~11 ms de
custo fixo que nada tem a ver com o código gerado.

| programa | caminho | p50 (ms) | p95 (ms) |
| --- | --- | --- | --- |
| trivial | asmjit JIT (dynasm-rs) | 0,000 | 0,000 |
| trivial | Cranelift JIT (opt_level=none) | 0,000 | 0,000 |
| trivial | Cranelift JIT (opt_level=speed) | 0,000 | 0,000 |
| trivial | LLVM AOT O0 (processo inteiro) | 11,027 | 12,280 |
| trivial | LLVM AOT O2 (processo inteiro) | 10,556 | 11,215 |
| somatorio(1e8) | **asmjit JIT (dynasm-rs)** | **112,085** | 116,268 |
| somatorio(1e8) | Cranelift JIT (opt_level=none) | 29,577 | 33,941 |
| somatorio(1e8) | Cranelift JIT (opt_level=speed) | 28,514 | 33,434 |
| somatorio(1e8) | LLVM AOT O0 (processo inteiro) | 54,340 | 62,421 |
| somatorio(1e8) | LLVM AOT O2 (processo inteiro) | 11,861 | 16,634 |
| fib(32) | **asmjit JIT (dynasm-rs)** | **11,353** | 12,988 |
| fib(32) | Cranelift JIT (opt_level=none) | 8,422 | 9,213 |
| fib(32) | Cranelift JIT (opt_level=speed) | 8,346 | 9,421 |
| fib(32) | LLVM AOT O0 (processo inteiro) | 20,404 | 22,100 |
| fib(32) | LLVM AOT O2 (processo inteiro) | 15,696 | 17,427 |

Descontando o piso de ~11 ms do AOT para estimar só o trabalho, o quanto se perde
é este:

| laço pesado, `somatorio(1e8)` | tempo de cálculo | contra o montador |
| --- | --- | --- |
| asmjit JIT (dynasm-rs) | ~112 ms | — |
| Cranelift, sem otimizar | ~30 ms | montador **3,8× mais lento** |
| Cranelift, `speed` | ~29 ms | montador **3,9× mais lento** |
| LLVM AOT O0 | ~43 ms | montador **2,6× mais lento** |
| LLVM AOT O2 | ~1,3 ms | montador **~85× mais lento** |

**A previsão do enunciado se confirma, e o tamanho da perda depende da forma do
programa.** No laço pesado o montador perde 3,8× para o Cranelift **sem
otimização nenhuma dos dois lados** — a diferença é puramente o modelo de
execução: o Cranelift tem alocador de registradores e mantém o acumulador e o
índice em registrador, enquanto este montador lê e escreve a pilha em toda
operação. Contra o LLVM em `-O2` a perda vai a ~85×, porque o `-O2` reconhece o
somatório e não executa o laço.

Em `fib(32)` a perda cai para **1,35×** contra o Cranelift. O motivo é
esclarecedor: `fib` é dominado por chamadas, e o custo de uma chamada — prólogo,
epílogo, `call`, `ret` — é parecido nos dois backends. O modelo de pilha só custa
caro onde há trabalho aritmético em laço. Um montador sem alocador de
registradores não é uniformemente 4× mais lento; ele é ~1,3× mais lento em código
que chama e ~4× mais lento em código que calcula.

Vale notar o que o eixo 2 diz sobre o AOT no perfil de desenvolvimento: para
`trivial`, os ~11 ms de criar o processo são **mil vezes** o tempo de execução do
código. Para um ciclo de recarga, o custo fixo do processo domina tudo, que é
exatamente a aritmética de que o `PLANO.md` já advertia.

### Eixo 3 — custo de construção

#### O que o dynasm-rs cobra

Uma linha no `Cargo.toml`. O `dynasmrt` traz **14 crates** para o grafo, todos
Rust, nenhum com `build.rs` que exija ferramenta externa:

```text
dynasmrt 5.1.0 · dynasm 5.1.0 (proc-macro) · byteorder 1.5.0 · bitflags 2.13.2
lazy_static 1.5.0 · fnv 1.0.7 · memmap2 0.9.11 · proc-macro-error3 3.1.1
proc-macro-error-attr3 3.1.1 (proc-macro) · proc-macro2 1.0.107 · quote 1.0.47
syn 2.0.119 · syn 3.0.6 · unicode-ident 1.0.26
```

Onze desses 14 já estavam no grafo do workspace por outros caminhos; os
acréscimos reais são `dynasmrt`, `dynasm` e `memmap2`, mais uma **segunda cópia do
`syn`** (3.0.6 ao lado da 2.0.119), que é o item mais caro da lista porque `syn` é
grande e proc-macro.

Um custo que este experimento acrescentou de propósito e que é justo declarar:
`crates/asmjit-jit` tem `dartforge-cranelift-jit` em `[dev-dependencies]`, só para
que `examples/experimento.rs` possa medir os dois backends na mesma execução — que
é a única forma de a comparação do eixo 2 valer algo. Isso faz
`cargo test -p dartforge-asmjit-jit` compilar todo o Cranelift na primeira vez.
Nenhum **teste** usa o Cranelift, então a correção deste backend não fica acoplada
à do outro; quem quiser pagar menos remove a dependência e perde só as colunas de
comparação.

**O que passa a ser exigido do ambiente: nada.** `cargo build --workspace`
continua bastando. Não há CMake, não há compilador C ou C++, não há variável de
ambiente nova, e o crate compila com a toolchain de `rust-toolchain.toml` sem
nenhum passo prévio.

Tempo: um build limpo do `dynasmrt` isolado, num diretório de destino vazio,
levou **11m16s** de relógio — e esse número está **contaminado**, não é uma
medição da biblioteca. Havia três outros agentes compilando o mesmo workspace no
momento; o build `release` do próprio `examples/experimento.rs`, que inclui todo o
Cranelift, levou 6m39s na mesma janela. Um crate de dependência única não leva 11
minutos numa máquina livre. O contador honesto aqui é o de 14 crates / 3
acréscimos reais, não o relógio.

#### O que o UJIT cobraria

| Item | dynasm-rs | UJIT por FFI |
| --- | --- | --- |
| Ferramenta externa | nenhuma | CMake ≥ 3.24 (`CMakeLists.txt:46`) |
| Compilador | o `rustc` do projeto | C++20, `PUBLIC` (`CMakeLists.txt:430`) |
| Terceiros compilados | 14 crates Rust | ~94.922 linhas de C++, 48 unidades de tradução |
| Código de cola escrito por nós | nenhum | shim `extern "C"` sobre 8 tipos C++ |
| `cargo build --workspace` basta? | **sim** | não: `build.rs` + invocação de CMake |
| Enxugável? | n/a | pouco: UJIT exige Builder + Introspection + Compiler (`CMakeLists.txt:81-97`) |

Vale registrar o que é possível enxugar: `ASMJIT_NO_FOREIGN=ON` zera o backend da
arquitetura não usada via `asmjit/core/build_defs.h:306-314`, derrubando o total de
112.343 para ~94.922 linhas. Mas as 11 unidades de tradução AArch64 continuam
sendo invocadas pelo CMake — elas pré-processam para nada, o que economiza
compilação de código mas não as invocações.

### Eixo 4 — o que oferece para hot reload

#### dynasm-rs

A propriedade que importa está na documentação do `dynasmrt::Assembler`: **nenhuma
memória é gravável e executável ao mesmo tempo**. A montagem acontece num buffer
comum e só a confirmação (`commit`) copia para páginas marcadas como executáveis.
Isso é uma garantia de segurança de graça, e é o motivo de `src/execucao.rs` ser o
único módulo com `unsafe` neste crate.

O que **não** existe: redefinição in-place de uma função já publicada. O
`ExecutableBuffer` é imutável depois do `finalize`. A recarga possível hoje é
montar a versão nova num bloco novo; as duas coexistem e a memória de cada uma é
devolvida ao sistema quando o seu `ProgramaCompilado` é destruído. Está fixado em
dois testes: `recompilar_cria_bloco_novo_e_mantem_o_antigo_valido` e
`recargas_sucessivas_nao_reaproveitam_codigo_vivo`.

O tipo é deliberadamente `!Send` e `!Sync`: o código gerado só pode ser chamado
pela thread que o montou, e a captura de saída do runtime é por thread.

#### AsmJit / UJIT, para registro

A AsmJit é substancialmente mais rica neste eixo, e é justo dizê-lo:

* `JitRuntime::add` / `release` (`core/jit_runtime.h:82-95`), com
  `JitAllocator::alloc`/`release`/`shrink`/`write` **documentados como
  thread-safe** (`core/jit_allocator.h:336-404`);
* contrato completo de W^X: mapeamento duplo, `MAP_JIT` da Apple,
  `ProtectJitReadWriteScope`, `flush_instruction_cache` e PAUTH
  (`core/jit_allocator.h:293-312`, `core/virt_mem.h:189-265`);
* `JitAllocator::write(span, offset, src, size, policy)`
  (`core/jit_allocator.h:372-377`) — que é a API correta para **remendar um
  trampolim no lugar**, o caso de uso do hot reload;
* `CodeHolder::reinit()` (`core/code_holder.h:748-771`), um laço de regeneração
  rápida que preserva arena, logger e emissores anexados;
* política de retenção `kImmediateRelease` (`core/jit_allocator.h:50-55`), cuja
  ausência mantém um bloco vazio por pool — o padrão certo para um laço de
  recarga.

E uma ausência que vale tanto quanto as presenças: **a AsmJit não documenta
nenhuma condição sob a qual é seguro liberar ou substituir código que pode estar
executando.** Não há contador de época, contagem de referência, safepoint nem
aviso do tipo "não libere com quadros na pilha". `JitRuntime::release` devolve a
região ao alocador na hora, e o alocador a reutiliza. A AsmJit dá mecanismo
thread-safe e **nenhuma** política de tempo de vida; rastrear liveness é
integralmente do chamador. Nisso o `dynasm-rs` e a AsmJit estão no mesmo lugar, e
é o `crates/jit` com ORCv2 que tem a história mais completa (`docs/JIT.md`).

#### Quanto custa uma recarga, medido

O ciclo completo de uma recarga neste backend — montar da HIR, executar uma vez e
liberar o bloco antigo:

| programa | ciclo p50 (ms) | p95 | bytes mapeados por recarga |
| --- | --- | --- | --- |
| trivial | 0,010 | 0,012 | 4096 |
| dez-funcoes | 0,018 | 0,019 | 4096 |

O contador independente de carga é o da direita, e ele expõe uma propriedade
importante: **cada recarga consome uma página inteira, 4096 bytes, mesmo para um
programa de 44 bytes de código**. O `dynasmrt` arredonda o mapeamento para páginas
e não compartilha mapeamento entre blocos. Num laço de recarga longo isso é 4 KB
de espaço de endereços por versão até o `ProgramaCompilado` correspondente ser
destruído — aceitável, mas é o tipo de custo que só aparece se o contador for
medido, e é por isso que ele está aqui.

Comparado ao piso de ~11 ms para criar um processo AOT (eixo 2), uma recarga de
0,010 ms é ruído. O gargalo de um ciclo de hot reload não está em nada que este
eixo meça.

### Eixo 5 — custo de manutenção, e quantas arquiteturas por emissor

Este é o eixo decisivo, e a tabela é qualitativa de propósito: o número que
importa não é um tempo.

| | dynasm-rs (adotado) | UJIT (recusado) | Cranelift | LLVM |
| --- | --- | --- | --- | --- |
| **Arquiteturas por emissor** | **1** — um emissor por arco | **2** (x64, aarch64), um código-fonte | todas as do alvo | todas |
| Alocador de registradores | nenhum, escrevemos o modelo de pilha | sim, da AsmJit | sim | sim |
| Prólogo/epílogo/ABI | **nossos** (`abi.rs`, `quadro.rs`) | da AsmJit | da biblioteca | da biblioteca |
| Forma da instrução | fixada em tempo de compilação do Rust | em tempo de execução | IR | IR |
| Custo de um recurso novo do Dart | × nº de arquiteturas | × 1 (mas ver furos) | × 1 | × 1 |

O custo por arquitetura do `dynasm-rs` não é teórico neste crate, e há um lugar
onde ele aparece de forma quase caricatural. No `dynasm-rs` a **forma** da
instrução é fixada quando o próprio Rust compila: não há como parametrizar o
sufixo de uma comparação. As seis comparações do Dart viram, por isso, seis
braços de `match` literalmente idênticos a menos de `sete`/`setne`/`setl`/`setle`/
`setg`/`setge` (`src/tradutor.rs`, `fn binaria`). Multiplique isso por duas
arquiteturas e por cada operação nova.

Em compensação, o que se ganha é real e não deve ser subestimado: **não há
alocador de registradores, não há IR e não há passe nenhum**. O `src/` inteiro são
7 arquivos e 1.839 linhas — contra as 112.343 linhas da AsmJit ou a superfície do
Cranelift e do LLVM — e a tradução é uma única passagem sem estado global. Para um tier 0 no modelo
*template JIT* — que é o lugar que o `PLANO.md` reserva a assembly escrito à mão —
essa simplicidade é exatamente a característica desejada.

## O que a suíte cobre

21 testes: 2 unitários em `src/abi.rs`, 16 de integração em `tests/execucao.rs` e
3 exemplos de documentação.

| Teste | O que fixa |
| --- | --- |
| `a_fatia_coberta_reproduz_a_saida_do_dart_3_6_2` | 7 programas, saída byte a byte igual à da VM do Dart 3.6.2 |
| `a_saida_do_jit_bate_com_a_do_executavel_aot` | **contrato central**: os 7 programas dão a mesma saída no JIT e no executável AOT |
| `o_jit_e_o_backend_llvm_aceitam_o_mesmo_corpus` | os dois backends aceitam exatamente o mesmo corpus |
| `jit_e_llvm_recusam_o_mesmo_ponto_do_programa` | quando ambos recusam, o span é idêntico |
| `cada_forma_fora_da_fatia_tem_mensagem_e_span_exatos` | 17 formas fora da fatia, mensagem e recorte do span conferidos |
| `codigo_morto_depois_do_return_ainda_e_recusado` | nada fora da fatia passa por não executar |
| `inteiro_e_i64_com_estouro_modular` | `int` é i64 modular, igual ao LLVM e ao Dart |
| `recursao_simples_de_arvore_e_mutua` | `call` para rótulo não vinculado vira relocação resolvida no `commit` |
| `chamadas_aninhadas_e_sombreamento_de_escopo` | chamada em posição de argumento não corrompe os argumentos externos |
| `break_e_continue_nos_tres_lacos` | destino de `continue` nos três laços e `break` em laço aninhado |
| `o_quinto_parametro_e_recusado_por_causa_da_abi` | o limite de 4 parâmetros é recusado, não divergido entre alvos |
| `os_contadores_de_trabalho_sao_deterministicos` | os contadores do eixo 1 não dependem da carga da máquina |
| `as_medicoes_separam_traducao_de_publicacao_do_bloco` | as fases são coerentes e o mapeamento nunca é menor que o código |
| `recompilar_cria_bloco_novo_e_mantem_o_antigo_valido` | eixo 4: as versões coexistem, a antiga não é invalidada |
| `recargas_sucessivas_nao_reaproveitam_codigo_vivo` | 8 recargas seguidas, nenhuma página reciclada sob código vivo |
| `a_execucao_e_repetivel_no_mesmo_programa_compilado` | executar duas vezes não acumula nem perde saída |

O teste diferencial contra o AOT é `#[ignore]` porque exige Clang e rustc:

```text
$env:DARTFORGE_CLANG = 'D:\LLVM\22.1.8\bin\clang.exe'
cargo test -p dartforge-asmjit-jit -- --include-ignored
```

`unsafe` aparece em **um** bloco, em `src/execucao.rs:91`, com a invariante
documentada em português: o ponteiro aponta para o prólogo de `main` dentro do
bloco confirmado pelo `dynasmrt`, que continua vivo durante toda a chamada porque
`&self` empresta o programa inteiro, e o corpo foi emitido com zero parâmetros e o
prólogo/epílogo da convenção nativa — exatamente `extern "C" fn()`.

## Conclusão

`dynasm-rs` fica como a implementação do experimento, verde e medida, e o nome do
crate continua registrando de onde a pergunta veio.

O UJIT **não** é adotado. A recomendação é fundamentada e, ao mesmo tempo,
corrige um ponto do `PLANO.md`: a afirmação de que um montador exige
necessariamente um emissor por arquitetura **não é mais verdadeira em geral** — o
UJIT demonstra o contrário, com registradores virtuais, RA, ABI e prólogo
resolvidos numa API única para x64 e aarch64. O que o impede aqui não é a ideia,
é a travessia: zero API em C, ~95.000 linhas de C++ e um shim de oito tipos
atravessados por template, sobre uma API declarada experimental, cujo único furo
de portabilidade é a operação mais central de uma linguagem — a chamada de
função — e cujo próprio teste de 5.952 linhas nunca a exercita.

Se algum dia a AsmJit publicar uma API em C, ou se o UJIT ganhar `invoke`
independente de alvo e um teste que o cubra, a conta muda. A evidência para
reabrir a pergunta está toda aqui, com arquivo e linha.
