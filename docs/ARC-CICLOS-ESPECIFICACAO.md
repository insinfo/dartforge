# Especificação de pesquisa — ARC com coleta de ciclos no backend nativo

**Estado:** proposta de engenharia e plano de prova; não é implementação nem mudança
do gerenciador padrão. **Documento-base imutável:**
[`EXPERIMENTO-ARC.md`](EXPERIMENTO-ARC.md). Este texto detalha aquele experimento,
sem alterar sua sintaxe, suas condições de entrada, suas garantias de Dart ou sua
decisão de manter tracing como referência. **Alvo semântico inicial:** Dart 3.6.2.

## 0. Escopo, linguagem de certeza e critério de aceitação

ARC significa *automatic reference counting*: operações inseridas pelo compilador,
sem `retain`, `release`, `weak` ou anotações novas no código Dart. O mecanismo inclui
um coletor de ciclos; ARC puro é insuficiente. O modo experimental deve ser opt-in,
por exemplo `--memoria=arc`, e coexistir com o modo tracing. Um programa só pode
ser aceito pelo compilador ARC se todas as suas arestas, raízes, transições nativas
e mecanismos fracos tiverem semântica implementada; não converter uma falha do
verificador em uma execução com vazamento ou uso após liberação.

**Requisito:** comportamento observável admitido pela VM Dart, não cronologia
idêntica de coletas. `identical`, mutações, ordem de avaliação, exceções, isolamento,
resultado de `WeakReference` quando o alvo ainda é fortemente alcançável, regras de
`Finalizable`, agendamento de `Finalizer` e encerramento de `NativeFinalizer` não
podem mudar. O momento de desaparecimento de objeto já inalcançável, e a execução
de callbacks sem garantia temporal, podem variar apenas dentro das permissões da
API. `dispose()` e `close()` jamais viram destrutores implícitos.

**Hipótese, a medir:** operações RC eliminadas pelo otimizador podem compensar o
custo de `retain`/`release` remanescente e reduzir trabalho de tracing em certos
programas. Nem velocidade, nem menor pico, nem latência limitada são pressupostos.

**Fora desta especificação:** trocar a sintaxe Dart; expor ownership ao usuário;
substituir o modelo de erros por destrutores; transformar referências fortes em
fracas automaticamente; prometer tempo real; assumir que o passe ObjC ARC do LLVM
ou o borrow checker do Rust gerenciam objetos Dart.

## 1. Evidência estudada e limite de transferência

| Fonte | Evidência aproveitável | Limite para DartForge |
| --- | --- | --- |
| Nim, `doc/mm.md`, `lib/system/orc.nim`, `lib/system/arc.nim`, `lib/system/cyclebreaker.nim` | ORC é RC com *trial deletion*; a implementação mantém candidatos, marca cinza, restaura subgrafos alcançáveis e libera brancos em lote. Nim distingue ARC puro de ORC e registra ciclos do `async`. | Destrutores e recursos de Nim não definem quando um `Finalizer` Dart executa. A otimização `acyclic` de Nim não pode aparecer como exigência de anotação em Dart. |
| Swift, `docs/OwnershipManifesto.md`, SIL e runtime | Modelo intermediário com valores owned/guaranteed, transferência e verificação; inspiração para representação e diagnóstico do compilador. | SIL, ABI de Swift e runtime de objetos Swift não são a ABI Dart. |
| Koka, `doc/spec/why.kk.md`, `src/Compile/Optimize.hs`, primitivas `@dup`/`@drop` | Perceus insere contagem na IR e otimiza cópias e reaproveitamento; reforça a necessidade de medir operações RC depois da otimização. | Seu modelo e suas provas de reutilização não autorizam copiar ou mover silenciosamente objetos Dart mutáveis com identidade observável. |
| LLVM, `llvm/lib/Transforms/ObjCARC` | Exemplos de eliminação de pares retain/release e movimento seguro dessas operações. | Passe específico de Objective-C, `objc_*` e semântica de autorelease: não reutilizar diretamente para objetos Dart. |
| TinyGo, `compiler/gc.go`, `src/runtime/gc_precise.go`, `gc_blocks.go` | IR registra ponteiros vivos; descritores precisos de heap e varredura de blocos mostram como auditar layouts e raízes. | É tracing, não uma implementação de ARC ou de coleta de ciclos. |
| gollvm e gofrontend, `bridge/go-llvm.cpp`, `libgo/` | Integração de estratégia GC ao LLVM, identificação de chamadas GC leaf, barreiras e fronteira com runtime; README da revisão pedida descreve o arranjo dos repositórios. | São arquitetura de compilação e tracing de Go; `libffi` e `libbacktrace` são dependências para FFI e rastros, não coletores de ciclos. O README antigo não é receita válida para compilar LLVM atual. |
| DartForge, `crates/runtime/src/{heap,espaco,finalizadores,nativos_sistema}.rs`, `crates/emit_native/src/{hir,lower,llvm}.rs` | Representação de `Ref`, bits de aresta por campo, raízes de stack, objetos permanentes, weak, ephemeron, eventos e FFI existentes. | O modo ARC ainda não existe; esses arquivos são pontos de integração, não prova de que ARC passa nos testes. |

