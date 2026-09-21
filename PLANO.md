# Plano de implementação — DartForge

## Objetivo e limites

Construir em Rust um compilador Dart → JavaScript, analisador reutilizável, servidor LSP,
compilador de templates ngdart 8 e ferramenta de desenvolvimento web.
Objetivos separados: baixa latência de compilação, menor memória/CPU, JavaScript pequeno e
execução rápida no navegador. Melhorar um não implica melhorar os outros.

O workspace é um ponto de partida, não um substituto funcional de DDC/dart2js.
A versão de referência ngdart publicada selecionada é 8.0.0-dev.4, ainda prévia.
O alvo inicial de compatibilidade é Dart 3.6.2, definido pelo proprietário para ngdart. O executável no PATH foi verificado e corresponde ao alvo Dart 3.6.2. O pubspec da referência ngdart exige Dart >=3.1.0 <4.0.0, intervalo que inclui esse alvo.
O clone da branch padrão de AngularDart e o pacote publicado ficam separados.
Dart 3.6.2 permanece o baseline mínimo do ngdart e das regressões. Recursos mais recentes e macros próprias são autorizados, com contratos e testes identificados por versão. Fixar o pacote ngdart e as revisões dos testes; não tratar o baseline como teto.
Não assumir que a branch HEAD do SDK equivale ao Dart exigido pelo ngdart escolhido.

## Princípios técnicos

1. Correção antes de alegações de velocidade: recurso não implementado gera diagnóstico explícito.
2. Dart não é TypeScript: remover tipos não preserva construtores, operadores, coleções,
   igualdade, casts, runtime types, null safety, async, isolates ou bibliotecas padrão.
3. Compartilhar lexer/parser, resolução, tipos e banco de fontes entre build, LSP e lint.
4. Separar desenvolvimento incremental de otimização global de produção.
5. Medir antes de adotar arenas, paralelismo ou estruturas especializadas.
6. Não copiar o pipeline interno do SDK indiscriminadamente: usar especificação e testes
   como contrato, implementações como referência.
7. Um bundler Rust pode otimizar módulos JS, mas não substitui resolução e semântica Dart.
8. Rust como linguagem das ferramentas; JS como saída/runtime gerado. Node e browsers são
   runners de teste, não implementação do compilador.

## Fase 0 — Fundação (iniciada)

- [x] Instalar toolchain Rust em D:\Rust e configurar Cargo/rustup fora de C:.
- [x] Criar workspace com 12 crates e CLI.
- [x] Implementar primeiro caminho vertical: void main() com print de strings simples.
- [x] Criar testes de Unicode, erros e rejeição de sintaxe não implementada.
- [x] Adicionar CI Windows/Linux, fmt, Clippy, testes, build release, lockfile e comparação Dart 3.6.2.
- [x] Definir Dart 3.6.2 como versão alvo inicial, conforme o ambiente ngdart do proprietário.
- [x] Definir licença MIT para o código original do projeto.
- [x] Definir política de contribuições em [CONTRIBUTING.md](CONTRIBUTING.md), incluindo documentação Rust em português.
- [ ] Automatizar corpus de conformidade com revisões fixadas.

Aceite: clone limpo compila com cargo build --locked; exemplo executa e entradas inválidas falham.

## Fase 1 — Fontes, lexer e parser completos

Crates: diagnostics, syntax, lexer, parser; criar source quando houver APIs necessárias.

- IDs estáveis de arquivo, spans, tabela de linhas, conversão UTF-8/UTF-16 para LSP.
- Lexer: comentários aninhados, todas as strings/escapes/interpolação, números e operadores.
- Parser: precedência, declarações, expressões, funções, classes, generics, records/patterns
  conforme versão alvo, directives, annotations, nullability e recuperação de erros.
- AST tipada com IDs; testar arenas por arquivo versus vetores/índices com medições.
- Modo tolerante para edição e modo estrito para build; nenhum erro recuperado pode virar
  emissão de programa alegadamente válido sem regra explícita.
- Golden tests de AST/diagnósticos e fuzzing de lexer/parser.

Aceite: corpus selecionado do SDK cobre sintaxe suportada; sem panic em entrada arbitrária,
spans válidos e diagnóstico estável. Registrar explicitamente testes pulados e motivos.

## Fase 2 — Pacotes, resolução e análise

Crates: semantic, compiler; extrair packages e database quando necessário.

- Ler pubspec.yaml e .dart_tool/package_config.json; resolver package:, dart: e arquivos.
- Inicialmente consumir package_config existente; substituir pub é um projeto separado.
- imports/exports, show/hide, prefixos, part/part of, privacidade por biblioteca e ciclos.
- Tabelas de símbolos e resolução com dependências explícitas.
- Tipos, inferência, null safety, promoção de fluxo, generics, subtyping, overrides,
  parâmetros nomeados/opcionais, tear-offs e resolução de operadores.
