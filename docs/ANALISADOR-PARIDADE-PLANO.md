# Paridade do analisador com o `dart analyze` — especificação (2026-09-30)

Plano de trabalho do analisador (`crates/analise`, a inferência em
`crates/types`, o motor em `crates/paridade`) e do LSP (`crates/lsp`,
`editors/vscode`). Cada família traz a regra do analyzer (arquivo:linha), o
que falta no DartForge, o ganho medido no corpus e como se confere. A ordem
de execução (§4) é a do ganho por custo. O resultado medido de cada passo
está em §6.

Referências: `an611:` é o `analyzer-6.11.0` do pub-cache (o analyzer do Dart
3.6.2, que gravou o oráculo; caminhos relativos a `lib/`). O
`E:/references/dart-sdk/pkg/analyzer` é o analyzer 14.5-dev, de outra
geração: serve para entender a intenção, mas as linhas citadas aqui são as da
6.11, que é o comportamento medido.

## 0. Estado medido (o enunciado estava defasado)

O pedido partia de "12.241/23.030 (53,2%), FP 1.086, FN 10.502" e de "o LSP
ainda não roda `types`". Medido no `main` de hoje (`4e23a447`, oráculo
3.6.2, `DARTFORGE_SDK_LIB` do 3.6.2, lotes de 12 arquivos):

| | posição exata | mensagem igual | FP | FN | posição errada |
|---|---:|---:|---:|---:|---:|
| enunciado (2026-09-27) | 12.241 (53,2%) | 11.849 | 1.086 | 10.502 | 287 |
| **partida real** | **16.014 (69,5%)** | 15.581 | 667 | 6.749 | 267 |

Os passos 2–4 do A01 (docs/PENDENCIAS.md) já tinham entrado. O LSP já roda
a análise com tipos (auditoria L01, `crates/lsp/src/tipado.rs`, docs/LSP.md
"Diagnósticos tipados"): a mesma `Motor::analisar_com` do `dartforge
analyze`, por pacote, com cancelamento e versão. O que falta no LSP é outra
coisa (§3.12).

`verificados.txt`: 50 códigos publicados; o placar lista **95 códigos sem
erro emitido no corpus** fora da lista, à espera da conferência nos projetos
reais (§3.11).

## 1. Ferramentas

* `dartforge-paridade placar [--lote N] [--detalhes]` — o placar. Com
  `DARTFORGE_PARIDADE_AMOSTRAS=100000`, `--detalhes` lista **todas** as
  divergências (base dos agrupamentos abaixo).
* `crates/paridade/examples/sonda_arquivos.rs` (novo) — compara arquivo a
  arquivo, diagnóstico a diagnóstico (`=` acerto, `~` mensagem, `-` FN, `+`
  FP), num grupo do corpus ou, com `--livre <dir>`, contra o `dart analyze`
  3.6.2 rodado na hora sobre um diretório de rascunho. Todo caso mínimo deste
  plano foi conferido assim antes de virar regra.
* `scripts/paridade-pub-cache.py` (novo) — prepara os pacotes do pub-cache
  como projetos reais (a última versão de cada, sem Flutter, linguagem ≥ 2.12;
  `lib/` copiado; um `package_config` com todos) e o registro para
  `dartforge-paridade projetos --oraculo --corpus <dir>/corpus`. Nesta
  máquina: 209 pacotes.

## 2. Onde está a perda (agrupamento por causa raiz)

Das 6.749 ausências (FN), 949 estão em arquivos com `augment` (a recuperação
do fasta sem o experimento de macros), 573 em arquivos de sintaxe 3.13 e
5.227 no resto. As causas que atravessam muitos códigos:

| causa raiz | códigos afetados | perda medida |
|---|---|---:|
| C1. Diagnósticos de `types` sem arquivo: a unidade era adivinhada pelo intervalo, e dois arquivos do lote com o mesmo intervalo colidiam (o segundo sumia na deduplicação) | `undefined_class`, `not_a_type` (e qualquer código do outline) | 79 |
| C2. "Porta de sintaxe": num arquivo com erro de recuperação, **todos** os códigos de resolução de nome eram descartados | `not_a_type`, `undefined_class`, `creation_with_non_type`, `undefined_*` | ~170 |
| C3. Nome de tipo não resolvido no escopo da classe (`InstanceScope`): um membro com o nome do tipo esconde o tipo de topo | `not_a_type` (`int isn't a type` num enum com campo `int`) | FP 2 após C2 |
| C4. Resolução de nome de tipo nos **corpos** não relata nada (só o outline relata) e não distingue o contexto do `NamedTypeResolver` | `undefined_class`, `not_a_type`, `non_type_as_type_argument`, `cast_to_non_type`, `type_test_with_*`, `non_type_in_catch_clause`, `wrong_number_of_type_arguments*` | ~430 |
| C5. Igualdade de elementos da 6.11: duas declarações de mesmo nome e tipo na mesma unidade são o **mesmo** elemento (`ElementImpl.==` por localização), e o `DuplicateDefinitionVerifier` junta os membros delas | `duplicate_definition`, `conflicting_static_and_instance`, `duplicate_constructor` | ~250 (quase todo o bloco `augment`) |
| C6. Código morto em expressão (só havia em comando) | `dead_code` | ~100 |
| C7. Atribuição definida de `final`/`late` | `read_potentially_unassigned_final`, `definitely_unassigned_late_local_variable`, `late_final_local_already_assigned` | 142 |
| C8. Avisos de tipo do `BestPracticesVerifier`/`ErrorVerifier` | `unnecessary_type_check`, `unnecessary_null_comparison`, `dead_null_aware_expression`, `invalid_null_aware_operator` (resto), `receiver_of_type_never` | ~290 |
| C9. Exibição de tipos: alias de `typedef` e `Never?` | mensagens de `type_argument_not_matching_bounds` (161), variância (27) | 188 mensagens |
| C10. Extensões | `ambiguous_extension_member_access` | 73 |
| C11. Padrões | `pattern_never_matches_value_type`, `constant_pattern_never_matches_value_type`, `non_exhaustive_switch_*`, `unreachable_switch_case` | 275 |
| C12. Anotações | `invalid_annotation` | 108 |
| C13. Recuperação do parser | `expected_token` (FP 107, FN 85, posição 58), `extraneous_modifier` 69, `missing_identifier`, `expected_type_name` | ~400 |

Cauda: ~350 códigos com menos de 20 casos cada (FFI, constantes,
`could_not_infer`, construtores, etc.).

## 3. Especificação por família

### 3.1 C1 — a unidade de cada diagnóstico de `types`

**Regra.** O analyzer relata cada diagnóstico no `ErrorReporter` do arquivo
que está resolvendo (`an611:src/dart/analysis/library_analyzer.dart`, um
reporter por unidade). Não há adivinhação.

**Falta.** `resolve_outline` e `infer_bodies_*` devolviam só `Diagnostic`
(intervalo sem arquivo); o motor atribuía pelo intervalo exato de algum nó
(`Indice::atribuir`), e o mesmo intervalo em dois arquivos do lote escolhia
um deles — o outro diagnóstico era descartado como repetido.

**Especificação.** `OutlineResolver` guarda `unidades_dos_avisos` paralelo a
`diagnostics` (cada aviso nasce em `resolve_annotation`, que conhece a
unidade); `types::resolve_outline_com_unidades` e
`types::infer_bodies_das_bibliotecas_com_unidades` (o `BodyInferrer` já tinha
`unidades_dos_avisos`; o `erro_de_linguagem` não o mantinha alinhado — agora
mantém). `Motor::analisar_com` usa a unidade registrada e só cai na
heurística quando ela falta. API antiga intacta para os outros chamadores.

**Conferência.** Placar com lote 1 (um arquivo por programa, sem colisão) e
lote 12 dão o mesmo para os códigos do outline.

### 3.2 C2 — a porta de sintaxe por código

**Regra.** O analyzer resolve a árvore recuperada como qualquer outra; erro
de sintaxe não desliga verificador (`an611:src/dart/analysis/library_analyzer.dart`,
`_resolveFile` roda sempre).

**Falta.** `crates/paridade/src/analise.rs` (fase 6) descartava os códigos de
`depende_de_declaracoes` em todo arquivo com erro de recuperação, porque a
nossa recuperação podia perder declarações. Desde o porte da recuperação do
fasta (A01, quarto passo) a árvore é a mesma na maioria dos casos. Medido
sem a porta: `not_a_type` +116 / 0 FP novo, `undefined_class` +41 / 2 FP (os
dois são C3), `creation_with_non_type` +5 / 0, `undefined_identifier` +7 /
8 FP, `undefined_getter` 0 / 8 FP, `undefined_method` +3 / 3 mensagens.

