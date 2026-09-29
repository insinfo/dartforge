# Casos do motor de build em versões novas do `build_runner`

Fora de `corpus/builders` porque o `pub get` exige Dart ≥ 3.11 (o CI resolve
aquela pasta com o 3.6.2). O oráculo de cada configuração
(`oraculos/<nome>/`, listadas em `configuracoes.json`) é o do
`build_runner` oficial da versão do lock, gerado por

    python3 scripts/corpus-perfis.py corpus/builders_novos/perfil_2_16 --dart <dart 3.13>

`perfil_2_16`: `build_runner` 2.16.1 / `build_config` 1.3.3 — `triggers` de
anotação (direta, com prefixo, construtor nomeado, membro, parte escrita à mão
e parte gerada), de import (`package:`, relativo, `export`, e o trigger que o
pacote de apoio acrescenta), `run_only_if_triggered` desligado no alvo e por
`--define`, builder sem trigger, `enabled: false`, e as configurações
padrão, `--release`, `--define`, `--config alt` e `--build-filter` (com
`asset:`). O teste é `crates/build/tests/perfis.rs`.