As revisões exatas e licenças dos clones constam da seção 17 e do catálogo
[`REFERENCIAS.md`](REFERENCIAS.md). Não copiar código de referência para o
runtime; adaptar ideias, registrar a origem e testar os contratos Dart.

## 2. Estado do DartForge que o experimento deve respeitar

O heap de tracing já distingue campos `(bits, e_ref)`; `Campo` e o mapa de
referências do cabeçalho são o ponto de partida para enumerar arestas fortes.
`Ref` é um handle etiquetado: `null`, `Smi` e outros escalares não recebem RC;
objetos gerenciados recebem. Há estáticos e objetos permanentes, raízes globais,
literais, constantes canonizadas, tear-offs, quadros de código gerado e raízes do
runtime. O modo de mapas de pilha e o modo de pilha-sombra são contratos atuais
da trilha tracing; não apagar essas informações antes de provar ARC.

O heap mantém tabelas para `WeakReference`, propriedades efêmeras de `Expando`,
anexos de `Finalizer`/`NativeFinalizer` e callbacks prontos para o laço de eventos.
O coletor atual faz ponto fixo de efêmeros. O modo ARC deve manter a mesma
classificação das arestas: o portador de uma entrada de metadados pode ser forte;
alvo fraco e chave de ephemeron não o são; valor de ephemeron é condicionalmente
forte. Uma varredura genérica de todos os `i64` destruiria esse contrato.

O runtime Rust chama código Dart por portas; Dart chama runtime e FFI. O modo de
exceções pode ser `checagem` ou `tabelas`. Toda proposta de ownership vale para
ambos: retorno normal, exceção pendente, desenrolamento por landing pad,
`finally`, cancelamento de `async`, callbacks e encerramento do isolate.

## 3. Modelo formal mínimo

Seja `H` o conjunto de objetos Dart gerenciados de um isolate. Para `x ∈ H`,
`E(x)` é um multiconjunto de **arestas fortes**, cada ocorrência com origem e
destino identificáveis; multiplicidade importa quando dois campos apontam ao
mesmo objeto. `R` é o multiconjunto de raízes owned externas ao heap: locais,
globais, runtime, portas, tarefas assíncronas e handles nativos. `I` é a classe
de objetos imortais/canonizados com política explícita. Fracos não entram em
`E`; ephemerons têm função de ativação separada.

Invariante RC, fora da pausa de uma operação transacional:

```text
rc(x) = multiplicidade(R → x) + Σ_{y∈H} multiplicidade(E(y) → x)
```

Para um objeto normal, `rc=0` permite iniciar a desalocação; não autoriza
executar código Dart sincronicamente. Objetos imortais não contam para liberar
seus próprios cabeçalhos, mas **as arestas fortes que saem deles** mantêm seus
destinos vivos. Uma aresta interna de um ciclo mantém `rc>0`; o coletor de ciclos
deve provar que não há caminho de raiz externa antes de romper o grupo.

**Ponto observacional:** uma operação do heap só expõe estado após publicar a
nova aresta e estabilizar o contador. Coleta, callback, sinal, FFI reentrante e
`throw` não podem observar metade de uma substituição. A implementação pode
combinar instruções, mas tem de preservar esse efeito como relação de ordem.

### 3.1 Unidade de ownership

Uma ocorrência owned vale **uma** contribuição ao contador. Um uso guaranteed
tem empréstimo limitado por uma ocorrência owned que permanece viva até o fim
do uso. Uma referência bruta não gerenciada não pode atravessar chamada que
libera, coleta ciclos, realoca ou reentra em Dart; se precisar atravessar, vira
handle owned registrado. Alias não cria nova ocorrência por si só: criar um
segundo proprietário exige `retain`; mover a ocorrência não exige.

### 3.2 Identidade e armazenamento

O handle visível ao código gerado deve continuar identificando o mesmo objeto
durante toda a vida dele. ARC não transforma objeto em valor nem usa contagem
para decidir igualdade. Reutilização de endereço depois da morte exige que
`WeakReference`, tabelas de identidade, finalizadores e depuração não confundam
gerações: entrada fraca guarda geração/identidade ou é removida antes do reuso.
`const` canonizado, valores de enum e sentinelas têm política de retenção
compatível com as tabelas atuais; um cache que deveria ser fraco não pode virar
raiz permanente só para simplificar RC.