**Especificação.** A porta fica só para os nomes resolvidos em expressões
(`undefined_identifier`, `undefined_function`, `undefined_method`,
`undefined_getter`, `undefined_setter`, `undefined_operator`), onde a
recuperação ainda diverge; sai para os nomes de tipo (`undefined_class`,
`not_a_type`, `creation_with_non_type` e os códigos novos de C4), com C3
feito.

### 3.3 C3 — escopo de instância na resolução de tipos

**Regra.** Os membros de uma classe, mixin, enum, extensão ou extension
type são resolvidos num `InstanceScope` que contém os getters (inclusive os
implícitos de campos e constantes de enum) e os métodos **declarados**, e
os setters à parte (`an611:src/dart/element/scope.dart:273-279`, montado em
`src/summary2/reference_resolver.dart:84,151,207,380`). A busca do nome de
tipo pega o `getter` do resultado (`src/dart/resolver/named_type_resolver.dart:295-302`);
um nome achado só como setter devolve `getter == null` — tipo indefinido.
As cláusulas `extends`/`implements`/`with`/`on` são resolvidas **antes** do
escopo de instância.

**Especificação.** No `OutlineResolver`, ao resolver o tipo de um campo, de
um método/getter/setter/operador/construtor (parâmetros e retorno) e da
representação, o contêiner fica em escopo: nome declarado como getter,
campo, constante de enum ou método do contêiner → "não é um tipo"; nome só
de setter → "não encontrado". Os parâmetros de tipo do membro e do
contêiner vêm antes (já era assim).

### 3.4 C4 — o `NamedTypeResolver` inteiro

**Regra.** `an611:src/dart/resolver/named_type_resolver.dart`:
* nome sintético não é relatado (`:519-521`); `boolean` → `undefined_class_boolean` (`:523-531`);
* em `catch (on T)`: `non_type_in_catch_clause` (`:533-541`); em `as`:
  `cast_to_non_type` (`:543-551`); em `is`: `type_test_with_non_type` (achou
  algo) ou `type_test_with_undefined_name` (`:553-571`);
* alvo de construtor redirecionador: `redirect_to_non_class` (`:574-582`);
* dentro de uma lista de argumentos de tipo: `non_type_as_type_argument` (`:584-593`);
* criação de instância: `new_with_non_type`/`const_with_non_type` (`reportNewWithNonType`, `:495-513`);
* tipo de cláusula de herança: nada (o erro é o `*_non_class`, `:599-605`);
* variável local ou função local antes da declaração: `referenced_before_declaration` (`:607-617`);
* achou um elemento que não é tipo: `not_a_type` (`:619-627`); `await` sem prefixo: `undefined_identifier_await` (`:629-635`); senão `undefined_class` (`:637-643`).
* Número de argumentos de tipo errado para classe/alias:
  `wrong_number_of_type_arguments` (`named_type_resolver.dart:145`, com a
  instanciação para os limites).

**Falta.** Só o outline relatava, e sempre como `undefined_class`/`not_a_type`
(o contexto se perdia na `ponte`). Os corpos (`inferencia/tipos.rs`,
`resolver_anotacao`) não relatavam nada: tipo de local, `is`, `as`, `catch`,
argumentos de tipo de chamadas e literais, criação de instância.

**Especificação.** Um contexto da anotação (`Normal`, `ArgumentoDeTipo`,
`Catch`, `As`, `Is`, `Clausula`) desce pela resolução; os dois caminhos (o
`OutlineResolver` e o `BodyInferrer`) relatam com o código e a mensagem do
analyzer (com código: `Diagnostic::com_codigo`, sem passar pela `ponte`).
O contexto de argumento de tipo vale para os argumentos aninhados (`List<Undef>`
→ `non_type_as_type_argument` em `Undef`). A contagem de argumentos:
`wrong_number_of_type_arguments` com os nomes e números, no intervalo do
tipo inteiro.

### 3.5 C5 — elementos iguais por localização (6.11)

**Regra.** `ElementImpl.operator ==` compara `kind` e `location`
(`an611:src/dart/element/element.dart:2877-2884`); a localização é a cadeia
de nomes (biblioteca, unidade, nome). Duas `class A` na mesma unidade são o
mesmo elemento. O `DuplicateDefinitionVerifier` guarda os nomes de membros
por elemento (`_getElementContext`, `error/duplicate_definition_verifier.dart:808-811`,
mapa `_instanceElementContexts`), então os membros da segunda `class A` são
conferidos contra os da primeira: `duplicate_definition`,
`conflicting_static_and_instance` (`:411-458`) e `duplicate_constructor`
(`:365-370`) entre as duas. É o que acontece em todo arquivo `augment class
A` sem o experimento (a recuperação deixa duas `class A`).

