# Especificação do ngdart (compilador AngularDart)

Especificação do compilador oficial — `ngast` 3.0.0-dev.2 e `ngcompiler`
3.0.0-dev.3 — escrita a partir da leitura do código-fonte dele, para o porte
byte a byte em `crates/gerador_ng`. Cada regra cita arquivo:linha do Dart,
traz exemplo de entrada e saída (os marcados "oráculo" conferidos contra
`corpus/ngdart/oraculo`, os "derivado" tirados só do código) e diz o estado
no porte (coberto, parcial, recusado, falta). Cada seção termina com as
lacunas do porte em ordem de prioridade.

| Seção | Conteúdo |
|---|---|
| [01 — Metadados e DI](01-metadados-e-di.md) | `@Component`/`@Directive`/`@Pipe`, entradas/saídas e herança, `@HostBinding`/`@HostListener`, consultas, ganchos, provedores, tokens, dependências do construtor, `@GenerateInjector` |
| [02 — Parser de template](02-parser-de-template.md) | tokenização, parse, desaçúcar (`*`, banana, microssintaxe), espaços, seletores, atributos × ligações, esquema DOM (tabelas completas), referências, erros |
| [03 — Ligações e expressões](03-ligacoes-e-expressoes.md) | gramática, conversão para Dart, `isImmutable`/`canBeNull`/`_TypeResolver`, interpolação, alvos de ligação e o código de cada um, eventos, pipes |
| [04 — Compilação de visões](04-compilacao-de-visoes.md) | forma do arquivo gerado, numeração de nós e campos, fases do `build()`, atributos, projeção, embutidas, detecção e ciclo de vida, OnPush, genéricos |
| [05 — Provedores na visão](05-provedores-na-visao.md) | resolução por nó (ansioso/preguiçoso, ciclos, apelidos), embutidos, `@Self`/`@Host`/`@SkipSelf`/`@Optional`, `injectorGetInternal`, hospedeira |
| [06 — Consultas](06-consultas.md) | `@ViewChild`/`@ContentChild`: estática × dinâmica, campos sujos, `read:`, visões embutidas, ordem das atualizações |
| [07 — Emissor, imports e estilos](07-emissor-imports-estilos.md) | ordem e forma dos imports, formatação do emissor e do `dart_style`, arquivo trivial, `.css.dart`/`.css.shim.dart` e o `ShadowCss` |
| [10 — Casos do ngcomponents](10-casos-do-ngcomponents.md) | a regra oficial por trás de cada recusa do placar do ngcomponents e o que o porte faz |

Regra de trabalho: uma forma nova no gerador começa por esta especificação
(e, se a regra não estiver aqui, pela leitura do código oficial, que então
entra aqui); a sonda do corpus confere o resultado, não o descobre.
