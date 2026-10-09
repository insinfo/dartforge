# Especificação de implementação — ARC com coleta de ciclos no backend nativo

**Estado:** proposta de engenharia e plano de prova; não é implementação nem mudança
do gerenciador padrão. **Documento-base imutável:**
[`EXPERIMENTO-ARC.md`](EXPERIMENTO-ARC.md). Este texto detalha aquele experimento,
sem alterar sua sintaxe, suas condições de entrada, suas garantias de Dart ou sua
decisão de manter tracing como referência. **Alvo semântico inicial:** Dart 3.6.2.

**Guia de execução:** as seções 18–26 fixam a infraestrutura, os pontos
de alteração no código, os contratos e as entregas verificáveis. As seções
27–34 especificam a análise estática que seleciona regiões, grupos, empréstimos
e ARC especializado para reduzir a coleta de ciclos em execução. Nomes marcados
como **novos** são APIs/arquivos a criar, não recursos já implementados. As
seções anteriores fornecem os requisitos e a motivação. As especializações
27–34 complementam os contratos gerais; toda especialização exige certificado
validado e preserva a política geral quando não houver prova suficiente.
As seções 35–36 acrescentam um pacote opcional de anotações e contratos para
caminhos de baixa latência; não tornam essas anotações requisito para usar ARC.

## 0. Escopo, linguagem de certeza e critério de aceitação

ARC significa *automatic reference counting*: operações inseridas pelo compilador,
sem exigir `retain`, `release`, `weak` ou anotações no código Dart. Metadados
opcionais da seção 35 usam a sintaxe de anotações já existente na linguagem.
O mecanismo inclui
recuperação residual de ciclos; ARC puro não cobre grafos Dart arbitrários.
Imagens integralmente provadas podem dispensar o trial ordinário (§32.3).
O modo experimental deve ser opt-in,
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

**Objetivo de otimização:** resolver na compilação toda política de vida que as
análises consigam provar, inclusive a liberação conjunta de grafos cíclicos
confinados. Gerar coleta residual somente para as relações não resolvidas.
Medir essa cobertura por objetos/bytes executados e trabalho de coleta, sem
prometer uma fração residual pequena para todo programa nem reter lixo até o
fim do processo para obter artificialmente zero coletas.

**Fora desta especificação:** trocar a sintaxe Dart; expor ownership ao usuário;
substituir o modelo de erros por destrutores; transformar referências fortes em
fracas automaticamente; prometer tempo real; assumir que o passe ObjC ARC do LLVM
ou o borrow checker do Rust gerenciam objetos Dart.

O perfil opcional `Realtime` (§36) verifica um conjunto de operações permitido
no trecho crítico. Não promete prazo máximo independente de hardware, sistema
operacional e integração de áudio. Compilar sem esse perfil continua aceitando
o Dart suportado; uma falha de contrato é diagnóstico da opção solicitada.

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

Essa equação define o caminho ARC ordinário. Com as especializações estáticas,
distinguir grafo **semântico** de referências Dart e grafo **de retenção**
gerado: uma região/grupo tem uma unidade de vida própria; empréstimos possuem
âncora independente; contadores somam apenas arestas contadas (§§27.2 e 31).
A auditoria deve conhecer ambos os grafos. Não atribuir `rc=0` de objeto ARC
ordinário a um objeto regional para representar ausência de contador.

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

Antes de inserir operações RC, executar a seleção de política de memória das
seções 27–31: eliminação de alocações, inferência de regiões, prova de
aciclicidade por sítio/contexto, empréstimos e grupos selados. A eliminação de
pares retain/release abaixo otimiza o trabalho restante. Resumos incompletos
reduzem precisão; não justificam exigir mudanças no código Dart.

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

Objetivo: recuperar subgrafos fechados residuais com RC positivo causado só
por arestas internas, depois da seleção estática de política. Seguir a ideia
de Nim ORC/Bacon–Rajan, sem importar seus destrutores. Um `release` que não
zera o RC de objeto potencialmente cíclico o coloca,
uma vez, no buffer de candidatos. Descritor comprovadamente sem caminho de
retorno pode dispensar candidatura; isso exige prova de layout, inclusive
coleções mutáveis e subclasses.

Uma rodada no isolate tem fases explícitas:

1. **Selecionar região candidata.** Drenar até um orçamento de candidatos;
   capturar estabilidade do grafo por pausa no mutador. A versão especificada
   usa pausa; concorrência exigiria outro protocolo e não é presumida. O tamanho
   da pausa deve ser medido, não é necessariamente curto.
2. **Marcar cinza / subtração experimental.** Para cada objeto alcançado por
   arestas fortes contadas da região, registrar `trial(x)=rc(x)` e subtrair **cada
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
    → points-to/resumos/forma → plano de memória certificado por alocação
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
| `crates/emit_native/src/lib.rs`, `sdk_modulo.rs` | Ambos usam `otimizar::preparar_para_emissao`, que define o modo de memória, otimiza e materializa as tabelas por último. Integrar a preparação excepcional, a inserção e a verificação ARC (§20) neste pipeline comum. |
| `crates/emit_native/src/hir.rs` | Acrescentar ownership, origem de ponteiros, operações ARC, extensão de vida e saídas excepcionais explícitas (§20). |
| `crates/emit_native/src/lower/{mod,fn_builder,comandos,closures,async_sm,literais,membros}.rs` | Preservar classificação de referências, locais, capturas, globais, `Finalizable`, estado suspenso e destinos de erro antes da análise de ownership. |
| `crates/emit_native/src/otimizar/{mod,operandos,cfg,tabelas}.rs` | Atualizar visitantes/CFG e separar preparação de exceções de sua emissão; inserir e verificar ARC, sem tratar operações de contador como instruções puras. |
| `crates/emit_native/src/llvm/mod.rs`, `llvm/listas_ir.rs`, `llvm/raizes.rs`, `llvm/externs.rs` | Emitir a ABI ARC; cobrir stores, inicializações, resultados, raízes e chamadas especiais (§21). |
| `crates/runtime/src/heap.rs` | Integrar estado ARC em `Heap`; alterar `antes_de_alocar`, alocação, `definir_campo`, raízes, tabelas laterais e `collect`. Separar política de vida da varredura física. |
| `crates/runtime/src/espaco.rs`, `layout.rs` | Implementar reclamador ARC sem reutilizar marcas de tracing como evidência de vida; preservar layout e invariantes do alocador (§19.4). |
| `crates/runtime/src/{nucleo,gc_raizes}.rs` | Adaptar `dartforge_object_new/get/set`, `dartforge_alocar`, registro de imagem e raízes; não abastecer TLAB sem registro ARC. |
| `crates/runtime/src/{listas,nativos_listas,colecoes,closures,tipos}.rs` | Enumerar e contabilizar todos os campos/buffers gerenciados, inclusive substituição, cópia, crescimento e redução. |
| `crates/runtime/src/{nativos_sistema,finalizadores,eventos,excecoes}.rs` | Integrar promoção weak, ephemerons, anexos, eventos e pendências com a máquina de estados de morte. |
| `sdk_nativo/core/finalizer_patch.dart`, `sdk_nativo/ffi/ffi_native_finalizer_patch.dart` | Preservar ligação do callback à zona, classificar a ação capturada e transportar `externalSize` ao runtime; atualmente o patch nativo valida esse tamanho, mas não o passa à extern. |
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
O handle Dart continua sem geração embutida: não alegar que comparar metadados
detecta todo uso de handle cru após reuso. Filas/weak/handles persistentes guardam
a geração; para código gerado, prevenção vem do verificador e a depuração usa
quarentena e registros de origem. Alterar a representação pública para carregar
geração seria outra ABI.

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

No caminho ARC ordinário, `Heap::alocar_bloco` registra metadados antes de publicar o handle. Inicializar
com `rc=1`, estado `Construindo`, corpo/bitmap zerados e descritor válido.
Essa ocorrência pertence ao construtor; a conclusão a transfere para o resultado.
Cada campo inicializado por cópia retém o destino. Construtor com falha solta
apenas campos já inicializados e consome a ocorrência de construção. Enquanto
construindo, o objeto consta das raízes temporárias do runtime.

`reter(h)` ignora null/Smi e imortais, verifica estado e faz incremento
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
Getter retorna owned na ABI geral: copiar a referência retendo-a antes de
permitir reentrada. Leitura borrowed é otimização HIR com prova de estabilidade
do slot, não simples consequência de o receptor permanecer vivo.

Contratos das APIs **novas** internas de `Heap` (assinaturas de projeto):

| Método proposto | Resultado e obrigação |
| --- | --- |
| `arc_registrar(h: Ref, descritor: IdDescritor)` | Registra bloco já reservado/zerado, gera identidade e owner de construção; falha antes de publicar `h`. |
| `arc_reter(h: Ref)` / `arc_soltar(h: Ref)` | Opera uma ocorrência, sem safepoint nem código Dart; violações internas seguem diagnóstico fatal definido, nunca wraparound. |
| `arc_ler_forte(slot: SlotArc) -> Ref` | Valida tag, retém e devolve owned; `SlotArc` descreve receptor/localização, não deixa ponteiro Rust sobreviver à mutação. |
| `arc_substituir(slot: SlotArc, valor: Valor, modo: ModoStore)` | Valida tudo antes do commit; falha mantém slot e owner de entrada; sucesso cumpre Copy/Move. |
| `arc_drenar_zeros(limite: usize)` | Processa somente objetos vivos de geração correspondente e RC ainda zero, sem proteção; budget conta trabalho entre transações. |
| `arc_coletar(motivo: MotivoColeta) -> EstatisticasArc` | Exige safepoint/publicação de raízes, sem transação ativa; inclui rodada completa para pressão, condicionais ou solicitação explícita. |
| `arc_auditar() -> Result<(), ErroArc>` | Reconta ocorrências e valida grafos/filas sem modificar decisão de vida. Só roda com heap estabilizado. |

`SlotArc`, `ModoStore`, `MotivoColeta`, `EstatisticasArc` e `ErroArc` são tipos
novos. Separar erro Dart de validação de campo/índice, ainda tratado pelo
chamador, de violação interna de RC. Não retornar um handle inválido depois
de falha interna. A integração com Rust usa um guard de transação cuja saída
restaura o estado de controle; não depende de unwind C para desfazer contagem.

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

Com unidades especializadas, `D` contém unidades completas elegíveis: grupo
não pode ser descartado membro a membro e região ativa só termina pelo seu
token. Expandir membros depois de decidir a morte da unidade, sem chamar
release individual sobre metadata de contador coletivo. As saídas da unidade
são debitadas uma vez; o §31.4 define o fechamento para a coleta global.

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

Representação disponível em `otimizar::arc::Ownership`: `Trivial`, `Owned`
e `Borrowed { owner, escopo }`. A origem pode ser o chamador (parâmetro) ou
um `ValueId` local. `vivacidade_classificada` exige uma entrada explícita
por parâmetro/definição, rejeita IDs obsoletos e converte dependências locais
para a análise de empréstimos. O inventário é fornecido pelo chamador desta
API; ainda não há produtor automático, transporte pela HIR/otimizações nem
validação semântica de contratos, proveniência, escopos e consumo de tokens.
O inventário confere também a dominância SSA dos owners locais em blocos
alcançáveis, incluindo ordem das definições no bloco e simultaneidade de
`Phi`. Isso não prova a disponibilidade do token após consumo nem a
existência de um resultado de `invoke` na saída excepcional; ambas exigem
o verificador de fluxo descrito na §20.3.

`vivacidade_classificada_com_excecoes` confere a disponibilidade dos
resultados de invoke nos usos diretos, além da definição de aliases:
retornos/instruções precisam atravessar a aresta de sucesso; entradas de
Phi usam a aresta do predecessor, permitindo null no erro e o resultado
no sucesso. Essa checagem ainda não prova consumo ou transferência de tokens.

`arc::verificar_escopos` recebe `PlanoEscopos` explícito, com limites antes
de instruções/terminadores e nas arestas. Confere usos e definições borrowed,
incluindo sua cadeia transitiva, exige pilhas iguais nas junções/backedges
e aplica cleanup de aresta antes dos usos de Phi. O plano ainda não é
produzido pelo lowering nem transportado nas otimizações; essa API não
insere proteção, prova escape/consumo nem transfere owners no retorno.
Erros de fluxo dessa API incluem caminho de blocos desde a entrada;
junções incompatíveis mostram os dois caminhos. A árvore de descoberta
guarda um predecessor por bloco, reconstruindo os caminhos só no erro.

`arc::verificar_tokens` recebe contratos explícitos por instrução ordinária
(`PlanoTokens`); contrato ausente é erro, não um empréstimo presumido. Sobre
CFG/SSA e representações previamente válidos, confere disponibilidade e
consumo único: copy cria token, move transfere, drop consome, Phi owned
transfere simultaneamente na aresta. Junções/backedges exigem inventários
iguais. Invoke produz resultado apenas no sucesso e aplica consumos de
sucesso/erro separadamente. Retorno owned transfere ao chamador; retorno
borrowed exige origem externa. Saídas rejeitam tokens restantes. Isso não
certifica os contratos fornecidos, escopos, invalidação de slots/borrows,
regiões ou estados suspensos; o produtor e a integração continuam pendentes.

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

`ArcCopy`, `ArcDrop` e `ArcMove` estão representados na HIR e emitidos:
copy chama a ABI retain; drop chama release; move transfere os bits sem
operação RC física. Copy/move têm resultado Ref fixo, drop Void. Recebem
SSA Ref já avaliado ou null, sem boxing/alocação implícita no operador;
I64 sem proveniência e literal ainda não avaliado são recusados. Visitantes
de leitura/substituição e do corpo assíncrono incluem seus operandos.
Os passes gerais preservam as operações, inclusive inlining com remapeamento
de parâmetros/resultados. Isso ainda não insere ownership automaticamente,
não integra a prova auxiliar de tokens ao pipeline nem transporta obrigações
através de suspensão.
ArcLoadStrong/ArcStoreStrong também são representados para
`SlotForte::Quadro { quadro, indice }`, com ID SSA I64 e índice do slot Ref
proprietário. O runtime valida identidade/propriedade/limite; ponteiro nativo
não é descritor de quadro. Load cria token owned independente. Store Copy
retém uma ocorrência, Move consome o token do código sem reter a origem;
ambos publicam antes de soltar o conteúdo antigo. O verificador de tokens
aplica esses efeitos fixos e exige ID do quadro Trivial. Ainda não certifica
abertura/fechamento ou vida do quadro, nem produz descritores automaticamente.
`SlotForte::Global { simbolo }` identifica o armazenamento Ref declarado
exatamente uma vez no módulo. A carga lê os bits da área de globais do isolate
e retém o resultado, inclusive null/Smi. Store Move publica os bits e transfere
o token ao registro de owner, sem safepoint no intervalo. Store Copy usa token
temporário até publicar e atualizar o registro. Esse registro não é o valor
do global: ele omite null/Smi. O verificador rejeita símbolo ausente/duplicado
ou representação diferente de Ref. Não insere inicialização lazy nem certifica
proveniência geral/ABI importada; inserção no lowering continua pendente.
Slots de heap/nativos, as demais operações e o produtor semântico
continuam pendentes.

`Alloca` de Ref representa slot proprietário inicializado com null. `Load` faz
cópia owned; `Store` substitui ou move. `mem2reg` converte essa propriedade em
SSA antes de inserir contadores. `StoreGlobal` usa slot global contado. Store
genérico sem classificação stack/global/heap/native é erro do compilador ARC.
Adicionar variantes aos visitantes em `otimizar/operandos.rs`, poda, impressão,
substituição, clonagem, inlining e visitantes de `lower/async_sm.rs`.

### 20.2 Inserção por CFG

Implementar `arc::inserir(module) -> Result<(), DiagnosticoArc>` em fases:

0. Consumir `PlanoMemoria` validado (§27). Materializar limites de região/grupo
   e empréstimos certificados antes dos tokens ordinários; valores regionais
   têm obrigação de vida da região, mesmo sem retain/release individual. Sem
   certificado, selecionar ARC geral e continuar a compilação normalmente.
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

Base disponível: `otimizar::arc::vivacidade` calcula o ponto fixo reverso
nos blocos alcançáveis e expõe conjuntos antes/depois das instruções, por
bloco e por aresta. As entradas de `Phi` são usos apenas na aresta do seu
predecessor, inclusive backedges e as arestas excepcionais preparadas.
O inventário de referências/slots é fornecido pelo chamador; a análise não
infere ownership por largura de representação.
`vivacidade_com_emprestimos` inclui a cadeia de sustentação em cada uso do
alias, também nas entradas de `Phi` por predecessor. Rejeita dependências
cíclicas e IDs ausentes com `ARC003`, origem e caminho; diagnósticos seguem
ordem de ID. Isso não verifica escopo, dominância ou escape do empréstimo.
Ainda falta a classificação semântica, escopos e keep-alive, tokens,
inserção e verificação. A análise permanece fora do pipeline de emissão
até esses consumidores serem implementados.

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