- Dependências por biblioteca/unidade; hashes estáveis e invalidação transitiva.
- Cache versionado por compilador, SDK, flags, conteúdo e dependências.

Aceite: testes positivos e negativos do analyzer; diferenças auditadas contra SDK fixado.
Não depender permanentemente de dart analyze para um compilador anunciado como independente.

## Fase 3 — IR e runtime Dart para JavaScript

Crates: hir, codegen; criar mir e runtime conforme o pipeline crescer.

- HIR com símbolos resolvidos; IR mais baixa com CFG e efeitos quando necessário.
- Lowering explícito para construtores/factories, campos, getters/setters, mixins,
  operadores, closures, super e métodos extension.
- Especificar semântica numérica do backend web, ~/ e %, NaN/-0, bitwise e limites.
- Implementar List/Map/Set, iteração, igualdade/hashCode, exceptions e stack traces.
- Generics observáveis, is/as, runtimeType, casts e checks de null safety.
- Future/Stream, async/await, sync*/async*, interoperabilidade JS e plataforma web.
- Subconjunto dart:core primeiro, depois async/collection/convert e APIs web necessárias.
- Não representar coleções/métodos Dart por equivalentes JS sem testes semânticos.
- Emissão ESM, source maps e política explícita de nomes/símbolos.
- Avaliar AST/codegen Oxc ou SWC por custo total e estabilidade de API; não integrar ambos
  antes de medir necessidade.

Aceite: suites diferenciais executam a mesma entrada Dart em compiladores oficiais e
DartForge e comparam saída, erros e comportamento observável.

## Fase 4 — Compilação incremental para desenvolvimento

Crates: compiler, web e futuro database.

- Grafo de imports + dependências semânticas, cache por unidade/biblioteca, cancelamento.
- Rebuild somente de nós afetados com invalidação conservadora correta.
- Paralelismo em unidades independentes; evitar invalidar caches ou emitir fora de ordem.
- Resultados determinísticos com 1 e N workers; orçamento de memória/threads configurável.
- Medir granularidade; evitar paralelismo excessivo em arquivos pequenos.

Aceite: cold build e rebuild após mudar corpo, assinatura, export e dependência produzem
o mesmo programa de um build limpo. Nenhum cache obsoleto.

## Fase 5 — ngdart 8

Crate: ngdart; dividir template_ast/template_parser/template_codegen se justificado.

- Fixar 8.0.0-dev.4 ou uma revisão escolhida e extrair requisitos de SDK do pubspec.
- Inventariar annotations, dependency injection, providers, componentes e diretivas.
- Parsing de HTML, interpolation e expressões de binding com contextos de escape.
- Inputs/outputs, referências locais, diretivas estruturais, pipes e templates aninhados.
- Views, lifecycle, change detection, eventos e integração com o runtime ngdart escolhido.
- Estudar *.template.dart e builders existentes. Começar reproduzindo contratos compatíveis;
  emissão direta em IR/JS só depois de equivalência demonstrada.
- Diagnostics com origem Dart/template e source maps.
- Fixture mínima de componente → bindings → serviços/DI → router/forms quando requeridos.
- Inventariar build_runner/source_gen de terceiros: suporte ou diagnóstico explícito,
  sem alegar compilar qualquer aplicação ngdart.

Aceite: aplicação representativa ngdart abre no browser, renderiza e responde a eventos;
testes DOM/DI/change detection equivalentes à toolchain oficial.

## Fase 6 — LSP, análise e lint

Crate: lsp; criar lint independente das regras quando houver implementação.

- JSON-RPC/LSP por stdio; initialize/shutdown/exit, versões de documento e cancelamento.
- didOpen/didChange/didClose com sincronização incremental e diagnóstico UTF-16 correto.
- Hover, definição, referências, completion, rename e code actions.
- Usar o mesmo banco de análise do compilador; não recompilar o projeto inteiro por tecla.
- Regras de lint com IDs, severidade, configuração e fixes verificáveis.
- Separar erros da linguagem de recomendações estilísticas.
- Estudar analyzer, analysis_server e linter do SDK; repositórios históricos apenas para contexto.
- Adicionar testes de protocolo e transcrições de sessões de edição.

Aceite: editor conecta sem stdout contaminado, alterações invalidam diagnósticos corretos,
rename respeita escopos e o servidor responde a cancelamento.

## Fase 7 — Ferramenta web em Rust

Crate: web; binário separado quando houver comandos implementados.

