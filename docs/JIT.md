# JIT ORCv2 — execução em memória do subconjunto nativo

O DartForge passa a ter dois perfis de execução nativa sobre **o mesmo LLVM IR**:

| | Desenvolvimento (`crates/jit`) | Produção (`crates/native`) |
| --- | --- | --- |
| Comando | `dartforge run <entrada.dart>`, `dartforge reload <entrada.dart> [--preservar-estado]` | `dartforge aot <entrada.dart> <saida.exe>` |
| Geração de código | ORCv2 (`LLJIT`), em memória | Clang, em processo separado |
| Runtime nativo | endereços das funções Rust publicados como símbolos absolutos | `rustc` compila `RUNTIME_MAIN` e o linker resolve os símbolos |
| Artefato | nenhum | executável no disco |
| Isolamento do programa | **nenhum**: executa no processo do compilador | processo próprio |

O contrato entre os dois é observável e testado: `crates/jit/tests/execucao.rs`
compila um programa Dart uma única vez, executa o IR resultante pelos dois
caminhos e exige saída idêntica. Divergir em tempo de compilação é esperado;
divergir em resultado é defeito.
> **Estado em 2026-09-23 — rebase na trilha nova, parte 1.** O crate deixou de
> depender da trilha velha e passou a consumir o IR do `crates/emit_native`:
> runtime publicado a partir da fonte do harness AOT, pré-verificação de
> externos, alvo fixado (`x86-64`, `CodeGenLevelNone`) e o executor isolado
> `dartforge-executar-ir`. Na CLI (`--features jit`): `dartforge run` e
> `dartforge reload` mantém R0 por padrão (reinício a quente, estado NÃO
> preservado; cada geração num processo `dartforge run --ir`). Com
> `--preservar-estado`, publica as gerações numa `JitSession` R1 e chama a entrada
> na mesma thread: estáticos e heap permanecem vivos, inclusive com a DLL do
> SDK da fonte. No harness: `--jit` e
> `--jit-aot`.
> Runtime de fonte única (`dartforge_runtime::abi`) e sessão persistente com
> cache de módulos (executor de macros), ver as seções abaixo. As seções «O que
> executa» e «Hot reload» abaixo descrevem a trilha velha; o mecanismo de
> `src/reload.rs` continua, mas os testes dele esperam migração em
> `crates/jit/testes-pendentes/`. O plano completo está no plano do JIT
> (passos 1–13).
>
> **O caminho de execução (`run_ir`, `dartforge-executar-ir`, `dartforge run` e
> `dartforge reload` R0) não passa por `src/reload.rs`.** O módulo entra
> por `add_ir_module`, sem trampolim nem célula: as chamadas são diretas, como
> no AOT. Isso é contrato, não detalhe. O trampolim acrescenta um quadro por
> chamada, e a profundidade de recursão divergiria do executável AOT
> (`docs/PESQUISA-HOT-RELOAD.md` §4.3). A indireção de R1 será uma célula lida
> no ponto de chamada, emitida só no perfil recarregável.


## O que executa

Exatamente o subconjunto que `dartforge_compiler::compile_llvm_with_options`
aceita — nem mais, nem menos. O JIT não tem emissor próprio: ele recebe o IR
textual pronto. Na prática isso cobre `int`/`bool`/`String`, classes com campos,
métodos e despacho virtual, enums, controle de fluxo e `print`, com o coletor
preciso de `crates/runtime` participando por frames de raízes explícitas.

O que o emissor nativo recusa continua recusado, com a mesma mensagem: `double` e
`num`, coleções, genéricos reificados, funções como valores, records, `Duration`
e `Timer`. Isso é limite de `crates/llvm`, não deste crate.

## O que não executa