## 4. Representação do objeto e descritores

Proposta de cabeçalho lógico (layout físico a fixar por alvo e medição):

```text
estado/classe/layout_versao | rc ou marcador imortal | bits de candidato/cor
identidade_geracao       | tamanho/descritor         | campos do objeto
```

O descritor por versão de layout fornece: tamanho e alinhamento; iterador de
arestas fortes, inclusive elementos de `List`, `Map`, `Set`, records, objetos de
tipo e arrays; campos fracos/ephemeron; política de finalização; função de
rompimento de arestas; tipo de isolate; e versão para hot reload. Tipos sem
arestas fortes podem usar caminho rápido comprovado. O bit de referência do
heap tracing já fornece evidência de layout, mas não substitui as operações de
barreira: a escrita deve atualizar o contador na mesma transação.

Todo objeto gerenciado, inclusive closure, ambiente, `Future`, callback,
`SuspendState`, representação de `Type`, exceção e `StackTrace`, precisa de
descritor. Buffers nativos com ponteiros Dart precisam de adaptador com ciclo
de vida declarado; ponteiros `void*` arbitrários não são arestas ocultas.

**Hot reload:** objeto antigo continua usando seu descritor antigo. Se uma
classe ganha, perde ou reordena campos, a migração conserva identidade e ajusta
os contadores numa transação, ou mantém a versão antiga acessível. Nunca
interpretar campos antigos com um layout novo.

## 5. Operações primitivas e ordem de efeitos

Interface conceitual do runtime, com semântica independente do nome final:

| operação | contrato |
| --- | --- |
| `arc_new(layout)` | Cria objeto inicializado sem arestas publicadas; retorna uma ocorrência owned. Não libera temporários vivos durante a alocação. |
| `arc_retain(x)` | Se `x` for objeto normal, acrescenta uma ocorrência com checagem de overflow; é no-op para null, escalares e imortais. |
| `arc_release(x)` | Retira uma ocorrência; em zero, agenda rompimento e liberação iterativos. Se resta positivo e o objeto pode participar de ciclo, registra candidato sem duplicá-lo. |
| `arc_replace(slot, novo)` | Obtém a nova ocorrência antes de soltar a velha; suporta `slot == novo`, `a.f=a.f`, setter reentrante e mudança de classificação do campo. |
| `arc_move(slot_origem, slot_destino)` | Transfere a única ocorrência; origem fica vazia antes de qualquer ponto observável. Não cria RC adicional. |
| `arc_weak_load(w)` | Lê alvo ainda vivo sob uma seção crítica de weak e o promove a owned antes de qualquer possível `release`. Devolve null se já morto. |
| `arc_cycle_step(budget)` | Executa trabalho de ciclo num ponto seguro, sem callback Dart e sem usar ponteiros cuja vida não esteja garantida. |

**Pseudo código da substituição simples** (a publicação física pode variar):

```text
antigo = slot
se novo é objeto gerenciado: retain(novo)
slot = novo
release(antigo)
```

Essa ordem evita liberar `novo` quando é igual a `antigo`. Para transferir um
owned temporário a um campo, o compilador pode consumir o temporário sem
`retain`, mas somente depois de provar que ele tem proprietário independente
até a publicação. Se o `release(antigo)` chega a zero, sua cascata ocorre após
o estado novo estar visível. A implementação não pode chamar `free` recursivo
profundo: usa fila de objetos a romper, com orçamento ou drenagem controlada.

**Falhas e reentrância:** overflow de RC é erro do runtime, não wraparound;
OOM durante fila de candidatos usa reserva de emergência ou mantém objetos
vivos até uma coleta segura; falha não pode deixá-los liberados cedo. Nenhuma
operação de contador entra em código Dart arbitrário. Uma extern Rust pode
registrar callbacks/handles, mas entrega ou retira ownership em ponto fixado.

## 6. Ownership na HIR e nas chamadas

Adicionar metadados internos `owned`, `guaranteed` e `trivial`, sem acrescentar
token Dart. Para cada valor, a HIR deve registrar origem, consumo, prazo de
empréstimo e saída em aresta de CFG. Instruções conceituais: `copy_value`,
`begin_borrow`, `end_borrow`, `move_value`, `destroy_value`, `store_strong`,
`load_strong`, `load_weak`, `call_owned`, `call_borrowed`. Somente referências
gerenciadas participam; escalares etiquetados são triviais. Blocos φ fazem
merge de propriedade **por aresta predecessora**, não duplicam contagem.

