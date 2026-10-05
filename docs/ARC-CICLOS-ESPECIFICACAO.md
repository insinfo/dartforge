# Especificação de implementação — ARC com coleta de ciclos no backend nativo

**Estado:** proposta de engenharia e plano de prova; não é implementação nem mudança
do gerenciador padrão. **Documento-base imutável:**
[`EXPERIMENTO-ARC.md`](EXPERIMENTO-ARC.md). Este texto detalha aquele experimento,
sem alterar sua sintaxe, suas condições de entrada, suas garantias de Dart ou sua
decisão de manter tracing como referência. **Alvo semântico inicial:** Dart 3.6.2.

**Guia de execução:** as seções 18–26 fixam a primeira implementação, os pontos
de alteração no código, os contratos e as entregas verificáveis. Nomes marcados
como **novos** são APIs/arquivos a criar, não recursos já implementados. As
seções anteriores fornecem os requisitos e a motivação; em alternativas de
engenharia, a versão inicial escolhida nas seções 18–26 é a receita a executar.

## 0. Escopo, linguagem de certeza e critério de aceitação

ARC significa *automatic reference counting*: operações inseridas pelo compilador,
sem `retain`, `release`, `weak` ou anotações novas no código Dart. O mecanismo inclui
um coletor de ciclos; ARC puro é insuficiente. O modo experimental deve ser opt-in,
com `--memoria arc` (também aceitar `--memoria=arc`), e coexistir com o modo tracing. Um programa só pode
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

Para um objeto normal sem proteção condicional (§22), `rc=0` permite iniciar
a desalocação; não autoriza
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

Cabeçalho lógico; a versão inicial usa metadados laterais (§19), sem ampliar
o cabeçalho físico atual. Embutir contadores no cabeçalho é otimização posterior:

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
| `arc_release(x)` | Retira uma ocorrência; em zero, agenda rompimento e liberação iterativos, sujeitos à proteção condicional (§22). Se resta positivo e o objeto pode participar de ciclo, registra candidato sem duplicá-lo. |
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

Convenção geral de chamadas (otimizações internas na seção 25):

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
   chamada externa exige resumo de ownership, mesmo quando seus efeitos são
   classificados como desconhecidos. Sem resumo de ownership, rejeitar ARC.

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
   capturar estabilidade do grafo por pausa no mutador. A versão especificada
   usa pausa; concorrência exigiria outro protocolo e não é presumida. O tamanho
   da pausa deve ser medido, não é necessariamente curto.
2. **Marcar cinza / subtração experimental.** Para cada objeto alcançado por
   arestas fortes da região, registrar `trial(x)=rc(x)` e subtrair **cada
   ocorrência interna**, uma única vez por aresta. Não alterar o contador
   verdadeiro; usar contador lateral evita deixar RC corrompido após OOM.
3. **Localizar entradas externas.** Todo nó com `trial>0` é raiz preta da
   região. Restaurar alcançabilidade preta transitivamente. Nós restantes
   são candidatos brancos; objetos protegidos entram como sementes de
   sobrevivência (§22). Mutação concorrente é erro de protocolo; não publicar
   descarte com versão do grafo divergente.
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

Uma entrada weak não incrementa RC. Quando a morte é confirmada por zero
sem proteção condicional ou por coleta, invalidar todas as entradas dos mortos
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

Manter as entradas em estrutura especial, proteger a clausura forte dos
valores entre rodadas e computar o ponto fixo de alcançabilidade a partir das
raízes fortes (§22). Ativar o valor quando portador e chave estiverem vivos;
repetir até não aparecer valor novo. Só depois decidir chaves mortas, valores
mortos e finalizadores. O tracing existente serve como comparação, após
validação semântica. Otimizar índices/worklists preservando o ponto fixo e os
testes de ciclos `valor→chave`, duas chaves e cadeia de ephemerons.

### 9.3 `Finalizer`, `NativeFinalizer`, `Finalizable`

O objeto alvo não ganha raiz por ter finalizador. Token, callback e dono
seguem o contrato Dart e o modelo de arestas detalhado na seção 22.4; o
inventário do tracing é evidência a conferir, não definição da linguagem.
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
em `collecting` por via de ponteiro cru. O callback de `NativeFinalizer` recebe
token nativo, não acesso ao objeto Dart morto; não criar uma extensão de
ressurreição para acomodar ponteiros fora do contrato da API.

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

## 18. Arquitetura executável e decisões fixadas

Esta seção transforma o experimento em tarefas de implementação. O produto final
é ARC com recuperação de ciclos, suporte às fronteiras Dart e otimizações
verificadas. Os marcos intermediários servem para encontrar erros por camada;
um marco parcial não satisfaz a especificação. Nenhuma etapa autoriza aceitar
programas com semântica incompleta ou anunciar ganhos ainda não medidos.

### 18.1 Componentes e fluxo

```text
CLI → configuração imutável de memória → HIR com proveniência de referências
    → otimização normal → normalização das saídas excepcionais
    → inserção de ownership → verificação → otimização ARC → verificação
    → materialização de exceções e raízes → verificação final → LLVM
    → runtime da mesma ABI → contadores + descritores + coleta de ciclos
                          → weak/ephemerons/finalização → alocador
```

Decisões obrigatórias:

1. `tracing` permanece padrão; `arc` é seleção por programa e isolate. Não há
   alteração na gramática, tipos públicos, assinatura ou código-fonte Dart.
2. O contador ordinário é por ocorrência forte, não por variável nem por objeto
   distinto. Duas posições da mesma lista apontando para `x` contam duas vezes.
3. Contadores são não atômicos no heap confinado ao isolate. Uma thread externa
   não acessa esse heap: publica uma mensagem/registro para execução no isolate.
4. A primeira ABI ARC conserva `Cabecalho`, o deslocamento de handle e as tags.
   Os metadados ARC são laterais. Sua localização poderá ser otimizada sem mudar
   a semântica, por uma nova versão explícita de ABI.
5. A coleta de ciclos opera com mutador parado e sem reentrada. Não é chamada
   simultaneamente com mutação. Coleta concorrente fica fora desta versão.
6. Toda escrita forte, inclusive inicialização e `memcpy` de referências,
   passa por contrato de ownership. Emissão em linha só volta quando implementar
   o mesmo contrato e passar pelos mesmos testes da chamada ao runtime.
7. Referências fracas e ephemerons têm armazenamento próprio; não são campos
   fortes disfarçados. A determinação de vida condicional usa ponto fixo (§22).
8. A liberação física só acontece depois da invalidação dos observadores,
   remoção das arestas e publicação das ações de finalização permitidas.

### 18.2 Mapa das alterações

Caminhos abaixo são relativos à raiz do repositório. Símbolos existentes são
pontos de entrada para a alteração; assinaturas propostas aparecem nas seções
seguintes. Conferir o símbolo no checkout ao implementar, em vez de depender de
números de linha que mudam.