O pipeline comum já separa `tabelas::preparar` de `tabelas::materializar`:
a primeira expõe as arestas existentes e guarda um inventário de sítios;
a segunda confere o inventário e calcula as saídas sobre a HIR final antes
de publicar `Module::tabelas`. Não há ainda inserção ARC entre elas. A
preparação mantém em tracing a política de atravessar funções sem tratador.
Em ARC, não elimina o pouso local só porque a saída devolve o valor padrão:
chamadas diretas e chamadas que podem voltar pendentes expõem a aresta de
unwind. Ainda será necessário inserir os cleanups dos owners nessas arestas;
pouso existente não prova limpeza. Eliminar esse pouso em ARC exigirá prova
de ausência de obrigações de cleanup.
Mudanças de IDs ou ordem das funções exigem atualizar o inventário; o estágio
atual rejeita um inventário incompatível em vez de emitir tabelas antigas.

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

Catálogo inicial disponível em `crates/runtime/ownership.tsv`: 13 externs da
família `dartforge_arc_`, auditadas com Ref borrow/consume, escalar/native e
resultado owned/escalar/void, retenção persistente e invalidação de borrows.
O build rejeita duplicatas, símbolo inexistente, tipos/modos inválidos,
ausência de contrato nessa família e efeito que passe a lançar/rodar Dart.
Coerções de ponteiros de função gerados conferem aridade e representação Rust.
`dartforge_runtime::ownership::contrato` retorna erro para símbolos ausentes.
Este catálogo ainda é parcial: cobertura completa das externs, retornos
borrowed, saídas excepcionais e consumo pelo produtor HIR/LLVM permanecem
pendentes. Não há convenção borrowed implícita para completar a cobertura.

`otimizar::arc::contrato_chamada_runtime` traduz essas chamadas auditadas
para consumo SSA e classe do resultado, conferindo aridade e tipos declarados.
Ref deve estar em SSA ou ser null; I64 não vira Ref por ter a mesma largura.
O verificador de tokens confere planos de chamadas auditadas contra esse
catálogo. Retain direto é recusado: sua produção sem resultado SSA precisa
ser representada por ArcCopy. Retenção persistente/invalidação são devolvidas
ao produtor, mas não certificadas pelo verificador de tokens; a análise de
borrows/slots continua obrigatória. Esse acesso ainda não gera automaticamente
o inventário nem os contratos de todas as instruções do lowering.

Extensão auditada do catálogo: `dartforge_gc_global_root` recebe ID nativo
de armazenamento e Ref borrowed, cria owner persistente e pode invalidar
borrows associados ao valor substituído; não consome o token SSA do argumento.
`dartforge_marcar_constante` recebe Ref borrowed e endereço nativo do getter,
mantém a referência permanente e também não consome SSA. O catálogo passa a
15 entradas; todas as auditadas exigem cobertura e assinatura Rust compatível.
Novas linhas explícitas não precisam pertencer ao prefixo ARC. Execução Dart
continua exigindo esquema e auditoria próprios, sem contrato normal implícito.

O esquema passa a exigir seis colunas: a última declara `normal` ou `pending`,
conferida exatamente com o flag de exceção da tabela de efeitos. Modos Ref
`consume`, `consume-success` e `consume-error` representam consumo comum,
no sucesso e na pendência; os dois últimos exigem saída pending. Resultado
owned é produzido apenas no sucesso. A tradução HIR gera os três conjuntos
de EfeitoTokens e exige invoke preparado no verificador. `gc_collect` e
`marcar_permanente` têm contratos explícitos pending, ampliando o catálogo
para 17 externs. Os modos de consumo condicional têm testes sintéticos do
gerador; não há ainda uma extern real do catálogo que consuma por uma única
aresta. Retornos borrowed, execução Dart, cobertura geral e geração automática
do plano completo continuam pendentes.

`produzir_contratos_runtime` gera classes dos resultados e efeitos de todas
as CallRuntime de uma função auditável, sem modificar as entradas se alguma
chamada falhar na validação. Metadados anteriores incompatíveis, extern sem
contrato, IDs duplicados e resultado mal tipado são erros. Os contratos
retornados conservam retenção/invalidação para os próximos passes. Parâmetros,
outras instruções e chamadas Dart ainda exigem produtores próprios; esta API
não está ligada à emissão padrão, não insere contadores nem certifica
proveniência, slots, borrows ou cleanup. Um teste de CFG usa o plano gerado
para gc_collect e verifica que retirar o pouso ou negar pending é recusado.

`produzir_contratos_arc` acrescenta classes fixas das cinco primitivas ARC
explícitas ao produtor runtime: copy/move/load são Owned; drop/store são
Trivial, sem efeito ordinário sobrescrevendo a regra da primitiva. Faz staging
dos resultados antes de alterar os mapas; erro de tipo, classe incompatível
ou plano que sobrescreva primitiva não deixa estado parcial. Os tipos dos
operandos, a vida dos slots e o consumo no CFG continuam sujeitos aos seus
verificadores. Não classifica parâmetros ou referências de outras operações
por largura, nem insere ARC no lowering padrão.

Phi Ref sem classe recebe Owned quando todas as entradas são owned/null e
existe origem fora do ciclo de Phi/move. O produtor segue moves, propaga
fundação pelo grafo de dependências e rejeita ciclos sem origem, entrada
emprestada sem cópia e planos que sobrescrevam Phi. Metadados explícitos de
Phi borrowed/trivial são preservados e continuam exigindo suas provas próprias.
O staging cobre também erro nesta fase, após classificação ARC/runtime.
O verificador de tokens prova disponibilidade e transferência simultânea nas
arestas; a conectividade do grafo não substitui essa prova. Testes nullable,
laço com move e troca simultânea de dois Phi usam as classes geradas.

`produzir_e_verificar_tokens` combina produção ARC, verificação de tokens e
verificação de escopos antes de publicar os dois mapas. Falha de inventário,
consumo, CFG excepcional ou escopo conserva as entradas originais. Os planos
devem corresponder à mesma versão do CFG. Isso não certifica vida de slots,
invalidação de borrows, Finalizable nem cobertura dos produtores ausentes;
a emissão padrão ainda precisa integrar essa entrada após obter os metadados
semânticos completos.

Externs **novas** mínimas: `dartforge_arc_retain`, `dartforge_arc_release`,
`dartforge_arc_collect`, `dartforge_arc_verificar_abi`. Retain/release são
`nounwind` somente se falhas internas não desenrolarem pelo limite C. Elas
não executam Dart nem coleta de ciclos; o coletor roda em safepoints próprios.
Externs de campo existentes passam a seguir o modo do runtime selecionado,
com o contrato de resultado owned em ARC. O chamador não pode inferir o modo
do callee pelo nome do símbolo: o registro de ABI deve garanti-lo.

Base disponível para slots explícitos: `arc_abi.rs` exporta
`dartforge_arc_quadro_{abrir,copiar,mover,fechar}_v1`, declarados no emissor
e na tabela de efeitos. Quadros/índices são escalares; copiar recebe Ref
borrowed e cria ocorrência no slot, mover transfere a ocorrência entre
slots, fechar consome as ocorrências do quadro do topo. Todos usam o
inventário proprietário existente do Heap, inclusive em tracing. Não
coletam nem chamam Dart. Cópia/fechamento rejeitam quadro observacional.
Essa base ainda não é inserida pelo lowering e não substitui a tabela
ownership.tsv ou a ABI geral de retornos owned.

`dartforge_arc_quadro_carregar_v1(quadro, indice) -> Ref` copia o owner do
slot para um token independente em `owners_codigo`, sobrevivendo ao fecho
do quadro. `dartforge_arc_quadro_receber_v1(quadro, indice, valor)` valida
destino e token antes de mutar, transfere o token ao slot sem retain da
origem e solta o conteúdo antigo após publicação. Null/Smi não contam
fisicamente. As duas operações não coletam nem executam Dart; sua declaração
LLVM é nounwind, com falhas internas fatais no limite C. Isso é a ponte dos
slots de quadro com SSA, não um catálogo geral de contratos das externs.

As quatro externs mínimas passam a ter implementação em `arc_abi.rs`:
retain/release recebem Ref e criam/consomem ocorrências no inventário
`Heap::owners_codigo`, separado de owners de quadros, campos e mensagens.
Null/Smi não contam. Releases sem token do código abortam, mesmo quando
outro slot ainda sustenta o objeto. A multiplicidade participa da auditoria
de RC; o inventário também sustenta alcance em tracing e promoção de jovens.
Retain/release não coletam; `arc_collect()` é o safepoint explícito e pode
executar callbacks nativos, mas apenas enfileira callbacks Dart.
`arc_verificar_abi(versao: i64) -> i8` devolve 1 para versão 1 com ARC ativo,
0 para versão/modo incompatível. O chamador deve recusar o módulo nesse caso.
Isso ainda não certifica contratos de externs de um módulo, retornos owned
ou tokens produzidos/consumidos pelo lowering. O inventário por handle é
uma implementação inicial auditável; não demonstra o gate de desempenho.

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

Aplicar antes o filtro de candidatura do §32. Objetos `AciclicoProvado` não
entram na fila, mas suas arestas não desaparecem. O percurso de referência
continua completo; poda de percurso requer prova adicional. Regiões ativas
publicam suas saídas para o heap; grupos são unidades explícitas do grafo
de retenção. Borrows certificados não são subtraídos de RC como arestas contadas.

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
L = clausura_forte_e_de_unidades(raízes reais + imagens + regiões ativas)
repetir:
    para cada entrada (p, k, v):
        se p pertence a L e k é chave viva segundo a representação da API:
            acrescentar v e sua clausura forte/e_de_unidades a L
até L não crescer
D = unidades gerenciadas completas elegíveis que não pertencem a L
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

No caminho sem grupos/regiões, cada objeto é sua própria unidade e a fórmula
é a marcação comum. Com especialização, alcançar membro de grupo mantém a
unidade inteira e suas saídas, sem usar RC positivo como raiz (§31.4). A mesma
decisão de vida rege limpeza de weak/ephemerons e finalização; não limpar uma
entrada condicional por uma marcação que ignora retenção válida da unidade.

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

`Heap::raizes` hoje enumera `a.dono` e ações Dart dos anexos. Para o desenho
ARC, substituir essa raiz incondicional por arestas com origem definida:

| Parte do anexo | Representação escolhida |
| --- | --- |
| `dono` | Identidade fraca com geração no índice de anexos; o índice não mantém o finalizador vivo. |
| `valor` e `desanexo` | Identidades fracas com geração; nunca mantêm alvo/chave vivos. |
| `AcaoDeFinalizador::Dart(acao)` em anexo registrado | Aresta forte **do dono para a ação**, enumerada pelo descritor do dono mesmo estando em tabela lateral. Conta uma ocorrência; não é raiz global. |
| Ação Dart já enfileirada | Owner da fila de eventos, transferido da aresta do dono; conclusão ou descarte da fila consome essa ocorrência. |
| Ação nativa | Ponteiro de função e token C, sem aresta para objeto Dart; registro nativo mantém módulo de código/biblioteca carregado até cancelar/executar. |

Assim, `dono→ação→dono` é um ciclo normal coletável. Ação/token que realmente
referenciam o alvo o mantêm vivo enquanto o dono vive; não retirar essa aresta
para forçar finalização. No descarte de conjunto, se dono e alvo morrerem juntos,
cancelar a ação Dart registrada em vez de promover uma ação também morta.
Se dono sobreviver e alvo morrer, a ação já deve estar viva; transferi-la para
a fila antes de remover a aresta do anexo. Na morte isolada do dono, cancelar
seus anexos Dart e soltar as ações. Ao remover anexo durante descarte do dono,
o descritor/lote é responsável pelo débito: não executar um segundo release.

O registro de obrigações nativas pode sobreviver à morte do wrapper do
finalizador sem reter objetos Dart. Mantém callback/token até morte do alvo,
desanexação válida ou encerramento normal do grupo. Identidade antiga de dono
não pode corresponder a wrapper novo no mesmo endereço. Esse registro dá um
caminho explícito às obrigações nativas sem transformar o wrapper em raiz.