- Comandos build, serve, watch e clean com configuração explícita.
- Servidor local, pipeline de assets, proxy opcional, atualização de módulos e source maps.
- Primeiro live reload; HMR somente com política de estado e compatibilidade comprovada.
- Integração nativa com bundler Rust após avaliação de Rolldown/Rspack.
- Resolução de assets e caminhos independente do diretório atual.
- Cache persistente, logs e diagnósticos; nenhuma dependência obrigatória de wrapper Node.

Aceite: app de exemplo compila/serve/recompila no Windows e Linux e mantém erros visíveis.

## Fase 8 — Produção e otimizações

- Análise de alcance por símbolos/tipos e tree shaking preservando efeitos.
- Constant folding, propagação, simplificação de CFG, DCE e inlining com orçamento.
- Especialização/desvirtualização apenas quando semântica e modelo de runtime permitem.
- Minificação, code splitting, deduplicação segura e integração do runtime por demanda.
- Funções textualmente iguais não podem ser fundidas ignorando closures, identidade ou efeitos.
- Avaliar custo de otimização versus ganho no bundle e no tempo de execução.

Aceite: toda otimização preserva suites semânticas, tamanho e execução medidos;
comparação de produção usa dart2js com flags documentadas e checks equivalentes.

## Fase 9 — Qualidade, desempenho e distribuição

- Fuzz/property tests, regressões de bugs, determinismo e limites de recursos.
- Harness para compilação fria, quente, incremental; tempos por fase, pico RSS,
  CPU, bytes alocados, tamanho bruto/gzip/brotli e desempenho do JS.
- Corpus: programa pequeno, pacote médio, app ngdart real e cenários sintéticos identificados.
- Comparar desenvolvimento com DDC e produção com dart2js separadamente.
- Rodar warmups/repetições; publicar medianas e p95, versões, hardware, flags e cache.
- Gate de regressão depois de estabelecer baseline; sem números de marketing não medidos.
- Releases reproduzíveis e binários Windows/Linux/macOS.

Aceite: relatório reproduzível demonstra o desempenho em programas semanticamente equivalentes.
“Mais rápido que DDC/dart2js” continua objetivo até isso existir.

## Primeiro incremento implementado

- [x] Variáveis locais, literais inteiros, expressões e precedência no subconjunto.
- [x] Escopos, resolução local, tipos simples e validação de final.
- [x] Geração JavaScript com nomes protegidos e preservação da árvore de expressões.
- [x] Runner diferencial com controle de versão do SDK e cinco fixtures originais.
- [x] Confirmar que o SDK instalado corresponde ao alvo corrigido: Dart 3.6.2.
- [ ] Ampliar para testes da revisão 3.6.2 do SDK e semântica completa de inteiros.

## Segundo incremento

- [x] Corrigir o alvo inicial para Dart 3.6.2, conforme instrução do proprietário.
- [x] Funções tipadas, parâmetros, chamadas, recursão, return e if/else.
- [x] Validar assinaturas, argumentos, retornos, escopos e ordem de avaliação.
- [x] Clonar e avaliar Rust Sitter sem adicionar dependência ao compilador.
- [x] Preparar repositório público e workflow de CI; ignorar references/ integralmente.

## Terceiro incremento — Laços, atualizações e strings

- [x] Implementar `while`, `do/while`, `for` clássico e `break`/`continue` sem rótulos.
- [x] Preservar escopos de cabeçalhos, atualização após `continue` e ordem de avaliação.
- [x] Aceitar `++`, `--`, `+=`, `-=` e `*=` sobre identificadores como instruções e cláusulas de `for`.
- [x] Decodificar escapes de strings, Unicode e pares UTF-16; aceitar strings raw.
- [x] Rejeitar explicitamente surrogates isolados, interpolação e strings triplas.
- [x] Documentar módulos e funções Rust em português, com exemplos executáveis e contratos.

O alcance e as limitações estão em [docs/SUBCONJUNTO.md](docs/SUBCONJUNTO.md).
Esses recursos não concluem o lexer/parser completo nem a semântica integral de Dart.

## Quarto incremento — Null safety, objetos e medições

- [x] Tipos primitivos e classes anuláveis, ??, ! e promoção local com merges conservadores.
- [x] Classes nominais, campos, métodos, construtor implícito e herança simples.
- [x] Validar hierarquia, sobrescritas, mutabilidade e despacho; preservar ordem de inicialização Dart.
- [x] Carregar grafo de imports relativos com ciclos e diagnóstico por arquivo (sem namespace/linkagem ainda).
- [x] Adicionar --optimize com avaliação de constantes puras após validação semântica.
- [x] Benchmarks por fase e processos DDC/dart2js com limitações explícitas e saídas verificadas quando disponíveis.
- [ ] Resolver símbolos por biblioteca, prefixos, privacidade, exports e package_config.
- [ ] Persistir membros resolvidos na HIR e implementar extensions com despacho estático.
- [ ] Ampliar construtores, super, interfaces, generics e análise de fluxo por subtipos.