| Arquivo existente | Alteração exigida |
| --- | --- |
| `crates/cli/src/nativo.rs` | Nos parsers de `aot` e `run_compile_native`, reconhecer as duas formas de `--memoria`; validar valor e combinações antes de compilar o SDK. Usar `definir_modelo_de_excecoes` e `definir_modo_de_raizes` como referência de integração, não como razão para reler ambiente em cada passe. |
| `crates/emit_native/src/alvo.rs`, `context.rs`, `hir.rs` | Criar `ModoMemoria { Tracing, Arc }`, ler a seleção uma vez e transportá-la no contexto/módulo. Registrar versão da ABI e configuração de raízes/exceções. |
| `crates/emit_native/src/lib.rs`, `sdk_modulo.rs` | Aplicar o mesmo pipeline ARC ao programa e ao SDK; hoje ambos chamam `otimizar` e depois `excecoes_por_tabelas`. Centralizar essa sequência para impedir divergência. |
| `crates/emit_native/src/hir.rs` | Acrescentar ownership, origem de ponteiros, operações ARC, extensão de vida e saídas excepcionais explícitas (§20). |
| `crates/emit_native/src/lower/{mod,fn_builder,comandos,closures,async_sm,literais,membros}.rs` | Preservar classificação de referências, locais, capturas, globais, `Finalizable`, estado suspenso e destinos de erro antes da análise de ownership. |
| `crates/emit_native/src/otimizar/{mod,operandos,cfg,tabelas}.rs` | Atualizar visitantes/CFG e separar preparação de exceções de sua emissão; inserir e verificar ARC, sem tratar operações de contador como instruções puras. |
| `crates/emit_native/src/llvm/mod.rs`, `llvm/listas_ir.rs`, `llvm/raizes.rs`, `llvm/externs.rs` | Emitir a ABI ARC; cobrir stores, inicializações, resultados, raízes e chamadas especiais (§21). |
| `crates/runtime/src/heap.rs` | Integrar estado ARC em `Heap`; alterar `antes_de_alocar`, alocação, `definir_campo`, raízes, tabelas laterais e `collect`. Separar política de vida da varredura física. |
| `crates/runtime/src/espaco.rs`, `layout.rs` | Implementar reclamador ARC sem reutilizar marcas de tracing como evidência de vida; preservar layout e invariantes do alocador (§19.4). |
| `crates/runtime/src/{nucleo,gc_raizes}.rs` | Adaptar `dartforge_object_new/get/set`, `dartforge_alocar`, registro de imagem e raízes; não abastecer TLAB sem registro ARC. |
| `crates/runtime/src/{listas,nativos_listas,colecoes,closures,tipos}.rs` | Enumerar e contabilizar todos os campos/buffers gerenciados, inclusive substituição, cópia, crescimento e redução. |
| `crates/runtime/src/{nativos_sistema,finalizadores,eventos,excecoes}.rs` | Integrar promoção weak, ephemerons, anexos, eventos e pendências com a máquina de estados de morte. |
| `crates/runtime/src/{ffi,ffi_callbacks,ffi_api_nativa,isolados,portas}.rs` | Implementar contratos de handles, callbacks, publicação, cópia entre isolates e encerramento. |
| `crates/runtime/{build.rs,efeitos.tsv}`, `crates/runtime/src/lib.rs` | Registrar novos módulos/fragmentos e símbolos; gerar e validar tabela de ownership junto à tabela de efeitos. Incluir os módulos também em `RUNTIME_MAIN`. |
| `crates/emit_native/src/{cache,cache_objeto,sdk_modulo,driver}.rs`, `crates/emit_native/build.rs` | Selecionar runtime correto e impedir mistura de ABI em caches, bibliotecas, SDK, objetos e ligação (§24). |
| `crates/jit/src/{lib,vivo,reload,migracao,delta}.rs` | Propagar modo, registrar descritores, reter código em uso e migrar layouts transacionalmente (§23). |

Arquivos **novos** com responsabilidade delimitada:

| Arquivo proposto | Responsabilidade |
| --- | --- |
| `crates/runtime/src/arc.rs` | Contadores, estados, filas, identidade com geração, primitivas e estatísticas; nenhuma chamada Dart. |
| `crates/runtime/src/arc_descritores.rs` | Enumeração tipada de arestas/raízes e descarte de armazenamento externo; adaptadores dos layouts atuais. |
| `crates/runtime/src/arc_ciclos.rs` | Trial deletion e transação de descarte de conjunto; usa `arc.rs`, não duplica regras de release. |
| `crates/runtime/src/arc_condicionais.rs` | Índices de weak/ephemerons, conjunto protegido e ponto fixo global de alcançabilidade. |
| `crates/runtime/src/arc_abi.rs` | Fragmento de externs `dartforge_arc_*`; chama `Heap` e valida a ABI, sem duplicar algoritmos. |
| `crates/runtime/ownership.tsv` | Contrato por parâmetro/resultado/saída excepcional de cada extern. |
| `crates/emit_native/src/otimizar/arc/{mod,inserir,verificar,otimizar}.rs` | Pipeline, inserção, verificador de fluxo e otimizações de ownership. |
| `crates/emit_native/src/llvm/verificar_arc.rs` | Verificação complementar de emissão: stores e alocações reconhecidos, raízes e ABI consistentes. A prova principal continua na HIR. |

## 19. Runtime, representação e recuperação de memória

### 19.1 Estado por objeto e por isolate

O cabeçalho físico atual em `layout.rs` tem 16 bytes e campos `estado`, `flags`,
`n`, `class_id`, `mapa`, `metadado`; `DESLOCAMENTO_DO_HANDLE` vale 2.
Não encaixar um contador em um campo ocupado. Preservar as asserções de layout.

Tipos abaixo são **novos**, em pseudocódigo Rust; a implementação deve usar os
tipos de handle/coleções locais e documentar os contratos de `unsafe`:

```rust
struct IdArc { handle: i64, geracao: u64 }
enum EstadoArc { Construindo, Vivo, Coletando, Morto }
struct MetaArc {
    geracao: u64,
    rc: u64,                 // owners + ocorrências fortes; não inclui weak
    descritor: IdDescritor,   // identifica também a versão do layout
    estado: EstadoArc,
    candidato: bool,
    protegido_condicional: bool,
    imortal: bool,
}
struct EstadoDoArc {
    objetos: TabelaPorHandle<MetaArc>,
    candidatos: Fila<IdArc>,
    zeros: Fila<IdArc>,
    mortos: Fila<IdArc>,
    epoca_grafo: u64,
    transacao: bool,
    condicionais_sujos: bool,
    // índices weak/ephemeron, descritores, contadores e reserva de trabalho
}
```

`TabelaPorHandle`, `Fila` e `IdDescritor` são nomes conceituais a implementar,
não tipos existentes. A primeira tabela pode usar o mapa de `crate::hash`;
medir depois uma tabela por página/índice de bloco, evitando um hash por retain.
Geração é monotônica por alocação no isolate; overflow é falha interna detectada,
nunca reutilização silenciosa. Filas guardam `IdArc`, verificam geração antes de
acessar e ignoram entradas obsoletas. Elas não são raízes nem incrementam RC.

