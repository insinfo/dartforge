# Pesquisa aplicada: JIT e hot reload

Este documento reúne o que as referências locais mostram sobre como trocar código
num processo vivo. Ele é leitura obrigatória para quem for mexer em
`crates/jit`, no contrato de nomes do `emit_native` ou em `crates/runtime` com
vistas ao `dartforge reload`. O molde é o de `docs/PESQUISA-OTIMIZACAO.md`.

**Alvo governante.** O `dartforge reload` deve ter a semantica de hot reload do
Dart, como a VM e o Flutter a entregam:

- atualiza o código;
- preserva o estado;
- não reexecuta `main` nem `initState`;
- recusa mudanças incompatíveis.

Hot restart é outra coisa: ele perde o estado. **O oráculo é a VM oficial.**

**Convenção de citação.**

- As linhas citadas são das cópias em `D:\Projects\dartforge\references`, na data
  deste documento. `references/dart-sdk` está em **3.14.0-dev** (`tools/VERSION`),
  não em 3.6.2; onde isso importa, o texto avisa.
- "Não verificado" quer dizer que a afirmação não foi conferida no código local.

---

## 1. A tese e o que as referências confirmam ou corrigem

### 1.1 Primeira afirmação

> JIT e hot reload são capacidades independentes.

**Confirmada, com uma ressalva que muda o desenho.**

Há hot reload sem JIT:

- **Mun** carrega bibliotecas dinâmicas compiladas AOT
  (`mun/crates/mun_runtime/src/assembly.rs:87-136`).
- **Swift** tem `@_dynamicReplacement`, gerado pelo IRGen AOT
  (`swift/lib/IRGen/GenDecl.cpp:3083-3150`).
- **InjectionIII** carrega uma dylib AOT e interpõe símbolos
  (`InjectionIII/README.md:200-215`).

A ressalva: na **VM do Dart**, o reload só existe no runtime com JIT. Todo o
arquivo fica sob `!defined(PRODUCT) && !defined(DART_PRECOMPILED_RUNTIME)`
(`dart-sdk/runtime/vm/isolate_reload.h:461`, `isolate_reload.cc:2942`). O motivo
é que a VM depende de três coisas:

- **recompilar sob demanda** o código das bibliotecas sujas: `ClearCode` e
  `SwitchToLazyCompiledUnoptimizedCode` (`isolate_reload.cc:2305`, `:2340-2346`);
- **desotimizar** o que dependia de CHA e de guardas de campo (`:943-956`,
  `:1530-1563`);
- **zerar os caches de chamada** (`:2110-2154`).

O que o hot reload exige de fato não é JIT. São duas coisas:

- carregar código novo num processo vivo, por dylib ou por ORC;
- um **ponto de indireção** por onde a chamada nova encontra o corpo novo.

No DartForge, o ORCv2 é o carregador mais barato, porque consome o mesmo IR do
AOT. Não é pré-requisito semântico.

### 1.2 Segunda afirmação

> Mun faz hot reload com LLVM AOT.

**Confirmada, e há detalhes que a tese não trazia:**

- A unidade de troca é a **assembly** inteira, não a função (`assembly.rs:87-88`).
- Chamadas **dentro** da mesma assembly são diretas. Só as que cruzam assemblies
  passam pela tabela: `should_runtime_link_fn` em
  `mun_codegen/src/module_group.rs:113-119` e `ir/body.rs:1092-1121`.
- A dylib antiga é **descartada logo depois** de religar
  (`assembly.rs:433-444`). Isso só é seguro porque o hospedeiro chama
  `Runtime::update()` quando não há quadro Mun na pilha (`mun_runtime/src/lib.rs:418`).
  Não há coexistência de versões.

### 1.3 Terceira afirmação

> BEAM gera código com AsmJit e mantém a versão antiga e a atual de um módulo.

**Não verificada.** Não há Erlang/OTP em `references`; só `asmjit`. A afirmação
fica como hipótese.

### 1.4 Quarta afirmação

> Julia faz JIT sobre ORCv2, e o Revise atualiza métodos com world age.

**Confirmada no que está no código local.**

- O JIT usa ORC, com `JITDylib`s próprias (`julia/src/jitlayers.cpp:1917-1919`).
- Cada `Task` carrega um `world_age` (`src/task.c:330`, `:1128`).
- O despacho genérico usa o mundo da tarefa corrente (`src/gf.c:4842-4850`).
- Definir um método incrementa o contador global (`gf.c:3466-3479`).
- `invokelatest` sobe o mundo só durante a chamada
  (`doc/src/manual/worldage.md:124-143`).
- "Tarefa em curso continua no código velho" é a invariante documentada: o mundo
  não muda dentro de uma função comum (`worldage.md:108-122`).

O **Revise não está nas referências**; o que se diz dele aqui é não verificado.

### 1.5 Quinta afirmação

> A proibição de revisar structs no Revise valeria só antes do Julia 1.12.

**Meio certa.** Desde a 1.12, redefinir um `struct` é permitido e segue o world
age (`julia/HISTORY.md:172-173`). A definição nova, porém, é **outro tipo**.

As instâncias antigas **não migram**: continuam sendo do tipo velho, exibido como
`@world(MyStruct, a:b)` (`worldage.md:187-218`). É o oposto da VM do Dart, que
converte as instâncias vivas campo a campo (§2.1). Julia não serve de modelo para
estado; serve para visibilidade.

### 1.6 Proposta inicial, item 1

> Trocar corpos preservando assinaturas e layouts.

**Serve como primeiro passo e não serve como contrato.** A VM do Dart aceita as
duas mudanças:

- **Assinatura.** A VM não recusa. Chamadas estáticas pendentes são religadas
  **por nome** (`object_reload.cc:806-841`). Quem chama um membro que sumiu ou
  mudou de aridade recebe `NoSuchMethodError` **na hora da chamada**: testes
  `IsolateReload_PendingStaticCall_DefinedToNSM`
  (`isolate_reload_test.cc:1479-1531`) e `CallDeleted_*ArityChange` (`:3205-3235`).
- **Layout.** Campos novos, removidos e reordenados são aceitos, e as instâncias
  são convertidas (§2.1).

Se o primeiro passo recusar essas mudanças, o formato dos dados precisa já
prever a conversão. Isso é a regra de "arquitetura antes de código".

### 1.7 Proposta inicial, item 2

> Publicar num ponto seguro, deixar chamadas já iniciadas terminarem na versão
> antiga e manter o código antigo alocado.

**Confirmada, e a VM vai além.** No teste `LiveStack`, o quadro vivo termina no
corpo antigo, mas **as chamadas que ele faz depois da recarga vão para o corpo
novo**: `x + helper()` dá `7 + 100` (`isolate_reload_test.cc:1048-1079`).

Três correções:

- **O ponto seguro da VM não exige pilha vazia.** A recarga pede um safepoint de
  nível `kGCAndDeoptAndReload` (`heap/safepoint.cc:134-163`). O mutador só
  atende ao processar a mensagem OOB `kCheckForReload`
  (`safepoint.cc:97-100`, `isolate.cc:1386-1395`), o que pode acontecer com
  quadros Dart ativos.
- **O código antigo continua vivo mesmo com a pilha vazia**, por dois caminhos:
  - **closures** criadas antes da recarga executam o corpo **antigo**, e as
    chamadas que fazem vão para o corpo **novo** (`IsolateReload_SavedClosure`,
    `:416-448`: sai `"postapocalyptic!"`);
  - **funções `async` suspensas** retomam no código antigo
    (`InvalidateSuspendStates`, `isolate_reload.cc:2362-2417`).

  Por isso, "entre turnos do laço de eventos" **não basta** para liberar a
  geração antiga (§4.6).
- **Tear-off de método de instância não é closure anônima.** Ele passa a executar
  o corpo novo, `==` continua verdadeiro e `identical` fica falso
  (`TearOff_Instance_Equality`, `:1628-1664`).

### 1.8 Proposta inicial, item 3

> Exigir hot restart para mudanças estruturais e expandir depois.