## Quinto incremento — Bibliotecas, extensions e escalabilidade

- [x] Compilar grafos de imports relativos com namespaces e privacidade por biblioteca.
- [x] Preservar escopos locais, classes globais, ciclos declarativos e diagnósticos por arquivo.
- [x] Resolver chamadas de extensions nomeadas por tipo estático em uma unidade.
- [x] Transportar alvos de extensions na HIR e preservá-los durante otimização.
- [x] Substituir ordenação quadrática de classes por algoritmo topológico iterativo.
- [x] Adicionar benchmarks de hierarquias e compilação de bibliotecas sintéticas.
- [ ] Imports package:/dart:, prefixos, combinadores, exports e parts.
- [ ] Visibilidade de extensions importadas, getters/setters e generics.
- [ ] Cache incremental por unidade, dependências semânticas e invalidação conservadora.

## Sexto incremento — Pacotes, reexports e cache verificado

- [x] Resolver `package:` por package_config v2, URIs e versão de linguagem 3.6.
- [x] Compor show/hide e reexports com ciclos, identidade declarativa e precedência local.
- [x] Cache limitado da última saída com releitura do grafo e comparação exata de conteúdo.
- [x] Consultar testes do SDK na tag 3.6.2 e registrar decisões e experimentos.
- [x] Comparar cache hit/miss no benchmark de bibliotecas, sem confundir cache com incremental.
- [ ] Recompilação incremental por unidade e invalidação por dependências semânticas.
- [ ] Prefixos, runtime dart:, parts, extensions importadas e aplicações ngdart reais.

## Sétimo incremento — AOT LLVM e runtime Rust inicial

- [x] Clonar Dartino LLVM (dart-archive/sdk) e LLVM histórico, registrando revisão/licença.
- [x] Reutilizar frontend, HIR e resolução de bibliotecas para múltiplos backends.
- [x] Emitir LLVM IR textual com i64 modular, bool, funções, escopos e CFG explícito.
- [x] Gerar objetos via Clang e executáveis com runtime Rust pelo linker do rustc.
- [x] Oferecer `emit-llvm` e `aot`, com O0/O2 e diagnóstico de recursos não suportados.
- [x] Corpus nativo com oráculos Dart VM/AOT 3.6.2 e testes em Windows/Linux na CI.
- [ ] Instrumentar tempos por fase e cachear runtime/objetos com chaves de toolchain/ABI.
- [ ] Definir IR tipada própria mais completa, ABI de valores/objetos e runtime de strings.
- [ ] Planejar GC, raízes, barreiras e safepoints antes de objetos móveis.
- [ ] Ampliar null safety nativa, despacho, closures, generics e exceções portáveis.

## Oitavo incremento — Null safety escalar AOT e tempos por fase

- [x] Representar int?/bool? em agregados sem heap, com tag explícita de presença.
- [x] Emitir `??` preguiçoso, `!` checado, igualdade nullable e retornos implícitos null.
- [x] Reutilizar validação de fluxo; não aceitar operações nulas inválidas por inserir checks.
- [x] Medir frontend, gravação, LLVM, runtime/link e publicação via `aot --timings`.
- [x] Cobrir O0/O2, loops com reatribuição, efeitos e falha de ! em subprocessos.
- [x] Objetos, strings e GC adicionados no incremento 9.
- [ ] Exceções capturáveis no backend nativo.
- [ ] Cache de runtime/objetos por ABI/toolchain, orientado pelas medições registradas.

## Próximas tarefas

### Nono incremento — objetos gerenciados e fusão opcional

- [x] Clonar Swift, SwiftWasm, swift-to-js e Shift.JS com revisões registradas.
- [x] Runtime Rust com handles, strings, campos tipados e coleta de ciclos por tracing.
- [x] Classes/herança e despacho nativo, mantendo ordem de inicialização e null safety.
- [x] Corpos de expressão tipados e fusão opt-in de funções estruturalmente idênticas.
- [x] Slots estáticos reutilizáveis para locais e temporários, sem crescimento por iteração.
- [x] Gatilho adicional de GC por bytes estimados de cabeçalhos/payloads vivos.
- [ ] Reduzir duração de raízes com análise de vida útil e limpeza nos fins de escopo.
- [ ] Gerações/barreiras e comparação de throughput/pausas em aplicações representativas.
- [ ] Generalizar IR com IDs de símbolos; especializar genéricos após suporte semântico.
- [ ] Cache de runtime/objetos para reduzir o custo medido de rustc/link por compilação.