`permanentes`, `constantes` e caches existentes de `Heap` não implicam
imortalidade: o tracing atual expurga entradas mortas dessas tabelas. Só objetos
de imagem estática comprovadamente vivos durante toda a imagem recebem
`imortal=true`. Enumerar suas arestas de saída; imortalidade não se propaga aos
destinos por conveniência. Literais/globais/caches fortes mantêm ocorrências
contadas, conforme sua política real. Caches fracos mantêm identidade e geração.

### 19.2 Descritores e inventário de arestas

`IdDescritor` resolve em `{ classe, versao_layout, forma, modulo }` e em três
operações: `visitar_fortes`, `visitar_condicionais`, `descartar_armazenamento`.
O visitante forte recebe **cada ocorrência**, com localização estável enquanto
o mutador estiver parado. Não usar um conjunto de destinos: isso perderia
multiplicidade. Um campo `(bits, e_ref=false)` nunca é contado pelo padrão dos bits.

| Forma | Arestas e procedimento de descarte |
| --- | --- |
| Instância, célula, ambiente, record | Usar o bitmap e comprimento válidos; slots não inicializados são zero. Limpar slot e tag ao romper a aresta. |
| Closure | Ambiente e referências de tipo/capturas são fortes; endereço de código não é objeto Dart. Reter separadamente o módulo de código. |
| Lista expansível | Objeto aponta para seu armazenamento; o armazenamento conta os elementos uma vez. Não contar também lista→elemento se já existe buffer→elemento. |
| Lista compacta/heterogênea | Interpretar `Elemento`/tags reais; percorrer comprimento lógico, não capacidade não inicializada. Ao reduzir comprimento, limpar e soltar a cauda. |
| Map/Set | Percorrer armazenamento efetivo, inclusive chaves/valores e tabelas auxiliares gerenciadas; tombstones e hashes são triviais. Rehash transfere owners. |
| String, caixa numérica, dados tipados | Payload numérico/bytes é trivial; views contam o backing store gerenciado. Buffer C tem recurso externo e política de posse própria. |
| Tipos, exceção, rastro, Future/estado async | Enumerar os campos gerenciados efetivos, sem pressupor que metadados sejam triviais. |
| Weak/ephemeron/finalizador | Adaptador especializado (§22); não percorrer alvo/chave/token fraco como campo forte por engano. |

O inventário de P0 deve conter, para cada forma: construtor, todos os escritores,
leitores, visitante, política do buffer, raízes externas e teste. Uma nova forma
sem entrada não pode ser emitida em ARC. Descritores dinâmicos usam `mapa` e
versão do layout; não dependem somente de `class_id`. Atualizar `CONTRACT.md`
ao implementar: suas descrições históricas não substituem o layout atual.

### 19.3 Alocação, contagem e substituição

`Heap::alocar_bloco` registra metadados antes de publicar o handle. Inicializar
com `rc=1`, estado `Construindo`, corpo/bitmap zerados e descritor válido.
Essa ocorrência pertence ao construtor; a conclusão a transfere para o resultado.
Cada campo inicializado por cópia retém o destino. Construtor com falha solta
apenas campos já inicializados e consome a ocorrência de construção. Enquanto
construindo, o objeto consta das raízes temporárias do runtime.

`reter(h)` ignora null/Smi e imortais, verifica estado/geração e faz incremento
com overflow verificado, inclusive em release build. `soltar(h)` exige `rc>0`;
decrementa; zero vai para fila de zeros, positivo potencialmente cíclico vai
para candidatos. Operações de contador não alocam objetos Dart, não lançam
exceção Dart e não executam callbacks. Reserva de fila é obtida antes da
transação; fila intrusiva/prealocada é alternativa a `Vec::push` falível.

Uma substituição em `Heap::definir_campo` segue esta sequência:

```text
validar receptor, índice, tag e valor; reservar metadados de proteção
ler (antigo, tag_antiga)
reter(novo) se nova tag for referência
se receptor for protegido: proteger transitivamente novo antes de publicar (§22)
publicar payload e bitmap como uma transação sem safepoint
soltar(antigo) se tag antiga for referência
encerrar transação; só agora permitir drenagem/coleta
```

`x.f=x.f` deve permanecer válido. Validação que possa lançar ocorre antes da
alteração. Para store por movimento, substituir o retain pelo consumo do owner
de entrada **somente no sucesso**; na falha ele continua pertencendo ao chamador.
Getter retorna owned na ABI inicial: copiar a referência retendo-a antes de
permitir reentrada. Leitura borrowed é otimização HIR com prova de estabilidade
do slot, não simples consequência de o receptor permanecer vivo.

### 19.4 Morte lógica, lote e alocador

`EspacoDeObjetos::varrer` reconstrói intervalos livres a partir de marcas;
`varrer_jovens` depende de gerações do tracing. Nenhum deles pode rodar sobre
ARC sem fornecer uma fonte explícita de liveness. As funções de liberação do
sistema em `espaco.rs` liberam páginas; não são `free(handle)`.

Implementar **novo** `reclamar_mortos_arc` no espaço, compartilhando a mecânica
de reconstrução de regiões com a varredura completa, mas usando metadados ARC
`Morto` como autorização. O primeiro reclamador pode percorrer páginas; deverá:

1. Invalidar TLABs/cursors antes de reconstruir intervalos; nenhuma thread usa
   cursor antigo. Drenar/remover entradas de `jovens` e `lembrados` incompatíveis.
2. Para cada bloco morto, liberar uma vez payload externo e registrar o intervalo
   livre; não confundir header e corpo externo. Remover metadados só depois disso.
3. Preservar blocos `Vivo`, `Construindo`, `Coletando` e mortos ainda em quarentena.
   Reconstituir mapas de ocupação e marcas auxiliares sem convertê-los em RC.
4. Coalescer intervalos livres sem cruzar fronteiras de página/classe; devolver
   regiões grandes vazias conforme a política existente; manter quarentena de
   depuração quando habilitada.
5. Atualizar `em_uso`, vivos, bytes externos e estatísticas por delta verificável;
   nunca descontar o mesmo objeto na morte lógica e novamente no retorno físico.

É permitido adiar a devolução física em uma fila com limite de bytes; não é
permitido voltar a observar o objeto morto. Pressão de memória/OOM drena essa
fila e tenta coleta completa antes de falhar. O caminho otimizado posterior
mantém listas livres por classe para inserir blocos individualmente; exige
os mesmos invariantes e testes de fragmentação do reclamador por páginas.

Para descartar conjunto `D`, de cascata ou de ciclos:

```text
preparar reserva e ações; congelar mutador; marcar todos de D como Coletando
invalidar weak de D; classificar anexos e registrar ações elegíveis
para cada ocorrência forte u→v com u em D:
    zerar ocorrência
    se v não pertence a D: soltar(v)
    senão: não executar release comum sobre v
remover metadados condicionais/caches de D e suas proteções
marcar D como Morto; zerar RC descartado; publicar fila de memória morta
encerrar transação; drenar novos zeros seguros; despachar ações fora do heap
```

