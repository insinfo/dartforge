# ngdart numa dependência

`app/` usa um componente de `dep/` (dependência `path`). O `oraculo/` tem os
`.template.dart` que o `build_runner` oficial (ngdart 8.0.0-dev.4, lock de
`app/pubspec.lock`, com `build_web_compilers`, que é quem pede as saídas
opcionais do ngdart) gravou em `.dart_tool/build/generated/<pacote>/` para os
dois pacotes. Regenera-se com `scripts/corpus-ngdart-dependente.sh`.
Teste: `crates/build/tests/ng_transparencia.rs`
(`ng_dependencia_pelo_motor_igual_ao_oraculo`).