1. Estabelecer matriz Dart 3.6.2/ngdart e inventário de testes da revisão correspondente do SDK.
2. Ampliar o parser para interpolação, strings triplas, `for-in` e controle de fluxo ainda ausente; planejar representação que preserve surrogates isolados.
3. Ampliar resolução para bibliotecas e introduzir IDs de símbolo na IR.
4. Expandir o runner diferencial inicial com fixtures do SDK fixado e casos inválidos.
5. Medir lexer/parser/emissão separadamente; só então escolher arena e paralelismo.
6. Expandir o corpus em pequenos incrementos com critérios de aceite acima.

## Referências e rastreabilidade

Consulte docs/REFERENCIAS.md para os caminhos e docs/references-manifest.json para os commits dos clones locais ignorados pelo Git.
Cada teste importado deve registrar origem, revisão e licença; alterações locais devem ser
explícitas. Os clones de referência não são membros do workspace nem dependências de build.

## Décimo primeiro incremento — contratos e alvos

- [x] Classes abstratas, interfaces, implements e despacho polimórfico JS/LLVM.
- [x] Enums simples canônicos com identidade e raízes permanentes no runtime.
- [x] Perfis explícitos Windows x64, Linux x64 e wasm32; validação escalar de ABI C.
- [x] Prova de chamada LLVM para C e emissão de objeto wasm32 em teste.
- [ ] Frontend dart:ffi: tipos Pointer/NativeFunction, lookup/asFunction e bibliotecas.
- [ ] Lifetime de ponteiros, memória externa, callbacks, structs e convenções de chamada.
- [ ] Runtime wasm32, imports, ligação e testes de execução Dart em WebAssembly.
- [ ] Closures, generics, construtores explícitos e interfaces com propriedades.

Contrato e limites: [ABI/FFI/Wasm](docs/ABI-FFI-WASM.md).

## Décimo segundo incremento — coleções e closures

- [x] Arena de tipos estruturais para List<T>, Iterable<T> e assinaturas de funções.
- [x] Closures arrow/bloco, funções como valores e capturas léxicas no JS.
- [x] Listas expansíveis, indexação e callbacks com inferência contextual no JS.
- [x] Iterable lazy e import dart:core sem filtros.
- [x] Células, ambientes, closures e listas rastreados no runtime Rust.
- [x] Preservar identidade desativando fusão quando funções são valores observáveis.
- [ ] Closure conversion, chamadas indiretas e coleções no emissor LLVM.
- [ ] Genéricos do usuário, bounds, especialização e tipos reificados.
- [ ] Set/Map, API core completa e bibliotecas padrão adicionais.

Veja [contratos e limites](docs/COLECOES-CLOSURES.md).

### Incremento 13 — funções genéricas, constantes e enums avançadas

Implementar e validar o subconjunto JavaScript descrito em
[GENERICS-CONST-ENUMS.md](docs/GENERICS-CONST-ENUMS.md). Os próximos passos incluem
classes genéricas/bounds, constantes top-level e construtores const gerais,
padrões estruturais e lowering LLVM com preservação de identidade e raízes GC.
A avaliação de desempenho deve comparar programas equivalentes, com a mesma
semântica e modos de otimização; os microbenchmarks atuais não comprovam vantagem
sobre DDC/dart2js.

### Incremento 14 — modificadores, mixins e sealed

Implementar as restrições nominais por biblioteca, propagação base/final,
aplicações de mixins e exaustividade sealed conforme Dart 3.6.2. Contrato em
[CLASS-MODIFIERS.md](docs/CLASS-MODIFIERS.md). Próximas etapas: partes da biblioteca,
restrições on e super, contratos completos de propriedades, padrões de objeto
com desestruturação e lowering LLVM de switch.

### Incremento 15 — diretivas condicionais

Seleção por primeira correspondência em imports/exports, perfis imutáveis Dart
3.6.2, inspeção de grafo JS/Native/Wasm e cache sensível ao ambiente implementados.
Veja [contrato e limites](docs/IMPLEMENTACAO-15.md). Próximos passos incluem
implementação das bibliotecas padrão adicionais e backend Wasm; as flags atuais
descrevem a seleção compatível com o SDK, sem habilitar essas APIs.

### Incremento 16 — anotações e FFI escalar estático

Metadados reconhecidos, assinatura @Native/external, validação de import e ligação
de objetos nativos com Int32/Int64/Void. Veja [contrato](docs/IMPLEMENTACAO-16.md).
Próximos passos: tipos Pointer e memória externa com lifetime explícito, layout de
structs/unions por ABI, callbacks, finalização, resolução de assets e bibliotecas
dinâmicas; expandir metadata para constantes customizadas e ferramentas.