Uma fila iterativa evita estouro de pilha em cadeias grandes. Interromper a
drenagem entre objetos/lotes concluídos é permitido; não interromper a transação
que torna um grupo morto. Estado pendente deve preservar toda memória ainda
necessária e não permitir promoção weak de objeto já `Coletando`.

## 20. HIR de ownership, algoritmo de inserção e verificador

### 20.1 Tipos e instruções internas

`Type::Ref` é a representação gerenciada; `I64` e `Ptr` não viram referências
por ter 64 bits. Preservar proveniência em conversões/boxing, chamadas e cargas.
Adicionar tabela por `ValueId` com `Trivial`, `Owned` ou `Borrowed(owner, escopo)`.
Parâmetros Ref são borrowed até o retorno; resultados Ref são owned. A convenção
vale para dispatch direto, seletor, closure e stubs de SDK.

Operações **novas** sugeridas na `Instruction`:

| Operação | Efeito de ownership |
| --- | --- |
| `ArcCopy { value } -> Ref` | Produz owner independente por retain; não consome entrada. |
| `ArcDrop { value } -> Void` | Consome exatamente um owner; pode agendar morte, nunca chamar Dart. |
| `ArcMove { value } -> Ref` | Consome owner de entrada e produz owner de saída sem RC físico. |
| `ArcLoadStrong { slot } -> Ref` | Lê slot classificado e retorna owned; slot inclui receptor/índice/tag, não ponteiro arbitrário. |
| `ArcStoreStrong { slot, value, modo }` | Copy retém; Move consome no sucesso. Troca libera antigo depois da publicação. |
| `ArcBeginBorrow`, `ArcEndBorrow` | Delimitam empréstimo verificável; podem desaparecer após verificação. |
| `ArcKeepAlive { value, escopo }` | Exige vida até uma saída de escopo, inclusive excepcional; não é comentário removível. |

`Alloca` de Ref representa slot proprietário inicializado com null. `Load` faz
cópia owned; `Store` substitui ou move. `mem2reg` converte essa propriedade em
SSA antes de inserir contadores. `StoreGlobal` usa slot global contado. Store
genérico sem classificação stack/global/heap/native é erro do compilador ARC.
Adicionar variantes aos visitantes em `otimizar/operandos.rs`, poda, impressão,
substituição, clonagem, inlining e visitantes de `lower/async_sm.rs`.

### 20.2 Inserção por CFG

Implementar `arc::inserir(module) -> Result<(), DiagnosticoArc>` em fases:

1. Classificar instruções e chamadas pela tabela de ownership. Construir CFG
   contendo saídas normais, pendência, unwind, retorno, throw e suspensão.
2. Calcular vivacidade reversa de Ref e slots em ponto fixo. Incluir usos de
   `ArcKeepAlive` e dependências dos borrows. Não decidir vida apenas por texto.
3. Para cada resultado owned, associar um token de ownership. Cópia cria token;
   movimento/retorno/store move consome token. Alias borrowed não cria token.
4. Inserir cópias para usos que precisam reter após a duração de empréstimo e
   para transferências quando o owner ainda terá outro uso no caminho.
5. Inserir drops nas fronteiras em que o token deixa de ser vivo, inclusive nas
   arestas de exceção e nos caminhos que não usam um resultado intermediário.
6. Dividir arestas críticas antes de colocar drops/cópias por predecessor.
   Em `Phi`, cada predecessor transfere uma ocorrência para o resultado; cópia
   somente se a entrada continua necessária. Um predecessor inalcançável não
   cria owner. Backedge transfere o token da iteração, sem acumular retain.
7. Após uma chamada que pode falhar, o resultado só existe no sucesso. Na aresta
   de erro, limpar argumentos temporários/locais vivos conforme a convenção,
   nunca dar drop em resultado inexistente/default escalar.
8. Recalcular vivacidade depois de inserir blocos e passar pelo verificador.

Exemplo de redução legítima, conservando a ordem das chamadas Dart:

```text
// r = produzir(); objeto.campo = r; usar(r); return objeto;
r = CallOwned produzir
ArcStoreStrong Copy objeto.campo, r   // campo ganha ocorrência independente
CallBorrowed usar(r)                 // r permanece owned até retornar/lançar
ArcDrop r                           // também na saída excepcional de usar
ReturnMove objeto                   // exige owner; copiar se era borrowed

// Sem usar(r), store Move pode consumir r e eliminar retain + drop.
```

### 20.3 Verificação e diagnósticos

O `otimizar::valida` existente não é verificador de ownership. Criar análise
dedicada com estado por token `{ausente, disponível, consumido}` e obrigações
de slot/borrow. Junções devem concordar após normalização das arestas; não usar
uma união que transforme “consumido em um caminho” em “disponível em todos”.
Loops convergem por estados finitos, com tokens de bloco/φ, não por simular
um número arbitrário de iterações. Checar dominância de definições e vida do
owner que sustenta cada empréstimo.

Diagnósticos obrigatórios incluem função, bloco, instrução, token/origem e
caminho contraexemplo: `ARC001` uso após consumo; `ARC002` owner sem consumo na
saída; `ARC003` borrow escapado; `ARC004` escrita sem contrato; `ARC005` extern
sem resumo; `ARC006` cleanup excepcional ausente; `ARC007` ABI incompatível;
`ARC008` layout não descrito. São erros internos/de capacidade de compilação,
não exigências de anotações ao programador Dart.

Verificar após inserção, após cada otimização ARC em testes/debug, após a série
de otimizações em produção e após materialização de exceções. Em modo auditor,
recontar `owners de raiz + arestas fortes` e comparar com `rc` em safepoints.
Raízes observacionais da pilha não acrescentam uma segunda ocorrência (§21.3).

### 20.4 Exceções: alterar o passe atual, não apenas inserir drops no retorno

`otimizar/tabelas.rs` atualmente permite que unwind atravesse funções sem
tratador, e `TabelasDaFuncao` guarda invocações por `ValueId`, pousos e saídas
por `BlockId`. Em ARC, uma função sem `catch` ainda pode ter owners a limpar.

Separar o passe em **preparação do CFG excepcional** e **materialização das
tabelas**. Preparar antes da inserção ARC; materializar depois. IDs devem ser
estáveis e metadados reconstruídos quando blocos/instruções forem substituídos.
Não executar um passe genérico de simplificação sobre pousos já materializados.

Para cada call que pode desenrolar com owners ativos, emitir `invoke` e bloco
de cleanup correspondente ao conjunto vivo naquele ponto. O cleanup executa
drops uma vez e continua a propagação pelo protocolo da plataforma. Distinguir
cleanup de tratador Dart: um cleanup não captura/limpa a pendência e não cria
uma exceção nova. Reutilizar a infraestrutura de personality/unwind do projeto,
estendendo-a para resumir a exceção original; em SEH, respeitar o modelo de
funclets usado pelo emissor. Não inventar uma chamada C comum para “resume”.

