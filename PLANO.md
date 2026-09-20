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
A linguagem, as bibliotecas, o Analyzer e os baselines DDC/dart2js devem seguir Dart 3.6.2. Fixar também o pacote ngdart e as revisões dos testes antes de ampliar suporte.
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

## Próximas tarefas

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