**Especificação.** Em `analise::duplicatas`, o contexto de membros é por
`(unidade, espécie, nome)` da declaração, acumulado na ordem do arquivo. Classe
e mixin com o mesmo nome são espécies diferentes (não se juntam).

### 3.6 C6 — código morto em expressões

**Regra.** O `NullSafetyDeadCodeVerifier.visitNode` é chamado para **todo**
nó (`an611:src/error/dead_code_verifier.dart:392-420`): o primeiro nó
inalcançável abre o trecho, que se fecha no fim do bloco básico. Em
expressão: o ramo de `true ? a : b`/`false ? a : b`, o operando direito de
`true || x`/`false && x`, o que segue um operando `Never`, argumentos depois
de um argumento `Never`.

**Especificação.** O fluxo das condições literais já existe
(`inferencia/expr.rs`, `condicao`); falta relatar o primeiro nó de expressão
visitado com o fluxo inalcançável quando não há trecho aberto, com a
extensão do analyzer (o nó inteiro). Conferir caso a caso com a sonda livre
(`least_upper_bound_test.dart` tem 32).

### 3.7 C7 — atribuição definida de `final` e `late`

**Regra.** `an611:src/generated/resolver.dart:655-680`
(`_checkReadOfNotAssignedLocalVariable`): leitura de `late` definitivamente
não atribuída → `definitely_unassigned_late_local_variable`; leitura de
`final` sem `late` potencialmente não atribuída →
`read_potentially_unassigned_final`; escrita em `late final` já
potencialmente atribuída → `late_final_local_already_assigned`
(`src/dart/resolver/assignment_expression_resolver.dart:372`,
`resolver.dart:1380`).

**Especificação.** O estado de atribuição que já serve a
`not_assigned_potentially_non_nullable_local_variable` passa a dar os três.

### 3.8 C8 — avisos de tipo

* `unnecessary_type_check` (`an611:src/error/best_practices_verifier.dart:760-790`):
  `e is T` sempre verdadeiro (o tipo estático é subtipo de `T`, sem `dynamic`)
  ou `e is! T` sempre falso, e `null is Null`/`x is! Null` com `x` não anulável.
* `unnecessary_null_comparison` (`best_practices_verifier.dart:1040-1070`):
  `x == null`/`x != null` com `x` não anulável (e não `dynamic`/variável de
  tipo potencialmente anulável), no operador e operando.
* `dead_null_aware_expression` (`src/generated/error_verifier.dart:3040-3052`):
  `a ?? b` e `a ??= b` com `a` não anulável → `b` é morto.
* `invalid_null_aware_operator` (o resto: `?..` de cascata, índice `?[`,
  `...?`; `error_verifier.dart:5555-5640`).
* `receiver_of_type_never` (`binary_expression_resolver.dart:425`,
  `method_invocation_resolver.dart:543`, …): membro usado num receptor
  `Never`.

### 3.9 C9 — exibição de tipos (alias e `Never?`)

**Regra.** O analyzer guarda o alias com que o tipo foi escrito
(`TypeImpl.alias`) e o exibe (`'Fcon<Never?>'`); `Never?` não é normalizado
para `Null` na exibição nem como receptor (`unchecked_use_of_nullable_value`
em `Never?`, e não `invalid_use_of_null_value`).

**Decisão.** Exige um tipo que carregue o alias fora da identidade do
hash-consing e um `Never?` distinto de `Null` na tabela — muda a
representação comum a todos os backends (`emit_js`, `emit_native`, `mundo`).
Fica fora desta rodada (docs/PENDENCIAS.md A02), com a especificação acima.

### 3.10 C10–C13

* **C10** `ambiguous_extension_member_access`
  (`an611:src/dart/resolver/extension_member_resolver.dart:100-125`): duas ou
  mais extensões aplicáveis, nenhuma mais específica → o erro no nome. A
  escolha da mais específica já existe (`inferencia/membros.rs`); falta
  relatar o empate.
* **C11** padrões: `pattern_never_matches_value_type`
  (`src/generated/resolver.dart:620-640`, a relação de tipos do padrão de
  objeto/declaração com o escrutínio) e
  `constant_pattern_never_matches_value_type`
  (`src/dart/constant/constant_verifier.dart:140-155`). A exaustividade
  (`non_exhaustive_switch_*`, `unreachable_switch_case`,
  `constant_verifier.dart:940-975`) exige o porte do algoritmo de espaços do
  `_fe_analyzer_shared/exhaustiveness` — rodada própria.
