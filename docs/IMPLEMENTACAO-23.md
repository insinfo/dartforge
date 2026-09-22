# Incremento 23 — alcance global e execução assíncrona

## Tree shaking opcional

`dartforge compile main.dart app.mjs --tree-shake` ativa o passe.
`--no-tree-shake` explicita o comportamento padrão, sem executar a análise de alcance.
As opções são independentes de `--optimize` e `--merge-identical-functions`.
Na API Rust, use `CompileOptions { tree_shaking: true, ..Default::default() }`.

O passe ocorre depois de linking, expansão de macros e análise semântica completa.
Declarações não usadas com erros continuam sendo rejeitadas. O ponto de entrada
e as extensions são raízes; uma fila percorre chamadas, referências a funções,
construções, constantes e descritores de tipos. Ciclos sem raiz podem desaparecer.
Os IDs de classes e tipos permanecem estáveis.

Classes alcançadas conservam todos os campos, construtores, métodos e fábricas,
além dos contratos de herança, interfaces e mixins. É uma análise conservadora de
alcance de declarações, não a TFA completa do Dart SDK. Não faz especialização de
despacho dinâmico, poda de métodos individuais nem elimina instruções ou efeitos.
Tabelas semânticas ainda podem conservar descritores de declarações removidas.

A flag é suportada inicialmente na emissão JavaScript. A API LLVM rejeita o pedido
explicitamente; a validação de recursos nativos continua abrangendo código morto.
O benchmark `pipeline` registra a mesma entrada com o passe desligado e ligado,
separando tempo de compilação e tamanho da saída. Não estabelece superioridade
sobre DDC/dart2js, que compilam uma linguagem e biblioteca muito mais completas.

## Execução assíncrona JavaScript

O frontend preserva `async` em funções, métodos, closures e `main`, além de
`await`, `Future<T>.value`, `Future<T>.delayed`, `Duration`, `Timer` de disparo
único e `scheduleMicrotask`. Imports de `dart:async` participam da resolução de
namespaces, reexports, filtros e ambiguidades; as APIs desse import não viram
nomes globais implicitamente disponíveis.

O runtime usa Futures próprios e continuação controlada pelo compilador.
Representar todos os Futures apenas por Promises nativas altera a ordem de
notificação e apaga camadas em `Future<Future<T>>`. A adoção de Futures e a
suspensão de `await` precisam preservar tanto a ordem quanto o tipo reificado.
O início de uma função async executa sincronamente até a primeira suspensão.

`Duration` recebe unidades nomeadas na ordem escrita. O Timer oferece `cancel`,
`isActive` e `tick`; um timer cancelado não dispara. A duração zero ainda agenda
um evento e não executa o callback dentro da chamada do construtor.

Limites atuais: `Timer.periodic`, `Future.then`, Zones, `async*`, `yield`,
`await for`, Streams, controllers, isolates, ports e TransferableTypedData ainda
exigem implementação própria. Prefixos de imports `dart:async` são rejeitados.
Chamadas de Future sem informação suficiente para inferir o tipo exigem argumento
de tipo ou contexto explícito; o tipo `dynamic` ainda não foi implementado.
Await em seções de cascata e braços de switch-expression também recebe diagnóstico.
O backend LLVM rejeita async até existir lowering e runtime nativos correspondentes.

## Referências e validação

Semântica consultada no Dart SDK 3.6.2, em `sdk/lib/async/future.dart`,
`sdk/lib/async/timer.dart`, `sdk/lib/async/schedule_microtask.dart` e
`sdk/lib/core/duration.dart`. Os testes diferenciais usam a execução do SDK como
oráculo, incluindo ordem das microtarefas, cancelamento e Futures aninhados.
Os testes são locais; este incremento não altera nem acompanha CI.

## Isolates, ports e transferência — pesquisa fechada, contrato

Pesquisa completa em [ISOLATES-WEB.md](ISOLATES-WEB.md); o que segue é o
contrato, não o resumo. Fontes verificadas por `git show` no clone
`references/dart-sdk-1.24.3` (tag `1.24.3`, commit `0b0b41e`, 2017-12-13).

### D1 — Três camadas, semântica numa só

`package:forge_isolate` (Dart comum, compila em DartForge/dart2js/DDC/VM)
define a semântica: `WorkerTask<T,R>` (`const`, nome declarado + referência
direta à função), `WorkerIsolate.guard/run/spawn/terminate`,
`SendPort<T>/ReceivePort<T>` tipados, `TransferableBuffer` de uso único.
`@WorkerEntrypoint()` é metadata sem efeito semântico, só diagnóstico e raiz
de chunk. O reconhecimento como intrínseco no DartForge muda bytes e tempo,
nunca comportamento. Remover as camadas 2 e 3 mantém o programa correto.

### D2 — Entrypoint restrito, nos quatro alvos (resposta a (a))

