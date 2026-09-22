# Plano de implementação — DartForge

## Objetivo e limites

Construir em Rust um compilador Dart → JavaScript, analisador reutilizável, servidor LSP,
compilador de templates ngdart 8 e ferramenta de desenvolvimento web para substituir o webdev lento.
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
10. **Referência antes de código, correção inteira de uma vez**: ao encontrar
   uma falha, primeiro localizar a regra na referência (emissor do DDC e
   saída do `dartdevc`, `docs/CONTRATO-DDC.md`, especificação e CFE,
   `runtime/lib/*.cc` da VM, `pkg/analyzer`, `PLANO.md`/`docs/`), ler o
   código próprio envolvido inteiro, achar todas as ocorrências do mesmo
   problema e corrigi-las juntas; compilar **uma vez** para confirmar.
   Compilar/executar para descobrir o que fazer é proibido: cada ciclo
   cego custa minutos numa máquina de 8 GB com vários agentes e produz
   patches parciais em série. `cargo build` direto quando o passo seguinte
   é executar (o `check` não é reaproveitado pelo `build`).
9. **Regra fundamental — foco estrito em funcionar**: Não perder tempo com formatação, estilo cosmético ou minúcias de linter. O critério exclusivo de sucesso é o código compilar, funcionar, passar na suíte de testes do compilador, manter latência ultrabaixa e respeitar o limite de memória.

## Fase 0 — Fundação (iniciada)

- [x] Instalar toolchain Rust em D:\Rust e D:\LLVM\22.1.8\bin e configurar Cargo/rustup fora de C:.
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
- Pesquisa de isolates/workers **fechada como contrato** (2026-09-21):
  desenho em [ISOLATES-WEB.md](docs/ISOLATES-WEB.md), decisões em
  [IMPLEMENTACAO-23.md](docs/IMPLEMENTACAO-23.md) — API portátil
  `package:forge_isolate` + `WorkerTask` const, entrypoint restrito nos quatro
  alvos, serialização dirigida por tipo com `TransferableBuffer` de uso único,
  `dart:mirrors` não implementado (reflexão estática mirando
  `package:reflectable`, verificado 5.2.3 em pub.dev). Sem protótipo neste
  ciclo; ordem real começa por Dart puro validado nos 4 alvos.
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
- [x] Núcleo `dart:core` nominal em 2026-09-21: `Comparable`/`Iterator`/
      `Iterable`/`Exception`/`StringBuffer` como interfaces sintéticas,
      `implements` aceitando palavras reservadas (`Iterable<int>`) e type
      arguments, `compareTo` apagado resolvendo por receptor de tipo
      `Comparable`, formas cruas de `List`/`Set`/`Iterable`, `Map.remove`
      anulável. Suíte em `crates/compiler/tests/nucleo.rs` (31 testes,
      oráculo Dart 3.6.2). Limites honestos: `Map` cru recusado (só chave
      `String` na emissão), `implements Error` recusado (sem `StackTrace`).
      [Contrato](docs/NUCLEO.md).
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

### Decisão: o backend consome a HIR, não assembly escrito à mão

Três bibliotecas de geração de código foram avaliadas como backend de JIT —
Cranelift, asmkit-rs e dynasm-rs — além do LLVM ORCv2. A decisão de arquitetura
é anterior à escolha da biblioteca:

**O caminho principal gera código a partir da HIR.** Escrever o emissor como
macros de assembly foi descartado como fundação por quatro motivos específicos
do Dart:

1. Genéricos reificados, checagens de null safety, raízes e safepoints de GC e
   propagação de exceções são lowering de verdade. Escrevê-los em assembly
   multiplica o trabalho pelo número de arquiteturas.
2. x64 e aarch64 são ambos necessários. Uma IR gera os dois; assembly à mão
   exige dois emissores mantidos em paralelo, e cada recurso novo do Dart entra
   duas vezes.
3. Hot reload precisa representar exceções atravessando chamadas. Cranelift tem
   `try_call`/`try_call_indirect`; em assembly puro isso vira ABI manual.
4. A HIR já existe e é onde as decisões da linguagem estão. Ignorá-la
   descartaria a camada que separa o significado do Dart da forma da CPU.

**Assembly escrito à mão tem um lugar, e não é esse.** Stubs e trampolins do
hot reload — entrada estável que salta por ponteiro atualizável, stub de
compilação preguiçosa, patching de chamadas — são fragmentos pequenos, fixos e
específicos de arquitetura. É exatamente onde escrever o assembly ganha de
descrevê-lo numa IR. Um tier 0 no modelo template JIT, com forma fixa por
operação, é otimização posterior legítima, não fundação.

**Os múltiplos de velocidade de geração não se transferem para o ciclo.** Os
fatores citados na literatura comparam geração de código isolada. O que decide
o hot reload são as oito etapas do ciclo. Este projeto já mediu a mesma
aritmética morder: a compilação fria é 5,17 ms e acelerar o parser em 10×
reduziria o total em 6%. Se a geração nativa for 15% do ciclo de recarga,
trocar por um backend 20× mais rápido leva o ciclo a 85%, não a 5%. O relatório
de recarga por etapa vem antes da escolha do backend.

**libtcc resolve outro problema.** Ele compila C, então usá-lo significaria
gerar código C a partir da HIR — transpilação, não backend. Rápido e simples,
ao custo de perder controle sobre ABI, integração com o GC e depuração, e de
exigir uma toolchain C em execução.

## Decisão de foco — um backend pronto antes de muitos pela metade

Decidido pelo proprietário: **o backend JavaScript é o que precisa chegar a
100%**, com dois perfis sobre o mesmo front-end — compilação ultrarrápida para
desenvolvimento e compilação lenta e otimizada para produção.

### Por que esta decisão

A aferição contra código real deu o veredito: o backend JavaScript, o mais
completo dos quatro, aceitava 5 de 179 arquivos de um pacote do pub.dev. Os três
backends de JIT em construção — ORCv2, Cranelift e dynasm — compilam funções
com inteiros, laços e chamadas; nenhum compila uma classe. A distância entre
isso e código Dart de produção não é de polimento, é de ordem de grandeza.

Construir os quatro em paralelo, antes de qualquer um compilar Dart real, é
otimizar o gargalo errado. É a mesma armadilha que já apareceu neste projeto
quando quase se trocou o backend de geração de código para resolver contenção da
trava do Cargo.

### O que muda