`FnBuilder::emit_call_with_check`, `emit_throw_op`, `emit_rethrow` e
`lower_try_stmt` devem preservar owners da exceção/rastro ao registrar ou limpar
pendência. `finally` executa uma vez; se lança outra exceção, a pendência antiga
é substituída transacionalmente. No modo pendência, inserir as mesmas limpezas
nos blocos de erro. Resultado/retorno guardado durante `finally` tem owner
próprio até ser retornado ou substituído por erro.

Restauração do topo de pilha-sombra não equivale a liberar owners. Um quadro
Rust `extern "C"` continua usando retorno com pendência, conforme o protocolo
atual; não permitir unwind Dart atravessá-lo. Testar A→B→C com owner em B,
throw em C e catch apenas em A, nos dois modelos e em cada plataforma.

## 21. LLVM, ABI de runtime e raízes

### 21.1 Pontos de emissão que não podem contornar ARC

Em `llvm/mod.rs`, alterar separadamente:

1. `Instruction::AllocObject`: campos atualmente gravados diretamente por
   `emitir_gravacao_de_campo` devem usar inicialização ARC ou stores certificados.
2. `Instruction::SetField`: substituir o store/bitmap/barreira geracional por
   transação ARC; `dartforge_lembrar` sozinho não conta referências.
3. A especialização de `CallRuntime("dartforge_object_set")` com índice constante
   precisa do mesmo tratamento. Alterar só `SetField` deixa esse desvio ativo.
4. `alocacao_em_linha`/TLAB: emitir o registro de metadados e owner inicial antes
   de publicar objeto. Até esse caminho estar pronto, escolher o construtor
   ARC externo. Isso é uma etapa de integração, não uma licença para declarar
   concluída a otimização de alocação.
5. `GetField`, elementos de lista, ambientes, células e records: resultado
   gerenciado é owned ou borrow comprovado; nunca inferir pelo tipo LLVM `i64`.
6. `Load`, `Store`, `LoadIndexed`, globais e cópias em lote: preservar a
   classificação HIR. Guardar Ref por um ponteiro sem proveniência é rejeitado.

`llvm/verificar_arc.rs` deve conferir os pontos emitidos com um registro de
operações produzido pelo emissor, e testes de IR devem procurar stores não
classificados. Regex no IR é defesa auxiliar, não prova suficiente. LLVM não
recebe primitivas RC como `readnone`/`readonly` e não pode reordená-las através
de observações weak, safepoints ou chamadas reentrantes sem prova.

### 21.2 Contratos das externs

Adicionar `ownership.tsv`, associado por nome exato a `efeitos.tsv`. Cada entrada
descreve: tipos semânticos de parâmetros, `borrow`/`consume`/`scalar`/`native`,
resultado `owned`/`borrow(arg N)`/`immortal`/`scalar`, sucesso versus pendência,
retenção persistente interna e capacidade de invalidar empréstimos.

`build.rs` deve exigir uma entrada por símbolo exportado, rejeitar duplicatas,
parâmetro inexistente e resultado incompatível. Gerar acesso tipado em
`llvm/externs.rs`. Não tratar todas as externs como borrowed só para completar
a tabela. Uma extern que guarda argumentos retém internamente; entrada
`consume` só se sua implementação tomar posse nos caminhos documentados.

Externs **novas** mínimas: `dartforge_arc_retain`, `dartforge_arc_release`,
`dartforge_arc_collect`, `dartforge_arc_verificar_abi`. Retain/release são
`nounwind` somente se falhas internas não desenrolarem pelo limite C. Elas
não executam Dart nem coleta de ciclos; o coletor roda em safepoints próprios.
Externs de campo existentes passam a seguir o modo do runtime selecionado,
com o contrato de resultado owned em ARC. O chamador não pode inferir o modo
do callee pelo nome do símbolo: o registro de ABI deve garanti-lo.

### 21.3 Raízes: contadas versus observacionais

Separar no inventário:

| Origem | Regra ARC |
| --- | --- |
| Locais SSA e slots proprietários do código gerado | Owners já contados pelas instruções ARC. Pilha-sombra/mapas apenas descrevem sua localização e vida. |
| `Heap::frames`, `set_root`, `root`, `pop_frame` usados por Rust | Slots proprietários: entrada/cópia retém, substituição retém antes de soltar, saída solta. Criar operação explícita de movimento quando o resultado já é owned. |
| `set_global_root`, `set_raiz_do_runtime`, literais e tabelas fortes | Contar cada slot persistente; mover raiz não é reter uma segunda vez. `soltar_raiz_global` consome sua ocorrência. |
| Pendência de exceção/rastro, eventos, timers, mensagens | Slots persistentes contados com transferência explícita ao consumir a fila. |
| Weak, candidatos, índices de identidade e filas de mortos | Não são owners; validar geração e estado. |

Não mudar cegamente `set_root` para reter se o emissor usar a mesma entrada
para raízes observacionais. Separar as APIs ou adicionar categoria interna
inequívoca. A enumeração para alcance (§22) visita ambos os tipos, mas a
auditoria de RC conta só os proprietários. Slots observacionais mortos devem
ser zerados antes do safepoint; uma cópia obsoleta não pode ressuscitar objeto.

Manter uma localização observável para cada owner vivo em safepoints, incluindo
owners temporários de Rust. Durante prototipagem, a pilha-sombra fornece isso;
mapas de pilha podem substituí-la quando o caminho ARC estiver coberto pelo
verificador de mapas. Não remover raízes só porque os contadores existem:
ephemerons e auditoria precisam distinguir raízes de arestas internas.

## 22. Ciclos, ephemerons e finalização: algoritmos completos

### 22.1 Trial deletion sem alterar RC real

No safepoint, drenar uma seleção de candidatos válidos. Construir região `R`
pela clausura transitiva das arestas fortes; visitar cada objeto uma vez,
mas registrar a multiplicidade das arestas. Em memória de trabalho, iniciar
`trial[v]=rc[v]`; para cada `u→v` com ambos em `R`, subtrair uma ocorrência.
Underflow indica erro de inventário/contagem e aborta a coleta com diagnóstico.

Marcar como entradas de sobrevivência os objetos com trial positivo, imortais,
em construção ou protegidos condicionalmente. Propagar sobrevivência pelas
arestas fortes. Os restantes formam `D`; descartar por §19.4. Proteção pode
adiar recuperação, mas nunca autoriza liberar. Objetos descartados são retirados
dos índices de candidatos; entradas antigas de fila são filtradas por geração.

Se limite de memória/trabalho for atingido antes de concluir `R`, abandonar a
tentativa e recolocar candidatos. Nenhum RC real foi modificado. Não retomar
um trial antigo após o mutador rodar. Pressão de memória força coleta completa,
com reserva suficiente ou falha de alocação definida; não deixar ciclos
permanentemente esquecidos porque ultrapassam o orçamento normal.

### 22.2 Proteção condicional: impedir a morte prematura por RC zero

