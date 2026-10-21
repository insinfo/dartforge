# Plano de implementação: DartForge → LLVM → WebAssembly MVP

Data: 20/10/2026. **Estado: planejamento; backend e runtime Wasm ainda não
implementados.** Este documento não registra execução Dart→Wasm bem-sucedida.

## 1. Objetivo e contrato de entrega

Adicionar um destino AOT Wasm32 ao DartForge, reaproveitando frontend, resolução,
HIR, otimizações compatíveis e geração LLVM. O mesmo arquivo `.wasm` deverá
executar no navegador e em um executor independente que implemente a ABI de host
do DartForge. A primeira entrega usa Dart 3.6.2 como referência semântica.

“MVP” significa o conjunto original de recursos WebAssembly, não apenas um
produto inicial incompleto. Cada marco terá dois estados separados:

1. **Conformidade binária:** módulo inteiro e dependências respeitam o perfil MVP.
2. **Cobertura Dart:** conjunto de recursos/bibliotecas efetivamente implementados,
   com recusas explícitas para o restante.

Não anunciar suporte geral a Dart, `dart:io`, FFI ou aplicações gráficas por ter
executado aritmética, hello ou um objeto LLVM isolado. Não usar o compilador
oficial Dart→Wasm para produzir os artefatos próprios; ele pode ser uma referência
adicional, identificada separadamente dos oráculos VM/JS.

### Critérios da primeira versão utilizável

- AOT standalone de programas com classes, closures, coleções, strings, tipos,
  exceções verificadas e GC, dentro de um subconjunto explicitamente catalogado.
- Bibliotecas portáteis de `core`, `collection`, `typed_data` e `convert` necessárias
  ao corpus aprovado; suporte `async` acrescentado antes de declarar esse perfil.
- Mesmo SHA-256 do `.wasm` executado no navegador e no executor independente.
- Nenhum import implícito de libc, POSIX, Win32, AppKit ou WASI.
- Sem WasmGC, threads compartilhadas, SIMD, EH Wasm ou outras extensões pós-MVP.
- Compatibilidade do frontend/ABI dos cinco destinos nativos preservada.
- Instalação e execução documentadas, sem LLVM/Dart SDK exigidos em produção
  para apenas executar um módulo já compilado.

## 2. Base existente e lacunas confirmadas

| Área | Base reaproveitável | Trabalho necessário |
|---|---|---|
| Frontend/HIR | Parser, elementos, tipos, lowering e otimizações atuais | Seleção de bibliotecas/capacidades e auditoria de pressupostos nativos |
| ABI | `Target::Wasm32`, ponteiro de 32 bits, `i64` escalar | Contrato completo de runtime, imports/exports, layouts e chamadas indiretas |
| Teste ABI | LLVM escalar emitido como objeto Wasm, cabeçalho conferido | Ligação e execução de Dart real; validar instruções e semântica |
| LLVM instalado | Clang lista `wasm32`/`wasm64`; `lld -flavor wasm` disponível | Registrar WebAssembly no LLVM embutido e controlar features em ambos os geradores |
| Driver | Seleção imutável de destino e caches por alvo | Separar arquitetura, ambiente de execução, formato e capacidades de plataforma |
| Runtime | Objetos, RTI, tracing, exceção pendente, raízes explícitas | Heap linear, contexto por instância, plataforma portátil e runtime Wasm compilável |
| SDK | Fontes e overlays revisados, efeitos/ownership catalogados | Perfil/overlays Wasm e fronteiras de host verificadas |

Pontos de entrada para leitura antes das alterações:

- [ABI e teste](../crates/abi/src/lib.rs),
  [native_call](../crates/abi/tests/native_call.rs).
- [Destinos](../crates/emit_native/src/target.rs),
  [gerador](../crates/emit_native/src/generator.rs),
  [integração LLVM](../crates/llvm/src/lib.rs).
- [Emissão LLVM](../crates/emit_native/src/llvm/mod.rs),
  [cache](../crates/emit_native/src/cache.rs),
  [SDK](../crates/emit_native/src/sdk_module.rs).
- [Runtime](../crates/runtime/src/lib.rs),
  [fragmentos](../crates/runtime/build.rs),
  [espaço de objetos](../crates/runtime/src/space.rs),
  [layout](../crates/runtime/src/layout.rs),
  [raízes](../crates/runtime/src/gc_roots.rs).