A escolha acima implementa o contrato de alvo/chave fracos e callbacks Dart
como eventos; a possibilidade de coletar o finalizador e a ligação à zona
constam da [fonte de `Finalizer` do Dart 3.6.2](https://github.com/dart-lang/sdk/blob/3.6.2/sdk/lib/core/weak.dart).
Auditar o tracing com esses mesmos testes antes de tomá-lo como oráculo.

`eventos.rs::laco_de_eventos` recebe ações Dart prontas; executá-las fora do
empréstimo mutável do `HEAP` e fora da transação de coleta. Callbacks nativos
usam o protocolo permitido pela API e o mecanismo de encerramento existente;
não receberão acesso ao objeto Dart morto. Recursos C liberados por callback
não viram destrutores Dart. `encerrar_finalizadores` deve distinguir ações já
executadas, anexos cancelados e obrigações nativas ainda pendentes.

`ArcKeepAlive` para `Finalizable` é introduzido durante lowering, quando o
escopo semântico ainda existe; `await` transfere essa obrigação para o estado
suspenso. Não tentar reconstruir o escopo a partir do LLVM otimizado.

O lowering preserva atualmente o tipo estático original e a classificação
Finalizable em `Local`, além da identidade de sua ligação léxica. Escopos têm
IDs monotônicos por função (zero é a invocação), independentes da profundidade;
salvar/restaurar um caso de switch preserva a identidade do registro. Ligar
uma captura no corpo corrente atribui o escopo desse corpo, mantendo o tipo
original. Esses metadados ainda não produzem ArcKeepAlive/PlanoEscopos na HIR,
nem certificam cleanup excepcional ou transferência para estados suspensos.

Aplicar a regra ao tipo estático: subtipo de `Finalizable` exceto `Never`, e
recursivamente `T?`/`FutureOr<T>`. Incluir `this`, capturas efetivas e duração do
corpo da closure. Não reter variáveis não capturadas apenas porque a closure
foi criada no mesmo bloco. Suspensão comprovadamente incapaz de retomar não
cria uma raiz eterna. O callback nativo deve poder executar fora do isolate;
trampolim Dart de `Pointer.fromFunction` não é callback válido de
`NativeFinalizer`. Essas regras e a garantia de encerramento normal são da
[fonte de `Finalizable`/`NativeFinalizer` do Dart 3.6.2](https://github.com/dart-lang/sdk/blob/3.6.2/sdk/lib/ffi/native_finalizer.dart).

Propagar `externalSize` do patch do SDK ao anexo e à pressão de memória; retirar
a contabilização uma vez em detach ou execução. Bytes externos não se tornam
arestas fortes. `encerrar_finalizadores_do_isolado` precisa respeitar a unidade
real de grupo: se vários isolates compartilham obrigações, manter registro de
grupo e só concluir o encerramento após confirmar todos os membros. Não
confundir saída abrupta do processo com encerramento normal garantido.

### 22.5 Política de agendamento e limites de trabalho

Adicionar contadores de dívida para candidatos, zeros, mortos físicos e
metadados condicionais sujos. `Heap::antes_de_alocar`, pontos seguros de laço,
retorno ao executor de eventos e coleta explícita são os pontos de atendimento.
Retain/release não chamam o coletor de ciclos por surpresa. O caminho de
alocação publica todos os owners temporários antes de atender dívidas.

Definir limites em estrutura interna versionada: número de candidatos, arestas
de trial, bytes de scratch e bytes mortos aguardando reclamador. Os valores
numéricos padrão são parâmetros de tuning a fixar com as medições de E6, não
constantes sem evidência de desempenho. Testes usam limites pequenos explícitos
para exercitar todas as transições. Dívida não atendida permanece registrada;
ao exceder o limite de retenção física, a próxima alocação segura atende-a.

Coleta explícita/pressão faz rodada completa, incluindo ponto fixo condicional,
independentemente de haver candidatos ordinários. Saída de longa sequência sem
alocações precisa de safepoints de laço/evento para progresso. Nenhum orçamento
de pausa é garantia de tempo real: uma transação grande só pode terminar ou
ser abandonada antes do commit. Reserva de emergência permite preparar o
commit; exaustão fatal deve encerrar com diagnóstico, sem continuar com RC
parcialmente alterado.

Custos a instrumentar: trial `O(|R| + |E_R|)` em tempo e scratch proporcional
à região; ponto fixo com worklist `O(|H_vivo| + |E_vivas| + |entradas|)` sob
operações de índice de custo esperado constante; marcação de proteção
proporcional aos nós/arestas novos visitados. A versão que varre todas as
entradas repetidamente pode ter custo multiplicado pelo número de rodadas;
medir explicitamente até habilitar o índice. Reclamador por páginas visita
ocupação do espaço, não apenas objetos mortos.

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
| Selecionar memória por prova estática | `otimizar/arc/analise/` (§§27–34) | Regiões, grupos, empréstimos e poda por sítio/contexto antes do ARC ordinário; validar o certificado e medir retenção adicional. |
| Suprimir candidatos acíclicos | Points-to, forma, descritores e runtime (§§28–32) | Provar ausência de participação em ciclos durante toda a vida; não remover a travessia nem inferir de objeto atualmente sem campos. |
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
| E6 — seleção estática, otimizações e relatório | Passes ARC, LLVM/alocador e benchmarks | Cumprir S0–S7 do §34, com certificados e contraexemplos; comparar com referência, reportar regressões, métricas e limites. |

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
e S0–S7 da seção 34
têm evidência, o inventário não contém entradas desconhecidas, o verificador
barra os contraexemplos, a matriz semântica passa e os relatórios distinguem
suporte real de plataforma não testada. A seleção ARC permanece opt-in até
decisão explícita de mudar o padrão. O documento especifica trabalho futuro;
sua conclusão editorial não afirma que o runtime, as otimizações ou os testes
descritos já foram implementados.

### 26.5 Estado da implementação (2026-10-08)

Esta subseção registra o que existe no código, medido nesta data; ela é
atualizada a cada avanço e não altera os requisitos acima. Detalhes de
engenharia em `docs/ARC-IMPLEMENTACAO.md` (§6, "ARC puro").

| Entrega | Estado | Evidência |
| --- | --- | --- |
| E0 | parcial: o inventário de formas é o do `GrafoDoHeap` (bitmap da instância, `REFS`, corpo de fora, anexos, efêmeros e fracos), sem a tabela formal por construtor/escritor | auditoria por recontagem (`DARTFORGE_ARC_CONFERIR=1`) em toda drenagem |
| E1 | feita: `--memoria arc`/`--memoria=arc` no AOT e no JIT; o modo entra na chave do SDK compilado; tracing segue padrão, com o IR de sempre | corpus 238/238 nos dois modos |
| E2 | **não iniciada**: não há owners na HIR (§20); a contagem é toda do runtime | — |
| E3 | feita, com desvio: o RC conta **toda** gravação forte (também em jovem), pela barreira do runtime e, nas alocações em linha do código gerado, por `dartforge_arc_inicial`; a escrita crua de um native tira uma foto, recontada na drenagem. As raízes não contam (RC adiado, Deutsch–Bobrow): a drenagem, no lugar da coleta menor, protege só o que as raízes veem; o jovem registrado sobrevive, o jovem que só as raízes veem é registrado, o resto morre sem varredura do heap e solta o que contava; os zeros cascateiam; o morto pelo RC volta ao alocador um a um (`soltar_morto`) | corpus 238/238 com auditoria, com `--gc-stress` e JIT × AOT 238/238 |
| E4 | feita: *trial deletion* dos candidatos (§8) na drenagem completa, ou em toda drenagem com `DARTFORGE_ARC_CICLOS=sempre`; efêmeros pelo ponto fixo do §22.3 quando há algum; fracos e finalizadores despachados na drenagem | grafos aleatórios com semente contra oráculo independente (3000 sementes × 3 modos, `arc_grafos_aleatorios_*`); corpus 238/238 com ciclos em toda drenagem e auditoria |
| E5 | parcial: exceções, async, eventos, isolados e JIT passam no corpus; a matriz `tracing/arc × pendência/tabelas × debug/otimizado` não está no harness (as combinações rodam por variável de ambiente) | corpus nativo 238/238 em Windows; Linux e macOS 238/238 no modo anterior (berçário), ainda não com o ARC puro |
| E6 | início: o filtro mínimo do §32 — objeto de corpo `BRUTO` (texto, caixa numérica, dados tipados) não tem aresta e não vira candidato a ciclo | — |

**Desvios em relação ao §19, deliberados e reversíveis.** A fila de
candidatos não é limitada; o reclamador solta bloco a bloco em vez de
reconstruir páginas (§19.4, "caminho otimizado posterior"). A tabela de
metadados já é a do §19.1 por página e índice de bloco (o `HashMap` ficou só
para os handles sem página); os de RC zero seguros por uma raiz ficam numa
lista própria, uma vez por objeto, e voltam à fila de zeros uma vez por coleta.

**Medida** (`bench/desempenho` em produção, 2026-10-08, preso a um núcleo P):
ARC/A0 = **2,81** de média geométrica antes das otimizações abaixo e **2,13**
depois (`json` 2,2–3,7×, `lista_ligada` 8,8×, `arvores` 15×; o resto em
0,8–1,1×). Os números de antes: Numérico, listas e chamadas ficam em
0,7–1,1; os que alocam objetos que escapam pagam a drenagem: `lista_ligada`
19,9×, `arvores` 17,2×, `json` 4–8×. No `objetos_escapam`, as drenagens tomam
~7 dos ~9 s: ~15 ms por drenagem de 65 mil jovens, ~230 ns por objeto, com cinco
consultas ao `HashMap` por jovem, o conjunto das raízes montado a cada drenagem e
a soltura física bloco a bloco.

**Feito em 2026-10-08, pela medida por fase** (`fases=`, `laco=`,
`zeros_vistos=`, `adiados=` no rastro): os adiados uma vez por coleta (no
`json`, 557 milhões de entradas da fila de zeros para 30 milhões); os
metadados por página e índice de bloco (§19.1); o descarte de um objeto sem
conjunto; as tabelas laterais sem o conjunto dos mortos.

**Próximos passos, na ordem.**
1. A decisão dos jovens e a cascata sem consultas repetidas (a marca de
   registrado no próprio bloco, um percurso do corpo por morte).
2. A soltura física em lote por página (§19.4).
3. Retain/release em linha no código gerado para o caso comum (§21), com o
   caminho lento no runtime.
4. E2: owners na HIR (§20), que tiram do caminho a maior parte dos
   retain/release das variáveis locais.
5. Linux e macOS com o ARC puro; a matriz do §26.3 no harness.

## 27. Seleção estática da política de memória

### 27.1 Uso das pesquisas e objetivo verificável

As duas conversas fornecidas pelo proprietário motivam esta extensão: fazer
o compilador escolher a estratégia de memória, inclusive para grafos cíclicos,
e reduzir o trabalho residual em execução. Elas são material de pesquisa, não
prova de resultados no DartForge. As decisões a seguir são requisitos deste
projeto; não atribuem a Swift, Nim ou ASAP suporte à semântica Dart.

| Evidência | Aplicação e limite |
| --- | --- |
| [Swift: Automatic Reference Counting](https://github.com/swiftlang/swift-book/blob/main/TSPL.docc/LanguageGuide/AutomaticReferenceCounting.md) | Referência para ownership e ciclos de capturas. `weak`/`unowned` são escolhas explícitas; não inferir que o Swift elimina ciclos fortes automaticamente. |
| [Nim: gerenciamento de memória](https://nim-lang.org/docs/mm.html) e [destrutores/cursor](https://nim-lang.org/docs/destructors.html) | ORC complementa RC com coleta; cursor inference inspira eliminação de cópias. O pragma cursor não é prova automática de segurança de uma referência de retorno. |
| Nim local, revisão da seção 17: `compiler/types.nim::canFormAcycle`, `compiler/ccgtypes.nim::genTypeInfoV2Impl`, `compiler/injectdestructors.nim::isCriticalLink`, `lib/system/orc.nim::rememberCycle` | Antecedentes concretos de classificação por tipo e filtragem de operações potencialmente cíclicas. Não transplantar o protocolo parcial de marcação sem provar a cobertura do coletor DartForge. |
| [Proust, ASAP, UCAM-CL-TR-908](https://www.cl.cam.ac.uk/techreports/UCAM-CL-TR-908.html) | Inspiração para inserir gerenciamento automático a partir de análise estática e especializar o trabalho dinâmico restante. Não é componente pronto nem garantia de desempenho para Dart. |
| [Chin, Craciun, Qin e Rinard, Region Inference for an Object-Oriented Language, PLDI 2004](https://scqin.github.io/papers/pldi04.pdf) | Precedente para inferir regiões e restrições de vida sem anotações do usuário, com polimorfismo. Nosso protocolo para Dart, observadores e ARC é especificado abaixo. |
| [Chang e colaboradores, Cyclic reference counting by typed reference fields, 2012](https://www.ece.iastate.edu/~morris/papers/12/RC_chang_12.pdf) | Inspiração para reduzir trabalho de coleta com informação de tipos/campos. Resultados no Jikes RVM não são estimativas de ganhos do DartForge. |

A meta é minimizar, por prova e medição, alocações com RC, candidaturas,
percursos e rodadas residuais. Não afirmar antecipadamente “70–90%”, “80%” ou
“sempre caminho frio”. Um único sítio desconhecido pode alocar a maioria dos
objetos. Diminuir chamadas ao coletor à custa de retenção ilimitada reprova a
otimização. Falta de prova seleciona a política geral, sem rejeitar Dart válido.
Isso se refere à otimização automática. Um contrato opcional solicitado pelo
usuário (§35) pode reprovar sua verificação, com diagnóstico próprio; ignorá-lo
não autoriza usar a propriedade que deixou de ser provada.

### 27.2 Dois grafos, unidades de vida e cobertura

Definir explicitamente:

- `G_sem`: referências fortes observáveis do programa, incluindo campos,
  capturas e arestas sintéticas do SDK/runtime; preserva a semântica Dart.
- `G_ret`: owners e arestas efetivamente contados, mais unidades de região/grupo
  e suas âncoras. Borrows certificados não acrescentam contagem.
- `G_cond`: weak, ephemerons e dependências de finalização, com seus contratos
  próprios; não converter tudo em arestas fortes nem ignorar esses mecanismos.

Cada objeto concreto tem uma unidade responsável por sua vida: ele próprio,
uma região ou um grupo. Eliminação de objeto só é válida quando sua identidade
e observações podem ser preservadas sem armazenamento. Deve valer, em todo
estado observável: se o programa consegue alcançar um objeto segundo `G_sem`
e o ponto fixo de `G_cond`, a unidade que o sustenta ainda está viva.

Todo ciclo concreto deve estar coberto por uma das alternativas: vida
delimitada comprovada; unidade coletiva com liberação correta; transformação
de retenção com prova independente; ou visibilidade ao coletor residual.
Essa obrigação inclui ciclos que cruzam políticas. SCC abstrata de tipos,
sítios ou funções não é uma lista de objetos concretos a liberar em conjunto.

### 27.3 Plano por alocação e por aresta

Criar os seguintes tipos **novos** em `otimizar/arc/analise/modelo.rs`, com a
parte necessária ao emissor em `hir.rs`. Campos são contratos de projeto:

```rust
struct SitioArc { origem: IdOrigem, copia: IdEspecializacao }
enum ArmazenamentoArc {
    Eliminado,
    Pilha { prova: IdProva },
    Regiao { regiao: IdRegiao, prova: IdProva },
    Grupo { grupo: IdGrupo, prova: IdProva },
    HeapRc,
}
enum ParticipacaoCiclica {
    NaoParticipa { prova: IdProva },
    Possivel { componente: IdComponente },
}
enum RetencaoDaAresta {
    Contada,
    InternaDaUnidade { unidade: IdUnidade },
    Emprestada { ancora: IdAncora, prova: IdProva },
    Fraca,
    Condicional,
}
struct PlanoMemoria {
    sitios: Mapa<SitioArc, PoliticaDoSitio>,
    arestas: Mapa<IdAresta, RetencaoDaAresta>,
    provas: Mapa<IdProva, CertificadoMemoria>,
    dependencias: DependenciasDoPlano,
}
```

`NaoParticipa` significa que a unidade não pode pertencer a ciclo contado;
**não significa que seus descendentes também sejam acíclicos**. Guardar
separadamente `nao_alcanca_ciclo`, quando houver prova dessa propriedade mais
forte. `Pilha` exige tamanho limitado e protocolo de representação; grafos de
tamanho variável usam arena no heap, sem arriscar overflow da pilha nativa.

Na alocação, gravar a política validada na metadata da instância/unidade.
Objetos da mesma classe criados em sítios/contextos distintos podem ter
políticas diferentes. Alias não muda política ao ser carregado. Chamada
genérica de release consulta metadata; variante especializada só dispensa
essa consulta quando todas as alternativas do valor têm a mesma política
e uma prova válida. Nunca confiar em tag fornecida pelo chamador que contradiga
o objeto real, nem marcar a classe inteira a partir de um único sítio. Todos
os aliases consultam o componente/versionamento da alocação real; a SCC de um
parâmetro ou contexto do chamador não substitui o identificador da instância.

Ordem de decisão: eliminação → pilha/região delimitada → grupo selado/borrow
com prova → heap RC sem candidatura → heap RC residual. Não é uma ordenação
cega de desempenho: rejeitar uma região/grupo quando seu custo de retenção
for pior, mesmo que elimine mais operações. Registrar a razão da escolha.

### 27.4 Pipeline obrigatório e validade dos fatos

```text
AST tipada + hierarquia + mundo alcançável
  → HIR com tipos semânticos, campos, sítios e capturas identificados
  → async explícito + poda segura + mem2reg/inlining/eliminação escalar
  → CFG com saídas normais e excepcionais
  → points-to e resumos de heap até ponto fixo
  → SCC de tipos/sítios + refinamentos de fluxo/forma
  → restrições de escape e vida → regiões/grupos/borrows
  → plano por sítio + certificados + verificador de política
  → inserção de ownership e operações de unidade
  → otimização ARC + verificador de ownership/política
  → materialização de exceções/raízes → LLVM + runtime compatível
```

IDs de sítio são estáveis através de clonagem/inlining: `ValueId` isolado
não é identidade persistente. Inlining remapeia a origem e o contexto da cópia.
Uma transformação que altera heap/CFG depois da prova deve atualizar a prova
por regra verificada ou invalidá-la e recalcular os afetados antes da emissão.
Com `DARTFORGE_OTIMIZAR_HIR=0`, manter ARC geral correto e seus verificadores;
desligar otimização nunca desliga a recuperação residual nem cria certificados.

## 28. Análise de referências e resumos interprocedurais

### 28.1 Domínio, identidade e finitude

Implementar análise de inclusão (*points-to*) em HIR, sensível a campos e a
contextos limitados. Nó abstrato: `(SitioArc, contexto, instancia_generica)`;
acrescentar nós para parâmetros, resultados externos, globais e unidades
sintéticas. Um nó de sítio em loop/recursão pode representar muitos objetos.
Registrar cardinalidade `Unico` ou `Muitos`; só singleton em cada estado
abstrato/ativação relevante, comprovado para todas as execuções, permite
atualização forte de seu campo. Observar uma instância em teste não é prova.
Nós simbólicos de parâmetro/resultado/global são placeholders do resumo:
instanciá-los com os points-to reais, preservando aliases. Não criar um objeto
independente para cada parâmetro e assim esconder um caminho de retorno.

Domínio mínimo:

```text
Pt(valor)                    = conjunto de nós possíveis
Campo(no, campo/layout)      = conjunto de destinos possíveis
Escape(no)                   = conjunto de causas/destinos de publicação
Observa(no)                  = weak | ephemeron | finalizer | ffi | identidade
Desconhecido(regiao_afetada)  = topo com possíveis leituras/escritas/retencoes
```

Null e valores triviais não criam nós. Chaves, valores, elementos, backing
stores, receptor de tearoff, ambiente, célula, argumentos RTI, frame async e
callback do SDK possuem posições distintas. Índice não constante de coleção
usa elemento resumido; partição por índice só é usada com limites provados.
Guardar origem de cada aresta para relatório e contraexemplo.

Começar com contexto de uma chamada para funções e receptor abstrato para
métodos; refinar seletivamente. Limites de contextos, nós, profundidade de
caminho de campo e estados de fluxo são determinísticos e versionados.
Ao exceder limite, unir contextos ou elevar ao topo. Conjunto truncado jamais
vira vazio; interromper a análise invalida as provas dependentes. A análise
termina por domínio finito/worklist e widening, não por supor convergência
de uma exploração sem limites.

### 28.2 Transferência de instruções

| HIR/operação | Restrição/efeito |
| --- | --- |
| Alocar | Adicionar nó fresco ao `Pt(resultado)`; reconhecer tanto variantes `Alloc*` quanto construtores `CallRuntime` usados pelo lowering. |
| Copy, movimento, cast de referência, `Phi` | Propagar união dos conjuntos; preservar as condições de caminho disponíveis. Cast de tipo não prova ausência de alias. |
| `SetField(obj, campo, valor)` | Para cada origem possível, incluir `Pt(valor)` no campo. Remover aresta antiga somente com atualização forte comprovada naquele estado. |
| `GetField` | Unir destinos dos campos de todas as origens possíveis. Leitura `dynamic` depende da resolução de chamada/getter; não assumir campo puro. |
| Coleção | Resumo da operação distingue elemento/chave/valor/backing store; add/set/rehash/cópia conservam todas as referências possíveis e seus aliases. |
| Global/retorno/throw | Marcar escape do valor e do alcance transitivo para o destino correspondente. Uma exceção contendo objeto local é escape. |
| Closure | Criar closure/ambiente/célula e arestas de captura reais. Armazenar closure publica transitivamente suas capturas. |
| `await`/retomada | Vida passa ao frame e às filas/continuations; resumir a operação gerada, não apenas a expressão Dart anterior ao lowering. |
| Weak/ephemeron/anexo/FFI | Registrar observador e publicação potencial, mesmo sem aresta ordinária forte. Promoção weak e callback podem produzir owners externos. |
| Chamada | Instanciar resumo com argumentos/contexto, incluir todos os alvos possíveis e aplicar efeitos normais e excepcionais. |

Exemplo: `conectar(a,b) { a.proximo=b; }` gera efeito parametrizado
`arg0.proximo ← arg1`. Duas chamadas `conectar(a,b); conectar(b,a)` precisam
criar o ciclo abstrato sem depender de inlining.

Uma chamada sem resumo de heap válido torna desconhecidos os objetos que
pode tocar, incluindo alcance transitivo e globais acessíveis. Se puder reentrar
em Dart, considerar os alvos de callback e suas publicações. Preservar fatos
de conjuntos comprovadamente separados; não contaminar todo o módulo sem
necessidade. Resumo ausente de **ownership da ABI** continua erro (§21.2);
resumo ausente apenas de **precisão de heap** seleciona ARC geral.

### 28.3 Formato de resumo e solução recursiva

Criar `ResumoHeapArc` separado do `Resumo` de alcance em `poda.rs` e dos três
efeitos atuais de `efeitos.tsv`. Deve conter:

- Resultado: `fresh`, alias de parâmetro/campo/global, união ou desconhecido;
  nulabilidade e observadores não se perdem na união.
- Leituras/escritas por caminho de campo, cópia/movimento e publicações em
  argumento, global, retorno, exceção, fila, handle ou estado async.
- Retenção além da chamada, execução/captura de callback, reentrada, suspensão,
  registro de weak/ephemeron/finalizador e vida do ponteiro interior.
- Regiões paramétricas e restrições `vida(destino) ≥ vida(origem)` das arestas
  não contadas; condições de unicidade/separação necessárias à especialização.
- Hash do corpo, esquema de campos, resumos de callees, dispatch, SDK/runtime,
  versão da análise e geração de recarga.

Construir SCC do **grafo de chamadas**, resolver resumos de cada SCC por
worklist monotônica e reaplicar chamadores dependentes quando o resumo crescer.
Usar a hierarquia/RTA para um superconjunto de alvos; points-to pode reduzi-lo
apenas com prova. Método ainda não analisado não é `noescape` por padrão.
SDK já compilado precisa exportar esses resumos ou ser tratado como opaco.

Especialização cria versões internas por política/região quando reduzir
trabalho estimado e respeitar limite de código. Resumos podem ser paramétricos
sem clonar todas as funções. Entradas públicas, dispatch aberto e FFI mantêm
a convenção geral ou um stub comprovado; uma declaração `borrow` na ABI
não significa, sozinha, que a função não retém uma cópia internamente.

## 29. Provar aciclicidade sem confundir tipos e instâncias

### 29.1 Filtro de tipos e SCC de sítios

Construir primeiro grafo de layouts/tipos concretos possíveis: `T→U` quando
um objeto T pode guardar referência forte para U. Incluir herança, genéricos,
tipos apagados, buffers, capturas, estruturas SDK e arestas laterais fortes.
`dynamic`/`Object` ou dispatch aberto amplia o conjunto compatível; não é nó
sem saída. `final` e `sealed` não provam aciclicidade do conteúdo.

Executar Tarjan sobre esse supergrafo. Tipo fora de SCC cíclica pode fornecer
prova barata de não participação. Tipo dentro de SCC continua elegível ao
refinamento por sítio/contexto; recursão nominal é possibilidade, não condenação
de todas as instâncias a um mesmo caminho de memória.

Sobre o grafo de sítios obtido no §28, SCC é cíclica se tiver mais de um nó ou
uma autoaresta. Um sítio fora dessas SCCs recebe `NaoParticipa` somente se o
grafo cobrir todas as mutações/entradas possíveis durante toda a vida de suas
instâncias. Fatos válidos apenas na saída do construtor não bastam. Prova pode
depender de contrato selado de módulo/sítio, que precisa acompanhar o objeto
até o descarte e ser invalidado antes de qualquer operação incompatível.

Tarjan é linear no tamanho do grafo fornecido; construção de points-to,
sensibilidade de contexto e análise de forma têm custo próprio. Reutilizar
algoritmo de SCC não equivale a reutilizar SCC de chamadas como SCC de objetos.

### 29.2 Precisão de fluxo e forma

Refinamentos obrigatórios para evitar que loops e branches simples caiam
sempre no residual:

1. **Estados disjuntos:** se `a.f=b` e `b.f=a` ocorrem em ramos mutuamente
   exclusivos, manter estados separados até o limite configurado. Certificado
   deve valer em todos os estados e em continuações futuras. Unir os estados
   pode produzir falso ciclo; isso perde otimização, sem comprometer segurança.
2. **Atualização forte:** permitida apenas em localização concreta única e
   estado sem interferência; um nó resumindo instâncias de um loop não autoriza
   apagar referências de todas elas porque um objeto foi sobrescrito.
3. **Forma indutiva:** reconhecer construções por nó fresco apontando para
   estrutura anterior, com ausência comprovada de backedge/mutação posterior.
   Guardar invariantes `fresh`, `disjunto`, `alcanca`, `aciclico(campos)` e uma
   ordem estrita de criação/região que diminui em cada aresta relevante.
4. **Checagem de laço:** demonstrar base, passo e preservação em todas as
   backedges, saídas e chamadas; se uma escrita ou resumo pode quebrar a ordem,
   perder o invariante. Não concluir aciclicidade após desenrolar só N iterações.

Exemplo alvo: uma lista construída repetidamente por `novo.next = cabeca;
cabeca = novo` pode ser acíclica mesmo com autoaresta abstrata no sítio `novo`.
Isso não autoriza `cabeca.next = cabeca` nem ligações arbitrárias recebidas
de fora. Se o mesmo objeto puder ter outros campos fortes que retornam ao
grafo, a prova de `next` sozinho é insuficiente.

Começar com domínio de forma pequeno e certificado verificável. `Desconhecido`
é resultado explícito quando a fórmula não pertence ao domínio; aumentar
precisão direcionado pelos relatórios de sítios com maior custo, sem converter
hipóteses de profiling em fatos semânticos.

### 29.3 Filtro de candidato versus poda do percurso

Considere `A↔B → D → E↔F`: D não pertence a ciclo, mas transporta alcance e
contagens entre dois grupos cíclicos. D pode omitir candidatura no seu release
positivo; D e suas saídas precisam continuar no percurso completo do trial.
Um nó acíclico também pode morrer como consequência de descartar um ciclo.

Manter inicialmente o visitante completo do §22.1. Uma especialização de
percurso pode cortar uma aresta apenas com certificado de ausência de caminho
de retorno ao componente analisado no grafo de retenção/unidades resultante
da seleção de políticas. Recalcular prova anterior que não cubra grupos,
regiões ou borrows introduzidos. Nesse caso, o destino permanece fora do
trial; se a origem morrer, o release normal dessa saída agenda o que for
necessário no componente seguinte. Se a origem sobreviver, a aresta continua
contada. Registrar destinos de fronteira e provar progresso das coletas
subsequentes; não subtrair RC de nó não visitado e esquecer a fronteira.

Descritores terão visitantes distintos: semântico completo, retenção contada,
arestas internas de unidade e opcional percurso especializado. A máscara de
percurso não é a máscara de descarte. Sem certificado válido, usar visitante
completo. Não inferir máscaras a partir de zero coletas observadas em benchmark.

## 30. Regiões inferidas para grafos confinados

### 30.1 Prova e escolha do limite de vida

Região é unidade dinâmica de armazenamento cuja duração o compilador prova;
pode conter quantidade variável de objetos e ciclos. O código Dart continua
criando e usando referências normais. Não exige sintaxe nova nem anotação
obrigatória; as preferências opcionais do §35 usam metadata Dart existente.

Para um conjunto de sítios, resolver restrições de duração: toda aresta não
contada `u→v` exige `vida(v) ≥ vida(u)` enquanto a aresta estiver válida. Ciclo
dessas restrições exige duração conjunta, não liberação arbitrária de um nó.
Escolher o menor domínio de controle que contenha construção e usos, e cujas
saídas normais/excepcionais possam encerrar a unidade sem referência pendente.
Postdominância isolada não basta: caminhos sem término, loops e suspensão têm
de aparecer na prova.

Certificado `RegiaoConfinada` contém:

1. Sítios/membros incluídos, limite de entrada e todas as saídas, caminhos de
   falha de construção e obrigações de `finally`/`Finalizable`.
2. Ausência de publicação persistente em global, retorno, throw, campo externo,
   fila, captura escapada, mensagem ou handle FFI. Parâmetro emprestado em
   chamada síncrona é admissível somente com resumo que preserve a região.
3. Ausência de observação weak/ephemeron/finalizer ou ponteiro interior capaz
   de gerar acesso fora dessa duração, inclusive por reentrada. Alias interno
   e `identical` podem continuar válidos se os objetos mantiverem identidade.
4. Lista de arestas para fora e contrato de retenção de seus destinos; política
   de recursos externos; prova de que código de limpeza não publica membro.
5. Restrição de retenção: região por iteração quando não há alias entre
   iterações; limite de tamanho/duração, ou demonstração de que os membros
   permanecem necessários até o final. Não acumular todos os temporários de
   um servidor em uma arena que só termina junto com o processo.

Na primeira transformação regional, observadores especiais, `Finalizable`,
callbacks opacos e FFI com exposição de endereço selecionam HeapRc para os
objetos afetados. Isso preserva suporte à linguagem, apenas perde otimização.
Uma versão regional que os aceite exige certificado específico de identidade,
promoção e agendamento; chamar finalizador Dart em `EndRegion` é sempre inválido.
Região lexical de pilha não atravessa `await`. Vida persistente precisa de
owner no frame ou grupo do §31 e de análise das continuações/encerramento.

### 30.2 HIR e representação no runtime

Instruções **novas**: `ArcBeginRegion -> RegionId`,
`ArcAllocRegion { region, descritor, tamanho } -> Ref`,
`ArcEndRegion { region }`. Um token de região é criado uma vez por ativação,
consumido uma vez em cada caminho de saída e domina todas as referências
regionais. Borrows de membro dependem desse token; não existe owner individual
que possa ser transferido a armazenamento persistente sem mudar a política.

APIs internas **novas** em `runtime/src/arc_regioes.rs`:

| Operação | Contrato |
| --- | --- |
| `arc_criar_regiao(prova, politica) -> IdRegiao` | Cria unidade ativa, registro de membros e raiz da unidade; pode usar reserva sob demanda. |
| `arc_alocar_em_regiao(id, descritor, tamanho) -> Ref` | Registra bloco zerado e identidade antes de retornar; pertence à unidade e não tem RC individual. |
| `arc_gravar_regional(id, slot, valor)` | Interna: preserva referência sem RC individual. Para fora: retém/substitui owner de saída e atualiza inventário. |
| `arc_liberar_regiao(id)` | Consome token, invalida observações admitidas, solta saídas contadas e libera membros uma vez; não executa Dart dentro da transação. |

Chamadas especializadas podem receber token/região implícito e retornar alias
dependente dessa região. Registrar isso no resumo e no verificador: resultado
regional não ganha vida independente por usar fisicamente o mesmo `i64` de
um resultado owned. A convenção genérica do §6 só pode ser usada quando o
contrato de chamada estiver adaptado sem exportar o membro; callee opaco ou
stub capaz de guardar owner persistente impede essa regionalização. Um no-op
em `retain` de referência regional não é prova de que a chamada seja segura.

Integrar ao espaço existente: manter cabeçalho, `Ref`, endereço e classificação
por página válidos para `bloco_de`, leitura de campos e identidade. A primeira
implementação pode alocar blocos no `EspacoDeObjetos`, marcados por `IdRegiao`,
com lista de membros; reclamá-los em lote pelo §19.4. Arena em páginas
reservadas é otimização posterior, também registrada no espaço. Um `malloc`
externo sem integração com o mapa de páginas não é implementação de `Ref`.

Estender `MetaArc` com `gestao: Rc | Regional(IdRegiao) | Coletiva(IdGrupo)`
e tornar o contador válido apenas na variante RC. Região mantém metadados de
layout, identidade e geração; “sem RC” não significa “sem metadata”. Reservar
capacidade antes de publicar membro. Falha deixa a região com sua lista de
membros inicializados e é limpa pelas mesmas saídas excepcionais.

### 30.3 Referências de fronteira e coleta durante a região

Referência entre membros da mesma região não incrementa contador individual.
Referência região→objeto HeapRc ou grupo externo é contada normalmente, salvo
borrow com âncora independente. Substituição retém antes de soltar; fechamento
consome cada ocorrência externa uma vez. Manter o inventário em slots dos
membros com descritor ou em índice estável; não registrar cada store antigo
como uma nova saída permanente.

Região ativa é raiz lógica no coletor global. Enumerar seus membros/arestas
de saída e manter os destinos externos alcançáveis; a auditoria não soma de
novo uma saída já contada. Uma otimização pode visitar só o índice de saídas
se provar equivalência ao percurso dos membros. Não permitir que a coleta
recolha um destino externo vivo porque o owner está dentro da arena.

Referência persistente HeapRc→membro regional viola confinamento. Detecção
estática seleciona HeapRc antes de gerar código. Não resolver a violação com
retain no membro, pois ele não possui contador individual. Se forem geradas
versões especializadas/generalizadas, decidir a versão antes de começar a
construção; nunca copiar silenciosamente um grafo já observado para outra
arena, alterando identidade.

Região aninhada pode apontar à região externa, pois a externa sobrevive à
interna. Referência inversa exige provar limpeza antes do fim da interna;
sem isso, unir durações ou selecionar ARC. Se união introduzir retenção
excessiva, ARC é a escolha de custo, não uma falha de compilação.

### 30.4 Saídas e exemplo de código gerado

```text
reg = ArcBeginRegion
try interno do compilador:
    lista/nós = ArcAllocRegion(reg, ...)  // N pode depender da entrada
    conectar os membros, inclusive fechando um anel
    resultado = calcular um escalar
    executar finally Dart ainda dentro da vida de reg
cleanup em todas as saídas:
    ArcEndRegion(reg)
retornar resultado ou propagar exceção original
```

O cleanup interno não é `finally` adicional observável no Dart. Em unwind,
usar o protocolo do §20.4; em pendência, os blocos de erro; em construção
parcial, apenas membros registrados. Exceção que contém um membro impede
esta regionalização. `break`/`continue` de região por iteração encerram a
unidade correta; recursão cria tokens distintos por ativação.

Fechamento pode percorrer a lista conhecida de membros e suas saídas:
elimina a busca de alcançabilidade/ciclos, mas não promete custo O(1) para
liberar buffers ou decrementar destinos externos. Medir separadamente esse
trabalho. Se a função continua produzindo lixo antes de terminar, subdividir
regiões em pontos certificados ou usar ARC; apenas transferir a pausa para
o fim da função não satisfaz a meta de memória.

## 31. Empréstimos estruturais e grupos que escapam

### 31.1 Prova de âncora independente

Um campo Dart continua semanticamente forte mesmo quando sua ocorrência RC
é eliminada. Para trocar `u→v` contado por borrow, provar, em todo estado
observável enquanto o slot existir:

```text
alcançavel_semanticamente(raízes, u) e campo_semantico(u, v)
    implica vivo_por_retencao_independente(raízes, v)
```

O caminho que sustenta v não pode depender da mesma ocorrência eliminada.
Dependências entre provas de âncora devem formar DAG terminado em owner
contado, token de região ou grupo válido. Justificar A por B e B por A não
cria owner. Provar somente ausência de um próximo `load` não basta quando
weak/finalização/reentrada podem observar a vida do objeto.

No padrão `pai→filho→pai`, a aresta de retorno pode ser borrowed se toda
exposição do filho estiver limitada por owner independente do pai. Retornar
apenas o filho, capturá-lo em callback, guardá-lo no frame async ou promover
weak do filho pode quebrar a condição. A análise deve provar ausência desses
caminhos ou instalar uma unidade coletiva que os cubra. Reconhecer o nome
`pai`, formato de árvore, `final` ou monomorfismo não constitui prova.

### 31.2 Emissão e mutação

Adicionar `ArcStoreBorrowed { slot, value, ancora, prova }` à HIR e uma marca
de aresta no descritor da instância/especialização. O store continua atualizando
payload, tag semântica e informação para auditoria. Ele não vira store escalar
sem metadata. O visitante de retenção não subtrai essa aresta de RC; o visitante
semântico a conserva para conferir alcançabilidade.

Todos os escritores/leitores do slot devem concordar com sua política. A
primeira versão só especializa slots com política uniforme por instância; se
um slot alterna entre counted/borrowed, exigir discriminante e transição
transacional que retém o novo owner antes de remover a âncora antiga. Não
reutilizar bitmap forte do tracing como prova de contribuição RC.

Uma leitura borrowed dura até a âncora; produzir um resultado owned externo
exige retain válido no destino, transferência do token apropriado ou prova de
que continua confined. Reentrada, throw e limpeza devem manter a âncora até
o último estado observável do borrow. Atalhos que violam esse requisito
voltam ao campo contado. `WeakReference` não é o código gerado desse passe.

### 31.3 Grupo selado como unidade de ownership

Para um grafo que escapa, pode-se escolher `GrupoSelado` quando houver prova
de composição delimitada e vida conjunta aceitável. Cada objeto mantém seu
handle/identidade e aponta para uma metadata de grupo. Contador do grupo:

```text
rc_grupo(G) = owners externos para qualquer membro de G
            + ocorrências fortes de outras unidades para membros de G
```

Arestas internas preservam `G_sem` mas não incrementam `rc_grupo`. Qualquer
alias externo para A, B ou outro membro mantém o grupo. Copiar tal alias retém
o grupo; mover transfere; soltar consome. `arc_reter/soltar` genéricos resolvem
a unidade através de `gestao`. Empréstimo de membro tem duração do owner do
grupo; não exigir que o programa guarde uma “raiz principal”.

APIs **novas** em `arc_grupos.rs`: `arc_grupo_iniciar`,
`arc_grupo_alocar`, `arc_grupo_selar`, `arc_grupo_reter`,
`arc_grupo_soltar`. O owner de construção sustenta o grupo até publicação;
ao selar, congelar membership e validar política de campos/saídas. Falha
descarta apenas membros construídos **desde que nenhum tenha sido publicado**.
O certificado deve provar ausência de escape/reentrada observável durante
a construção até o commit de selagem. Se um construtor puder publicar `this`
e depois lançar, selecionar HeapRc desde o início; não destruir o objeto que
ficou acessível num global. Um protocolo futuro de construção publicada teria
de contar owners externos e, na falha, soltar apenas o owner de construção.
Não adicionar membro arbitrário após
selagem; crescimento mutável exige outro certificado ou ARC geral.

Campos entre membros podem mudar para membros existentes se o certificado
cobrir as mutações e a retenção conjunta. Saídas para objetos externos são
contadas por ocorrência. Descarte do grupo invalida registros, solta saídas,
libera recursos nativos segundo seus contratos e reclama os membros; não
executa `dispose`, `close` ou callback Dart como destrutor.

### 31.4 Ciclos entre grupos e retenção excessiva

Construir grafo quociente de **unidades**: objetos HeapRc individuais,
grupos selados e raízes de regiões ativas. Uma aresta G→x→G ou G1→G2→G1
ainda é ciclo. Grupo só recebe `NaoParticipa` se o quociente inteiro relevante
provar ausência de retorno; caso contrário, participa do trial como nó com
contador e descritor de saídas. O coletor não mistura subtrações individuais
dos membros com o contador coletivo. Nas raízes, contar cada owner externo
uma vez, independentemente de quantos membros possa alcançar por campos.

Na coleta global/condicional, distinguir `L_sem` do oráculo por membro de
`L_mem`, a retenção efetiva do plano. Construir `L_mem` a partir de raízes
reais, imagens e regiões ativas. Sempre que alcançar qualquer membro de
grupo, acrescentar todos os seus membros e saídas contadas; propagar fortes
e âncoras válidas até estabilizar, intercalando o ponto fixo de ephemerons.
O grupo não marcado não se torna raiz só porque `rc_grupo>0`: G→x→G sem
entrada externa continua coletável.

O conjunto de descarte usa unidades inteiras fora de `L_mem`. Não liberar
isoladamente membro ou destino contado de grupo vivo que esteja fora de
`L_sem`; isso deixaria slot pendente e causaria um segundo débito/free no
encerramento do grupo. Usar `L_mem` de forma coerente para conservar/limpar
weak, ativar ephemerons e decidir morte/finalização, impedindo que promoção
weak devolva uma chave cujo Expando foi apagado por outra noção de vida.
A diferença `L_mem - L_sem` é retenção adicional da política, entra no modelo
de custo e precisa permanecer dentro da admissibilidade semântica do certificado.
Não usar esse fechamento para transformar proteção condicional em raiz.

Não fundir toda SCC **abstrata** em grupo: ela pode representar número
ilimitado de instâncias com vidas independentes. Usar um evento concreto de
construção e relação de membership verificável. Não migrar objetos ordinários
já publicados para grupo apenas por terem aparecido numa mesma SCC de tipos.

Weak/ephemeron/finalizadores, ponteiros nativos para membros e recarga sem
contrato estável impedem inicialmente esta especialização, mantendo HeapRc.
Suporte especializado futuro precisará manter observação **por membro**, não
transformar todos em um único alvo fraco indistinto. Ausência de observadores
não elimina a obrigação de limitar retenção: se um membro duradouro conserva
uma quantidade crescente de membros sem uso, recusar agrupamento, reduzir
seu domínio ou manter coleta individual. Relatar custo previsto/medido.

Um grupo com número fixo pequeno de membros pode ser útil mesmo mantendo
alguns vivos juntos por mais tempo. Tornar explícito o limite usado para
aceitar esse custo; nenhum percentual universal de economia justifica
ignorar pico de memória ou duração de retenção.

## 32. Runtime residual especializado e eliminação por programa

### 32.1 Decisão de release e cobertura dos ciclos

Alterar o caminho de release do §19.3 para respeitar a unidade selecionada:

```text
unidade = resolver_gestao(handle)
se alias depende de região: validar obrigação; nenhum RC individual
se grupo: operar contador do grupo
se HeapRc: operar contador do objeto
se contador chegou a zero: fila de zeros, respeitando proteção condicional
senão se politica == NaoParticipa: terminar sem candidatura
senão: inserir candidato uma vez segundo o protocolo abaixo
```

Para `Possivel`, o protocolo de referência continua candidatar em todo
release positivo. O trabalho estático deve reduzir o conjunto que chega a
essa política. Ao remover a última entrada externa de um ciclo residual,
deve existir pelo menos um candidato válido que permita alcançar o componente.
Filas com identidade/geração e reencaminhamento após budget/OOM preservam
essa obrigação. Redução do número de candidatos sem essa prova é vazamento.

### 32.2 Filtro por escritas potencialmente cíclicas

Otimização adicional, com protocolo próprio em vez de copiar parcialmente o
`maybeCycle` de outra linguagem: usar bit **monotônico por componente abstrato**
da imagem/isolate, `houve_escrita_ciclica`. Esse bit não é marca de liveness.

1. Componente pode começar limpo somente se todos os construtores, stores,
   operações SDK, cópias, importações e publicações estiverem instrumentados e
   não houver objetos preexistentes de proveniência desconhecida.
2. Todo store que possa criar aresta interna a uma SCC potencialmente cíclica
   seta o bit **antes** de publicar a aresta. Inicialização conta como store;
   reconhecer todas as especializações LLVM e campos sintéticos.
3. Com bit limpo, release positivo desse componente dispensa candidatura:
   nenhuma execução de aresta capaz de formar um ciclo ocorreu ali.
4. Com bit sujo, **todos** os objetos/unidades `Possivel` do componente usam
   o protocolo ordinário, não apenas o alvo do store. Não limpar o bit depois
   de uma coleta parcial ou porque um candidato sobreviveu.
5. Fronteira desconhecida, módulo novo ou objeto existente sem prova começa
   em componente sujo; invalidação ativa a política geral antes da publicação.

Instâncias/aliases usam ID de componente e versão comuns da imagem. Instrumentar
também clones, desserialização, tabelas laterais, migração e materialização;
imagem sem histórico certificado começa suja. A marcação é infalível e não
aloca. Se recarga unir componentes antes separados, marcar os componentes
antigos/novos afetados antes de qualquer escritor novo operar. Não eliminar
candidatos pendentes por causa da mudança do filtro.

Essa versão sacrifica precisão depois da primeira escrita crítica, mas tem
uma justificativa verificável: todo ciclo real projeta um ciclo abstrato, cuja
construção executa ao menos uma aresta instrumentada, e daí em diante a última
entrada externa perdida aciona o protocolo geral em qualquer membro. Uma
reentrada capaz de coletar não pode ocorrer entre a publicação e a marcação.

Não usar a regra insegura “marcar só o alvo do store e esperar release nele”:
a última raiz pode desaparecer por outro membro depois de uma rodada que
considerou aquele alvo vivo. Reset de bit e filtragem por instância exigem
novo protocolo de cobertura; ficam desabilitados até haver prova e testes.
Medir se consultar/escrever o bit custa menos que a fila evitada. Pode-se
eliminar estaticamente consultas em caminhos já dominados por bit sujo.

### 32.3 Execução sem coletor ordinário de ciclos

Gerar certificado **da imagem inteira**, `SemCiclosResiduais`, somente se:

- Toda alocação alcançável de programa, SDK, runtime, stubs, callbacks e
  inicializadores tiver política comprovada; código opaco não conta como vazio.
- Todas as unidades e suas relações de fronteira forem livres de ciclos
  residuais; borrows/grupos/regiões tiverem certificados válidos e cobertura
  de todas as saídas. Coleta explícita continua com contrato coerente.
- A imagem for fechada para introdução de código/mutações incompatíveis, ou
  houver protocolo que instale capacidade residual antes de admitir a extensão.
- O certificado for verificado na ligação/registro e entrar no hash da ABI,
  do plano de memória, SDK e artefatos de cache.

Nesse caso, remover filas/trial ordinário e suas chamadas alcançáveis na
ligação. Isso não remove automaticamente ephemerons, invalidação weak,
finalização, reclamador físico, contadores ou safepoints exigidos por outras
funções. Emitir também capacidades independentes, como
`precisa_alcance_condicional`; `Expando` pode exigir trabalho dinâmico mesmo
com zero ciclos de referências ordinárias.

O comando de compilação pode expor **nova opção proposta**
`--exigir-sem-coletor-de-ciclos`: ela verifica o certificado e falha explicando
as obrigações não provadas; nunca força desativação insegura. A seleção normal
é automática e compila com residual quando necessário. Não confundir essa
opção com `--memoria arc` nem prometer um programa sem gerenciamento dinâmico.

### 32.4 Invalidação e transição antes da publicação

Provas dependem de corpo de função, alvos de dispatch, campos, resumos e
políticas, além da época de layout. Recarregar somente um método pode
introduzir o primeiro backedge sem mudar layout. Invalidar consumidores
transitivos antes de `vivo::publicar`/instalar geração.

Para instância HeapRc `NaoParticipa`, permitir transição monotônica para
`Possivel` em safepoint: atualizar a metadata e tornar candidatos os objetos
afetados com RC positivo, preservando os campos/contadores existentes; instalar
escritores/leitores compatíveis e só depois publicar a nova execução.

Regiões, borrows persistentes e grupos precisam de mais que um bit. Enquanto
não houver protocolo de materialização completo, não aplicar essa especialização
a código/objetos expostos a recarga incompatível: usar ARC geral na sessão
editável ou manter contratos antigos para objetos antigos e recusar só a
recarga que os viole. Não negar suporte a Dart válido fora dessa combinação.
Frames antigos em execução, layouts antigos e resumos usados por stubs também
fazem parte da checagem; recompilar só os chamadores novos não basta.

Se implementada, materialização deve reservar metadata/contadores, inventariar
aliases de fronteira e arestas internas, reconstruir contagem sem mover handles,
atualizar observadores e caminhos genéricos, e fazer um único commit antes
do store/retorno/handle que escaparia. Falha deixa a versão anterior íntegra.
Até esses passos serem implementados e verificados, não gerar uma guarda que
prometa converter objetos depois de publicá-los.

## 33. Onde implementar a análise e como verificar seus certificados

### 33.1 Infraestrutura existente: capacidades e limites

Este mapa foi conferido no código do repositório. Nenhum dos passes abaixo
deve ser apresentado como análise de regiões/points-to já implementada:

| Ponto existente | Trabalho concreto |
| --- | --- |
| `crates/mundo/src/lib.rs::{Hierarquia,Raizes,Mundo,calcular}` e `emit_native/src/mundo_nativo.rs::calcular` | Usar classes instanciadas, hierarquia e seletores vivos como superconjunto. RTA determina alcance de código/tipos; não determina aliases de instâncias. |
| `emit_native/src/context.rs::Context` e `lower/membros.rs::{layout,tipo_da_variavel}` | Exportar esquema estruturado de campos com IDs, tipos declarados e classes possíveis antes do apagamento para `Type::Ref`. Não interpretar `CampoDoLayout::tipo`, que é texto para migração, como domínio de tipos. |
| `lower/membros.rs::FnBuilder::{indice_campo,repr_do_campo,gravar_campo,ler_campo}` | Associar `IdCampo` e origem aos índices físicos, incluindo herança/mixins. Registrar stores gerados e seus efeitos. |
| `hir.rs::{ClassDef,Function,Module,Instruction}` | Acrescentar sítios, plano, provas, resumo de heap, políticas de aresta e operações de região/grupo/borrow. Preservar tipos/forma de coleções. |
| `otimizar/mod.rs::otimizar` | Integrar planejamento depois de transformações normais e antes de ARC; não perder metadados em limpeza/inlining. Centralizar programa e SDK na mesma entrada. |
| `otimizar/escape.rs::substituir_objetos` | Preservar a substituição escalar existente. Hoje reconhece objetos com acessos simples por índice constante; argumento, retorno, phi ou identidade impedem essa transformação. Acrescentar análise interprocedural separada, sem tratar o passe atual como prova de região. |
| `otimizar/inline.rs::{copiavel,inlining}` | Remapear origem/contexto ao clonar e aplicar orçamento de especializações; invalidar resumos dependentes do corpo. |
| `otimizar/efeitos.rs::{instrucao_lanca,nao_lancam,em_ciclo}` | Separar efeitos de heap dos efeitos de exceção existentes. Extrair Tarjan iterativo de `em_ciclo` como utilitário reutilizável, preservando testes; seus resultados atuais são SCCs de chamadas. |
| `lower/captura.rs::{analisar,analisar_com,livres}` | Alimentar o grafo com capturas/células; ausência de resolução vira desconhecido, nunca ausência de captura. |
| `lower/funcoes_diretas.rs::{Passagem,Captura,Direta}` e `FnBuilder::{declarar_funcao_direta,chamar_direta}` | Reaproveitar a eliminação existente de closures que só têm chamadas diretas; expandir com contratos de não retenção verificados. |
| `lower/closures.rs::{preparar_capturas,lower_closure,tearoff_de_metodo,tearoff_instanciado}` | Registrar todos os objetos sintéticos, receptor, ambiente, células e tipos capturados. |
| `lower/async_sm.rs::{lower_corpo_async,fechar_corpo,funcao_novo_quadro,guardar_vivos,converter_allocas}` | Atualizar fatos após criação do estado suspenso. Owner no frame é escape da função síncrona original; continuations participam do grafo. |
| `poda.rs::{Resumo,resumir,resumo_da_hir,podar_hir,ler_resumos}` | Resumos atuais são de alcance. Adicionar seção versionada separada para heap ou arquivo próprio, sem inferir noescape a partir de símbolo podado. |
| `sdk_modulo.rs::{BibliotecaDoSdk,emitir_bibliotecas_do_sdk,chave_do_sdk}` | Publicar resumos de heap junto ao SDK, versões e hashes; consumir antes de tornar o SDK opaco em LLVM. |
| `llvm/mod.rs`, `llvm/listas_ir.rs`, `llvm/raizes.rs` | Consumir plano validado, emitir gestão de unidade, raiz regional e filtros; conferir todos os atalhos de store/alocação. |
| `runtime/src/{arc,arc_descritores,arc_ciclos,arc_condicionais}.rs` propostos no §18 | Estender metadata/unidades, visitantes distintos, auditoria e coleta; manter ephemerons independente de fila de ciclos ordinários. |
| `runtime/src/{espaco,heap}.rs` | Registrar regiões/grupos no alocador, preservar identidade e contabilização; `Heap::migrar_instancias` e `epoca_de_layout` participam da invalidação. |
| `jit/src/reload.rs::{hot_reload,install_generation,check_contract}`, `migracao.rs::{layouts_do_ir,planejar}`, `vivo.rs::publicar` | Conferir dependências de prova antes de publicar geração; métodos são associados aos tipos correspondentes, como `JitSession`. Mudança de corpo também invalida. |
| `cli/src/nativo.rs`, `emit_native/src/{cache_objeto,cache,driver}.rs`, `emit_native/build.rs` | Expor relatório/modos de comparação, incluir plano e capacidades no cache/ABI, selecionar artefato com ou sem coletor apenas sob certificado global. |

Nesta tabela, `lower/`, `otimizar/`, `llvm/`, `hir.rs`, `poda.rs` e
`sdk_modulo.rs` são relativos a `crates/emit_native/src/`; `emit_native/`,
`runtime/`, `jit/` e `cli/` são relativos a `crates/`. A poda AOT
não equivale a mundo fechado eterno no JIT: futuras gerações podem introduzir
seletores. Provas editáveis precisam de dependências e invalidação próprias.

### 33.2 Arquivos novos e responsabilidades

Sob `crates/emit_native/src/otimizar/arc/analise/`, criar:

| Arquivo novo | Responsabilidade e saída |
| --- | --- |
| `mod.rs` | `planejar_memoria(module, configuracao) -> PlanoMemoria`; coordena dependências e budgets. |
| `modelo.rs` | IDs, domínios, placeholders, fatos, políticas, provas e motivos de desconhecimento. |
| `tipos.rs` | Esquema de tipos/layouts/arestas sintéticas e filtro nominal de ciclos. |
| `points_to.rs` | Worklist de restrições de inclusão, campos, contextos, cardinalidade e topo. |
| `resumos.rs` | Resumos parametrizados, SCC recursivas, instanciação, leitura/gravação e invalidação. |
| `forma.rs` | Estados de fluxo e invariantes indutivos de aciclicidade/separação em loops. |
| `escape.rs` | Publicações fortes e observáveis, duração de chamadas/frames e fronteiras. |
| `regioes.rs` | Restrições de vida, seleção de limites, custo de retenção e saída `RegiaoConfinada`. |
| `emprestimos.rs` | Provas de âncora, DAG de dependências e política por slot. |
| `grupos.rs` | Membership, selagem, contabilidade coletiva e grafo quociente. |
| `certificados.rs` | `verificar_plano(module, plano)` independente da heurística de seleção. |
| `relatorio.rs` | Explicações por sítio/aresta e dados para comparar com perfil de execução. |

Adicionar `crates/emit_native/src/otimizar/scc.rs` para o algoritmo comum,
sem associá-lo a um grafo específico. No runtime, criar `arc_regioes.rs` e
`arc_grupos.rs`, registrá-los em `MODULOS`, `lib.rs` e `RUNTIME_MAIN` conforme
§18; externs públicas correspondentes entram no fragmento `arc_abi.rs`, na
tabela de símbolos e em `ownership.tsv`/`efeitos.tsv` com os efeitos reais.

### 33.3 Conteúdo e checagem do certificado

`CertificadoMemoria` deve identificar hipótese, conclusão, sítios/arestas,
invariantes de loop, âncoras, saídas de cleanup e dependências versionadas.
O verificador deve reconstruir e conferir obrigações, não apenas aceitar
`seguro=true` produzido pelo mesmo passe. Reusar utilitários de CFG é válido;
heurística que escolhe região e lógica que confere sua vida são separadas.

Obrigações mínimas:

- **Acíclico:** supergrafo fechado ou invariante indutivo; nenhuma mutação
  possível omitida; validade durante toda a vida e nos caminhos de erro.
- **Região:** dominância da entrada, consumo único nas saídas, nenhuma
  referência escapada após cleanup e todas as saídas externas contabilizadas.
- **Borrow:** âncora independente viva em todo estado observável, sem ciclo
  de justificativas e com escritores/leitores compatíveis.
- **Grupo:** cada membro pertence a uma única unidade, todos os owners
  externos redirecionam ao grupo e todas as saídas entram no quociente.
- **Poda/filtro:** cobertura de qualquer ciclo residual e das fronteiras,
  inclusive after lowering, clones, SDK e writers em linha.

Uma prova rejeitada antes da transformação permite refazer os sítios afetados
em ARC geral. Uma contradição descoberta depois de consumir tokens, remover
stores ou emitir código é erro interno: reconstruir a HIR a partir de estágio
íntegro ou abortar a compilação, nunca continuar com IR parcialmente convertido.
Adicionar diagnósticos `ARC009` certificado inválido, `ARC010` escape regional,
`ARC011` âncora circular/expirada, `ARC012` unidade/fronteira sem contabilidade
e `ARC013` capacidade global sem prova. “Não consegui otimizar” aparece como
motivo no relatório, não como erro do programa nem recomendação de usar weak.

### 33.4 Relatório e modos de comparação

Nova opção proposta `--relatorio-arc <arquivo.json>` emite dados versionados:

```json
{
  "versao": 1,
  "sitio": "funcao:origem:especializacao",
  "armazenamento": "HeapRc",
  "candidatura": "Possivel",
  "motivo": "argumento publicado por callback sem resumo de heap",
  "dependencias": ["resumo:callback", "layout:No"],
  "prova": null,
  "refinamento_sugerido": "analisar efeitos da captura"
}
```

Produzir também contagem por política, arestas removidas, candidatos omitidos,
tempo/memória do analisador, budgets atingidos, especializações e tamanho de
código. IDs estáveis permitem juntar perfil dinâmico sem usar endereços como
identidade. Distinguir ciclo possível, ciclo comprovado em caminho e falta de
prova; nenhum deles sozinho é diagnóstico de vazamento em ARC com coletor.

Modo interno `--arc-analise referencia|completa` (novo) mantém, respectivamente,
ARC geral ou seleção de políticas. Ambos preservam toda a linguagem e o
verificador. Refinamentos podem ser desligados individualmente no harness para
atribuir ganhos a regiões, aciclicidade, borrows, grupos e filtro de stores.
Opções são configuração imutável e entram no cache, não variáveis de ambiente
relidas por worker. Não ocultar queda de precisão causada por budget.

## 34. Entregas e testes da redução estática de ciclos

### 34.1 Sequência de implementação

As entregas abaixo detalham E6; fatos de S0 devem ser preservados desde E2.
Execução de cada política depende do suporte de runtime/verificador de E3–E5.
É possível testar o analisador primeiro emitindo apenas relatórios/certificados
e manter ARC de referência até habilitar sua transformação correspondente.

| Entrega | Implementar | Critério de conclusão |
| --- | --- | --- |
| S0 — fatos e resumos | IDs estáveis, tipos/campos/capturas, domínio de heap e exportação SDK | Cada escritor/extern tem contrato ou desconhecido explícito; clones/async preservam origem. |
| S1 — grafo e poda de candidatos | Points-to por campo/contexto, SCC, certificado de `NaoParticipa` | Casos de múltiplas instâncias/aliases não ganham prova falsa; runtime omite candidatura certificada mantendo visitas/saídas corretas. |
| S2 — refinamento de fluxo/forma | Branches disjuntos, singleton e invariantes indutivos de listas | Lista de sítio único em loop pode ser provada; backedge arbitrário invalida a prova. Budget menor perde otimização sem perder correção. |
| S3 — regiões | Restrições de vida, HIR, alocador, saídas e custos | Anel local de tamanho variável usa região, sem RC interno/trial; throws/falha parcial/referências externas são corretos. |
| S4 — borrows estruturais | Âncoras, política por slot e visitantes | Caso confinado elimina retenção redundante; filho escapado e promoção weak mantêm política segura. |
| S5 — grupos | Construção/selagem, aliases para qualquer membro, quociente | Grupo fechado sem ciclo externo dispensa trial; ciclo grupo↔heap é recuperado pelo residual; retenção adicional é medida. |
| S6 — residual e imagem fechada | Filtro monotônico, poda certificada, capacidades de ligação | Última raiz em qualquer membro agenda coleta; imagem sem residual é provada incluindo SDK/runtime; condicionais preservados. |
| S7 — validação e custo | Diferencial, grafo aleatório, invalidação/reload, relatórios | Todos os contraexemplos abaixo passam; resultados brutos com ablação, custos de compilação, RSS e retenção publicados. |

### 34.2 Matriz de contraexemplos e resultados exigidos

Criar `crates/emit_native/tests/arc_estatico_ciclos.rs`,
`arc_regioes.rs`, `arc_certificados.rs` e testes correspondentes em
`crates/runtime/tests/arc_regioes.rs`, `arc_grupos.rs`, `arc_filtro_ciclos.rs`.
Os nomes são arquivos novos; aproveitar os harnesses existentes. Casos Dart
entram no corpus nativo e os de recarga nos testes da crate JIT.

| Caso | Prova/transformação esperada e verificação |
| --- | --- |
| Anel local com N definido pela entrada, resultado escalar | Região; zero retain/release entre membros e zero visita de trial desses membros. Resultado coincide com referência; bytes são recuperados na saída. |
| Mesmo anel retornado ou lançado dentro de exceção | Não usar região local; HeapRc/grupo certificado. Usos posteriores conservam identidade e vida. |
| Nó fresco aponta para cabeça anterior em loop | Invariante de lista prova ausência de ciclo mesmo com um único sítio. Variante `novo.next=novo` deve invalidá-lo. |
| Arestas opostas em ramos exclusivos | Refinamento pode provar acíclico; união com budget reduzido conserva residual. Nunca interpretar falso positivo como erro Dart. |
| Uma classe com sítio acíclico e sítio cíclico | Políticas distintas por instância; ambos tratados corretamente por chamada genérica de release. |
| Instâncias do mesmo sítio em recursão/loop | Atualizar uma não apaga arestas das demais no domínio; forte atualização requer singleton real. |
| `A↔B → D → E↔F` | D sem candidatura, mas visitado/contado; descartar ambos os ciclos e seus descendentes quando não houver raiz. |
| Pai/filho local sob âncora | Borrow permitido apenas com prova independente. Variante que retorna filho, guarda em callback ou frame mantém pai acessível. |
| Weak promovida em callback reentrante | Impedir regionalização/borrow incompatível ou ter protocolo certificado; objeto não morre no fim de um escopo que já deixou de ser dono exclusivo. |
| Região com saída para objeto ARC externo | RC da saída atualizado em sobrescrita e encerramento; coleta global durante a região não mata o destino. |
| `try/finally`, rethrow, erro no construtor e OOM de reserva | EndRegion/cleanup uma vez; nenhuma leitura após descarte; exceção/rastro original preservado. |
| Construtor publica `this` em global e depois lança | Impedir região/grupo de construção não publicada; referência global continua válida após o erro. |
| Grupo retornado por membro não principal | Owner mantém grupo inteiro; `identical` dos membros não muda; último owner libera saídas/membros uma vez. |
| Grupo→objeto externo→grupo | Quociente cíclico exige residual; não aplicar certificado baseado só nas arestas internas do grupo. |
| Grupo vivo com membro desconectado e saída ARC durante coleta de Expando | Fechamento mantém unidade/saídas; nenhum free individual ou débito duplicado; retenção adicional aparece no relatório. |
| Grupo mutável retendo membros desconectados em loop | Rejeitar especialização sem limite aceitável; relatório revela custo de retenção, não celebra zero trials. |
| Store crítico num membro; última raiz retirada de outro | Filtro suja componente inteiro e release posterior agenda candidato; repetir após coleta que encontrou o grupo vivo. |
| Autociclo em inicializador/cópia/desserialização/SDK | Instrumentação marca antes da publicação; caminho em linha não contorna filtro nem contagem. |
| Expando com valor→chave, portador morto e zero candidatos | Executar ponto fixo condicional mesmo sem trial ordinário; não classificar entrada como weak ou forte comum. |
| Finalizable, finalizador com captura de alvo, recursos externos | Mesmas garantias semânticas de referência; nenhuma ação Dart em EndRegion; externalSize contabilizado uma vez. |
| Chamada dynamic/FFI que conserva argumento | Resumo desconhecido impede prova afetada; corpo posteriormente conhecido pode permitir especialização, sem alterar fonte Dart. |
| Reload muda apenas corpo e adiciona backedge | Invalidar plano dependente antes da publicação; objetos antigos não conservam `NaoParticipa` inválido. |
| Certificado/cache SDK adulterado, obsoleto ou incompleto | Verificador/carga rejeita artefato e recompila corretamente; nunca presume ausência de ciclos. |
| Executável certificado sem trial e outro com uma alocação opaca | Primeiro não referencia símbolos do trial; segundo mantém residual ou falha somente sob `--exigir-sem-coletor-de-ciclos`. |

Comparar código gerado além da saída: teste que só confere o resultado não
prova que o coletor foi removido. Inspecionar plano/IR/símbolos e contadores de
execução. Para referências fracas/finalização, comparar garantias permitidas,
sem exigir cronologia idêntica à VM ou ao caminho de referência.

### 34.3 Oráculo, certificados falsos e propriedades metamórficas

Estender o gerador de grafos do §26 com unidades de região/grupo, borrows,
mudanças de política, fronteiras e registro de observadores. O oráculo usa
`G_sem` e ponto fixo condicional; a implementação usa `G_ret`. A propriedade
principal é nenhum objeto semanticamente alcançável perder sua unidade viva.
Após coleta completa, lixo elegível deve ser recuperado ou explicado por
retenção de unidade ainda viva dentro do limite aceito.

Injetar certificados inválidos: omitir campo, trocar geração de layout, apagar
cleanup, criar ciclo de âncoras, falsear singleton, remover alvo de chamada,
atribuir componente diferente a alias e omitir store sintético. O verificador
deve rejeitar cada caso antes de executar o artefato. Teste que apenas repete
a decisão do planejador não serve como oráculo independente.

Propriedades metamórficas: reduzir budget, apagar resumo de heap ou desligar
um refinamento pode aumentar o residual, mas preserva segurança/semântica;
inlining e devirtualização preservam aliases; inserir publicação externa
invalida a prova regional relevante; acrescentar uma escrita de retorno
invalida aciclicidade; regenerar IDs não reaproveita certificado de outro sítio.

### 34.4 Medidas e critério de aceitação do objetivo

Comparar, com mesmo programa/SDK/flags, tracing, ARC de referência, ARC com
análise completa e ablações de cada política. Relatório deve incluir:

- Alocações e bytes **executados** em cada política, não só quantidade de
  classes/sítios; objetos eliminados estimados separadamente dos realmente alocados.
- Retains/releases evitados, candidaturas antes/depois, consultas do filtro,
  escritas que o ativaram, componentes permanentemente sujos, visitas e tempo
  de trial, ponto fixo condicional e fechamento de regiões/grupos.
- Bytes de metadata/scratch, memória viva e residente, pico, fragmentação,
  bytes sem uso retidos pela unidade e duração dessa retenção. Instrumentação
  de oráculo para retenção deve ser reportada separadamente do benchmark normal.
- Tempo/memória de compilação por passe, budgets excedidos, resumos reutilizados,
  especializações, tamanho do binário e compatibilidade/invalidação de cache.
- Latência e throughput nos termos do §25, incluindo pausas de descarte em
  lote. Eliminar trial não pode ocultar um custo equivalente no fechamento.

Casos dirigidos como anel confinado e lista acíclica precisam demonstrar a
transformação prevista, com contadores coerentes. Para cargas gerais, registrar
ganhos e regressões por benchmark; selecionar especializações com modelo de
custo calibrado e manter a configuração de referência para investigação.
Não fixar promessa percentual antes dos dados. A especificação exige mecanismos
para reduzir o residual e provas para removê-lo onde possível; a quantidade
que sobra em cada aplicação é resultado mensurável da implementação.

## 35. Pacote Dart opcional de anotações para o compilador

### 35.1 Possibilidade, portabilidade e fronteira de responsabilidade

É possível distribuir as anotações como pacote Dart convencional no pub.dev.
Dart permite metadata por constante ou chamada a construtor `const`; a
interpretação especial deve ser implementada no DartForge. Instalar o pacote
sozinho não modifica a VM Dart, o compilador Flutter nem o gerenciamento de
memória deles. [Referência oficial de metadata](https://dart.dev/language/metadata).

Nome de trabalho: **`dartforge_annotations`**, proposto, sem pressupor que
esteja disponível ou publicado. O pacote contém declarações de metadata,
documentação e exemplos; não contém um coletor, não carrega DLL e não executa
código na entrada de cada função anotada. Se as constantes forem usadas como
valores normais pelo programa, seguem a semântica normal de constantes Dart.
Não prometer remoção de metadata que seja observável por mecanismos suportados.

Regras de compatibilidade:

1. Todas as análises 27–34 continuam automáticas sem pacote. Anotação orienta
   a busca ou pede verificação; não é necessária para recuperar ciclos.
2. Nenhuma anotação converte campo forte em weak, muda identidade, omite
   exceção ou permite use-after-free. Cada transformação precisa de prova.
3. Fonte anotada continua Dart válido em outras ferramentas, com a dependência
   resolvida. Essas ferramentas não fornecem automaticamente a certificação
   DartForge. Metadado desconhecido não altera, por si, a execução de um método.
4. Contratos são opt-in de compilação: o usuário pode exigir que uma função
   satisfaça uma propriedade. Um erro de contrato não redefine a linguagem
   nem significa que o programa seria inválido em Dart comum.
5. O nome de uma classe ou uma string escrita pelo usuário nunca é autoridade
   suficiente para descartar contagem, coletor ou checks de segurança.

### 35.2 API pública inicial

Definições propostas, em `lib/dartforge_annotations.dart`; classes são metadata
constante, sem lógica de runtime:

```dart
final class PreferRegion {
  const PreferRegion();
}

final class PreferAcyclic {
  const PreferAcyclic();
}

final class NoEscape {
  const NoEscape();
}

final class Realtime {
  const Realtime();
}

const int dartForgeAnnotationsSchema = 1;
```

| Anotação | Alvos suportados na versão 1 | Significado e comportamento sem prova |
| --- | --- | --- |
| `@PreferRegion()` | Função ou método síncrono com corpo | Prioriza inferência de regiões para alocações confinadas no corpo e especializações elegíveis. Sem prova/benefício, conserva a política geral e informa o motivo no relatório. |
| `@PreferAcyclic()` | Classe, construtor ou declaração de variável local | Prioriza prova de não participação em ciclos nos sítios/valores associados. Não classifica todas as subclasses nem o alcance transitivo por declaração. Sem prova, conserva o residual. |
| `@NoEscape()` | Parâmetro formal de função/método com corpo disponível ou resumo certificado | Solicita prova de não publicação além da chamada, conforme §35.3. Em verificação de contratos, ausência de prova é erro com caminho explicativo. |
| `@Realtime()` | Função ou método síncrono com corpo, sem `async`, `sync*` ou `async*` | Solicita o contrato transitivo do §36. Falha de prova impede certificar/compilar esse contrato no modo de verificação. |

Preferências não criam uma região que necessariamente contém tudo o que o
corpo aloca; objetos que escapam podem permanecer HeapRc. Na variável local,
`PreferAcyclic` prioriza os sítios presentes no seu points-to: a anotação não
persegue só o nome da variável após reatribuição. Na classe/construtor, considerar
subclasses, campos sintéticos, mutações futuras e escape; `final` não é prova.

Usar `@Target` de `package:meta/meta_meta.dart` para feedback de alvo quando
houver uma versão compatível com o SDK mínimo; o DartForge valida alvos
independentemente desse aviso. Não inventar sintaxe de anotação diretamente
num `for`, bloco arbitrário ou expressão `new`. Se necessário, extrair uma
função auxiliar Dart comum e anotá-la.

API v1 deliberadamente não oferece `@TrustMeAcyclic`, `@WeakBackEdge`,
`@DisableGc` ou `@AssumeRealtimeSafe`. Elas permitiriam confundir intenção com
garantia. Nova anotação só entra com semântica, alvos, prova, fallback e testes.

### 35.3 Contrato de não escape

`NoEscape` não transfere ownership nem promete ausência de alocação, mutação,
exceção ou bloqueio. Para o parâmetro anotado, verificar que a chamada não
cria publicação persistente dele ou de referências obtidas transitivamente
dele em retorno, exceção, global, campo externo, closure escapada, frame async,
fila, porta, weak promovível ou handle nativo. Registro de observador que
possa expor o objeto posteriormente também precisa ser considerado.

Uma variável global que já apontava ao objeto antes da chamada não viola
sozinha o contrato; a obrigação é não criar nova fuga atribuível à chamada
nem usar a anotação para apagar aliases preexistentes. Ler/escrever elementos
numéricos do buffer pode ser permitido. Guardar o parâmetro em outro objeto
recebido e restaurar depois só é aceito se a análise provar que nenhuma
reentrada/observação retém a referência; sem essa prova, rejeitar o contrato.

Resumo de `NoEscape` é emitido **após** provar o corpo e todas as chamadas
relevantes. Em recursão, resolver o SCC por ponto fixo; anotações mútuas não
provam umas às outras. Implementações/overrides alcançáveis precisam cumprir
a obrigação antes de um chamador polimórfico usá-la. Callback síncrono pode
ser aceito com contrato de retenção conhecido; callback opaco impede a prova.
Corpo FFI externo não recebe noescape verificado apenas por estar anotado.

### 35.4 Identidade, avaliação e políticas de verificação

Reconhecer metadata pelo elemento resolvido e biblioteca definidora canônica,
obtidos da resolução de pacotes/imports. A identidade v1 é a declaração
correspondente em `package:dartforge_annotations/dartforge_annotations.dart`
com esquema compatível; não comparar só o lexema `Realtime` nem texto da
declaração `library`. Prefixos, reexports e alias de constante devem resolver
ao mesmo elemento/valor. Uma classe homônima de outro pacote é metadata comum.
Binding ambíguo não pode ser reconhecido como contrato: preservar o diagnóstico
de resolução. Em alias `const`, conferir identidade/tipo do valor avaliado,
não o nome da variável que o referencia.

Avaliar argumentos/constantes pelo avaliador constante do compilador. Versão,
URI normalizada, conteúdo/identidade do pacote resolvido, argumentos constantes
e esquema entram no hash das provas. `dependency_overrides`/dependência `path`
podem ser usados em desenvolvimento, mas invalidam o cache conforme seu
conteúdo. Nenhuma origem — mesmo o pacote oficial — torna uma afirmação uma
prova de efeitos do corpo. Não consultar a rede durante análise da anotação.

Nova opção proposta: `--contratos-dartforge verificar|ignorar`, padrão
`verificar` para contratos conhecidos encontrados no programa. Preferências
apenas alimentam planejamento/relatório; seus fracassos não viram erros.
No modo `ignorar`, registrar explicitamente no manifesto que os contratos não
foram certificados, sem usá-los como fatos. Esse modo não produz certificado
Realtime e não serve como artefato de aprovação do pipeline de áudio.
Modo de verificação e versão do perfil também entram nas chaves de cache e no
manifesto; um artefato de modo `ignorar` não fornece provas para `verificar`.

Uma versão de esquema não suportada no pacote designado é diagnosticada no
modo de verificação; não tratá-la silenciosamente como contrato satisfeito.
Annotation removida por tree shaking só dispensa análise se a declaração e
todas as suas entradas realmente estiverem inalcançáveis. Override sem
anotação não pode enfraquecer um contrato que o compilador utiliza na chamada.

### 35.5 Integração com a seleção de memória

Preferências fornecem prioridade de análise, pedidos de especialização e
objetivos de custo. Elas podem justificar gastar mais budget num sítio,
separar contexto ou procurar invariante de região, mas não criar uma aresta
ausente no modelo nem eliminar um alvo possível. Contrato comprovado produz
fato reutilizável em `ResumoHeapArc`; contrato pendente é obrigação.

Acrescentar ao plano/relatório: anotação resolvida, span, versão, propriedade
pedida, estado `provado|não_provado|ignorado`, razão, certificado e mudança
de código produzida. Mostrar, por exemplo, por que uma região preferida não
foi selecionada ou qual chamada faz o parâmetro escapar. Não apresentar
“anotação reconhecida” como “otimização aplicada”.

Respeitar contratos depois de inlining, boxing, lowering de exceções, SDK e
emissão. Uma prova sobre a AST que é invalidada por código gerado deixa de
ser válida. Anotações fazem parte das dependências de recarga; remover ou
alterar contrato que sustentava chamadas certificadas exige revalidação
antes de publicar a geração.

### 35.6 Estrutura do pacote e preparação para pub.dev

Estrutura proposta **a criar em uma etapa própria**:

```text
packages/dartforge_annotations/
  pubspec.yaml
  README.md
  CHANGELOG.md
  LICENSE
  lib/dartforge_annotations.dart
  example/
  test/
```

O pacote é uma dependência normal de quem importa suas anotações. Declarar
SDK mínimo efetivamente testado, incluindo Dart 3.6.2 enquanto for alvo;
evitar exigir features mais novas somente para metadata. Se depender de
`meta`, fixar faixa compatível. Testar importação normal, prefixada, reexport
e constantes com as ferramentas Dart e com DartForge.

README deve distinguir preferências e contratos, compiladores que interpretam
as anotações, versões/esquema, limitações de áudio e exemplos comprovados.
Documentar que o pacote não liga automaticamente um plugin de analyzer nem
instala runtime de áudio. O suporte correspondente precisa existir no compilador.

Antes de publicação, executar análise, testes e `dart pub publish --dry-run`,
conferindo os arquivos/metadata do pacote. Publicação efetiva é uma entrega
separada, depois de verificar nome, versão e titularidade; esta especificação
não afirma que um pacote foi criado ou enviado ao pub.dev. O fluxo de preparação
e publicação consta da [documentação oficial do pub](https://dart.dev/tools/pub/publishing).

## 36. Contrato de execução para áudio e outros caminhos de baixa latência

### 36.1 Alcance preciso de `Realtime`

`@Realtime()` solicita o perfil **`dsp-v1`**: execução síncrona de processamento
com buffers/contexto preparados, sem operações de memória ou espera de custo
não controlado no caminho crítico. O perfil verifica efeitos, recursos e
limites estruturais; não é garantia de deadline do sistema operacional.

Coleta de ciclos zero é insuficiente: release que zera contador pode iniciar
cascata, fechamento de região pode percorrer N objetos, biblioteca pode
alocar/bloquear e o wrapper pode inicializar um isolate. A preocupação inclui
alocar, liberar e adquirir mutex; ferramentas como o
[RealtimeSanitizer do Clang](https://clang.llvm.org/docs/RealtimeSanitizer.html)
detectam essas operações em execução. O perfil DartForge descrito aqui é
projeto próprio e exige suporte explícito do backend/runtime.

Certificar corpo **e** fecho de chamadas, prólogo, epílogo, cleanup, stub C,
dispatch, acesso TLS/raízes e caminhos de erro possíveis. Capturar o contexto
numa closure e marcar só seu corpo não certifica a criação nem a entrada da
closure. Uma alteração de SDK/runtime/link pode invalidar o certificado.

### 36.2 Efeitos proibidos e recursos permitidos

Estender `ResumoHeapArc` com um resumo independente de efeitos de baixa
latência. Resolver transitivamente por SCC do grafo de chamadas; externo ou
alvo não modelado recebe efeito `Desconhecido`, que impede a certificação.

| Efeito/operação | Regra `dsp-v1` |
| --- | --- |
| Alocação gerenciada/nativa | Proibida no trecho: inclui boxes numéricas, closures, listas/strings temporárias, crescimento de buffer, metadata, fila e scratch. Scalar replacement provado pode eliminar a alocação antes da verificação final. |
| Desalocação/cascata | Proibir `free`, último release, destruição de grupo/região e drenagem de zeros. Preferir borrows de owners mantidos fora da chamada; retain/drop só é admissível se não tocar caminho lento nem candidato e tiver custo comprovado. |
| Coleta e finalização | Nenhum trial, ponto fixo condicional, reclamador, finalizador ou envio de evento no callback. Não silenciar esses mecanismos globalmente para fazer a função passar. |
| Bloqueio e comunicação | Proibir mutex, espera de condição, I/O, logging, chamada de sistema sem contrato apropriado, spin de duração não limitada e sincronização que espere outra thread. Atomics só com operação/ordenação/custo compatíveis e algoritmo limitado. |
| Inicialização | Resolver antes: lazy static/late, carregamento de biblioteca, lookup de símbolo, JIT, recompilação, TLS, cache RTI, intrínsecos e preparação de stack/contexto. Warm-up observado sozinho não prova ausência de outro caminho lazy. |
| Controle e erros | Provar que os caminhos aceitos não lançam nem criam erro/rastro; exceções/range checks não podem ser removidos só por anotar. Loops têm limite derivável do contrato de buffers/configuração; recursão sem limite provado é recusada. |
| Números e buffers | Permitir operações escalares/SIMD e acesso a armazenamento numérico pré-alocado. Verificar o lowering efetivo: operação Dart aparentemente escalar pode ainda encaixotar resultado ou chamar helper com efeitos. |
| Stack/safepoints | Uso de stack limitado; entrada e pontos seguros não podem iniciar trabalho variável nem esperar coleta de outro domínio. Exigir protocolo do §36.4 antes de certificar entrada real do dispositivo. |

Resumos têm origem e versão: `provado pelo corpo`, `intrínseco verificado` ou
`contrato externo auditado`. Uma anotação colocada sobre `external` não promove
a função ao último grupo. O relatório identifica a fronteira de confiança
nativa e os testes executados. `isLeaf` de FFI não deve ser interpretado como
sinônimo de ausência de alocação, prazo limitado ou auditoria de RT.
A [API oficial de `Native.isLeaf`](https://api.dart.dev/dart-ffi/Native/isLeaf.html)
descreve uma chamada curta, sem bloqueio ou retorno ao Dart; esse contrato
de interoperabilidade não substitui o resumo de efeitos exigido aqui.

O perfil não introduz um timeout que aborta a função para simular uma prova
de duração. Quando um bound não puder ser demonstrado, rejeitar a certificação
solicitada. O programa permanece compilável sem esse contrato; não gerar
comportamento de erro novo em uma execução Dart anteriormente válida.

### 36.3 Exemplo de uso e separação das fases

Exemplo de **API proposta**, condicionado à implementação/verificação do
kernel e dos acessos numéricos; não é afirmação de que o compilador atual
já certifica este programa:

```dart
import 'dart:typed_data';
import 'package:dartforge_annotations/dartforge_annotations.dart';

final class BlocoDeGanho {
  // Construir e preparar antes de iniciar o fluxo de áudio.
  final Float32List entrada = Float32List(256);
  final Float32List saida = Float32List(256);

  @Realtime()
  void processar() {
    for (var i = 0; i < 256; i++) {
      saida[i] = entrada[i] * 0.5;
    }
  }
}
```

O host mantém o contexto e backing stores vivos durante toda a sessão; a
ponte chama o método com empréstimo válido. Comprimentos e limite do laço
precisam ser provados para eliminar caminhos de erro sem mudar semântica.
Esse exemplo pressupõe ausência de mutação concorrente dos mesmos buffers;
a anotação não implementa sincronização, binding de dispositivo ou conversão
automática de layout de áudio.

Fases obrigatórias do protocolo:

1. **Preparar:** alocar/preencher buffers, resolver código, validar tamanho e
   layout, preparar páginas/stack e registrar a ponte/contexto fora do callback.
2. **Processar:** somente operações certificadas, contexto emprestado e
   recursos com capacidade já garantida. Não crescer arena por pressão.
3. **Atualizar:** produzir novo estado na thread/domínio de controle e publicar
   por protocolo limitado de troca; manter o estado anterior até quiescência.
4. **Encerrar:** impedir novas entradas, confirmar que nenhum callback usa o
   contexto e só então liberar owners, regiões/grupos e recursos fora do trecho.

Em 48 kHz, bloco de 256 frames corresponde a aproximadamente 5,33 ms de
intervalo entre blocos. Isso é uma referência para o orçamento do pipeline,
não tempo integral disponível para este método: dispositivo, transporte e
outras etapas também consomem tempo. O tamanho real deve vir da configuração
do host; uma anotação não muda o tamanho do buffer entregue pelo dispositivo.

### 36.4 Threads, isolates e descarte adiado

Thread de callback nativa não pode acessar arbitrariamente `Heap` de outro
isolate. `dsp-v1` precisa de ponte AOT preparada e afinidade/domínio explícitos:
ou entrada compatível com o isolate sem espera variável, ou kernel nativo
certificado sobre dados transferidos/emprestados por contrato. Enquanto essa
ponte não existir, certificar apenas o kernel não certifica a integração de
áudio; emitir diagnóstico para a entrada real, sem esconder custo do wrapper.
Como referência concreta da fronteira nativa, a
[documentação de callbacks do PortAudio](https://portaudio.com/docs/v19-doxydocs/writing_a_callback.html)
descreve execução em thread especial e restrições a alocação, liberação, I/O
e mutex. A ponte deve conferir o contrato do host efetivamente utilizado.

Não basta não alocar na função se outro thread puder pará-la para coleta,
recarga ou depuração. O certificado de entrada deve incluir protocolo que
evite essas pausas no domínio crítico durante a sessão, preserve raízes e
obtenha quiescência sem fazer o callback esperar. Plataforma sem esse suporte
não recebe certificação `dsp-v1` de entrada, mesmo com corpo compatível.

Na v1, evitar criação/destruição de owners no callback e manter o estado
completo sob ownership do host. Uma futura fila de aposentadoria precisa ser
pré-alocada, ter limite provado e política de cheia definida antes de iniciar
a sessão. Não usar fila que cresce, descartar releases, vazar objetos ou fazer
`free` síncrono como fallback da fila cheia. Rejeitar/adiar uma atualização de
controle deve pertencer ao protocolo explícito da aplicação; o compilador
não pode inventar esse comportamento silenciosamente.

Um worker não pode executar releases no heap não atômico de outro isolate.
Descarte ocorre no domínio proprietário após a quiescência, ou por protocolo
de transferência de unidades formalmente implementado. Adiar coleta numa
sessão ilimitada sem limite de memória não é otimização válida para áudio.

### 36.5 Limites de garantia e validação em execução

O certificado declara backend, alvo, perfil, versão de runtime, grafo de chamadas,
limites de buffers/stack, resumos nativos e capacidades de entrada. Ele prova
as propriedades modeladas; não prova ausência de preempção, page fault ou
interferência externa em qualquer máquina. Prazo máximo de um sistema exige
análise e configuração da plataforma, além deste compilador.

Criar modo de auditoria com contadores pré-alocados por thread/domínio para
alocação, liberação, coleta, caminho lento RC, crescimento de fila, entrada
nativa e pausas. O callback não formata mensagens nem escreve log ao registrar
evento. Drenar diagnóstico fora dele. Uma violação pode abortar o **teste de
auditoria**; não usar instrumentação como justificativa para certificar código
cuja prova estática falhou.

Medir duração de callbacks, p50/p95/p99/p99,9, máximo observado, jitter,
underruns/overruns, carga concorrente e duração das sessões. Máximo observado
não é WCET provado. Usar harness nativo e, quando a toolchain/plataforma permitir,
RTSan para fronteiras instrumentadas; não afirmar que instrumentar só um wrapper
C cobre automaticamente todo o IR/Rust/Dart gerado.

### 36.6 Mapa de implementação, resumos e integração

Os pontos abaixo existem no repositório, salvo os explicitamente marcados
**novos**. Implementar o reconhecimento e a prova em camadas distintas:
o parser não pode transformar presença de metadata em uma propriedade de heap.

| Arquivo/símbolo | Alteração exigida |
| --- | --- |
| `crates/frontend/src/ast.rs::Annotation` e `pais.rs::todas_as_anotacoes` | Preservar metadata, argumentos e spans já representados. Percorrer declarações e parâmetros; manter origem após desugaring. Não acrescentar palavra-chave nem gramática de ownership. |
| `crates/types/src/anotacoes.rs::elemento_invocado`, `constantes/avaliador.rs::Motor::avaliar_anotacao` e `constantes/verificador.rs::anotacao` | Unificar resolução inequívoca, avaliação constante e proveniência para o reconhecimento especial. Respeitar shadowing; só usar valor cuja identidade/tipo constante corresponda à declaração suportada. Não substituir resolução por busca textual. |
| `crates/elements/src/model.rs::{Library,Binding,Program}`, `config.rs::PackageConfig` e `load.rs::canonical_file_uri` | Usar URI definidora canônica e binding não ambíguo, inclusive imports prefixados/reexports. `language_version` é versão da linguagem, não versão do pacote. Obter versão resolvida do lock/pubspec quando disponível, esquema constante e hash do conteúdo efetivo; dependência local sem versão confiável continua identificada por conteúdo. |
| **Novo** `crates/types/src/anotacoes_otimizacao.rs` e `resolved.rs::{BodyTypes,UnitBodyTypes}` | Normalizar `PreferenciaRegiao`, `PreferenciaAciclica`, `ContratoNoEscape` e `ContratoRealtime` em mapa por ID de elemento e span. Registrar esquema/origem, alvo e estado de reconhecimento; ainda não emitir fatos de segurança. |
| **Novo** `crates/analise/src/anotacoes_otimizacao.rs` e `meta.rs::verificar` | Verificar alvos, argumentos e combinações. Usar infraestrutura de metadata existente; manter prova profunda no backend compartilhado, sem implementar outra análise de escape no analyzer. |
| `crates/emit_native/src/hir.rs`, `context.rs` e `lower/` | Transportar IDs de contratos/preferências, relação parâmetro–valor e origem de alocações. Inlining/specialization devem clonar obrigações e remapear sítios; função sintética recebe efeitos reais mesmo sem anotação na fonte. |
| **Novo** `crates/emit_native/src/otimizar/arc/analise/contratos.rs` | Vincular as obrigações normalizadas aos sítios/resumos dos §§28–33. Verificar não escape transitivo; priorizar análises de região/aciclicidade; produzir fatos somente com certificado e dependências. |
| **Novo** `crates/emit_native/src/otimizar/arc/analise/latencia.rs` | Calcular efeitos transitivos, limites de laços/stack e obrigações de entradas. Validar perfil `dsp-v1`, com proveniência e caminho de chamada para cada impedimento. Reutilizar SCCs e resumos; não equiparar `nao_lancam` a segurança de áudio. |
| `crates/emit_native/src/otimizar/{mod,efeitos}.rs`, `sdk_modulo.rs` | Rodar análise inicial para orientar otimizações e verificação final sobre a HIR transformada. SDK fornece resumos versionados; funções externas sem resumo têm efeito desconhecido. |
| `crates/emit_native/src/lower/ffi.rs::{lower_ffi,corpo_do_callback}`, `llvm/` e `llvm/raizes.rs` | Incluir conversões, boxing, dispatch, registros de raiz, prólogo/epílogo e helpers nos efeitos da entrada. Relacionar chamadas LLVM emitidas à obrigação HIR; validar também helpers introduzidos pela toolchain ou impedir sua introdução no alvo certificado. |
| `crates/runtime/src/ffi_callbacks.rs::{ContextoCallback,dartforge_ffi_callback_entrar,dartforge_ffi_callback_sair,dartforge_ffi_callback_postar}` | Separar capacidades verificadas da ponte comum e da futura ponte preparada. A atual postagem cria `Vec`/mensagem; a saída pode soltar contexto fechado. Esses caminhos não recebem resumo RT vazio. |
| `crates/runtime/src/{heap,typed_data,ffi,gc_raizes}.rs` e módulos ARC propostos | Expor efeitos reais de helpers, garantir vida de buffers/backing stores e auditar alocação, último drop, safepoint e acesso nativo. Preparar contexto sem acesso concorrente ilegal ao heap do isolate. |
| **Novo** `crates/runtime/src/latencia_conferir.rs` | Auditoria com IDs/contadores de capacidade fixa por domínio, preparados antes da sessão; sem `String`, crescimento de `Vec` ou logging no callback. Registrar no build/`RUNTIME_MAIN` conforme §18. |
| `crates/cli/src/nativo.rs`, `emit_native/src/lib.rs::CompileOptions`, `{cache,cache_objeto,driver}.rs` e `sdk_modulo.rs` | Propagar modo de contratos e perfil imutáveis, incluir dependências no cache e produzir manifesto/certificado por entrada. Não reutilizar objeto cuja ABI, efeitos, alvo ou política de verificação difiram. |
| `crates/jit/src/reload.rs` e `vivo.rs` | Invalidar certificados por mudanças de corpo/layout/SDK. Código de sessão crítica só pode mudar após quiescência e nova admissão; tornar explícita incompatibilidade do modo ativo quando a plataforma não suportar esse protocolo. |
| **Novo** `crates/diagnostics/src/otimizacao.rs`, `crates/lsp/src/semantica.rs::diagnosticar` e `servidor.rs::converter_diagnostico` | Representar diagnósticos DartForge em namespace próprio, com span, callee e motivo. Estender representação/serialização compartilhada; não inserir códigos próprios na tabela gerada de códigos upstream. Exibir contrato pendente quando o LSP não executou a prova do backend. |

`Diagnostic.code` hoje usa `Option<Codigo>`, cuja tabela upstream inclui
`crates/diagnostics/src/codigos_g.rs`. A implementação precisa acomodar códigos
próprios de maneira explícita — por enum discriminada ou canal tipado separado —
e atualizar consumidores. Não simular um código Dart oficial nem depender
de interpretar texto livre. Manter os diagnósticos de paridade upstream em
`crates/paridade/src/analise.rs` separados desses contratos opt-in.

Também não reutilizar diretamente `runtime/src/efeitos_conferir.rs` no callback:
seu auditor atual cria `String` e empilha entradas em `Vec`. A tabela existente
de efeitos coleta/lança é ponto de integração, mas esses dois bits não descrevem
liberação, bloqueio, I/O, inicialização ou limites de trabalho.

**Estruturas novas, em pseudocódigo de interface:**

```text
ContratoNormalizado {
  elemento, parametro_opcional, origem, span,
  identidade_pacote, esquema, especie
}
ResumoLatencia {
  efeitos_possiveis: conjunto<EfeitoLatencia>,
  fronteira_desconhecida: bool,
  limites: { iteracoes, profundidade, stack, recursos },
  precondicoes_provadas, dependencias, testemunhas
}
CertificadoLatencia {
  entrada, alcance: Kernel | EntradaNativa,
  perfil, alvo, abi, hash_codigo, hash_runtime,
  resumos, provas_de_limites, protocolo_de_sessao, contratos_externos
}
```

`EfeitoLatencia` distingue pelo menos alocar, desalocar, RC lento/candidato,
coletar, finalizar, bloquear, I/O, inicializar e lançar. O resumo retém
testemunhas por efeito: instrução, callee, instância de contexto e fronteira.
O conjunto vazio só descreve ausência dos efeitos modelados; limites e
precondições precisam de verificação separada.

Algoritmo obrigatório:

1. Resolver e validar metadata antes de qualquer consumo como contrato.
   Manter obrigações, preferências e fatos certificados em coleções distintas.
2. Calcular efeitos diretos por instrução/helper e propagar a união de todos
   os alvos possíveis das chamadas. Destino opaco adiciona desconhecido.
   Resolver SCCs por ponto fixo monotônico; ausência inicial de efeito não
   autoriza certificar antes da convergência.
3. Calcular limites com expressões simbólicas sobre capacidades/configurações
   validadas: composição sequencial soma trabalho, escolha usa limite superior,
   laço multiplica pelo limite provado. Recursão exige limite de profundidade
   e stack. Widening/budget que perca um limite resulta em desconhecido.
   Essas expressões medem trabalho estrutural, não nanossegundos portáveis.
4. Executar as otimizações legais — unboxing, substituição escalar, inlining,
   borrows, regiões — e refazer resumos afetados. Uma região que elimina trial
   ainda pode falhar no contrato por alocar ou liberar durante o trecho crítico.
5. Validar `NoEscape` sobre todas as saídas, inclusive excepcionais; conferir
   os efeitos/limites de `Realtime` e suas precondições para todos os chamadores
   alcançáveis ou instâncias especializadas certificadas. Precondições não
   provadas não viram suposições silenciosas nem checks que lançam no callback.
6. Após lowering, revalidar efeitos reais e amarrar o certificado à imagem
   gerada e às bibliotecas ligadas. Alteração de alvo, flags relevantes, link
   ou resumo invalida a certificação. Uma passagem LLVM sem preservação
   demonstrada exige nova conferência no estágio que ela produz.
7. Emitir estado separado `reconhecido`, `não_provado`, `provado_kernel` ou
   `provado_entrada`. O estado `ignorado` nunca é aprovação. Relatórios de
   memória continuam independentes: contrato provado não implica que uma
   preferência mudou a política de armazenamento.

Recursos auxiliares de uma entrada certificada têm capacidade e vida explícitas.
O esquema v1 não aceita um inteiro escrito numa anotação como prova de tamanho
de buffer/stack ou de iterações. Derivar os limites do programa e do protocolo
de preparação certificado. Uma futura API de configuração deve especificar
validação antes da sessão e impedir que a configuração mude enquanto em uso.

### 36.7 Entregas, testes e critérios de aceite

Estas entregas complementam E/S dos §§26 e 34; não substituem a infraestrutura
de ARC e não alegam que o suporte esteja implementado.

| Entrega | Resultado verificável |
| --- | --- |
| A0 — pacote e identidade | Declarações Dart válidas, documentação e resolução canônica com esquema; preferências aparecem no relatório sem alterar segurança do código. Publicação só depois da validação própria do pacote. |
| A1 — contratos de heap | `NoEscape` provado por corpo/resumos e ligado ao plano de memória; preferências refinam análise sob orçamento; falhas mostram caminho e conservam semântica quando o contrato é ignorado. |
| A2 — kernel de baixa latência | Resumos transitivos, limites, certificação após lowering, diagnóstico por chamada e manifestação explícita de que a entrada nativa ainda não foi certificada. |
| A3 — entrada nativa preparada | Ponte AOT, domínio/afinidade, raízes, precondições, quiescência e descarte fora do callback certificados em cada plataforma habilitada. |
| A4 — auditoria e medição | Harness nativo, contadores preparados, testes negativos, relatório de latência/memória e invalidação de caches/recarga. Exemplos publicados identificam alvo e versão que passaram. |

Matriz mínima de testes:

| Grupo | Casos e resultado esperado |
| --- | --- |
| Identidade | Import direto/prefixado, reexport, alias `const`, shadowing, biblioteca homônima, binding ambíguo, `path`/override e esquema incompatível. Somente declaração resolvida compatível solicita o contrato; erros de resolução não são mascarados. |
| API e alvos | Metadata nos alvos permitidos, alvo inválido, argumento extra, valor não constante, função async/generator e duplicatas. Erros determinísticos com spans; duplicata idêntica pode ser deduplicada, sem obrigações divergentes. |
| Preferências | Região possível/impossível, aciclicidade provada e sítio desconhecido. Ausência de ganho não reprova compilação; relatório distingue prova, escolha de custo e fallback. Comparar também com fonte sem anotações. |
| Não escape | Retorno de alias, retorno transitivo de campo, throw do parâmetro, captura/async, global, weak/handle, publicação durante reentrada, campo de argumento externo, função recursiva e override. Falhar nos casos não provados; aceitar processamento numérico local e aliases anteriores sem tratá-los como exclusividade. |
| Efeitos ocultos | Callee aloca, getter lazy, fechamento de região, `List.clear` com releases, último drop, box numérica, callback que fecha seu próprio contexto, dispatch desconhecido, FFI apenas `isLeaf`, throw/cleanup. Todos devem impedir certificado quando alcançáveis e não eliminados por prova. |
| Limites e código gerado | Laço fixo sobre buffers preparados passa; limite opaco/recursão ilimitada falha. Inserir helper de alocação depois da análise inicial deve invalidar a prova final. Limite numérico sem prova de capacidade não elimina range check. |
| Entrada nativa | Kernel provado chamado por wrapper alocador ou `NativeCallable.listener` comum não recebe certificado de entrada. Testar thread/domínio incorreto, preparo incompleto, fechamento concorrente e proibição de liberar antes da quiescência. |
| Auditoria | No harness preparado, contadores de alocação/liberação/coleta/bloqueio/caminhos lentos permanecem zero no trecho certificado. Versões negativas devem acionar cada detector; a própria instrumentação não deve introduzir os efeitos que mede. |
| Cache e recarga | Alterar corpo de callee, anotação, conteúdo do pacote, resumo SDK, ABI/target ou ponte invalida provas dependentes. Recarga durante sessão exige quiescência/readmissão; nenhum certificado antigo acompanha código novo. |
| Semântica e portabilidade | Rodar fonte sem metadata, anotada/verificada e anotada/ignorada no diferencial. Comparar resultados, identidade, ordem de efeitos e exceções conforme as permissões de Dart; ferramentas Dart normais devem aceitar os exemplos com dependência compatível. |

Os testes de resolução/constantes pertencem aos crates `types`/`elements`; os
de alvos ao `analise`; os de contratos/efeitos ao `emit_native`; os de ponte e
instrumentação ao `runtime`, com harness de integração por plataforma. Acrescentar
casos de serialização no `diagnostics`/`lsp` e casos semânticos no `diferencial`.
Snapshots isolados do texto do diagnóstico não substituem verificação do efeito
ou da transformação. Publicar fixtures positivos e negativos com suas provas.

Aceitar a entrega de áudio somente com escopo declarado: **kernel** ou **entrada
nativa**. O relatório registra buffer/sample rate, alvo, versões, cenário de
carga, tempo de preparação, duração da sessão, bytes retidos e percentis/máximo.
Medir separadamente o efeito das anotações sobre tempo/memória de compilação,
RC residual e visitas/coletas de ciclos. Não atribuir ganho à metadata quando
o compilador já produzia o mesmo código automaticamente.