- **A métrica de sucesso passa a ser uma só**: quantos dos 179 arquivos do
  pacote `pdf` compilam, e depois o mesmo contra pacotes de outros domínios.
  Sondagens sintéticas de construções isoladas continuam úteis para regressão,
  mas não definem prioridade.
- **Os experimentos de JIT já entregaram o que se precisava deles agora**: a
  comparação medida entre os três backends, que responde qual escolher quando o
  front-end estiver pronto. Eles são pausados, não descartados, e voltam com a
  decisão de backend já tomada por medição.
- **O LLVM/AOT acompanha, não lidera.** A regra de que os dois backends
  concordam no comportamento observável do mesmo programa é o que impede o
  JavaScript de virar um dialeto próprio. Acompanhar é diferente de liderar.

### Os dois perfis do backend JavaScript

| | Desenvolvimento | Produção |
| --- | --- | --- |
| Objetivo | menor latência entre editar e executar | menor JavaScript e maior velocidade no navegador |
| Emissão | modular, um módulo por biblioteca Dart, nomes estáveis | agrupada, com nomes minificados |
| Otimizações | nenhuma que atravesse fronteira recarregável | alcance, constantes, fusão, tree shaking |
| Cache | sessão em memória, revalidação de grafo, cache em disco | irrelevante; o custo é aceitável uma vez |

As duas granularidades continuam sendo requisitos distintos e ambos necessários:
**granularidade do trabalho**, não reanalisar bibliotecas não afetadas, e
**granularidade da saída**, um arquivo por biblioteca Dart.

## Meta governante — qualquer projeto Dart 3.6 válido no dart2js/DDC compila

**A compilação Dart → JavaScript tem de funcionar para qualquer projeto Dart
3.6.0 válido que o dart2js ou o DDC compilem.** Registrado em 2026-09-21 por
instrução do proprietário. Esta meta é superior a toda decisão anterior de
"recusar de propósito" — `dynamic`, `runtimeType`, `noSuchMethod`, `DateTime`,
`RegExp`, `Uri`, `Stopwatch`, `Map` cru, `dart:convert`, `dart:collection`,
`dart:math`, `dart:typed_data`, `dart:io` — porque um projeto de produção usa
todos eles, e o critério de aceite passa a ser o do compilador oficial, não o do
subconjunto.

### O que a meta implica na arquitetura, medido no código de hoje

O front-end atual é um compilador de subconjunto, e as representações centrais
foram desenhadas para isso:

* o parser **resolve tipos durante a análise sintática** (`type_at` em
  `crates/parser/src/lib.rs`): exige um ambiente de nomes de classe, apaga os
  argumentos de tipo de classes genéricas do usuário e recusa qualquer nome que
  não conheça com `expected an explicitly supported type`. Um parser de Dart
  completo não pode depender de resolução: `Foo<Bar>` é sintaxe válida sem saber
  o que `Foo` é;
* `dartforge_syntax::Type` é uma enumeração `Copy` com variantes `Int`,
  `NullableInt`, `Duration`, `Timer`, e `TypeShape` com `List`/`Map`/`Future`
  fixos. Não representa `dynamic`, tipos de função com parâmetros nomeados,
  `FutureOr`, tipos genéricos de usuário reificados, bounds nem `Never`;
* não existe fonte de `dart:core`: os membros são tabelas em Rust
  (`crates/semantic/src/nucleo.rs`), 40 nomes de membro, com `Comparable`,
  `Iterator` e vizinhos numa faixa reservada de identificadores;
* a aferição de 2026-09-21 (`docs/CORPUS-REAL.md`, modo grafo) mostra **0 de
  113 pontos de entrada** dos quatro pacotes medidos chegando ao front-end: o
  carregador para em `dart:convert`, `dart:io`, `dart:math`, `dart:collection`
  e `dart:typed_data`, e o parser para em `#símbolo`.

Estender essas representações uma variante por vez não converge para a meta:
cada recurso novo do Dart pede outra variante fixa, e a meta é a linguagem
inteira. A decisão é construir o **front-end completo** como uma trilha nova,
com o mesmo desenho de memória já registrado (arenas, índices, `SymbolId`), e
migrar a emissão para ele quando a análise estiver completa. O subconjunto
atual continua compilando o que já compila até lá — nenhuma capacidade
existente é removida antes de a nova trilha a cobrir.

### A trilha, em ordem, com o critério de aceite de cada passo

1. **Sintaxe completa** — `crates/frontend`: lexer, AST e parser de Dart 3.6
   inteiro, sem resolução. Aceite: **100%** dos 426 arquivos de `lib/` do SDK
   3.6.2 (`C:/tools/dartsdk-3.6.2/lib`) e **100%** dos arquivos do corpus
   (`references/pub/`) analisados sem diagnóstico. O `lib/` do SDK é o corpus
   de parser definitivo porque contém o runtime do dart2js escrito em Dart
   (`_internal/js_runtime`) e usa toda a gramática.
   **Cumprido em 2026-09-21**: 426/426 do SDK e 1.969/1.969 do corpus
   (26 pacotes), afirmado por `crates/frontend/tests/corpus.rs`. As últimas
   lacunas, cada uma com teste de regressão: o `?` após `is T`/`as T`
   decidido pela lista de `computeTypeAfterIsOrAs` do SDK (com tentativa
   para `{`/`when`); função local com retorno anulável; `.5` como `double`;
   `operator` como nome de campo; padrão de objeto em `for-in`; vírgula
   final nos atualizadores do `for` (aceita pelo Dart 3.6.2 embora fora da
   gramática escrita); elementos null-aware `?e` (3.8, o corpus usa); e
   strings com surrogates soltos — o escape de surrogate alto sozinho é
   literal válido, então `StringPart::Text` passou a ser `DartStr` (WTF-8,
   `utf16_len()` é o `length` do Dart) em vez de `Box<str>`. Duas
   expectativas de teste estavam erradas em relação ao oráculo e foram
   corrigidas: `await x;` fora de `async` é declaração de tipo `await` no
   SDK, e a contagem de membros de uma classe de teste.
2. **Modelo de elementos e resolução** — bibliotecas, imports/exports com
   combinadores e prefixos, `part`, escopos, classes/mixins/extensions/
   extension types, enums, typedefs. Aceite: toda referência de nome do SDK e
   do corpus resolve para uma declaração.
   **Cumprido em 2026-09-21** (`crates/elements`): 36 bibliotecas do SDK,
   179 unidades, 1.525 classes, 19.849 funções em 0,46 s; patches do DDC
   fundidos (`int.parse` sem `external`); 269/269 supertipos resolvidos;
   `crates/elements/tests/sdk.rs`, 14 testes.