### Incremento 17 — this e construtores posicionais

Construtores generativos sem nome, initializing formals, campos sem inicializador
e resolução de membros com escopo léxico. JS e LLVM preservam a ordem de avaliação
e inicialização. Próximos passos: construtores nomeados, listas de inicialização,
super explícito, factory, const geral e parâmetros opcionais/nomeados.
Contrato em [THIS-CONSTRUCTORS.md](docs/THIS-CONSTRUCTORS.md).

### Incremento 18 — bounds e reificação no JavaScript

Funções genéricas top-level ganham bounds, anulabilidade e argumentos de tipo
preservados em execução. Descritores suportam testes/casts e tipos originais dos
elementos de coleções. Próximos passos: binders distintos para classes e métodos,
tipos nominais aplicados, substituição na herança, bounds recursivos e padrões
parametrizados; depois compartilhar os descritores com LLVM/Wasm e GC. Medir custo
de descritores e especializar somente preservando identidade e verificações Dart.

### Incremento 19 — records e desestruturação

Records estruturais e declarações var/final por padrão simples no JavaScript.
Próximos passos: padrões aninhados e de atribuição, constantes de records e layout
LLVM/Wasm; extension types precisam de identidade estática e erasure distinto de
classes. Macros oficiais foram canceladas, portanto uma eventual metaprogramação
experimental própria exige contrato separado da compatibilidade Dart 3.6.2.

## Incremento 20 — cascatas

- Implementar `..`/`?..` em JavaScript com avaliação única, null safety, efeitos
  ordenados, imports e integração com otimizações: ver [contrato](docs/IMPLEMENTACAO-20.md).
- Próximos passos: atribuições compostas em seções, lowering nativo com raízes GC
  e extensão dos testes diferenciais. Recursos posteriores ao Dart 3.6.2 exigem
  identificação da versão mínima por recurso, sem alterar silenciosamente a linguagem.

## Incremento 21 — macros e evolução da linguagem

- Dart 3.6.2 é o mínimo de compatibilidade, não um teto. O abandono oficial de
  macros não impede a implementação experimental própria do DartForge.
- Primeira macro: @JsonCodable incorporada em Rust, expansão AST anterior à
  semântica, proveniência e transação; Map com chave String e fábricas nomeadas JS.
- Pendentes: executar macros de usuário, protocolo/resolução de pacotes, cache
  das consultas/expansões, serialização composta e backend nativo.
- Implementar progressivamente wildcard variables, null-aware elements,
  dot shorthands, parâmetros nomeados privados, primary constructors e
  extension types com contratos versionados e testes de SDK correspondentes.
- Referências e detalhes: [IMPLEMENTACAO-21](docs/IMPLEMENTACAO-21.md).

## Incremento 22 — fases e planos de macros

- Barreiras Types/Declarations/Definitions, transação restrita às classes alteradas,
  cache LRU com limites, invalidação de schema e proveniência rematerializada.
- Próximos: consultas entre bibliotecas, protocolo de workers isolados e macros Dart
  de usuário. A macro Rust incorporada não é um executor genérico de Dart.
- [Contrato e medição](docs/IMPLEMENTACAO-22.md).

## Incremento 23 — alcance e concorrência

- Tree shaking conservador JS opt-in; análise semântica completa anterior à poda.
- Future/async/await, microtasks e timers one-shot com oráculo Dart 3.6.2.
- Próximos: Timer.periodic, composição de Futures e Zones; Streams com cancelamento,
  pausa e erros; async* e await-for; isolates com heaps/event loops distintos, ports,
  validação de mensagens e transferência de propriedade de buffers.
- Isolate.run não pode ser implementado como chamada síncrona nem Promise na mesma
  thread. No JS precisa de workers; no nativo exige heaps/GC isolados e protocolo.
- [Contrato e limites](docs/IMPLEMENTACAO-23.md).

## Incremento 24 — macro @DataClass e augmentations

- Segunda macro incorporada com o mesmo contrato de fases, plano cacheável e
  proveniência; coexistência verificada com @JsonCodable.
- Emissão textual determinística das declarações geradas em forma de
  `augment class`, para inspeção e prova de proveniência; o texto não é
  reconsumido pelo compilador.
- Limites do subconjunto documentados: `copyWith` posicional anulável,
  `igualA` no lugar de `operator ==` e `descrever()` reduzido.
- [Contrato](docs/IMPLEMENTACAO-24.md).

## Incremento 25 — sintaxe moderna posterior ao Dart 3.6.2

