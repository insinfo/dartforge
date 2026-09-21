# JIT ORCv2 — execução em memória do subconjunto nativo

O DartForge passa a ter dois perfis de execução nativa sobre **o mesmo LLVM IR**:

| | Desenvolvimento (`crates/jit`) | Produção (`crates/native`) |
| --- | --- | --- |
| Comando | `dartforge run <entrada.dart>`, `dartforge reload <v1.dart> <v2.dart> …` | `dartforge aot <entrada.dart> <saida.exe>` |
| Geração de código | ORCv2 (`LLJIT`), em memória | Clang, em processo separado |
| Runtime nativo | endereços das funções Rust publicados como símbolos absolutos | `rustc` compila `RUNTIME_MAIN` e o linker resolve os símbolos |
| Artefato | nenhum | executável no disco |
| Isolamento do programa | **nenhum**: executa no processo do compilador | processo próprio |

O contrato entre os dois é observável e testado: `crates/jit/tests/execucao.rs`
compila um programa Dart uma única vez, executa o IR resultante pelos dois
caminhos e exige saída idêntica. Divergir em tempo de compilação é esperado;
divergir em resultado é defeito.

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

Esta é a diferença estrutural entre os dois perfis, e a decisão menos óbvia do
crate.

`crates/runtime` **não** exporta os símbolos da ABI nativa como biblioteca. Ele
publica `RUNTIME_MAIN`, um programa Rust completo em forma de texto
(`include_str!` de `heap.rs` + `runtime_main.rs`), que o driver AOT grava no
staging e manda o `rustc` compilar. Os `#[unsafe(no_mangle)] extern "C" fn` só
existem *nesse* binário. O processo que hospeda o JIT não os tem.

Logo, registrar o gerador de símbolos do processo
(`LLVMOrcCreateDynamicLibrarySearchGeneratorForProcess`) **não resolveria nada**:
`dartforge_print_i64` não está entre os símbolos do processo. Além disso, esse
gerador exporia ao código gerado todo símbolo do executável hospedeiro, uma
superfície muito maior que o contrato de `crates/llvm`, e resolveria por
acidente um nome homônimo em vez de falhar alto.

A sessão faz o equivalente exato da ligação AOT, pela via explícita:
`crates/jit/src/runtime.rs` reimplementa a mesma ABI sobre o mesmo
`dartforge_runtime::heap::Heap`, e `crates/jit/src/ffi.rs` publica os endereços
dessas funções como símbolos absolutos (`LLVMOrcAbsoluteSymbols` +
`LLVMOrcJITDylibDefine`) na `JITDylib` principal, com os nomes decorados por
`LLVMOrcLLJITMangleAndIntern` para casar com a decoração que o `lookup` aplica.

São 18 nomes, os mesmos que `crates/llvm` declara no IR e que o harness AOT
define. A lista está em `dartforge_jit::RUNTIME_SYMBOLS` e é verificada por
teste. **Toda mudança em `crates/runtime/src/runtime_main.rs` tem contrapartida
em `crates/jit/src/runtime.rs`**; o teste diferencial é o que impede a divergência
de passar despercebida.

### Captura de saída

O runtime do JIT imprime no stdout do processo, como o AOT. Para o teste
diferencial isso não serve — seria preciso redirecionar o stdout do processo de
teste, o que não é confiável sob execução paralela. Por isso as funções de
impressão escrevem num destino `thread_local` que `run_entry_capturing` e
`run_ir_capturing` desviam para um buffer, com quebras `\n` em qualquer sistema.
É a única diferença deliberada de comportamento entre os dois runtimes, e ela
não afeta o texto produzido.

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
   toda referência externa é resolvível, versionar as implementações e entregar o
   módulo à `LLJIT` sob um tracker novo.
2. **Publicar**: materializar as implementações (aqui é onde o código nativo é
   gerado e ligado em memória) e **só então** escrever os novos ponteiros nas
   células.
3. **Aposentar** o código antigo — que nesta versão não acontece; veja a política
   de retenção.