**Corrigida no alvo.** A lista de recusas da VM é **curta**
(`object_reload.cc:351-479`, `:481-607`). Mudar a superclasse é aceito
(`SuperClassChanged`, `isolate_reload_test.cc:760-789`), assim como adicionar ou
remover campos, mudar o tipo de um campo e apagar funções ou classes. O custo
disso fica em erros tardios:

- `NoSuchMethodError`;
- `TypeError` na leitura, via guarda de carga (`FieldInvalidator`,
  `isolate_reload.cc:2419-2655`).

Exigir restart para mudança estrutural é aceitável como degrau (R1), desde que o
texto de recusa diga que é uma limitação nossa, e não semântica do Dart.

### 1.9 Correção a um documento do próprio repositório

A tabela de `docs/JIT.md:342-353` diz que a VM recusa a recarga quando mudam
«herança ou assinaturas». **O código mostra o contrário** (§1.6 e §1.8). A
tabela deve ser corrigida quando `crates/jit` for retomado.

---

## 2. As referências, uma a uma

### 2.1 Dart VM: a semântica a copiar

**Mecanismo, na ordem de `IsolateGroupReloadContext::Reload`
(`isolate_reload.cc:805-1159`):**

1. **Delta.**
   - A VM recebe um kernel.
   - Marca as bibliotecas modificadas e o **fecho transitivo dos dependentes**
     (`:880-891`; bitvectors em `isolate_reload.h:254-263`).
   - Sem nenhuma biblioteca modificada, a recarga é *pulada*, e não falha
     (`:904-924`).
2. **Preparação.**
   - A VM garante código não otimizado para os quadros na pilha
     (`:1491-1528`).
   - Desotimiza o que dependia de CHA e de guardas de campo (`:1530-1563`).
3. **Checkpoint.**
   - A tabela de classes é **clonada** (`:1565-1611`). A partir daí coexistem a
     `heap_walk_class_table`, com o layout antigo que o GC usa, e a
     `class_table`, com o novo.
   - O comentário que resume a regra: «everything we do must be reversible»
     (`:1358-1364`).
4. **Carga e validação.**
   - Casa bibliotecas por **URL** (`IsSameLibrary`, `:550-557`).
   - Casa classes por **nome + chave privada da biblioteca** (`IsSameClass`,
     `:530-548`).
   - `RegisterClass` **mantém o `cid`** da classe antiga (`:1436-1437`) e herda
     as constantes canônicas (`:1438`, `object_reload.cc:233-249`).
   - `ValidateReload` chama `Class::CheckReload` para cada par
     (`isolate_reload.cc:2055-2095`).
5. **Recusa ou commit.**
   - Com qualquer `ReasonForCancelling`, a recarga é desfeita
     (`ReloadPhase4Rollback`, `:1411-1414`).
   - No commit (`:1846-1945`):
     - **estáticos**: o campo novo passa a apontar para o *mesmo* `field_id` do
       antigo, e o valor é compartilhado (`object_reload.cc:198-231`);
     - **`const` é atualizado**, não preservado («We let const fields be
       updated», `:218-225`);
     - funções e campos antigos vão para uma *patch class* que guarda o script
       antigo (`:274-321`); é por isso que closures antigas ainda compilam;
     - tear-offs estáticos migram por `become` (`:323-349`).
6. **Conversão de instâncias (*morphing*).**
   - `RequiresInstanceMorphing` compara nome e posição dos campos
     (`object_reload.cc:630-692`).
   - O `InstanceMorpher` casa campos **por nome**, ignorando o tipo
     (`isolate_reload.cc:118-160`).
   - Campo novo recebe `Object::sentinel()` e guarda de carga (`:128-133`,
     `:212-215`, `:361-364`). Na primeira leitura, o inicializador roda
     (`RunNewFieldInitializers`, `isolate_reload_test.cc:4908-4942`); se ele
     lançar, a exceção sai da leitura (`:5139-5177`).
   - O objeto antigo vira enchimento, e um `become` redireciona todas as
     referências (`isolate_reload.cc:366-371`, `:1042-1073`).
   - Objetos canônicos vão para o espaço velho e mantêm o hash (`:249-279`).
7. **Invalidação** (`InvalidateWorld`, `:2668-2678`).
   - Descarta os caches megamórficos.
   - Zera os ICs dos quadros vivos.
   - Limpa o código das bibliotecas sujas (`:2321-2346`).
   - Religa chamadas estáticas por nome (`object_reload.cc:770-845`).
   - Confere o tipo de cada valor de campo contra o tipo novo e liga a guarda
     quando não bate (`isolate_reload.cc:2443-2528`, `:2617-2636`).
8. **Enums.**
   - Valores antigos são redirecionados para os novos **por nome**
     (`ForwardEnums`, `:1955-1959`). O resultado é que `identical(x, Fruit.Banana)`
     continua `true` mesmo depois de reordenar (`isolate_reload_test.cc:2060-2093`).
   - Valor apagado vira `Deleted enum value from Fruit`, com `index == -1`
     (`:2180-2215`).
9. **Relatório.**
   - A VM devolve um JSON `ReloadReport` com `success`, as `notices` com os
     motivos e o `shapeChangeMappings` (`isolate_reload.cc:1451-1489`).

**Recusas.** A lista a espelhar tem entre 7 e 9 itens:

| Classe da recusa | Mensagem da VM | Onde |
| --- | --- | --- |
| `EnumClassConflict` | «Enum class cannot be redefined to be a non-enum class» / o inverso | `object_reload.cc:351-363` |
| `TypeParametersChanged` | «Limitation: type parameters have changed for %s» | `:430-449` |
| `ConstToNonConstClass` | «Const class cannot become non-const» (só com constantes vivas) | `:394-404`, `:539-547` |
| `ConstClassFieldRemoved` | «Const class cannot remove fields» | `:406-416`, `:549-584` |
| `DeeplyImmutableChange` | «Classes cannot change their @pragma('vm:deeply-immutable')» | `:381-392` |
| `NativeFieldsConflict` | «Number of native fields changed» | `:418-428` |
| `EnsureFinalizedError` | o erro de finalização da classe nova | `:365-379` |
| `PreFinalizedConflict` / `InstanceSizeConflict` | internas da VM (classes pré-finalizadas) | `:451-479`, `:718-735` |
| `Aborted` | erro de compilação ou de carga | `isolate_reload.cc:621-634`, `:856-866` |

**Fora da lista, portanto aceito:**

- mudar a hierarquia;
- adicionar ou remover métodos e campos;
- mudar assinaturas;
- mudar tipos de campo;
- apagar classes, cujas instâncias vivas seguem com o código antigo
  (`isolate_reload.cc:2786-2793`);
- mudar `main`, cujo corpo novo **não é executado**, porque a recarga não chama
  nada.

**O que tirar:**

- identidade por nome;
- `cid` estável;
- estáticos compartilhados por identidade;
- `const` atualizado;
- conversão por nome de campo, com sentinela e inicialização preguiçosa;
- religação de chamada por nome, com `NoSuchMethodError` na chamada;
- enums redirecionados por nome;
- transação com checkpoint;
- fecho transitivo de dependentes;
- a própria lista de recusas;
- o relatório JSON;
- **os testes**: 172 casos em `runtime/vm/isolate_reload_test.cc` e 173
  diretórios em `tests/hot_reload`.

**O que não copiar:**

- o `become` por varredura do heap: o nosso heap já é por *handles* (§4.7);
- a desotimização e os ICs: não temos otimizador especulativo;
- o safepoint no meio da execução por mensagem OOB. Adotamos um ponto mais
  estrito (§4.5), cuja semântica observável é um subconjunto da da VM.

### 2.2 Mun: hot reload nativo com LLVM AOT

**Mecanismo.**

- **Carga e versão da ABI.** Cada assembly é uma dylib com `AssemblyInfo`, e a
  versão da ABI é conferida na carga (`assembly.rs:114-136`).
- **Ligação.** O runtime mantém uma `DispatchTable`, um mapa de nome de função
  para definição (`dispatch_table.rs:9-67`). Ele preenche a tabela de ponteiros
  de cada assembly, conferindo a assinatura por **tipo**; se divergir, dá
  `MismatchedSignature` (`assembly.rs:165-256`, e o erro em `:205-233`).