O contrato inicial em [ABI-FFI-WASM](ABI-FFI-WASM.md) não representa um runtime
Wasm pronto. As referências antigas ao driver host-only são históricas; os
destinos nativos atuais estão em [CROSS-COMPILACAO](CROSS-COMPILACAO.md).

## 3. Arquitetura proposta

```text
Dart + package_config + SDK/overlays Wasm
                   ↓
          frontend → HIR → LLVM IR
                   ↓
       LLVM WebAssembly + runtime Wasm
                   ↓
              LLD Wasm
                   ↓
       app.wasm + manifesto da ABI
             /              \
    adaptador JS          host de executor
    navegador             independente
```

O compilador continua executando nativamente em macOS, Linux ou Windows.
Compilar o próprio DartForge para executar dentro de Wasm fica fora desta entrega.
Não será necessário iniciar as VMs para cada compilação Wasm; elas serão usadas
para validar a distribuição e os executores nos sistemas hóspedes.

Mapa inicial de responsabilidades, sem criar arquivos de implementação agora:

| Componente | Alteração planejada |
|---|---|
| `crates/cli` | Comando/opções, erros, manifesto e publicação segura de saídas |
| `crates/emit_native/src/target.rs` e `cross.rs` | Descritor Wasm, capacidades, layout e isolamento de bundles |
| `crates/llvm` e `emit_native/src/generator.rs` | Registro do backend, CPU/features explícitas, objetos/bitcode |
| Ligação em `crates/emit_native` | Caminho próprio LLD Wasm; imports e exports permitidos |
| `emit_native/src/llvm` e HIR | Remover pressupostos sp/rsp/TLS nativos e aplicar ABI/layout do destino |
| `runtime` e seleção de fragmentos | Núcleo portátil, allocator/GC/contexto e implementações de plataforma |
| `native_sdk` e carregador de SDK | Overlays/capacidades Wasm, efeitos e fingerprint independente |
| Integração/distribuição | Adapters JS/executor, validator, corpus e ferramentas empacotadas |

Extrair novos módulos/crates somente quando a separação do núcleo/host estiver
definida; não começar por duplicar frontend ou runtime. Usar nomes/APIs/código
em inglês e preservar a seleção de fontes revisadas usada pelas provas atuais.

### 3.1 Perfil binário fixado

Identificador proposto: `wasm32-mvp`, mapeado internamente para
`wasm32-unknown-unknown` mais um perfil explícito de features e ABI.
Um triple sozinho não descreve o contrato completo.

Baseline: memória linear de 32 bits, uma memória e, quando houver chamadas
indiretas, uma tabela de funções com inicialização compatível com MVP. Usar
tipos numéricos MVP e no máximo um resultado direto por função. Agregados usam
memória de saída/convenção revisada, sem depender de multivalue.

Recusar instruções/formatos de GC, reference-types adicionais, bulk-memory,
sign-extension pós-MVP, conversões saturantes pós-MVP, SIMD, atomics/threads,
exception handling, tail calls, memory64, multi-memory e multivalue. Globais
mutáveis internas necessárias ao código LLVM não devem ser confundidas com a
extensão de importação/exportação de globais mutáveis: esta também será excluída
do perfil estrito. Exportar funções de acesso quando necessário.

Fixar `target-cpu=mvp` e features em **todos** os insumos: programa, SDK Dart,
runtime Rust, `core`/`alloc`, builtins e qualquer biblioteca auxiliar. Auditar a
compatibilidade do toolchain Rust/LLVM selecionado; o LLVM usado por rustc pode
ter versão diferente do LLVM usado na emissão Dart.

Não confiar nos padrões atuais de `wasm32-unknown-unknown`, em um comentário
`target_features`, nem no cabeçalho `\0asm` como prova. Uma etapa obrigatória
validará instruções, seções, tipos, imports e limites do módulo final com
ferramenta/configuração versionadas que rejeitem recursos fora do perfil.
`wasm32v1-none` não substitui automaticamente o MVP estrito: o perfil documentado
pelo Rust inclui `mutable-globals`.

### 3.2 Valores, ponteiros, heap e chamadas

- Preservar inicialmente a representação Dart interna de 64 bits onde isso
  mantiver o contrato semântico; `i64` é admitido no MVP.
- Separar rigorosamente `Ref`/handle, índice da tabela de funções e offset de
  memória. Nenhum deles é um ponteiro nativo do host.