Convenção inicial conservadora:

| fronteira | entrada | saída e obrigação |
| --- | --- | --- |
| chamada Dart direta/dinâmica | argumento guaranteed durante toda a chamada; callee que o guarda retém | resultado objeto owned; chamador consome, transfere ou libera em toda saída |
| `this`, argumento nomeado/default, closure | `this` e cada argumento permanecem vivos durante avaliação sequencial | ambiente/célula captura ocorrência própria se sobreviver à chamada |
| getter/setter/operador | mesma regra de chamada; side effects não podem ser deslocados pelo passe RC | resultado e antigo valor têm donos separados quando necessário |
| runtime Rust → Dart | porta constrói owners temporários para argumentos e recebe resultado owned | em retorno e exceção limpa todos os owners e registra resultado antes de soltar a porta |
| Dart → runtime Rust | runtime declara `borrow`, `consume`, `return-owned` ou `return-immortal` em tabela de efeitos | regra auditada para cada extern; desconhecida impede ARC |
| FFI e callback C → Dart | ponteiro Dart não pode ser guardado sem handle de API owned | liberação do handle é operação RC; exceções não escapam por ABI C indevida |
| isolate → isolate | serialização/cópia ou transferência explícita conforme a semântica atual | nenhum objeto comum fica com contador não atômico compartilhado |

No caminho de exceção, `throw e` transfere uma ocorrência para a pendência.
`catch` toma posse ou empresta conforme a HIR; `rethrow` preserva o objeto e
o rastro. Toda instrução que pode lançar tem aresta excepcional com limpezas
de locais owned exatamente uma vez. Para exceções por tabelas, o landing pad
executa essas limpezas antes do despacho, sem deixar o desenrolador atravessar
um quadro Rust `extern "C"`. Para checagem de pendência, o bloco de erro faz a
mesma limpeza. `finally`, `break`, `continue` e retorno antecipado são saídas
distintas que o verificador deve percorrer.

`await` transfere owners vivos ao estado suspenso **antes** de fechar o
quadro; a retomada recupera ownership sem retenção dupla. Cancelamento e
completar Future em erro liberam o estado por caminho próprio. Uma closure
que captura a própria célula cria ciclo forte legítimo para o coletor de
ciclos, inclusive quando participa de `Future.then`.

## 7. Verificador de ownership e otimizações permitidas

Executar verificador antes e depois do passe RC, e de novo após lowering que
altera CFG. Cada ocorrência owned é consumida uma vez em todo caminho;
guaranteed não sobrevive ao owner; nenhum uso ocorre depois de `destroy` ou
`move`; resultado de chamada não fica sem dono; toda aresta excepcional
equilibra suas ocorrências. A checagem deve considerar φ, loop, irreducível,
chamada indireta, `late`, padrão, `try/finally`, inlining e tail call. Em
debug, inserir contadores de referência e geração para acusar primeira
violação, com função, instrução e origem do owner.

Otimizações são provas locais ou interprocedurais com resumo verificado:

1. Eliminar `retain(x); release(x)` quando nenhum uso, publicação, safepoint,
   reentrada, finalização ou alteração de weak ocorre entre ambos.
2. Propagar ownership de resultado diretamente ao armazenamento/retorno,
   mantendo avaliação na ordem Dart.
3. Emprestar argumento temporário se o proprietário vive até o retorno,
   inclusive em caminho excepcional.
4. Fundir retain/release de φ e laços apenas com prova em todas as arestas.
5. Classificar função como `noescape`/`norelease` por corpo e chamada transitiva;
   chamada externa sem resumo usa convenção conservadora.

Não mover `release` através de chamada com efeito desconhecido, acesso weak,
saída do bloco exigida por `Finalizable`, publicação em outro isolate ou
observação de identidade. `llvm.lifetime.end` não é prova de morte de Dart;
otimização de LLVM posterior não pode apagar os efeitos das primitivas RC.
Registrar no IR atributos de efeito corretos (`nounwind`, `readonly`, etc.)
somente depois de verificar a implementação, especialmente em presença de
coleta de ciclos e hooks de FFI.

## 8. Coleta de ciclos: *trial deletion* segura

Objetivo: recuperar subgrafos fechados com RC positivo causado só por arestas
internas. Seguir a ideia de Nim ORC/Bacon–Rajan, sem importar seus destrutores.
Cada `release` que não zera o RC de objeto potencialmente cíclico o coloca,
uma vez, no buffer de candidatos. Descritor comprovadamente sem caminho de
retorno pode dispensar candidatura; isso exige prova de layout, inclusive
coleções mutáveis e subclasses.