- **Transação.** Tabela de despacho e tabela de tipos são clonadas antes de
  religar, e uma falha deixa as originais intactas (`assembly.rs:335-339`,
  `lib.rs:515-529`).
- **Memória.**
  - Os tipos antigos e novos são comparados: struct casado por **nome**, ou por
    campos idênticos (`mun_memory/src/diff.rs:170-289`).
  - Campo a campo, a comparação usa nome e tipo, com *heurística de rename*:
    campo de nome diferente e mesmo tipo, na posição mais próxima
    (`diff.rs:292-413`, rename em `:353-410`).
  - As ações são `Copy`, `Cast`, `ZeroInitialize`, `StructAlloc`, entre outras
    (`mapping.rs:44-83`).
  - A conversão é **in loco**. O GC usa *handles* com dupla indireção
    (`gc/ptr.rs:1-18`), e o `map_memory` troca o conteúdo do `ObjectInfo` sem
    mudar o handle (`gc/mark_sweep.rs:629-679`).
  - Campo novo nasce zerado (`book/src/ch04-04-hot-reloading.md:76-80`).
- **Ponto seguro.** O hospedeiro decide: ele chama `update()` no laço dele, por
  exemplo entre quadros de um jogo (`lib.rs:418-530`). A detecção de mudança usa
  um arquivo de trava que o compilador apaga ao terminar (`lib.rs:478-513`).

**O que tirar:**

- *handles* permitem migrar sem `become`: o DartForge **já tem isso** (§4.7);
- tabela de despacho por módulo, preenchida pelo runtime;
- clonar e depois trocar, como transação;
- a ABI com número de versão.

**O que não copiar:**

- A **heurística de rename.** A VM casa só por nome. Adivinhar rename mudaria o
  resultado observável, porque um campo renomeado herdaria o valor antigo, e o
  oráculo daria diferente.
- **Zerar** o campo novo. Em Dart ele roda o inicializador.
- **Descartar a dylib antiga na hora.** Em Dart, closures e `async` suspensos
  seguram código antigo.
- Assinatura divergente como erro de ligação. Em Dart vira `NoSuchMethodError`
  na chamada.

Não verificado: se assemblies **dependentes e não modificadas** são religadas
quando uma dependência muda. `relink_all` só percorre as assemblies recarregadas
(`assembly.rs:312-431`).

### 2.3 Julia: world age e invalidação por backedges

**Mecanismo.**

- **Mundo global.** O contador `jl_world_counter` (`gf.c:37`) sobe a cada
  definição (`gf.c:3466-3479`).
- **Faixa de validade.** Cada `CodeInstance` tem `min_world` e `max_world`, e a
  busca filtra pela faixa (`gf.c:415`, `:1764`).
- **Invalidação.** Invalidar é **fechar a faixa** (`max_world`) e propagar pelos
  *backedges* aos chamadores que dependiam do método (`gf.c:2423-2452`,
  `:2472-2490`). O backedge é registrado na compilação (`gf.c:2639`). Nenhum
  código é apagado: ele só deixa de valer para mundos novos.
- **Publicação.** O ponteiro de código de um `CodeInstance` é publicado **uma
  vez**, por `cmpswap` (`jitlayers.cpp:430-444`). Versão nova é outro
  `CodeInstance`, e não escrita no lugar.
- **Mundo por tarefa.** Uma tarefa nova herda o mundo de quem a criou
  (`task.c:1128`, `worldage.md:263-287`).

**O que tirar:**

- A ideia de **faixa de validade** por versão de código, com invalidação por
  dependência registrada: backedges. É exatamente o que faltará quando houver
  inlining ou constantes dobradas entre bibliotecas no JIT (§6, risco 6).
- A regra: publicar ponteiro novo, nunca remendar código.

**O que não copiar:**

- **world age como semântica visível.** Em Dart, depois da recarga o código novo
  é visto **imediatamente** por qualquer chamada nova, inclusive de quadros
  antigos (`LiveStack`). Julia faz o contrário: a tarefa segue no mundo velho
  até `invokelatest` ou até voltar ao topo.
- **Tipo novo sem migração** (§1.5).

Não encontrei remoção de código em `jitlayers.cpp` (procurei `ResourceTracker` e
`remove`): pelo que se vê, a Julia retém tudo. Não verificado de forma
exaustiva.

### 2.4 .NET: CoreCLR (Edit and Continue) e Mono (hot reload)

A cópia é esparsa. Só existem `src/coreclr/vm/encee.*`,
`src/mono/mono/component/hot_reload*` e `src/mono/mono/metadata/metadata-update.*`.
`debug/ee`, `docs/design` e o `MetadataUpdater` **não foram baixados**; o que
dependeria deles fica não verificado.

**Delta.**

- O compilador (Roslyn) produz metadados, IL e PDB de delta.
- O runtime aplica o delta **token a token**:
  - `MethodDef` vira uma nova versão de IL pelo `CodeVersionManager`, o mesmo
    mecanismo do ReJIT (`encee.cpp:237-297`, `:265-282`);
  - `FieldDef` novo é adicionado (`:300-343`).
- Quem decide o que é permitido é o **compilador**, a partir das *capabilities*
  que o runtime declara:
  - Mono: `"Baseline AddMethodToExistingType AddStaticFieldToExistingType
    NewTypeDefinition … UpdateParameters GenericAddFieldToExistingType"`
    (`hot_reload.c:3361-3364`);
  - o enum copiado do Roslyn: `hotreload-utils/…/EnC/EditAndContinueCapabilities.cs:10-70`.
- As **rude edits** são diagnósticos do Roslyn, que não está nas referências.

**Campo novo em instância viva: tabela lateral.**

- O objeto **não muda de layout**.
- O valor fica num `EnCAddedField` pendurado no `SyncBlock` do objeto, por
  *dependent handle*: fraco para o objeto, forte para o valor
  (`encee.h:300-324`, `encee.cpp:1194-1253`).
- Estático novo fica pendurado no `FieldDesc` (`encee.h:326-351`).
- O acesso ao campo novo é lento. Os campos antigos não mudam.

**Quadro ativo.** O CoreCLR consegue **remapear** um quadro vivo para a versão
nova, num ponto de sequência, sob o depurador (`ResumeInUpdatedFunction`,
`encee.cpp:665-797`). O Mono não faz isso: nada equivalente foi encontrado em
`hot_reload.c`.

**Gerações no Mono.**

- `update_alloc_frontier` e `update_published` são globais. Cada thread tem a
  sua **geração exposta** em TLS.
- `prepare` reserva a geração nova e a mostra só à thread que atualiza.
- `publish` escreve `update_published` com barreira; `cancel` desfaz
  (`hot_reload.c:747-863`).
- Uma thread só vê a geração nova quando chama `thread_expose_published`
  (`:809-823`). É um "world age" por thread.

**Condições de habilitação.** O Mono só aceita delta com
`DOTNET_MODIFIABLE_ASSEMBLIES=debug` e em assembly compilada com
`DisableOptimizations` (`hot_reload.c:390-410`).

**O que tirar:**

- **capabilities** declaradas pelo runtime e consultadas pelo compilador. No
  DartForge, o compilador e o runtime são nossos, mas a separação vale: o
  front-end decide a recusa lendo uma lista que o runtime declara, e o texto da
  recusa cita a capability.
- **Tabela lateral** como alternativa barata ao *morphing* quando não se quer
  tocar em instâncias.
- **Geração preparada, publicada ou cancelada**, com visibilidade controlada.

**O que não copiar:**

- A tabela lateral **como semântica final.** Em Dart o campo novo precisa estar
  no objeto para `==`, `hashCode` e cópias; a VM converte.
- **Remapear quadro ativo:** exige mapa de variáveis entre versões, e a VM do
  Dart não faz isso.
- Deixar as recusas **só no compilador.** A VM valida no runtime
  (`Class::CheckReload`), e nós devemos validar nos dois.

### 2.5 hotreload-utils: gerar deltas a partir de um roteiro

**Mecanismo.**

- Um roteiro JSON lista `{"document": X.cs, "update": X_v1.cs}`, um por geração
  (`src/hotreload-delta-gen/example/diffscript.json`).