- Fazer layout do runtime depender do destino, não de `usize`/`size_of` do
  compilador host. Descrever e testar cabeçalhos, alinhamentos, offsets de
  campos, RTI, closures, listas, typed data, frames e estruturas FFI admitidas.
- Auditar `ptrtoint`/`inttoptr`, tagging, sinais e truncamentos. Conversões para
  offsets Wasm32 precisam preservar faixa e identidade; não truncar silenciosamente
  um handle arbitrário de 64 bits para “fazê-lo caber”.
- Centralizar o allocator da instância. A heap Rust de suporte e a heap Dart
  devem usar regiões disjuntas ou um allocator coordenado; duas cópias independentes
  de `memory.grow`/allocator não podem reservar a mesma região.
- Tratar falha de `memory.grow`, limite configurado e overflow de tamanho antes
  de publicar objetos. Definir fronteiras entre dados estáticos, stack LLVM,
  stacks/frames gerenciados e heap; reservas não podem se sobrepor.
- Liberar objetos para reutilização interna; não depender de devolver páginas
  lineares ao sistema operacional após cada coleta.
- Rebaixar ponteiros de função para índices/convenções LLVM Wasm revisadas.
  Preservar assinatura das chamadas indiretas; trap de assinatura não é um
  mecanismo de despacho dinâmico Dart.

### 3.3 Runtime portátil, GC e exceções

Separar semântica comum das implementações de plataforma. Não copiar todo o
runtime para uma segunda implementação que possa divergir silenciosamente.
O perfil Wasm deverá ter um núcleo sem dependências de SO, preferencialmente
`no_std` + `alloc`, com seleção explícita dos fragmentos/bibliotecas necessários.
Isso não exige converter o compilador host nem todo o runtime desktop para
`no_std`; exige isolar as dependências para o artefato Wasm.

Inicialmente: tracing GC e shadow stack explícita, contexto por instância,
single-thread. Portar os contratos de objetos e raízes existentes. Priorizar
uma primeira coleta não movente se isso reduzir os pré-requisitos; a escolha
deve ser registrada no marco de heap e não alterar identidade, weak references
ou finalização anunciadas como suportadas. A evolução geracional é posterior à
correção do coletor básico. ARC e mapas de pilha nativos não são requisitos iniciais.

Não localizar raízes na stack opaca do engine nem ler `sp`/`rsp` ou usar
registradores TLS nativos. Referências vivas em chamadas, alocações, closures,
globais, async e retornos precisam ser visíveis ao coletor. Auditoria específica
das raízes em otimizações/LTO e nas fronteiras com o host.

Usar o modelo de exceção pendente/verificações. `throw`, `catch`, `rethrow` e
`finally` preservam ordem de efeitos e limpeza sem Wasm EH ou `setjmp/longjmp`.
Panic do runtime, trap Wasm, erro retornado pelo host e exceção Dart têm caminhos
distintos. Não declarar um trap irrecuperável como exceção Dart capturável.

Para stack overflow, definir um orçamento instrumentado e verificar os limites
dos engines; profundidade fixa não garante prevenir todo overflow da stack
Wasm. Se não houver garantia demonstrada de `StackOverflowError` capturável,
registrar a limitação e mantê-la fora da cobertura aprovada. Stack traces
iniciais usam metadados próprios de funções/locais, sem unwinder nativo.

### 3.4 ABI comum de host v1

Namespace proposto de imports: `dartforge_host_v1`. Nomes/assinaturas finais
serão fixados antes de escrever os dois adaptadores. A ABI usará `i32`, `f64`
e buffers em memória linear na fronteira JS portátil; inteiros de 64 bits serão
transferidos por buffer ou duas metades, sem exigir a integração JS BigInt.
Callbacks assíncronos usam IDs, nunca objetos JS como referências Dart brutas.

| Operação | Contrato a definir |
|---|---|
| Saída | UTF-8 `(offset, length)`, canal e resultado explícito; consumo síncrono/cópia |
| Relógios | Monotônico e civil separados, unidade e origem documentadas |
| Entropia | Preencher buffer, erro explícito quando indisponível; sem fallback inseguro |
| Agendamento | Solicitação/cancelamento por ID; entrega somente em ponto permitido |
| Recursos opcionais | Capacidade identificada e retorno “não suportado” explícito |
| Inicialização/execução | Inicializar runtime e executar entrada uma vez, com estado verificável |
| Conclusão/erro | Status numérico e buffer diagnóstico, sem encerrar o processo do host |

