
#### E.3.8 Exemplos mínimos (oráculo vivo 3.6.2)

Saída integral do `dart analyze --format=json` de cada arquivo (todos os diagnósticos, ordenados por
offset; `⏎` = quebra de linha na entrada). Os arquivos estão em
`E:\dftemp\analise\spec-r4\casos\e3\{a,b,c}\<caso>.dart`; `res-a.txt`, `res-b.txt`, `res-c.txt` trazem
os 338 casos rodados (os citados no texto que não estão na tabela estão lá). Leitura das cascatas:
- `c05`: `+` solto → operador sem `operator` → sem parâmetros → sem corpo; o
  `wrong_number_of_parameters_for_operator` é do verificador sobre a árvore recuperada.
- `c10`: o segundo `;` no topo é `unexpected_token`, não `expected_executable`.
- `c14`: `final` seguido de `var` não aciona o `ModifierContext` (§E.3.4): campo `final` sem nome.
- `c23`: `augment` não é palavra-chave sem `macros` → campo sem tipo `augment`, depois `class X {}`.
- `c24`: enum sem corpo relata no token seguinte (EOF, length 0), ao contrário de classe (`c06`).
- `d37`: `enum const E3 { a }` no 3.6.2 — `const` não é nome de enum (`missing_identifier`), o corpo
  falta (`missing_enum_body`), e `const E3 { a }` é lido como membro de topo. O 3.13.4 dá um único
  `unexpected_token` no `const` (oráculo gravado de `empty_body_error_test.dart:18:6`).
- `d57`: `foo.bar = 1;` em classe é "construtor" `foo.bar` sem parâmetros, com redirecionamento.