- O gerador aplica cada versão sobre a solução e pede o delta ao serviço de hot
  reload (`DeltaProject.cs:84-96`).
- Havendo diagnóstico de erro, a execução sai com código 8 (`:98-104`). Senão,
  grava `dmeta`, `dil`, `dpdb` e a lista `UpdatedTypes` (`:106-121`).
- `UpdatedTypes` é o que alimenta os *handlers* de atualização do framework;
  esses handlers não foram verificados, porque `System.Runtime.Loader` não foi
  baixado.

**O que tirar:** o **corpus de reload como dado**: fonte base, sequência de
versões, resultado esperado, com falha de compilação esperada tratada à parte.
O `tests/hot_reload` do SDK do Dart segue o mesmo formato (§5.1).

### 2.6 Compose Hot Reload: JVM com JBR/DCEVM

**Mecanismo.**

- A JVM roda com `-XX:+AllowEnhancedClassRedefinition` (JBR), que permite mudar
  campos e hierarquia (`hot-reload-gradle-plugin/.../arguments.kt:313`).
- O agente chama `Instrumentation.redefineClasses`
  (`hot-reload-agent/.../reload.kt:34-118`).
- **Detecção de mudança por hash de escopo.** É um CRC sobre opcodes e nomes de
  chamados, ignorando informação de fonte (`hot-reload-analysis/.../ScopeHash.kt:18-40`).
  Escopos sujos se propagam aos dependentes (`resolveDirty.kt:56-75`, `:152-232`).
- **Recomposição.** Só os grupos Compose sujos são invalidados, via
  `Recomposer.invalidateGroupsWithKey` (`agent/compose.kt:64-109`). Há um modo
  "duro" que salva o estado, descarta e recompõe (`:144-179`).
- **Estáticos.**
  - Se o `<clinit>` mudou, o agente o **reexecuta**: cria um `$chr$clinit` e tira
    o `final` dos campos (`staticsInitialization.kt:28-80`).
  - Mudar o método de entrada Compose é recusado (`verifyRedefinitions.kt:24-40`).

**O que tirar:**

- **Impressão digital de corpo**, normalizada, para decidir o que está sujo sem
  depender de texto: o mesmo que o `ScopeHash`, aplicado a HIR ou IR.
- **O framework precisa cooperar para redesenhar.** A troca de código não
  reexecuta nada. É o análogo do `reassemble` do Flutter (não verificado; o
  Flutter não está nas referências) e do que o ngdart precisará (§6, pergunta 5).

**O que não copiar:** **reexecutar inicializador estático alterado.** A VM
preserva o valor (`IsolateReload_StaticFieldInitialValueDoesnotChange`,
`isolate_reload_test.cc:5504-5527`). Copiar isso daria saída diferente do
oráculo.

### 2.7 Swift: `@_dynamicReplacement`, e o `lib/Immediate`

**Mecanismo.**

- Uma função `dynamic` ganha, no IRGen, uma **cadeia de substituições**
  `{funPtr, next}` (`GenDecl.cpp:1876-1929`) e um **prólogo** no próprio corpo
  (`GenDecl.cpp:3083-3150`).
- Há duas formas de prólogo:
  - no modo padrão, uma chamada a `swift_getFunctionReplacement`, que lê a
    célula e consulta um TLS (`stdlib/public/runtime/FunctionReplacement.cpp:19-35`);
  - no modo `basic-dynamic-replacement`, um `load` e uma comparação
    (`GenDecl.cpp:87-89`, `:3130-3137`).

  Se a célula aponta para o próprio corpo, segue. Senão, faz chamada de cauda
  para a substituta.
- **Quem chama não muda.** A chamada continua direta no símbolo original.
- A substituição é registrada **na carga da imagem**
  (`MetadataLookup.cpp:3973-4027`) ou por escopo (`:4029-4041`), e encadeada:
  `swift_getOrigOfReplaceable` chama "o original"
  (`FunctionReplacement.cpp:37-41`).
- A documentação diz que a troca acontece no início do programa ou na carga de
  uma biblioteca, «instead of at an arbitrary point in time»
  (`docs/ReferenceGuides/UnderscoredAttributes.md:123-131`).

**`lib/Immediate`.**

- É o JIT do `swift` interpretado.
- Monta `LLJIT` com `EPCIndirectionUtils`, `LazyCallThroughManager` e
  `IndirectStubsManager` (`lib/Immediate/SwiftMaterializationUnit.cpp:72-92`).
- Gera um *stub* preguiçoso por função, com `lazyReexports` (`:440-455`).
- Carrega o runtime Swift por `dlopen` e o resolve pelo processo
  (`lib/Immediate/Immediate.cpp:61`, `:268`).
- Os stubs e o `RedirectionManager` (`llvm/ExecutionEngine/Orc/RedirectionManager.h:22-45`,
  presente na distribuição LLVM 22.1.8 local) são **API C++**. Na API C
  (`include/llvm-c/*.h`) não há `redirect` nem `updatePointer`, o que confirma
  `docs/JIT.md:189-200`.

**O que tirar:**

- **"Função substituível" é propriedade da declaração, e o compilador gera a
  indireção.** É o modelo mais próximo do que o DartForge quer: identidade
  estável e implementação substituível, num compilador AOT com LLVM.
- **O custo cai no prólogo do chamado, não em cada chamada**: um `load` e uma
  comparação no modo básico.

**O que não copiar:**

- A **cadeia de substituições**: no Dart há só a versão corrente.
- O TLS de "chamar o original".
- A **troca só na carga**.
- O `dlopen` do runtime com resolução pelo processo. O executor do DartForge usa
  uma tabela de símbolos absolutos gerada da fonte; ver o plano do JIT, §(c).

### 2.8 InjectionIII, HotReloading, InjectionNext e Inject: interposição em Swift

**Mecanismo.**

- A ferramenta recompila o arquivo editado, liga uma dylib e a carrega.
- Com o app ligado com `-Xlinker -interposable`, toda chamada a um símbolo
  global passa por um ponteiro gravável. O `fishhook` (`rebind_symbols_image`) ou
  o `dyld_dynamic_interpose` o regravam (`InjectionIII/README.md:200-215`,
  `HotReloading/Sources/HotReloading/SwiftInterpose.swift:110-137`).
- Para classes não `final`, a **vtable** da classe antiga é sobrescrita com a da
  nova (`SwiftInjection.swift:13-40`, `:226-256`).
- Uma varredura da memória acha instâncias vivas para enviar `injected()`
  (`SwiftSweeper.swift:6-8`).
- Estáticos podem ser preservados por interposição dos *addressors*
  (`SwiftInterpose.swift:146-165`).

**O que não consegue, e por quê:**

- **Propriedades armazenadas**: nada de adicionar, remover ou reordenar
  (`InjectionIII/README.md:80-84`). Não há runtime que conheça o layout nem
  converta instâncias. A memória é do programa, sem GC nem descrição de campos.
- **Mudar o número de métodos de classe não `final`.** A vtable é uma estrutura
  de dados de tamanho fixo, e o aviso diz «Your application will likely crash»
  (`SwiftInjection.swift:247-255`, `README.md:82-84`).
- **Símbolos `private`** não são interponíveis (`README.md:88-91`).
- **O tipo de retorno de `body` no SwiftUI** muda com a view, e isso é mudança de
  layout. A saída é embrulhar em `AnyView` com `.enableInjection()`
  (`README.md:118-131`).

**Inject**, o invólucro:

- o `ViewControllerHost` **recria** o controlador a cada injeção e perde o estado
  (`Inject/Sources/Inject/Integrations/Hosts.swift:39-66`);
- o SwiftUI observa uma notificação e redesenha
  (`InjectConfiguration.swift:78-80`, `README.md:85-86`).

**O que tirar:**

- **Um ponteiro gravável por símbolo, no lugar de chamar direto.** O custo é um
  salto indireto por chamada, sem quadro extra. É o formato certo para as nossas
  células (§4.3).
- **A lição sobre o framework:** sem cooperação dele, "recarregar" vira
  "recriar", e o estado se perde.

**O que não copiar:**