* **`@Native` de `dart:ffi`.** O AOT resolve esses símbolos ligando objetos
  nativos passados em `--link-object`. A sessão JIT define apenas os nomes de
  [runtime](#como-o-runtime-é-ligado); um `@Native` não resolvido vira erro na
  etapa `lookup`, não uma chamada para endereço errado.
* **Dois programas completos na mesma sessão.** Todo programa nativo define
  `@dartforge_entry`, e cada nome existe uma única vez na `JITDylib`. O segundo
  `add_ir_module` devolve erro na etapa `add-module`. Um programa por sessão, ou
  módulos com símbolos distintos — como no teste `two_modules_share_one_session`.
* **Execução em outra thread.** O heap gerenciado é `thread_local`, como no
  harness AOT. `JitSession` contém ponteiros crus e por isso não é `Send` nem
  `Sync`: o compilador impede o engano em vez de deixá-lo virar heap vazio em
  tempo de execução.
* **Substituição de quadros ativos.** O hot reload existe (veja
  [Hot reload de comportamento](#hot-reload-de-comportamento)), mas uma chamada
  já iniciada termina no corpo em que entrou. Quem está no meio de um laço
  continua no laço antigo.

### Falhas que derrubam o processo hospedeiro

É a consequência direta de não haver isolamento, e vale registrar explicitamente:

* `dartforge_null_assert_fail` escreve `Null check operator used on a null value`
  em stderr e encerra o processo com código 101 — idêntico ao AOT, exceto que no
  JIT o processo encerrado é o do próprio compilador.
* Um erro **interno** do runtime (handle inválido, tipo errado num campo) chega
  como `panic` dentro de uma função `extern "C"`, o que aborta o processo. A
  alternativa seria desenrolar por quadros de pilha gerados pelo LLVM, que não
  sabem desenrolar. Abortar é o final definido; `crates/runtime/CONTRACT.md` já
  trata esses casos como erro de compilador, não como exceção Dart.

Nenhuma dessas é erro *esperado*. Todo erro esperado — IR inválido, símbolo
ausente, símbolo duplicado, falha ao descarregar módulo — vira `Result` com
`JitError { stage, message }` em português, com o diagnóstico original do LLVM
anexado. Não há `panic` em caminho de erro esperado.

## Como o runtime é ligado

A regra é **uma fonte só**: as funções `dartforge_*` que o código JIT chama são
as de `crates/runtime/src/runtime_main.rs`, as mesmas que o driver AOT compila
com `rustc` avulso e liga ao executável. Nenhuma delas é reimplementada aqui.

O arquivo é compilado de duas formas (`crates/runtime/src/lib.rs`):

1. **pelo AOT**, como texto (`RUNTIME_MAIN`), com `rustc -O` avulso, virando a
   `.lib` que o executável liga. Com o `main` C que chama `@dartforge_entry`;
2. **pelo crate `dartforge-runtime`**, como o módulo `dartforge_runtime::abi`,
   que o JIT usa. O `build.rs` do crate liga a cfg `dartforge_runtime_embutido`,
   que tira o `main` C e a declaração de `dartforge_entry` (colidiriam com o
   `main` de qualquer binário Rust). Ele também gera `dartforge_runtime::simbolos`:
   a tabela `(nome, endereço)` de todo `#[unsafe(no_mangle)]` do arquivo.

O JIT publica essa tabela com `LLVMOrcAbsoluteSymbols`, junto com `_fltused`, o
marcador que o gerador COFF exige quando há `double` e que no AOT vem da CRT
estática. Depois que `dartforge_entry` retorna, os dois perfis rodam o mesmo
`abi::finalizar_programa()`: exceção não capturada → `Uncaught exception: …` e
101; `DARTFORGE_GC_STATS`. É o único trecho do runtime que os perfis dirigem, e
está escrito uma vez só.

O perfil de compilação de `dartforge-runtime` está fixado no `Cargo.toml` raiz
em opt-level 2, sem debug-assertions e sem overflow-checks, que é o que o
`rustc -O` dá ao AOT. Sem isso, a mesma fonte entraria em `panic` por estouro
em `dev`, onde o AOT faz aritmética modular. `crates/runtime/tests/fonte_unica.rs`
confere três coisas: que o texto do AOT contém os mesmos dois arquivos, que a
tabela tem cada símbolo do arquivo e que estouro é modular no perfil de teste.

O gerador de símbolos do processo **não** é a fonte do runtime. Um `.exe`
Windows não exporta os `#[no_mangle]` das bibliotecas Rust que liga. Além
disso, o gerador resolveria nomes homônimos por acaso. Antes de entregar um
módulo ao LLVM, a sessão confere cada nome **declarado** no IR. Ele tem de
estar na tabela do runtime, em `CRT_SYMBOLS` (cópia de memória e matemática de
`double`), ser `llvm.*` ou ser definido por um módulo residente. Se não
estiver, o módulo é recusado na etapa `símbolos`, com o nome na mensagem.
Símbolos que o próprio gerador de código introduz sem aparecer no IR
(`__chkstk`, `memcpy` de intrínsecos) são resolvidos pela `LLJIT` no processo.

O teste `cada_extern_do_emissor_existe_no_runtime` confere, sem LLVM nem Clang,
que toda extern da tabela do emissor (`emit_native::llvm::externs::EXTERNS`) está na do runtime.


### Execução e término

A entrada é chamada numa thread nova com 1 MiB de pilha, a reserva padrão da
thread principal de um `.exe`. Depois vem `finalizar_programa()`, o mesmo código
que o `main` C do AOT roda depois da entrada. Todo o estado do runtime é
`thread_local`, então cada execução começa com heap, classes registradas e
exceção pendente zerados. As globais mutáveis do módulo, que são os estáticos
preguiçosos `@dfg_*`, voltam a zero antes de cada execução (ver «Sessão
persistente»).

`finalizar_programa()` devolve 101 para exceção não capturada, e o executor sai
com esse código. Durante a execução, o runtime ainda encerra o processo com
`process::exit` nos mesmos casos que o AOT: asserção de não nulidade (101) e
teto do heap (255). Por isso testes e harness executam por subprocesso, com
`dartforge-executar-ir <programa.ll> [--timings]`: stdout e código de saída
são os do programa, e uma falha do próprio JIT sai com 70. A captura de saída
em processo da versão anterior foi removida, porque exigia uma segunda cópia
das funções de impressão.

O JIT roda o **verificador do LLVM** (`LLVMVerifyModule`) em todo módulo, como
o Clang faz no AOT. Nem o analisador de IR textual nem o ORC o rodam. Sem ele,
o JIT executava IR que o AOT recusa: no corpus inteiro foram 13 programas, com
`phi` sem entrada para cada predecessor, instrução que não domina o uso e
valor indefinido.

Divergência conhecida: estouro de pilha. No AOT o processo morre com
`STATUS_STACK_OVERFLOW`. No JIT, a thread Rust tem o tratador do `std`, que
imprime `thread '<unnamed>' has overflowed its stack` e aborta.

## Sessão persistente e cache de módulos

O executor de macros e builders vai ser persistente (regra de `PLANO.md`): um
processo por sessão, atendendo várias execuções, com o código compilado uma vez
e em cache, e custo zero para quem não usa. A biblioteca já tem as peças.

* **`JitSession`** vive o quanto o chamador quiser. O LLVM é inicializado uma
  vez por processo, e a sessão uma vez.
* **`compile_module(nome, ir) -> CompiledModule`** analisa, verifica e compila o
  IR para objeto COFF, com a mesma máquina-alvo da sessão, **sem** abrir sessão.
  `CompiledModule::object()` dá os bytes para um cache em disco. A chave fica
  com o chamador: hash do IR, versão do LLVM e do runtime.
* **`JitSession::add_compiled_module(&CompiledModule)`** carrega o objeto sem
  análise nem geração de código, com as mesmas verificações do IR (alvo e
  externos). O mesmo módulo compilado entra em quantas sessões quiser.
* **`run_entry()`** pode ser chamado muitas vezes. Cada execução começa com o
  estado Dart limpo: thread nova (heap e tabelas do runtime zerados) e globais
  mutáveis do módulo zeradas antes de executar. Para achá-las, a sessão muda a
  ligação delas de `internal` para externa **na cópia** do módulo, sem mudar o
  IR emitido. Global que não começa em zero é recusada na etapa `globais`, em
  vez de ser reiniciada errado.
* **`run_main()` com SDK da fonte** cria uma thread nova e zera as globais
  mutáveis dos módulos JIT do programa, inclusive da geração recarregável
  ativa, antes de cada chamada. Os globais recebem nomes distintos por módulo
  e geração na `JITDylib` compartilhada; só os da geração publicada são
  reiniciados. O teste de
  regressão executa `main` duas vezes na mesma sessão sem carregar a DLL do
  SDK. Os estáticos internos da DLL do SDK não são exportados nem reiniciados
  por esse caminho e continuam persistentes. A reutilização de uma sessão com
  SDK da fonte ainda não garante estado completamente limpo entre execuções.

Custo medido em 2026-09-23, `cargo test -p dartforge-jit --test
sessao_persistente medicao -- --ignored --nocapture`. Perfil `test` (o Rust sem
otimização; o LLVM é a DLL otimizada), 30 amostras depois de 3 aquecimentos,
máquina local com 2,4 GB livres e nenhum outro build rodando.

| peça | trivial (46 B de IR) | programa Dart pequeno (12,5 KB de IR) |
| --- | --- | --- |
| (i) criar a sessão (LLJIT + 150 símbolos) | 0,27 ms (p95 0,65) | — |
| (ii) carregar pelo IR: análise + verificação + geração, até a entrada resolvida | 0,36 ms (p95 1,0) | 3,37 ms (p95 6,2) |
| compilar para o cache (uma vez por módulo) | 0,29 ms | 2,91 ms (p95 4,7) |
| (ii) carregar do cache, até a entrada resolvida | **0,015 ms** (341 B de objeto) | **0,23 ms** (3,2 KB de objeto) |
| (iii) uma execução numa sessão aquecida (thread nova + finalização) | **0,07 ms** (p95 0,19) | **0,17 ms** (p95 0,5) |

Leitura:

* Com o módulo no cache, uma execução custa décimos de milissegundo. O que
  pesa é o **processo**. No CI, o executor isolado leva 43 ms por programa,
  entre criar o processo e carregar a `LLVM-C.dll` de 72 MB (ver «JIT × AOT no
  corpus»). É isso que a sessão persistente elimina.
* O cache paga 15× no programa pequeno (3,4 ms contra 0,23 ms). A geração de
  código cresce com o IR, e o carregamento do objeto quase não cresce.
* A thread nova por execução (~50 µs) é o preço do estado limpo sem `reset`
  manual no runtime.

## JIT × AOT no corpus inteiro

`dartforge-diferencial --jit-aot` emite o IR de cada programa **uma vez**. Esse
IR passa pelo executor do JIT e pelo AOT (Clang + ligação), e o harness compara
os dois programa a programa (stdout e código). Qualquer divergência é defeito.
No CI é o job `jit` do `pesado.yml`: corpus inteiro, `--jobs 1`, cache de
objetos desligado para o Clang rodar sempre. O relatório só é aceito com
`0 divergentes`.

Primeira rodada (Pesado 35823438137): 14 divergências. Treze eram IR que o
Clang recusa e o JIT executava, e foram resolvidas com o verificador (acima). A
outra era um programa que estoura o tempo nos dois perfis com stdout parcial
diferente. Estouro nos dois agora é listado à parte, e não como divergência.

Segunda rodada (Pesado 35824630444, runner `windows-latest`): **222/222
idênticos, 0 divergentes**. Nela, 32 programas não geram IR e 6 estouram o
tempo nos dois perfis. O placar contra a VM é 7/222 nos dois perfis, com o
emissor de antes do contrato de representação. Tempos pós-IR dos 41 programas
que terminaram nos dois perfis:

| etapa | mediana | p95 | soma |
| --- | --- | --- | --- |
| JIT até executar (processo + DLL + sessão + IR + geração) | 42,6 ms | 43,4 ms | 2,6 s |
| AOT Clang + ligação | 156,1 ms | 207,4 ms | 18,1 s |
| JIT execução do programa (dentro do processo) | 0,1 ms | 0,4 ms | 0,0 s |
| AOT execução do `.exe` (processo inteiro) | 25,4 ms | 26,0 ms | 1,0 s |
| **JIT total** | **42,7 ms** | 43,6 ms | 2,6 s |
| **AOT total** | **181,5 ms** | 232,6 ms | 19,1 s |

Pelas somas, Clang + ligação custam **6,9×** o JIT até executar, e o AOT total
**7,3×** o JIT total.

Terceira rodada, com o contrato de representação do nativo e o runtime de fonte
única (Pesado 35827208951): **222/222 idênticos, 0 divergentes**, e o placar
contra a VM é **50/222 nos dois perfis**, igual ao do `--nativo`. Nela, 3
programas não geram IR e nenhum estoura o tempo. Nos 54 que terminaram nos dois
perfis, o JIT até executar levou 42,7 ms (p95 43,2) e o Clang + ligação
165,2 ms (p95 182,1), medianas. Pelas somas, **8,1×** até executar e **8,6×**
no total (AOT 23,8 s, JIT 2,8 s). Quase todo o tempo do JIT é criar o processo e carregar a
DLL: a sessão persistente leva esse número a décimos de milissegundo por
execução.

## Ciclo de vida e liberação

Cada módulo adicionado recebe um `LLVMOrcResourceTracker` próprio
(`LLVMOrcJITDylibCreateResourceTracker`), guardado junto do nome do módulo.
`JitSession::remove_module(i)` chama `LLVMOrcResourceTrackerRemove`, que
descarrega o código daquele módulo.

> **Remover só é seguro quando nenhuma função daquele módulo está executando**,
> em nenhuma thread e em nenhum quadro de pilha abaixo do chamador. O LLVM não
> verifica isso: o que era código vira memória liberada, e chamar um endereço
> resolvido antes da remoção é uso de memória liberada.

A assinatura carrega a parte da garantia que Rust consegue dar: `remove_module`
exige `&mut self` e `run_entry` toma `&self`, então uma execução em curso e uma
remoção não coexistem *nesta* sessão. O que o compilador não impede é a remoção
feita de dentro de uma chamada ao código gerado — por exemplo, a partir de uma
função de runtime invocada pelo programa.

O hot reload não precisa dessa garantia manual, e é por isso que ele **não
remove** nada: trocar o ponteiro de uma entrada estável é uma escrita de palavra,
não um descarregamento. A geração antiga fica retida justamente porque provar que
ela não está em nenhum quadro de pilha é o que a sessão não sabe fazer.

Os handles do heap **não** são liberados pela remoção: o heap pertence à thread,
não ao módulo, e sobrevive ao descarregamento.

A ordem de destruição também é contrato: `JitSession` declara o vetor de módulos
**antes** do campo da `LLJIT`, porque os campos são destruídos na ordem de
declaração e liberar um `ResourceTracker` depois de destruir a `LLJIT` que o
criou seria uso de memória liberada.

## Hot reload de comportamento

O laço de desenvolvimento não é «compilar e executar»: é **editar com o processo
vivo**. O que o hot reload troca é o corpo das funções; o que ele preserva é o
heap gerenciado, e é essa preservação que o distingue de reiniciar o processo.

### O mecanismo: identidade estável, implementação substituível

Cada função recarregável tem três coisas distintas:

| Peça | Nome | Vida |
| --- | --- | --- |
| **Entrada estável** (trampolim) | `df_fn_0` | permanente; nunca descarregada |
| **Célula de ponteiro** | `__dfslot$df_fn_0` | permanente; pertence ao Rust |
| **Implementação** | `df_fn_0$gen3` | uma por geração, sob `ResourceTracker` próprio |

O trampolim é uma função LLVM de três instruções: lê a célula e repassa a
chamada.

```llvm
@__dfslot$df_fn_0 = external global ptr
define i64 @df_fn_0() {
entry:
  %alvo = load ptr, ptr @__dfslot$df_fn_0
  %r = call i64 %alvo()
  ret i64 %r
}
```

A célula é um `Box<AtomicUsize>` da sessão, publicado na `JITDylib` como símbolo
absoluto de **dado** (`LLVMOrcAbsoluteSymbols` sem a flag `Callable`). Recarregar
é escrever nela.

Ao entrar na sessão, o módulo de uma geração é reescrito: cada função definida é
renomeada para `nome$genN` e **todos** os usos dela — inclusive os de dentro do
próprio módulo e as recursões — passam a apontar para uma declaração externa com
o nome estável. Consequência direta: as chamadas entre funções do programa também
atravessam os trampolins.

Vale ser preciso sobre o que isso compra hoje. Como `hot_reload` toma
`&mut self` e executar toma `&self`, **uma recarga não pode acontecer com código
gerado na pilha**, então o caso «quadro antigo faz uma chamada nova» não ocorre
nesta versão. A indireção das chamadas internas não é, portanto, observável por
teste agora: ela existe para que a regra de visibilidade continue valendo quando a
recarga for disparada de fora do fluxo — de um observador de arquivos em outra
thread, ou de dentro de uma função de runtime chamada pelo programa. Sem ela, esse
passo futuro precisaria reescrever call sites já materializados; com ela, é uma
escrita de palavra. O que está testado é o caso que importa agora: um endereço de
entrada estável capturado antes da edição executa o corpo novo depois dela.

### Por que não `LLVMOrcCreateLocalIndirectStubsManager`

`PLANO.md` aponta `LLVMOrcCreateLocalIndirectStubsManager` e
`LLVMOrcCreateLocalLazyCallThroughManager` como as primitivas prontas, e
`llvm-sys 221.1.0` **expõe as duas**. Elas não resolvem o problema, e a razão está
na lista de símbolos que a API C publica: de `IndirectStubsManager` existem apenas
`LLVMOrcCreateLocalIndirectStubsManager` e `LLVMOrcDisposeIndirectStubsManager`.
Não há entrada C para `createStub` nem para **`updatePointer`** — que é exatamente
a operação de uma recarga. O único consumidor exposto é `LLVMOrcLazyReexports`,
que produz *aliases preguiçosos*: o stub resolve na primeira chamada e o ponteiro
vale para sempre; redefinir o mesmo alias numa geração seguinte é definição
duplicada, isto é, erro.

Ficou o caminho que o próprio plano prevê como alternativa — tabela de ponteiros
mutáveis com salto indireto —, com uma diferença: o trampolim é **IR gerado pela
sessão**, não assembly por arquitetura. Vale em qualquer alvo do LLVM e não abre
um segundo emissor.

**Custo declarado:** um `load` e um quadro de pilha por chamada de função
recarregável. Não existe em `add_ir_module` (caminho não recarregável) nem no AOT.

### Gerações e `JITDylib`

`PLANO.md` pede «um `JITDylib`/tracker próprio» por recarga. O tracker é próprio
de fato — um `LLVMOrcResourceTracker` por geração. O `JITDylib` **não**, e a razão
é a mesma de antes: a API C cria dylibs
(`LLVMOrcExecutionSessionCreateJITDylib`) mas não expõe nenhum `SetLinkOrder`, de
modo que uma dylib nova nasce sem ordem de ligação e não alcançaria os 18 símbolos
de runtime nem os trampolins — o código da geração simplesmente não ligaria. A
separação de nomes que a dylib daria vem do sufixo `$genN`; o descarregamento por
geração vem do tracker, que é o que o LLVM oferece para isso.

### Publicação transacional em três fases

1. **Preparar**, sem tocar no que está executando: analisar o IR novo, ler a
   impressão digital do contrato e compará-la com a da versão viva, conferir que
   toda referência externa é resolvível (inclusive pelas exportações da DLL do
   SDK da fonte carregada nesta sessão), recusar nomes de função já ocupados por
   outro módulo, resolver as globais externas de dados antes da promoção,
   versionar as implementações e entregar o
   módulo à `LLJIT` sob um tracker novo.

   Na sessão com SDK da fonte, só os exports usados pela primeira geração são
   publicados na abertura. Se uma recarga introduz outro membro exportado,
   o JIT consulta `exportados.def` e publica somente esse nome antes de ligar
   a geração nova; um nome ausente recusa a recarga na etapa `contract`.
   Os exports adicionais já publicados permanecem na sessão mesmo se uma fase
   posterior falhar, mas nenhuma entrada estável muda antes de a geração nova
   estar ligada.
   Um trampolim órfão, criado por uma tentativa anterior que falhou na ligação,
   não satisfaz referências de outros módulos até receber uma implementação
   publicada; a célula dele ainda contém ponteiro nulo.

2. **Publicar**: materializar as implementações (aqui é onde o código nativo é
   gerado e ligado em memória) e **só então** escrever os novos ponteiros nas
   células.
3. **Aposentar** o código antigo — que nesta versão não acontece; veja a política
   de retenção.

A garantia que importa: **uma falha de análise, de contrato ou de ligação não
destrói a versão que está funcionando**. Falha na fase 1 devolve `Err` sem ter
tocado em nada. Falha na materialização descarrega a geração recém-adicionada —
remoção demonstravelmente segura, porque nenhuma célula aponta para ela e nada
pode tê-la chamado — e as entradas da versão anterior continuam apontando para
os mesmos corpos.

`crates/jit/tests/hot_reload.rs::falha_de_recarga_nao_destroi_a_versao_boa` cobre
IR inválido e referência não resolvível, e
exige que a versão boa continue executando e que uma recarga válida ainda
funcione depois delas.
`cinco_recargas_preservam_entrada_estavel` exercita cinco publicações
consecutivas, conferindo identidade do trampolim e contagem de gerações
retidas; não mede vazamento, pois a retenção é deliberada nesta versão.

### A única janela não transacional: promoção

Um módulo incorporado por `add_ir_module` define os nomes estáveis ele mesmo. Para
que o módulo de trampolins possa definir `df_fn_0`, o módulo simples precisa ser
descarregado primeiro — e esse é o único ponto destrutivo do hot reload. Ele
acontece **uma vez**, na primeira recarga de um módulo simples, depois de todo o
trabalho que pode falhar por causa do código novo (análise, contrato,
referências), e o relatório o marca em `promoted: true`. Se algo falhar dentro
dessa janela, a sessão é **envenenada**: as recargas seguintes recusam com a etapa
`poisoned` e uma mensagem que diz para reiniciar, em vez de deixar o chamador
descobrir por corrupção.

Para não ter janela nenhuma, comece pelo caminho recarregável:

```rust
let mut sessao = JitSession::new()?;
sessao.add_reloadable_module("app", &ir)?;   // geração 1 já nasce atrás dos trampolins
sessao.hot_reload("app", &ir_novo)?;         // transacional de ponta a ponta
```

O nome de `hot_reload` deve corresponder ao nome informado na carga inicial.
Um nome desconhecido devolve erro de contrato; a sessão nunca promove o único
módulo ativo por aproximação. A primeira geração de um módulo novo entra por
`add_reloadable_module`.

### O laço pela linha de comando

Sem `--preservar-estado`, `dartforge reload` mantém o R0: recompila a cada
mudança e recomeça `main` em outro processo, sem preservar o estado.

`dartforge reload app.dart --preservar-estado` observa os arquivos `.dart` do
diretório da entrada. A primeira versão entra por `add_reloadable_module`; cada
edição válida entra por `hot_reload`. A CLI executa `dartforge_entry` na **mesma
thread**, sem zerar globais ou recriar o runtime, e o teste
`crates/cli/tests/reload_estado.rs` confirma um contador estático (`1 → 11`) e
uma lista no heap (`1 → 2`) depois de editar o mesmo arquivo. Falhas de compilação
ou publicação mantêm a geração anterior; `--timings` relata emissão, recarga e
quantidade de gerações retidas.

Este é o primeiro aceite R1 da CLI, com limites explícitos: cada edição **torna
a chamar `main`**, enquanto a Dart VM não o reexecuta; programas que dependem de
um `main` que fica ativo, de uma thread diferente ou de `process::exit` ainda
precisam do R0. Com `DARTFORGE_SDK_DA_FONTE=1`, a sessão carrega a DLL indicada
por `DARTFORGE_SDK_DLL`, chama o `main` gerado via trampolim e conserva o
runtime da DLL na mesma thread. As versões devem manter o caminho da biblioteca e o
contrato das entradas, pois o nome do arquivo participa dos símbolos emitidos.

### Regra de visibilidade

Chamadas já iniciadas terminam no corpo antigo; chamadas novas usam a
implementação nova, inclusive quando partem de um quadro de uma geração antiga.
Substituir quadros ativos está **fora de escopo**.

### Política de retenção de memória

**Nenhuma geração é liberada antes do fim da sessão.** Provar que é seguro
descarregar uma geração exigiria saber que nenhuma função dela está em nenhum
quadro de pilha de nenhuma thread; a sessão não tem essa informação — o código
gerado não publica safepoints e o LLVM não verifica nada disso. Descarregar sem
essa prova transformaria código em memória liberada debaixo de um `call` em
andamento.

O custo é **linear no número de recargas**: cada recarga retém o código nativo e
as constantes daquela geração. Um laço de desenvolvimento longo cresce em memória
até o processo ser reiniciado. `JitSession::retained_generations()` expõe o
contador, e `HotReloadReport::retained_generations` o repete em cada recarga, para
que o crescimento seja observável e não uma surpresa. A única remoção que acontece
é a de uma geração que falhou antes de ser publicada.

Isto é um compromisso da versão 1, não uma afirmação de que liberar é impossível:
liberar exige safepoints no código gerado, que ainda não existem.

### Escopo da versão 1 e as mensagens exatas

Só mudança de corpo com contrato compatível. O resto é recusado na etapa
`contract`, **sem alterar a sessão**, com estas mensagens:

| Situação | Mensagem (etapa `contract`) |
| --- | --- |
| Assinatura alterada | `a assinatura de df_fn_0 mudou de i64 () para i64 (i64); mudança de contrato de chamada exige reiniciar a sessão` |
| Função que desaparece | `a função df_fn_0 existe na versão em execução e não existe no código novo; a entrada estável dela ficaria presa no corpo antigo, então a recarga é recusada — reinicie a sessão` |
| Campos de classe | `a classe de id 0 tinha 1 campos e passou a ter 2; os objetos já vivos no heap gerenciado mantêm o layout antigo, então a recarga é recusada — reinicie a sessão` |
| Referência não resolvível | `o código novo chama sqlite3_open, que esta sessão não define; o JIT publica apenas os 18 símbolos de runtime e as entradas estáveis já criadas` |
| Variádica | `a função df_fn_0 é variádica, e o contrato do emissor nativo não prevê variádicas` |
| Sessão envenenada (etapa `poisoned`) | `a promoção do módulo a recarregável falhou em '<etapa>' (<detalhe>); os nomes estáveis já haviam sido descarregados e esta sessão precisa ser reiniciada` |

Limites adicionais, e explícitos:

* **A identidade é o nome do símbolo emitido, e os nomes de `crates/llvm` são
  posicionais** (`df_fn_0`, `df_method_0_0`, `df_new_0`). Inserir ou reordenar
  declarações no Dart renumera os símbolos, e a recarga passa a comparar
  contratos de funções diferentes: `df_fn_0` na geração nova pode ser outra função
  Dart. A recarga continua com contrato compatível e portanto é aceita — o que
  muda de corpo é outra coisa. Identidade estável por *declaração Dart* exige o
  front-end incremental do plano, e não este crate.
* **Layout de classe só é conferido para classes que o código novo constrói.** A
  impressão digital vem das chamadas a `@dartforge_object_new(i64 id, i64 campos)`
  com argumentos constantes, que é a forma que `crates/llvm` emite em `@df_new_*`.
  Uma classe que o código novo não instancia não aparece, e não é verificada.
* **Ambiente de closure não é verificado** porque o emissor nativo ainda não tem
  funções como valores; quando tiver, a impressão digital precisa crescer.
* **Métodos de instância não são recarregáveis individualmente.** O que se
  recarrega é o módulo inteiro do programa; o despacho continua passando por
  `@df_dispatch_*`, e o corpo novo do método chega porque `@df_method_*` também é
  uma entrada estável.

### Correspondência com o hot reload da Dart VM

Vale registrar o precedente. **Correção:** a versão anterior deste texto dizia
que os limites acima não eram atalho, porque a implementação oficial teria os
mesmos. Não tem. A VM aceita bem mais do que esta versão aceita (ver
`docs/PESQUISA-HOT-RELOAD.md`), e as recusas acima são degrau nosso.

| Dart VM | Aqui |
| --- | --- |
| O front-end manda um **Dill delta**: só o que mudou | `hot_reload` recebe o IR do **programa inteiro**. É a diferença mais custosa, e a medição abaixo mostra onde ela aparece |
| Localiza a classe no heap e **troca os ponteiros de método**; instâncias não mudam de endereço e os campos ficam intactos | Troca os ponteiros das **células** das entradas estáveis; os objetos do heap gerenciado não são tocados, mesmo handle, mesmos campos. A ideia é a mesma, um nível acima: lá por método de classe, aqui por função |
| Código já promovido ao otimizador é **descartado**, não remendado; a função volta ao tier não otimizado | Não há dois níveis aqui. Se houver, esta é a política a seguir: descartar a geração otimizada e recompilar, nunca remendar código otimizado no lugar |
| **Corrigido** (esta linha dizia que a VM recusa mudança de herança ou de assinatura; o código mostra o contrário, `docs/PESQUISA-HOT-RELOAD.md` §1.6, §1.8 e §1.9). A VM **aceita** mudar assinatura, superclasse e campos. Chamadas pendentes são religadas por nome, e quem chama um membro que sumiu ou mudou de aridade recebe `NoSuchMethodError` **na chamada** (`object_reload.cc:806-841`; `isolate_reload_test.cc:1479-1531`, `:3205-3235`, `SuperClassChanged` `:760-789`). Campo de tipo novo vira `TypeError` na leitura, pela guarda de carga. A lista de recusas é curta: enum ↔ classe, número de parâmetros de tipo, classe `const` que perde campos ou deixa de ser `const`, e campos nativos (`object_reload.cc:351-607`). Mudar o corpo de `main()` ou de `initState()` também é aceito; só não é reexecutado | As recusas da etapa `contract` (assinatura, função removida, campos de classe) são **limitação desta versão, não semântica do Dart**. O alvo (pesquisa §4.4) é célula nova por `(DeclId, abi)` e a célula antiga reescrita para lançar `NoSuchMethodError`, sem recusa |
| Inicializador de global já inicializada **preserva o valor antigo** | As globais do programa (`@dfg.<biblioteca>.<dono>.<nome>` do emissor atual, incluindo o indicador `$ok`; `@dfg_*` e `@df_statics` da IR antiga) são copiadas da geração anterior para a nova antes da troca das entradas estáveis. Globais adicionadas começam em zero; mudança de tamanho de um estático existente é recusada na etapa `contract`. Caches de despacho não são copiados, pois podem conter endereços de código antigo. O estado persiste em chamadas pela `StableEntry` na mesma thread; `run_entry()` e `run_main()` ainda iniciam uma execução isolada e zeram os estáticos dos módulos JIT, por seu contrato de executor. Os estáticos internos da DLL do SDK não são reiniciados por esse mecanismo |

### Medição do ciclo de recarga

`PLANO.md` exige separar as etapas, porque «um backend duas vezes mais rápido na
geração nativa não torna a recarga duas vezes mais rápida quando a geração nativa
é uma fração pequena do total». Este projeto já mediu a mesma aritmética morder no
incremento 26: acelerar o parser em 10× reduziria a compilação total em 6%.

Reproduzir:

```
cargo test -p dartforge-jit --test hot_reload medicao_do_ciclo_de_recarga -- --ignored --nocapture
```

Metodologia de [DESEMPENHO.md](DESEMPENHO.md): mediana e p95 de 20 amostras após
3 aquecimentos, cada intervalo cronometrado no próprio trecho e nenhum obtido por
subtração. A análise do Dart e a geração do IR são medidas pelo teste, porque
`dartforge_compiler::compile_llvm` devolve as duas somadas; o teste confere que o
pipeline que ele monta produz IR idêntico ao da API pública.

Medição de 2026-09-21, perfil `test` (**sem otimização**), Windows 11, LLVM
22.1.8, corpus de uma classe com método, despacho virtual, string, função de topo
e laço — IR de 4.366 bytes, 20 amostras após 3 aquecimentos.

| Etapa | Mediana (µs) | p95 (µs) | % do ciclo |
| --- | --- | --- | --- |
| análise do Dart novo | 221,1 | 323,1 | 5,8% |
| geração do IR | 165,7 | 257,5 | 4,3% |
| análise do IR (`parse-ir`) | 155,0 | 224,4 | 4,0% |
| verificação de contrato | 37,1 | 50,8 | 1,0% |
| adição do módulo ao JIT | 19,3 | 24,8 | 0,5% |
| publicação dos trampolins | 1,5 | 2,0 | 0,04% |
| **ligação em memória** | **3.263,0** | **3.762,1** | **84,9%** |
| publicação dos ponteiros | 3,7 | 5,2 | 0,1% |
| aposentadoria | 0 | 0 | 0% |
| total de `hot_reload` | 3.475,5 | 3.982,2 | 90,5% |
| **ciclo completo** | **3.841,8** | **4.545,5** | **100%** |

> **Estes números estão contaminados.** A máquina tinha entre 16 e 26 processos
> `cargo`/`rustc` de outros agentes compilando durante a coleta, e o teste roda em
> perfil de teste, sem otimização. Trate as medianas como ordem de grandeza, não
> como linha de base limpa: o p95 traz a interferência de escalonamento. A
> **proporção** é o resultado utilizável, e ela é robusta à carga porque todas as
> etapas sofrem a mesma interferência.
>
> Há um viés que a carga não explica e que empurra na direção contrária: a análise
> do Dart, a geração do IR, o `parse-ir` e a verificação de contrato são código
> Rust deste repositório, compilado **sem otimização** no perfil de teste, enquanto
> a ligação em memória acontece dentro da `LLVM-C.dll`, que é otimizada. Em perfil
> `release` as quatro primeiras linhas encolhem e a fatia da geração de código
> cresce ainda mais — o 85% é um piso, não um teto.

O que a proporção diz, e é o oposto do que o incremento 26 mediu para a
compilação fria:

* **A geração de código nativo domina o ciclo** — 85% dele, contra 10% das duas
  fases de front-end somadas. Aqui a aritmética de `PLANO.md` funciona a favor de
  trocar o backend: um gerador de código 10× mais rápido levaria o ciclo de 3,8 ms
  para cerca de 0,9 ms. É exatamente a medição que `PLANO.md` pede antes de
  escolher entre ORCv2 e Cranelift, e ela **não** repete o resultado do parser: lá
  a fase grande era a análise, aqui é a geração.
* **O protocolo de recarga em si é ruído**: trampolins (1,5 µs) e troca de
  ponteiros (3,7 µs) somam 5 µs, 0,1% do ciclo. O mecanismo de identidade estável
  não é o que custa; publicar uma versão nova é praticamente gratuito depois que o
  código existe.
* **A verificação de contrato custa 37 µs**, um vigésimo do front-end. Recusar uma
  mudança incompatível é barato o bastante para ser feito sempre.
* O que **não** está medido aqui é o ganho que a Dart VM tem com o *delta*: a
  recarga recompila o programa inteiro, então as duas primeiras linhas crescem com
  o tamanho do projeto, e não com o tamanho da edição. Num projeto grande a
  proporção muda, e é lá que o front-end incremental do plano entra.


| Campo Rust de `HotReloadReport` | Campo JSON de `dartforge reload --timings` | Intervalo medido |
| --- | --- | --- |
| — | `frontend_ns` | Análise do Dart novo e geração do IR, no chamador |
| `parse_ir` | `parse_ir_ns` | `LLVMParseIRInContext2` do IR novo |
| `contract` | `contract_ns` | Impressão digital do contrato e comparação com a versão viva |
| `add_module` | `add_module_ns` | Versionamento das implementações e `AddLLVMIRModuleWithRT` |
| `stubs` | `stubs_ns` | Células novas e módulo de trampolins (zero quando não há entrada nova) |
| `link` | `link_ns` | **Ligação em memória**: materialização das implementações desta geração |
| `publish` | `publish_ns` | Escrita dos ponteiros nas células |
| `retire` | `retire_ns` | Aposentadoria do código antigo; zero pela política de retenção |
| `total` | `reload_total_ns` | Chamada inteira de `hot_reload` |
| `ir_bytes` | `ir_bytes` | Bytes de IR textual analisados |

## Medição por fase

No mesmo espírito de `LinkStats` e de [DESEMPENHO.md](DESEMPENHO.md): cada
intervalo é cronometrado no próprio trecho, nunca por subtração, e `total` é pelo
menos a soma das fases.

| Campo Rust | Campo JSON de `dartforge run --timings` | Intervalo medido |
| --- | --- | --- |
| — | `frontend_ns` | Carga do grafo, análise e emissão do IR |
| `JitReport::session` | `session_ns` | `LLVMOrcCreateLLJIT` e publicação dos 18 símbolos de runtime |
| `ModuleReport::parse_ir` | `parse_ir_ns` | `LLVMParseIRInContext2` e construção do `ThreadSafeModule` |
| `ModuleReport::add_module` | `add_module_ns` | `LLVMOrcLLJITAddLLVMIRModuleWithRT` |
| `EntryReport::lookup` | `lookup_ns` | `LLVMOrcLLJITLookup`, **incluindo a geração de código sob demanda** |
| `EntryReport::execute` | `execute_ns` | Execução de `@dartforge_entry`, do `call` ao retorno |
| `JitReport::total` | `jit_total_ns` | Sessão inteira, incluindo sua destruição |
| `ModuleReport::ir_bytes` | `ir_bytes` | Bytes de IR textual analisados |

A linha que mais engana é `lookup_ns`. No ORCv2 um módulo só é compilado quando
algum de seus símbolos é procurado: o primeiro `lookup` de uma sessão carrega
toda a geração de código, e `execute_ns` mede apenas a execução do que já está
materializado. Comparar `execute_ns` do JIT com o tempo de processo do AOT seria
comparar coisas diferentes — o número comparável com o AOT é a soma
`session + parse_ir + add_module + lookup + execute`.

Tempos incluem escalonamento do sistema; não são tempos exclusivos de CPU.

## Por que ORCv2 antes de Cranelift

A pergunta não é qual gera código melhor, e sim qual chega antes a um perfil de
desenvolvimento **correto**.

1. **Zero divergência de backend.** O JIT consome o IR já emitido por
   `crates/llvm`, byte a byte o mesmo que o AOT. Um backend Cranelift exigiria um
   segundo emissor, a partir da HIR, e cada recurso da linguagem precisaria ser
   implementado e conferido duas vezes. O teste diferencial deste crate só é
   barato porque não há segundo emissor: ele compara execuções, não compiladores.
2. **O runtime já existe para esta ABI.** As 18 funções, o layout de handles e o
   protocolo de raízes foram desenhados contra as declarações de `crates/llvm`.
   Reaproveitá-los custou um módulo de adaptação; refazê-los para outra ABI não.
3. **Hot reload tem caminho pronto — quase.** Os `ResourceTracker` dão o
   descarregamento por geração, e isso se confirmou. O que **não** se confirmou é
   a expectativa sobre os stubs indiretos: a API C expõe
   `LLVMOrcCreateLocalIndirectStubsManager` e
   `LLVMOrcCreateLocalLazyCallThroughManager`, mas não o `updatePointer` que uma
   recarga precisa, então o redirecionamento ficou com trampolins em IR gerados
   pela sessão. Detalhes em
   [Hot reload de comportamento](#hot-reload-de-comportamento). O argumento
   sobrevive, com uma correção: a vantagem do ORCv2 aqui é o descarregamento por
   tracker, não os stubs.
4. **O custo da dependência já estava pago.** O projeto já exige LLVM para o
   perfil de produção.

Cranelift continua interessante pelo tempo de compilação e por dispensar uma
instalação do LLVM, que são os argumentos reais a favor dele; `crates/cranelift-jit`
explora esse caminho em paralelo, sobre uma fatia escalar da HIR. As duas coisas
não competem nesta etapa: trocar de backend só faz sentido depois que existir uma
linha de base medida e um diferencial verde para comparar contra o AOT — e é
exatamente isso que este crate entrega.

## Requisito de build e de execução

`crates/jit` usa `llvm-sys` 221.1.0 para as assinaturas da API C e para os
invólucros de `LLVMInitializeNative*`. Ele localiza o LLVM pelo `llvm-config` da
**distribuição completa** — a que traz `bin/llvm-config.exe`, `include/llvm-c/**`
e as bibliotecas. O instalador reduzido de Windows (`LLVM-*-win64.exe`), que só
tem `LLVM-C.dll`/`LLVM-C.lib` e os cabeçalhos `Remarks.h`/`lto.h`, **não serve**.

O repositório já traz `.cargo/config.toml` com

```toml
[env]
LLVM_SYS_221_PREFIX = "E:/llvm/clang+llvm-22.1.8-x86_64-pc-windows-msvc"
```

de modo que qualquer `cargo` executado dentro dele enxerga a variável. Uma
definição no ambiente do shell tem precedência, para testar outro prefixo.
`DARTFORGE_LLVM_DIR`, o nome preferido do projeto para apontar um prefixo LLVM,
também é aceito pelo `build.rs` deste crate — mas quem procura o `llvm-config` é
o `build.rs` do `llvm-sys`, que só conhece `LLVM_SYS_221_PREFIX`; mantenha as
duas com o mesmo valor.

Sem um prefixo válido a build do workspace inteiro falha, porque `crates/jit` é
membro de `crates/*`. Essa é a consequência de o perfil de desenvolvimento ser
parte do produto, não um extra.

### Linux e macOS

Fora do Windows o `build.rs` escolhe pelo pacote:

* se o prefixo traz a biblioteca compartilhada completa (`libLLVM-22.so`,
  `libLLVM.dylib` — pacotes do apt.llvm.org e do Homebrew), liga contra ela;
* senão — o pacote oficial `LLVM-22.1.8-Linux-X64.tar.xz` e o de macOS só
  trazem as estáticas —, liga **estático**, pelos componentes `orcjit native
  irreader passes` do `llvm-config --link-static`. Não há o conflito de CRT do
  Windows (há uma `libc` só). Das bibliotecas do sistema que o pacote declara,
  `xml2` fica de fora (só o `LLVMWindowsManifest` a usa) e o `zstd`, que vem
  como caminho absoluto da máquina que empacotou, cai na `libzstd.so.N` do
  sistema quando o `.a` não existe.

Não há dependência de execução: o `dartforge` não carrega nada do LLVM ao
iniciar. A biblioteca do SDK da fonte (`libdfsdk_<chave>.so`/`.dylib`) é
carregada com `dlopen(RTLD_NOW | RTLD_LOCAL)` e os nomes vêm de
`exportados.def`, como no Windows. `dartforge run`/`reload` acham a biblioteca
sozinhos (o cache, compilada na primeira vez) quando `DARTFORGE_SDK_DLL` não
está definida.

Ambiente mínimo, medido num Linux x86-64 (o mesmo que o CI prepara):

```sh
export LLVM_SYS_221_PREFIX=/opt/LLVM-22.1.8-Linux-X64
export DARTFORGE_LLVM_DIR=$LLVM_SYS_221_PREFIX
export DARTFORGE_CLANG=$LLVM_SYS_221_PREFIX/bin/clang
cargo build --release -p dartforge-cli --features nativo,jit
```

Com isso o corpus dá 184/224 no JIT e no AOT com o SDK da fonte, 224/224
saídas idênticas JIT × AOT, e os testes de `crates/jit` passam inteiros.

### Por que a ligação é dinâmica no Windows, e o que isso custa

A feature `no-llvm-linking` do `llvm-sys` está ligada: as diretivas de ligação
saem do `build.rs` deste crate, que liga `LLVM-C` — a **biblioteca
compartilhada** da API C — em vez das bibliotecas estáticas.

O motivo é concreto, não preferência. As bibliotecas estáticas do pacote oficial
`clang+llvm-22.1.8-x86_64-pc-windows-msvc` são compiladas com a CRT **estática**
(`libcmt`); o Rust usa a CRT dinâmica (`msvcrt`). Ligar as duas produz

```
LINK : warning LNK4098: defaultlib 'libcmt.lib' conflita com uso de outras
bibliotecas; use /NODEFAULTLIB:library
```

e um binário com **dois heaps**. O efeito observado, reproduzível: um
`LLVMParseIRInContext2` que falha devolve a mensagem de erro, ela é lida
corretamente, e o `LLVMDisposeMessage` seguinte derruba o processo com
`STATUS_ACCESS_VIOLATION` — alocada por uma CRT, liberada pela outra. O mesmo
pacote também traz `llvm-config --system-libs` pedindo um `xml2s.lib` que não
está no pacote, e não aceita `llvm-config --link-shared`, que procura um
`LLVM-22.dll` inexistente. Com `LLVM-C.dll`, alocação e liberação acontecem as
duas dentro da DLL, com a CRT dela, e nenhum dos dois problemas aparece.

O preço é uma dependência de execução: **`LLVM-C.dll` precisa estar alcançável
pelo carregador** — no `PATH`, em Windows — tanto para `dartforge run` quanto
para os testes. `scripts/env.ps1` acrescenta `<prefixo>/bin` ao `PATH` quando
encontra a DLL lá. Sem ela o executável nem chega a iniciar; a falha é do
carregador do sistema operacional, não do programa:

```
dartforge.exe: error while loading shared libraries: LLVM-C.dll:
cannot open shared object file: No such file or directory
```

Por isso os testes que abrem uma `LLJIT` são marcados

```rust
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
```

seguindo o padrão do repositório para testes que dependem de um toolchain
externo, como os de `crates/native`. Os testes unitários da biblioteca — formato
das mensagens de erro, tabela de símbolos, captura de saída do runtime — não
tocam no LLVM e continuam rodando sempre. A verificação canônica do projeto
(`cargo test --locked --workspace -- --include-ignored`, em CONTRIBUTING.md) já
executa os ignorados, então nada fica de fora do que o projeto considera verde.

`jit_and_aot_agree_on_the_same_ir` acumula os dois requisitos: precisa da DLL e
de Clang/`rustc` para construir o lado AOT da comparação.

### Integração contínua

O workflow atual instala apenas o Clang do ambiente e **não** provisiona a
distribuição completa do LLVM nem define `LLVM_SYS_221_PREFIX`. Enquanto isso não
for ajustado, `cargo build --workspace` falhará na CI por falta de `llvm-config`.

## Referências oficiais consultadas

- [LLVM ORCv2 design and implementation](https://llvm.org/docs/ORCv2.html):
  `JITDylib` como unidade de nomes, `ResourceTracker` para descarregamento e o
  papel das unidades de materialização.
- [LLVM C API — Orc](https://llvm.org/doxygen/group__LLVMCExecutionEngineORC.html)
  e LLJIT: propriedade dos argumentos em `LLVMOrcCreateNewThreadSafeModule`,
  `LLVMOrcLLJITAddLLVMIRModuleWithRT` e `LLVMOrcAbsoluteSymbols`; consumo do
  `LLVMErrorRef` por `LLVMGetErrorMessage`.
- [`llvm-sys` 221.1.0](https://docs.rs/llvm-sys/221.1.0/): assinaturas usadas e a
  convenção `LLVM_SYS_<versão>_PREFIX`.
- `crates/runtime/CONTRACT.md` e [historico/AOT-DRIVER.md](historico/AOT-DRIVER.md): a ABI que este
  crate precisa reproduzir e a fronteira `unsafe` do perfil de produção.