Só importar o necessário ao perfil do artefato. Nenhum export assinado como
“ponteiro” concede acesso fora da memória linear. Validar faixas com aritmética
que detecte overflow, alinhar estruturas e especificar ownership/lifetime.
Copiar dados que precisem sobreviver ao retorno. Handles externos precisam de
retenção/liberação e proteção contra IDs obsoletos.

O host não pode chamar Dart de novo durante GC, inicialização ou import síncrono
não reentrante. A primeira ABI enfileira conclusões e drena eventos em pontos
controlados. Uma exceção JS lançada por um adaptador não pode atravessar a
fronteira como se o unwinding Dart tivesse ocorrido; adaptadores capturam e
traduzem falhas nos termos da ABI.

### 3.5 Biblioteca padrão e perfis de ambiente

Produzir um mapa explícito de biblioteca/API/capacidade. Núcleo portátil não
declara `dart:io`, bibliotecas dinâmicas ou DOM como disponíveis. Imports
condicionais e constantes `dart.library.*` precisam refletir o perfil compilado,
incluindo dependências transitivas e fontes geradas. Não detectar “Linux” pelo
sistema onde o compilador está rodando.

Para o **mesmo binário nos dois ambientes**, manter as mesmas decisões de
compilação e ABI; diferenças de capacidades do host são consultadas em runtime.
Um futuro perfil exclusivamente web com interop DOM pode produzir outro módulo,
mas deve ser identificado como tal. Não prometer que alterar somente o loader
muda constantes Dart já dobradas durante a compilação.

`dart:ffi` nativo, `DynamicLibrary`, ponteiros do host, sockets POSIX, subprocessos
e filesystem geral ficam fora do perfil portátil inicial. Erros são detectados
na compilação quando conhecidos, ou retornados pela operação quando dependem
da capacidade fornecida pelo host. Não inserir stubs que retornem sucesso falso.

## 4. Fases, dependências e portas de aceite

Nenhum item abaixo está marcado como concluído. O trabalho será dividido em
commits revisáveis em `main`, sem branches/worktrees adicionais por padrão.

### W0 — Inventário e prova técnica limitada

1. Inventariar dependências de plataforma/representação do runtime, SDK e LLVM.
2. Fixar versões do toolchain, validator, engines e configuração MVP estrita.
3. Conferir ABI de LLVM↔Rust: escalares, `i64`, offsets, alinhamento e chamada
   indireta, sem GC ou bibliotecas completas.
4. Provar objeto→ligação→execução em browser e executor com imports mínimos.
5. Confirmar que runtime auxiliar/builtins podem ser produzidos no mesmo perfil;
   verificar controle de panic, allocator e bibliotecas Rust reconstruídas.

**Aceite:** módulo escalar válido no perfil estrito, mesmo hash nos dois hosts,
resultado esperado e nenhuma dependência inesperada. **Não conta como Dart→Wasm.**
Se Rust/LLVM ou builtins exigirem recursos extras, corrigir essa fronteira antes
de portar o runtime; não renomear o perfil moderno como MVP.

### W1 — Modelo de destino, ABI e caches

1. Acrescentar o destino ao modelo sem fingir que Wasm é Linux/Unix.
2. Definir descritor compartilhado de layout e ABI, versões e feature set.
3. Registrar o backend WebAssembly no LLVM embutido; configurar Clang e LLD.
4. Separar caches host/wasm, MVP/outros perfis e revisões da ABI/runtime/SDK.
5. Validar opções incompatíveis: JIT nativo, CPU host, shared SDK nativo,
   table exceptions, stack maps e bundles/sysroots desktop.
6. Fixar allowlist de imports/exports e resolver todos os demais símbolos na
   ligação. Não usar liberação irrestrita de símbolos indefinidos para esconder
   fragmentos do runtime ainda não portados. Preservar os entry points exigidos
   pela ABI mesmo sob dead stripping/LTO.

**Aceite:** consultas de alvo não inicializam plataforma errada, metadados
corretos, recusa de combinações inválidas e testes dos destinos antigos passam.
Nenhum cache/bundle incompatível pode ser aceito por alteração manual do manifesto.

### W2 — Núcleo do runtime e heap linear