- A troca **sem conhecer o layout.** O DartForge tem runtime com heap descrito:
  pode e deve converter como a VM.
- **Patch de vtable de tamanho fixo.**

---

## 3. Tabela comparativa

| | Unidade de troca | Ponto seguro | Coexistência de versões | Migração de dados | Recusas | Custo no caminho feliz |
| --- | --- | --- | --- | --- | --- | --- |
| **Dart VM** | biblioteca modificada + dependentes transitivos; código recompilado sob demanda por função | safepoint de recarga via mensagem OOB, que pode ter quadros vivos (`safepoint.cc:97-163`) | quadros vivos, closures e `async` suspensos seguem no código antigo; chamadas novas vão ao novo | *morphing* por nome de campo com sentinela e inicialização preguiçosa; estáticos compartilhados; `const` atualizado; enums por nome | curta, 7 a 9 classes (§2.1); o resto vira `NoSuchMethodError` ou `TypeError` tardio | nenhuma indireção extra; o custo fica na recarga (desotimização, ICs zerados, recompilação) |
| **Mun** | assembly (dylib) | o hospedeiro chama `update()` fora de código Mun | não há: a dylib antiga sai na religação | conversão in loco via handles; campo novo zerado; heurística de rename | assinatura divergente; tipo ou dependência ausente | chamada entre assemblies: `load` + chamada indireta; toda leitura de objeto via handle |
| **Julia** | método (`CodeInstance`) | definição no topo; tarefas seguem no mundo delas | todas, por faixa de mundo; código não é removido (aparente) | nenhuma: `struct` redefinido é tipo novo (1.12+) | nenhuma no nível do método | filtro de mundo no cache de despacho; invalidação por backedges |
| **CoreCLR EnC** | método (versão de IL) e campo adicionado | depurador com o runtime suspenso | quadro vivo segue no velho ou é **remapeado** num ponto de sequência | campo novo em tabela lateral (`SyncBlock` + dependent handle) | *rude edits* do Roslyn via capabilities | métodos: não verificado; campos novos: acesso lento pela tabela |
| **Mono** | delta de metadados/IL | publicação sob `publish_lock`; cada thread expõe a geração | por geração exposta em TLS | campo novo adicionado (capability); detalhe não verificado | capabilities; só assembly de debug sem otimização | não verificado (interpretador: `should_invalidate_transformed_code`) |
| **Compose/JBR** | classe (redefinição da JVM) | agente, com recomposição na thread de UI | a JVM cuida; não verificado | JBR permite mudar campos; `<clinit>` alterado é reexecutado | método de entrada Compose; ViewModel só avisa | nenhum no código; custo na recomposição |
| **Swift `dynamic`** | função `dynamic` | carga de imagem ou escopo | cadeia de substituições; o original continua chamável | nenhuma | não se aplica (é declarativo) | prólogo no chamado: chamada de runtime + TLS, ou `load` + comparação |
| **InjectionIII** | arquivo → dylib | notificação de injeção | a dylib antiga fica carregada | nenhuma; propriedades armazenadas **não mudam** | layout, métodos de classe não `final`, `private` | `-interposable`: salto indireto por chamada a símbolo global |
| **BEAM** | módulo | não verificado | "antiga e atual" (tese) | — | — | não verificado |
| **DartForge hoje** (`crates/jit/src/reload.rs`, `docs/JIT.md`) | programa inteiro; troca por função | recarga fora de execução (`&mut self`) | gerações retidas até o fim da sessão | nenhuma | assinatura, função removida, campos, externo | **trampolim**: `load` + chamada + **quadro extra** por chamada (`docs/JIT.md:207`) |
| **DartForge proposto** (§4) | declaração (biblioteca + membro), aplicada por biblioteca suja + dependentes | chamada de runtime `hotReload()` (R1a), depois fronteira de turno do laço de eventos (R1b) | quadros, closures e `async` antigos no código antigo; geração liberada quando o GC prova que não está alcançável | *morphing* por nome, in loco pelos handles; sentinela + inicializador; estáticos no runtime | as da VM; o resto vira `NoSuchMethodError` ou `TypeError` | JIT: `load` da célula + chamada indireta, **sem quadro extra**; AOT: chamada direta, custo zero |

---

## 4. Proposta para o DartForge

### 4.1 Contrato com o backend nativo

A proposta assume o contrato atual do backend nativo. Nada nela depende de
recurso ausente no IR:

- IR textual único para AOT e JIT;
- exceção pendente verificada depois da chamada, sem desenrolamento através de
  código gerado (`docs/NATIVO-PLANO.md` §1);
- raízes explícitas por quadro: `push_frame`, `set_root`, `pop_frame`
  (`crates/runtime/src/heap.rs:316-372`);
- heap por *handles*, com campo que diz se é referência (`heap.rs:83-115`,
  `:169-172`).

### 4.2 Identidade estável de função, que é a mesma exigência da separação de módulos do SDK

**Hoje.**

- O nome da função depende da **ordem de carga**: `df_fn_{f_idx}_{nome}`
  (`crates/emit_native/src/lower/mod.rs:165-178`,
  `lower/fn_builder.rs:2309`, `:2378`, `:2620`).
- O `class_id` também é posicional, `c_idx + 1` (`lower/mod.rs:86-95`).
- Inserir uma função antes de outra renumera tudo. O próprio `docs/JIT.md:324-330`
  registra o efeito: a recarga passa a comparar funções diferentes.

**Proposta.** A identidade de declaração passa a ser:

```text
DeclId = (URI canônico da biblioteca, dono, nome, tipo de membro)
  dono          = classe | extensão | topo
  tipo de membro = método | getter | setter | construtor | inicializador de campo
  nome privado  = qualificado pela biblioteca (como a chave privada da VM, isolate_reload.cc:547)
```

- O símbolo LLVM é uma codificação legível e determinística do `DeclId`, por
  exemplo `df.<lib>.<Classe>.<membro>`, com `<lib>` saneado ou com hash curto e
  estável do URI.
- A **impressão digital do contrato de chamada** (aridade, nomes dos parâmetros
  nomeados, tipos de ABI) entra no nome da **célula** (§4.3), não no do
  `DeclId`.
- O `class_id` sai de uma tabela da sessão, indexada pelo `DeclId` da classe.
  Uma classe existente mantém o `cid`, como `RegisterClass` da VM
  (`isolate_reload.cc:1436-1437`).

**A mesma mudança destrava a separação de módulos do SDK.**

- Compilar `dart:core` e o resto do SDK **uma vez**, como objeto ou módulo
  próprio, e ligar o programa contra eles exige que o programa chame o SDK por
  nomes que não dependam do programa.
- Com nomes posicionais, `df_fn_812_add` em um programa é outra coisa no
  seguinte. O cache de objeto recém-entrado (`036e7dd`, cache por hash do IR) só
  acerta se o IR inteiro for igual.
- Com nome por `DeclId`, o módulo do SDK tem nomes fixos, o cache acerta por
  biblioteca, e a recarga por biblioteca (R2) é o mesmo mecanismo. **É uma
  mudança no emissor com três consumidores:** hot reload, cache por módulo e
  separação do SDK.
- Isso é pedido ao dono do `emit_native`, não edição avulsa. É também a
  pré-condição R1-2 do plano do JIT.

Prova (§5.3, T-ID): emitir o mesmo programa com uma função inserida no começo de
uma biblioteca e exigir o **mesmo símbolo** para todas as demais declarações. Emitir
dois programas diferentes que usam `dart:core` e exigir que os símbolos do SDK
sejam iguais.

### 4.3 A indireção: onde fica e quanto custa

**Hoje.**

- Um trampolim por função, que lê a célula e chama a implementação
  (`docs/JIT.md:145-187`).
- Custo declarado: um `load` e **um quadro de pilha por chamada** (`:207`).
- Esse quadro extra tem consequência semântica. A **profundidade de recursão**
  do JIT recarregável diverge do AOT; o teste T-dif-5 do plano do JIT (recursão
  até perto do limite) exige que ela bata.

**Proposta: a indireção fica no ponto de chamada, sem quadro extra.** É o formato
de Mun e do `-interposable`.