3. **Tipos e inferência** — o sistema de tipos do Dart 3: `dynamic`, `Never`,
   `FutureOr`, tipos de função com parâmetros nomeados e opcionais, generics
   com bounds e variância por uso, promoção de fluxo, inferência de literais e
   de argumentos de tipo, extensões. Aceite: o SDK e o corpus passam sem erro
   de tipo, e os testes negativos do `analyzer` reprovam onde ele reprova.
   Em andamento (`crates/types`, brief em `brief-types.md`): primeira metade
   é representação hash-consed, resolução das anotações do outline,
   hierarquia instanciada e subtipagem regra a regra.
4. **SDK a partir da fonte** — compilar `dart:core`, `dart:async`,
   `dart:collection`, `dart:convert`, `dart:math`, `dart:typed_data`,
   `dart:js_interop` e `dart:_internal`/`_js_helper` a partir do `lib/` do SDK
   com os *patch files* do dart2js, como o dart2js faz. Isso substitui as
   tabelas em Rust de `nucleo.rs` por código Dart, e é o único caminho em que
   a cobertura de biblioteca é a do SDK e não uma lista mantida à mão.
   **Cumprido em 2026-09-22** (`crates/types`, commit 2e779d8): escopos,
   inferência de expressões, promoção por fluxo, constantes, 40 negativos do
   `analyzer`; SDK 120.055 expressões em 155 ms; `new_sali` 132.146
   expressões em 26 ms, +4,4 MB.
4. (**decisão 2026-09-22**) O `dart:*` em JavaScript vem primeiro do
   `dart_sdk.js` que o `dartdevc` do próprio SDK gera do
   `ddc_platform.dill` (`scripts/gerar-dart-sdk.ps1`; verificado no Node
   24). A emissão do DartForge segue o **contrato de módulos do DDC**, e o
   `dartdevc` é o oráculo do contrato construto a construto. Compilar o SDK
   pela nossa fonte continua sendo o passo 4, com o mesmo contrato.
   Ver `docs/EMISSAO-DDC.md`.
5. **Emissão completa** — despacho dinâmico, reificação de tipos genéricos,
   `noSuchMethod`, `runtimeType`, `is`/`as` sobre qualquer tipo, tearoffs,
   `async*`/`sync*`, isolates conforme `docs/ISOLATES-WEB.md`. Aceite: os
   pontos de entrada do corpus executam no Node com a mesma saída do dart2js.

O tree shaking e a minificação continuam sendo o ganho legítimo do DartForge,
mas passam a ser **otimizações sobre um programa completo**, nunca recusas: um
receptor `dynamic` reduz o que se pode remover, e o compilador mede isso em vez
de proibir o programa.

## `dartforge dev` — o compilador residente (desenho fixado em 2026-09-22)

Depois de o emissor passar o corpus (correção primeiro), o próximo salto de
latência não está em `emitir_programa`, está antes dele: hoje cada chamada
refaz `load → outline → infer → emit` do programa inteiro. O desenho, na
linha de ReScript (interfaces `.resi` com hash), Scala.js
(`fastLinkJS` incremental por módulo) e Kotlin/JS (`per-module`):

1. **Sessão residente** — `dartforge dev` mantém vivos `Interner`,
   `TypeTable`, ASTs, outlines, `BodyTypes`, JS emitido e hashes por
   biblioteca; o processo não termina a cada edição.
2. **Dois hashes por biblioteca**: `hash_fonte` e `hash_api_publica`
   (o outline sem privados). Corpo mudou e API não → reanalisa e reemite
   **só** aquela biblioteca; API mudou → só os dependentes que a usam.
3. **Dependências por tipo**, não só "importa": `Import`, `Type`,
   `Inheritance`, `Constant`, `Runtime` — `class A extends B` invalida
   diferente de `print(B.x)`.
4. **SDK como artefato compilado**: além de `runtime/ddc/dart_sdk.js`, um
   outline serializado do SDK (`dart_sdk.dfi`: classes, membros,
   assinaturas, hierarquia, extensions) para não reanalisar as 36
   bibliotecas a cada início (hoje ~460 ms no `elements` + 155 ms no
   `types`).
5. **Escrever só o que mudou**: hash do JS anterior por módulo; `.mjs`
   igual não é reescrito (timestamp intacto, o Vite/HMR não recarrega).
6. Três níveis: `dev` (latência, sem otimização global), `build`
   (tree shaking, constantes, inlining local) e `build --release`
   (programa inteiro sobre a mesma IR: DCE global, desvirtualização,
   especialização, minificação). O caminho de desenvolvimento nunca
   recebe otimização de programa inteiro.

Referências fixadas para os dois modos (2026-09-22): **dev** copia a forma
do DDC (`references/dart-sdk/pkg/dev_compiler/lib/src/kernel/compiler.dart`
é o emissor de referência; a saída do `dartdevc` é o oráculo). **prod**
estuda o dart2js (`references/dart-sdk/pkg/compiler`: inferência global de
tipos, alcance, especialização) e o Scala.js (IR + linker + otimizador
global), com o ReScript como padrão de "como o JS final deve parecer":
`class Point { sum() { return this.x + this.y } }`, não
`runtime.add$int(this.x$1, this.y$1)`. Nenhuma otimização de produção
entra no caminho de desenvolvimento.

Meta observável no `new_sali` (80 mil linhas): alterar um método privado
→ parse de 1 biblioteca, tipos de 1 biblioteca, 1 `.mjs` reescrito,
dezenas de ms — medido pelo mesmo harness de `crates/instrument`.

## Latência medida contra a toolchain oficial — `new_sali/frontend`

Mesmo projeto (ngdart 8.0.0-dev.4, 284 arquivos do projeto, 3.183 unidades
no fecho com os pacotes, 2.970 bibliotecas), mesma máquina (8 núcleos,
8 GB, Windows com Defender ativo). Medido em 2026-09-22.