Ephemeron não é resolvido apenas executando ponto fixo na coleta: entre duas
coletas, um valor com `rc=0` pode continuar vivo por uma chave alcançável.
Adicionar `protegido_condicional` e índices de entradas `(portador,chave,valor)`.
Essa proteção **não aumenta `rc` e não é raiz** na análise de alcance.

Algoritmo escolhido para esta versão:

1. Antes de publicar/alterar um valor de ephemeron, marcar como protegido o
   valor e toda sua clausura forte. Isso é uma sobreaproximação de retenção
   física, inclusive quando ainda não se sabe se a chave vive.
2. Se um objeto protegido ganhar uma aresta forte, proteger transitivamente o
   novo destino antes da publicação. Integrar a barreira em todos os stores,
   inicializações, cópias de buffers e movimentos. Remoção de aresta/entrada
   apenas marca `condicionais_sujos`; pode manter proteção extra até a rodada.
3. `rc=0` de objeto protegido não inicia descarte. Colocá-lo em espera; a
   proteção de seus descendentes impede uma cascata de destruir valor vivo.
4. Um safepoint com condicionais sujos, pressão de memória ou coleta explícita
   executa o ponto fixo global abaixo. A fila de zeros por si só não decide
   vida condicional.

Esse esquema mantém uma propriedade simples: toda clausura forte de qualquer
valor condicional registrado está protegida entre rodadas. O custo é a possível
retenção de lixo até o ponto fixo. Não requer tornar todos os objetos do heap
imortais, nem conta uma referência condicional como forte ordinária.

### 22.3 Ponto fixo e transação de descarte global

Com o mutador parado e raízes exatas publicadas:

```text
L = clausura_forte(raízes reais + objetos de imagem vivos)
repetir:
    para cada entrada (p, k, v):
        se p pertence a L e k é chave viva segundo a representação da API:
            acrescentar v e sua clausura forte a L
até L não crescer
D = todos os objetos gerenciados elegíveis que não pertencem a L
preparar finalizações permitidas sem tornar o alvo uma raiz
invalidar entradas cujos portadores/chaves morreram
reconstruir proteção condicional dos valores das entradas restantes
descartar D em lote; drenar zeros não protegidos; reclamar armazenamento
```

Para chave gerenciada, “viva” significa pertencer a `L`; valores sentinela/
escalares são tratados pelo contrato da operação interna, após validar as
restrições públicas de `Expando`. Não considerar `rc>0` nem proteção como
evidência de vida de chave. O caso `valor→chave` sem raiz independente morre.
O portador também precisa estar alcançável; uma tabela lateral não mantém
sozinha seu portador vivo.

No lote, objetos com RC positivo apenas por ciclos também morrem. Registrar
as arestas de saída de `D` e debitar sobreviventes exatamente uma vez. Recriar
proteção antes de drenar zeros evita uma janela de morte de valores ainda
condicionalmente vivos. Se a preparação falhar, manter o estado anterior
intacto e tentar novamente com reserva/pressão; nunca publicar meia limpeza.

Otimização obrigatória a avaliar depois da versão de referência: índice
`chave → entradas pendentes` e worklist acionada quando uma chave/portador
entra em `L`, eliminando varreduras repetidas de todas as entradas. Cada entrada
ativa no máximo uma vez por rodada. Proteção incremental por regiões só será
aceita se a barreira provar a mesma clausura; manter o algoritmo de referência
para comparação aleatória. Informar separadamente o custo de tracing
condicional: ARC com ephemerons não promete ausência total de tracing.

### 22.4 Weak e finalizadores

Manter índice reverso `alvo IdArc → portadores weak` para invalidar sem varrer
todas as weak em cada morte acíclica. Alterar `WeakReference_getTarget` para
consultar estado/geração e produzir owner antes de sair da seção crítica do
isolate. Alvo `Coletando`/`Morto` retorna null; `Vivo` protegido pode ser promovido.
Ao mudar alvo, retirar entrada antiga do índice; weak nunca incrementa RC.

Em `finalizadores.rs`, separar estados do anexo: `Registrado`, `Desanexado`,
`Elegivel`, `Enfileirado`, `Executado`. A transição para elegível remove o anexo
da estrutura de detecção uma única vez. Token/callback da ação pronta ganham
owners de fila antes da liberação dos owners do anexo. `concluir_finalizacao`
solta a ação consumida; `desanexar` retira apenas anexos ainda desanexáveis.

Auditar `Heap::raizes`: hoje enumera `a.dono` e ações Dart dos anexos. Não tomar
esse fato isoladamente como prova de que a vida do dono está correta segundo
Dart. Mapear papel de cada campo do anexo à API: alvo e token de desanexação
são fracos; callback/token entregue precisam permanecer vivos enquanto a
ação for elegível; a vida do finalizador governa quais ações são exigidas ou
permitidas. Se a implementação tracing divergir da semântica, registrar e
corrigir a base antes de usá-la como oráculo. ARC não deve eternizar finalizadores
por transformar seu índice de registro em raiz incondicional.

`eventos.rs::laco_de_eventos` recebe ações Dart prontas; executá-las fora do
empréstimo mutável do `HEAP` e fora da transação de coleta. Callbacks nativos
usam o protocolo permitido pela API e o mecanismo de encerramento existente;
não receberão acesso ao objeto Dart morto. Recursos C liberados por callback
não viram destrutores Dart. `encerrar_finalizadores` deve distinguir ações já
executadas, anexos cancelados e obrigações nativas ainda pendentes.

`ArcKeepAlive` para `Finalizable` é introduzido durante lowering, quando o
escopo semântico ainda existe; `await` transfere essa obrigação para o estado
suspenso. Não tentar reconstruir o escopo a partir do LLVM otimizado.

## 23. Async, isolates, FFI e recarga

### 23.1 Capturas e suspensão

Em `lower/closures.rs`, captura de valor cria ocorrência no ambiente; captura
mutável conta a célula compartilhada. Em `lower/async_sm.rs`, cada campo do
estado suspenso é slot forte. Antes de suspender, mover owners vivos para
esses slots; limpar slots de origem/observação. Ao retomar, mover de volta ou
emprestar enquanto o frame continua proprietário. Ao terminar normalmente ou
em erro, limpar campos de estado que não serão usados novamente. Não pressupor
que um Future possui operação genérica de cancelamento: tratar somente os
caminhos de cancelamento/encerramento realmente oferecidos pela API envolvida.

Eventos e timers em `eventos.rs::{enraizar,soltar_raiz}` devem adotar a mesma
convenção de transferência. Ao tirar callback da fila, o executor recebe o
owner; ao cancelar, a fila solta sua ocorrência. Um timer periódico mantém
um owner até cancelamento/encerramento; cada execução tem empréstimo protegido.

### 23.2 Isolates e mensagens

`isolados.rs::copiar_para_outro_isolado` e a serialização em `portas.rs` mantêm
mapa origem→destino durante cópia, preservando alias e ciclos. Criar objetos
destino zerados e enraizados antes de preencher arestas; só publicar mensagem
depois da construção completa. Falha desfaz o grafo parcial sem deixar roots
temporárias. A fila receptora possui o grafo/mensagem até entrega ou fechamento.