```llvm
; perfil JIT recarregável
@df.cell.<DeclId>$<abi> = external global ptr      ; definida pelo runtime como dado
  %alvo = load ptr, ptr @df.cell.<DeclId>$<abi>
  %r = call i64 %alvo(i64 %a)

; perfil AOT: o mesmo IR com a célula constante
@df.cell.<DeclId>$<abi> = internal constant ptr @df.impl.<DeclId>
```

**No AOT**, a célula é `constant`, e o LLVM dobra o `load` e transforma a chamada
em direta. Isso é suposição de otimizador, não verificada em `-O0`. **Alternativa
segura:** o emissor escolhe `call @impl` direto quando o perfil é AOT. Um
parâmetro do perfil, zero custo, e o mesmo HIR.

**No JIT**, o custo é um `load` (acerto de L1, em regime) mais uma chamada
indireta bem prevista. O custo de 1 a 3 ciclos por chamada é **estimativa não
medida**. Medir com a metodologia de `docs/DESEMPENHO.md`, comparando o
trampolim atual, a célula no ponto de chamada e a chamada direta, num programa
de chamadas curtas como `fib(30)`.

**Onde as células ficam.**

- Uma célula por `(DeclId, abi)`, num bloco de memória do runtime que não se
  move.
- A célula é publicada na `JITDylib` como símbolo **absoluto de dado**, o
  mecanismo que `reload.rs` já usa (`docs/JIT.md:168-170`).
- Nome por símbolo, e não índice numa tabela, para que o IR emitido **não
  dependa do estado da sessão**. Isso preserva o determinismo e o cache: o IR de
  uma biblioteca é função só da fonte.

**O que também passa pela célula:**

- tear-offs de funções de topo e estáticas;
- o `code_id` de tear-off (`crates/runtime/src/runtime_main.rs:923-929`).

Assim, `identical(f, g)` se preserva por identidade do `DeclId`, como a VM migra
tear-offs estáticos (`object_reload.cc:323-349`).

**O que não passa pela célula:** o **corpo de closure anônima**. Ela guarda o
`code_id` **da geração em que foi criada** e executa aquele corpo; as chamadas que
ela faz passam pelas células e veem o código novo. É a semântica de
`SavedClosure` (§1.7). Consequência: o `code_id` de closure precisa ser
**versionado por geração**, sem ser reaproveitado.

### 4.4 Mudança de assinatura sem recusa

- Quando o contrato de `DeclId` muda, nasce uma célula nova `(DeclId, abi')`. O
  código recompilado, que é o das bibliotecas sujas e dos dependentes (§4.8), só
  conhece a nova.
- A célula antiga `(DeclId, abi)` é **reescrita para um stub que lança
  `NoSuchMethodError`**, com o nome do membro. Só código antigo ainda a usa:
  quadros vivos, closures e `async` suspensos.
- Isso reproduz `CallDeleted_*ArityChange` e `PendingStaticCall_DefinedToNSM`
  (`isolate_reload_test.cc:3205-3235`, `:1479-1531`).
- Função apagada: mesma coisa, sem célula nova.
- **Nenhuma recusa.** Isso substitui as recusas «assinatura alterada» e «função
  que desaparece» de `docs/JIT.md:315-316`.

### 4.5 Ponto seguro

A VM aceita recarga com quadros vivos. Nós adotamos dois pontos, nesta ordem.

- **R1a: dentro de uma chamada de runtime `hotReload()`.**
  - O programa chama a função; o runtime pede ao executor que emita, carregue e
    publique a geração nova, e retorna.
  - Os quadros abaixo são código antigo e terminam nele. As chamadas que fazem
    depois do retorno vão às células novas.
  - É exatamente o `LiveStack` da VM (`isolate_reload_test.cc:1048-1079`), e
    **não exige laço de eventos nem `async`**. Serve ao corpus antes de o nativo
    ter `async` (ESTADO §2.5: «Continuam faltando `async` e event loop»).
- **R1b: na fronteira de turno do laço de eventos.** Quando houver laço, o
  `dartforge reload` observa arquivos e publica entre turnos. Não há quadro Dart
  na pilha nativa, mas **há** closures e continuações `async` apontando para
  código antigo (§1.7).

A troca é sempre na thread do isolado. As threads auxiliares só compilam: emitem
o IR e montam o módulo ORC. A publicação, com a escrita das células e o
*morphing*, acontece no ponto seguro. É a divisão da VM entre carregar e
comitar (`isolate_reload.cc:981-1113`).

### 4.6 Retenção e liberação do código antigo

- Hoje nenhuma geração é liberada (`docs/JIT.md:289-306`).
- O fim do quadro vivo não é o único critério: closures e `async` também seguram
  código.
- **Proposta:** o GC marca as **gerações alcançáveis**. Cada closure e cada
  continuação `async` guarda um `code_id` que pertence a uma geração; o `code_id`
  é versionado (§4.3).
- Num ponto seguro R1b, com a pilha nativa sem quadros Dart, uma geração sem
  `code_id` alcançável e sem célula apontando para ela é liberada com
  `ResourceTracker::remove`.
- Em R1a, com quadros antigos possíveis na pilha, **não se libera nada**.
- Critério de aceitação: a memória estabiliza depois de 100 recargas
  (PESQUISA-OTIMIZACAO §16).

### 4.7 Estado: heap, estáticos e layout de classe versionado

**Heap.** Já é preservado por construção: a sessão e o isolado continuam.

**O heap por handles é vantagem.** `Heap.slots: Vec<Option<Value>>`, e o objeto é
`Object { class_id, fields: Vec<(i64, bool)> }` (`heap.rs:83-92`, `:169-172`).
Converter uma instância é **trocar `slots[h]` no lugar**: o handle, e portanto
toda referência, continua válido.

- Não precisa de `become` nem de varredura de referências. A VM precisa
  (`isolate_reload.cc:1042-1073`); Mun não precisa, pelo mesmo motivo que nós
  (§2.2).
- **Regra de arquitetura:** se um dia o heap passar a ponteiros diretos por
  desempenho, a recarga passa a exigir `become`. Essa troca não é local.

**Layout de classe versionado.**

- O runtime passa a registrar, por `cid`, uma **versão de classe**: a lista de
  campos com nome, "é referência" e `DeclId` do inicializador.
- Hoje o runtime registra só o nome da classe (plano do JIT, §5.4).
- Na publicação, para cada `cid` cuja lista mudou, a conversão percorre os slots
  e reconstrói `fields`:
  - campos de mesmo nome são copiados, ignorando o tipo, como a VM
    (`isolate_reload.cc:119`);
  - campo novo recebe um **sentinela**.

**Leitura de campo.**

- Hoje, `dartforge_object_get` e `dartforge_object_set` são chamadas de runtime.
- A checagem de sentinela entra ali, **sem mudar o código gerado**. Ao achar o
  sentinela, o runtime chama o inicializador do campo pelo `DeclId` e grava o
  resultado; se ele lançar, a exceção sai da leitura
  (`RunNewFieldInitializersThrows`).
- Campo cujo tipo mudou e cujo valor não é mais subtipo: a guarda na leitura
  lança `TypeError` (`FieldInvalidator`, `isolate_reload.cc:2617-2636`). Isso
  exige que o runtime saiba testar subtipo, o que depende de genéricos
  reificados, ainda ausentes (ESTADO §2.5). Até lá, é **recusa declarada como
  limitação nossa**.

**Índice de campo no código gerado.** O emissor hoje acha o campo por índice,
procurando em todas as classes (`fn_builder.rs:2322-2329`). O layout está
embutido no código que acessa campos. Por isso a recompilação vale para as
bibliotecas sujas **e as dependentes** (§4.8), como a VM faz.

**Estáticos no runtime.**

- Cada variável de topo ou estática vira uma entrada `DeclId → (estado, valor)`:
  não inicializada, inicializando ou inicializada.
- A inicialização é preguiçosa, o que já é a semântica do Dart para topo e
  estáticos.