* **C12** `invalid_annotation`
  (`an611:src/dart/resolver/annotation_resolver.dart:80-410`): anotação que
  não é referência a constante nem construtor constante.
* **C13** parser: as divergências de `expected_token` são recuperações
  específicas (amostras no placar); cada uma pede porte do trecho do fasta
  correspondente.

### 3.11 Publicação

Regra de sempre (`crates/analise/verificados.txt`): entra o código sem
nenhum FP, posição ou mensagem errada no corpus **e** nos projetos reais. Os
três projetos do proprietário não existem nesta máquina; os projetos reais
aqui são os 209 pacotes do pub-cache (`scripts/paridade-pub-cache.py`). Todo
candidato do placar com 0 emissão errada nos 209 pacotes entra.

### 3.12 LSP

Já feito (L01): diagnósticos com tipos no editor, incrementais (um programa
por pacote, cancelamento entre fases, versão), latência medida no Linux
(docs/LSP.md). Nesta rodada:

* os códigos novos publicados aparecem no editor sem mudança no LSP (a
  publicação tipada é `dartforge_paridade::publicaveis`);
* a correção de C1 vale também para o LSP (o motor é o mesmo): diagnóstico
  de um arquivo aberto não vai mais parar noutro com o mesmo intervalo;
* latência remedida no Windows com `examples/latencia_diagnosticos`;
* testes do crate `lsp` verdes.

## 4. Ordem de execução

1. C1 (feito primeiro: corrige a atribuição de tudo o que vem depois).
2. C2 + C3 (porta por código e escopo de instância).
3. C5 (elementos iguais; só `analise`).
4. C4 (`NamedTypeResolver` nos dois caminhos + número de argumentos de tipo).
5. C7, C6 (fluxo).
6. C8 (avisos de tipo).
7. C10, C12, C11 sem exaustividade.
8. Publicação (§3.11) e LSP (§3.12).
9. C13 e a cauda, pelo ganho.

C9 e a exaustividade ficam registrados como próximos passos (decisão de
representação de tipos; porte grande).

## 5. Verificação

* `dartforge-paridade placar` depois de cada família (nenhum código pode
  perder acerto nem ganhar FP sem explicação; nenhum verificado com erro);
* `dartforge-paridade projetos --corpus <pub>/corpus` (0 FP de código publicado);
* `cargo test -p dartforge-types -p dartforge-analise -p dartforge-paridade -p dartforge-lsp`;
* no fim, `cargo test --workspace` e `cargo run --release -p dartforge-diferencial`
  (o `crates/types` é comum a todos os backends).

## 6. Resultado (2026-10-01)

Placar (oráculo 3.6.2, lotes de 12, `DARTFORGE_SDK_LIB` do 3.6.2):

| passo | posição exata | mensagem igual | FP | FN | pos. errada |
|---|---:|---:|---:|---:|---:|
| partida (`4e23a447`) | 16.014 (69,5%) | 15.581 | 667 | 6.749 | 267 |
| C1 unidade dos diagnósticos | 16.093 | 15.660 | 660 | 6.669 | 268 |
| C2 + C3 porta e escopo de instância | 16.292 | 15.859 | 657 | 6.470 | 268 |
| C5 elementos iguais (6.11) | 16.455 | 16.022 | 657 | 6.307 | 268 |
| C4 `NamedTypeResolver` | 16.571 | 16.146 | 626 | 6.203 | 256 |
| C6–C8, C10, super | 17.203 | 16.777 | 640 | 5.576 | 251 |
| C12, padrões, `Never`, construção abstrata, retornos | **17.428 (75,7%)** | 17.000 | **617** | **5.351** | 251 |

Acertos por código (partida → agora; FP partida → agora):