`TransferableTypedData` transfere posse do buffer externo de forma indivisível:
origem perde acesso segundo sua API, destino ganha descritor/owner; exceção antes
do ponto de publicação mantém posse na origem. Objetos Dart ordinários não
compartilham contadores entre heaps. Imagens imutáveis compartilhadas usam
registro de vida da imagem, não retain não atômico de outro isolate.

No encerramento, impedir novas publicações, drenar/cancelar entradas conforme
protocolo, executar obrigações nativas, soltar raízes e coletar o heap final.
`ponto_seguro` é candidato à drenagem/coleta; não coletar no meio de publicação
em `rodar_isolado` ou de callback externo.

### 23.3 Fronteira C

Handles locais da API nativa pertencem a um escopo; handles persistentes a uma
tabela de raízes. Criar copia/retém; destruir consome uma vez. Callback estrangeiro
na thread errada agenda execução no isolate quando a API permitir, ou segue
o diagnóstico da API; não acessa `Heap` diretamente. Um `Pointer` para memória
C não incrementa RC do conteúdo; uma view gerenciada mantém seu backing store.

Auditar `ffi_api_nativa.rs`, `ffi_callbacks.rs` e trampolins de `lower/ffi.rs`:
todo retorno gerenciado deve entrar no protocolo owned antes de um safepoint;
argumentos permanecem vivos por toda a chamada e reentrada; ponteiros interiores
dependem de owner explícito do objeto/buffer; saída excepcional limpa escopos.
O modo ARC não autoriza guardar endereço de objeto por C fora de um handle.

### 23.4 JIT e hot reload

Registrar descritores e ABI antes de publicar código em `crates/jit/src/lib.rs`.
Em `vivo.rs`/`reload.rs`, versão antiga continua registrada enquanto houver
objetos, closures ou frames que a usem. `migracao.rs` prepara novos layouts,
retém novos destinos, publica a migração e solta removidos; falha antes da
publicação restaura o estado original. `epoca_de_layout` participa da validação
de descritores e invalida resumos de otimização dependentes de classe.

Não mudar endereço/handle de objeto observado sem um mecanismo de identidade
já suportado pelo runtime. Quando a migração exigir corpo maior, usar a
indireção de armazenamento existente ou conservar a versão antiga; não trocar
o handle por conveniência. `delta.rs` deve rejeitar mistura tracing/ARC.
Enquanto um desses caminhos não estiver implementado, rejeitar a combinação
na entrada; a entrega final exige testes JIT e recarga, não apenas a rejeição.

## 24. Configuração, ABI, caches e distribuição

Adicionar identificador versionado de memória ao registro da imagem:
`{ modo, versao_arc, versao_descritores, versao_ownership, alvo }`. O runtime
valida antes de executar inicializadores Dart. Para bibliotecas estáticas,
emitir referência a símbolo versionado de ABI, fazendo mistura falhar também
na ligação. Para DLL/JIT, verificar registro antes de publicar os símbolos.

`sdk_modulo.rs::chave_do_sdk` já considera raízes/exceções; acrescentar memória
e versões. `cache_objeto.rs::{chave,chave_de_bytes}` deve receber a configuração
efetiva, não depender de o texto IR incidentalmente diferir. Incluir modo no
caminho/manifesto dos artefatos e nos relatórios de benchmark.

`RuntimeCache` seleciona bibliotecas precompiladas via variantes principal,
DLL e produção; `crates/emit_native/build.rs` deve gerar as variantes ARC
correspondentes e incorporá-las ao pacote. Atualizar `driver::compile_and_link`
para escolher runtime e SDK da mesma configuração em desenvolvimento e em
produção/LTO. Não exigir que o usuário compile runtime em sua máquina.

Testes negativos: SDK tracing + programa ARC; DLL ARC de outra versão;
objeto reutilizado de cache tracing; módulo JIT com descritor incompatível.
Todos falham antes de executar Dart, com diagnóstico que identifica os dois
artefatos/configurações. Seleção desconhecida em CLI também é erro antecipado.

## 25. Otimizações planejadas, provas e medição

O projeto deve entregar uma implementação correta e um caminho definido de
otimização. O mapa lateral e as chamadas explícitas são referência verificável;
não justificam deixar a versão final com sobrecarga evitável sem investigação.
Cada otimização mantém o caminho de referência selecionável em testes.

| Otimização | Onde implementar | Condição de segurança e medida |
| --- | --- | --- |
| Transferir resultado direto para campo/retorno | `otimizar/arc/otimizar.rs` | Owner com único consumo naquele caminho; falha do store preserva o token. Medir retains/drops eliminados. |
| Empréstimo de leitura | Mesmo passe + efeitos | Owner do receptor e estabilidade do slot até o último uso; chamada que pode mutar invalida a prova. Não basta `rc(receptor)>0`. |
| Eliminar copy/drop | Mesmo passe | Sem efeito observável/interferência entre as operações; respeitar weak e `Finalizable`. Validar CFG e exceções depois. |
| Propagar transferências em φ/loops | Mesmo passe | Prova por aresta e backedge, inclusive saídas excepcionais; não colocar retain a cada iteração desnecessariamente. |
| Especializar retornos/argumentos internos | Resumos HIR e stubs | Todas as chamadas conhecidas e ABI explícita; exportações/dinâmicas mantêm convenção geral. Cache do resumo depende do corpo e layout. |
| Inlining e eliminação de caixas | Otimização normal antes de ARC | Preservar identidade observável, captura e extensões de vida. Recalcular ownership sobre o corpo resultante. |
| Alocação em stack/eliminação de objeto | Análise de escape e LLVM | Não escapa, não tem observação weak/finalização/identidade incompatível e não atravessa suspensão. `rc==1` sozinho não prova unicidade semântica. |
| Contador por página/índice | `arc.rs`, `espaco.rs` | Mesmas gerações/estados, benchmark de localidade e bytes por objeto; não ampliar header sem versionar ABI. |
| Retain/release em linha | `llvm/mod.rs` | Tag, imortalidade, overflow, zero/candidato e geração corretos; caminho lento obrigatório. Comparar IR otimizado, tamanho e chamadas. |
| TLAB ARC | Alocador + LLVM | Reservar metadados e registro de objetos junto aos blocos; publicação atômica; stress em cada fronteira. |
| Cópia de buffers em lote | `listas.rs`, `colecoes.rs` | Multiplicidade, alias e sobreposição; reter entradas antes de soltar saídas, ou mover ownership comprovado. `memmove` de bytes sozinho não transfere contagem. |
| Suprimir candidatos acíclicos | Descritores/análise de tipos | Prova fechada de ausência de caminho de retorno, invalidada por subclasses/layout/dynamic; não inferir de objeto atualmente sem campos. |
| Índice de ephemerons | `arc_condicionais.rs` | Mesmo ponto fixo da implementação de referência; worklist por chave/portador e proteção de mutações. |
| Reclamador por classe | `espaco.rs` | Mesma morte lógica, contabilização e quarentena; medir fragmentação e RSS, não só número de blocos. |