- Na recarga, uma entrada inicializada **fica**, mesmo que o inicializador tenha
  mudado (`StaticFieldInitialValueDoesnotChange`). Uma entrada nova começa não
  inicializada (`TopLevelFieldAdded`). **`const` é recalculado**
  (`object_reload.cc:218-225`; testes `SimpleConstFieldUpdate` e
  `ConstFieldUpdate`, `isolate_reload_test.cc:4864-4906`).
- Hoje o HIR do `emit_native` não tem instrução de estático (`hir.rs:85-229`), e
  o `crates/jit` antigo mostrava o defeito oposto: estáticos reinicializados a
  cada entrada (`docs/JIT.md:353`). A instrução nova deve nascer no formato
  acima: `StaticGet(DeclId)` e `StaticSet(DeclId)`, resolvidos por célula de
  dado.

**Enums.** O valor é identificado por `(cid, nome)`, não por índice. Hoje a chave
é `(class_id, index)` (`heap.rs:280-292`) e precisa passar a nome.

- Reordenar preserva `identical` (`EnumReorderIdentical`).
- Valor apagado vira o objeto "Deleted enum value from X" com `index == -1`
  (`EnumDelete`).

**Constantes canônicas.** Uma classe `const` mantém as constantes vivas, como
`CopyCanonicalConstants`. Uma classe `const` com constantes vivas que vira não
`const`, ou que perde campo, é **recusada**, como na VM.

### 4.8 Delta: o que recompilar

A partir das bibliotecas cujo texto mudou:

- recompila o **fecho transitivo dos dependentes por import e export**, como o
  `modified_libs_transitive` da VM (`isolate_reload.cc:880-891`);
- sem nenhuma biblioteca modificada, a recarga é **pulada**, com relatório de
  sucesso, como a VM (`:904-924`).

**Otimização futura.** Recompilar só o dependente cujo **resumo de interface**
usado mudou: assinaturas, layouts, constantes.

- É o resumo por biblioteca de PESQUISA-OTIMIZACAO §6.
- A detecção de corpo sujo usa impressão digital normalizada de HIR, como o
  `ScopeHash` do Compose (§2.6).
- Só é correta enquanto não houver inlining entre bibliotecas. Com inlining,
  será preciso registrar *backedges*, como a Julia (§2.3).

### 4.9 Recusas: espelhar `isolate_reload.cc`

**A lista final** (R2) é a da VM (§2.1), com as mesmas palavras em inglês no
relatório, para que a comparação com o oráculo seja por texto:

- enum ↔ classe;
- número de parâmetros de tipo;
- `const` → não `const`, e classe `const` que perde campo;
- `@pragma('vm:deeply-immutable')`;
- número de campos nativos (`NativeFieldWrapperClass`, FFI);
- erro de compilação (`Aborted`).

**Durante R1**, recusas **nossas**, marcadas como limitação («limitação do
DartForge: … exige reinício») e retiradas uma a uma:

- mudança de layout de classe com instâncias vivas, até o *morphing* existir;
- tipo de campo alterado, até haver teste de subtipo no runtime.

**Validação nos dois lados.** O front-end decide a partir de capabilities que o
runtime declara (lição do .NET, §2.4). O runtime revalida na publicação, como
`Class::CheckReload`.

**Transação.** Uma recusa ou falha em qualquer etapa antes da escrita das células
deixa a versão viva intacta. A publicação em três fases de `reload.rs`
(`docs/JIT.md:221-242`) já é isso. Falta estender o checkpoint à tabela de
classes, às versões de classe e aos estáticos novos, como `CheckpointClasses`.

**Relatório.** O formato é o do `ReloadReport` da VM: `success`, `notices` com os
motivos, `details` com contagens e `shapeChangeMappings`. O corpus compara o campo
`success` e o texto dos motivos.

---

## 5. Ordem de implementação e como provar

### 5.1 O corpus de reload, com a VM como oráculo

**O formato já existe, no SDK.**

- `tests/hot_reload/<nome>/main.N.dart`: a geração N do arquivo (173 diretórios
  em `references/dart-sdk`).
- `.reject` no nome indica geração que deve ser recusada, com `config.json`
  `{"expectedErrors": {N: "…"}}`; `.restart` indica hot restart
  (`pkg/reload_test/README.md`).
- O programa se verifica sozinho com `Expect` e chama `await hotReload()` e
  `hotReloadGeneration` de `package:reload_test` (exemplo:
  `tests/hot_reload/add_new_static_field/main.0.dart` e `main.1.dart`).

**Como o SDK roda esse corpus contra a VM** (`pkg/dev_compiler/test/hot_reload_suite.dart`):

1. O `frontend_server` roda com `--incremental` e gera um `.dill` por geração
   (`:1424-1448`), com `--platform=vm_platform.dill --target=vm` (`:1807-1811`).
2. A VM inicia na geração 0 com `--enable-vm-service=0
   --disable-service-auth-codes --disable-dart-dev generation0/x.dill`
   (`:1797-1860`).
3. O programa se conecta ao próprio VM service. `Service.controlWebServer` dá a
   URI (`pkg/reload_test/lib/src/_vm_reload_utils.dart:100-124`).
4. O programa chama `reloadSources(isolateId, rootLibUri: generationN/x.dill)`
   (`:130-152`, RPC em `pkg/vm_service/lib/src/vm_service.dart:1565-1599`). A
   rejeição esperada é conferida pelo `ReloadReport` (`:154-221`).

**O que o SDK 3.6.2 local tem, conferido por listagem:**

- `bin/snapshots/frontend_server_aot.dart.snapshot`;
- `bin/dartaotruntime.exe`;
- `lib/_internal/vm_platform_strong.dill`.

**Não verificado:**

- se `tests/hot_reload`, `pkg/reload_test` e as flags acima existem ou valem na
  tag 3.6.2. A cópia de referência é 3.14-dev;
- se `package:vm_service` resolve sem rede.

**Alternativa sem pacote:** o cliente fala JSON-RPC por `WebSocket` de
`dart:io`, só com o método `reloadSources`.

**Outra rota, também não verificada:** `reloadSources` **sem** `rootLibUri`,
depois de editar o `.dart` no disco. Nesse caso a VM acha as fontes modificadas
por data (`FindModifiedSources`, `isolate_reload.cc:1642`) e compila pelo kernel
service (`CompileToKernel`, `:851-873`). Dispensaria o `frontend_server`.

**Proposta de corpus: `corpus/reload/`.**

- Formato idêntico ao do SDK, para importar os 173 casos (licença BSD, com o
  cabeçalho preservado) e escrever os nossos.
- O `package:reload_test` do corpus tem duas implementações do mesmo URI,
  escolhidas pelo `package_config` do executor:
  - **VM**: o helper acima;
  - **DartForge**: `hotReload()` chama `dartforge_hot_reload_request(geração)`,
    uma chamada de runtime que publica a próxima geração (R1a).
- `hotReloadGeneration` é um estático de biblioteca não modificada e **tem de**
  sobreviver à recarga. É o primeiro teste de estáticos.
- O oráculo é a VM:
  - executa cada caso uma vez;
  - grava stdout, código de saída e o recibo por geração (`HotReloadReceipt`:
    aceita ou recusada, com o motivo);
  - o executor do DartForge tem de produzir **os mesmos bytes**.
- Os casos vêm do `tests/hot_reload`, e o resultado esperado sai da execução da
  VM, não do texto do teste.

**Casos síncronos, antes de `async`.** Os testes do SDK usam `async main` e
`await hotReload()`, e o nativo ainda não tem `async`. A primeira leva do corpus
vem dos **testes unitários da VM**:

- são 172, em `runtime/vm/isolate_reload_test.cc`;
- usam `reloadTest()` síncrono dentro de `main`, e o resultado esperado está no
  próprio teste (por exemplo `EXPECT_STREQ("init()=new value,value=old value",
  …)`, `:389-414`);
- aqui o oráculo é a **expectativa escrita no teste da VM**, transcrita com
  arquivo:linha em cada caso, porque a CLI `dart` não oferece recarga síncrona
  de dentro do programa;
- quando `async` existir, cada caso ganha a forma `await hotReload()` e passa a
  ser verificado ao vivo contra a VM.

### 5.2 Os passos