| código | antes | agora | FP antes | FP agora |
|---|---:|---:|---:|---:|
| `not_a_type` | 4 | 171 | 23 | 0 |
| `undefined_class` | 168 | 293 | 15 | 7 |
| `invalid_annotation` | 0 | 101 | 0 | 0 |
| `invalid_null_aware_operator` | 42 | 134 | 0 | 0 |
| `duplicate_definition` | 925 | 1.007 | 14 | 14 |
| `unnecessary_type_check` | 0 | 76 | 0 | 5 |
| `read_potentially_unassigned_final` | 0 | 74 | 0 | 0 |
| `dead_code` | 74 | 133 | 3 | 5 |
| `ambiguous_extension_member_access` | 0 | 54 | 0 | 0 |
| `pattern_never_matches_value_type` | 0 | 53 | 0 | 2 |
| `conflicting_static_and_instance` | 156 | 207 | 0 | 0 |
| `unnecessary_null_comparison` | 0 | 46 | 0 | 0 |
| `definitely_unassigned_late_local_variable` | 0 | 44 | 0 | 0 |
| `wrong_number_of_type_arguments` | 0 | 36 | 0 | 1 |
| `non_type_as_type_argument` | 0 | 32 | 0 | 1 |
| `dead_null_aware_expression` | 0 | 29 | 0 | 2 |
| `instantiate_abstract_class` | 0 | 29 | 0 | 0 |
| `duplicate_constructor` | 95 | 123 | 0 | 0 |
| `receiver_of_type_never` | 0 | 24 | 0 | 0 |
| `yield_of_invalid_type` | 0 | 24 | 0 | 0 |
| `constant_pattern_never_matches_value_type` | 0 | 19 | 0 | 0 |
| `return_without_value` | 0 | 19 | 0 | 0 |
| `late_final_local_already_assigned` | 0 | 18 | 0 | 1 |
| `not_initialized_non_nullable_variable` | 0 | 17 | 0 | 0 |
| `type_parameter_referenced_by_static` | 0 | 17 | 0 | 0 |
| `return_of_invalid_type_from_closure` | 0 | 16 | 0 | 0 |
| `abstract_super_member_reference` | 0 | 15 | 0 | 0 |
| `undefined_super_member` | 0 | 15 | 0 | 0 |
| `non_type_in_catch_clause`, `type_test_with_undefined_name`, `cast_to_non_type`, `type_check_with_null`, `undefined_annotation` | 0 | 10, 10, 8, 7, 6 | 0 | 0, 2, 0, 0, 0 |
| `assignment_to_final_local` | 49 | 49 | 15 | 0 |
| `type_arguments_on_type_variable` | 0 | 0 | 11 | 0 |

**Projetos reais.** Os 209 pacotes do pub-cache (oráculo 3.6.2 gravado
neles): nenhum FP, posição ou mensagem errada de código publicado. A
partida tinha 7 FP publicados ali (`unnecessary_cast` 4,
`unnecessary_non_null_assertion` 3), de duas causas corrigidas: a
importação condicional escolhia o ramo `dart.library.js_interop` (o analyzer
fica com a URI principal, `file_state.dart:785-800`) e `super[i]`/`super.x`
achavam o membro da própria classe ou um abstrato da interface (agora a
cadeia concreta do super, com encaminhadores de `noSuchMethod`).

**Publicação.** `verificados.txt`: 50 → 159 códigos (109 novos, todos com
0 erro emitido no corpus e nos 209 pacotes). Ficaram de fora, por FP nos
pacotes: `equal_elements_in_const_set` (12), `map_value_type_not_assignable`
(17), `getter_not_subtype_setter_types` (6), `invalid_override` (1). O
`constant_pattern_never_matches_value_type` tinha 16 FP nos pacotes (tipo
casado genérico, `BaseSqlType<D2>`): corrigido (fecho maior pelo lado
seguro) e publicado com a medição seguinte limpa.

**LSP.** Sem mudança de código além da regra de duplicatas com a versão
da sintaxe: os códigos novos chegam ao editor pela publicação tipada.
Latência remedida no Windows (docs/LSP.md): tipada após edição com mediana
de 51–68 ms nos quatro arquivos do exemplo, rajada de 20 edições = 1
publicação, publicação idêntica à do `dartforge analyze`. Testes do crate
`lsp` verdes.

**Testes novos.** `crates/paridade/tests/regras_do_analyzer.rs`: 18
diagnósticos de sete arquivos mínimos, cada um conferido contra o `dart
analyze` 3.6.2 (`sonda_arquivos --livre`); `duplicatas::declaracoes_de_mesmo_nome_juntam_os_membros`.

**Não feito nesta rodada** (registrado, com a regra em §3): C9 (alias de
`typedef` e `Never?` na exibição, mudança de representação comum aos
backends), exaustividade (`non_exhaustive_switch_*`,
`unreachable_switch_case`, porte do algoritmo de espaços), C13 (recuperação
do parser: `expected_token`, `extraneous_modifier`), `could_not_infer`, os
FN de `unused_element` (parâmetros opcionais nunca passados, `_` em
bibliotecas com curinga) e a cauda de códigos com menos de 20 casos.