| Ferramenta | O que faz | Tempo |
| --- | --- | --- |
| `dart run build_runner build --delete-conflicting-outputs` | templates ngdart + SCSS + **compila tudo com o DDC** (9.879 artefatos, 205 MB) | **2 min 59 s** |
| `dart2js` de produção (`webdev build`) | JavaScript otimizado de programa inteiro | **~4 min** |
| **`dartforge compile-js` a frio** | 616 módulos ES6, cache do SDK construído, saída vazia | **6,5 s** |
| **`dartforge compile-js` morno** | idem, 0 arquivos reescritos | **4,3 s** |
| **`dartforge dev`, edição de corpo** | 1 biblioteca reanalisada, 1 módulo escrito | **335 ms** |

A comparação com o `dart2js` **não é de igual para igual** e não deve ser
apresentada como tal: ele faz análise de programa inteiro, tree shaking e
minificação (4,5 MB de saída), e nós emitimos o modo de desenvolvimento
sem otimização global (48 MB). A comparação honesta com o `dart2js`
existirá quando o modo de produção existir.

A comparação com o `build_runner` **é** de igual para igual em objetivo —
"deixar a aplicação pronta para abrir no navegador em desenvolvimento" —
e o fator vem sobretudo de **não executar** o `build_web_compilers`: dos
9.879 artefatos que ele gera, ~8.900 são do compilador DDC que o
DartForge substitui. Os 773 `*.template.dart` do ngdart ainda vêm do
`build_runner` (uma vez, enquanto a Fase 5 não existir); os 192 `.css`
do `sass_builder` idem. Ver [BUILD-RUST.md](docs/BUILD-RUST.md).

## Regra governante — compatibilidade obrigatória com o ecossistema Dart

**Registrada em 2026-09-22 por instrução do proprietário.** O compilador,
o runtime, o analisador e o LSP são **nossos** — a VM oficial não faz
parte do produto — e têm de ser **compatíveis com o ecossistema**:
`json_serializable`, `freezed`, `drift`, `mockito`, `source_gen`,
`build`, `analyzer`, `build_runner` e os demais pacotes que os projetos
reais usam. **Um projeto que compila com a toolchain oficial tem de
compilar com a nossa.** Não construímos um fork incompatível, e também
não dependemos da implementação oficial para funcionar.

O `dart` oficial tem **um** papel e só ele: oráculo de verificação, como
`dart run` já é para o compilador JavaScript (213 programas comparados
byte a byte). Oráculo é o que se compara, não o que se embute.

Consequência direta para o backend nativo: executar um builder do
ecossistema é **compilar e executar esse builder com a nossa pilha**.
O alvo dominante é o `package:analyzer` — 438 arquivos, 227.252 linhas,
usando `dart:io`, `dart:isolate`, `dart:ffi`, `dart:typed_data`,
`dart:collection`, `dart:async`, `dart:convert` e `dart:_internal`.
Compilá-lo e rodá-lo é o teste de maturidade da implementação, e é o que
ordena as prioridades do `crates/emit_native` (hoje ~6/202 do corpus).