Uma rodada no isolate tem fases explícitas:

1. **Selecionar região candidata.** Drenar até um orçamento de candidatos;
   capturar estabilidade do grafo por pausa curta no mutador, ou usar barreira
   e versão de aresta que obrigam reinício ao detectar mutação. Primeira
   implementação deve usar pausa, por ser mais fácil de verificar.
2. **Marcar cinza / subtração experimental.** Para cada objeto alcançado por
   arestas fortes da região, registrar `trial(x)=rc(x)` e subtrair **cada
   ocorrência interna**, uma única vez por aresta. Não alterar o contador
   verdadeiro; usar contador lateral evita deixar RC corrompido após OOM.
3. **Localizar entradas externas.** Todo nó com `trial>0` é raiz preta da
   região. Restaurar alcançabilidade preta transitivamente. Nós restantes
   são candidatos brancos; qualquer aresta nova, alteração de versão ou
   objeto pinado aborta/reinicia a rodada.
4. **Confirmar os brancos.** Antes de romper arestas, executar política de
   weak/ephemeron/finalizer da seção 9. Para regiões com efêmeros, FFI não
   classificada ou dúvida de layout, recorrer a marcação de segurança a
   partir das raízes atuais, usando os mapas de pilha/raízes ainda mantidos;
   se não puder provar morte, conservar e diagnosticar, nunca liberar.
5. **Isolar e liberar.** Marcar todo branco como `collecting`, retirar suas
   arestas para nós pretos ajustando seus RCs, invalidar fracos, retirar
   entradas de cache, agendar finalizadores, e só então desalocar os brancos.
   Não chamar `release` normal em cada aresta interna branca: isso daria
   double free. Descritores processam cada campo e elemento exatamente uma
   vez; deallocação usa fila, não recursão.

Invariantes de prova: (a) objeto com caminho forte de raiz jamais vira branco;
(b) toda aresta que atravessa a fronteira branca→preta é debitada exatamente
uma vez; (c) o objeto branco não é entregue a callback Dart; (d) uma rodada
abortada não modifica RC real; (e) buffers e cores não guardam ponteiro para
memória liberada. Recolher candidatos pode ser incremental, mas **liberação
de um grupo precisa ocorrer como uma transação** ou carregar um estado de
quarentena seguro contra leitura concorrente.

Orçamento: candidatos por rodada, nós visitados, bytes de trabalho e tempo
de pausa têm limites medidos. Pressão de memória permite rodada completa.
`--gc-stress` equivalente solicita tentativa a cada alocação segura; não
transforma callbacks opcionais em garantidos. Uma cascata acíclica muito
grande também precisa de limite ou fila para não esgotar a pilha nativa.

## 9. Fracos, ephemerons e finalização

### 9.1 `WeakReference`

Uma entrada weak não incrementa RC. Quando `rc=0` ou um grupo cíclico vai
ser efetivamente retirado, invalidar todas as entradas de seus membros
antes do reuso dos endereços. `target` promove atomicamente o alvo ainda vivo
a owned temporário, ou retorna null. O fato de uma variável Dart `weak`
existir não é raiz do alvo; o resultado promovido passa a sê-lo. Se a VM
permite limpeza em um ponto de coleta, evitar expor null enquanto existe
um caminho forte, inclusive no runtime e no estado async.

### 9.2 `Expando` / ephemeron

`(chave, valor)` significa: o valor é alcançável por esta entrada **somente
quando a chave é alcançável independentemente da própria entrada**. Uma
simples contagem do valor como aresta forte vazaria `chave → valor → chave`;
uma contagem fraca do valor poderia liberá-lo enquanto a chave vive.

Proposta correta antes da otimização: manter as entradas em estrutura
especial e, nas rodadas que possam afetá-las, computar o ponto fixo de
alcançabilidade a partir das raízes fortes. Ativar cada valor cuja chave já
foi marcada; repetir até não aparecer valor novo. Só depois decidir chaves
mortas, valores mortos e finalizadores. O estado do runtime tracing existente
serve de oráculo e de implementação de reserva. Contador condicional por
chave/valor só pode substituir essa reserva após teste diferencial de ciclos
`valor→chave`, duas chaves e cadeia de ephemerons.

### 9.3 `Finalizer`, `NativeFinalizer`, `Finalizable`