Um Worker nasce de URL, não de função (`_spawnWorker`, L1115–L1116:
`uri ??= thisScript; new Worker(uri)`). O Dart 1 identificava a função pelo
nome de símbolo do compilador (`_getJSFunctionName`, L939; erro
`only top-level functions can be spawned.`, L955 — ambos confirmados no
clone). Essa chave global (`getGlobalFromName`) quebrava minificação e tree
shaking. O DartForge usa como chave o nome declarado no `WorkerTask` `const`,
com a referência direta servindo de uso normal para o tree shaker.
Limitação resultante, sem rodeio: só função top-level ou estática listada em
`guard`; sem closure com captura, tear-off de instância, `Function`
calculada ou biblioteca tardia. A restrição vale **também na VM**, embora a
VM suporte `Isolate.run` com qualquer closure (`sdk/lib/isolate/isolate.dart:253`
em 3.6.2): o contrato é a interseção, imposto igual nos quatro alvos. Não se
aceita `Function` na assinatura para lançar em execução na Web — assinatura
que aceita o que uma plataforma não suporta transforma erro de compilação em
erro de produção.

### D3 — Serialização: veredito sobre os 4 problemas e exigências (resposta a (b))

| Problema do Dart 1 | Veredito verificado |
|---|---|
| Clone 10–20% mais lento | Persiste como natureza (clone é cópia), muda de grau: o Dart 1 pagava cópia dupla (envelope em arrays + `postMessage` sobre ele, `serializeDartObject` L170). Passando nativos direto, sobra um array curto por instância de classe. |
| Sem transferência de posse | Resolvido pela plataforma (`postMessage(msg, transferList)`); o Dart 1 nunca usava o segundo argumento (`serializeByteBuffer` L79 manda o buffer pelo clone). |
| Blob/File/ImageData não transmissíveis | Resolvido; nunca foi limite da plataforma — era `serializeJSObject` (L127–134) recusando `constructor !== Object` antes do `postMessage`. |
| Inconsistência dart2js/Dartium | Obsoleta na forma (Dartium extinto), viva como Web×VM: a Web lança `DataCloneError` para qualquer `IsCallable` e perde protótipo; a VM aceita closures. Resposta: contrato = interseção. |

Exigências do protocolo: (i) despacho por tipo — nativo de clone estruturado
passa direto, só instância de classe usa envelope com **tag inteira por
compilação** (nunca `o.constructor.name`); (ii) `TransferableBuffer` é a
única categoria com transferência de posse, uso único, mesma classe de
exceção nos quatro alvos; (iii) ciclos e compartilhamento preservados (a VM
e o Dart 1 com `serializedObjectIds`+`ref` os preservam — perdê-los é
divergência); (iv) `dynamic`/genérico não resolvido no tipo da mensagem =
diagnóstico em compilação, nunca tabela global; (v) handshake com hash da
build, com recusa explícita — chunk de worker em cache com `app.mjs` novo
leria tags trocadas.

### D4 — Mirrors: não implementar (resposta a (c))

`disableTreeShaking() => preserveNames()` (`js_mirrors.dart:73`, confirmado
no clone, com chamadas em L141/L630/L822/L1063) e `mirrors_data.dart:216–224`
mostram o mecanismo: cada operação central de reflexão desligava a otimização
do programa inteiro. `dart:mirrors` não está nas seções `dart2js`/`dartdevc`
de `libraries.json` em 3.6.2 — recusá-lo na Web é divergência zero. O
substituto é reflexão estática gerada em compilação, dirigida por anotação,
com alvo na API de `package:reflectable` (contrato em
[MACROS-ARQUITETURA.md](MACROS-ARQUITETURA.md)). Perde-se, sem atenuar:
enumerar o programa (`findLibrary`, `libraries`); refletir objeto arbitrário
não anotado; invocar membro com nome calculado fora do conjunto declarado;
`noSuchMethod` com nomes reais (o que se quer minificar); reflexão sobre
funções, closures e privados; compatibilidade de fonte com
`import 'dart:mirrors'` (diagnóstico próprio que nomeia o substituto).

### D5 — Sem fallback silencioso; sem protótipo neste ciclo

Falha em criar o isolate lança `UnsupportedError`. Modo mesmo-thread só como
opt-in explícito e documentado como não-equivalente (`debugRunInline`, o
`useWorkers=false` do Dart 1), que ainda clona a mensagem. Não há protótipo:
as três peças mínimas (anotação/API no fonte, raízes arbitrárias no tree
shaker, N unidades de saída) caem fora de qualquer subconjunto de crates que
as conteria — ver ISOLATES-WEB.md §"Protótipo: por que não há". Ordem real:
(1) `package:forge_isolate` em Dart puro validado nos 4 alvos; (2) raízes
gerais no tree shaker; (3) emissão em N unidades; (4) intrínsecos + tabela de
chunks; (5) serialização estática + handshake; (6) anotação por último.

### Perguntas abertas

- Q1: URL do worker sob DDC (bootstrap + módulos — sem garantia de carga em
  escopo de Worker).
- Q2: veículo do gancho de tabela de chunks — `@pragma` é rejeitada
  explicitamente (`crates/parser/src/lib.rs:2518–2535`); ou se revê a
  rejeição, ou o gancho usa outro veículo degradável.
- Q3: verificar que dart2js/DDC expõem o uso-após-transferência como
  `StateError` legível pelo programa, não só a VM.
- Q4: formato do handshake (hash da build) e política de recusa — não
  desenhados, só exigidos.