- Curingas `_` (3.7), elementos null-aware `?valor` (3.8), atalhos de ponto
  `.membro` (3.10) e construtores primários `class C(...)` (3.13) no backend
  JavaScript, cada um identificado pela versão mínima em que existe.
- `part`/`part of` com namespace, imports e privacidade compartilhados pela
  biblioteca declarante: ver [PARTS](docs/PARTS.md).
- `switch` instrução e expressão com lowering real no backend LLVM.
- Pendentes: parâmetros nomeados, `required` e valores padrão — pré-requisito
  dos parâmetros nomeados privados (3.12); `Set`, spreads e `?...`; membros
  estáticos; extension types (3.3).
- [Contrato e limites](docs/IMPLEMENTACAO-25.md).

## Incremento 26 — desempenho de compilação como objetivo primário

O compromisso passa a ser explícito: uma edição pequena deve provocar uma
quantidade pequena de trabalho. Medição antes de otimização, e contadores de
trabalho realizado ao lado dos tempos, porque mediana sozinha não distingue
reuso de máquina rápida.

- [x] `LinkStats`/`CompileReport`: tempo por fase cronometrado no próprio trecho
      e contadores de unidades, tokens, classes, funções e bytes emitidos.
- [x] Crate `dartforge-instrument` com alocador contador (bytes vivos, pico e
      número de alocações), isolado no binário de benchmark.
- [x] Benchmark `incremental` com sequências reais de edição: comentário, corpo,
      assinatura pública, constante e import.
- [x] Primeiro gargalo eliminado: as tabelas de classes, funções e extensions
      deixaram de ser copiadas a cada ramificação de fluxo. Compilação fria
      21,04 ms → 6,04 ms; fase semântica 14,57 ms → 1,12 ms; alocações por
      compilação 311.676 → 16.411.
- [x] Revalidação incremental do grafo: releitura paralela dos arquivos já
      conhecidos e reuso da estrutura quando só os corpos mudaram, com as
      diretivas reextraídas e comparadas estruturalmente. Acerto de cache
      1,22 ms → 0,40 ms e 1.064 → 124 alocações.
- [x] Suíte de validade do cache com sequências de edições, exclusão, restauração
      e renomeação, exigindo igualdade com a compilação limpa em cada passo.
- [ ] Incrementalidade do front-end: a AST empresta `&str` da fonte, então nada
      sobrevive entre solicitações. Exige interning com `SymbolId` e arenas por
      unidade antes de qualquer cache por unidade.
- [ ] Incrementalidade semântica: resumo da biblioteca separado dos corpos, com
      identidade estável por declaração e impressão digital que interrompe a
      invalidação quando a interface não muda.
- [ ] Emissão modular por biblioteca com nomes estáveis e texto em cache.
- [ ] Leitura do grafo por níveis em paralelo, sem afrouxar a verificação por
      conteúdo exato.
- [ ] Interning de nomes e tipos; arenas por unidade e por revisão.
- [ ] Paralelismo por unidade no front-end, depois da incrementalidade.
- [ ] Um módulo JavaScript por biblioteca Dart (a biblioteca é o arquivo mais
      seus `part`, não o pacote), com as dependências preservadas como imports
      ESM. O empacotamento para produção fica como etapa separada. Granularidade
      da saída e granularidade do trabalho são requisitos distintos: recompilação
      realmente rápida precisa dos dois.

Metodologia, linha de base e perfil por cenário: [DESEMPENHO](docs/DESEMPENHO.md).

## Incremento 27 — Dart de produção

Medição que abriu esta frente: das 47 construções comuns de Dart verificadas por
sondagem, **9 compilavam**. Um compilador rápido que não compila código real não
serve; e cada recurso novo entra sob a mesma disciplina de desempenho do
incremento 26 — alocações por compilação medidas antes e depois, porque o tempo
tem ruído e a contagem de alocações não.

- [x] Parâmetros nomeados, posicionais opcionais, `required`, valores padrão e
      parâmetros nomeados privados `this._x` (Dart 3.12). Alocações por
      compilação inalteradas: 16.716 antes e depois.
      [Contrato](docs/PARAMETROS.md).
- [ ] Literais de string completos: interpolação, strings triplas e literais
      adjacentes. É a lacuna mais bloqueante: quase todo arquivo Dart real usa.
- [ ] Construtores nomeados, listas de inicialização, `super` explícito,
      construtores `const`, membros estáticos e declarações de topo.
- [ ] Controle de fluxo: `try`/`catch`/`finally`, `throw`, `rethrow`, `assert`,
      `for-in`, rótulos em `break`/`continue`, operador ternário e `late`.