O objeto alvo não ganha raiz por ter finalizador. Token, callback e dono
seguem exatamente as classificações de aresta da implementação tracing.
Quando o alvo realmente fica inalcançável, enfileirar `Finalizer` para o laço
de **eventos**, sem invocá-lo em `release`, no coletor ou como microtask. O
callback pode não executar se o processo terminar antes; sua execução não
tem ordem determinada entre objetos independentes. Native finalizer segue
sua regra própria de execução e de encerramento do grupo de isolates; não
substituí-lo por callback Dart.

Uma variável local de tipo `Finalizable` deve continuar fortemente viva até
o fim do bloco exigido pela especificação, mesmo após seu último uso
sintático. O lowering cria extensão de vida verificável, inclusive para
`this`, closure, `await`, retorno excepcional e inlining. O otimizador não
pode eliminar essa obrigação por análise de último uso. Se o runtime de FFI
registrou finalizador nativo, o anexo é removido uma vez antes de liberar o
alvo; nunca duplicar callback em coleta de ciclo e no encerramento.

`dispose()` e `close()` são métodos comuns; não aparecem no descritor ARC.
Finalizadores não podem ressuscitar um objeto cujo armazenamento já entrou
em `collecting` por via de ponteiro cru. Se a API permite registrar uma nova
referência forte ao alvo durante callback nativo, essa fronteira exige regra
de ressurreição explícita e teste; até lá, bloquear tal callback no modo ARC.

## 10. Fronteiras de execução e simultaneidade

**Isolates:** contador não atômico é permitido só quando todas as arestas do
objeto pertencem a um isolate. Uma mensagem por cópia cria grafo novo; uma
transferência válida move ownership do grafo e o registra no isolate destino
antes de desregistrá-lo da origem. Objetos imortais compartilhados precisam
ser realmente imutáveis e ter política própria. `TransferableTypedData`,
buffers externos e memória nativa obedecem às regras específicas da API;
nenhuma ponteiro Dart comum é simplesmente partilhado.

**FFI:** para cada chamada, tabela declara se argumentos são emprestados só
até o retorno, consumidos, guardados em handle persistente ou referem memória
externa. Callbacks que reentram em Dart constroem owners antes de acessar o
heap; `Dart_Handle`/handles finalizáveis são contabilizados como raízes.
Uma função C que mantém endereço de objeto Dart sem registro impede ARC;
`Pointer` Dart que aponta para memória C não faz a memória C virar objeto RC.
Arquivos, sockets, portas e timers nativos precisam de descritor de ownership
e caminho de encerramento, sem usar `release` como substituto de `close`.

**JIT, AOT, plugins e hot reload:** cache de código ou closure carregada
dinamicamente carrega também descritores e versão da ABI de memória. Módulo
não pode ser descarregado enquanto objeto vivo aponta a seu descritor ou
rotina de liberação. Misturar módulos `tracing` e `arc` na mesma heap sem
ponte de ABI formal deve falhar na ligação. O JIT pode permanecer só em
tracing enquanto o AOT é pesquisado, mas não fingir que um teste JIT cobre ARC.

## 11. Plano de implementação por marcos reversíveis

Os marcos seguintes **não revogam** as pré-condições do §7 de
[`EXPERIMENTO-ARC.md`](EXPERIMENTO-ARC.md). Se ainda não forem satisfeitas,
produzem apenas protótipos isolados e medições, sem ligar o modo ao produto.

| Marco | Entrega verificável | Rejeição imediata |
| --- | --- | --- |
| P0 | Inventário de cada classe/aresta, raiz, extern, callback e módulo; snapshot do tracing e corpus diferencial | aresta desconhecida ou baseline sem os recursos fracos |
| P1 | HIR owned/guaranteed/trivial, verificador em CFG e testes de erros deliberados; ainda em tracing | caminho com owner perdido ou uso após consumo não detectado |
| P2 | Runtime RC para objetos acíclicos com descritores versionados, stores transacionais, ABI de chamadas e contadores de debug | identidade, ordem de avaliação ou exceções divergem |
| P3 | Coleta de ciclos em pausa, candidate buffer, trial lateral, liberação em lote; corpus de ciclos e stress | qualquer branco com caminho forte ou ciclo fechado que nunca volta sob pressão |
| P4 | Weak, ephemeron por ponto fixo, Finalizer/NativeFinalizer/Finalizable, FFI e portas | promoção weak insegura, callback em momento proibido ou finalizável liberado cedo |
| P5 | Closures/async/isolate/hot reload/JIT conforme suporte; otimizações de RC com verificador após cada passe | melhora de benchmark acompanhada de mudança semântica |
| P6 | Comparação completa AOT ARC × tracing × VM, plataformas e perfis; decisão documentada | critério quantitativo ou cobertura insuficiente |