A garantia que importa: **uma falha de análise, de contrato ou de ligação não
destrói a versão que está funcionando**. Falha na fase 1 devolve `Err` sem ter
tocado em nada. Falha na materialização descarrega a geração recém-adicionada —
remoção demonstravelmente segura, porque nenhuma célula aponta para ela e nada
pode tê-la chamado — e a versão anterior continua publicada, bit a bit.

`crates/jit/tests/hot_reload.rs::falha_de_recarga_nao_destroi_a_versao_boa` cobre
as três falhas (Dart que não compila, IR inválido, referência não resolvível) e
exige que a versão boa continue executando e que uma recarga válida ainda
funcione depois delas.

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

### O laço pela linha de comando

`dartforge reload` abre uma sessão, executa a primeira versão e publica cada
arquivo seguinte como uma edição, mantendo o heap vivo entre elas:

```
$ dartforge reload v1.dart v2.dart v3.dart
42
100
150
```

`v2.dart` muda `base()` de 21 para 50; `v3.dart` muda `dobro()` para `* 3`. As
três saídas vêm da **mesma** sessão, do mesmo processo, sem reinício. Com
`--timings`, cada recarga imprime um objeto JSON com o custo de cada etapa
(`frontend_ns`, `parse_ir_ns`, `contract_ns`, `add_module_ns`, `stubs_ns`,
`link_ns`, `publish_ns`, `retire_ns`, `reload_total_ns`) mais `entries`,
`new_entries`, `generation` e `retained_generations`.

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

Vale registrar o precedente, porque os limites acima não são atalho deste
projeto — a implementação oficial tem os mesmos, pelos mesmos motivos.

| Dart VM | Aqui |
| --- | --- |
| O front-end manda um **Dill delta**: só o que mudou | `hot_reload` recebe o IR do **programa inteiro**. É a diferença mais custosa, e a medição abaixo mostra onde ela aparece |
| Localiza a classe no heap e **troca os ponteiros de método**; instâncias não mudam de endereço e os campos ficam intactos | Troca os ponteiros das **células** das entradas estáveis; os objetos do heap gerenciado não são tocados, mesmo handle, mesmos campos. A ideia é a mesma, um nível acima: lá por método de classe, aqui por função |
| Código já promovido ao otimizador é **descartado**, não remendado; a função volta ao tier não otimizado | Não há dois níveis aqui. Se houver, esta é a política a seguir: descartar a geração otimizada e recompilar, nunca remendar código otimizado no lugar |
| Recarga **falha** e exige reinício quando muda `main()`, `initState()`, o inicializador de uma global já inicializada, herança ou assinaturas | Recusas equivalentes na etapa `contract`, tabela acima. `main` é `@dartforge_entry`, uma entrada estável como as outras, e mudar o corpo dela é aceito — a restrição da VM é sobre o *estado* de `main`, não sobre o código |
| Inicializador de global já inicializada **preserva o valor antigo** | Divergimos, e o limite é concreto: `crates/llvm` guarda os estáticos de classe e as variáveis de topo numa área do heap cujo handle vive em `@df_statics = internal global i64 0`, e grava todos eles no começo de `dartforge_entry()`. `internal` significa uma cópia por geração, e a inicialização na carga significa que **executar a entrada de novo reinicializa os estáticos** — o que já vale sem hot reload nenhum. Consequências: o estado em estáticos **não** sobrevive a uma recarga, e depois de recarregar é preciso executar a entrada antes de chamar entradas estáveis que toquem estáticos, senão elas leem o handle zero da geração nova. O que persiste entre gerações é o **heap gerenciado** alcançado por handles que o chamador guarda, e é isso que o teste do contador vivo afirma |

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
LLVM_SYS_221_PREFIX = "D:/DartSDKs/llvm/clang+llvm-22.1.8-x86_64-pc-windows-msvc"
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

### Por que a ligação é dinâmica, e o que isso custa

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
- `crates/runtime/CONTRACT.md` e [AOT-DRIVER.md](AOT-DRIVER.md): a ABI que este
  crate precisa reproduzir e a fronteira `unsafe` do perfil de produção.