É a mesma regra da meta governante da linguagem ("qualquer projeto Dart
3.6 válido"), estendida às ferramentas, e ela tem três consequências
duras:

1. **Nenhum gerador nativo é obrigatório.** Um gerador em Rust
   (`json_serializable`, `ngdart`, `sass`) é aceleração, e só entra
   quando produzir saída **byte a byte igual** à do builder oficial no
   corpus de compatibilidade. Sem isso, executa-se o builder Dart.
2. **Os builders rodam no nosso runtime.** Enquanto ele não alcançar um
   builder, a limitação é declarada e o projeto roda `dart run
   build_runner` à parte, como hoje com os `.template.dart` do ngdart —
   limitação conhecida, não arquitetura: nada no desenho depende do
   `dart`.
3. **`corpus/builders/`** — um projeto por gerador do ecossistema, com a
   saída do `build_runner` oficial gravada como referência — vem **antes**
   do motor. Sem ele, "compatível" é opinião, não medição.

Plano e fases: [BUILD-RUST.md](docs/BUILD-RUST.md).

## Geração de código em Rust — `dartforge build`

Plano e medições em [BUILD-RUST.md](docs/BUILD-RUST.md) (2026-09-22). O
`build_runner` sai do ciclo de desenvolvimento em fases: motor em Rust
(grafo, impressão digital, agendamento) com worker Dart persistente para
os builders do ecossistema; depois geradores nativos lendo o banco
semântico que o `dartforge dev` já mantém vivo. Medido no
`new_sali/frontend`: dos 9.879 artefatos do `build_runner`, ~8.900 são do
`build_web_compilers`, que **não executamos** — a compilação para
JavaScript é nossa. Restam três builders reais nos projetos do
proprietário: `ngdart` (que é a Fase 5 deste plano), `i18n` e
`sass_builder`. Compilar `package:analyzer` com o backend nativo para
rodar builders em LLVM é o marco final, não o primeiro passo: o backend
nativo está em ~6/202 do corpus básico.

## Regra de projeto — equivalência semântica com o Dart oficial

**O DartForge pode tornar código Dart padrão mais rápido, dividir workers
automaticamente e introduzir otimizações, mas não pode fazer um programa
semanticamente diferente do mesmo código compilado pelo Dart oficial.**

Essa regra é o que separa um compilador alternativo de um dialeto incompatível.
Um programador precisa poder trocar `forge compile` por `dart compile js` e
obter o mesmo comportamento — diferente apenas em desempenho e tamanho, nunca em
semântica.

### A consequência prática, com um exemplo concreto

Uma anotação **não pode** mudar onde o código executa. Considere:

```dart
@worker
Future<void> executar() async { contador++; }
```

Se o DartForge transformasse a chamada em execução num Worker e o dart2js a
executasse na thread principal, o mesmo programa teria duas semânticas: o
`contador` global chegaria a 1 num caso e continuaria 0 no outro, porque o
Worker tem memória isolada. Isso produz defeitos que só aparecem ao trocar de
compilador — exatamente o tipo de armadilha que esta regra existe para impedir.

Por isso **declaração e execução ficam separadas**:

```dart
@WorkerEntrypoint()
Future<PdfResult> processarPdf(PdfJob job) async { ... }

final resultado = await WorkerIsolate.run(processarPdf, job);
```

A anotação diz apenas *"esta função pode ser compilada como entrypoint de
worker"*. Quem determina a semântica de isolate é a chamada de API, que é a
mesma em todos os compiladores. Não há ambiguidade.

### Onde o DartForge pode ser melhor sem divergir

O ganho legítimo é de **compilação**, não de semântica: conhecendo a AST e o
grafo de dependências, o DartForge pode gerar um chunk de worker contendo apenas
o runtime mínimo e o fecho transitivo da função — em vez do modelo do Dart 1,
onde cada Worker carregava praticamente o programa inteiro. A biblioteca
portátil roda igual em dart2js e DDC; o que muda é o tamanho do que foi enviado.

### Nada de sintaxe nova

Nenhuma palavra-chave própria. `worker function f()` quebraria dart2js, DDC,
analyzer, IDE e `dart format` de uma vez, criando um fork da linguagem. Anotação
comum é metadata Dart válida que qualquer compilador aceita e ignora.

## Meta de projeto — cadeia de ferramentas própria, sem memória gerenciada

O Dart tem analyzer, compilador e servidor de linguagem escritos em Dart. A
consequência é medida em gigabytes: uma aplicação web compilada com `webdev`
chega com facilidade a 8 GB e trava a máquina. Temos obrigação de fazer melhor, e
o meio é construir **tudo do zero em Rust**: compilador, analisador, servidor LSP
e extensão de editor.

### A causa é o modelo de dados, não a velocidade da linguagem

Registrar isso importa porque decide o que fazemos diferente. O AOT do Dart está a
um fator pequeno de nativo; os 8 GB não vêm de throughput. Vêm de o element/AST
model ser um **grafo de objetos alocados individualmente**: cada nó com cabeçalho,
ponteiro para o element, ponteiro para o `staticType`, ponteiros para tokens, e
cada identificador como `String` própria. Milhões deles.

O que trava a máquina é o coletor, não o processamento. Um live set de vários GB é
o pior caso para um coletor geracional: nada morre, então toda coleção maior
percorre o grafo inteiro.

O corolário nos obriga: **uma implementação em Rust que refizesse o mesmo grafo de
ponteiros com `Rc<RefCell<…>>` também usaria gigabytes.** Não ganhamos por
escolher Rust — ganhamos por escolher arena, interning e índices. Rust apenas
torna essa escolha disponível. Quem tratar "é Rust" como garantia perde a meta.

Estado real hoje: o AST empresta `&'a str` da fonte em 54 pontos, então nunca
copia string de identificador — já é estruturalmente melhor que o analyzer no eixo
dominante. Interning/`SymbolId` e arena **não começaram**, bloqueados exatamente
nesses pontos de empréstimo. É o item que decide se a vantagem é real ou anedota.

### Precisão obrigatória: "sem coletor de lixo" vale para a ferramenta, não para o programa

O compilador, o analisador e o LSP não têm coletor. O **programa compilado tem**,
e é obrigado a ter: Dart tem coletor, e a regra de equivalência semântica exige
que o alvo o tenha também. `crates/runtime` já implementa um (`heap.rs:415`
`collect`, protocolo de raízes em `runtime_main.rs`); no alvo JavaScript o coletor
é o do motor.

Sem essa distinção registrada, alguém "otimiza" removendo o coletor do runtime e
quebra ciclos e finalizadores.

### Precisão obrigatória: a extensão do VS Code não pode ser Rust

O host de extensões do VS Code executa JavaScript. A forma correta, e a que o
rust-analyzer usa, é **cliente TypeScript fino** que só localiza e inicia o
binário (`vscode-languageclient`), com toda a lógica no **servidor Rust**.
Tentar "tudo em Rust" literalmente aqui gasta esforço contra a plataforma.

### O LSP é a peça mais difícil das quatro, por razão estrutural

Um compilador em lote monta a arena e joga tudo fora. Um LSP mantém modelo
**vivo e mutável** entre teclas, responde consultas e não pode bloquear — e é
justamente aí que arena incomoda: não se libera uma declaração de dentro de uma
arena. Foi essa pressão que produziu o desenho do analyzer do Dart.

A resposta conhecida não é "recompilar rápido": é computação incremental sob
demanda com memoização de consultas e interning, o desenho do **rust-analyzer** —
LSP de uma linguagem mais difícil que Dart, em Rust, com memória
incomparavelmente menor. É a prova de existência da meta, e está clonado em
`references/rust-analyzer`.

Duas lacunas concretas a atacar antes de qualquer transporte JSON-RPC:

1. `dartforge_compiler::compile` devolve `Result<String, Diagnostic>` — **um**
   erro. `crates/lsp/src/lib.rs` já expõe `diagnose() -> Vec<Diagnostic>`, e o
   parser agora recupera em fronteiras de declaração (três declarações
   quebradas rendem três diagnósticos) — **correção em 2026-09-21**: a frase
   antiga "o vetor nunca pode ter mais de um elemento" valia só para o
   passado; o gargalo restante são as fases pós-sintaxe (macros, mixins,
   semântica, emissão), que ainda param no primeiro erro. Um LSP precisa de
   **todos** os erros também da semântica.
2. Logo, **recuperação de erro na semântica é o pré-requisito restante** do
   LSP (o do parser foi cumprido). Sem ela o editor mostra um erro
   semântico por arquivo.

`crates/lsp` tem 22 linhas e documenta honestamente que não tem transporte,
JSON-RPC nem sincronização de documentos. Não existe extensão de editor ainda.

### Evidência de campo: o crescimento é monotônico, não um pico

Medição relatada em projeto real (`new_sali`, apenas o front-end):

* `webdev` chega a **~10 GB** durante a compilação;
* o **LSP do Dart vai a ~6 GB**;
* e o decisivo: **edições sucessivas fazem a memória só subir**, até ser preciso
  matar todos os processos Dart e reabrir.

Crescimento monotônico sob edição repetida não é "modelo grande em memória" — é
retenção. Cada edição deveria tornar inalcançável o estado da edição anterior. Se
não torna, alguma estrutura viva guarda referência ao modelo antigo: cache sem
teto, contexto de análise por versão que nunca é descartado, ou grafo de build
retendo saídas antigas.

Isso muda o que a nossa meta precisa **provar**, e o teste é diferente do
benchmark de compilação fria:

> Aplicar N edições sucessivas no mesmo arquivo e afirmar que `live_bytes` volta
> a um platô, em vez de crescer com N.

`crates/instrument` (`CountingAllocator`, `live_bytes`, `peak_bytes`) já mede
exatamente isso; falta o teste que o afirma. Sem ele, "usa menos memória" é
anedota — e é justamente a propriedade cuja ausência trava a máquina do usuário.

Nota de execução: `cargo bench -p dartforge-compiler --bench incremental`
terminou com código 101 e sem saída nesta tentativa. O número de memória do
DartForge **ainda não foi medido**; o benchmark precisa ser consertado antes de
qualquer comparação ser afirmada.

### Por que retenção é o problema difícil, e o que Rust de fato resolve

Os dois problemas coexistem e se multiplicam: modelo grande em memória **e**
retenção. O modelo grande fixa o piso; a retenção faz o piso subir a cada edição.

A vantagem de Rust aqui não é principalmente "libera mais cedo". É que **Rust
torna a retenção uma decisão visível e tipada, em vez de um acidente invisível**.
Para segurar um snapshot de análise em Rust foi preciso guardar um `Arc` em algum
lugar — está escrito no tipo, e aparece na revisão. Em Dart, qualquer campo de
qualquer objeto vivo pode manter um grafo de gigabytes alcançável, e nada no tipo
mostra isso. É por isso que essa classe de defeito é difícil de *achar*, não só de
consertar.

Soltar a última referência em Dart torna a memória *elegível* para coleta em
algum GC futuro, e só se não houver mesmo nenhuma outra referência. Descobrir que
havia é trabalho de heap dump, não de compilador.

#### O contra-exemplo que nos impede de simplificar

"GC é a causa" não sobrevive inteiro à evidência. O `tsserver` do TypeScript é
escrito em TypeScript e sofre do mesmo mal; o `gopls` é Go e teve problemas de
memória notórios. E a equipe do TypeScript escolheu **Go** para a reescrita
(`references/typescript-go`) — que também tem coletor.

Ou seja: a maior parte do ganho vem de linguagem compilada com tipos de valor
baratos e de um modelo de dados compacto. O que Rust acrescenta sobre Go é a
visibilidade da retenção, e é exatamente o eixo em que o Dart falha pior.

#### E onde Rust não ajuda, que precisamos planejar

Um LSP precisa de modelo compartilhado, mutável e de vida longa. Ownership torna a
retenção visível, mas torna o compartilhamento incômodo — por isso o
rust-analyzer usa salsa (interning, memoização de consultas, snapshots em `Arc`)
em vez de ownership ingênuo. E o cache de consultas do salsa **cresce**, e precisa
de despejo por LRU.

Conclusão para o plano: escrever em Rust não previne o modo de falha que trava a
máquina do usuário. Só o torna detectável. Portanto o teste de platô — N edições
sucessivas e `live_bytes` estabilizando — não é refinamento posterior, é a
verificação que substitui a garantia que a linguagem não dá.

## O que falta para não repetir webdev, analyzer e LSP do Dart

Lista de trabalho, não de princípios. Cada item nomeia **a qual falha observada
ele responde** e **como se verifica** — item sem verificação não conta como
feito, porque "usa menos memória" sem teste é anedota.

### Ponto de partida: o que já está certo, e não pode regredir

Três propriedades existem hoje e são a razão de o DartForge ainda não ter o
problema. Precisam de teste que as trave:

* `CompilerSession` retém **exatamente um** snapshot (`cached: Option<Cached>`,
  `session.rs:45`), com orçamento de 16 MiB. Uma compilação substitui a anterior,
  então o lote é à prova de crescimento **por construção**.
* O cache de macros já é LRU com limite duplo — entradas e bytes — e expõe
  `evictions` (`macros/src/cache.rs:30`, `with_limits`). É o padrão a copiar.
* A expansão de macro acontece **em memória**; não escreve `.g.dart`. O ciclo do
  `build_runner` — gerar arquivo, reanalisar o arquivo gerado — não existe aqui.
* O AST empresta `&'a str` da fonte; nenhuma string de identificador é copiada.

### 1. Teste de platô — o portão de tudo

**Falha a que responde:** memória subindo a cada edição até ser preciso matar os
processos Dart.

Aplicar N edições sucessivas e afirmar que `live_bytes` estabiliza num platô em
vez de crescer com N. `crates/instrument` já mede (`live_bytes`, `peak_bytes`,
`reset_peak`); falta o teste. Deve cobrir as três formas de edição que invalidam
coisas diferentes: corpo de função, assinatura e import.

**Sem este teste, nenhum outro item desta lista pode ser declarado concluído.**

Estado em 2026-09-21: existe e passa — `crates/compiler/tests/plato_memoria.rs`
(`edicoes_sucessivas_estabilizam_em_plato`, 60 edições alternando corpo,
assinatura e import, tolerância de 8 KiB sobre o platô de referência).

### 2. Teto e despejo em todo cache que sobreviva a uma requisição

**Falha a que responde:** cache sem teto é a causa mecânica mais comum de
retenção monotônica.

`DiskCache` (`compiler/src/disk.rs:43`) **não tem limite de entradas**. Cada cache
novo nasce com teto, política de despejo e `hits`/`misses`/`evictions`
observáveis — o cache de macros já faz assim e serve de molde.

**Verificação:** encher além do teto e afirmar que o tamanho não passa dele.

Estado em 2026-09-21: feito — teto padrão de 256 registros, despejo do mtime
mais antigo (LRU aproximado; acerto atualiza recência), contadores
compartilhados entre clones via `Arc` (a sessão guarda um clone, então as
expulsões que ela provoca aparecem na alçada do teste), `DiskCacheStats`
exportado. Verificação em `crates/compiler/tests/teto_disco.rs` (3 testes,
incluindo o LRU tocar-A-expulsa-B). Detalhe que quebrou o primeiro teste:
`Clone` copiava os contadores, e a sessão incrementava a cópia dela.

### 3. O desenho de snapshot único não escala para o LSP

**Falha a que responde:** o LSP do Dart a 6 GB.

`Option<Cached>` resolve o lote porque há uma compilação por vez. Um LSP tem N
arquivos abertos × M versões. É exatamente aí que a retenção entra, e o desenho
atual não diz quem descarta a versão anterior.

Precisa de dono explícito por documento e descarte determinístico da versão N−1
quando N chega. **Verificação:** abrir K documentos, editar cada um K vezes,
afirmar platô — o teste do item 1 aplicado ao servidor, não ao compilador.

Estado em 2026-09-21: o dono existe — `DocumentStore` em `crates/lsp`
(uma entrada por documento, chegada da versão N substitui a N−1 no lugar,
versão menor ou igual ignorada, `close` remove; sincronização integral por
enquanto, incremental por intervalos pendente). Verificação K×K feita —
`crates/lsp/tests/plato_documentos.rs`: 24 documentos × 24 edições com
diagnóstico a cada uma, `live_bytes` em platô (tolerância 8 KiB); fechar os
16 de uma segunda fase devolve ao nível inicial, e o `drop` do servidor volta
ao byte exato de antes (3.847 → 3.847). Detalhe que enganou a primeira
versão: o alocador contador é global ao processo, e o harness libera as
estruturas de um `#[test]` terminado em momento não determinístico — dois
testes no mesmo binário mediam um ao outro, e `Mutex` não resolve porque a
liberação ocorre fora da região travada. As duas fases vivem num só `#[test]`.

### 4. Interning e `SymbolId`

**Falha a que responde:** o piso de memória do modelo, não o crescimento.

Não começou. O AST não copia strings, o que já evita o pior; falta colapsar
identificadores repetidos na camada semântica em índice de 4 bytes. Os 54 pontos
que emprestam `&'a str` em `crates/syntax` definem a fronteira do trabalho.

**Verificação:** `live_bytes` do modelo semântico antes e depois, no corpus real.

Estado em 2026-09-21: a facilidade existe — `crates/intern` (`SymbolId` de 4
bytes `Copy`, `Interner` com `payload_bytes`, 4 testes) — mas a migração dos
54 pontos (lexer/parser → semântica/HIR/codegen) não começou; o crate documenta
a fronteira.

### 5. Recuperação de erro no parser

**Falha a que responde:** editor que mostra um erro por arquivo.

Pré-requisito do LSP: `compile` devolve `Result<String, Diagnostic>` — um erro —
enquanto `crates/lsp` já promete `Vec<Diagnostic>`. Em andamento.

Estado em 2026-09-21: recuperação no parser pronta (fronteiras de declaração,
3 erros independentes saem com spans próprios, sem cascata, entradas
patológicas terminam; `diagnose()` multi-erro). A semântica ainda para no
primeiro erro — o gargalo restante é semântica multi-erro, não mais o parser.

### 6. Transporte do LSP

Não existe: `crates/lsp` tem 22 linhas e documenta que não tem JSON-RPC nem
sincronização de documentos. Precisa de `didChange` **incremental** — reenviar o
arquivo inteiro a cada tecla reintroduz o custo que se quer evitar.

Estado em 2026-09-21: `crates/lsp` tem `diagnose()` multi-erro + `DocumentStore`
(ver item 3); transporte JSON-RPC, `didChange` incremental com UTF-16 correto e
cancelamento continuam inexistentes.

Estado em 2026-09-22: transporte pronto — `crates/lsp/src/` tem
`transporte.rs` (quadros `Content-Length` por stdio, leitora + despacho,
`stdout` só protocolo), `utf16.rs` (tabela de linhas por documento,
recalculada do ponto editado em diante), `servidor.rs` (`Servidor` dono de
tudo, fila com `$/cancelRequest` determinístico, `textDocumentSync:
incremental`, `publishDiagnostics` com `source: "dartforge"`) e
`trait Analisador` com `AnalisadorSintatico` (parser novo; a semântica entra
sem tocar no transporte). Aceite em `tests/protocolo.rs` (4 testes pelo fio
em processo filho) + `tests/plato_protocolo.rs` (K=24 × K=24 reais pelo
protocolo, binário separado). Detalhe em `docs/LSP.md`.

### 7. Consultas sob demanda com memoização, e despejo delas

**Falha a que responde:** recompilar o mundo a cada tecla, que é o que força o
modelo inteiro a ficar vivo.

Desenho do rust-analyzer (`references/rust-analyzer`): interning, memoização por
consulta, snapshots compartilhados. Com a ressalva já registrada — **o cache de
consultas cresce** e precisa de despejo LRU, senão trocamos um vazamento por
outro.

Mapeamento fechado em 2026-09-21 (verificado nos clones; não é analogia
livre). Onde o analyzer do Dart retém
(`references/dart-sdk/pkg/analyzer/lib/src/dart/analysis/`):

* `driver.dart:248` — `_resolvedLibraryCache: Map<String, …>` **sem teto**
  (só `.clear()` em L569); `driver.dart:245` — `_priorityResults` idem;
* `file_state.dart` — cada edição gera nova `_unlinkedKey`/assinatura, mas as
  entradas antigas permanecem nos mapas; `FileContentCache` e
  `UnlinkedUnitStoreImpl` sem despejo determinístico por versão;
* `summary2/` (link, element_builder, ~40 arquivos) — element model como
  grafo de objetos com identidade por referência, não por índice.

Peças do salsa do rust-analyzer que se aplicam a `crates/semantic` +
`crates/lsp` (verificado em `references/rust-analyzer/crates/`):

* `base-db/src/input.rs` + `lib.rs` — `#[salsa::input]` para texto/raízes,
  `#[salsa::interned]` para identidades (Crate), `#[salsa::tracked]` para
  consultas memoizadas retornando `Arc`; `Durability` separa o que muda por
  tecla (baixa) do que é quase imutável (bibliotecas, alta);
* `base-db/src/change.rs` — `FileChange` transacional: N edições aplicadas de
  uma vez, invalidação calculada pelo salsa, não à mão;
* `intern/` — interning global com `gc.rs` (`GarbageCollector` explícito):
  prova de que interning sem coleta é outro vazamento; `SymbolId` do
  DartForge precisa do par (internar, coletar);
* `vfs/` (`path_interner.rs`, `file_set.rs`) — identidade de arquivo por
  índice, nunca por `String` de caminho;
* `rust-analyzer/src/` (`global_state.rs`, `mem_docs.rs`, `main_loop.rs`,
  `op_queue.rs`) — documentos em memória versionados, fila de operações com
  cancelamento, estado global dono dos snapshots.

Aplicação, em ordem: (i) `SymbolId` + arenas por unidade (desbloqueia os 54
pontos `&'a str`); (ii) resumo da biblioteca separado dos corpos, com
impressão que interrompe invalidação (o `unlinkedKey` do analyzer, mas com
despejo); (iii) consultas memoizadas por (arquivo, revisão) com LRU; (iv)
dono explícito por documento no LSP descartando N−1 quando N chega.
Verificação de cada peça: o teste de platô do item 1.

### 8. Emissão modular por biblioteca

**Falha a que responde:** os 10 GB do `webdev`, que vêm de tratar o programa como
uma unidade.

Um módulo JavaScript por biblioteca Dart: recompilar e reter uma biblioteca em
vez do programa. Não começou.

### 9. Extensão do VS Code

Não existe. Cliente TypeScript fino que só localiza e inicia o binário; toda a
lógica no servidor Rust.

Estado em 2026-09-22: existe — `editors/vscode/` (`package.json`,
`tsconfig.json`, `src/extension.ts`, `README.md`): só localiza o binário
(`dartforge.serverPath`, senão `PATH`) e o inicia com `--stdio` via
`vscode-languageclient`; ativa em `onLanguage:dart`; `npm run compile`
limpo. Verificação funcional em `docs/LSP.md` (arquivo real do `new_sali`
com erro inserido rende o diagnóstico pelo fio).

### 10. Medir contra o alvo real, não contra nós mesmos

A comparação honesta é RSS do DartForge contra `webdev` e contra o LSP do Dart
**no mesmo projeto**, com a mesma sequência de edições. Projeto de referência
relatado: `new_sali` (front-end), onde o `webdev` chega a ~10 GB e o LSP a ~6 GB.

Pendência registrada: `cargo bench -p dartforge-compiler --bench incremental`
terminou com código 101 e sem saída. **Nenhum número de memória do DartForge foi
medido ainda**, e nenhuma comparação pode ser afirmada antes disso.

**Alvo de referência fixado em 2026-09-21 por instrução do proprietário:
`C:/MyDartProjects/new_sali`** — ngdart no front-end, angel3 no back-end,
1.258 arquivos `.dart`, 8,52 MiB de fonte. Ninguém aceita que 10 MB de Dart
virem 10 GB de RAM para compilar ou para o LSP no VS Code; memória e tempo
de compilação são critério permanente, e o compromisso é ser melhor que
dart2js, DDC e o LSP do Dart **neste projeto**, medido.

Primeira medição do front-end novo sobre o `new_sali` inteiro
(`cargo run --release -p dartforge-frontend --example memoria --
C:/MyDartProjects/new_sali`, alocador contador, **todas as árvores retidas**
como num editor com o projeto aberto): 1.258/1.258 arquivos aceitos, 254 ms
(33,6 MiB/s), **132 MiB vivos** com tudo retido — 15,5× a fonte —, pico
igual ao vivo (nada transitório sobra), 367 mil alocações, 18.583 símbolos
internados em 0,25 MiB. Contra os ~6 GB do LSP do Dart é 45× menos, mas o
número honesto a perseguir é o 15,5×: os nós da arena são gordos
(`Expr` 120 bytes, `Stmt` 96, `TypeAnnotation` 80, `Decl` 224, `Member`
248, `CollectionElement` 88), e é aí que o modelo de dados ainda paga —
`Vec` e `Box` dentro de variantes de enum e `Span` de 16 bytes em cada nó.
Reduzir o nó é trabalho de representação, medido por esta mesma linha de
comando antes e depois. A medição encontrou e corrigiu uma recusa real do
parser (record nomeado dentro de `<...>` em função local), o que confirma
que o projeto do proprietário é corpus de aceite, não só de memória.

Sequência medida em 2026-09-21/22, sem mudar a API de leitura da AST
(quem consome `iter`/`len`/índice não muda de código): `shrink_to_fit` das
arenas ao fim do parse, 132 → 103 MiB (29 MiB eram folga do `Vec`
dobrando); `Box<Arguments>` em `Call`/`InstanceCreation`, `Expr` 120 → 72
bytes, 103 → 88; `Box<[T]>` de tamanho exato em `Arguments` e `StringLit`
via rascunho compartilhado no parser (uma alocação por lista), 88 → 81;
`SymbolId` com nicho (`NonZeroU32`) — `Option<Name>` 32 → 24 — e as listas
de `ExprKind` em `Box<[T]>`, `Expr` 72 → 56, 81 → 72; as 49 listas de alto
volume da AST em `Box<[T]>`, `Stmt` 96 → 88, `Member` 240 → 216, 72 →
**62,75 MiB (7,4× a fonte), 212 ms**. Pendente com ganho estimado: `Span`
de `usize` para `u32` (8 bytes por nó e por `Name`, ~6–8 MiB) exige tocar
~250 sítios inclusive na trilha antiga — fica para quando ela for removida;
`CollectionElement` de 88 bytes; `Parameter` de 136.

Regra de trabalho aprendida a custo: com três agentes na mesma árvore, um
estado intermediário que não compila é revertido por quem "conserta o
build" com `git checkout` — foi assim que uma conversão inteira sumiu às
23:49. Mudanças de representação do front-end são feitas numa **worktree
própria** (`git worktree add -b <ramo> D:/Projects/dartforge-frontend main`),
commitadas lá, rebaseadas e integradas em `main` por fast-forward só quando
compilam com todos os consumidores.

Estado em 2026-09-21: **desbloqueado** — a causa era `nucleo_rotulo` privado
(E0624) num módulo que o bench usa. Primeira medição (25 unidades, ~38 KiB de
fontes): cenário `assinatura` 4,86 ms de mediana (p95 5,62 ms), 15.834
alocações/execução, `live_bytes_growth` 839, pico de 3,25 MiB vivos; acerto de
cache (`sem_edicao`) 0,39 ms e 124 alocações. São números do harness, não
comparação com `webdev` — a comparação continua bloqueada até o teste de platô
do LSP existir.

Medição do LSP em 2026-09-22 (`scripts/medir-lsp.ps1`, `docs/LSP.md`):
mesma sequência nos dois servidores sobre o `new_sali` (1.258 `didOpen` +
200 edições, RSS a cada 50 mensagens) — DartForge: pico 21,3 MiB, platô
19,0 MiB, 2,5 s, 1.460 quadros com diagnóstico por abertura e edição;
Dart (`dart language-server --protocol=lsp`): pico 640,5 MiB, platô
633,7 MiB, nada publicado em 2 min (sem `package_config` a resolução fica
prejudicada, ou a análise nunca alcança 1.258 arquivos de uma vez). O 6 GB
do relato de campo não foi reproduzido aqui; o afirmado é só o medido:
in-process, 8,52 MiB de fonte → 19,94 MiB vivos (2,34×) com tudo retido, e
o teste `plato_pelo_protocolo` trava o platô sob edição repetida.
