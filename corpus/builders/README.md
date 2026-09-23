# Corpus de compatibilidade do motor de build

Cada pasta é um pacote Dart pequeno que usa um gerador real. O `oraculo/` de
cada caso é o que o **build_runner oficial** produziu com o SDK 3.6.2 e as
versões fixadas no `pubspec.lock` do caso; o motor do DartForge tem de
reproduzir o mesmo byte a byte. Regenera-se com
`scripts/corpus-builders.ps1 [-Caso x] [-Atualizar] [-Limpar]` (um caso por
vez; `-Atualizar` regrava o lock; `-Limpar` apaga `.dart_tool/build` e as
saídas source da árvore depois de gravar).

## Forma de um caso

```
<caso>/
  pubspec.yaml, pubspec.lock   lock commitado = versões fixadas
  build.yaml                   quando o caso exercita configuração
  .gitignore                   .dart_tool/, build/ e as saídas build_to: source
  lib/**, bin/main.dart        main imprime saída determinística
  oraculo/
    manifesto.json
    plano.dart                 cópia de .dart_tool/build/entrypoint/build.dart
    source/<caminho>           saídas build_to: source (caminho relativo ao pacote)
    cache/<pkg>/<caminho>      .dart_tool/build/generated/<pkg>/<caminho>, filtrado
    saida.txt                  stdout de `dart run bin/main.dart` (se executável)
  edicoes/NN-nome/<caminho>    arquivos completos que substituem os do caso,
                               aplicados em sequência (cumulativos) — teste incremental
```

As saídas source **não** ficam versionadas nas pastas de fonte (`lib/`,
`test/`): só em `oraculo/source/`. `oraculo/**` é `-text` no
`.gitattributes` para o git não mexer nos bytes.

`manifesto.json` (UTF-8, 2 espaços, chaves nesta ordem, saídas em ordem
ordinal de `asset`):

```json
{
  "dart": "3.6.2",
  "pacotes": { "build_runner": "2.4.15", "...": "..." },
  "saidas": [
    { "asset": "<pacote>|<caminho>", "build_to": "source" | "cache", "sha256": "<hex minúsculo>" }
  ],
  "excluidos": 3,
  "executavel": true
}
```

- `pacotes`: versões do lock da infraestrutura (analyzer, build, build_config,
  build_resolvers, build_runner, build_runner_core, dart_style, sass,
  source_gen — quando presentes) e dos geradores/anotações de que o caso
  depende diretamente. Ordem alfabética.
- Saídas `cache`: tudo em `.dart_tool/build/generated/**` exceto
  `*.transitive_digest` (build_resolvers) e artefatos de
  build_web_compilers/build_modules; `excluidos` conta os tirados. O asset é
  o AssetId: `generated/<pkg>/<caminho>` → `<pkg>|<caminho>`.
- Saídas `source`: arquivos da árvore do pacote escritos pelo build (achados
  por comparação da árvore antes/depois de um build limpo).
- Casos sem build_runner não têm `plano.dart`.

## Casos

| caso | o que exercita | executável |
|---|---|---|
| `json_serializable` | json_serializable → `.json_serializable.g.part` (cache) → combining_builder → `.g.dart` | sim |
| `mockito` | mockito 5.4.4 (analyzer 6.x), `@GenerateMocks`/`@GenerateNiceMocks` em `test/`; `build_extensions` nas opções troca as extensões de execução (`^test/{{}}_test.dart` → `test/mocks/{{}}_test.mocks.dart`) — diferentes das do build.yaml do mockito | sim (bin/main.dart roda os "testes" sem package:test) |
| `freezed` | freezed 3 + json_serializable na mesma classe: `.freezed.dart` (source) antes do json_serializable (cadeia) | sim |
| `built_value` | built_value_generator → `.built_value.g.part` → combining_builder | sim |
| `drift` | drift_dev: builder de três fábricas, preparing_builder (`.drift`), cleanup | não (sqlite3 por FFI) |
| `riverpod_generator` | riverpod_generator 2.6 em Dart puro | sim |
| `sass_builder` | sass_builder 2.2.1 em dev (sourceMaps → `.css` e `.css.map` no cache), parciais, `@use`, `@import`, `@media`, `.sass` | não |
| `sass_builder_compressed` | o mesmo com `outputStyle: compressed` nas opções do alvo (forma do new_sali/frontend) | não |
| `cadeia_configuracao` | builders locais (`import: 'tool/builders.dart'`) e um pacote de apoio por caminho: `runs_before`, `required_inputs`, `applies_builders`, `enabled: false`, `generate_for` include/exclude, `global_options` (options, dev_options, runs_before), build_to source/cache, builder com duas fábricas, extensões `{{}}` e `^`, `auto_apply` none/dependents/all_packages/root_package, `defaults` com generate_for/options/dev_options/release_options, post_process_builder. Cada saída traz `options.config` (JSON, chaves ordenadas) e `options.isRoot` | sim |
| `sem_builders` | controle do custo zero: sem build_runner, sem build.yaml; `saidas: []` | sim |

Fora do corpus por decisão do proprietário: `go_router_builder` (exige Flutter).