Áreas de código previstas: `crates/emit_native/src/hir.rs` e `lower/` para
propriedade e arestas; `otimizar/` para eliminação de operações RC;
`llvm/` para emissão e verificação; `crates/runtime/src/espaco.rs` e
`heap.rs` para objeto, descritor e RC; `finalizadores.rs`, `nativos_sistema.rs`,
`eventos.rs`, `ffi.rs`, `isolados.rs` e `portas.rs` para contratos especiais;
`crates/cli/src/nativo.rs` para a opção; `crates/diferencial` e
`corpus/nativo` para comparação. A lista é ponto de partida, não permissão
para alterar arquivos antes de cada prova.

## 12. Matriz de testes semânticos

Cada caso deve executar **o mesmo Dart** na VM 3.6.2 e nos dois modos nativos,
comparando saída, exceção e estado observável. Para observações de finalizador
cuja hora não é garantida, comparar propriedades permitidas (nunca prematuro,
no máximo uma vez, fila correta), não exigir timestamp idêntico.

| Grupo | Casos mínimos e sabotagem útil |
| --- | --- |
| RC básico | alias, autoatribuição, cadeia de 100 mil nós, substituição de campo por filho, `List`/`Map` mutáveis, identidade após cópia; omitir retain e detectar uso após free |
| CFG | φ em loop, retorno antecipado, `break`, `continue`, `try/finally`, throw/rethrow, chamada indireta e falha no meio de avaliação; remover cleanup excepcional e detectar owner vazado |
| Ciclos | autorreferência, par, ciclo com entrada externa, duas arestas para o mesmo nó, SCCs conectadas, fechamento com callback, Future/async; injetar entrada externa e garantir sobrevivência |
| Fracos/efêmeros | weak antes/depois da morte, promoção seguida de coleta, `Expando` cujo valor referencia a chave, cadeia de dois ephemerons, chave viva por closure; contar valor como forte e detectar vazamento |
| Finalização | evento versus microtask, token forte, detach, native no fim do grupo, `Finalizable` ao fim do bloco inclusive após `await`; retirar extensão de vida e detectar callback prematuro |
| Fronteiras | Dart→Rust→Dart, FFI guarda handle, callback reentrante, isolate transfer, objeto permanente, literal, const, hot reload de layout; descarregar descritor cedo e detectar falha |
| Stress | coleta de ciclos a cada alocação segura; orçamento mínimo, OOM injetado em cada buffer, stack overflow, thread de callback, milhões de retains, overflow simulado e teardown do isolate |

Testes internos adicionais: verificar o RC exato contra contagem reconstruída
de raízes/arestas em todo safepoint de debug; verificar que conjunto branco é
disjunto da marcação tracing de segurança; comparar cardinalidade de owners
na HIR antes/depois de cada otimização; exigir que descritores enumerem cada
aresta forte uma vez. O teste de sabotagem tem de falhar pela checagem correta,
não por acesso inválido aleatório.

## 13. Medição e critérios de decisão

Medir AOT tracing e ARC com mesmo programa, SDK, arquitetura, otimização,
entrada, ferramentas e política de exceção/raiz. Relatar separadamente:

* tempo total, instruções, cache misses e custo de retain/release por chamada;
* p50/p95/p99/máximo de pausa do coletor de ciclos e de cascata RC, incluindo
  tempo de finalização nativa;
* pico e área sob a curva de RSS, bytes de objetos, cabeçalhos, descritores,
  buffer de candidatos e tabelas fracas;
* crescimento de `.text`, dados, tabelas de exceção e código do runtime;
* objetos, arestas e candidatos visitados, abortos de rodada, SCCs liberadas,
  RCs eliminados pelo passe, fallback de ephemeron e latência de callbacks;
* compilação, LTO e tempo de cold start.

Usar pelo menos quatro perfis: muitos objetos acíclicos pequenos; grafo grande
com ciclos raros; callbacks/Future/async; `Map`/`Expando`/weak/FFI. Incluir um
programa que libera árvore grande de uma vez e um que lança exceções. Repetir
com e sem stress; apresentar dispersão e regressões individuais, não só média.
Critério mínimo de correção é absoluto: nenhuma divergência de semântica
reprodutível. Critério de desempenho será definido **antes** de medir; se ARC
for pior, mantê-lo experimental ou abandoná-lo, sem enfraquecer Dart.

## 14. Riscos que exigem decisão explícita

1. **Tempo de vida visível por weak/finalizer.** A retirada imediata em RC
   pode ocorrer antes da próxima coleta tracing. A especificação Dart permite
   coleta não determinística, mas não morte de alvo ainda alcançável; testar
   extensões de vida e promoção weak.
