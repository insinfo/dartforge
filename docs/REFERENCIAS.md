# Referências de código

Os clones locais são shallow (--depth 1) com histórico limitado e working tree disponível.
--filter=blob:none reduz histórico transferido, mas o checkout baixa os arquivos necessários.
Não são dependências do workspace. Não executar scripts dos clones automaticamente.
Preservar LICENSE/NOTICE e atribuição ao reutilizar código ou testes.

| Pasta | Repositório | Uso |
|---|---|---|
| dart-sdk | https://github.com/dart-lang/sdk | Linguagem, compilers, analyzer, analysis server, runtime e testes |
| dart-language | https://github.com/dart-lang/language | Especificação e propostas da linguagem |
| oxc | https://github.com/oxc-project/oxc | AST, arenas, parser, codegen, minificação |
| swc | https://github.com/swc-project/swc | Transformações e emissão JavaScript |
| biome | https://github.com/biomejs/biome | Parser recuperável, diagnostics, analyzer e organização de ferramentas |
| rolldown | https://github.com/rolldown/rolldown | Grafo de módulos e bundling |
| rspack | https://github.com/web-infra-dev/rspack | Build graph, cache e compilação incremental |
| ngdart | https://github.com/angulardart-community/angular | Compilação de templates e runtime AngularDart |
| ngdart-8.0.0-dev.4 | https://pub.dev/packages/ngdart/versions/8.0.0-dev.4 | Snapshot publicado do alvo inicial |
| webdev | https://github.com/dart-lang/webdev | Fluxo de desenvolvimento e integração com DDC |
| dart-linter-archived | https://github.com/dart-archive/linter | Histórico de regras de lint; arquivado |
| dart-lints-archived | https://github.com/dart-archive/lints | Histórico das configurações de lint; arquivado |
| dart-core | https://github.com/dart-lang/core | Pacotes core atuais, incluindo configurações lints |
| typescript-go | https://github.com/microsoft/typescript-go | Port em Go, repositório de staging agora arquivado |
| typescript | https://github.com/microsoft/TypeScript | Continuação atual do compilador TypeScript nativo em Go |
| angular | https://github.com/angular/angular | Compilador oficial Angular/TypeScript, templates, DI e análise |
| oxc-angular-compiler | https://github.com/voidzero-dev/oxc-angular-compiler | Compilador experimental de templates Angular em Rust/Oxc |

## Dart Analyzer e lint

Analyzer não precisa de um clone separado: sua implementação está no monorepo dart-sdk.
O mesmo clone contém o language server e a implementação atual do linter.
Os antigos URLs dart-lang/linter e dart-lang/lints redirecionam para dart-archive e estão
arquivados. Não usá-los como fonte principal de compatibilidade atual.

Caminhos para estudar no SDK:

- pkg/analyzer/lib e pkg/analyzer/test: análise, tipos, resolução e testes.
- pkg/_fe_analyzer_shared: estruturas compartilhadas de frontend.
- pkg/analysis_server/lib e pkg/analysis_server/test: serviços de edição e LSP.
- pkg/linter/lib e pkg/linter/test: regras de lint e seus testes.
- pkg/front_end: frontend e diagnósticos.
- pkg/compiler: dart2js, otimizações e testes específicos.
- pkg/dev_compiler: DDC e compilação para desenvolvimento.
- pkg/kernel: representação intermediária Kernel.
- sdk/lib: contratos e implementação das bibliotecas padrão.
- tests/language e tests/corelib: conformidade de linguagem e bibliotecas.
- tests/lib: testes de APIs adicionais.
- tools: infraestrutura de execução; diversos testes exigem um SDK construído/configurado.

No dart-core, consultar pkgs/lints para as configurações atuais. Isso é diferente de implementar
as regras do linter.

Não executar “todos os testes Dart” diretamente com cargo test: selecionar/adaptar fixtures,
preservar metadados e construir um runner diferencial. Testes dependentes da VM, plataformas
não web ou flags internas precisam de classificação explícita.

## Versão alvo

O alvo inicial é Dart 3.6.2, definido pelo proprietário para ngdart. O executável identificado no PATH informa 3.6.2 e corresponde ao alvo. O clone dart-sdk foi baixado da branch padrão e ainda não foi alinhado a essa versão. Para testes normativos e comparação com Analyzer/DDC/dart2js, selecionar e registrar a revisão correspondente a 3.6.2. Código mais recente serve apenas como referência de arquitetura até essa seleção.

## Snapshots

manifest.json registra URLs e commits efetivamente baixados.
O pacote ngdart publicado é preservado também como tar.gz; não equivale necessariamente ao HEAD.
Para atualizar uma referência, escolher revisão deliberadamente e atualizar o manifesto.

## TypeScript em Go e compiladores Angular

- typescript-go/internal: scanner, parser, binder, checker, compiler, printer, transformers,
  project e lsp. testdata contém fixtures. O README indica migração para microsoft/TypeScript.
- typescript/tsc/internal e typescript/tsc/testdata: continuação atual, separada do snapshot arquivado.
- angular/packages/compiler: parsing e compilação de templates.
- angular/packages/compiler-cli: integração com TypeScript e compilação AOT.
- angular/packages/language-service: análise para editores.
- oxc-angular-compiler/crates/oxc_angular_compiler: implementação em Rust.
- oxc-angular-compiler/crates/angular_conformance: estratégia de testes de equivalência.

Angular para TypeScript e AngularDart/ngdart têm contratos e runtimes diferentes. Usar esses
compiladores como referência de arquitetura e testes não os torna diretamente compatíveis
com ngdart 8. O compilador Oxc Angular se declara experimental e busca mantenedores.

TypeScript/TypeScript-Go preservam Apache-2.0; Angular e Oxc Angular preservam MIT.
A licença MIT do DartForge não substitui as licenças dos fontes clonados.
## Rust Sitter

Clone: references/rust-sitter — https://github.com/hydro-project/rust-sitter — licença MIT.
Referência para gramáticas anotadas, AST e parsing. Avaliação: docs/RUST-SITTER.md.
O parser atual do DartForge continua próprio; nenhum código do Rust Sitter é compilado nele.

## Swift e Swift para JavaScript/WebAssembly

Clones `swift`, `swiftwasm-swift`, `swift-to-js` e `shift-js` em `references/`.
Veja [revisões, licenças, fontes consultadas e decisões](SWIFT-REFERENCIAS.md).
SwiftWasm é uma organização; seu clone local usa `swiftwasm/swift`, branch `swiftwasm`.
Os diretórios continuam ignorados pelo Git e não entram no build do workspace.