1. Isolar seleção de fragmentos e dependências Rust do perfil Wasm.
2. Implementar reservas/allocator na memória linear com limites configuráveis.
3. Implementar contexto por instância, estados de inicialização e destruição.
4. Validar layouts compartilhados entre Rust e LLVM, strings e objetos básicos.
5. Testar uma e duas instâncias independentes, crescimento e falta de memória.

**Aceite:** runtime liga sem imports ocultos; allocations não se sobrepõem;
memória e contexto não vazam entre instâncias. Falhas não corrompem o estado
ou viram sucesso silencioso. OOM irrecuperável deve ser distinguido de erro Dart.

### W3 — Primeiro Dart real e coleta precisa

1. Compilar hello, aritmética, chamadas, strings e classes simples pelo pipeline
   DartForge completo, com SDK/overlays Wasm.
2. Portar tracing/shadow stack e barreiras efetivamente necessárias ao coletor.
3. Cobrir raízes em campos/globais, closures, laços, chamadas indiretas e retornos.
4. Implementar exceção pendente, limpeza e unwinding lógico de frames.
5. Executar alocações repetidas e GC forçado em todas as oportunidades previstas.

**Aceite:** stdout e efeitos iguais à VM nos casos aprovados, GC sob estresse,
raízes precisas e memória reutilizada em execuções repetidas. Desabilitar GC
para passar hello não aprova este marco. Nenhum código deve requerer WasmGC/EH.

### W4 — Semântica Dart e bibliotecas portáteis

1. Classes/herança, dispatch, closures/tear-offs e parâmetros nomeados/opcionais.
2. Generics, RTI, `is`/`as`, null safety e casts implícitos/ordem de avaliação.
3. List/Map/Set, iteração, igualdade/hashCode e identidade de constantes.
4. Unicode/UTF-16 Dart, strings, buffers, UTF-8 de fronteira e typed data.
5. JSON/base64 e demais APIs de `convert` selecionadas; matemática e RegExp
   entram apenas com cobertura específica, sem herdar automaticamente o selo.
6. Validar inteiros, shifts, divisão/remainder, overflow, NaN/−0 e conversões
   double→int, inclusive sem instruções saturantes pós-MVP. Definir diferenças
   permitidas entre VM e o backend web antes de interpretar divergências.

**Aceite:** corpus versionado por recurso, testes positivos/negativos e
comparação de saída/efeitos/exceções com oráculo adequado. Publicar cobertura
e diferenças; não medir só o exit code do engine.

### W5 — Dois hosts, mesmo módulo

1. Implementar adaptador JS pequeno, independente do compilador, para browsers.
2. Implementar host de executor, inicialmente apoiado em uma engine incorporável
   como Wasmtime; fixar versão/configuração e não exigir a CLI WASI pura.
3. Definir API de entrada/conclusão, argumentos, códigos de saída e diagnóstico.
4. Tratar atualização de views JS após `memory.grow`, strings e buffers copiados.
5. Validar callbacks, encerramento, capacidade ausente, host que retorna erro,
   import faltante, assinatura errada e manifesto ABI incompatível.

**Aceite:** o **mesmo hash** executa ao menos em Chromium, Firefox e executor
independente; Safari/WebKit é a próxima porta antes da declaração de suporte
à web nessa plataforma. Não compilar separadamente para “provar” mesmo módulo.
Nenhum host fornece privilégios de filesystem/rede por padrão.

### W6 — Async e integração de ambiente

1. Microtasks, Future/Stream e máquinas de estado com raízes vivas.
2. Timers/cancelamento e ordem de eventos, erros assíncronos e encerramento.
3. Testar conclusões síncronas/atrasadas, cancelamento tardio e IDs obsoletos.
4. Adicionar I/O opt-in por capacidade: primeiro operações assíncronas pequenas,
   depois rede/arquivos virtuais segundo contratos separados.
5. Avaliar isolates com instâncias/heaps separados e cópia de mensagens.
   Threads/shared-memory não são introduzidos no perfil MVP para imitá-los.

**Aceite:** scheduler determinístico de teste e oráculo real, sem reentrada
durante GC, sem busy loop quando ocioso e sem bloquear o event loop do browser.
Programas CPU-bound permanecem síncronos até existir execução em worker ou outra
política explícita; event loop não torna laços computacionais preemptivos.

### W7 — CLI, distribuição e homologação

Sintaxe **proposta, ainda inexistente**:

```sh
dartforge compile-wasm app.dart --profile mvp --sdk /path/to/dart-sdk/lib \
  --packages .dart_tool/package_config.json -o build/app.wasm
```

Preferir um comando específico para não sobrecarregar conceitos de executável
nativo, sysroot e biblioteca dinâmica. Ele usará os componentes comuns do backend
e selecionará os perfis de forma explícita. Nome/opções finais devem ser fixados
no começo de W1, incluindo limites de memória e diagnóstico de recursos recusados.

Entregar `.wasm`, manifesto de ABI/features/capacidades e loader/adaptador
selecionado, com documentação de implantação. O manifesto registra hashes do
módulo, SDK/overlays, runtime, compilador, toolchain e opções. Definir nomes sem
ambiguidade para fonte de debug/mapas e separar artefatos de release/debug.

Validar construção/distribuição do compilador nos três hosts existentes,
reutilizando as VMs Parallels **uma por vez**, sem build/JIT pesado concorrente
nos hóspedes de 4 GB. Executar o mesmo corpus Wasm em cada executor suportado.
Não exigir igualdade binária entre hosts antes de controlar timestamps, caminhos
e versões das ferramentas; a porta de portabilidade do mesmo módulo usa cópia
e hashes do artefato efetivamente distribuído.

**Aceite:** instalação limpa compila e executa exemplos documentados; nenhum
caminho de desenvolvimento ou cache antigo é requisito escondido. Artefatos
inválidos/falhos não substituem a última saída válida nem retornam exit 0.
Antes de declarar suporte a Safari/WebKit, incluir sua execução real no mesmo
corpus e registrar a versão; até lá, identificar nominalmente os browsers validados.

## 5. Plano de testes e evidências

| Camada | Verificação obrigatória |
|---|---|
| Binário | Validator MVP estrito no módulo final, debug/release/LTO; imports/exports permitidos |
| ABI/layout | Rust↔LLVM↔host; limites, alinhamentos, i64 por buffer, função indireta, overflow |
| Runtime | Heap/allocator, GC forçado, erros, callbacks, duas instâncias e encerramento |
| Linguagem | Corpus com oráculos fixados e expectativas de efeitos/exceções; recusas explícitas |
| Bibliotecas | Cobertura por API de core/collection/typed_data/convert/async e recursos opcionais |
| Integração | Browser + executor com o mesmo hash; erros de import/capacidade/versão |
| Regressão nativa | Testes/contratos existentes nos cinco destinos, AOT/JIT desktop nos três SOs |
| Distribuição | Toolchain/cache limpos, relocação, ausência de dependências de máquina |

Adicionar controles negativos que insiram deliberadamente módulos com extensão
proibida, import desconhecido, assinatura incompatível, layout errado e manifesto
obsoleto. O gate precisa falhar nesses casos, não só aceitar artefatos válidos.
Conservar também resultados inesperados e contagem de casos não executados.

Fixar lista de corpus, hashes de fontes/configuração/dependências e versões dos
oráculos. Comparar saídas normalizadas somente nos campos não determinísticos
declarados, sem apagar mensagens/exceções relevantes. Timeouts são falhas ou
limitações identificadas, nunca testes aprovados por ausência de resposta.

Os checks de CONTRIBUTING continuam obrigatórios para implementação: fmt,
Clippy estrito, workspace com ignorados, CLI native/JIT separada e documentação.
A falha já existente de poda de backends estrangeiros não vira aprovação por
este plano; registrar o estado anterior e qualquer nova regressão separadamente.

## 6. Medições depois da correção

Separar cinco custos: compilação Dart→Wasm, validação/compilação pelo engine,
instanciação, primeira execução e execução aquecida. Não comparar startup de
um engine com throughput aquecido de outro.

- Compilação: processo novo e residente, quando existir; caches frios/aquecidos
  discriminados; mesmo corpus/dependências; medições alternadas e várias amostras.
- Tamanho: `.wasm` bruto/gzip/Brotli, loader/runtime JS, manifesto e total
  necessário ao primeiro carregamento; debug e release separados.
- Memória: páginas lineares/livres/vivas, pico e crescimento, memória do host
  e limite configurado. Memória linear reservada não equivale a objetos vivos.
- Execução: CPU-bound, strings/Unicode, JSON, coleções/alocação, RTI/dispatch,
  exceções e async. Conferir checksums antes de aceitar tempos.