| Passo | O que entrega | Exige antes | Como provar |
| --- | --- | --- | --- |
| **R0** — reinício a quente (plano do JIT §5.2) | `dartforge reload` reemite e reexecuta num isolado novo; edição com erro mantém a versão anterior | executor JIT sobre `emitir_ir` (plano do JIT, passos 1–7) | 3 edições seguidas; edição com erro não derruba a sessão; RSS estável em 100 reinícios. **Não** chamar de hot reload |
| **R1-0** — nomes estáveis | símbolos por `DeclId`, `cid` por tabela da sessão, enum por nome (§4.2) | pedido ao dono do `emit_native` | **T-ID**: inserir uma declaração não muda nenhum outro símbolo; dois programas → mesmos símbolos do SDK; `determinismo --trabalhadores 1,4,8` segue verde |
| **R1-1** — células no ponto de chamada | perfil JIT recarregável com `load` + chamada indireta; perfil AOT com chamada direta (§4.3) | R1-0 | IR do AOT **sem nenhuma** célula (grep no `.ll`); T-dif-5 (recursão) igual entre AOT e JIT recarregável; medição do custo por chamada (§4.3) |
| **R1-2** — estáticos no runtime | `StaticGet`/`StaticSet(DeclId)`, inicialização preguiçosa, `const` recalculado (§4.7) | R1-0; decisão de representação no HIR | casos `StaticValuePreserved`, `TopLevelFieldAdded`, `StaticFieldInitialValueDoesnotChange`, `SimpleConstFieldUpdate` do corpus síncrono |
| **R1a** — `hotReload()` síncrono | recarga dentro de chamada de runtime, publicação em três fases, `NoSuchMethodError` por célula antiga (§4.4, §4.5) | R1-1, R1-2 | corpus síncrono: `LiveStack` (107/105), `PendingStaticCall_DefinedToNSM`, `CallDeleted_*`, `TearOff_*`, `SavedClosure` (este exige `code_id` versionado) |
| **R1b** — laço de eventos | recarga entre turnos, observador de arquivos, liberação de geração pelo GC (§4.6) | laço de eventos e `async` no nativo (ESTADO §2.5) | corpus `tests/hot_reload` ao vivo contra a VM; memória estável em 100 recargas |
| **R2-1** — *morphing* | versão de classe no runtime, conversão in loco por nome, sentinela e inicializador na leitura (§4.7) | R1a | `ClassFieldAdded`/`Removed`, `RunNewFieldInitializers*`, `ShapeChange*` |
| **R2-2** — recusas e relatório da VM | lista de §4.9 com os textos da VM; `ReloadReport` | R2-1 | casos `.reject` e `EnumToNotEnum`, `ConstToNonConstClass`, …, com texto do motivo igual ao da VM |
| **R2-3** — delta por biblioteca | um módulo LLVM por biblioteca, com tracker próprio; fecho de dependentes; recarga pulada sem mudança (§4.8) | R1-0 (é o mesmo trabalho do cache por módulo e da separação do SDK) | recarga de 1 biblioteca num programa de N recompila só ela e os dependentes (contador no relatório); tempo por recarga medido |
| **R2-4** — enums, hierarquia, tipos de campo | enums por nome, mudança de superclasse, `TypeError` por guarda | genéricos reificados (teste de subtipo no runtime) | `Enum*`, `SuperClassChanged`, `ExistingFieldChangesType*` |

### 5.3 O que medir em cada recarga

O mesmo formato de `--timings` de `docs/JIT.md:276-281`, com mais dois campos:
`bibliotecas_recompiladas` e `instancias_convertidas`. O critério de desempenho
não é tempo absoluto: é **quanto cresce com o tamanho do programa** quando só uma
biblioteca muda (PESQUISA-OTIMIZACAO §9, "edição pequena de corpo").

---

## 6. Riscos e perguntas em aberto

**Riscos**

1. **Closures e `async` seguram código antigo.** Liberar geração sem o GC ver
   `code_id` versionado transforma código em memória liberada sob uma closure
   viva. Mitigação: §4.6; até lá, não liberar (R1a).
2. **Divergência de profundidade de pilha.** O trampolim atual adiciona um
   quadro por chamada. Mitigação: células no ponto de chamada (R1-1), com T-dif-5
   como prova.
3. **RTDyld COFF e relocação para células.** Um `load` de símbolo absoluto
   distante pode exigir relocação de 64 bits (risco R1 do plano do JIT, não
   verificado). Mitigação: um teste com célula absoluta num endereço alto, antes
   de R1-1.
4. **Nome estável colide com determinismo e cache.** Se o nome depender de
   estado da sessão (índice de tabela), o IR deixa de ser função da fonte. Por
   isso a célula é por nome (§4.3).
5. **O despacho ainda vai mudar.** Hoje, a chamada de método de instância é
   resolvida por nome estático, direto (`fn_builder.rs:2306-2320`), e o campo por
   índice procurado em todas as classes (`:2322-2329`). Quando houver despacho
   virtual de verdade (vtable ou seletor), ele precisa nascer **pela identidade
   de §4.2**: tabelas por geração e seletor por nome. Caso contrário, a recarga
   de método de instância não tem onde se apoiar. Isso vem antes de escrever o
   despacho, não depois.
6. **Inlining no JIT.** Com `-O0` não há inlining entre funções: o fecho de
   dependentes por import (§4.8) basta. Se um dia houver `-O2` no JIT, a
   invalidação exige *backedges* (Julia) ou a regra da VM: descartar e
   recompilar, nunca remendar (`docs/JIT.md:351`).
7. **Conversão durante o GC.** Com o heap por handles, a conversão é atômica em
   relação ao GC se feita inteira no ponto seguro, sem alocar no meio. A VM força
   crescimento do heap nesse intervalo (`isolate_reload.cc:1046-1052`). Os
   objetos novos da conversão precisam ser alocados **antes** de trocar os
   slots, ou com a coleta suspensa.
8. **Ecossistema e tempo.** Oráculo por versão: o corpus importado é da 3.14-dev.
   Casos que dependam de comportamento novo precisam ser checados contra a VM
   3.6.2 antes de entrar.

**Perguntas em aberto**

1. O SDK 3.6.2 tem `tests/hot_reload` e aceita `--disable-dart-dev`? E
   `reloadSources` sem `rootLibUri` recompila da fonte? Verificar executando,
   uma vez, com a máquina livre.
2. Onde o executor pede a recarga em R1a? O runtime é `thread_local`, e o
   executor vive no mesmo processo. Proposta: uma função de *callback*
   registrada pelo executor na tabela de símbolos, e `dartforge_hot_reload_request`
   chama esse callback. Confirmar que o ORC aceita `add_module` e `lookup`
   reentrantes a partir de código JIT em execução. O `LazyCallThroughManager` do
   `lib/Immediate` faz compilação dentro de chamada (§2.7), o que sugere que sim.
   Não verificado.
3. `identical` de tear-off estático através da recarga: a VM migra a closure
   canônica (`object_reload.cc:323-349`) e preserva o hash
   (`StaticTearOffRetainsHash`, `isolate_reload_test.cc:4648`). A nossa
   `dartforge_tearoff(code_id)` precisa do `DeclId` como `code_id`. Confirmar a
   representação.
4. Hot restart: a VM não o implementa (é o Flutter tooling que recria o
   isolado; não verificado). No DartForge, R0 **é** o hot restart. Faz sentido
   expor `--restart` explícito e manter `reload` para R1+?
5. ngdart e UI: a recarga não redesenha nada, e as referências mostram que o
   framework precisa cooperar. Exemplos: Compose `invalidateGroupsWithKey`
   (§2.6), Inject com `@ObserveInjection` e a recriação do host (§2.8), e o
   `reassemble` do Flutter (não verificado). Para o ngdart, que roda em JS, o
   gancho equivalente seria disparar uma detecção de mudanças em
   `ApplicationRef` depois da geração nova. Isso é do backend JS e do `dartforge
   serve`, fora deste documento; registrar aqui para não se perder.
6. BEAM: se a afirmação da tese sobre duas versões por módulo for relevante para
   o desenho, baixar `erlang/otp` (`erts/emulator/beam/beam_load.c` e o código de
   `code_purge`) e verificar.