- [ ] Coleções: `Set`, spreads `...`/`...?`, `if`/`for` em literais, acesso
      null-aware `?.` e os operadores `~/`, `|`, `&`, `^`, `<<`, `>>`, `~`.
- [ ] `double` e `num` com a semântica numérica do alvo web, que não é a da VM.
- [ ] Classes genéricas, `typedef` e extension types.

Correção incidental: a rota rápida de unidade isolada exigia apenas ausência de
`import`/`export`. Um arquivo que declara somente `library x;` ou `part` não tem
aresta alguma mas tem prefixo de diretivas, e era entregue inteiro ao parser.
A condição passou a exigir também ausência de `part` e prefixo vazio.

## Incremento 28 — perfis de execução: JIT, hot reload e AOT

Decisão de arquitetura: **um frontend incremental**, **uma HIR/MIR própria**, **um
runtime nativo compartilhado** e **dois perfis** sobre o mesmo alvo. `Native`
continua sendo o alvo; `DevJit` e `ReleaseAot` são perfis, não plataformas — do
contrário a seleção de bibliotecas condicionais passaria a enxergar
desenvolvimento e produção como sistemas diferentes.

A HIR própria existe para que as decisões da linguagem não fiquem amarradas ao
LLVM. Cada backend traduz as operações dela; nenhum backend recebe a IR de outro.

### Experimento de backend, em curso

Dois JITs sendo construídos para serem comparados por medição, não por
preferência:

- `crates/jit` — LLVM ORCv2 via `llvm-sys 221.1.0`, partindo do LLVM IR que o
  backend nativo já emite. Exige a distribuição completa do LLVM 22.1.8
  (`LLVM_SYS_221_PREFIX`), com `llvm-config`, cabeçalhos e libs estáticas.
- `crates/cranelift-jit` — Cranelift, Rust puro, sem dependência externa,
  partindo da **HIR**, nunca de LLVM IR.

Quatro eixos de comparação: tempo de geração de código em memória, tempo de
execução do código gerado, custo de construção do próprio compilador (inclusive
os ~3 GB da distribuição LLVM) e quais primitivas cada um oferece para redefinir
uma função já compilada.

Contrato central dos dois: **teste diferencial contra o AOT**. Um backend que
discorda do AOT no mesmo programa está errado, por mais rápido que seja.

### Hot reload — depois que o backend estiver escolhido

Misturar a depuração do protocolo de recarga com a escolha de backend é o jeito
mais rápido de não entender nenhum dos dois. O desenho:

- **Identidade estável, implementação substituível.** Cada função recarregável
  tem uma entrada permanente e uma implementação versionada por geração. As
  chamadas passam pela entrada. `LLVMOrcCreateLocalIndirectStubsManager` e
  `LLVMOrcCreateLocalLazyCallThroughManager` existem na API C e servem a isso.
- **Identidades separadas por propósito**: identidade da declaração na sessão,
  impressão digital do contrato de chamada, do corpo e do ambiente capturado.
  O hash do corpo não serve como identidade da função.
- **Atualização transacional em três fases**: preparar sem tocar na aplicação em
  execução, publicar num ponto seguro, e só então aposentar o código antigo.
  Uma falha de análise ou de geração não pode destruir a versão que funciona.
- **Regra de visibilidade**: chamadas já iniciadas terminam no corpo antigo;
  chamadas novas usam a implementação nova. Substituir frames ativos fica fora.
- **Retirada de código antigo** só quando nenhuma função da geração estiver
  executando. Na primeira versão, reter gerações até o reinício, com o
  crescimento de memória declarado.
- **Escopo da versão 1**: apenas mudanças de corpo com contrato compatível.
  Assinatura, campos de classe e ambiente de closure incompatíveis exigem
  reinício, com diagnóstico claro em vez de corrupção.
- **Fusão de funções desativada** no caminho de recarga: compartilhar corpo
  entre duas declarações impediria substituir uma sem afetar a outra.
- **Inlining não atravessa fronteira recarregável** na primeira versão. Se `A`
  incorporou o corpo de `B`, trocar a entrada de `B` não atualiza a cópia.

### Medição do ciclo de recarga

O relatório precisa separar: detecção da edição, análise incremental, geração da
IR, geração de código nativo, ligação em memória, espera pelo ponto seguro,
publicação e atualização da aplicação. Um JIT duas vezes mais rápido na geração
nativa não torna a recarga duas vezes mais rápida quando a geração nativa é uma
fração pequena do total — a mesma aritmética que já orientou o incremento 26.

### Trilha JavaScript, separada

Frontend incremental → JavaScript modular → protocolo de recarga no navegador.
ORC não entra nesse caminho.