- Comparadores: JS dev/produção próprios, DDC/dart2js com contratos identificados,
  AOT nativo e, opcionalmente, Dart→Wasm oficial quando compatível com o corpus.
  Esse último pode depender de features fora do MVP e não constitui comparação
  de requisitos de execução equivalentes.

Não prometer ganho de velocidade ou tamanho antes dessas medições. Otimizações
de layout/GC, LTO, inlining, tabelas e RTI só entram após W3–W5, com ablação,
correção sob GC e inspeção de features da saída final.

## 7. Riscos e decisões que bloqueiam os marcos

| Risco | Tratamento/porta |
|---|---|
| Rust/libs introduzem features modernas | Reconstruir dependências no perfil; validator final obrigatório em W0 |
| Runtime demasiado acoplado a std/SO | Inventário de fragmentos e núcleo portátil em W0/W2, sem stubs silenciosos |
| Layout host64 vaza para Wasm32 | Descritor por destino e fixtures independentes antes de objetos reais |
| Raiz perdida ou callback reentrante | Shadow stack, contratos de efeitos, GC forçado e host controlado |
| ABI LLVM/Rust para agregados diverge | Assinaturas escalares/buffers e testes bidirecionais; sem assumir C nativo |
| Heap Rust/Dart/stack colidem | Mapa de memória e allocator coordenado, limites e testes de crescimento |
| Stack overflow/panic vira trap irrecuperável | Contratos de erro separados e limitação documentada até prova |
| Bibliotecas arrastam FFI/I/O do SDK desktop | Overlays Wasm, capacidades e seleção de fontes no cache |
| “Mesmo módulo” depende de WASI ou DOM | ABI comum explícita, adapters separados e mesma cópia/hash nos testes |
| LTO reintroduz instrução proibida | Verificar o artefato ligado, não só IR/objetos intermediários |

Cada decisão de ABI/layout/features é registrada antes dos consumidores e
versionada no manifesto/cache. Não estimar prazo fechado antes de W0: a maior
incerteza é o isolamento do runtime, não a existência do backend Wasm do LLVM.
Depois de W0, estimar pacotes de trabalho por dependência e tamanho, com revisão
do escopo quando houver evidência nova.

## 8. Ordem recomendada e fora do primeiro escopo

Caminho crítico: **W0 → W1 → W2 → W3 → W4/W5 → W6 → W7**.
A especificação dos hosts começa em W1; implementações de W5 só se estabilizam
após o ABI/GC de W3. Packaging e CI podem ser preparados progressivamente,
mas não antecipam a homologação final.

Prioridade inicial: módulo escalar → núcleo de heap → Dart hello real com GC/exception check →
corpus portátil → mesmo binário browser/executor → async → distribuição.

Fora da primeira entrega: LLVM JIT dentro do browser, hot reload equivalente
ao JIT desktop, dynamic linking Wasm, WASI Component Model, threads compartilhadas,
SIMD, WasmGC, FFI nativo arbitrário e execução do explorador desktop completo.
Um futuro backend dart_ui para canvas/WebGL/DOM requer plano próprio; o
backend Win32/AppKit/Wayland existente não se torna web ao gerar Wasm.

## 9. Referências e natureza das decisões

Fontes baixados, ordem de leitura e relação com W0–W7:
[referências locais Wasm MVP](REFERENCIAS-WASM-MVP.md), em
`/Volumes/ssd_dock/references`. O guia distingue clones novos e snapshots
anteriores, com revisões e hashes em manifesto próprio.

Os contratos da implementação atual vêm dos arquivos locais vinculados na seção
2. As fases, nomes de comandos, ABI v1 e escolhas iniciais de runtime deste
documento são **propostas de engenharia**, não recursos existentes.

- [WebAssembly: portabilidade e imports](https://webassembly.org/docs/portability/):
  o módulo depende das APIs fornecidas pelo host, não de syscalls universais.
- [Rust: wasm32-unknown-unknown](https://doc.rust-lang.org/stable/rustc/platform-support/wasm32-unknown-unknown.html):
  limitações de std, evolução das features padrão e cuidados para MVP estrito.
- [Histórico de mudanças da especificação](https://webassembly.github.io/spec/core/appendix/changes.html):
  distinguir o conjunto original das extensões posteriores.

Referências externas consultadas em 20/10/2026. As versões efetivamente usadas
na implementação/testes deverão ser fixadas e registradas em W0, sem depender
de documentação mutável como definição única do perfil.