2. **Ephemeron:** o valor pode apontar para a própria chave; RC simples
   erra nas duas direções. Manter ponto fixo de tracing até prova melhor.
3. **Pinning e FFI:** um ponteiro cru não incrementa RC. Adaptador de handle
   é obrigatório, incluindo thread e retorno excepcional.
4. **Concorrência:** não atomizar contadores de todos os objetos por precaução;
   primeiro provar isolamento. Um subgrafo realmente partilhado exige modo
   próprio ou cópia; transferência deve ser atômica na fronteira.
5. **Latência:** destruição em cascata e coleta de ciclos são pausas possíveis;
   orçamento e fila mudam quando a memória é devolvida, não a semântica.
6. **AOT/JIT e exception ABI:** limpar owners em todo unwind; testes em
   Windows SEH e Unix Itanium separados. Jamais deixar unwind cruzar Rust
   com ABI sem suporte.
7. **Metadados de layout:** um único campo não classificado pode causar
   vazamento ou liberação precoce. CI deve gerar inventário a partir das
   escritas e confrontar com descritores e tabela de efeitos.

## 15. Artefatos de prova antes de mudar o padrão

Publicar: inventário de arestas/externs; formato de descritor e ABI versionada;
regras de HIR e saídas do verificador; traces de RC e trial deletion de casos
pequenos; relatório diferencial por programa; medições brutas e scripts de
reprodução; análise de falso positivo/negativo para weak e ephemeron; tabela
de plataformas; relatório de licenças das referências. O padrão só muda por
decisão posterior explícita, baseada nesses artefatos e nas pré-condições do
documento-base.

## 16. Interpretação das referências externas

Nim é o protótipo mais próximo para ciclos, mas sua noção de destrutor
determinístico não é uma regra Dart. Swift ajuda a organizar e verificar
ownership na IR, sem doar uma ABI. TinyGo e gollvm demonstram que enumerar
referências e atravessar chamadas nativas exige integração explícita com o
compilador, mesmo quando o gerenciador final é outro. LLVM ObjCARC demonstra
uma classe de otimizações, mas não reconhece `Ref` Dart automaticamente.
`libffi` e `libbacktrace` não resolvem ownership; servem como comparação de
fronteiras C e de rastros. A síntese acima é proposta para DartForge, não
uma afirmação de que qualquer projeto citado preserve a semântica Dart.

## 17. Proveniência dos clones estudados

Clones shallow e filtrados em `E:\references`; nenhum foi adicionado ao Git
do DartForge. Licenças permanecem nos próprios clones. Revisões consultadas:

| Fonte | Revisão | Arquivos de interesse |
| --- | --- | --- |
| Nim | `450dcf50969f9aeb42af2000def4136b5679972c` | `copying.txt`, `doc/mm.md`, `lib/system/{orc,arc,cyclebreaker}.nim` |
| TinyGo | `f6d269f74ab45bfa53a37c5708c6661ec33531ae` | `LICENSE`, `compiler/gc.go`, `src/runtime/{gc_precise,gc_blocks}.go` |
| gollvm | `605d1b6368b72e7bc15f66fac1f33f754537a090` | `LICENSE`, `bridge/go-llvm.cpp`, `libgo/CMakeLists.txt` |
| gollvm README pedido | `816aa0893286659b94cbd5e1b1cb960858825959` | `README.md` via `git show`, sem substituir o checkout |
| gofrontend | `d7cb797c46170ea43381064745514fd597cb8d7d` | fonte do frontend e `libgo` usada pelo gollvm |
| libffi | `bc553867367246d140cd156f060bd0409f57f157` | `LICENSE`, interfaces FFI |
| libbacktrace | `0b9b49cf4a2c9229fc052d6716e1528b2f23e91a` | `LICENSE`, APIs de rastros |
| llvm-project | `09910aa044808dfbfe1e0f96b5e39260789212fb` | checkout sparse: `llvm/lib/Transforms/ObjCARC`, `llvm/include/llvm/IR`, `clang/lib/CodeGen` |
| Swift já presente em `E:\references` | `6e75592c4025239130e370877ab0fab4006b675b` | `docs/OwnershipManifesto.md` |
| Koka | `9c55695dd2f7d4db8d93011693d37295e2b76c53` | `LICENSE`, `doc/spec/why.kk.md`, `src/Compile/Optimize.hs` |

O roteiro `llvm.org/git/llvm.git` do README histórico de gollvm foi lido como
documento da revisão indicada, não executado: a fonte LLVM clonada aqui é o
repositório atual `llvm/llvm-project`, em pasta separada. `gofrontend`,
`libffi` e `libbacktrace` também ficaram em pastas próprias, preservando seus
históricos e licenças.
