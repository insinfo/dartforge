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