Não habilitar redução de contadores a inteiros pequenos sem estratégia de
overflow. Saturação silenciosa que transforma objeto em imortal não atende a
recuperação de memória. Não usar RC para copiar mutável com identidade nem
aplicar reutilização destrutiva a objeto ainda observável por weak/FFI.

Medições mínimas por configuração: tempo total, throughput, p50/p95/p99 e máximo
de pausas, retains/drops emitidos/executados, candidatos, arestas de trial,
rodadas abortadas, tempo de ponto fixo, bytes de metadados/proteção/quarentena,
pico de heap/RSS, tamanho de binário e tempo de compilação. Informar versão,
alvo, SDK, opções, aquecimento e amostras brutas. Separar coleta acíclica,
cíclica e condicional. Não prometer vantagem universal sobre tracing.

## 26. Plano de execução, testes e conclusão

### 26.1 Entregas em ordem de dependência

| Entrega | Arquivos principais | Critério para prosseguir |
| --- | --- | --- |
| E0 — inventário e baseline | Tabelas de ownership/descritores, testes existentes | Cada construtor/escritor/raiz tem classificação; divergências de weak/finalizadores identificadas; pré-condições do documento-base verificadas. |
| E1 — configuração e ABI | CLI, contexto, módulo, caches, build/driver | Modo atravessa programa e SDK; mistura de artefatos é rejeitada; tracing mantém comportamento. |
| E2 — HIR e verificador | `hir.rs`, lowering, `otimizar/arc`, CFG excepcional | Testes positivos/negativos de tokens, φ, loops, borrows e saídas; emissão ainda pode usar tracing como base de auditoria. |
| E3 — runtime e stores | `arc.rs`, descritores, heap, listas, LLVM | RC recontado coincide em todos os safepoints do corpus acíclico; nenhuma escrita sem contrato; alocador recupera e reutiliza com geração correta. |
| E4 — ciclos e condicionais | `arc_ciclos`, `arc_condicionais`, weak/finalizadores | Provas/invariantes dos §§19 e 22 em testes aleatórios e dirigidos; nenhum ciclo fechado esquecido sob pressão. |
| E5 — fronteiras completas | Exceções, async, eventos, FFI, isolates, JIT/reload | Matriz semântica passa nos modos/plataformas implementados, com vida estendida e limpeza exata. |
| E6 — otimizações e relatório | Passes ARC, LLVM/alocador e benchmarks | Cada otimização tem teste de contraexemplo e comparação com referência; regressões reportadas; métricas e limites publicados. |

E3 pode ser testado isoladamente com grafos acíclicos, mas não é versão final
de ARC. E4 não substitui E5. Uma capacidade ainda ausente deve gerar diagnóstico
explícito durante desenvolvimento e permanecer pendência de entrega.

### 26.2 Testes a criar

Arquivos sugeridos são **novos**; adaptar ao harness existente sem inventar um
segundo executor de Dart:

| Local | Casos e asserções |
| --- | --- |
| `crates/runtime/tests/arc_contagem.rs` | Autoatribuição; duas arestas iguais; Ref→escalar→Ref; construtor que falha; overflow/underflow; cadeia longa sem recursão; zero protegido; bytes externos liberados uma vez. |
| `crates/runtime/tests/arc_ciclos.rs` | Autociclo, dois nós, diamante, múltiplas arestas, componente com entrada externa, branco→sobrevivente, candidato obsoleto, limite de orçamento e coleta sob pressão. |
| `crates/runtime/tests/arc_condicionais.rs` | Valor→chave, cadeia de chaves, portador morto, chave viva sem RC externo direto, mutação de valor protegido, remoção da última entrada, weak promovida e reuso do endereço. |
| `crates/emit_native/tests/arc_ownership.rs` | φ com consumo diferente, loop, aresta crítica, borrow escapado, store sem proveniência, extern desconhecida, resultado ausente no erro e `Finalizable` após último uso. |
| `crates/emit_native/tests/arc_ir.rs` | Todas as especializações de campo/alocação/lista; nenhum bypass de store; cleanup em função sem catch; atributos LLVM e modo/ABI. |
| `corpus/nativo/arc_*` | Identidade, ordem de getters/setters, avaliação de argumentos, finally/rethrow, closures cíclicas, async, coleções, timers, handles FFI e mensagens com alias/ciclos. Usar a organização/extensão do corpus existente. |
| `crates/jit/tests/arc_reload.rs` | Campo adicionado/removido, closure de versão antiga, falha de migração e tentativa de ABI diferente. |

Adicionar gerador de grafos pequenos com semente reproduzível. Operações:
alocar, copiar owner, mover, substituir slot, remover raiz, registrar/remover
ephemeron e promover weak. O oráculo independente calcula alcançabilidade forte
mais ponto fixo condicional. Após coleta completa, nenhum alcançável morreu;
nenhum inalcançável elegível permanece sem justificativa de fila/quarentena;
RC dos vivos coincide com a soma de ocorrências. Comparar proteção otimizada
e de referência. Injetar falha em reservas para confirmar atomicidade.

Testes de finalização devem afirmar o que Dart garante: alvo alcançável não é
finalizado, desanexação respeitada, callback não duplica, proteção `Finalizable`
preservada e obrigações nativas cumpridas. Não exigir da VM ordem exata nem
execução imediata de callbacks opcionais. Testes internos podem forçar drenagem
determinística sem transformar isso em contrato público.

### 26.3 Execução da validação

Durante cada entrega, rodar os testes direcionados da crate alterada. Quando
os novos testes existirem, os comandos de base são:

```powershell
cargo test --locked -p dartforge-runtime arc
cargo test --locked -p dartforge-emit-native arc
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace -- --include-ignored
cargo doc --locked --workspace --no-deps --document-private-items
cargo run --release -p dartforge-diferencial
```

O filtro `arc` exige que os nomes dos testes novos contenham esse prefixo; o
relatório deve mostrar quantidade executada e falhar se for zero. Esses comandos
não selecionam automaticamente ARC no diferencial atual: implementar no harness
a matriz explícita `tracing/arc × pendência/tabelas × debug/otimizado`, registrando
as opções efetivas. Acrescentar raízes sombra/mapas quando ambos forem suportados.
Reproduzir em Windows e Linux; macOS só é marcado coberto após execução real.

### 26.4 Definição de pronto

A implementação está pronta para avaliação final quando todas as entregas E0–E6
têm evidência, o inventário não contém entradas desconhecidas, o verificador
barra os contraexemplos, a matriz semântica passa e os relatórios distinguem
suporte real de plataforma não testada. A seleção ARC permanece opt-in até
decisão explícita de mudar o padrão. O documento especifica trabalho futuro;
sua conclusão editorial não afirma que o runtime, as otimizações ou os testes
descritos já foram implementados.
