# Especificação do analyzer 3.6.2 por código de diagnóstico (rodada 3, 2026-10-02; rodada 4, 2026-10-04)

Fonte: o código oficial na tag 3.6.2 do SDK, extraído para
`E:\references\dart-sdk-3.6.2\pkg\{analyzer,_fe_analyzer_shared,analysis_server}\lib`
e `pkg/analyzer/messages.yaml`, `pkg/front_end/messages.yaml` (o checkout
`E:\references\dart-sdk` está no main/3.14 e não serve). Citações
`analyzer/lib/src/...:linha`, `_fe_analyzer_shared/lib/src/...:linha`.

Cobertura: os **435 códigos** (a rodada 3 dizia 436: erro de contagem, ver §0.A) com alguma perda no placar `r7`
(18.535/23.030; perda total 5.256 = mensagem + posição + FP + FN), em seis
famílias com listas fixas (`E:\dftemp\analise\trab\familia-*.txt`):

| família | parte | códigos | perda | linhas |
|---|---|---:|---:|---:|
| A invocação, argumentos, membros, atribuição | §A | 101 | 1.603 | 2.731 |
| B fluxo, nulidade, padrões, código morto | §B | 60 | 703 | 1.859 |
| C declarações, herança, construtores, limites, FFI | §C | 179 | 1.402 | 5.876 |
| D constantes | §D | 34 | 439 | 3.903 |
| E parser e scanner (com o `for` de padrão) | §E | 38 | 477 | 5.070 |
| F nomes e tipos não resolvidos, não usados | §F | 23 | 632 | 5.325 |

Rodada 4: auditoria de completude (§0.A), aprofundamento das famílias E, D e F e complementos de A, B, C (no fim de cada família, em "Rodada 4 — família X"), planos de implementação dos achados transversais (§G), índice por código (§H). Os exemplos "entrada → diagnósticos" da rodada 4 foram rodados no oráculo vivo (`C:\tools\dartsdk-3.6.2\bin\dart analyze --format=json`); os casos ficam em `corpus\especificacao\`.

Cada código traz: **emissão** (visitante/verificador e método, arquivo:linha,
fase), **condição exata**, **posição** (nó/token, offset e length), **mensagem**
(modelo e formatação de cada argumento), **supressões e ordem**, e **no
DartForge** (onde a regra entra, o que existe e a causa das divergências, com
as amostras do placar).

## 0. Achados transversais — implementar ANTES das famílias

Estes pontos aparecem em várias famílias; cada um tem **uma** correção no
ponto comum, feita uma vez, antes de qualquer família que dependa dele.

### T1. Declarações homônimas e `augment` sem o experimento: a primeira vence

> **Corrigido na rodada 4 (§G, T1.1):** a regra tem quatro mecanismos e "a primeira vence" só vale entre homônimos da mesma espécie. Entre espécies diferentes vence a que entra antes no escopo (acessores, funções, enums, extensões, extension types, typedefs, mixins, classes); os caches por elemento ficam com quem foi **consultado** primeiro; e os tipos dos homônimos são distintos. Implementar pelo §G.

No 3.6.2, `augment class X …` sem o experimento é recuperado pelo parser como
uma variável de topo `augment` seguida de uma **segunda** declaração `X`
(`expected_token` + `missing_const_final_var_or_type` + `duplicate_definition`).
- O escopo dá ao nome a **primeira** declaração
  (`analyzer/lib/src/dart/element/scope.dart:793-797`, `_getters[id] ??= element`);
  o DartForge dá a **última** (`crates/elements/src/outline.rs:131-134`).
- A igualdade de elementos é por localização
  (`analyzer/lib/src/dart/element/element.dart:2872`, `hashCode` em `:2472`):
  caches por elemento (`InheritanceManager3._interfaces`,
  `inheritance_manager3.dart:69/239`; `ClassHierarchy._map`,
  `class_hierarchy.dart:14,44`; o `Map<FieldElement,…>` do verificador de
  inicialização) **compartilham** o resultado entre homônimos: a interface é a
  da primeira declaração, os membros AST são os de cada uma, e o relato vai no
  nome de cada declaração.
- O que não é cache por elemento continua separado
  (`element.augmented.constants` de `enum_without_constants`).

Correção única: (1) em `elements/src/outline.rs`, o nome de topo repetido
resolve para a **primeira** declaração (as seguintes continuam elementos
próprios, com os próprios membros); (2) remover os filtros que pulam classes
homônimas e unidades com linha `augment ` quando a biblioteca **não** tem
`Feature::Augmentations`/`Macros` — `analise/src/clausulas.rs:1114`
(`verificador_de_heranca_prossegue`), `types/src/sobrescritas.rs:1029-1042`,
`:1358-1364`, `:1525-1534`, `:699-710`, `analise/src/construtores.rs`
(`verificar`), `analise/src/inicializacao.rs` (índice por nome de classe);
(3) para homônimos, interface/hierarquia calculadas uma vez pela primeira
declaração e reaproveitadas, relato no nome de cada uma. Famílias que
dependem: C (herança, construtores: FN AUG de `invalid_factory_name_not_a_class`,
`non_abstract_class_inherits_abstract_member`, `conflicting_*`,
`recursive_interface_inheritance`, `final_not_initialized_constructor`…),
F (`undefined_class` em `augment mixin/extension`), A (membros resolvidos).

### T2. Qual oráculo julga cada arquivo (3.6.2 × 3.13.4)

Os arquivos de sintaxe nova são julgados pelo 3.13.4 (`corpus/diagnosticos/sintaxe-nova.json`
e a regra do parser em §E: versão da biblioteca > 3.6, ou uso de
dot-shorthands/primary-constructors/private-named-parameters). Códigos e
mensagens mudaram entre as versões: `unused_element_parameter` (3.13) ×
`unused_element` (3.6) para parâmetros opcionais; `assignment_to_primary_constructor_parameter`
e `unused_field_from_primary_constructor` só existem no 3.13;
`experiment_not_enabled` cita "3.6.0" para null-aware-elements no 3.6.2
(`releaseVersion` nula → versão do SDK, `analyzer/lib/src/fasta/ast_builder.dart:6078`)
e "3.8.0" no 3.13; `representation_field_modifier` mudou de texto;
`const_constructor_with_non_const_super` mostra a classe derivada no 3.6.2 e a
base no 3.13. Correção única: um marcador por biblioteca, calculado uma vez
(`LibraryFeatures` ou o `Program`), que os emissores consultam para o nome do
código e o texto — nada de regras ad hoc por família.

### T3. O SDK como o analyzer o vê: declarações públicas, sem patches

O analyzer lê `dart:core` etc. sem os patches da plataforma; o DartForge
aplica o patch `js_dev_runtime`. Efeitos: FP de `const_with_non_const` em
`bool.fromEnvironment` (`factory` sem `const` no patch × `const factory` em
`core/bool.dart:67`), documentação/definição caindo em `core_patch.dart` no
LSP, nomes de parâmetros do patch em mensagens e inlay hints. Correção única
no carregamento (`crates/elements`): para a análise, o elemento público do
SDK é a declaração original; o patch só substitui o corpo (já existe
`FunctionElement::patched_by`).

### T4. `InvalidType` distinto de `dynamic`

Nome ou tipo que não resolve dá `InvalidType` no analyzer (comporta-se como
`dynamic`, mas suprime diagnósticos de tipo e aparece como `InvalidType` em
mensagens). Hoje: `UnitBodyTypes::tipos_invalidos` só marca expressões não
resolvidas, `padroes_invalidos`/`locais_invalidos` cobrem padrões e
parâmetros (rodada 2). Correção única: o tipo de recuperação na tabela de
tipos (um `TypeId` decorado como os de exibição, estrutura `dynamic`,
`Exibicao::Invalido`), propagado pela resolução de anotações e pela leitura,
consultado pelas supressões (`hasInvalidType` da exaustividade, `argument_type_not_assignable`,
`invalid_assignment`, `unnecessary_*`) e exibido "InvalidType".

### T5. Ordem dos verificadores e supressões gerais

`analyzer/lib/src/dart/analysis/library_analyzer.dart:420-456`
(`_computeVerifyErrors`): `ConstantVerifier` → `InheritanceOverrideVerifier.verifyUnit`
→ `ErrorVerifier`; antes, por biblioteca, `MemberDuplicateDefinitionVerifier`.
O verificador de elementos não usados **não** tem porta de erro de sintaxe
(§F): a nossa porta por biblioteca (`paridade/src/analise.rs`,
`libs_com_erro_de_sintaxe`) custa FN em `unused_*`. Erros de sintaxe não
descartam o resto da análise; a lista dos que não descartam código
(`paridade::analise::recuperacao_do_parser`) é a referência.

### T6. Fluxo de padrões e o tipo casado

O analyzer modela o valor casado e o acumulador "não casou" no fluxo
(`_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart`, `switchStatement_*`,
`patternAssignment`, `promoteForPattern`): dele dependem `dead_code` em
padrões, `constant_pattern_never_matches_value_type` (tipo casado promovido
pelos casos anteriores), `pattern_never_matches_value_type`,
`unnecessary_*_pattern`. Ver §B Apêndice A; a tabela lateral do tipo casado
por padrão (`UnitBodyTypes`) é a interface entre B (inferência) e D
(verificador de constantes).

### T7. Formatação de tipos nas mensagens

`ErrorReporter._convertTypeNames` (`analyzer/lib/error/listener.dart:362-376`):
argumento `DartType` → `getDisplayString(preferTypeAlias: true)` (`TypeTable::format`);
argumento já `String` → sem alias (`format_sem_alias`). Tipos de função
genéricos aninhados renomeiam parâmetros que colidem com subscritos (`T₀`,
`ElementDisplayStringBuilder`). Alguns códigos mostram o **nome** do elemento,
não o tipo (`undefined_method` `{1}`: `'C'` sem argumentos, `'Function'` para
tipos de função, `'<unknown>'` para records — §A). Correção única no
`TypeTable` (renomeação `T₀`) e nos emissores pela regra do argumento.

### T8. Grafo de dependências de constantes

`analyzer/lib/src/dart/constant/compute.dart:62-73` +
`evaluation.dart:223-325` (Tarjan): ciclos dão `recursive_compile_time_constant`
e `recursive_constant_constructor`; as constantes de enum são avaliadas pelo
construtor (§D). Correção única no `Motor` de constantes.

### Ordem de implementação

1. T1, T2, T3, T4 (pontos comuns, um responsável cada, com placar antes/depois).
2. Famílias em paralelo, cada uma nos seus arquivos: A, B (com T6), C, D (com
   T8), E (com o scanner e o `for` de padrão ponta a ponta), F.
3. Placar, 209 pacotes, testes e publicação no fim.

### 0.A Auditoria de completude (rodada 4, 2026-10-04)

Método (script `E:\dftemp\analise\spec-r4\audit.py`, sobre o documento da rodada 3, 5.923 linhas): para cada código das listas `familia-*.txt`, achar o título `#####` que o cita entre crases dentro da seção da família e procurar, no bloco até o título seguinte, as palavras dos seis campos (Emissão, Condição, Posição, Mensagem, Supressões, No DartForge). A detecção é por palavra: um bloco que descreve o campo sem nomeá-lo conta como falta — por isso cada complemento da rodada 4 releu a seção antes de escrever.

Resultado:

| família | códigos | com os seis campos | sem título próprio | falta só "Supressões" | faltam 2+ campos | seção de ≤ 4 linhas |
|---|---:|---:|---:|---:|---:|---:|
| A | 101 | 19 | 0 | 46 | 16 | 32 |
| B | 60 | 9 | 0 | 44 | 7 | 0 |
| C | 179 | 24 | 0 | 67 | 59 | 11 |
| D | 34 | 3 | 0 | 8 | 3 | 0 |
| E | 38 | 1 | 20 | 1 | 1 | 2 |
| F | 23 | 13 | 0 | 9 | 1 | 0 |
| total | 435 | 69 | 20 | 175 | 87 | 45 |

Achados da auditoria:

1. **As listas somam 435 códigos, não 436** (A 101, B 60, C 179, D 34, E 38, F 23; `perdas-r7.txt` tem 435 linhas com perda). O "436" do cabeçalho é um erro de contagem da rodada 3.
2. **15 códigos das listas não existem no analyzer 3.6.2** (nenhuma constante com esse nome em `analyzer/lib/src/error/codes.g.dart`, `dart/error/*.g.dart` nem no scanner do 3.6.2 — levantado por `emissores.py`): `assignment_to_primary_constructor_parameter`, `const_primary_constructor_with_body`, `deprecated_optional`, `dot_shorthand_undefined_member`, `factory_constructor_new_name`, `field_initialized_in_declaration_and_initializer_of_primary_constructor`, `field_initialized_in_declaration_and_parameter_of_primary_constructor`, `initializing_declaring_parameter`, `mixin_class_declares_non_trivial_generative_constructor`, `multiple_primary_constructor_body_declarations`, `primary_constructor_body_with_expression_body`, `primary_constructor_cannot_redirect`, `super_initializing_declaring_parameter`, `unused_element_parameter`, `unused_field_from_primary_constructor`. São códigos do oráculo 3.13.4 (construtores primários, dot-shorthands, nomes novos); a especificação deles só pode vir do checkout main (`E:\references\dart-sdk`, 3.14), e está marcada assim onde aparece. Ver T2 (§G).
3. **Família E**: 20 dos 38 códigos não tinham título próprio (estavam em linhas da "Cauda" ou agrupados) e nenhum tinha exemplo de entrada → diagnósticos. Refeita por inteiro na rodada 4 (E.1–E.4).
4. **Famílias A, B, C**: o campo ausente mais comum é "Supressões e ordem" (157 códigos só com essa falta); os complementos estão em "Rodada 4 — família A/B/C" no fim de cada família.
5. **Famílias D e F**: os códigos grandes já tinham os seis campos em substância; faltavam o algoritmo do motor/verificador, a tabela de decisão do `NamedTypeResolver`, o `ImportsVerifier` e exemplos do oráculo — acrescentados em D.R4-1/2 e F.R4-1/2.

Lista dos códigos com falta (campos: E = Emissão, C = Condição, P = Posição, M = Mensagem, S = Supressões, D = No DartForge; `—` = sem título próprio):

- **A** (82): `type_argument_not_matching_bounds` (S), `could_not_infer` (S), `not_enough_positional_arguments` (S), `assignment_to_primary_constructor_parameter` (ECS), `wrong_number_of_type_arguments` (CS), `invocation_of_non_function` (CS), `new_with_undefined_constructor_default` (S), `extra_positional_arguments_could_be_named` (S), `ambiguous_extension_member_access` (S), `for_in_of_invalid_type` (S), `return_of_invalid_type_from_closure` (S), `assignment_to_final_no_setter` (S), `throw_of_invalid_type` (S), `for_in_of_invalid_element_type` (S), `list_element_type_not_assignable` (S), `new_with_undefined_constructor` (S), `undefined_super_member` (S), `wrong_number_of_type_arguments_function` (CMS), `invocation_of_non_function_expression` (S), `map_key_type_not_assignable` (S), `map_value_type_not_assignable` (ECSD), `missing_required_argument` (S), `super_formal_parameter_without_associated_positional` (S), `wrong_number_of_type_arguments_constructor` (S), `undefined_setter` (S), `doc_directive_missing_argument` (S), `argument_must_be_a_constant` (CS), `no_annotation_constructor_arguments` (S), `super_in_extension` (S), `undefined_extension_operator` (S), `duplicate_named_argument` (S), `extension_type_declares_member_of_object` (CPS), `instantiate_type_alias_expands_to_type_parameter` (S), `leaf_call_must_not_return_handle` (SD), `set_element_type_not_assignable` (ECPSD), `extension_type_implements_not_supertype` (S), `instance_member_access_from_factory` (S), `super_formal_parameter_without_associated_named` (S), `wrong_number_of_type_arguments_method` (C), `doc_directive_argument_wrong_format` (SD), `leaf_call_must_not_take_handle` (ECPSD), `doc_directive_has_extra_arguments` (SD), `extension_override_argument_not_assignable` (S), `extension_type_inherited_member_conflict` (S), `extension_type_representation_type_bottom` (S), `instantiate_abstract_class` (S), `wrong_number_of_type_arguments_extension` (CS), `argument_must_be_native` (CSD), `assignment_to_const` (S), `assignment_to_final` (S), `async_for_in_wrong_context` (S), `extension_override_with_cascade` (CS), `extension_type_constructor_with_super_invocation` (S), `extension_type_implements_itself` (CS), `main_first_positional_parameter_type` (S), `main_has_too_many_required_positional_parameters` (SD), `super_in_extension_type` (S), `undefined_named_parameter` (CS), `use_of_native_extension` (S), `wrong_number_of_type_arguments_enum` (CS), `await_in_late_local_variable_initializer` (S), `class_instantiation_access_to_member` (CMS), `deferred_import_of_extension` (S), `disallowed_type_instantiation_expression` (CSD), `doc_directive_has_unexpected_named_argument` (SD), `expected_two_map_type_arguments` (PS), `extension_type_constructor_with_super_formal_parameter` (SD), `extension_type_implements_representation_not_supertype` (SD), `generic_function_type_cannot_be_type_argument` (CSD), `invalid_extension_argument_count` (S), `main_has_required_named_parameters` (SD), `mixin_instantiate` (S), `non_sized_type_argument` (CSD), `positional_super_formal_parameter_with_positional_argument` (S), `enum_instantiated_to_bounds_is_not_well_bounded` (SD), `expected_one_list_type_arguments` (SD), `expected_one_set_type_arguments` (PSD), `extension_as_expression` (CS), `extension_override_without_access` (CS), `for_in_with_const_variable` (S), `must_return_void` (CS), `non_native_function_type_argument_to_pointer` (CSD).
- **B** (51): `not_initialized_non_nullable_instance_field` (S), `pattern_variable_assignment_inside_guard` (S), `unnecessary_cast_pattern` (S), `invalid_use_of_null_value` (CS), `missing_variable_pattern` (S), `unnecessary_null_check_pattern` (S), `unnecessary_set_literal` (S), `invalid_pattern_variable_in_shared_case_scope` (S), `late_final_field_with_const_constructor` (S), `switch_case_completes_normally` (S), `unnecessary_wildcard_pattern` (S), `dead_null_aware_expression` (S), `equal_keys_in_map_pattern` (S), `not_assigned_potentially_non_nullable_local_variable` (S), `not_initialized_non_nullable_variable` (S), `pattern_never_matches_value_type` (S), `unnecessary_null_comparison` (S), `unnecessary_type_check` (S), `non_nullable_equals_parameter` (S), `null_argument_to_non_null_type` (S), `pattern_assignment_not_local_variable` (S), `unnecessary_null_assert_pattern` (S), `cast_from_null_always_fails` (S), `duplicate_pattern_field` (S), `not_null_aware_null_spread` (S), `unnecessary_nan_comparison` (S), `break_label_on_switch_member` (S), `case_expression_type_is_not_switch_expression_subtype` (S), `duplicate_pattern_assignment_variable` (S), `null_check_always_fails` (S), `nullable_type_in_catch_clause` (S), `nullable_type_in_implements_clause` (S), `refutable_pattern_in_irrefutable_context` (CS), `unnecessary_question_mark` (S), `dead_code_on_catch_subtype` (S), `definitely_unassigned_late_local_variable` (S), `duplicate_variable_pattern` (S), `unnecessary_final` (S), `cast_from_nullable_always_fails` (S), `expected_two_map_pattern_type_arguments` (S), `late_final_local_already_assigned` (S), `read_potentially_unassigned_final` (S), `relational_pattern_operator_return_type_not_assignable_to_bool` (S), `rest_element_in_map_pattern` (CS), `unnecessary_no_such_method` (S), `dead_code_catch_following_catch` (S), `duplicate_rest_element_in_pattern` (CS), `empty_map_pattern` (CS), `expected_one_list_pattern_type_arguments` (S), `invalid_null_aware_operator` (CPMS), `non_exhaustive_switch_expression` (CPS).
- **C** (155): `invalid_factory_name_not_a_class` (S), `private_optional_parameter` (S), `supertype_expands_to_type_parameter` (S), `invalid_use_of_type_outside_library` (S), `invalid_language_version_override` (S), `undefined_constructor_in_initializer` (S), `duplicate_definition` (CMS), `initializing_formal_for_non_existent_field` (S), `primary_constructor_cannot_redirect` (S), `default_value_in_redirecting_factory_constructor` (S), `mixin_class_declares_non_trivial_generative_constructor` (ECMS), `wrong_explicit_type_parameter_variance_in_superinterface` (S), `final_not_initialized` (S), `type_parameter_supertype_of_its_bound` (S), `wrong_type_parameter_variance_in_superinterface` (S), `field_initialized_by_multiple_initializers` (S), `conflicting_inherited_method_and_setter` (S), `getter_not_subtype_setter_types` (S), `map_entry_not_in_map` (S), `mixin_application_not_implemented_interface` (S), `non_abstract_class_inherits_abstract_member` (S), `case_expression_type_implements_equals` (S), `field_initializer_not_assignable` (S), `integer_literal_out_of_range` (S), `recursive_interface_inheritance` (S), `return_in_generator` (CS), `top_level_cycle` (S), `type_parameter_referenced_by_static` (S), `field_initializing_formal_not_assignable` (S), `invalid_override` (S), `main_is_not_function` (S), `not_iterable_spread` (S), `redirect_to_invalid_function_type` (S), `unqualified_reference_to_non_local_static_member` (S), `wrong_type_parameter_variance_position` (S), `dot_shorthand_undefined_member` (ECMSD), `duplicate_field_formal_parameter` (CPMS), `field_initialized_in_parameter_and_initializer` (S), `invalid_return_type_for_catch_error` (CS), `invalid_super_formal_parameter_location` (S), `label_undefined` (PS), `no_default_super_constructor` (S), `unqualified_reference_to_static_member_of_extended_type` (S), `ambiguous_set_or_map_literal_either` (S), `conflicting_field_and_method` (CS), `default_value_on_required_parameter` (S), `duplicate_field_name` (S), `enum_with_abstract_member` (S), `initializer_for_non_existent_field` (S), `invalid_annotation` (CS), `invalid_use_of_covariant` (S), `subtype_of_struct_class` (M), `conflicting_method_and_field` (CS), `ffi_native_unexpected_number_of_parameters` (S), `field_initializer_outside_constructor` (S), `invalid_modifier_on_setter` (S), `not_map_spread` (S), `return_without_value` (S), `address_position` (S), `collection_element_from_deferred_library` (MS), `conflicting_constructor_and_static_member` (S), `field_initialized_in_initializer_and_declaration` (S), `initializing_declaring_parameter` (ECS), `invalid_implementation_override` (CS), `missing_size_annotation_carray` (C), `non_generative_constructor` (C), `redirect_to_missing_constructor` (S), `redirect_to_non_const_constructor` (MS), `super_formal_parameter_type_is_not_subtype_of_associated` (S), `abi_specific_integer_mapping_unsupported` (S), `continue_label_invalid` (PS), `deprecated_optional` (ECSD), `empty_struct` (S), `enum_constant_invokes_factory_constructor` (MS), `enum_mixin_with_instance_variable` (S), `field_initialized_in_declaration_and_parameter_of_primary_constructor` (EMS), `final_initialized_in_declaration_and_constructor` (S), `import_internal_library` (S), `obsolete_colon_for_default_value` (S), `receiver_of_type_never` (CPMS), `shared_deferred_prefix` (S), `size_annotation_dimensions` (CS), `super_initializing_declaring_parameter` (ECSD), `undefined_enum_constructor` (PMS), `uri_with_interpolation` (CS), `variable_type_mismatch` (CPMSD), `yield_in_non_generator` (CPS), `ambiguous_set_or_map_literal_both` (S), `equal_elements_in_set` (S), `equal_keys_in_map` (S), `expression_in_map` (CPMS), `ffi_native_unexpected_number_of_parameters_with_receiver` (CS), `field_initialized_in_declaration_and_initializer_of_primary_constructor` (ECSD), `integer_literal_imprecise_as_double` (S), `invalid_exception_value` (CS), `invalid_field_type_in_struct` (C), `native_field_invalid_type` (CS), `non_sync_factory` (CMS), `redirect_to_abstract_class_constructor` (S), `redirect_to_type_alias_expands_to_type_parameter` (CMS), `annotation_on_pointer_field` (CMS), `built_in_identifier_as_type` (CS), `concrete_class_has_enum_superinterface` (CPMS), `conflicting_type_variable_and_member` (CMS), `creation_of_struct_or_union` (CMS), `doc_directive_missing_closing_brace` (CPMS), `duplicate_constructor` (CMS), `export_internal_library` (S), `ffi_native_invalid_duplicate_default_asset` (CMS), `ffi_native_invalid_multiple_annotations` (CMS), `field_must_be_external_in_struct` (CMS), `generic_struct_subclass` (CMS), `instance_access_to_static_member` (CPMSD), `label_in_outer_scope` (PS), `mismatched_annotation_on_struct_field` (CMS), `missing_exception_value` (CPMS), `mixin_application_concrete_super_invoked_member_type` (CS), `multiple_primary_constructor_body_declarations` (ECMSD), `non_covariant_type_parameter_position_in_representation_type` (S), `packed_annotation_alignment` (CMS), `redirect_generative_to_non_generative_constructor` (CMS), `rethrow_outside_catch` (CS), `super_invocation_not_last` (ECPSD), `tearoff_of_generative_constructor_of_abstract_class` (CMS), `type_alias_cannot_reference_itself` (CMS), `undefined_enum_constant` (CS), `uri_does_not_exist_in_doc_import` (CPMS), `yield_of_invalid_type` (CPMS), `abi_specific_integer_mapping_extra` (CPMS), `abi_specific_integer_mapping_missing` (CPMS), `class_used_as_mixin` (CPMS), `deprecated_new_in_comment_reference` (CPMS), `deprecated_subtype_of_function` (S), `doc_import_cannot_be_deferred` (CPMS), `doc_import_cannot_have_configurations` (CPMS), `extra_annotation_on_struct_field` (ECMSD), `extra_size_annotation_carray` (ECPMSD), `factory_constructor_new_name` (ECPS), `ffi_native_must_be_external` (ECPMSD), `missing_annotation_on_struct_field` (ECPMSD), `missing_field_type_in_struct` (ECPMSD), `mixin_super_class_constraint_non_interface` (CMS), `multiple_super_initializers` (ECPMSD), `native_field_missing_type` (ECPMSD), `native_field_not_static` (ECPMSD), `non_bool_negation_expression` (ECPMSD), `non_generative_implicit_constructor` (CS), `not_binary_operator` (CSD), `packed_annotation` (ECPMSD), `primary_constructor_body_with_expression_body` (ECMSD), `static_access_to_instance_member` (ECPMSD), `super_in_enum_constructor` (ECPMSD), `type_check_with_null` (CPMSD), `type_test_with_non_type` (CPMSD), `undefined_annotation` (CPMSD).
- **D** (31): `const_with_non_const` (S), `const_with_undefined_constructor` (S), `const_eval_throws_exception` (CSD), `const_initialized_with_non_constant_value` (CMSD), `const_with_non_constant_argument` (CMS), `invalid_constant` (CMSD), `invalid_type_argument_in_const_literal` (S), `non_constant_relational_pattern_expression` (MSD), `recursive_constant_constructor` (S), `const_constructor_with_mixin_with_field` (S), `const_eval_property_access` (CPSD), `constant_pattern_with_non_constant_expression` (CPSD), `recursive_compile_time_constant` (CPS), `const_constructor_with_non_const_super` (S), `non_const_generative_enum_constructor` (S), `const_constructor_with_non_final_field` (S), `const_type_parameter` (CPMS), `const_constructor_field_type_mismatch` (CPMSD), `const_eval_method_invocation` (CPMSD), `const_eval_type_bool` (PMSD), `const_constructor_param_type_mismatch` (CMSD), `const_constructor_with_field_initialized_by_non_const` (CPMSD), `non_constant_default_value` (CPMSD), `non_constant_type_argument` (CPSD), `const_with_undefined_constructor_default` (ECPMSD), `non_constant_annotation_constructor` (ECPMSD), `non_constant_case_expression` (ECPMSD), `const_constructor_throws_exception` (ECPMSD), `const_eval_type_string` (ECPMSD), `const_initialized_with_non_constant_value_from_deferred_library` (ECPMSD), `non_constant_list_element` (ECPMSD).
- **E** (37): `expected_token` (S), `missing_identifier` (CMS), `expected_identifier_but_got_keyword` (CPSD), `representation_field_modifier` (CPSD), `missing_const_final_var_or_type` (CPMSD), `unexpected_token` (CPMSD), `expected_class_member` (ECPMSD), `extraneous_modifier` (CPMSD), `unterminated_string_literal` (ECPMSD), `expected_representation_type` (CMSD), `missing_statement` (ECPMSD), `unsupported_operator` (ECPMSD), `expected_representation_field` (CMSD), `const_primary_constructor_with_body` (ECPMSD), `expected_body` (ECPMSD), `extraneous_modifier_in_primary_constructor` (ECPMSD), `missing_function_body` (ECPMSD), `annotation_with_type_arguments_uninstantiated` (—), `empty_record_type_named_fields_list` (—), `expected_executable` (—), `expected_type_name` (—), `missing_digit` (—), `missing_function_parameters` (—), `missing_method_parameters` (—), `record_type_one_positional_no_trailing_comma` (—), `break_outside_of_loop` (—), `constructor_with_return_type` (—), `continue_outside_of_loop` (—), `continue_without_label_in_case` (—), `external_constructor_with_field_initializers` (—), `external_factory_redirection` (—), `illegal_character` (—), `invalid_super_in_initializer` (—), `invalid_unicode_escape_started` (—), `missing_hex_digit` (—), `named_function_expression` (—), `static_constructor` (—).
- **F** (10): `unused_field_from_primary_constructor` (S), `unused_element_parameter` (S), `non_type_as_type_argument` (S), `unused_label` (S), `part_of_non_part` (S), `type_test_with_undefined_name` (PS), `ambiguous_export` (S), `deprecated_member_use` (S), `type_annotation_deferred_class` (S), `undefined_identifier_await` (S).

**O que a rodada 4 não escreveu** (para quem for implementar saber onde ainda precisa ler a fonte):

1. **Campo "Supressões e ordem" interpretado, código por código, em A, B e C.** Para 250 códigos
   (A 44, B 51, C 155) a rodada 4 traz só os emissores e os desvios do método emissor copiados da fonte
   por script (A.R4-3, B.R4-1, C.R4-1). Os 41 códigos de A.R4-1 e A.R4-2 foram escritos à mão.
2. **Os 15 códigos que só existem no 3.13** (item 2 acima) não têm especificação pela fonte.
3. **F.R4-1**: estão escritas as seções 1 a 3 (escopos, escopo de importação, `NamedTypeResolver`).
   As seções previstas sobre identificadores em expressão, diretivas e a tabela "no DartForge" por
   regra não foram escritas; os 55 casos rodados estão transcritos em F.R4-1.9 e os seis campos de cada
   código continuam os da rodada 3.
4. **Planos (§G)**: T1 não rodou os três casos de homônimos entre partes e entre bibliotecas; T4 não
   fixou o LUB com `InvalidType`; T8 depende de um ponto não verificado de D.R4-1 (a ordem do último
   grupo de alvos). Cada plano diz isso no próprio texto.

---


## A. Invocação, argumentos, membros, atribuição

### Especificação — família A (invocação, argumentos, membros, atribuição) — r3-a

Fonte: analyzer **3.6.2** (`E:\references\dart-sdk-3.6.2\pkg\analyzer\lib`, `…\_fe_analyzer_shared\lib`, `…\analyzer\messages.yaml`); citações `analyzer/lib/src/...:linha` relativas a `E:\references\dart-sdk-3.6.2\pkg\`. Números de perda de `E:\dftemp\analise\trab\familia-A.txt` (placar base `placar-base-r3.txt`).

Cobertura: os 101 códigos da lista, agrupados por mecanismo (a ordem dentro de cada grupo segue a perda):

1. Atribuibilidade — argumentos, atribuições, retornos, coleções, `await`/`throw`/`for-in` (19 códigos).
2. Membros indefinidos, invocações e `void` (16).
3. `this`/`super` em contexto inválido, atribuição a final/const (10).
4. Declarações de extension type, `main`, diretivas de doc, FFI (24).
5. Argumentos de tipo — limites, inferência, contagem (17).
6. Aridade de argumentos, `super.x`, construtores indefinidos (15).

Achados transversais (valem para vários códigos):

- **Nome vs. tipo nos argumentos.** `UNDEFINED_METHOD {1}` é o **nome do elemento** (`'C'`, sem argumentos de tipo; `'Function'` para tipos de função; `'<unknown>'` para o resto); `UNDEFINED_GETTER/SETTER/OPERATOR {1}` de instância é o **DartType** (com argumentos); os estáticos usam o nome da classe (`typeReference.name`). Vários "mensagem errada" e os FP de `{1}` vazio vêm daí.
- **Chamada estática indefinida `C.m()`** sai hoje como `UNDEFINED_GETTER` sem `{1}`; no analyzer é `UNDEFINED_METHOD [m, C]` (ou `NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT` para `C.new()`).
- **`f()` indefinido** sai hoje como `UNDEFINED_IDENTIFIER`; no analyzer é `UNDEFINED_FUNCTION` (sem `this`) ou `UNDEFINED_METHOD [f, classe]` (com `this`).
- **Top-merge (NNBD_TOP_MERGE)** das assinaturas combinadas de interfaces: falta; causa ~60 divergências em `argument_type_not_assignable`, `use_of_void_result`, `return_of_invalid_type(_from_closure)` (`nnbd/top_merge/*`, `extension_type/combined_member_signature_error_test`).
- **Pontos de checagem ausentes** na atribuibilidade: `==` (parâmetro tornado anulável), `++`/`--` (`int` contra o parâmetro do operador, no operando; resultado contra o `writeType`, no nó), atribuição composta e `??=` (resultado do operador contra o `writeType`; lado direito contra o parâmetro), índice de leitura e escrita, valor padrão de parâmetro, elementos de coleção **não const**, closures `async`, `throw`, `await`, `for-in`.
- **`getErrorNode`** (desembrulha parênteses e cascata) vale para argumentos, inicializadores de variável e valores padrão; **não** para o lado direito de atribuição.
- **Tear-off implícita de `call`** (objeto com `call` em contexto de tipo de função) falta na inferência: origina FP e mensagens erradas em `invalid_assignment`.
- **Aridade** (já reescrita no working tree, a validar): posição do excedente/token depois do último posicional, quatro variantes do `NOT_ENOUGH_POSITIONAL_ARGUMENTS` com o nome conforme a forma da invocação, `_COULD_BE_NAMED`, `DUPLICATE_NAMED_ARGUMENT`, `super.x` contados como argumentos, entidade correta do `MISSING_REQUIRED_ARGUMENT`.
- Códigos que **não existem no 3.6.2** (julgados pelo oráculo 3.13 em arquivos de sintaxe nova): `assignment_to_primary_constructor_parameter` (40) e parte de `extension_type_declares_member_of_object`, `return_of_invalid_type` (`constructor_body/*`), `undefined_getter` (`anonymous_methods/*`), `instantiate_abstract_class` (`dot_shorthands/*`), `*_positional_arguments` (`dot_shorthands/*`).
- `implicit_super_initializer_missing_arguments` e `super_formal_parameter_without_associated_*` **já estão implementados** em `crates/analise/src/construtores.rs:515-588`, mas o placar dá *nosso 0*: o bloco não é alcançado (ou o diagnóstico não chega à paridade) — investigar antes de reimplementar.

**Estado em 2026-10-05 (escrito, não compilado).** `missing_required_param` (com e sem detalhes) ganhou
emissor em `crates/types/src/fase_requeridos.rs`, com o `RequiredParametersVerifier` da 6.11.0 aberto:
chamadas de função e de método e criações de instância; fora as anotações, as constantes de enum, as
invocações de expressão de função e `this(...)`/`super(...)`.

**Estado em 2026-10-04 (escrito, não compilado).** Dos 101 códigos, 55 não tinham emissor. Ganharam
emissor nesta data, cada um com os testes de unidade do próprio arquivo (não rodados):

| códigos | onde |
|---|---|
| `super_in_invalid_context`, `super_in_extension`, `super_in_extension_type`, `async_for_in_wrong_context`, `await_in_late_local_variable_initializer`, `expected_one_list_type_arguments`, `expected_one_set_type_arguments`, `expected_two_map_type_arguments`, `extension_type_constructor_with_super_invocation`, `extension_type_constructor_with_super_formal_parameter` | `crates/analise/src/a_contexto.rs` (só a árvore e o contexto sintático) |
| `doc_directive_missing_argument` (as três variantes), `doc_directive_has_extra_arguments`, `doc_directive_has_unexpected_named_argument`, `doc_directive_argument_wrong_format` | `crates/analise/src/a_doc.rs` (só comentários `///`; `Uri.tryParse` aproximado pelo lado seguro) |
| `main_first_positional_parameter_type`, `main_has_too_many_required_positional_parameters`, `main_has_required_named_parameters` | `crates/types/src/a_main.rs` |
| `deferred_import_of_extension` | `crates/analise/src/importacoes.rs` (`extensoes_adiadas`) |
| `mixin_instantiate` (só a criação escrita com `new`/`const`), `invocation_of_non_function` (só o parâmetro de tipo invocado) | `crates/types/src/inferencia/chamadas.rs` |
| `no_annotation_constructor_arguments` | `crates/types/src/inferencia/funcoes.rs` (`validar_anotacao`) |

Todos estão ligados em `crates/paridade/src/analise.rs`. Nenhum entrou em `verificados.txt`. Pontos
frágeis: o `await for` supõe que o intervalo do comando começa no `await`; o `super` de um
`UNDEFINED_SUPER_*` no mesmo contexto inválido ainda sai junto (a supressão é da inferência);
`for_in_with_const_variable` não foi feito porque a árvore não guarda o `const` da variável do laço.
Continuam sem emissor os demais 32 da lista (FFI, extension types por resolução, `not_instantiated_bound`,
`wrong_number_of_type_arguments_*` de construtor, método e enum, `super_formal_parameter_*`,
`implicit_super_initializer_missing_arguments`, `undefined_super_member` de setter e índice, e outros), e
os 46 que já tinham emissor não foram conferidos campo a campo.

#### Grupo 1 — atribuibilidade (argumentos, atribuições, retornos, coleções, await/throw/for-in)

Base comum (vale para vários códigos abaixo):

- `checkForAssignableExpressionAtType` (`analyzer/lib/src/generated/error_detection_helpers.dart:69-158`):
  1. se o tipo esperado não é `void` e `checkForUseOfVoidResult(expression)` relatou, para (sai `USE_OF_VOID_RESULT`, não o código pedido);
  2. se `isAssignableTo(actual, expected, strictCasts)`, nada;
  3. **nó do erro** = `getErrorNode(expression)`: desce `CascadeExpression → target` e `ParenthesizedExpression → expression` recursivamente (linhas 83-91);
  4. se `expected` é `RecordType` com exatamente 1 campo posicional, `actual` não é record, e a expressão é `ParenthesizedExpression`, e o tipo do campo é atribuível a `actual`: `RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA` **no parêntese inteiro** (sem `getErrorNode`) e para (linhas 93-108);
  5. para `ARGUMENT_TYPE_NOT_ASSIGNABLE` o 3º argumento `{2}` é `additionalInfo.join(' ')` (vazio fora de records; linhas 109-149);
  6. senão relata `errorCode` com `[actual, expected]` (DartType → `getDisplayString()` do ErrorReporter, com alias preferido).
- Atribuibilidade (`TypeSystemImpl.isAssignableTo`): `dynamic` atribuível a tudo (sem strict-casts); subtipo; **sem** conversão implícita de `call` — esta é feita antes, na inferência (o tipo da expressão já passa a ser o da tear-off `call`, ver `invalid_assignment`).

##### `argument_type_not_assignable` (perda 160: FN 86, FP 5, msg 59, pos 10)
- **Chamada de expressão com tipo parâmetro de tipo (2026-10-08):** `createT()('')` com `T extends int Function(int)`
  é `FunctionExpressionInvocation` do parser: o resolver do 3.6.2 não leva `T` ao limite; o `TypePropertyResolver`
  acha o `call` do tipo de função do limite só como `callFunctionType`, sem elemento, e a chamada fica `dynamic`, sem
  conferir os argumentos (`function_expression_invocation_resolver.dart:80-95`). A chamada por nome ou propriedade
  (`tValue('')`) é reescrita pelo `MethodInvocationResolver` com `resolveToBound` e confere. `chamadas.rs`
  (`invocar_valor`) distingue pelo alvo: nem identificador nem propriedade.

**Feito em 2026-10-07:** `super == x` confere `x` contra o parâmetro do `==` da cadeia de `super`, tornado
anulável (`_resolveEqual`, 3.6.2 `binary_expression_resolver.dart:117-124`): `super == 'a'` contra
`==(covariant num other)` relata `num?`. No DartForge, `condicao_binaria` por `buscar_operador_super`.
- **Emissão:** `_checkForArgumentTypeNotAssignableForArgument` (`analyzer/lib/src/generated/error_detection_helpers.dart:348-368`) → `checkForArgumentTypeNotAssignable` → `checkForAssignableExpressionAtType`. Chamado de:
  - argumentos de invocações: `checkForArgumentTypesNotAssignableInList` (`analyzer/lib/src/generated/resolver.dart:511-520`, um por argumento com `staticParameterElement`), feito pelos resolvedores de `MethodInvocation`, `FunctionExpressionInvocation`, `InstanceCreationExpression` (`analyzer/lib/src/dart/resolver/instance_creation_expression_resolver.dart:72`), redirecionamento/super, anotações e constantes de enum (`analyzer/lib/src/generated/resolver.dart:2556-2559`);
  - operador binário definido pelo usuário: `_resolveUserDefinableElement` + `checkForArgumentTypeNotAssignableForArgument(right)` (`analyzer/lib/src/dart/resolver/binary_expression_resolver.dart:301`);
  - `==`/`!=`: `binary_expression_resolver.dart:117-124` com `promoteParameterToNullable: true` (o parâmetro de `operator ==` é tornado anulável: `'A?'`);
  - atribuição composta `a op= b` (exceto `=` e `??=`): o lado direito contra o parâmetro do operador (`analyzer/lib/src/dart/resolver/assignment_expression_resolver.dart:337-341`);
  - índice: `checkIndexExpressionIndex` (`error_detection_helpers.dart:256-283`) — o índice é checado contra o parâmetro do `[]` (leitura) **e** do `[]=` (escrita) quando ambos existem (composta, `??=`, `++`): dois relatos possíveis no mesmo nó se os dois parâmetros recusam;
  - `++x`/`--x`/`x++`/`x--`: `_checkForIntNotAssignable(operand)` (`analyzer/lib/src/generated/error_verifier.dart:3899-3906`, chamado em `visitPrefixExpression` :1335-1344 e `visitPostfixExpression` :1311-1319): o tipo `int` (o `1` implícito) contra `operand.staticParameterElement` = 1º parâmetro do `operator +`/`-` resolvido (`analyzer/lib/src/dart/ast/ast.dart:6185-6192`, 14299-14310, 14510-14520). **O nó do erro é o operando** (`a` em `++a`, `a` em `a++`), não o `1`.
- **Condição exata:** `!isAssignableTo(argType, paramType)` depois do passo 1 (void) e do 4 (record de 1 campo).
- **Posição:** `getErrorNode(argumento)` — sem parênteses envolventes e, em cascata, o alvo; para argumento nomeado, a expressão (não o rótulo; `NamedExpression.expression`, `error_detection_helpers.dart:62`).
- **Mensagem:** `The argument type '{0}' can't be assigned to the parameter type '{1}'. {2}` — `{0}`,`{1}` DartType (display com alias); `{2}`:
  - só quando **ambos** são `RecordType`: `"Expected N positional arguments, but got M instead."` se `N != 0 && M != N`; `"Expected N named arguments, but got M instead."` se `N != 0 && M != N`; para cada campo nomeado do atual que não exista no esperado com mesmo nome **e mesmo tipo** (igualdade, não subtipo): ``"Unexpected named argument `x` with type `T`."`` (T com `getDisplayString()`); juntados com espaço (`error_detection_helpers.dart:109-139`);
  - fora disso, string vazia — a mensagem termina em `'. '` (ponto, espaço).
  - Records: campos nomeados exibidos **em ordem alfabética** (o `RecordType` normaliza a ordem): `'({int bb, int c})'`.
- **Supressões e ordem:** `void` antes (USE_OF_VOID_RESULT); record de 1 campo vira `RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA`; argumento sem parâmetro correspondente (aridade errada) não é checado; tipo do parâmetro `InvalidType` não relata (atribuível).
- **No DartForge:** `chamadas.rs::invocar` (`verificar_atribuivel_expr` com o molde de texto, convertido pela `ponte.rs` com `ULTIMO_OPCIONAL` que acrescenta `{2}` vazio); operadores em `expr.rs::operador_binario_com_membro` (:2218-2240) e índice de escrita em `expr.rs::escrita_indice` (:3083-3095); mensagem em `mod.rs::verificar_atribuivel` (texto) — não há `getErrorNode`, nem `{2}` de records, nem checagem para `==`, `++/--`, composta e índice de leitura.
  Causas no placar base:
  1. **46 FN `operator/equality_static_test.dart` + 2 `binary_eqEq` + 2 `super_equals…`**: `a == b` não checa `b` contra o parâmetro de `operator ==` tornado anulável (`Covar1?`), nem `super == x`.
  2. **8 FN `incrementAnd_*`**: `++a`/`a++` com `operator +(double)`/`(String)` — falta `_checkForIntNotAssignable` no operando.
  3. **4 FN `index_validR` / `map_indexSet_ifNull` / `implicit_downcast_during/indexed_*`**: índice em composta/`??=`/`++` checado só contra `[]=`; falta o `[]` de leitura, e o lado direito da composta contra o parâmetro do operador.
  **Estado em 2026-10-05 (escrito, não compilado):** o item 4 abaixo (assinatura combinada pelo `topMerge`) está
  em `crates/types/src/heranca.rs`, usado pela busca de membros da inferência (`membros.rs::membro_de_interface`).
  4. **~49 msg + 10 pos `nnbd/top_merge/*`**: a assinatura combinada de membros herdados de várias interfaces deve ser o **NNBD_TOP_MERGE** dos tipos (`Object?` de `FutureOr<dynamic>`/`Object?`/`void`), e nós pegamos a de uma das interfaces (`analyzer/lib/src/dart/element/inheritance_manager3.dart`, `_topMerge`/`combineSignatures`). As "posições" são o mesmo erro casado com a linha errada.
  5. **4 msg `recordType*`**: falta o `{2}` de records e a ordem alfabética dos campos nomeados no display.
  6. **1 pos + 1 FP `RecordLiteralOnePositional…`**: falta o passo 4 (`RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA`) e o `getErrorNode` (sem o parêntese).
  7. **13 FN `variance/*`**: arquivos com o experimento `variance` (não habilitado); o oráculo ainda resolve `in`/`out`… não vale perseguir.
  8. FP `records/type_inference_error_test.dart:130/137` (inferência de record com `dynamic`), `function/call_via_bound…` e `referenced_before_declaration…localFunction` (tipo de função local usada antes da declaração): casos isolados de inferência.
  **Mudança:** (a) emitir com `aviso_com_codigo(ARGUMENT_TYPE_NOT_ASSIGNABLE, getErrorNode(e), [atual, esperado, info])` com a regra de `{2}`; (b) `RecordType` exibido com nomeados ordenados (em `table.format`); (c) `==`: depois de inferir o operando direito, achar `operator ==` do tipo esquerdo promovido a não nulo e checar contra `nullable(param)`; (d) `++/--`: checar `int` contra o 1º parâmetro do operador resolvido, no span do operando; (e) composta: lado direito contra o parâmetro do operador; índice contra `[]` e `[]=`; (f) top-merge das assinaturas combinadas (fora deste arquivo: `membros.rs`/outline).

- **Feito em 2026-10-07 (índice de escrita composta, 364/368):** `checkIndexExpressionIndex` (3.6.2
  `error_detection_helpers.dart:257-287`, chamado de `resolveForWrite`, `resolver.dart:1443`) confere o índice
  contra o primeiro parâmetro do `[]` (com leitura) **e** do `[]=`: em `a[i] += v`, `++a[i]`, `a[i]++` e
  `m[k] ??= v`, `expr.rs::ler_para_escrita` agora confere também o `[]=` achado (`m['x'] ??= 0` em
  `Map<int, int>`: `'String'` para `'int'`).
- **Desambiguação (2026-10-07):** `verificar_atribuivel` (`INVALID_ASSIGNMENT` e `ARGUMENT_TYPE_NOT_ASSIGNABLE`)
  e o caminho de argumento com `{2}` saem por `aviso_com_args`: os tipos passam pela conversão do
  `ErrorReporter`, com alias e o `(where C is defined in …)` quando dois tipos do relato têm o mesmo nome
  (`regress/regress1363_test.dart:25:16`).

##### `invalid_assignment` (perda 126: FN 68, FP 33, msg 14, pos 11)
- **Emissão:**
  - declaração de variável com inicializador: `VariableDeclarationResolver.resolve` → `checkForAssignableExpressionAtType(initializer, type, element.type, INVALID_ASSIGNMENT)` (`analyzer/lib/src/dart/resolver/variable_declaration_resolver.dart:80-87`) — com `getErrorNode` (parênteses/cascata);
  - valor padrão de parâmetro: `ErrorVerifier.visitDefaultFormalParameter` (`analyzer/lib/src/generated/error_verifier.dart:640-652`) — idem;
  - atribuição `=`/composta/`??=`: `AssignmentExpressionResolver._checkForInvalidAssignment(writeType, right, assignedType)` (`analyzer/lib/src/dart/resolver/assignment_expression_resolver.dart:117-155`; chamado em :331-336) — erro **no `rightHandSide` sem `getErrorNode`** (o parêntese conta); `assignedType` é o tipo do lado direito para `=`, o **retorno do operador** para composta (`i += d` com `int + double → double`) e `LUB` para `??=`;
  - `++x`/`x++`: `_checkForInvalidAssignmentIncDec` (`analyzer/lib/src/dart/resolver/prefix_expression_resolver.dart:96-110`, `postfix_expression_resolver.dart:73-88`) — o retorno do operador `+`/`-` contra o `writeType` do operando, **no nó inteiro** (`++a`, `a++`); nos `?.` o tipo do resultado é anulável (`E?`).
  - também pelo conversor do CFE (`analyzer/lib/src/fasta/error_converter.dart:184-190`), só para erros de parser — irrelevante.
- **Condição exata:** void primeiro (`USE_OF_VOID_RESULT`), record de 1 campo (`RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA`), senão `!isAssignableTo(right, write)`.
- **O `writeType`** (`ResolverVisitor.setWriteElement`, 3.6.2 `resolver.dart:1709-1747`): `InvalidType` (ou
  `dynamic` em alvo `dynamic`) salvo quando o elemento de escrita é um setter (o sintético dá o tipo da
  variável) ou um `VariableElement` (local, parâmetro); no índice, o 2º parâmetro do `[]=`. A escrita que
  cai no elemento de recuperação (campo `final` ou getter sem setter, variável de topo `final`/`const`,
  receptor `Never` sem `[]=`) tem `InvalidType` e não dá este código (`finalOne ??= null` só relata
  `ASSIGNMENT_TO_FINAL`; `x[0] ??= 1` com `x: Never`, nada). No DartForge, `tipo_de_escrita_nome` e o braço
  `Index` de `ler_para_escrita`.
- **Posição:** acima; inicializador/padrão sem parênteses e cascata; atribuição no lado direito como escrito.
- **Mensagem:** `A value of type '{0}' can't be assigned to a variable of type '{1}'.` — DartTypes; quando dois tipos exibem o mesmo nome e são elementos diferentes, o ErrorReporter acrescenta `' (where C is defined in <uri>)'` a cada um (`analyzer/lib/error/listener.dart:362-425`, `_convertTypeNames`: só quando dois argumentos DartType da mesma mensagem têm o mesmo display e elementos distintos com o mesmo nome; o caminho é `source.fullName`): `'C (where C is defined in <raiz>/regress/regress1363_lib.dart)'`.
  Tear-off implícita de `call`: quando o valor é de classe com `call` e o contexto é tipo de função, a inferência insere `.call` (`ImplicitCallReference`); o `{0}` é o tipo da tear-off (`'void Function(int)'`, genérico instanciado se `constructor-tearoffs` habilitado — senão o genérico cru, `'T Function<T>(T)'`).
- **Supressões e ordem:** inicializador de **campo** em construtor não é este código (`FIELD_INITIALIZER_NOT_ASSIGNABLE`/`CONST_FIELD_INITIALIZER_NOT_ASSIGNABLE`, `error_detection_helpers.dart:160-...`); atribuição a método (`ASSIGNMENT_TO_METHOD`) não tem `writeType` → não relata; `writeType` dynamic/inválido não relata.
- **No DartForge:** `expr.rs::atribuicao` (:2800-2830, só `=`), `instrucoes.rs:643` (variável local), `mod.rs:418` (topo/campo), `funcoes.rs:157` (inicializador de campo — **errado**: deveria ser `FIELD_INITIALIZER_NOT_ASSIGNABLE`). Mensagem via `mod.rs::verificar_atribuivel` (texto + ponte).
  Causas:
  1. **27 FN `operator/number_operator_error_test.dart` + 3 `implicit_downcast_during/compound_assignment` + 1 `if_null_assignment` + `InvalidAssignment__ifNullAssignment`**: composta e `??=` não checam o tipo resultante (`retorno do operador`/LUB) contra o `writeType`.
  2. **4 FN `prefix/postfixExpression_*`** + **4 msg `null_aware/increment_decrement_test`** (`E` vs `E?`): falta `_checkForInvalidAssignmentIncDec` e, em `?.`, o tipo anulável.
  3. **2 FN `defaultValue_*`**: valor padrão de parâmetro não checado.
  4. **8 FP + 6 msg `ImplicitCallReference_*`/`callable_test`/`method_implicit_tear_off…`**: falta a conversão implícita de `call` na inferência (tipo atual deve ser a tear-off, e quando ela é atribuível não há erro).
  5. **6 FN `generic/function_subtype_parametrized_typedef_test`**: subtipo entre typedefs de função parametrizados (`H<B>` vs `H<A>`), provavelmente variância do parâmetro do typedef.
  6. **FP `field_initializer_not_assignable/*`, `const_field_initializer…`, `implicit_downcast_during/constructor_initializer_test`**: código errado no inicializador de campo.
  7. **FP `assignment_to_method/*` (4)**: atribuição a método relata também `INVALID_ASSIGNMENT`.
  8. **5 FP `spread_collections/null_spread_test`**: `[...?x]` com `x: X extends Null` deve ter elemento `Never` (o tipo do spread de `Null`/`Never`), nós deduzimos `dynamic`.
  9. **2 FP + 1 pos `compile_time_constant/static2/3_test`, 3 FP `prefix_shadowed…`, 2 FP `RecordLiteralOnePositional…`**: respectivamente erro em contexto já inválido, nome sombreado, e passo 4.
  10. **pos `cascadeExpression`/`parenthesizedExpression`/`null_aware/assignment_test` (6)**: falta `getErrorNode` nas declarações e, em `?.` atribuição, o span.
  11. **msg `regress1363_test`**: falta a desambiguação `(where C is defined in …)`.
  **Mudança:** completar os quatro pontos de emissão com as regras acima, emitir com `aviso_com_codigo`, trocar o código do inicializador de campo, e implementar a tear-off implícita de `call` na inferência de expressões com contexto de função.
- **Correção desta especificação (2026-10-06): o tipo de escrita.** `ResolverVisitor.setWriteElement`
  (`generated/resolver.dart:1709-1761`) só dá tipo de escrita a setter (o do parâmetro, ou o da variável no sintético)
  e a variável; para método, getter recuperado ou nada, o `writeType` é `InvalidType` (`dynamic` só num alvo dinâmico).
  Daí `a.foo = 0` com `foo` método sai só `assignment_to_method`, sem `invalid_assignment`.
- **Correções (2026-10-06, continuação):** (1) objeto com `call` **genérico** num contexto de função (`int
  Function(int) f = C()` com `T call<T extends num>(T)`): depois do `ImplicitCallReference`, o
  `insertGenericFunctionInstantiation` instancia o `call` pelo contexto (o `flatten` sem `?`, INFERENCIA §8.4) antes
  da atribuibilidade; (2) `x?[0]++`/`a?.b++`: o `_checkForInvalidAssignmentIncDec` confere o **retorno do operador**
  (`int`), sem o `?` do encurtamento nulo, contra o tipo de escrita.

- **Correções de 2026-10-06 (263/274; msg 17 para 5, FN 21 para 11):**
  - Referência implícita a `call` (`_insertImplicitCallReference`, `resolver.dart:4085-4142`): a mensagem mostra o tipo do `call` instanciado pelo contexto (`void Function(int, dynamic)`, `dynamic Function(dynamic)`), não o da classe (`expr.rs::verificar_atribuivel_expr_em`, com `tipo_do_call_implicito` e `instanciar_funcao_pelo_contexto`).
  - `?.` sobre literal de tipo: o 3.6.2 só dispensa o encurtamento quando o alvo é um `SimpleIdentifier` que nomeia um `InterfaceElement` (`resolver.dart:1777-1794`, `:3215-3226`); `h.D?.x = e`, `C<int>?.x`, alias e extensão encurtam, e o tipo fica anulável (`E?`). `expr.rs::encurtamento_dispensado`, usado em `propriedade`, `escrita_propriedade` e na invocação estática de `chamadas.rs`.
  - Valor padrão: `ErrorVerifier.visitDefaultFormalParameter` (`error_verifier.dart:640-651`) confere o valor contra o tipo do parâmetro (`INVALID_ASSIGNMENT`), também no `this.x = 'foo'` (tipo do campo). Não existia (`funcoes.rs::valor_padrao_atribuivel`).
  - Tipo de extensão sem `Object` e `this` em contexto estático: ver INFERENCIA §4.1.

##### `argument_type_not_assignable_to_error_handler` (perda 33: FN 33)
- **Emissão:** `ErrorHandlerVerifier.verifyMethodInvocation` (`analyzer/lib/src/error/error_handler_verifier.dart:43-150`), chamado do `BestPracticesVerifier.visitMethodInvocation` (`analyzer/lib/src/error/best_practices_verifier.dart:630`) — fase de avisos (WarningCode).
- **Condição exata:** com alvo `t` não nulo e argumentos não vazios:
  - `t.catchError(cb, …)` com `t` do tipo `Future` (`isDartAsyncFuture`) e 1º argumento posicional: `_checkFutureCatchErrorOnError` → `_checkErrorHandlerFunctionType(cb, tipo, FutureOr<T>)`;
  - `t.then(…, onError: cb)` (Future), `Stream.listen(…, onError: cb)`: com `checkFirstParameterType` só se `cb.expression is FunctionExpression`, retorno esperado `void`;
  - `Stream.handleError(cb)`, `StreamSubscription.onError(cb)`: 1º posicional, `checkFirstParameterType: cb is FunctionExpression`, retorno `void`.
  - `_checkErrorHandlerFunctionType` (:160-213) relata se: sem parâmetros; 1º parâmetro nomeado; (se checa) `Object` não é subtipo do tipo do 1º; com 2 parâmetros, o 2º nomeado ou `StackTrace` não subtipo dele; mais de 2 parâmetros.
  - Só quando o tipo estático do callback é `FunctionType`.
- **Posição:** a expressão do callback (`NamedExpression` inteiro para `onError:`, pois `callback` é o argumento nomeado? — não: em `then`/`listen` o nó passado é o `NamedExpression` (`callback`), então o span **inclui o rótulo** `onError: …`); nos posicionais, a expressão.
- **Mensagem:** `The argument type '{0}' can't be assigned to the parameter type '{1} Function(Object)' or '{1} Function(Object, StackTrace)'.` — `{0}` o tipo de função do callback, `{1}` `FutureOr<T>` (catchError) ou `void`.
- **Supressões e ordem:** independe de `ARGUMENT_TYPE_NOT_ASSIGNABLE` (que normalmente não sai porque `onError` é `Function`).
- **No DartForge:** não existe. **Mudança:** implementar no fim da invocação de método (`chamadas.rs`, ramo `Busca::Achado` com `m.metodo`), com os tipos já inferidos dos argumentos.

##### `return_of_invalid_type` (perda 17: FN 13, msg 4)
- **Emissão:** `ReturnTypeVerifier._checkReturnExpression` (`analyzer/lib/src/error/return_type_verifier.dart:134-275`), de `verifyReturnStatement`/`verifyExpressionFunctionBody` (ErrorVerifier).
- **Condição exata:** pula se o retorno declarado é ilegal (`hasLegalReturnType`) ou gerador. Síncrono: `T` void e `S` ∉ {void, dynamic, Null}; `S` void e `T` ∉ {void, dynamic}; senão (record de 1 campo → `RECORD_LITERAL_…`) `!isAssignableTo(S, T)`. Assíncrono: `T_v = futureValueType(T)`, `flatten(S)`; mesmas regras com `T_v`/`flatten(S)`, e o último teste é `!isAssignableTo(S, T_v) && !isSubtypeOf(flatten(S), T_v)`.
- **Posição:** a expressão retornada (sem desembrulhar parênteses).
- **Mensagem:** variantes por tipo de executável: `…_FROM_CONSTRUCTOR` (`'{2}'` = `displayName` do construtor: `C` ou `C.named`), `…_FROM_FUNCTION` (função de topo **ou local**), `…_FROM_METHOD`; `A value of type '{0}' can't be returned from the {function|method|constructor} '{2}' because it has a return type of '{1}'.`; `RETURN_OF_INVALID_TYPE_FROM_CATCH_ERROR` (WarningCode) quando dentro do `onError` de `catchError`.
- **Supressões e ordem:** closures são o código `…_FROM_CLOSURE`.
- **No DartForge:** `funcoes.rs::verificar_retorno_de_expressao` e `instrucoes.rs` (return). Causas: **9 FN `constructor_body/*`** são de sintaxe nova (corpos de construtor secundário com `return null`, oráculo 3.13 — construtores de classe/enum/extension type com `return expr`: o retorno `Null` contra o tipo da classe); **2 FN `enum/enhanced_enums_error_test`** (retorno em construtor que conflita com membro); **1 `NotMapSpread…`** (`{...iterable}` em contexto de Map infere `Set<int>`); **1 `UndefinedGetter__propertyAccess_functio…`** (`Function` retornado de `void`). **4 msg `extension_type/combined_member_signature_error_test`**: tipo combinado `(Object?, Object?)` vs nosso `(Object?, dynamic)` — mesmo top-merge do `argument_type_not_assignable`.
  **Mudança:** retornos em corpos de construtor (para 3.13: só quando a sintaxe nova está ativa), top-merge, e o caso `{...x}` em contexto de mapa.
- **Feito em 2026-10-07 (165/167):** construtor gerador com `=> e` (`funcoes.rs`, depois do corpo): o
  `verifyExpressionFunctionBody` confere `e` contra o `returnType` do construtor, o tipo da classe nos próprios
  parâmetros de tipo, com `RETURN_OF_INVALID_TYPE_FROM_CONSTRUCTOR` e o nome `C`/`C.nome`; vale para classe,
  enum e tipo de extensão, inclusive o construtor externo, o redirecionador e o `static int C() => 37`
  recuperado como construtor. O corpo `this => e` do construtor primário (o k2 elaborado da classe e do
  enum, ou a parte `this` com `parte_primaria` no tipo de extensão) fica de fora: o erro ali é o do corpo
  primário com `=>`.

##### `return_of_invalid_type_from_closure` (perda 17: FN 16, msg 1)
- **Emissão:** `return_type_verifier.dart:160-166` (`enclosingExecutable.isClosure`).
- **Condição exata:** a mesma do anterior; `T` é o **tipo de retorno inferido/imposto da closure** (para `() async {…}` em contexto `void Function()` é `Future<void>`; `T_v = void`).
- **Posição:** a expressão retornada.
- **Mensagem:** `The returned type '{0}' isn't returnable from a '{1}' function, as required by the closure's context.` — `{1}` é `T` (ex.: `'Future<void>'`).
- **No DartForge:** `funcoes.rs` (closure: `RETURN_OF_INVALID_TYPE_FROM_CLOSURE` existe para síncronas). Causa das **16 FN `invalid_returns/async_invalid_return_*`**: closures `async` não são checadas (regras com `flatten`/`futureValueType`). **1 msg**: top-merge.
  **Mudança:** aplicar as regras assíncronas às closures `async` com `T` = retorno da closure (`Future<flatten(contexto)>`).
- **Feito (32/32):** o `ReturnTypeVerifier` confere todo `return` de toda closure (não só quando o contexto trocou o retorno inferido) contra o tipo de retorno dela: o inferido pelo `computeInferredReturnType`, com o `_clampToContextType` (`body_inference_context.dart:91-137`: contexto `void` ou, em `async`, `FutureOr<void>`, dá `void`; senão `T` se `T <: R`, senão `R`) e o embrulho `Future<flatten(S)>`. O `=> e` com retorno achatado `void` aceita qualquer expressão (`verifyExpressionFunctionBody`, `return_type_verifier.dart:45-52`). `funcoes.rs` chama `retornos_da_closure` com o tipo estático da closure.

##### `list_element_type_not_assignable` (perda 14: FN 14)
- **Emissão:** `ErrorVerifier._checkForListElementTypeNotAssignable` (`analyzer/lib/src/generated/error_verifier.dart:4097-4122`) para **todo** `ListLiteral` (const ou não) → `LiteralElementVerifier` (`analyzer/lib/src/error/literal_element_verifier.dart:52-66` elementos; :176-262 spread). Para literais const também o `ConstantVerifier` (`analyzer/lib/src/dart/constant/constant_verifier.dart:1205-1213`) quando o valor avaliado não casa (duplicata possível no mesmo nó).
- **Condição exata:** tipo do elemento = `typeArguments[0]` do tipo **estático** do literal. Por elemento: expressão → void primeiro, depois `!isAssignableTo(tipo, E)`; `if`/`for` → recursivo nos corpos (`then` e `else`); `...x` → se `x` dynamic/Never/Null(com `?`) nada; senão `Iterable<T>` de `x` e `!isAssignableTo(T, E)` (com a tear-off de `call` como exceção); elemento null-aware `?e` (3.8+) → o tipo promovido a não nulo.
- **Posição:** o elemento (expressão) ou a expressão do spread.
- **Mensagem:** `The element type '{0}' can't be assigned to the list type '{1}'.`
- **No DartForge:** só o verificador de constantes (`crates/types/src/constantes/verificador.rs:1055`). **Todas as 14 FN** são literais não const (ou const com o ramo `if` não avaliado). **Mudança:** em `colecoes.rs`, depois de inferir o literal, percorrer os elementos com a regra do `LiteralElementVerifier` (usar o tipo final do literal); evitar duplicata com o relato de constante (mesmo código, mesmo nó: emitir uma vez).

##### `map_key_type_not_assignable` (perda 12: FN 12)
- **Corrigido em 2026-10-08** (os quatro `UseOfVoidResult__mapLiteralEntry_*`, julgados pelo 3.13.4): o
  `_verifyMapLiteralEntry` (3.13.4 `literal_element_verifier.dart:144-172`) só para a entrada na chave (ou no valor)
  `void` quando ela tem o `?` ou quando o tipo correspondente do mapa não é `void`; com `<int, void>` e valor `void`, a
  chave ainda é conferida (`types::inferencia::colecoes`). Antes, qualquer `void` na entrada a pulava inteira.
- **Emissão:** `LiteralElementVerifier._verifyMapLiteralEntry` (`literal_element_verifier.dart:121-155`) e `_verifySpreadForMap` (:266-325), via `ErrorVerifier._checkForMapTypeNotAssignable`; const: `constant_verifier.dart:1286-1292`.
- **Condição exata:** void primeiro (chave e valor); chave com `?` (null-aware) é promovida a não nula; `!isAssignableTo(K, mapKeyType)`. Spread de mapa: `Map<K, V>` de `x`.
- **Posição:** a chave (ou a expressão do spread).
- **Mensagem:** `The element type '{0}' can't be assigned to the map key type '{1}'.`
- **No DartForge:** só constantes. Causas: as 12 FN são literais não const / ramos não avaliados / spreads / chaves null-aware. **Mudança:** com `list_element…`, em `colecoes.rs`.

##### `map_value_type_not_assignable` (perda 12: FN 12)
- Igual ao anterior para o valor (`literal_element_verifier.dart:157-170`, spread :312-323; const `constant_verifier.dart` logo abaixo da chave). **Mensagem:** `The element type '{0}' can't be assigned to the map value type '{1}'.` **Posição:** o valor. Causa e mudança idênticas.

##### `set_element_type_not_assignable` (perda 7: FN 7)
- Igual a `list_element_type_not_assignable` com `forSet` (`literal_element_verifier.dart:58-61`; `ErrorVerifier._checkForSetElementTypeNotAssignable3` em `error_verifier.dart:5275-...`; const `constant_verifier.dart:1360-1366`). **Mensagem:** `The element type '{0}' can't be assigned to the set type '{1}'.` Causa e mudança idênticas.

##### `extension_override_argument_not_assignable` (perda 4: FN 4)
- **Emissão:** `ExtensionMemberResolver.resolveOverride` → bloco final (`analyzer/lib/src/dart/resolver/extension_member_resolver.dart:227-243`).
- **Condição exata:** receptor `void` → `USE_OF_VOID_RESULT` no receptor; senão `!isAssignableTo(receiverType, extendedType)` com `extendedType` já substituído pelos argumentos de tipo (explícitos ou inferidos).
- **Posição:** a expressão do argumento (`E(arg)` → `arg`).
- **Mensagem:** `The type of the argument to the extension override '{0}' isn't assignable to the extended type '{1}'.` (`{0}` tipo do argumento, `{1}` tipo estendido).
- **No DartForge:** `chamadas.rs::chamada` (ramo `RefTipo::Extensao`, :515-539) infere o argumento e não checa. **Mudança:** após `extensao_aplicavel`, checar atribuibilidade ao `on` substituído e relatar no argumento.
- **Corrigido em 2026-10-07 (4/4, FP 0):** o override alvo de `?.`, `?[` ou `?..` (`isNullAware`) confere o tipo
  não anulável do argumento (`E(a)?.foo()` com `a: int?` e `on int` não relata).

##### `await_of_incompatible_type` (perda 16: FN 16)
- **Emissão:** `ErrorVerifier.visitAwaitExpression` → `_checkForAwaitOfIncompatibleType` (`analyzer/lib/src/generated/error_verifier.dart:2214-2223`).
- **Condição exata:** `isIncompatibleWithAwait(T)` (`analyzer/lib/src/dart/element/type_system.dart:971-1001`): `S?` → recursivo em `S`; extension type que **não** é subtipo de `Future<Object?>`; parâmetro de tipo promovido `X & B` → recursivo em `B`; parâmetro de tipo com limite `S` → recursivo em `S`.
- **Posição:** o token `await`.
- **Mensagem:** `The 'await' expression can't be used for an expression with an extension type that is not a subtype of 'Future'.` (sem argumentos).
- **Supressões e ordem:** sai junto com `AWAIT_IN_WRONG_CONTEXT`/`USE_OF_VOID_RESULT` se houver (independentes).
- **No DartForge:** `expr.rs` (`ExprKind::Await`, :1070). **Mudança:** checar com a função recursiva e relatar no `await` (span de 5 caracteres no início da expressão).

##### `throw_of_invalid_type` (perda 16: FN 16)
- **Emissão:** `ErrorVerifier.visitThrowExpression` → `_checkForThrowOfInvalidType` (`analyzer/lib/src/generated/error_verifier.dart:5336-5347`); antes, `checkForUseOfVoidResult(expression)` (:1551).
- **Condição exata:** `!isAssignableTo(type, Object)` (não anulável). `dynamic` passa; `void` não é atribuível a `Object` → relata **também** (os dois: `USE_OF_VOID_RESULT` e `THROW_OF_INVALID_TYPE 'void'`, como no oráculo `UseOfVoidResult__throwExpression…`).
- **Posição:** a expressão lançada.
- **Mensagem:** `The type '{0}' of the thrown expression must be assignable to 'Object'.`
- **No DartForge:** `expr.rs` (`ExprKind::Throw`, :1076) não checa. **Mudança:** checar `atribuivel(t, Object)` e relatar na expressão.

##### `for_in_of_invalid_type` (perda 17: FN 17)
- **Emissão:** `ErrorVerifier._checkForEachParts` (`analyzer/lib/src/generated/error_verifier.dart:3103-3170`), de `visitForEachPartsWithDeclaration`/`WithIdentifier`; e `patternForInExpressionIsNotIterable` (`analyzer/lib/src/dart/resolver/shared_type_analyzer.dart:161-171`) para `for (var (a, b) in x)`.
- **Condição exata:** iterável `void` → `USE_OF_VOID_RESULT` e para; `dynamic` só com strict-casts; **tipo anulável (`isNullable`) → para** (o erro é o `UNCHECKED_USE_OF_NULLABLE_VALUE`); variável sem elemento → para; `T = resolveToBound(tipo)` (um parâmetro de tipo `SQ extends Stream<Object?>?` vira `Stream<Object?>?`, e aí relata!); topo → requerido; `!isAssignableTo(T, Iterable<dynamic>/Stream<dynamic>)` → relata.
- **Posição:** a expressão iterável.
- **Mensagem:** `The type '{0}' used in the 'for' loop must implement '{1}'.` — `{0}` o tipo **depois** de `resolveToBound`; `{1}` a string literal `'Iterable'` ou `'Stream'` (com `await`).
- **No DartForge:** `instrucoes.rs::cabecalho_for_in` (:703) não relata; o mesmo para `for` em coleções (`colecoes.rs`, `CollectionElement::ForIn`). **Mudança:** implementar a regra no cabeçalho de `for-in` de instrução e de elemento de coleção.
- **Padrão (2026-10-07, 17/17 sem FP):** `for (var (p) in e)` não passa por `_checkForEachParts`; o `analyzePatternForIn` (`type_analyzer.dart:1445-1463`) relata só se `asInstanceOf(Iterable/Stream)` falha, e `asInstanceOf` ignora o `?` e vai pelo limite do parâmetro de tipo: `T extends Iterable<int>?` não relata (o erro é o do nulo). Argumentos: o tipo sem `resolveToBound` e sempre `'Iterable'`. Em `instrucoes.rs::for_in_tipo_invalido`.

##### `for_in_of_invalid_element_type` (perda 14: FN 14)
- **Emissão:** mesma função (`error_verifier.dart:3172-3233`).
- **Condição exata:** com o tipo da sequência `Iterable<E>`/`Stream<E>` (`asInstanceOf`), `!isAssignableTo(E, tipo da variável)` — exceto se a tear-off implícita de `call` de `E` é atribuível.
- **Posição:** a expressão iterável.
- **Mensagem:** `The type '{0}' used in the 'for' loop must implement '{1}' with a type argument that can be assigned to '{2}'.` — `{0}` tipo do iterável (resolvido), `{1}` `'Iterable'`/`'Stream'`, `{2}` tipo da variável (declarada, ou a do identificador/variável existente no `for (x in …)`).
- **No DartForge:** inexistente. **Mudança:** junto com o anterior.

##### `for_in_with_const_variable` (perda 1: FN 1)
- **Emissão:** `ErrorVerifier.visitForEachPartsWithDeclaration` (`analyzer/lib/src/generated/error_verifier.dart:912-923`), só se `_checkForEachParts` devolveu `true`.
- **Condição exata:** variável do laço declarada `const`.
- **Posição:** o token `const`.
- **Mensagem:** `A for-in loop variable can't be a 'const'.`
- **No DartForge:** inexistente (o parser aceita `const` no `for`?). **Mudança:** em `instrucoes.rs::cabecalho_for_in`, se o alvo declarado é `const`, relatar no token.

##### `async_for_in_wrong_context` (perda 3: FN 3)
- **Emissão:** erro do **parser** do CFE (`codes_generated.dart:366`, mensagem `AsyncForInWrongContext`) convertido em `analyzer/lib/src/fasta/error_converter.dart:34-38` (`CompileTimeErrorCode.ASYNC_FOR_IN_WRONG_CONTEXT`).
- **Condição exata:** `await for` dentro de função que não é `async`/`async*` (o parser sabe o modificador do corpo).
- **Posição:** o token `await` do `await for` (offset do `await`, length 5) — ver oráculo `2:3`.
- **Mensagem:** `The async for-in loop can only be used in an async function.`
- **No DartForge:** inexistente. **Mudança:** no parser ou em `instrucoes.rs` (com o modificador da função corrente: `cx.funcoes.last().modificador`).

##### `record_literal_one_positional_no_trailing_comma` (perda 4: FN 4)
- **Emissão:** `error_detection_helpers.dart:93-108`, `assignment_expression_resolver.dart:133-146`, `return_type_verifier.dart:218-233` (o `ParserErrorCode` homônimo é de outra situação).
- **Condição exata:** tipo esperado é record de exatamente 1 campo posicional (e nenhum nomeado não importa), atual não é record, a expressão é `ParenthesizedExpression`, e o tipo do campo é atribuível ao tipo atual.
- **Posição:** o parêntese inteiro `(x)`.
- **Mensagem:** `A record literal with exactly one positional field requires a trailing comma.`
- **Supressões:** substitui o `ARGUMENT_TYPE_NOT_ASSIGNABLE`/`INVALID_ASSIGNMENT`/`RETURN_OF_INVALID_TYPE` daquele ponto.
- **No DartForge:** existe em algum ponto (2 acertos), falta nas três emissões comuns. **Mudança:** acrescentar o passo em `verificar_atribuivel_expr` (com o tipo esperado record) — e nos retornos.

##### `must_return_void` (perda 1: FN 1)
- **Emissão:** `FfiVerifier._validateNativeCallable…` (`analyzer/lib/src/generated/ffi_verifier.dart:1830-1838`): `NativeCallable.listener(f)` com tipo nativo de retorno ≠ `Void`.
- **Posição:** o argumento `f`. **Mensagem:** `The return type of the function passed to 'NativeCallable.listener' must be 'void' rather than '{0}'.` (`{0}` o tipo nativo de retorno, ex. `Int32`).
- **No DartForge:** inexistente (FFI; `crates/analise/src/nativos.rs` se for implementado).

##### `await_in_late_local_variable_initializer` (perda 2: FN 2)
- **Emissão:** `ErrorVerifier._checkForAwaitInLateLocalVariableInitializer` (`analyzer/lib/src/generated/error_verifier.dart:2205-2212`), de `visitAwaitExpression`.
- **Condição exata:** `await` dentro do inicializador de variável **local** `late` (pilha `_isInLateLocalVariable`, empilhada em `visitVariableDeclarationList` para locais `late` e desempilhada nas expressões de função — closures dentro do inicializador não contam).
- **Posição:** o token `await`. **Mensagem:** `The 'await' expression can't be used in a 'late' local variable's initializer.`
- **No DartForge:** inexistente. **Mudança:** em `instrucoes.rs` (declaração local `late`) marcar o contexto e relatar em `expr.rs` no `await`.

#### Grupo 2 — membros indefinidos, invocações e `void`

Base comum: a resolução de membro passa por `TypePropertyResolver.resolve` (`analyzer/lib/src/dart/resolver/type_property_resolver.dart`), que devolve `getter`/`setter` e os sinalizadores `needsGetterError`/`needsSetterError` (falsos quando houve outro erro já relatado: receptor anulável → `UNCHECKED_…`, extensão ambígua → `AMBIGUOUS_EXTENSION_MEMBER_ACCESS`, `dynamic`/`Never`/`InvalidType`). Regra de ouro dos argumentos: quando o argumento é `String` (nome de classe) sai sem argumentos de tipo; quando é `DartType` sai com `getDisplayString()` (com argumentos de tipo, alias preferido, `?`).

##### `undefined_method` (perda 87: FN 56, FP 9, msg 22)
- **Emissão:** `MethodInvocationResolver` (`analyzer/lib/src/dart/resolver/method_invocation_resolver.dart`):
  - `_resolveReceiverType` (:795-885): instância (`x.m()`), implícito `m()` dentro de classe (via `_resolveReceiverNull` → `thisType` não nulo, :640-655), cascata;
  - `_resolveReceiverNull` com setter de topo/extensão/estático e sem getter (:610-647: `noGetterIsPossible`);
  - `_resolveReceiverTypeLiteral` (:890-939): `C.m()` estático (classe, ou alias cujo `aliasedType` é interface — `Object`, `FutureOr`? não: só `InterfaceType`; `FutureOr`/`Null` aliasados viram `InterfaceType` do `FutureOr`/`Null`);
  - `FunctionReferenceResolver` (`analyzer/lib/src/dart/resolver/function_reference_resolver.dart:715-722`): tear-off `C.m<int>` indefinido (argumento DartType).
- **Condição exata:** sem getter (método/getter) no tipo e sem extensão aplicável, `needsGetterError`, e o nome não sintético. O alvo é resolvido **antes**: receptor `void` → `USE_OF_VOID_RESULT` no receptor; `dynamic`/`Never` → nada; receptor anulável com o membro só no não nulo → `UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE`. Se o membro achado é getter/campo, vira `FunctionExpressionInvocation` (não é este código).
- **Posição:** o nome do método (`methodName`).
- **Mensagem:** `The method '{0}' isn't defined for the type '{1}'.` — `{1}` é **String**:
  - instância/implícito (`_resolveReceiverType` :867-873, `_resolveReceiverNull` :631-635): `InterfaceType → element.name` (**sem argumentos de tipo**: `'C'`, não `'C<String>'`; `'Map'`, `'Pointer'`), `FunctionType → 'Function'`, outros (record, parâmetro de tipo, …) → `'<unknown>'`;
  - estático (`_resolveReceiverTypeLiteral` :935): `receiver.displayName` = o nome da classe (o **elemento** aliasado: `typedef T = Object` → `'Object'`; `typedef N = Null` → `'Null'`; `FutureOr` → `'FutureOr'`).
  - variante `UNDEFINED_METHOD_ON_FUNCTION_TYPE` (`'{0}' isn't defined for the '{1}' function type`, :169-182): receptor é literal de tipo de alias de função com argumentos de tipo (`Fn<int>.m()`), `{1}` = `qualifiedName` do alias.
  - `C.new()` sem construtor sem nome: `NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT` em vez deste (:917-929).
- **Supressões e ordem:** `C.m()` com `m` de instância → `STATIC_ACCESS_TO_INSTANCE_MEMBER`; extensão estática (`E.m()`) → `UNDEFINED_EXTENSION_METHOD`; prefixo de import (`p.f()`) → `UNDEFINED_FUNCTION` (abaixo); `super.m()` → `UNDEFINED_SUPER_METHOD`.
- **No DartForge:** `chamadas.rs::chamada`, ramo `Busca::Ausente` (:718-745) — texto `"… para o tipo '{tipo formatado}'"` (formata o DartType inteiro); estático (`C.m()`, :557-584) passa por `inferir(target)` e sai como `UNDEFINED_GETTER` sem `{1}`; índice `[]=` indefinido sai como `UNDEFINED_METHOD` em `expr.rs::escrita_indice` (:3096-3100).
  Causas:
  1. **22 msg**: `{1}` deve ser o nome do elemento (`'C'`, `'Map'`, `'Pointer'`), `'Function'` para tipos de função (`Comparator<dynamic>` → `'Function'`, `void Function()` → `'Function'`), `'<unknown>'` para record; estático via alias → o elemento aliasado (`'FutureOr'`, `'Null'`).
  2. **~20 FN estáticos** (`mixin/illegal_static_access`, `mixin_declaration_constructor_error`, `nonfunction_type_aliases/*`, `UndefinedMethod__typeAlias_interfaceType`, `…static_mixinApplicatio…`, `…method_undefined_enum`, `null_aware/invocation_test` `C?.toString()`): saem como `undefined_getter` com `{1}` vazio (FP lá). **Mudança:** em `chamada`, ramo `referencia_a_tipo(recv)`, quando não há membro estático: se `name == new` → `NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT [nome da classe]` no `new`; senão `UNDEFINED_METHOD [m, nome do elemento]` no nome, sem inferir o alvo como propriedade.
  3. **9 FP `[]=`/`call`**: `a[i] = v` com `[]=` ausente é `UNDEFINED_OPERATOR ['[]=', tipo]` no `[...]` (ver `undefined_operator`); `f.call()`/`f()` com `f: Function` não relata (o tipo `Function` tem `call` dinâmico, `property_element_resolver.dart:440-446`).
  4. **FN restantes**: `constructor/reference_test` (5, `C.m` sobre construtor), `extension_methods/static_extension_internal_basename_shadowing` (6, nome de extensão sombreado por membro), `this_promotion/*` (5), `type_variable/conflict2`, `private/member3`, `UndefinedMethod__localSetterShadowingEx…` (`noGetterIsPossible`: setter de topo/estático/de extensão no escopo sem getter → `UNDEFINED_METHOD` com `thisType`), `…extensionMethodHidden/Shadowi…`, `…functionAlias_typeInstantiated` (variante `_ON_FUNCTION_TYPE`).
  **Mudança:** emitir com `aviso_com_codigo(UNDEFINED_METHOD, name.span, [m, nome_para_mensagem(r_ty)])` com a função de nome acima.
- **Estado em 2026-10-07 (placar 112/115):** duas regras novas em `chamadas.rs`.
  * O `_resolveElement` (`:340-355`) só aceita o membro acessível: `C.m()` com `m` privado de outra biblioteca
    relata `UNDEFINED_METHOD`, inclusive pelo alias não genérico (`PublicClass._privateStaticMethod()`).
  * O literal de tipo instanciado de um alias de função (`Fn<int>.foo()`) relata
    `UNDEFINED_METHOD_ON_FUNCTION_TYPE` no nome, com `{1}` o texto do tipo. Também vale com `?.`.
  * Restam os 2 do oráculo de `equality_extension_override_error_test`. O de
    `static_extension_internal_basename_shadowing_error_test:290:5` saiu com o `noGetterIsPossible` (ver
    `ambiguous_extension_member_access`).

##### `undefined_getter` (perda 79: FN 42, FP 31, msg 6)
- **Emissão:** `PropertyElementResolver._resolve` (`analyzer/lib/src/dart/resolver/property_element_resolver.dart:383-540`; relato :505-512) para `x.g`, `x?.g`, cascata, implícito `g` em classe (`resolver.dart:1580-1590` via `SimpleIdentifierResolver`); estático `_resolveTargetInterfaceElement` (:640-705, relato :697-704).
- **Condição exata:** sem getter/método, `needsGetterError`; `x.call` com `x` de tipo função ou `Function` **não** relata (:440-446); receptor `void` → `USE_OF_VOID_RESULT` no nome (:448-453); tipo de alias de função com argumentos de tipo → `UNDEFINED_GETTER_ON_FUNCTION_TYPE` (:459-476). Estático: sem getter estático e sem método estático acessível → `UNDEFINED_GETTER` (ou `UNDEFINED_ENUM_CONSTANT` se o tipo é enum).
- **Posição:** o nome da propriedade.
- **Mensagem:** `The getter '{0}' isn't defined for the type '{1}'.` — instância: `{1}` = **DartType** do alvo (promovido a não nulo em `?.`), com argumentos de tipo; estático: `{1}` = `typeReference.name` (String, nome da classe/mixin).
- **Supressões e ordem:** ver base; em atribuição composta lê o getter (relata getter e, se faltar o setter, `UNDEFINED_SETTER` também).
- **Campo de instância de extensão:** o `external` (o `IntAddress.address` do `dart:ffi` 3.6.2, `external Pointer<Never>
  address;` em `extension IntAddress on int`) tem getter e setter sintéticos, membros de instância da extensão:
  `arr[0].address` não relata. No DartForge, `elements::outline::extension_members` cria os acessores
  implícitos (o lookup estático de `membro_estatico_de_extensao` só vê os campos estáticos).
- **No DartForge:** `expr.rs::propriedade`/`membros.rs`. Causas:
  1. **~25 FP com `{1}` vazio**: chamadas estáticas indefinidas (`C.m()`) — mesma causa do item 2 de `undefined_method`; e `E.TWO` de enum (`UNDEFINED_ENUM_CONSTANT`).
  2. **6 msg** (`static_undefined`, `static_definedInSuperclass`, `typeLiteral_conditionalAccess`, `compoundAssignment…`, `null_aware/access_test`): estático sem `{1}` — falta o nome da classe.
  3. **5 FP `call`**: `f.call` com `f: Function`/tipo de função não relata.
  4. **4 FP `ffi_address_of_cast/*`**: `.address` de extensão FFI em elementos (`array[i].address`) — o membro existe por extensão do `dart:ffi` (`int`? receptor deveria ser o tipo nativo) — inferência de FFI.
  5. **26 FN `anonymous_methods/*`**: sintaxe nova (oráculo 3.13); **6 FN `static_extension_getter_setter_conflicts`**: getter ausente quando só há setter na extensão/classe; **2 FN `write_promoted_value_in_switch`**; FN `UndefinedGetter__new_*` (`C.new` como getter em instância/dynamic/cascata), `…functionAlias_typeInst…` (`_ON_FUNCTION_TYPE`), `…extension_this/instance…`.
  **Mudança:** emitir com código; `{1}` instância = tipo formatado; estático = nome da classe; `call` em função; `UNDEFINED_ENUM_CONSTANT` para enum.

##### `use_of_void_result` (perda 44: FN 18, FP 4, pos 22)
- **Sobreposição de extensão (2026-10-08):** `E(f()).g` com `f()` `void` relata no argumento inteiro (`atNode(receiverExpression)`,
  `extension_member_resolver.dart:227-232`; não no nome do método de `f()`), e o acesso `.g` não relata de novo:
  o membro vem da extensão (`chamadas.rs`, `expr.rs` na leitura de propriedade).
- **Emissão:** `checkForUseOfVoidResult` (`analyzer/lib/src/generated/error_detection_helpers.dart:235-253`), chamado de muitos lugares (argumentos, atribuições, condições `bool_expression_verifier.dart:95-106`, `await`/`throw`/`!`/spread/for-in, elementos de coleção, `yield_statement_resolver.dart:45-62`, `record_literal_resolver.dart:150-156`); receptores: `method_invocation_resolver.dart:319` (no **receptor**), `property_element_resolver.dart:87` (índice: no **alvo**? não — `atNode(node.target)`? ver abaixo) e :448-453 (no nome da propriedade), `function_expression_invocation_resolver.dart:115-135`, `assignment_expression_resolver.dart:161-222`, `extension_member_resolver.dart:227-231`.
- **Condição exata:** o tipo estático é **identicamente** `void` (não `FutureOr<void>`); `checkForUseOfVoidResult(e)`: `MethodInvocation` → relata no `methodName`; senão no nó inteiro.
- **Posição:** método → nome do método; `FunctionExpressionInvocation` (variável/getter de tipo função que retorna void) → **a invocação inteira**; acesso `x.foo` com `x` void → o nome `foo`; índice `x[0]` com `x` void (depois de `resolveToBound`) → do `[` ao `]` (`property_element_resolver.dart:80-88` via `_reportUnresolvedIndex`); `x!` → o nó `x!`; record `(one: x,)` → o campo nomeado inteiro `one: x`; `x.foo = null` → `foo`.
- **Mensagem:** `This expression has a type of 'void' so its value can't be used.`
- **Supressões e ordem:** sai **no lugar** de `ARGUMENT_TYPE_NOT_ASSIGNABLE`/`INVALID_ASSIGNMENT` (destino não void); em mapa `{v: v}` a chave void relata e **para** (o valor não é checado — `literal_element_verifier.dart:121-133`); no `throw` sai junto com `THROW_OF_INVALID_TYPE`.
- **No DartForge:** `expr.rs::uso_de_void` (:1142-1169) e vários chamadores. Causas:
  1. **~20 pos + 9 FN `nnbd/top_merge/*`**: top-merge (`void` vs `Object?` combinados) — fora das chamadas.
  2. **4 FN `least_upper_bound_futureor_test`**: `if (x is Future) throw 0;` sobre `FutureOr<void>` deve rebaixar `x` a `void` (fatoração de tipo no fluxo: `factor(FutureOr<void>, Future) = void`) — `fluxo.rs`.
  3. **5 FN** `x.foo = null` (setter em void: no nome, sem `UNDEFINED_SETTER` — que é nosso FP), `x[0]`, `x!`, campos de record.
  4. **FP/pos `type_promotion/functions_test`**: chamada de **variável local** de tipo função (`funcDynToVoid(…)`) é `FunctionExpressionInvocation` → span da invocação inteira; nós usamos o nome (`cx.funcoes_locais` mal classificado).
  5. **FP `mapLiteralEntry_keyAnd…`**: parar depois da chave; **FP `control_flow_collections/void_error_test:92`**, **pos `extensionOverride_argu…`** (`E(v).g` com `v` void: no argumento `v`, `extension_member_resolver.dart:227-231`).
  **Mudança:** os pontos acima; manter `aviso_com_codigo`.
- **Feito em 2026-10-07 (473/479):** `e!` com `e` `void` (`visitPostfixExpression`, no nó inteiro); campo `void`
  de record (`record_literal_resolver.dart:134-158`, no campo; o nomeado do nome ao valor); escrita de
  propriedade com receptor `void` (`property_element_resolver.dart:449-455`, no nome, no lugar do
  `undefined_setter`); índice com alvo `void` pelo limite (`:81-90`, no `[…]`).
- **Estado em 2026-10-07 (placar 477/479):** o ramo falso de `x is T` promove direto ao fator, como o
  `_finishTypeTest` (INFERENCIA R-FLU-P1). Saíram os 4 FN de `least_upper_bound_futureor_test`, onde
  `FutureOr<void>` menos `Future` é `void`.
- **Depois (478/479):** a promoção do ramo verdadeiro só falha quando o tipo novo é igual ao anterior
  (INFERENCIA R-FLU-P1). Com isso o `if (f is FuncDynToVoid)` sobre `FuncDynToDyn` promove no 3.6.2, e os FP de
  `type_promotion/functions_test` saíram.
  * O padrão de record passou a tipar os campos pelo tipo casado na entrada (`matchedValueType`,
    `type_analyzer.dart:1608-1625`), e não pelo promovido para `(Object?, …)`.

##### `undefined_operator` (perda 40: FN 30, FP 10)
- **Emissão:** binário `BinaryExpressionResolver._resolveUserDefinableElement` (`analyzer/lib/src/dart/resolver/binary_expression_resolver.dart:440-460`, no **operador**); prefixo (`prefix_expression_resolver.dart:180-206`, no token do operador, nome `unary-`/`~`/`+`/`-` para `++`/`--`); pósfixo (`postfix_expression_resolver.dart:130-158`, no operador, `+`/`-`); índice (`property_element_resolver.dart:109-133` + `_reportUnresolvedIndex` :365-381: **do `[` até o `]` inclusive**, `'[]'` para leitura e `'[]='` para escrita — ambos podem sair na composta); composta (`assignment_expression_resolver.dart:240-256`, no operador `+=`); `resolver.dart:1635-1642` (`==` em extensão?).
- **Condição exata:** `needsGetterError` do operador no tipo do operando (lido; em `?.`/`?[` promovido).
- **Posição:** acima.
- **Mensagem:** `The operator '{0}' isn't defined for the type '{1}'.` — `{1}` DartType (com argumentos: `'C1<int>'`).
- **Supressões e ordem:** `super` → `UNDEFINED_SUPER_OPERATOR`; override de extensão → `UNDEFINED_EXTENSION_OPERATOR`; `dynamic`/`Never` nada.
- **No DartForge:** binário em `expr.rs` (:2185-2215, já com código); falta índice (`[]=` sai como `UNDEFINED_METHOD`, `[]` sem relato), prefixo/pósfixo (`++a`, `-a`, `~s`). Causas: **11 FN índice** (`UndefinedOperator__index*`, `static_extension_getter_setter_conflicts` 9, `string_test`, `first_class_types_literals`), **8 FN `++`/`--`/`-`** (`prefix/postfixExpression*`, `string/no_operator_test`), **2 FN `variance/syntax`** (experimento); **10 FP `number_operator_error_test`** (`O extends num?` promovido a `O & int` — o operador deve ser procurado no limite **promovido** `int`, não em `O`).
  **Mudança:** índice com o span `[..]` e os dois nomes; pré/pós-fixos no token; buscar operadores em tipos interseção pelo limite promovido.
- **FP de `number_operator_error_test` resolvidos em 2026-10-07:** a causa era outra: o código depois de `x += never` é inalcançável e a promoção por `is` era descartada ali; agora promove (ver INFERENCIA R-FLU-P1).
- **Correções e estado (2026-10-06):** (1) prefixo/sufixo (`-x`, `~x`, `++x`, `x--`) sem o operador no tipo:
  `UNDEFINED_OPERATOR` no token do operador (`'unary-'`, `'~'`, `'+'`, `'-'`), salvo receptor anulável (é o
  `unchecked_*`), `void`, tipo de função, inválido, e o operando que é tipo (`C++`, `dynamic++`: só
  `assignment_to_type`); (2) `x[i]++` procura também o `[]=` e relata os dois quando faltam; (3) a regra do par do
  `TypePropertyResolver` (INFERENCIA §8.1, passo 5a): `[]` de extensão não vale se a interface tem `[]=`, e
  vice-versa. 59 de 62.

##### `undefined_function` (perda 25: FN 25)
- **Emissão:** `MethodInvocationResolver._reportUndefinedFunction` (`analyzer/lib/src/dart/resolver/method_invocation_resolver.dart:296-310`), de `_resolveReceiverNull` (sem elemento no escopo **e** `thisType == null`, :620-629) e `_resolveReceiverPrefix` (`p.f()` sem `f` no prefixo, :718-724); também `library_analyzer.dart:600-608` (`loadLibrary`?).
- **Condição exata:** `f(...)` com `f` não encontrado no escopo léxico e fora de classe/extensão (sem `this`); ou `p.f(...)` com `p` prefixo. Ignorado se `shouldIgnoreUndefined(prefix, name)` (import de URI inexistente com o prefixo, ou nome em `show`… — `analyzer/lib/src/dart/element/scope.dart`/`library_fragment`: import não resolvido torna os nomes desconhecidos silenciosos).
- **Posição:** o nome do método.
- **Mensagem:** `The function '{0}' isn't defined.`
- **Supressões e ordem:** dentro de classe com `this` → `UNDEFINED_METHOD [f, nome da classe]`; nome de classe privada de outra biblioteca (`_Class()`) também cai aqui.
- **No DartForge:** hoje `f()` indefinido sai como `UNDEFINED_IDENTIFIER` (`expr.rs:2737`/`:423`) — é **FP** em `undefined_identifier` (vários dos 95 FP). **Mudança:** em `chamadas.rs::chamada`, ramo `_` com alvo `Identifier` que não resolve: se há `this` (`cx.tipo_this`) → `UNDEFINED_METHOD [nome, nome da classe]`; senão `UNDEFINED_FUNCTION [nome]`; em `p.f()` com prefixo → `UNDEFINED_FUNCTION`; sem relatar `UNDEFINED_IDENTIFIER` no alvo; respeitar `shouldIgnoreUndefined`.

##### `undefined_super_member` (perda 13: FN 13)
- **Emissão:** variantes: `UNDEFINED_SUPER_GETTER` (`property_element_resolver.dart:820-830`), `…_METHOD` (`method_invocation_resolver.dart:780-790`), `…_OPERATOR` (binário :448, pré :197, pós :148, índice :121/:131), `…_SETTER` (`property_element_resolver.dart:866-876`).
- **Condição exata:** membro não achado na superclasse (incluindo mixins aplicados) de `this`; para setter, `super.x = v` sem setter.
- **Posição:** o nome (getter/método/setter); o operador; para índice, `[`…`]`.
- **Mensagem:** `The {getter|method|operator|setter} '{0}' isn't defined in a superclass of '{1}'.` — `{1}` DartType de `this` da classe que contém (`'B'`, `'E'`).
- **No DartForge:** `expr.rs::membro_super` cobre getter/método (15 acertos). Faltam **4 índice `[]=`** e **9 setter** (`super.x = v`). **Mudança:** em `escrita_propriedade`/`escrita_indice` com alvo `super`.
- **Feito em 2026-10-06 (28/28):** `super.m = v` saía sem aviso quando a chave `m=` nunca tinha sido internada (`chave_setter` só faz `lookup`): sem a chave, nenhuma declaração tem o setter, e `membro_super` segue para o relato. `super[i] = v` e `super[i]++` procuram o `[]=` pela cadeia de `super` (`buscar_operador_super`), não no tipo de `this` (que achava o `[]=` da própria classe), e relatam `UNDEFINED_SUPER_OPERATOR` com o tipo de `this`.

##### `undefined_setter` (perda 11: FN 7, FP 4)
- **Emissão:** `AssignmentVerifier.verify` (`analyzer/lib/src/error/assignment_verifier.dart:28-110`, relato :100-108), chamado de `PropertyElementResolver` quando `needsSetterError`; estático em `_resolveTargetInterfaceElement` (setter estático ausente → `UNDEFINED_SETTER [nome, typeReference.name]`); `UNDEFINED_SETTER_ON_FUNCTION_TYPE` (:470-476).
- **Condição exata:** sem setter; se houver getter final/método no lugar, `AssignmentVerifier` relata outro código (`ASSIGNMENT_TO_FINAL`, `ASSIGNMENT_TO_FINAL_NO_SETTER`, `ASSIGNMENT_TO_METHOD`…).
- **Posição:** o nome. **Mensagem:** `The setter '{0}' isn't defined for the type '{1}'.` (instância DartType; estático nome da classe).
- **No DartForge:** `expr.rs::escrita_propriedade`. **FN**: `C.s = v` estático (2), `x.new = …` (2), `…functionAlias…`, `if_null/assignment_behavior_test`, `regress13494`. **FP**: receptor `void` (deve ser só `USE_OF_VOID_RESULT`), `instance_access_to_static_member` (estático acessado por instância: outro código), `getter/no_setter*_test` (getter sem setter de `this` implícito → `ASSIGNMENT_TO_FINAL_NO_SETTER`).
- **Feito em 2026-10-07 (25/29):** escrita estática pela classe (`A.B = 0`, `C.s = 1` com `s` estático só na
  superclasse, `C.new = 1`) sem setter nem getter (estático ou de instância, o `augmented.getGetter`): o
  `AssignmentVerifier` relata `UNDEFINED_SETTER` com o `thisType` da classe (`property_element_resolver.dart:708-731`,
  `assignment_verifier.dart:96-106`), em `expr.rs::escrita_propriedade`.
- **`x.new` (2026-10-07):** o `TypePropertyResolver` nunca acha `new` (`type_property_resolver.dart:75-79`,
  `_needsGetterError`/`_needsSetterError` sempre): `x.new` e `x.new = v` com `x` valor relatam
  `UNDEFINED_GETTER`/`UNDEFINED_SETTER` com o tipo do receptor, também em `dynamic`, parâmetro de tipo e
  anulável (`c..new` com `C?`), sem o relato de nulo (`expr.rs::propriedade` e `escrita_propriedade`).

##### `undefined_extension_operator` (perda 8: FN 8)
- **Emissão:** `binary_expression_resolver.dart:405-414` (e prefixo :160-168, índice `property_element_resolver.dart:52-70`).
- **Condição exata:** `E(x) op y` com o override de extensão sem o operador; para `==` também (`E(x) == y` sem `==` declarado na extensão).
- **Posição:** o operador. **Mensagem:** `The operator '{0}' isn't defined for the extension '{1}'.` (`{1}` nome da extensão).
- **No DartForge:** existe para operadores comuns (13); falta `==` (8 FN em `equality_extension_override_error_test`). **Mudança:** no `==`, quando o operando esquerdo é override de extensão.
- **Feito em 2026-10-07 (21/21):** `expr.rs::condicao_binaria` procura o `==` só na extensão do override esquerdo; ausente, relata no token do operador com o nome `==` (também para `!=`).

##### `ambiguous_extension_member_access` (perda 20: FN 19, msg 1)
- **Emissão:** `ExtensionMemberResolver.findExtension` (`analyzer/lib/src/dart/resolver/extension_member_resolver.dart:87-140`).
- **Condição exata:** mais de uma extensão aplicável com o membro e nenhuma mais específica (`_chooseMostSpecific`).
- **Posição:** a entidade do nome (nome do membro; para operador binário/unário o token do operador; para índice o `[`…`]`? — oráculo `10:3`/`11:15`: o nó do alvo/operador conforme o chamador passa `nameEntity`).
- **Mensagem:** `A member named '{0}' is defined in {1}, and none are more specific.` — `{1}`: lista de `"extension 'E'"` ou `"unnamed extension on 'T'"` (`extendedType.getDisplayString()`), juntadas com `, ` e ` and ` no último (`analyzer/lib/src/dart/resolver/extension_member_resolver.dart:122-135`, `StringUtilities.commaSeparatedWithAnd`?).
- **No DartForge:** `inf.relatar_ambiguidade_de_extensao` (54 acertos). FN: operadores (`+`, `unary-`, `[]`), getter+setter (`getter_setter`), membros em `static_extension_getter_setter_conflicts` e `internal_resolution_4`. **msg**: `'Iterable<InvalidType>'` — tipo inválido exibido como `InvalidType`.
  **Mudança:** consultar a ambiguidade também nos caminhos de operador/índice/atribuição.
- **Correções e estado (2026-10-06):** (1) `findExtension` torna candidata a extensão que declara o membro **ou o
  par** dele — getter/setter do nome base, `[]`/`[]=` no índice —; a mais específica sem o membro pedido não resolve
  (`E { get a }` e `E2 { set a }` → `0.a` ambíguo); (2) operadores relatam a ambiguidade: binário no nó inteiro
  (`a + a`), prefixo no operando (`-a`), índice no alvo (`a[0]`), composta no token (`a += 0`). 68 de 73; restam os
  membros de extensão alcançados pelo `this` implícito dentro de outra extensão.
- **Estado em 2026-10-07 (placar 72/73):** o nome sozinho num membro de classe, sem nada no escopo léxico e com
  duas extensões aplicáveis ao `this`, agora relata. Antes, `buscar_membro` devolvia `Dinamico` em silêncio.
  * Na leitura (`ThisLookup.lookupGetter`): a ambiguidade e, sem getter, `UNDEFINED_IDENTIFIER` no mesmo nome
    (`simple_identifier_resolver.dart:208-221`).
  * Na escrita (`lookupSetter`): a ambiguidade e o nome indefinido.
  * Na invocação (`_resolveReceiverNull`): só a ambiguidade.
  * O setter achado no escopo léxico, quando é estático ou declarado numa extensão (também o de instância),
    é o `noGetterIsPossible` (`method_invocation_resolver.dart:616-628`): `m()` relata `UNDEFINED_METHOD`.
    Antes saía a ambiguidade, em `static_extension_internal_basename_shadowing_error_test:290:5`.
  * Resta o índice `[]` de `static_extension_getter_setter_conflicts_test:172:3`.

##### `invocation_of_non_function` (perda 28: FN 28)
- **Emissão:** `_reportInvocationOfNonFunction` (`method_invocation_resolver.dart:257-270`), de `_resolveReceiverNull` (:575-600) quando o elemento achado no escopo não é executável, variável nem prefixo: **parâmetro de tipo** (`T()`), alias de tipo (`typedef T = dynamic; T()`), `dynamic`; e do tipo literal com elemento não executável (:902-915).
- **Posição:** o nome (`methodName`). **Mensagem:** `'{0}' isn't a function.` (`{0}` o nome).
- **No DartForge:** `chamadas.rs::invocar_valor` deixa de fora (comentário :389-396). **Mudança:** em `chamada`, ramo `Identifier` que resolve a parâmetro de tipo/alias não-classe/`dynamic` → `INVOCATION_OF_NON_FUNCTION [nome]` no nome, inferindo os argumentos livres.
- **Estado (2026-10-06):** 28 de 28. A regra que decide (`_resolveReceiverNull`,
  `method_invocation_resolver.dart:563-600`): o elemento achado para `nome(…)` que não é acessor, executável, variável
  nem prefixo relata no nome. Chegam aqui os aliases cujo alvo **não** é `InterfaceType` (o `AstRewriter` só reescreve
  como criação o alias de interface, `ast_rewrite.dart:154-160`): `typedef T<X> = dynamic`, `= void`, `= X`. Para o
  analyzer, `Null` e `FutureOr<…>` **são** `InterfaceType`: `typedef T<X> = Null` vira criação (`T()` →
  `new_with_undefined_constructor_default`, `T<Null>.named()` → `new_with_undefined_constructor`).

##### `invocation_of_non_function_expression` (perda 12: FN 12)

**Feito em 2026-10-08:** sem `call` nenhum o tipo da invocação é `InvalidType` (`result.isGetterInvalid`,
`function_expression_invocation_resolver.dart:89-92`). O `super(…)` procura o `call` pela cadeia de `super`
(o `TypePropertyResolver` com receptor `SuperExpression`): numa classe com `call()` próprio, `super()` relata.
O `assert(…)` como expressão é `FunctionExpressionInvocation` e o avaliador de constantes dá o erro genérico
(`visitNode`); o padrão constante de tipo inválido não é avaliado (`constant_verifier.dart:133-135`).
- **Emissão:** `FunctionExpressionInvocationResolver.resolve` (`analyzer/lib/src/dart/resolver/function_expression_invocation_resolver.dart:72-104`) e `resolver.dart:2015-2025`.
- **Condição exata:** o tipo do alvo não é função/`dynamic`/`Never`/`Function`, e `call` não é achado (`needsGetterError`), **ou** `call` existe mas não é método (getter/campo `call`, :92-101).
- **Posição:** a expressão alvo (`function`). **Mensagem:** `The expression doesn't evaluate to a function, so it can't be invoked.`
- **No DartForge:** `chamadas.rs::invocar_valor` (:386-414) — 9 acertos. FN: `call` como getter (`call/through_getter_test`), extensões sombreadas (`static_extension_internal_basename_shadowing` 6, `issue_45551`), `type_variable/conflict2`, `why_not_promoted/nullable_expression_call` (anulável?), `patterns/invalid_const_pattern`. Hoje excluímos "nome solto que não é local" e "receptor anulável"; rever cada exclusão contra o analyzer (o anulável é `UNCHECKED_INVOCATION_OF_NULLABLE_VALUE`, mas `null` literal/`Null` não).
- **Feito em 2026-10-07 (20/21; resta `case super():` em padrão):** em `chamadas.rs::invocar_valor`
  - o `call` achado que não é método (getter ou campo `call`, da classe ou de extensão) relata na função e
    dá o resultado inválido, também com receptor anulável (junto do relato do nulo);
  - com receptor anulável, a extensão só fornece o `call` se se aplica ao tipo anulável; senão não há
    `call` nem este relato (`needsGetterError` falso);
  - o nome solto que resolve para getter ou variável (de topo, estático, membro pelo `this` implícito, de
    extensão) é reescrito em `FunctionExpressionInvocation` (`method_invocation_resolver.dart:585-592`,
    `:900-905`) e relata; só o nome sem getter fica de fora;
  - `Type` entre parênteses (`(T)()`) relata; só o literal de tipo direto (`T<Null>()`) é outro código.

##### `class_instantiation_access_to_member` (perda 2: FN 1, FP 1)
- **Emissão:** `ConstructorReferenceResolver` (`analyzer/lib/src/dart/resolver/constructor_reference_resolver.dart:55-85`): `C<int>.x` onde `x` não é construtor: estático → `…_STATIC_MEMBER [x]`, instância → `…_INSTANCE_MEMBER [x]`, nada → `…_UNKNOWN_MEMBER [C, x]`.
- **Posição:** o nó `ConstructorReference` inteiro (`C<int>.x`, do início do tipo ao fim do nome; oráculo `265:3` comprimento 8 em `X<X>.any`). Só quando o tipo nomeado é classe (ou alias de interface); alias de função não relata. **No DartForge:** FN `explicit_type_instantiation_parsing_test:265` (`UNKNOWN_MEMBER`); FP `generic_usage_class_error_test:36` (alias genérico para classe com `staticMethod` — o analyzer não relata ali porque o alias instanciado… outro código).

##### `extension_as_expression` (perda 1: FN 1)
- **Emissão:** `SimpleIdentifierResolver` (`analyzer/lib/src/dart/resolver/simple_identifier_resolver.dart:315-323`) e `prefixed_identifier_resolver.dart:172-180`: identificador que resolve a uma extensão usado como expressão (não alvo de acesso). **Posição:** o identificador. **Mensagem:** `Extension '{0}' can't be used as an expression.` **No DartForge:** inexistente; em `expr.rs` (identificador que é extensão).

##### `extension_override_with_cascade` (perda 3: FN 3)
- **Emissão:** `method_invocation_resolver.dart:430-438`, `property_element_resolver.dart:605-613`, `function_reference_resolver.dart:370-376`: `E(x)..m()`/`..g`/`..s = v`. **Posição:** o override `E(x)` (o alvo da cascata). **Mensagem:** `Extension overrides have no value so they can't be used as the receiver of a cascade expression.` **No DartForge:** inexistente.

##### `extension_override_without_access` (perda 1: FN 1)
- **Emissão:** `ExtensionMemberResolver.resolveOverride` (`extension_member_resolver.dart:175-187`): `E(x)` fora de um acesso a membro (`_isValidContext`: alvo de propriedade/método/índice/operador/atribuição), exceto alvo de cascata. **Posição:** o override inteiro. **Mensagem:** `An extension override can only be used to access instance members.` **No DartForge:** `chamadas.rs` (`E(x)` isolado) não relata.

##### `deferred_import_of_extension` (perda 2: FN 2)
- **Emissão:** `ErrorVerifier._checkForDeferredImportOfExtensions` (`analyzer/lib/src/generated/error_verifier.dart:3053-3066`), de `visitImportDirective` para imports `deferred`.
- **Condição exata:** o namespace importado (depois de `show`/`hide`) contém alguma extensão. **Posição:** a URI do import. **Mensagem:** `Imports of deferred libraries must hide all extensions.` **No DartForge:** inexistente; lugar natural `crates/analise/src/importacoes.rs`.

#### Grupo 3 — `this`/`super` em contexto inválido e atribuição a final/const

##### `instance_member_access_from_static` (perda 18: FN 18)
- **Corrigido em 2026-10-08** (`InstanceMemberAccessFromStatic__class_t_*` e `__extensi_*`): a escrita sem
  qualificador (`foo = 0`) que acha um setter (ou campo não final) de instância onde `this` não vale relata o acesso
  sem `this` (`_checkForInvalidInstanceMemberAccess`), com o setter como elemento de escrita; antes só a leitura
  relatava (`types::inferencia::expr`, `tipo_de_escrita_nome`).
- **Emissão:** `ErrorVerifier._checkForInvalidInstanceMemberAccess` (`analyzer/lib/src/generated/error_verifier.dart:3975-4040`), chamado de `visitSimpleIdentifier`.
- **Condição exata:** (fora de comentário) estamos em inicializador de construtor, ou em método estático (`_enclosingExecutable.inStaticMethod`: método/getter/setter `static` de classe, mixin, enum **ou extensão**), ou em factory, ou em declaração de campo de instância não `late`, ou em declaração de variável estática; o identificador (lido ou escrito: `writeOrReadElement`) resolve a `MethodElement`/`PropertyAccessorElement` **de instância** cujo dono é `InterfaceElement` ou `ExtensionElement`; e não é o nome qualificado de `x.m()`, `x.p` ou `p.x` (só acesso **implícito**). Em método estático → este código.
- **Posição:** o identificador.
- **Mensagem:** `Instance members can't be accessed from a static method.` (sem argumentos).
- **Supressões e ordem:** a precedência é `inStaticMethod` → `INSTANCE_MEMBER_ACCESS_FROM_STATIC`; `inFactoryConstructor` → `…_FROM_FACTORY`; senão `IMPLICIT_THIS_REFERENCE_IN_INITIALIZER`. A resolução de nomes do analyzer **encontra** o membro de instância pelo escopo da classe mesmo em contexto estático (não é `UNDEFINED_IDENTIFIER`).
- **No DartForge:** inexistente. Hoje, num método estático, `expr.rs` resolve o nome implícito… (sem `this`, provavelmente como indefinido ou como membro sem relato). **Mudança:** em `expr.rs` (identificador e alvo implícito de chamada/atribuição), quando o nome resolve a membro de instância da classe/extensão envolvente e o contexto (`CtxFuncao`) é método estático → relatar no identificador; também em `chamadas.rs` para `m()` implícito (o identificador é o `methodName`).

##### `instance_member_access_from_factory` (perda 6: FN 6)
- **Emissão/condição/posição:** a mesma função (`error_verifier.dart:4028-4032`), com `_enclosingExecutable.inFactoryConstructor` (corpo de construtor `factory`, inclusive o redirecionamento? não — só o corpo).
- **Mensagem:** `Instance members can't be accessed from a factory constructor.`
- **No DartForge:** inexistente. **Mudança:** idem, com o contexto de factory.

##### `implicit_this_reference_in_initializer` (perda 17: FN 17)
- **Emissão:** a mesma função (`error_verifier.dart:4033-4038`).
- **Condição exata:** `_isInConstructorInitializer` (lista de inicializadores — campo, `super(...)`, `this(...)`, `assert` — marcado em `visitConstructorFieldInitializer` :619-630, `visitSuperConstructorInvocation` :1447-1452, `visitRedirectingConstructorInvocation` :1365-1369, `visitAssertInitializer` :348-352), ou inicializador de campo de instância **não `late`** (`visitFieldDeclaration` :845-889: `_isInInstanceNotLateVariableDeclaration`), ou inicializador de variável **estática** (`_isInStaticVariableDeclaration`) — e não é método estático nem factory. Inclui membros de instância de **extensão** e chamadas implícitas `m()`/tear-off `m`.
- **Posição:** o identificador.
- **Mensagem:** `The instance member '{0}' can't be accessed in an initializer.` — `{0}` o nome do identificador.
- **Supressões e ordem:** `this.x` explícito é outro código (`INVALID_REFERENCE_TO_THIS`); parâmetros de inicialização (`this.f`) e parâmetros de construtor com o mesmo nome sombreiam o membro (não relata). Campo `late` não relata.
- **No DartForge:** inexistente. **Mudança:** marcar no `Corpo` o contexto "inicializador" (em `funcoes.rs::inicializador` e no inicializador de campo não `late`/estático em `mod.rs`) e relatar em `expr.rs` quando um identificador implícito resolve a membro de instância.
- **Feito em 2026-10-07 (17/17; restam 3 FP de construtor primário):** `_checkForInvalidInstanceMemberAccess`
  vale também em `_isInInstanceNotLateVariableDeclaration` e `_isInStaticVariableDeclaration` (3.6.2
  `error_verifier.dart:3976-3986`): o inicializador de campo de instância não `late` e o de variável estática de
  classe ou extensão (inclusive dentro de closures nele) relatam o membro de instância sem `this`
  (`Corpo::inicializador_de_variavel`); os argumentos de constante de enum e as anotações continuam de fora.

##### `super_in_invalid_context` (perda 20: FN 20)
- **Emissão:** `ElementResolver.visitSuperExpression` (`analyzer/lib/src/generated/element_resolver.dart:404-424`) com `SuperContext.of` (`analyzer/lib/src/generated/super_context.dart`); os resolvedores (`method_invocation_resolver.dart:738`, `property_element_resolver.dart:795`, `binary_expression_resolver.dart:329`) apenas param de resolver o membro quando o contexto não é válido (não relatam `UNDEFINED_SUPER_*`).
- **Condição exata:** subindo a árvore a partir do `super`: `Annotation` → `annotation`; `ClassDeclaration`/`EnumDeclaration`/`MixinDeclaration` → válido; `CompilationUnit` → estático; construtor `factory` → estático; **qualquer `ConstructorInitializer`** → estático; `ExtensionDeclaration` → extensão; `ExtensionTypeDeclaration` → extension type; `FieldDeclaration` `static` → estático, ou **não `late`** → estático; `MethodDeclaration` `static` → estático. `annotation` e `static` relatam este código.
- **Posição:** o token `super` (a `SuperExpression`).
- **Mensagem:** `Invalid context for 'super' invocation.`
- **Supressões e ordem:** nenhum `UNDEFINED_SUPER_*` no mesmo `super`.
- **No DartForge:** inexistente (o `super` em contexto estático hoje cai em `membro_super` com `this` ausente). **Mudança:** em `expr.rs` (`ExprKind::Super` como alvo de acesso, invocação, índice, operador binário/unário), calcular o contexto pelo `Corpo` (função/membro/inicializador corrente) e relatar no `super`, sem resolver o membro.

##### `super_in_extension` (perda 8: FN 8)
- **Emissão/condição:** a mesma (`element_resolver.dart:414-418`), contexto `ExtensionDeclaration` (antes de achar classe).
- **Posição:** `super`. **Mensagem:** `The 'super' keyword can't be used in an extension because an extension doesn't have a superclass.`
- **No DartForge:** inexistente. **Mudança:** idem.

##### `super_in_extension_type` (perda 3: FN 3)
- **Emissão/condição:** `element_resolver.dart:419-423`, contexto `ExtensionTypeDeclaration`.
- **Posição:** `super`. **Mensagem:** `The 'super' keyword can't be used in an extension type because an extension type doesn't have a superclass.`
- **No DartForge:** inexistente. **Mudança:** idem.

##### `assignment_to_final_no_setter` (perda 16: FN 16)
- **Emissão:** `AssignmentVerifier.verify` (`analyzer/lib/src/error/assignment_verifier.dart:30-110`, relato :82-87) para atribuições resolvidas sem setter; `ErrorVerifier._checkForAssignmentToFinal` (`analyzer/lib/src/generated/error_verifier.dart:2138-2190`, relato :2172-2177) para `++`/`--` e `for (x in …)`.
- **Condição exata:** não há setter (`requested == null`) e o elemento de recuperação é um **getter** cuja variável (`variable2`) é um `FieldElement` **sintético** — i.e., getter explícito sem campo nem setter (inclusive getter **estático** acessado de construtor/`this.x`, getter de extensão: o "campo" sintético da extensão). Getter de campo `final` real → `ASSIGNMENT_TO_FINAL`; `const` → `ASSIGNMENT_TO_CONST`.
- **Posição:** o nome atribuído (identificador simples ou o nome em `x.nome`/`p.nome`).
- **Mensagem:** `There isn't a setter named '{0}' in class '{1}'.` — `{0}` nome da variável; `{1}` `displayName` do dono (classe, mixin, **extensão** — ex. `'E1A'`, `'E6'`).
- **No DartForge:** existe (44 acertos) em `expr.rs::escrita_propriedade`/identificador. Causas: **6 FN `static_extension_getter_setter_conflicts`** e **6 `static_extension_internal_basename_shadowing`** (getter de **extensão** sem setter: dono é a extensão, ou acesso implícito dentro da extensão), **4 FN `getter/no_setter*_test`** (getter **estático** atribuído de dentro de construtor — implícito `nextVar = 1` e `this.nextVar = 1`; hoje sai `UNDEFINED_SETTER`, FP lá).
  **Mudança:** ao não achar setter, procurar o getter também entre os **estáticos** (para `this.x`/implícito) e nas **extensões** aplicáveis, e relatar este código com o nome do dono.

##### `assignment_to_const` (perda 3: FN 3)
- **Emissão:** `assignment_verifier.dart:37-44` (variável `const` como alvo resolvido) e :76-81 (getter de variável const); `error_verifier.dart:2155-2171` (`++`/`--`, `for (x in …)`).
- **Condição exata:** alvo é variável (local, topo, campo estático) `const`.
- **Posição:** o identificador (ou a expressão).
- **Mensagem:** `Constant variables can't be assigned a value.` (no oráculo de 3.13 do grupo analyzer a mensagem é `…after initialization.`; o placar compara com a do 3.6.2).
- **No DartForge:** existe (5). FN: `for (x in …)` com `x` const local (`visitForEachPartsWithIdentifier`, `error_verifier.dart:926-932`, só se `_checkForEachParts` passou) e `case ERROR_B = 1:` (atribuição dentro de expressão de `case` legada). **Mudança:** no cabeçalho de `for-in` com identificador, aplicar a checagem de final/const.

##### `assignment_to_final` (perda 3: FN 3)
- **Emissão:** `assignment_verifier.dart:88-93`, `error_verifier.dart:2178-2183`.
- **Condição exata:** getter cuja variável é final e **não** sintética (campo/variável `final` real), sem setter.
- **Posição:** o nome. **Mensagem:** `'{0}' can't be used as a setter because it's final.`
- **No DartForge:** existe (66). FN `static_extension_internal_basename_shadowing:402-410`: campo `final` estático de **extensão** atribuído por nome implícito dentro de membro da extensão. **Mudança:** procurar os campos estáticos da extensão envolvente no lookup de escrita.

##### `assignment_to_primary_constructor_parameter` (perda 40: FN 40; feito, 40/40)
- **Não existe no analyzer 3.6.2** (ausente de `analyzer/messages.yaml` e de `codes.g.dart` da 3.6.2). Todos os casos vêm de arquivos com **construtores primários** (sintaxe nova, `primary_constructors/*`, `AssignmentToPrimaryConstructorParameter_*`), classificados em `corpus/diagnosticos/sintaxe-nova.json` e julgados pelo oráculo 3.13.4.
- **Mensagem (3.13):** `A primary constructor parameter can't be assigned to in an initializer.` — o parâmetro de construtor primário (declarado no cabeçalho `class C(int x)`) atribuído (`x = …`, `x++`, `x += …`) dentro de inicializador de campo/inicializador do construtor.
- **Posição (pelo oráculo):** o identificador atribuído (ou a expressão de atribuição; `2:27`, `65:6`/`65:11` = duas atribuições na mesma linha).
- **Emissor (checkout main, `pkg/analyzer/lib/src/generated/error_verifier.dart:3302-3327`):** `_checkForAssignmentToPrimaryConstructorParameter(node, element)`: o elemento escrito (o `writeElement` da atribuição, do `++`/`--` e do `??=`, ou o do `AssignedVariablePattern`) é um `FormalParameterElement` cujo dono é o construtor primário, e `_thisContext` é `constructorInitializers` ou `instanceFieldDeclaration`. O contexto não muda ao entrar numa função dentro do inicializador (`var f = () { x = 0; };` e `this : f = (() { x = 0; })` relatam). O corpo `this { x = 0; }` não relata; o campo `late` não vê o parâmetro (`undefined_identifier`). Chamadas: `visitAssignedVariablePattern` (`:443-446`), `visitAssignmentExpression` (`:456-457`), atribuição composta e `??=` (`:725-735`, `:948-956`, `:1598-1608`) e `++`/`--` (`:1712-1719`).
- **Posição:** o nome escrito (`x` em `x = 0`, `x++`, `++x`, `x += 1`, `x ??= 0`, `(x) = 0`, `(x!) = 2`, `[x] = [2]`; `(x && z) = 2` dá dois).
- **No DartForge (feito, 40/40):** `Corpo.parametros_primarios` guarda os `LocalId` dos parâmetros do primário: no escopo dos inicializadores do construtor primário (`funcoes.rs`; o corpo declara locais novos) e no inicializador de campo de instância não `late` (`Corpo::para_variavel` chama `declarar_parametros_do_primario`, agora também no campo sem tipo, que antes dava `undefined_identifier`). `expr.rs::escrita_em_primario` relata nas escritas de local da atribuição simples, de `ler_para_escrita` (composta, `??=`, `++`/`--`) e no `AssignedVariablePattern` (`padroes.rs` e `padroes/casamento.rs`).

#### Grupo 4 — declarações de extension type, `main`, diretivas de documentação e FFI

Estes códigos são de **declaração** (não de corpo); o lugar natural no DartForge é `crates/analise/src/*.rs` (verificações por declaração), exceto os de FFI que dependem de tipos resolvidos de invocações (`crates/types`). Nenhum deles existe hoje, salvo `EXTENSION_TYPE_DECLARES_MEMBER_OF_OBJECT` (`crates/analise/src/membros.rs:948`).

**Estado em 2026-10-05 (escrito, não compilado).** `crates/types/src/tipos_de_extensao.rs` (`verificar`,
ligado em `paridade/src/analise.rs` depois de `funcao_main`) porta os ciclos do link com Tarjan e auto-laço:
`extension_type_representation_depends_on_itself`, pelos tipos de extensão citados na representação como o
`_DependenciesCollector`, e `extension_type_implements_itself`. Porta também
`extension_type_representation_type_bottom`, `extension_type_inherited_member_conflict` (os conflitos da
`Heranca`, com o setter `x=`) e os três do `_verifyExtensionElementImplements`:
`implements_disallowed_type`, `implements_representation_not_supertype` e `implements_not_supertype`.
Representação cíclica ou inválida cala o `implements` e o fundo. Um `dynamic` do outline que não foi escrito
`dynamic` é tratado como nome que não resolveu. Desvio: alias de `dynamic` no `implements` não relata.

##### `extension_type_representation_depends_on_itself` (perda 8: FN 8)
- **Emissão:** `ErrorVerifier._checkForExtensionTypeRepresentationDependsOnItself` (`analyzer/lib/src/generated/error_verifier.dart:3491-3500`), de `visitExtensionTypeDeclaration`; o sinal `hasRepresentationSelfReference` é calculado no link (`analyzer/lib/src/summary2/extension_type.dart:120-180`, `_Node`/`_Walker` — grafo de dependência).
- **Condição exata:** o tipo da representação, percorrido por `_DependenciesCollector` (visita argumentos de tipo, tipos de função, records, e o próprio extension type), alcança um extension type da mesma componente fortemente conexa (ciclo, inclusive `extension type A(A it)` e ciclos mútuos A→B→A). Todos os da SCC são marcados, e a representação vira `InvalidType`.
- **Posição:** o nome do extension type (token `name`), em **cada** declaração do ciclo (oráculo: `1:16` e `5:16`).
- **Mensagem:** `The extension type representation can't depend on itself.`
- **Supressões:** com a representação inválida, os demais checks de representação não relatam.
- **No DartForge:** inexistente. **Mudança:** em `crates/analise` (ou no outline de `types`, que já conhece a representação), montar o grafo extension type → extension types citados no tipo da representação, achar SCCs (Tarjan) e relatar no nome de cada membro da SCC (inclusive auto-laço).

##### `extension_type_declares_member_of_object` (perda 7: FN 7)
- **Emissão:** `error_verifier.dart:3384-3390` (método/getter/setter com nome de membro de `Object`: `==`, `hashCode`, `toString`, `noSuchMethod`, `runtimeType`), no token do nome.
- **Mensagem:** `Extension types can't declare members with the same name as a member declared by 'Object'.`
- **No DartForge:** `crates/analise/src/membros.rs:948` (48 acertos). As **7 FN** são de `primary_constructors/header/extension_type_error_test.dart` — **construtor primário com parâmetros nomeados** (`extension type ET12({required final int hashCode})`), sintaxe nova julgada pelo 3.13: o campo declarado no cabeçalho é membro. **Mudança:** quando o parser aceitar o cabeçalho com parâmetros nomeados (3.13), aplicar a regra aos campos do cabeçalho.
- **Feito em 2026-10-07 (55/55):** o 3.13.4 confere o nome do único parâmetro da representação contra os membros
  de `Object` (checkout main `error_verifier.dart:5641-5655`), qualquer que seja a forma do parâmetro (nomeado,
  opcional, `final`); o 3.6.2 não confere (o oráculo de `ExtensionTypeDeclaresMemberOfObject__re_6471d06e.dart`
  é vazio). O parser relata no nome e marca o relato como só do 3.13.4 (`so_3_13`).

##### `extension_type_implements_disallowed_type` (perda 7: FN 7)
- **Emissão:** `ResolutionVisitor._verifyExtensionElementImplements` (`analyzer/lib/src/dart/resolver/resolution_visitor.dart:1786-1800`), para cada tipo da cláusula `implements`.
- **Condição exata:** `!isValidExtensionTypeSuperinterface(type)` (`analyzer/lib/src/dart/element/type_system.dart:1361-1378`): não é `InterfaceType` (parâmetro de tipo `X`, `dynamic`, tipo de função, record…), ou é anulável (`T?`), ou é `FutureOr`, `Function`, `Null`, `Record`.
- **Posição:** o `NamedType` na cláusula. **Mensagem:** `Extension types can't implement '{0}'.` — `{0}` DartType (`'X'`, `'dynamic'`, `'Function'`, `'int?'`).
- **Supressões e ordem:** sai e **para** (não checa supertipo da representação). Só o `hasErrorReported`
  do `NamedTypeResolver` (o alias que expande a parâmetro de tipo, `named_type_resolver.dart:449-475`) pula a
  verificação. O tipo chega depois do `_verifyNullability` (`:388-414`), que relata
  `NULLABLE_TYPE_IN_IMPLEMENTS_CLAUSE` e **tira o `?`**: `implements A?` e `typedef X = num?` não dão este
  código. O nome indefinido não é relatado na cláusula (`reportNullOrNonTypeElement`, `:601-608`: "reported
  elsewhere") e chega como `InvalidType`: `implements Unresolved` e `implements FutureOr<int>` sem importar
  `dart:async` dão `Extension types can't implement 'InvalidType'.`. `{0}` sem o alias: `typedef X = void`
  dá `'void'`, `typedef X = dynamic` dá `'dynamic'`.
- **No DartForge:** `tipos_de_extensao::verificar`; o nome indefinido na cláusula, pelo contexto
  `ContextoDeTipo::Hierarquia` de `resolve.rs`; o `FutureOr` só é o especial de `dart:async` quando está no
  escopo.

##### `extension_type_implements_not_supertype` (perda 6: FN 6)
- **Emissão:** `resolution_visitor.dart:1802-1838`.
- **Condição exata:** tipo válido (acima), a representação **não** é subtipo dele, e ele não é extension type (para extension type, ver `…_REPRESENTATION_NOT_SUPERTYPE`).
- **Posição:** o `NamedType`. **Mensagem:** `'{0}' is not a supertype of '{1}', the representation type.` — `{0}` o tipo implementado, `{1}` a representação declarada (DartTypes).
- **No DartForge:** inexistente; mesmo lugar.

##### `extension_type_inherited_member_conflict` (perda 4: FN 4)
- **Emissão:** `ErrorVerifier._checkForExtensionTypeMemberConflicts` (`error_verifier.dart:3448-3485`) com os `conflicts` da interface (`InheritanceManager3.getInterface`).
- **Condição exata:** algum conflito na interface combinada do extension type: `CandidatesConflict` (dois membros distintos de mesmo nome vindos de supertipos sem sobrescrita local), `HasNonExtensionAndExtensionMemberConflict` (membro de classe e de extension type com mesmo nome), `NotUniqueExtensionMemberConflict` (dois membros de extension types distintos).
- **Posição:** o nome do extension type. **Mensagem:** `The extension type '{0}' has more than one distinct member named '{1}' from implemented types.` (`{0}` nome, `{1}` membro). Um relato por conflito.
- **No DartForge:** inexistente; requer a interface combinada de extension types (`crates/analise/src/heranca.rs`).

##### `extension_type_representation_type_bottom` (perda 4: FN 4)
- **Emissão:** `error_verifier.dart:3503-3513`.
- **Condição exata:** o tipo da representação é bottom (`isBottom`, `analyzer/lib/src/dart/element/type.dart`: `Never` não anulável, parâmetro de tipo cujo limite/promoção é bottom (:1538-...); `Never?` não é bottom).
- **Posição:** o **tipo** do campo de representação (`fieldType`). **Mensagem:** `The representation type can't be a bottom type.`
- **No DartForge:** inexistente; em `crates/analise` com o tipo da representação do outline.

##### `extension_type_constructor_with_super_invocation` (perda 3: FN 3)
- **Emissão:** `error_verifier.dart:3393-3402`, de `visitSuperConstructorInvocation` (:1443-1453).
- **Condição:** inicializador `super(...)`/`super.n(...)` em construtor de extension type. **Posição:** o token `super`. **Mensagem:** `Extension type constructors can't include super initializers.`
- **No DartForge:** inexistente; `crates/analise/src/construtores.rs`.

##### `extension_type_implements_itself` (perda 3: FN 3)
- **Emissão:** `error_verifier.dart:3438-3447`; sinal calculado em `summary2/extension_type.dart:63-110` (`_ImplementsNode`/`_ImplementsWalker`: SCC no grafo extension type → extension types da cláusula `implements`).
- **Posição:** o nome de cada extension type do ciclo. **Mensagem:** `The extension type can't implement itself.`
- **No DartForge:** inexistente; `crates/analise/src/heranca.rs` (há detecção de ciclos de classe que pode ser reaproveitada).

##### `extension_type_constructor_with_super_formal_parameter` (perda 2: FN 2)
- **Emissão:** `ErrorVerifier.visitSuperFormalParameter` (`error_verifier.dart:1456-1466`).
- **Condição:** parâmetro `super.x` em construtor de extension type (antes de qualquer outra checagem de `super.x`). **Posição:** o token `super`. **Mensagem:** `Extension type constructors can't declare super formal parameters.`

##### `extension_type_implements_representation_not_supertype` (perda 2: FN 2)
- **Emissão:** `resolution_visitor.dart:1808-1829`.
- **Condição:** o tipo implementado é extension type cuja representação não é supertipo da representação declarada (e a representação declarada não é subtipo do próprio tipo implementado).
- **Posição:** o `NamedType`. **Mensagem:** `'{0}', the representation type of '{1}', is not a supertype of '{2}', the representation type of '{3}'.` — `{0}` representação implementada (DartType), `{1}` nome do implementado, `{2}` representação declarada, `{3}` nome do declarante.

##### `main_first_positional_parameter_type` (perda 3: FN 3)
- **Emissão:** `ErrorVerifier._checkForMainFunction2` (`analyzer/lib/src/generated/error_verifier.dart:4143-4188`), de `visitFunctionDeclaration` de topo.
- **Condição exata:** função de topo `main` com algum parâmetro posicional; `List<String>` não é subtipo do tipo do primeiro posicional.
- **Posição:** o **tipo** escrito do primeiro posicional (`first.notDefault.typeOrSelf`: o `NamedType`, ou o próprio parâmetro se sem tipo). **Mensagem:** `The type of the first positional parameter of the 'main' function must be a supertype of 'List<String>'.`
- **No DartForge:** inexistente; `crates/analise` (declarações de topo).

##### `main_has_too_many_required_positional_parameters` (perda 3: FN 3)
- **Emissão:** a mesma função (`error_verifier.dart:4160-4166`). **Condição:** mais de 2 posicionais obrigatórios. **Posição:** o nome `main`. **Mensagem:** `The function 'main' can't have more than two required positional parameters.`

##### `main_has_required_named_parameters` (perda 2: FN 2)
- **Emissão:** `error_verifier.dart:4168-4173`. **Condição:** algum nomeado `required`. **Posição:** o nome `main`. **Mensagem:** `The function 'main' can't have any required named parameters.` (sai junto com o anterior se ambos.)

##### `doc_directive_missing_argument` (perda 10: FN 10)
- **Emissão:** `DocCommentVerifier.validateArgumentCount` (`analyzer/lib/src/error/doc_comment_verifier.dart:52-92`), sobre as diretivas `{@tipo …}` montadas por `DocCommentBuilder` (`analyzer/lib/src/fasta/doc_comment_builder.dart`); chamado de `ErrorVerifier.visitComment`/`DocCommentVerifier.docDirective`.
- **Condição exata:** menos argumentos posicionais que os exigidos pelo tipo (`DocDirectiveType`: `animation` (width, height, url), `youtube` (width, height, url), `macro`/`template` (name), `canonicalFor` (element), `category`/`subCategory` (resto), `inject-html`, `tool`, etc.). Variantes por diferença: 1 → `DOC_DIRECTIVE_MISSING_ONE_ARGUMENT [tipo, último exigido]`; 2 → `…_TWO_ARGUMENTS [tipo, penúltimo, último]`; 3 → `…_THREE_ARGUMENTS`.
- **Posição:** a tag inteira (`tag.offset` a `tag.end`: do `{@` ao `}`).
- **Mensagem:** `The '{0}' directive is missing a '{1}' argument.` / `… a '{1}' and a '{2}' argument.` / `… a '{1}', a '{2}', and a '{3}' argument.`
- **No DartForge:** inexistente; exige um analisador de comentários de documentação (`///` e `/** */`) que reconheça as diretivas. Lugar: um módulo novo em `crates/analise`.
- **Feito (`crates/analise/src/a_doc.rs`), posição corrigida em 2026-10-07 (10/10, e `doc_directive_missing_closing_brace` 2/2):** o `_DirectiveParser` do 3.6.2 (`doc_comment_builder.dart:1084-1200`) guarda em `_offset` o offset do `{@` e soma a ele o ÍNDICE NO CONTEÚDO DA LINHA (tudo depois do `///`, com o espaço inicial): o fim da tag é `_offset + index` e a chave ausente fica em `_offset + _length - 1`. Os dois saem deslocados pelo que precede o `{@` na linha (`/// {@youtube 600 400` dá tag de comprimento 18 e a chave ausente uma coluna depois do fim da linha), e reproduzimos o deslocamento. A diretiva sem nada depois do nome (`/// {@youtube`) termina no fim da linha, sem argumentos e SEM `DOC_DIRECTIVE_MISSING_CLOSING_BRACE` (`directive` fixa `_end` quando `index == _length`, e `_parseArguments` retorna antes).

##### `doc_directive_argument_wrong_format` (perda 5: FN 5)
- **Emissão:** `doc_comment_verifier.dart:121-150`. **Condição:** argumento posicional com formato esperado `integer` que não é inteiro (`width`, `height` de `animation`/`youtube`), ou `uri` inválida. **Posição:** o argumento. **Mensagem:** `The '{0}' argument must be formatted as {1}.` — `{1}` = `displayString` do formato (`an integer`, `a URI`, …).

##### `doc_directive_has_extra_arguments` (perda 4: FN 4)
- **Emissão:** `doc_comment_verifier.dart:97-108` (e `doc_comment_builder.dart:1241` para blocos). **Condição:** mais posicionais que os exigidos e o tipo não aceita resto. **Posição:** do primeiro excedente ao fim do último argumento. **Mensagem:** `The '{0}' directive has '{1}' arguments, but only '{2}' are expected.` (`{1}`,`{2}` números).

##### `doc_directive_has_unexpected_named_argument` (perda 2: FN 2)
- **Emissão:** `doc_comment_verifier.dart:110-118`. **Condição:** argumento nomeado (`nome=valor`) que o tipo não declara. **Posição:** o argumento nomeado inteiro. **Mensagem:** `The '{0}' directive has an unexpected named argument, '{1}'.`

##### `argument_must_be_a_constant` (perda 9: FN 9)
- **Emissão:** `FfiVerifier` (`analyzer/lib/src/generated/ffi_verifier.dart`): `_validateIsLeafIsConst` (:1606-1624: argumento nomeado `isLeaf` de `asFunction`/`lookupFunction` não constante), `_validateFromFunction` (:1595-1602: `exceptionalReturn` de `Pointer.fromFunction`), e `NativeCallable.isolateLocal` (:1822-1828).
- **Posição:** a expressão do argumento (sem o rótulo). **Mensagem:** `Argument '{0}' must be a constant.` (`{0}` `isLeaf`/`exceptionalReturn`).
- **No DartForge:** sem verificador de FFI; seria em `crates/types` (invocações resolvidas de membros de `dart:ffi`).

##### `argument_must_be_native` (perda 3: FN 3)
- **Emissão:** `ffi_verifier.dart:1715-1755` (`Native.addressOf(arg)`): o argumento não é um identificador/tear-off constante de função ou variável anotada `@Native`. **Posição:** o argumento. **Mensagem:** `Argument to 'Native.addressOf' must be annotated with @Native`.

##### `leaf_call_must_not_return_handle` (perda 7: FN 7)
- **Emissão:** `FfiVerifier._validateFfiLeafCallUsesNoHandles` (`ffi_verifier.dart:1413-1430`), chamado para `asFunction(isLeaf: true)`, `lookupFunction(isLeaf: true)`, e `@Native(isLeaf: true)`/`@FfiNative(isLeaf: true)` em funções/getters.
- **Condição:** tipo nativo é função com retorno `Handle`. **Posição:** depende do chamador: `asFunction(isLeaf: true)` → o token do nome `asFunction` (`ffi_verifier.dart:1239-1240`); `lookupFunction<S, F>(…, isLeaf: true)` → o primeiro argumento de tipo `S` (:1661-1662); anotação `@Native`/`@FfiNative` → o `errorToken` da declaração anotada (nome da função/getter, :606-609). **Mensagem:** `FFI leaf call can't return a 'Handle'.`

##### `leaf_call_must_not_take_handle` (perda 5: FN 5)
- Mesma função (:1424-1429): algum parâmetro normal `Handle` (um relato por parâmetro). **Mensagem:** `FFI leaf call can't take arguments of type 'Handle'.`

##### `non_sized_type_argument` (perda 2: FN 2)
- **Emissão:** `ffi_verifier.dart:1474-1488` (campo `Array<T>` de `Struct`/`Union` com `T` não dimensionado, ex. `Void`). **Posição:** o primeiro argumento de tipo do campo (ou o tipo). **Mensagem:** `The type '{1}' isn't a valid type argument for '{0}'. The type argument must be a native integer, 'Float', 'Double', 'Pointer', or subtype of 'Struct', 'Union', or 'AbiSpecificInteger'.` (`{0}` `'Array'`, `{1}` o tipo).

##### `non_native_function_type_argument_to_pointer` (perda 1: FN 1)
- **Emissão:** `ffi_verifier.dart:1210-1225` (`Pointer<T>.asFunction` com `T` que não é `NativeFunction` de assinatura C válida). **Posição:** `errorNode` (o nome `asFunction`). **Mensagem:** `Can't invoke 'asFunction' because the function signature '{0}' for the pointer isn't a valid C function signature.` (`{0}` o tipo `T`).

##### `use_of_native_extension` (perda 3: FN 3)
- **Emissão:** `LibraryAnalyzer._reportImportDirectiveErrors` (`analyzer/lib/src/dart/analysis/library_analyzer.dart:672-684`) e export (:906-916); também para `@docImport`.
- **Condição:** URI selecionada começa com `dart-ext:`. **Posição:** a URI (string). **Mensagem:** `Dart native extensions are deprecated and aren't available in Dart 2.15.`
- **No DartForge:** inexistente; `crates/analise/src/importacoes.rs` (antes de `uri_does_not_exist`).
- **Estado em 2026-10-05 (escrito, não compilado):** na fase 2 da `paridade`, antes do alvo ausente, para
  `import`/`export` e para os `@docImport` do doc da biblioteca.

#### Grupo 5 — argumentos de tipo (limites, inferência, contagem)

##### `type_argument_not_matching_bounds` (perda 99: FN 97, FP 2)
- **Corrigido em 2026-10-08** (`TypeAliasCannotReferenceItself__functio_*` e `__generic_*`): o `typedef` que chega a si
  mesmo (`TypeAliasSelfReferenceFinder`, `summary2/type_alias.dart`) instancia como `dynamic`
  (`TypeAliasElementImpl.instantiate`, `element.dart:9690-9696`), e a conferência de limites não roda no argumento
  dele. No DartForge o detector saiu do `analise` para `types::auto_referencia` e vale nos dois resolvedores
  (`resolve.rs`, com cache por `typedef`, e `inferencia/tipos.rs`); o `is F` com esse alias é o `is dynamic` do
  `_checkAllTypeChecks` (`best_practices_verifier.dart:793-797`), sempre `UNNECESSARY_TYPE_CHECK_TRUE`.
- **Emissão:** vários pontos, todos com a mesma mensagem:
  - `TypeArgumentsVerifier.checkNamedType` → `_checkNamedTypeArguments` (`analyzer/lib/src/error/type_arguments_verifier.dart:290-400`): tipos nomeados escritos (`C<String>` em anotações de tipo, `extends`/`implements`, literais de tipo, `new`/`const`), com a regra de **bem-limitado**: primeiro regular-bounded (`T_i <: B_i[T/X]`); se falha, e o contexto permite super-bounded (não em `new`/`const`/supertipo — `allowSuperBounded`), testa super-bounded (troca topo↔`Never` em posições covariantes/contravariantes, `isWellBounded`) e só relata se também falha (:340-400);
  - `checkEnumConstantDeclaration` (:91-140): argumentos (explícitos **ou inferidos**, `constructorElement.returnType.typeArguments`) de constante de enum; posição o argumento explícito ou o **nome** da constante;
  - `_checkInvocationTypeArguments` (:404-492): `f<T>()`/`o.m<T>()`/`f<T>` tear-off (`checkFunctionReference`, `checkMethodInvocation`, `checkFunctionExpressionInvocation`): argumento explícito contra o limite;
  - `InvocationInferrer.resolveInvocation` (`analyzer/lib/src/dart/resolver/invocation_inferrer.dart:186-209`): para anotações e construtores **com argumentos explícitos** (`_needsTypeArgumentBoundsCheck`);
  - `ExtensionMemberResolver._checkTypeArgumentsMatchingBounds` (`analyzer/lib/src/dart/resolver/extension_member_resolver.dart:250-270`): `E<String>(x)` — só com argumentos **explícitos** (`typeArgumentList != null`), no argumento `i`; os inferidos do override (`E(x)`) passam pelo `GenericInferrer` com `errorEntity: node.name` (:354) e saem como `COULD_NOT_INFER` no **nome da extensão**;
  - inferidos em geral **não** relatam este código (o `GenericInferrer` usa `COULD_NOT_INFER`).
- **Condição exata:** `!isSubtypeOf(T_i, B_i[T/X])` (limite substituído pelos próprios argumentos), com o desconto de super-bounded onde permitido; argumentos em número errado não checados (outro código).
- **Posição:** o argumento de tipo `i` (o nó do argumento); se não há nó (inferido), o nó do tipo/constante.
- **Mensagem:** `'{0}' doesn't conform to the bound '{2}' of the type parameter '{1}'.` — `{0}` argumento (DartType), `{1}` nome do parâmetro, `{2}` limite substituído (DartType).
- **No DartForge:** `crates/types` (outline/tipos; 1078 acertos). Causas das FN: **34 `generic/super_bounded_types_error*_test`** (o teste de super-bounded: tipos que não são regular- nem super-bounded em contextos onde super-bounded não é aceito — `new`, `extends`, argumentos de construtor), **14 `variance/*`** (experimento), **8 `inference/issue_61370*`** e **`closure/partial_instantiation_static_bounds_check`** (instanciação implícita de tear-off genérico), **3 `extension_methods/static_extension_bounds_error_test`** + `TypeArgumentNotMatchingBounds__extensio_*` (override de extensão), **enum** (`enum_inferred`, `enum_wit…`: argumentos inferidos de constante de enum), **`methodIn_*`** (`o.m<String>()`), **`function*`/`functionReference`** (`f<String>`), **`typeLiteral_*`/`typeLite_*`** (`C<String>` como expressão), **`metadata_*`** (`@A<String>()`), `extends__…` (`T extends U` com `U` não conforme), `null_aware_elements/flow_analysis_test` (3), `type_variable/bounds2-4`, `regress34532/18628`. FP: `type_alias_cannot_reference_itself/*` (alias cíclico: o analyzer não checa limites de alias inválido).
  **Mudança:** completar cada ponto de emissão (invocações com argumentos explícitos em `chamadas.rs::invocar`/`argumentos_de_tipo`, overrides de extensão em `chamada`, constantes de enum em `funcoes.rs`, literais de tipo em `expr.rs`) e o teste de super-bounded no verificador de tipos nomeados.
- **Correções desta especificação (2026-10-06):**
  - os `variance/*` não dependem do experimento: a subtipagem de interfaces usa a variância declarada
    (`subtype.dart:305-344`, ver INFERENCIA §4.1), e é ela que faz `Contravariant<Lower>` violar o limite
    `Contravariant<Middle>`;
  - o limite comparado em `_checkInvocationTypeArguments` é o do tipo **invocado já substituído** (`node.function.
    staticType`): em `x.expectStaticType<Exactly<int>>()` o limite `Exactly<T>` da extensão sai `Exactly<int?>` pelo
    tipo inferido de `T` (formais frescos na substituição, INFERENCIA §8.4);
  - `checkNamedType` lê o `NamedType.type` de **todo** nó, inclusive os aninhados (argumentos de tipo, partes de
    função e de registro) e os crus (instanciados aos limites, com a mensagem de contexto "The raw type was
    instantiated as …"); o alvo de `factory … = B<T>.nome` e a criação sem `new` (`Alias<A>()`, `C<T>.nome()`) são
    `ConstructorName` (sem super-bounded); `Cb<String>` com `Cb` alias de tipo de função é literal de tipo;
  - `@B<T>()` com `B` alias: os argumentos contra os parâmetros **do alias** (`AnnotationInferrer`);
  - constante de enum sem construtor declarado (`v<String>()` com o `const E()` sintético): os escritos.
- **Estado (2026-10-06):** 1174 acertos, FN 1, FP 17. Os FP restantes são diferenças de promoção nossas expostas pela
  conferência (`Exactly<…>` em cascatas, variável booleana de condição, registros) e os 2 de alias cíclico.

##### `could_not_infer` (perda 45: FN 26, msg 16, pos 3)
- **Emissão:** `GenericInferrer.chooseFinalTypes`/`_chooseTypes` (`analyzer/lib/src/dart/element/generic_inferrer.dart:250-370`) e `_checkArgumentsNotMatchingBounds` (:390-420), com o `errorEntity` dado pelo chamador (`InvocationInferrer._errorEntity`: o nó da invocação; para `MethodInvocation` o `methodName`; para criação de instância o `constructorName`; tear-off/`FunctionReference` o nó).
- **Condição exata (variantes do `{1}`):**
  1. o tipo inferido não satisfaz as restrições (inclui o limite `extends`): `{1}` = `_formatError(...)`: `"\n\nTried to infer '<T>' for '<X>' which doesn't work:\n  <restrições, uma por linha>\nConsider passing explicit type argument(s) to the generic.\n\n"` (texto exato em `generic_inferrer.dart:~430-520`, `_formatError`/`_messageForConstraint`);
  2. candidato é tipo de função genérico sem `generic-metadata`: `" Inferred candidate type ... has type parameters [...], but a function with type parameters cannot be used as a type argument."`;
  3. `instantiateToBounds` com limite recursivo (`hasError`): `"\nRecursive bound cannot be instantiated: '<bound>'.\nConsider passing explicit type argument(s) to the generic.\n\n'"`;
  4. sem erro anterior, argumentos finais que não respeitam o limite substituído: `"\n'<arg>' doesn't conform to the bound '<bound>', instantiated from '<rawBound>' using type arguments [<args>]."` (lista com `toString` de `List`: `[A, B]`).
- **Posição:** o `errorEntity`.
- **Mensagem:** `Couldn't infer type parameter '{0}'.{1}` — `{1}` como acima (tipos com `_typeStr` = `getDisplayString()`).
- **No DartForge:** `chamadas.rs::invocar` → `GenericInferrer::falhas` (`crates/types/src/constraints.rs`). Causas: **16 msg** — o sufixo `{1}` (a variante 4 com "instantiated from … using type arguments […]" e a variante 1 com a lista de restrições) não está igual; **FN**: `downwardInference_fixes` (contexto de retorno fixa `T` incompatível), `functionType_parameterIs*` (parâmetro de tipo função com variável em posição contravariante), `instanceCreation_viaType…` (via alias), `topLevel`, `variance/*` (experimento), `generic/function_type_as_type_argument` (variante 2); **pos**: `static_extension_bounds_error_test:37/60/127` (`E1(s).e1` com inferência do override falhando: a entidade é o **nome da extensão** `E1` — `extension_member_resolver.dart:354`, `errorEntity: node.name`).
  **Mudança:** reproduzir `_formatError` e as variantes 2–4 literalmente; ligar os pontos que faltam.
- **Feito em 2026-10-06 (51/54, msg 0):**
  - **Decisão pela escolha do `_chooseTypes`.** O `tryChooseFinalTypes` confere cada parâmetro com a escolha do `_chooseTypes(preliminary: false)` (a inferior conhecida vence, com a cláusula `extends` substituída pelos já escolhidos), e só sem erro roda `_checkArgumentsNotMatchingBounds` sobre os tipos instanciados para os limites. O nosso `GenericInferrer::falhas` decidia pelo tipo final, "pelo lado seguro", e filtrava restrições de argumento `dynamic` e parâmetros `FutureOr`; as duas aproximações saíram.
  - **`isSatisfiedBy`** (`type_constraint.dart:128-135`): o tipo contra o **fecho maior** do esquema (`A<_>` vale `A<Object?>`), e o `_` é topo e fundo no `isSubtypeOf` (`subtype.dart:41-45`, agora também em `subtyping.rs`).
  - **Restrições de parâmetro já fixado são descartadas** (`_tryMatchSubtypeOf`, `generic_inferrer.dart:880-906`): `Future<Future<String>> v = Future.value(Future<String>.value(''))` não relata, porque o `String <: T` do argumento não entra (`T` já é `Future<String>` pela fase preliminar). Nada é registrado se o casamento falha (já era assim).
  - **Contexto `dynamic` é `_`**: o `analyzeExpression` compartilhado (`type_analyzer.dart:557-561`) troca o esquema `dynamic` por `_`, e o corpo com retorno `dynamic` (escrito ou implícito) não tem tipo imposto (`resolver.dart:2325`, `:3185`, `function_expression_resolver.dart:46`). Em `dynamic c = max(x, y)` a fase preliminar não fixa `T`, e a escolha final é `dynamic` contra o `extends num`. O gerador cujo retorno não é `Iterable<S>`/`Stream<S>` cai, como o assíncrono, em `FutureOr<futureValueTypeSchema(r)>` (`body_inference_context.dart:168-214`).
  - **Instanciação de tear-off** (`inferFunctionTypeInstantiation`): o `GenericInferrer` com relator, origem `Function type declared as … used where … is required.` e a expressão como entidade; também na referência implícita a `call` genérico (`expr.rs::instanciar_em_contexto_com`).
  - **Literais de coleção** (`typed_literal_resolver.dart:460-490`, `:740-815`): `chooseFinalTypes` com o literal como entidade; os parâmetros sintéticos são `element`, `key` e `value` sem `genericClass` (origem `Parameter '…'`), e os parâmetros de tipo têm os nomes do SDK (`E`, `K`, `V`).
  - **Variante 2** (candidato tipo de função genérico) só sem `generic-metadata` (2.14): `GenericInferrer.metadados_genericos` pela biblioteca.
- **Restam (3 FN):** `instanceCreation_viaType_5aa50abd.dart`, criação por alias com limite F (`typedef A<X extends G<C<X>>> = C<X>; A()`), em que o `X` inferido é `C<Object?> Function(C<Never>)`.

##### `not_instantiated_bound` (perda 35: FN 35)
- **Emissão:** `ErrorVerifier.visitTypeParameter` → `node.bound?.accept(_uninstantiatedBoundChecker)` (`analyzer/lib/src/generated/error_verifier.dart:1590`, classe `_UninstantiatedBoundChecker` :7167-7190).
- **Condição exata:** no **limite** de um parâmetro de tipo (de classe, mixin, função, typedef, método…), cada `NamedType` **sem argumentos de tipo** cujo elemento é genérico e **não é simplesmente limitado** (`isSimplyBounded == false`, calculado em `analyzer/lib/src/summary2/simply_bounded.dart`: um parâmetro cujo limite menciona, direta ou indiretamente, tipos genéricos não instanciados que dependem dele — ciclos `class A<T extends A>`). Os `NamedType` com argumentos são visitados recursivamente (só os argumentos).
- **Posição:** o `NamedType` cru. **Mensagem:** `Type parameter bound types must be instantiated.`
- **Supressões:** o TODO diz que relata mesmo com `TYPE_ALIAS_CANNOT_REFERENCE_ITSELF`.
- **No DartForge:** inexistente. **Mudança:** calcular "simplesmente limitado" para classes/aliases (algoritmo da especificação, seção "Simply bounded") no outline e, nos limites de parâmetros de tipo, relatar cada tipo cru não simplesmente limitado (`crates/analise`/`crates/types`).
- **Estado em 2026-10-05 (escrito, não compilado):** `crates/analise/src/limites_simples.rs` porta o
  `computeSimplyBounded`: o `_visitType` e o `_TypeCollector` sobre o AST, dependências por tipos crus e
  Tarjan com auto-laço. Roda uma vez por programa (`calcular`, na `paridade`). `verificar` relata
  `not_instantiated_bound` em todas as listas de parâmetros de tipo da unidade: declarações, funções, métodos,
  locais, tipos de função e parâmetros-função. Um nome é parâmetro de tipo quando alguma lista cujo dono
  contém a posição o declara.

##### `wrong_number_of_type_arguments` (perda 32: FN 31, FP 1)
- **Emissão:**
  - `NamedTypeResolver._buildTypeArguments` (`analyzer/lib/src/dart/resolver/named_type_resolver.dart:137-148`): qualquer tipo nomeado escrito com contagem errada (inclusive `dynamic<int>`, `Never<int>`, `new C<int>()`, `const C<int>()`); `{0}` = `node.name2.lexeme` (o nome escrito, sem prefixo), posição **o `NamedType` inteiro** (`C<int>`, `dynamic<int>`);
  - `InvocationInferrer` (`invocation_inferrer.dart:316-327`, anotação: `_wrongNumberOfTypeArgumentsErrorCode = WRONG_NUMBER_OF_TYPE_ARGUMENTS` em `AnnotationInferrer` :84-87 e `AugmentedInvocationInferrer` :127-130): `@A<int>()` com contagem errada — `{0}` = o **tipo de função cru do construtor** (`'A Function()'`, `'A<T, U> Function()'`), posição a **lista de argumentos de tipo** `<int>`;
  - `function_reference_resolver.dart:316-325`/`:825-833` (tear-offs de tipo `C<int>.new`? / literal de tipo).
- **Mensagem:** `The type '{0}' is declared with {1} type parameters, but {2} type arguments were given.`
- **No DartForge:** existe para tipos nomeados em anotações de tipo (36). FN: **`new`/`const` com contagem errada** (`new_*`, `const_*`, `factory*_test`, `unsorted/invalid_type_argument_count_test`), **anotações** (`metadata_*`, `messageText_ca92c414`/`afc9f656`), `dynamic<int>`/`Never<int>` (`messageText_dynamic`/`never`), `mixin/type_parameters_errors_test` (em `with`/`on`), `type_object/explicit_instantiated_type_literal…`. FP: `augmentation_type_parameter_count/*` (augmentation — outro código).
  **Mudança:** em `chamadas.rs::instanciacao` (tipo da criação) e no caminho de anotações (`funcoes.rs::anotacao_sem_validar`, com o tipo de função do construtor), e no resolvedor de tipos para `dynamic`/`Never`.
- **Correção e estado (2026-10-06):** o `{0}` da variante de anotação é o tipo do construtor **como função genérica
  nos parâmetros de tipo** da classe — `'A<T, U> Function<T, U>()'` — e, por alias (`@B<int>()` com `typedef B<T, U> =
  A`), nos do alias, com o retorno pelo alvo (`'A Function<T, U>()'`). Todos os pontos portados (67 de 67): o tipo da
  criação `new`/`const` (contagem no `NamedType` inteiro, argumentos `InvalidType`), a criação sem `new` (`Map<int>()`,
  `C<T>.nome()`: o `NamedType` da reescrita), o alvo de `factory … = C<T>.nome`, `dynamic<int>`/`Never<int>` e a
  anotação.

##### `wrong_number_of_type_arguments_function` (perda 13: FN 13)
- **Emissão:** variantes `WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION` (`'The function '{0}' is declared with {1} type parameters, but {2} type arguments were given.'`) e `…_ANONYMOUS_FUNCTION` (`'This function is declared with {0} type parameters, but {1} type arguments were given.'`), em `FunctionReferenceResolver` (`analyzer/lib/src/dart/resolver/function_reference_resolver.dart:110-130`, :255-300): tear-off `f<int, String>` (função nomeada; `{0}` = nome, inclusive `'call'` para `o.call<…>`) ou expressão de função (`(f)<int>`, `fn<int>` de variável de tipo função → anônima). Também nas constantes (`constant/evaluation.dart:2315-2332`).
- **Posição:** a lista de argumentos de tipo.
- **No DartForge:** inexistente (tear-offs com argumentos de tipo em `expr.rs`, `ExprKind::TypeArguments`). **Mudança:** checar a contagem nas instanciações explícitas de tear-off.
- **Estado em 2026-10-07 (placar 13/13):** o `ExprKind::TypeArguments` do `expr.rs` relata na lista `<…>`.
  * O nome vem de `nome_de_funcao_referida`. A função local conta pelo conjunto `funcoes_locais`, porque a
    marca `funcao_local` some depois da inferência dela (`foo<int, int>` de uma função local leva `'foo'`).
  * A variante anônima tem três argumentos: `[declarados, dados, tipo da função]`. O texto do 3.13.4 é
    `The type of this function is '{2}', which has {0} type parameters, but {1} type arguments were given.`,
    e a variante sai pelo `VARIANTES_3_13` do `gerar_codigos.rs`.
  * O objeto com o método `call` (`c<int>`, `_resolveAsImplicitCallReference`, `:287-306`) confere a contagem
    com os parâmetros de tipo do `call`, com `{0}` = `'call'`. O tipo é o do `call` instanciado, com `dynamic`
    nos argumentos quando a contagem erra. Um `call` que é getter ou campo não entra nesse ramo
    (`_getCallMethod` só aceita `MethodElement`) e cai no `_resolveDisallowedExpression`.

##### `wrong_number_of_type_arguments_constructor` (perda 12: FN 12)
- **Emissão:** `ast_rewrite.dart:430-445` e :600-612 (`C.named<int>()` reescrito para criação: argumentos de tipo **depois** do nome do construtor), `named_type_resolver.dart:333-345` (`p.C<int>` onde `C` é construtor? — `prefix.Class<…>` reinterpretado), `function_reference_resolver.dart:52-62` (`C.named<int>` tear-off).
- **Condição:** argumentos de tipo aplicados ao **construtor** (`C.named<int>`), o que nunca é permitido.
- **Posição:** a lista de argumentos de tipo. **Mensagem:** `The constructor '{0}.{1}' doesn't have type parameters.` (`{0}` classe, `{1}` construtor). Variante de dot shorthand (3.10, não 3.6.2) no oráculo novo.
- **No DartForge:** inexistente; `chamadas.rs::alvo_construtor` (alvo `TypeArguments` sobre `C.named`).
- **Estado em 2026-10-05 (escrito, não compilado):** `chamadas::tipos_no_construtor` relata na lista `<…>`
  (`faixa_da_lista_de_tipos`), com `{0}` o tipo escrito com o prefixo. Cobre três formas. Em `C.nome<T>()`, a
  `chamada` passa a não aplicar a lista à classe. Em `new C.nome<T>()`, o `instanciacao` cobre o ramo
  `ctor_do_nome`. O tear-off `C.nome<T>` sai no `ExprKind::TypeArguments` do `expr.rs`, com o nó
  `InvalidType`. No mesmo ramo saem `wrong_number_of_type_arguments_function`/`_anonymous_function`, conforme
  `nome_de_funcao_referida` (função, método ou função local declarados dão o nome), com os argumentos
  `dynamic`. Sai também `disallowed_type_instantiation_expression`, de 2.15 em diante, para tipo que não é
  função nem objeto com `call`; o parâmetro de tipo vale pelo limite.
- **Estado em 2026-10-07 (placar 12/12):** a variante do atalho de ponto (3.13) entrou como código novo do
  suplemento, `WRONG_NUMBER_OF_TYPE_ARGUMENTS_DOT_SHORTHAND_CONSTRUCTOR` no `gerar_codigos.rs`.
  * A mensagem é `The dot shorthand resolves to the constructor '{0}.{1}', …`.
  * Sai na lista `<…>` de `.a<T>()` quando a classe não é a abstrata do código anterior.

##### `wrong_number_of_type_arguments_method` (perda 6: FN 6)
- **Emissão:** `FullInvocationInferrer._wrongNumberOfTypeArgumentsErrorCode` (`invocation_inferrer.dart:151-155`) via `_reportWrongNumberOfTypeArguments` (:316-327): `f<int>()`/`o.m<int, String>()` com contagem diferente da do tipo invocado.
- **Posição:** a lista de argumentos de tipo. **Mensagem:** `The method '{0}' is declared with {1} type parameters, but {2} type arguments are given.` — `{0}` é o **tipo de função cru** (`'void Function<T, U>()'`, DartType), não o nome.
- **Supressões:** os argumentos viram `dynamic` e a inferência segue sem os explícitos.
- **No DartForge:** `chamadas.rs::invocar` ignora explícitos com contagem errada (`if ex.len() == type_params.len()`). **Mudança:** relatar com o tipo formatado.
- **Estado em 2026-10-05 (escrito, não compilado):** `invocar` relata o código quando a lista explícita tem contagem diferente da do tipo invocado, inclusive quando ele não é genérico; a inferência segue sem os explícitos, como antes. A lista `<…>` é localizada no texto, do `<` antes do primeiro argumento ao `>` depois do último. Fora: os argumentos passam a ser `dynamic` no original, e aqui continuam inferidos.

##### `wrong_number_of_type_arguments_extension` (perda 4: FN 4)
- **Emissão:** `extension_member_resolver.dart:325-345` (`E<int>(x)` com contagem errada). **Posição:** a lista de argumentos de tipo. **Mensagem:** `The extension '{0}' is declared with {1} type parameters, but {2} type arguments were given.`
- **No DartForge:** `chamadas.rs::chamada` (ramo `RefTipo::Extensao`) usa `ext_args` só se a contagem bate; relatar quando não bate.

##### `wrong_number_of_type_arguments_enum` (perda 3: FN 3)
- **Emissão:** `TypeArgumentsVerifier.checkEnumConstantDeclaration` (`type_arguments_verifier.dart:91-112`). **Posição:** a lista `<…>` da constante. **Mensagem:** `The enum is declared with {0} type parameters, but {1} type arguments were given.`
- **No DartForge:** inexistente; em `funcoes.rs` (constantes de enum, onde `k.type_args` é lido).
- **Estado em 2026-10-05 (escrito, não compilado):** `inferir_metadados_da_unidade` relata, com o
  construtor resolvido (declarado ou o sintético), na lista `<…>` da constante.

##### `expected_one_list_type_arguments` (perda 1: FN 1)
- **Emissão:** `TypeArgumentsVerifier.checkListLiteral` (`type_arguments_verifier.dart:160-172`). **Condição:** literal de lista com ≠ 1 argumento de tipo. **Posição:** a lista de argumentos de tipo. **Mensagem:** `List literals require one type argument or none, but {0} found.`

##### `expected_one_set_type_arguments` (perda 1: FN 1)
- **Emissão:** `checkSetLiteral` (:208-221). **Condição:** literal resolvido como conjunto com ≠ 1 argumento (com mais de 2 argumentos o literal `{}` é decidido conjunto/mapa pelos elementos). **Mensagem:** `Set literals require one type argument or none, but {0} were found.`

##### `expected_two_map_type_arguments` (perda 2: FN 2)
- **Emissão:** `checkMapLiteral` (:175-188). **Condição:** literal de mapa com ≠ 2 argumentos. **Mensagem:** `Map literals require two type arguments or none, but {0} found.`
- **No DartForge (os três):** inexistentes; em `colecoes.rs` ao ler os argumentos explícitos do literal.

##### `invalid_extension_argument_count` (perda 2: FN 2)
- **Emissão:** `ExtensionMemberResolver.resolveOverride` (`extension_member_resolver.dart:189-197`). **Condição:** `E(...)` com número de argumentos ≠ 1 (inclusive nomeados). **Posição:** a lista de argumentos `(...)`. **Mensagem:** `Extension overrides must have exactly one argument: the value of 'this' in the extension method.`
- **No DartForge:** `chamadas.rs::chamada` só trata `args.len() == 1`; relatar nos outros casos.

##### `enum_instantiated_to_bounds_is_not_well_bounded` (perda 1: FN 1)
- **Emissão:** `error_verifier.dart:3239-3255`. **Condição:** o tipo do campo `values` (`List<E<instanciado aos limites>>`) tem argumento não bem-limitado (`isWellBounded(…, allowSuperBounded: true)`). **Posição:** o nome do enum. **Mensagem:** `The result of instantiating the enum to bounds is not well-bounded.`

##### `generic_function_type_cannot_be_type_argument` (perda 2: FN 2)
- **Emissão:** `type_arguments_verifier.dart:300-310` (tipo nomeado) e :465-475 (invocação), **só sem** o recurso `generic-metadata` (linguagem < 2.14). **Posição:** o argumento. **Mensagem:** `A generic function type can't be a type argument.` — os 2 casos do corpus são arquivos de versão de linguagem antiga (`// @dart=2.x`).

##### `disallowed_type_instantiation_expression` (perda 2: FN 2)
- **Emissão:** `function_reference_resolver.dart:270-280`, :336-345, :490-497, :562-570: argumentos de tipo aplicados a uma expressão que não é tipo genérico, função genérica, método genérico nem construtor genérico (ex.: `(f)<int>` com `f` não genérica de tipo não função, getter, etc.). **Posição:** a expressão alvo (`function`). **Mensagem:** `Only a generic type, generic function, generic instance method, or generic constructor can have type arguments.`
- **Corrigido em 2026-10-07 (2/2, FP 0):** `p.nome<…>` com `nome` que não resolve no prefixo (import ausente,
  `shouldIgnoreUndefined`) não chega ao `_resolve` da referência: tipo inválido, sem relato.

##### `instantiate_type_alias_expands_to_type_parameter` (perda 7: FN 7)
- **Emissão:** `NamedTypeResolver._verifyTypeAliasForContext` (`analyzer/lib/src/dart/resolver/named_type_resolver.dart:420-450`). **Condição:** alias cujo `aliasedType` é parâmetro de tipo (`typedef T<X> = X;`) usado em `new`/`const T<…>()` (criação) ou como alvo de redirecionamento de factory. **Posição:** `_ErrorHelper._getErrorRange(node)` — o nome do tipo **com prefixo e argumentos de tipo** (do início do nome ao fim dos argumentos). **Mensagem:** `Type aliases that expand to a type parameter can't be instantiated.`
- **No DartForge:** `chamadas.rs::instanciacao`, ramo `Element::Typedef` (comentário diz "ficam mudos" para `typedef A<X> = X`). **Mudança:** relatar quando o alvo do alias é parâmetro de tipo (também na criação implícita `T<int>()` reescrita).
- **Estado em 2026-10-05 (escrito, não compilado):** `instanciacao` relata quando o tipo do alias no outline é
  um parâmetro de tipo, também por outro alias. A posição é o nome, sem os argumentos, como no oráculo, com
  comprimento 1. `new A.nome()` com `A` alias de parâmetro relata no `A`. A criação implícita `T()` não é
  reescrita pelo `AstRewriter` para alias de parâmetro e fica de fora.

#### Grupo 6 — aridade de argumentos, `super.x` e construtores indefinidos

Base: `ResolverVisitor.resolveArgumentsToParameters` (`analyzer/lib/src/generated/resolver.dart:4229-4355`) é chamado, com `errorReporter`, por: `InvocationInferrer.resolveInvocation` (`analyzer/lib/src/dart/resolver/invocation_inferrer.dart:281-288`) — `MethodInvocation`, `FunctionExpressionInvocation`, `InstanceCreationExpression` (inclusive a implícita `C()` reescrita pelo `AstRewriter`), anotações; `ElementResolver._resolveArgumentsToFunction` (`analyzer/lib/src/generated/element_resolver.dart:440-458`) — `SuperConstructorInvocation` (com `enclosingConstructor`, :392-399; pulado se a superclasse do `extends` é tipo indefinido, :385-391) e `RedirectingConstructorInvocation`; e as constantes de enum com argumentos (`resolver.dart:2523-2531`). Só há relato quando o alvo resolveu a um tipo de função/elemento executável (`dynamic`, `Function`, membro indefinido: nada).

Algoritmo (`resolver.dart:4234-4352`):
1. parâmetros → `unnamedParameters` (obrigatórios + opcionais posicionais), `requiredParameterCount`, `namedParameters` (mapa por nome);
2. argumentos posicionais na ordem: conta `positionalArgumentCount`; o primeiro além dos posicionais declarados é `firstUnresolvedArgument`; guarda `lastPositionalArgument`; se algum é `SimpleIdentifier` vazio (recuperação do parser), `noBlankArguments = false`;
3. com `enclosingConstructor` (só `super(...)`): `verifySuperFormalParameters` (`analyzer/lib/src/error/super_formal_parameters_verifier.dart:11-37`) soma os `super.x` posicionais ao `positionalArgumentCount` e põe os nomes dos `super.x` nomeados em `usedNames` (e relata `POSITIONAL_SUPER_FORMAL_PARAMETER_WITH_POSITIONAL_ARGUMENT` se há posicionais explícitos);
4. argumentos nomeados, na ordem: nome desconhecido → `UNDEFINED_NAMED_PARAMETER` no rótulo; nome repetido em `usedNames` (inclusive vindo de `super.x`) → `DUPLICATE_NAMED_ARGUMENT` no rótulo;
5. se `positionalArgumentCount < requiredParameterCount && noBlankArguments` → `NOT_ENOUGH_POSITIONAL_ARGUMENTS_*`; senão, se `positionalArgumentCount > unnamedParameterCount && noBlankArguments` → `EXTRA_POSITIONAL_ARGUMENTS(_COULD_BE_NAMED)` **só se houver `firstUnresolvedArgument`** (excesso vindo só de `super.x` não relata).
O `MISSING_REQUIRED_ARGUMENT` é de outra fase (`RequiredParametersVerifier`, no `ErrorVerifier`).

Forma da invocação no analyzer (decide nome citado e entidade):
- `C(...)`, `p.C(...)`, `C<T>(...)`, `C.n(...)`, `new/const C…(...)` → `InstanceCreationExpression` (o `AstRewriter` reescreve as implícitas);
- `f(...)` com `f` função de topo, função local, método (implícito ou `x.m`, `p.f`, `C.m`, `super.m`) → `MethodInvocation`;
- `v(...)` com `v` variável/parâmetro/getter (inclusive de `this` implícito), `x.g(...)` com `g` getter/campo, `(e)(...)`, `e()(...)` → `FunctionExpressionInvocation` (`method_invocation_resolver.dart:575-600`, :856-858: `_rewriteAsFunctionExpressionInvocation`).

##### `extra_positional_arguments` (perda 64: FN 20, FP 23, pos 21)
- **Emissão:** `resolver.dart:4333-4352`.
- **Condição exata:** passo 5, e `namedParameters.length <= usedNames.length` (não sobra nomeado não usado; `usedNames` conta nomes distintos dos argumentos nomeados — inclusive os indefinidos — e os `super.x` nomeados).
- **Posição:** o **primeiro argumento posicional excedente** (`firstUnresolvedArgument`, a expressão inteira).
- **Mensagem:** `Too many positional arguments: {0} expected, but {1} found.` — `{0}` = número de parâmetros posicionais (obrigatórios + opcionais), `{1}` = posicionais passados (incluindo `super.x`).
- **Supressões e ordem:** com argumento "em branco" de recuperação, nada; tipo do alvo `dynamic`/`Function`/não resolvido: nada.
- **No DartForge:** `chamadas.rs::verificar_aridade` relatava texto genérico (`aviso` com molde e "esperava no máximo…") **na lista de argumentos inteira** — daí as 21 posições erradas (o oráculo aponta o excedente) e as 23 FP (os casos que são `_COULD_BE_NAMED` saíam como este código). As 20 FN: `constructor/redirect2_test` (`this(...)`), `regress35258`, `deferred/load_library_wrong_args` (`p.loadLibrary(1)`: tipo `Future<void> Function()`), `enumConstant` (`v(1)` em enum sem construtor que aceite), `field_initializer_outside_constructor…`, `parameter/named_aggregated_test`, `dot_shorthands/*` (3.10, sintaxe nova).
  **Mudança (já escrita no working tree, a validar):** `verificar_aridade` reescrita com o algoritmo acima, emitindo `aviso_com_codigo` com `[sem_nome, total_pos]` no primeiro excedente; `chamar_construtor_de` (`funcoes.rs`) passa a usar a assinatura **inteira** do super construtor com os `super.x` contados no alvo (`AlvoDaAridade.super_posicionais`/`super_nomeados`) em vez de removê-los (`sem_parametros_super` removida); faltam `loadLibrary` e enum sem construtor compatível.
- **Feito em 2026-10-06 (41/41; também `not_enough_positional_arguments` 39/39, `enum_constant_invokes_factory_constructor`, `recursive_constructor_redirect` sem FP):**
  - O construtor do atalho de ponto (`.new(1, 2)`, `.named(…)`, `const .x(…)`) passa pelo caminho comum da criação (`chamadas.rs::construir`: argumentos de tipo pelo contexto e `resolveArgumentsToParameters`), com o nome do construtor nos erros de aridade (`'new'`, `'named'`); o método estático `.m(…)` também leva o nome (`DotShorthandInvocation`).
  - Construtor repetido: o nome resolve para o **primeiro** declarado (`getNamedConstructor`), não para o último (`outline.rs`).
  - Função local e closure: o `this.x`/`super.x` obrigatório não entra nos parâmetros (`ResolutionVisitor` sem `ElementWalker` só o encerra, `resolution_visitor.dart:614-635`, `:1263`); o opcional entra pelo `DefaultFormalParameter` (`:440-467`).

##### `extra_positional_arguments_could_be_named` (perda 21: FN 21)
- **Quebra do `FfiVerifier` (2026-10-08):** `NativeCallable<Int32 Function(Int32)>.isolateLocal(f, 0)` — o
  `_validateNativeCallable` faz `(arguments[1] as NamedExpression)` (3.6.2; `as NamedArgument` no 3.13.4), e com o
  `exceptionalReturn` posicional o cast lança: a biblioteca inteira sai sem diagnósticos (`QuebraDoAnalyzer`,
  `fase_ffi.rs`), inclusive este (`ffi_async_callback/…_8bca9a2a.dart:4:57`).
- **Emissão/condição:** `resolver.dart:4335-4340`: passo 5 com `namedParameters.length > usedNames.length` (há nomeado declarado ainda não passado).
- **Posição/mensagem:** idem ao anterior (mesmo texto `Too many positional arguments: {0} expected, but {1} found.`; muda só o `correctionMessage`).
- **No DartForge:** não existia (saía como `extra_positional_arguments`, FP lá). **Mudança:** na mesma função (feito).

##### `not_enough_positional_arguments` (perda 41: FN 4, FP 2, pos 35)
- **Emissão:** `resolver.dart:4318-4331` → `_reportNotEnoughPositionalArguments` (:4371-4446); constantes de enum **sem** argumentos (`v;`) com construtor sem nome que exige posicionais: `resolver.dart:2531-2544` (só com `enhanced-enums`).
- **Condição exata:** passo 5 (`positionalArgumentCount` inclui `super.x`).
- **Posição:** um **token**: `lastPositionalArgument.endToken.next` (o token logo depois do último posicional — `,` ou `)`), ou, sem posicionais, `argumentList.leftParenthesis.next` (o primeiro token depois do `(`: `)` ou o rótulo do primeiro nomeado), ou o `)`. Enum sem argumentos: o nome da constante.
- **Mensagem (4 variantes):** `isPlural = requiredParameterCount > 1`; com nome: `NAME_PLURAL` `'{0} positional arguments expected by '{2}', but {1} found.'` / `NAME_SINGULAR` `'1 positional argument expected by '{0}', but 0 found.'`; sem nome: `PLURAL` `'{0} positional arguments expected, but {1} found.'` / `SINGULAR` `'1 positional argument expected, but 0 found.'`. O nome (`nameNode` = pai da lista de argumentos):
  - criação: `constructorName.name` (inclusive `'new'` em `C.new(...)`) ou `'<lexema do tipo>.new'` (sem prefixo nem argumentos: `'A.new'`);
  - `this(...)`/`this.n(...)`: o nome, ou `'<returnType.getDisplayString()>.new'` (com parâmetros de tipo: `'A<T>.new'`);
  - `super(...)`/`super.n(...)`: o nome, ou `'<tipo de retorno do construtor da superclasse substituído>.new'` (`'A<int>.new'`);
  - `MethodInvocation`: o nome do método;
  - `FunctionExpressionInvocation`: o identificador se a função é `SimpleIdentifier` (`v(...)`, `getter()` implícito), senão **sem nome** (`(f)()`, `x.g()`, `f()()`);
  - constante de enum: `declaredElement.type.getDisplayString()` (`'E'`, ou `'E<int>'` em enum genérico);
  - anotação: `@A()` → `'A.new'`; `@A.n()` → `'n'`; `@p.A()` → `'A'`; `@p.A.n()` → `'A'` (o `identifier` do nome prefixado, ignorando o construtor); `@A<int>.n()` → `'A.new'`.
- **No DartForge:** texto genérico na lista inteira (35 posições erradas, mensagens sem as variantes). FP: `AugmentationFormalParameterShape…` (augmentation) e `UndefinedMethod__localSetterShadowingEx…` (o analyzer não invoca: `noGetterIsPossible` → `UNDEFINED_METHOD`). FN: `enumConst_35efe1a3` (enum sem argumentos), `dotShorth_*` (3.10).
  **Mudança (escrita no working tree):** `verificar_aridade` calcula o token com `proximo_token` (pula espaços e comentários) e escolhe a variante; o nome vem do `AlvoDaAridade` definido por cada chamador (`chamadas.rs::chamada`/`instanciacao`, `funcoes.rs` para `super`/`this`, constantes de enum e anotações); enum sem argumentos em `chamadas.rs::aridade_sem_argumentos`. Falta o `localSetterShadowing` (é do `undefined_method`).

##### `missing_required_argument` (perda 12: FN 2, pos 10)
- **Emissão:** `RequiredParametersVerifier` (`analyzer/lib/src/error/required_parameters_verifier.dart:21-135`), chamado pelo `ErrorVerifier` (`error_verifier.dart:270`, e :1440-1444 para `super(...)` com `enclosingConstructor`).
- **Condição exata:** para cada parâmetro **nomeado obrigatório** (`required`) do elemento/tipo invocado, sem argumento nomeado com o mesmo nome, e (em `super(...)`) sem `super.x` nomeado com esse nome. (A variante com `@required` legada é `WarningCode.MISSING_REQUIRED_PARAM`.)
- **Posição (`errorEntity`):** `MethodInvocation` → o nome do método; `m.call(...)` com alvo de tipo função → a lista de argumentos; `FunctionExpressionInvocation` → a invocação inteira; `InstanceCreationExpression` → o `constructorName` (tipo com prefixo e argumentos de tipo + `.nome`); `this(...)`/`super(...)` → a invocação inteira (`super.n(...)` inclusive); constante de enum → o nome da constante (com ou sem argumentos); anotação → o identificador do construtor, ou o da classe (`@A()` → `A`; `@A.n()` → `n`; `@p.A()` → `A`).
- **Mensagem:** `The named parameter '{0}' is required, but there's no corresponding argument.`
- **No DartForge:** antes, na lista de argumentos (10 posições erradas). FN: constante de enum sem argumentos (`enumConstant_with_86449ed9`) e `constructor_field_ce0ca2ef` (nome vazio `''`: parâmetro `required` de campo privado/sintaxe nova). **Mudança (escrita):** entidade vinda do `AlvoDaAridade`; enum sem argumentos em `aridade_sem_argumentos`.

##### `duplicate_named_argument` (perda 7: FN 7)
- **Emissão:** `resolver.dart:4308-4315` (passo 4).
- **Condição:** nome de argumento nomeado já usado na mesma lista, ou já coberto por um `super.x` nomeado do construtor corrente (em `super(...)`).
- **Posição:** o rótulo do argumento repetido (a segunda ocorrência). **Mensagem:** `The argument for the named parameter '{0}' was already specified.`
- **No DartForge:** inexistente; e o `super.a` + `a:` explícito saía como `UNDEFINED_NAMED_PARAMETER` (FP) porque `sem_parametros_super` tirava `a` da assinatura. **Mudança (escrita):** em `verificar_aridade`.

##### `undefined_named_parameter` (perda 3: FP 3)
- **Emissão:** `resolver.dart:4297-4303` (passo 4). **Posição:** o rótulo. **Mensagem:** `The named parameter '{0}' isn't defined.`
- **No DartForge:** existe (32 acertos, publicado? não). FP: `DuplicateNamedArgument__constructor_sup…` (ver acima), `ExperimentalMemberUse__incorrectlyNeste…`, `UseOfPrivateParameterName__andVoidLhsError` (parâmetro nomeado privado `_x` de sintaxe nova). **Mudança:** o primeiro já resolvido; os outros dois dependem de sintaxe nova.

##### `positional_super_formal_parameter_with_positional_argument` (perda 2: FN 2)
- **Emissão:** `verifySuperFormalParameters` (`super_formal_parameters_verifier.dart:22-33`), chamado com `errorReporter` em `resolveArgumentsToParameters` (super explícito, `hasExplicitPositionalArguments: positionalArgumentCount != 0`).
- **Condição:** o `super(...)` explícito tem argumento posicional **e** o construtor tem `super.x` posicional. **Posição:** o nome de **cada** `super.x` posicional. **Mensagem:** `Positional super parameters can't be used when the super constructor invocation has a positional argument.`
- **No DartForge:** inexistente. **Mudança:** em `funcoes.rs::chamar_construtor_de` (onde o alvo já conta os `super.x`), ou em `crates/analise/src/construtores.rs`.

##### `super_formal_parameter_without_associated_positional` (perda 12: FN 12)
- **Emissão:** `ErrorVerifier.visitSuperFormalParameter` (`analyzer/lib/src/generated/error_verifier.dart:1456-1493`).
- **Condição:** (não em extension type; construtor gerador não redirecionador) `superConstructorParameter == null` (`analyzer/lib/src/dart/element/element.dart:9291-9311`): o construtor da superclasse **invocado** (`superConstructor`: o do `super.n(...)` explícito, ou o sem nome implícito) existe, e o índice deste `super.x` **entre os `super.x` do construtor** (`indexIn`, :9318-...) não é menor que o número de parâmetros posicionais (obrigatórios + opcionais) daquele construtor — os posicionais explícitos do `super(...)` **não** deslocam o índice. Sem `superConstructor` (superclasse sem o construtor) não relata.
- **Posição:** o nome do parâmetro. **Mensagem:** `No associated positional super constructor parameter.`
- **No DartForge:** existe em `crates/analise/src/construtores.rs:533-551`, mas o placar mostra **nosso 0** — o bloco não é alcançado (ver `implicit_super_initializer_missing_arguments`). FN `primary_constructors/cycle_error_test` são sintaxe nova.
- **Estado em 2026-10-07 (placar 12/12):** três regras em `funcoes::parametros_super`.
  * **Sem `superConstructor`:** no 3.13.4 (`error_verifier.dart:2391-2408` do checkout main) a verificação para;
    no 3.6.2, cada `super.x` relata. Exemplo: `this : super.named()` com `named` indefinido.
  * **Enum ou ciclo de herança:** a superclasse em ciclo é cortada para `Object`, e o enum também não tem
    posicionais no construtor associado. Nas duas referências, cada `super.x` posicional relata. O ciclo é
    detectado pela cadeia de superclasses (`em_ciclo_de_superclasse`).
  * **Nomes repetidos:** o índice do posicional é o `indexIn` do elemento (`element.dart:9318-9323`). No 3.6.2,
    dois `super._` homônimos são o mesmo elemento pela localização, e o segundo fica com o índice do primeiro,
    sem relato.

##### `super_formal_parameter_without_associated_named` (perda 6: FN 6)
- **Emissão/condição:** a mesma (`error_verifier.dart:1484-1488`): `super.x` nomeado sem parâmetro nomeado `x` no construtor da superclasse. **Posição:** o nome. **Mensagem:** `No associated named super constructor parameter.`
- **No DartForge:** idem (`construtores.rs:543-546`, nosso 0).

##### `implicit_super_initializer_missing_arguments` (perda 22: FN 22)
- **Emissão:** `ErrorVerifier._checkForUndefinedConstructorInInitializerImplicit` (`analyzer/lib/src/generated/error_verifier.dart:5444-5552`), de `visitConstructorDeclaration` (:606).
- **Condição exata:** construtor gerador, não `external`, sem inicializador `super(...)`/`this(...)`; superclasse com algum construtor gerador; o construtor sem nome da superclasse existe e não é factory; com `super-parameters` habilitado: `requiredPositional(super ctor) > #super.x posicionais` ou algum nomeado obrigatório do super ctor não coberto pelos `super.x` nomeados.
- **Posição:** do `returnType` (nome da classe no construtor) ao fim do nome do construtor (`B.named` → `B.named`; `B` → `B`). Para construtor **primário** (sintaxe nova, 3.13) o oráculo aponta o nome da classe no cabeçalho.
- **Mensagem:** `The implicitly invoked unnamed constructor from '{0}' has required parameters.` — `{0}` = `superType` (DartType: `'A'`, `'A<int>'`).
- **Supressões e ordem:** sem `super-parameters` → `NO_DEFAULT_SUPER_CONSTRUCTOR_EXPLICIT`; sem construtor sem nome → `UNDEFINED_CONSTRUCTOR_IN_INITIALIZER_DEFAULT`; factory → `NON_GENERATIVE_CONSTRUCTOR`. Classe **sem construtores** usa outro caminho (`_checkForNoDefaultSuperConstructorImplicit`, :4634-4678: `NO_DEFAULT_SUPER_CONSTRUCTOR_IMPLICIT`).
- **No DartForge:** `crates/analise/src/construtores.rs:552-588` implementa a regra, mas **nosso 0** no placar: o trecho não é alcançado ou o diagnóstico não chega à paridade (investigar `cx.superclasse`/`cx.construtor`/`cx.assinatura` e se o passe de construtores roda para classes comuns). As 22 FN incluem 13 de `no_default_super_constructor/*` simples (`B.named();` com `A({required int? a})`). `{0}` usa só o nome da classe (`nome_sup`): deveria ser o tipo com argumentos.

##### `new_with_undefined_constructor_default` (perda 26: FN 26)
- **Emissão:** `ErrorVerifier._checkForNewWithUndefinedConstructor` (`error_verifier.dart:4596-4626`), de `visitInstanceCreationExpression` (não const, :1117-1119); e `C.new(...)` em `method_invocation_resolver.dart:917-929`.
- **Condição exata:** criação (explícita `new C()` ou implícita `C()`) cujo construtor não resolveu; o tipo não é enum nem mixin (já relatado); sem nome de construtor → este código.
- **Posição:** o `constructorName` (para `new C()`/`C()`: o tipo nomeado com prefixo e argumentos de tipo; `C.new()` implícito: o `new`). **Mensagem:** `The class '{0}' doesn't have an unnamed constructor.` — `{0}` = `namedType.qualifiedName` (`'p.C'` com prefixo; o nome do **alias** quando via typedef: `'T'`); no caminho `C.new`, `receiver.displayName`.
- **No DartForge:** só `C.new` (`expr.rs:1681-1683`). FN: `new A()`/`A()` sem construtor sem nome (`call/nonexistent_constructor_error_test`, `constructor/unresolved_default_constructor_test`), **aplicação de mixin** sem construtor encaminhado (`mixin_constructor_forwarding/*`, 8: só construtores geradores da superclasse são encaminhados, e com parâmetros opcionais não… regra de encaminhamento), **alias de `FutureOr`/`Null`** (`T()` com `typedef T = FutureOr<…>`/`Null`, 16: o elemento aliasado não tem construtor).
  **Mudança:** em `chamadas.rs::instanciacao`, no `let Some(f) = f else` (construtor não achado e classe com construtores declarados ou alias de classe sem construtor), relatar; em `chamada` (criação implícita via `referencia_a_tipo`) idem. **Código publicado** (`verificados.txt`): cuidado com FP.
- **Estado (2026-10-06):** 28 de 28. Pontos que faltavam: (1) a criação sem `new` (`C()`, `T<X>()` por alias, também
  de `Null`/`FutureOr`) sem construtor sem nome — no tipo nomeado da reescrita, com o nome escrito; (2) aplicação de
  mixin: os construtores são só os **geradores** encaminhados da superclasse (`Base()` factory não conta), e o sem
  nome implícito só existe se a superclasse (recursivamente) não declara construtor; (3) `A.new()` declarado numa
  versão sem `constructor-tearoffs` é o sem nome.

##### `new_with_undefined_constructor` (perda 14: FN 14)
- **Emissão:** a mesma função (`error_verifier.dart:4612-4618`) com nome de construtor. **Posição:** o nome do construtor. **Mensagem:** `The class '{0}' doesn't have a constructor named '{1}'.` — `{0}` `qualifiedName` (`'private.Class'` com prefixo), `{1}` o nome.
- **Condição extra:** a forma implícita `C.n()` sem construtor `n` **não** é reescrita para criação (o `AstRewriter` só reescreve se o construtor existe) — sai `UNDEFINED_METHOD`; logo este código vem de `new`/`const`, de `C<T>.n()` (sempre criação) e de `p.C.n()`.
- **No DartForge:** inexistente fora de `C<T>.new`. FN: `new A.bar()`, `mixin/illegal_constructor_test` (`new C0.named()` em aplicação de mixin), `with_two_implicit_constructors`, aliases, construtor privado de outra biblioteca (`new Class._constructor()`). **Mudança:** idem em `instanciacao`. **Publicado.**

##### `instantiate_abstract_class` (perda 4: FN 4)
- **Emissão:** `error_verifier.dart:2950-2973`. **Condição:** `ClassElement` abstrata, construtor resolvido não factory. **Posição:** o `NamedType`. **Mensagem:** `Abstract classes can't be instantiated.`
- **No DartForge:** existe (29). As 4 FN são `dot_shorthands/*` (3.10, sintaxe nova) — fora do 3.6.2.
- **Estado em 2026-10-07 (placar 33/33):** os 4 FN eram de atalho de ponto (3.13). Agora
  `atalhos::construcao` segue o `resolveDotShorthand` (`constructor_invocation_resolver.dart:96-117` do
  checkout main).
  * Classe abstrata com construtor que não é fábrica dá o erro no nó inteiro (`.new()`, `.new<T>()`).
  * Vale também o construtor implícito: `.new()` de uma classe abstrata sem construtor declarado, como
    `Function`.

##### `mixin_instantiate` (perda 2: FN 2)
- **Emissão:** `error_verifier.dart:2976-2984`. **Condição:** criação cujo tipo é `MixinElement` — `new M()`/`const M()`, e também a implícita `M()`/`M.n()` (o `AstRewriter` reescreve qualquer `InterfaceElement`, `analyzer/lib/src/dart/resolver/ast_rewrite.dart:139`, :168, :228). O construtor indefinido do mixin fica mudo (`_checkForNewWithUndefinedConstructor` pula mixin). **Posição:** o `NamedType`. **Mensagem:** `Mixins can't be instantiated.`
- **No DartForge:** inexistente; `chamadas.rs::instanciacao` (classe de tipo mixin) antes do construtor indefinido (que fica mudo para mixin).

##### `no_annotation_constructor_arguments` (perda 9: FN 9)
- **Emissão:** `ConstantVerifier.visitAnnotation` (`analyzer/lib/src/dart/constant/constant_verifier.dart:103-128`).
- **Condição:** a anotação resolve a um **construtor** `const` e não tem lista de argumentos (`@A` com `A` classe, `@A.named` sem `()`, `@Native` sem `()`). Construtor não const → `NON_CONSTANT_ANNOTATION_CONSTRUCTOR` antes.
- **Posição:** a anotação inteira (do `@` ao fim do nome). **Mensagem:** `Annotation creation must have arguments.`
- **No DartForge:** inexistente; `funcoes.rs::validar_anotacao` (quando o nome resolve a classe/construtor e `m.arguments` é `None`).

### Rodada 4 — família A

Complementos da rodada 4 (2026-10-04) para os códigos da família A que a auditoria (§0.A) achou sem algum dos seis campos; cada bloco completa ou corrige a seção do código acima. Os lotes 1 e 2 foram escritos à mão, com exemplos do oráculo vivo; o lote 3 traz, para os códigos restantes, os emissores e os desvios do método emissor copiados da fonte por script.

### A.R4-1 Complementos da família A (rodada 4, lote 1)

Completa os códigos da família A que a auditoria da rodada 4 achou sem algum dos seis campos
(lista `E:\dftemp\analise\spec-r4\tail-A-00.txt`). Fonte: 3.6.2 (`analyzer/lib/src/...`). Exemplos
rodados no oráculo vivo (`dart analyze --format=json` do 3.6.2), casos em
`corpus\especificacao\a1\`; cada linha é `código | offset, length | linha:coluna | mensagem`.

##### `type_argument_not_matching_bounds` — complemento (perda 99: FN 97, FP 2)
- **Supressões e ordem** (tipo nomeado escrito: `TypeArgumentsVerifier._checkForTypeArgumentNotMatchingBounds`,
  `analyzer/lib/src/error/type_arguments_verifier.dart:268-400`):
  1. `namedType.type == null` → nada (`:269-272`); o tipo não é alias nem `InterfaceType` → nada (`:286-289`);
     sem parâmetros de tipo → nada (`:291-293`).
  2. Argumento que é tipo de função genérico sem `generic-metadata` → `GENERIC_FUNCTION_TYPE_CANNOT_BE_TYPE_ARGUMENT`
     e `continue` (o limite desse argumento não é checado) (`:302-310`).
  3. Coleta as faltas regular-bounded (`!isSubtypeOf(arg, limite substituído)`, `:312-325`); nenhuma → fim (`:328-330`).
  4. **Contexto que não aceita super-bounded** (`_shouldAllowSuperBoundedTypes`, `:604-621`: pai
     `ClassTypeAlias`, `ConstructorName`, `ExtendsClause`, `GenericTypeAlias`, `ImplementsClause`,
     `MixinOnClause`, `WithClause`; ou o tipo é extension type) → um relato **por falta**, e `return` (`:369-383`).
  5. Senão troca topo↔fundo (`replaceTopAndBottom`, `:386`) e só relata os argumentos **invertidos** que
     ainda violam o limite (`:400-…`), com as mensagens de contexto "The inverted type … is also not
     regular-bounded". Daí: `A<dynamic>`, `A<Object?>` em anotação de tipo **não** relatam (super-bounded),
     mas `A<dynamic>()` em criação relata (exemplo, linhas 6–7 × 13).
  - Contagem errada de argumentos: `NamedTypeResolver._buildTypeArguments` devolve `InvalidType` para
    todos (`named_type_resolver.dart:137-147`) e sai só `WRONG_NUMBER_OF_TYPE_ARGUMENTS` (linha 18).
  - Argumento não resolvido: sai `NON_TYPE_AS_TYPE_ARGUMENT` e o argumento vira `InvalidType`, subtipo de
    tudo — sem este código (linha 19). O `ResolutionVisitor.visitNamedType` visita `typeArguments` **antes**
    de resolver o nome, seja ele o que for: em `class C7<A> extends A<Unresolved>` (o nome é parâmetro de
    tipo), `dynamic<X>`, nome indefinido ou que não é tipo, os argumentos são resolvidos e relatam os
    próprios erros. No DartForge: `resolve_annotation` resolve os argumentos que o caminho do nome não
    resolveu (os que não estão em `tipos_escritos`), em contexto `ArgumentoDeTipo`.
  - Tipo cru (`B` sem argumentos) em anotação: instanciado aos limites; se a instanciação não é
    regular-bounded mas é super-bounded, nada (linha 9); em `new B()` os argumentos vêm da inferência e
    saem **os dois**: este código (no nome do tipo, por não haver nó de argumento — `_typeArgumentErrorNode`
    devolve o `NamedType`, `:624-630`) e `COULD_NOT_INFER` no mesmo intervalo (linha 14).
  - Invocações com argumentos explícitos (`_checkInvocationTypeArguments`, `:404-…`): só se
    `typeArgumentList != null` e os tipos genérico/instanciado são `FunctionType`; a contagem é
    truncada ao mínimo (`:421-423`: "the mismatch in size is reported elsewhere"); **sem** desconto de
    super-bounded (`g<dynamic>()` relata — linha 17). O genérico é o tipo estático de `node.function`: no
    `call` implícito (`c<String>()` com `c` de uma classe com `call<T extends num>`) ele é o tipo de
    interface e nada é conferido. No DartForge, `BodyInferrer::call_implicito` antes do `invocar`.
- **Confere com §A:** emissão, condição, posição e mensagem certas (`{0}` e `{2}` DartType, `{1}` nome).
- **Exemplo (oráculo vivo 3.6.2):**
  ```dart
  class A<T extends num> {}
  class B<T extends B<T>> {}
  typedef F<T extends num> = void Function(T);
  class C extends A<String> {}
  A<String>? a1;
  A<dynamic>? a2;
  A<Object?>? a3;
  A<Never>? a4;
  B? b1;
  B<B>? b2;
  B<Object?>? b3;
  var n1 = new A<String>();
  var n2 = A<dynamic>();
  var n3 = new B();
  F<String>? f1;
  void g<T extends num>() {}
  void h() { g<String>(); g<dynamic>(); var l = <A<String>>[]; print(l); print(A<String>); }
  A<String, int>? w1;
  A<Undefined>? w2;
  ```
  - `type_argument_not_matching_bounds` | offset 116, length 6 | 4:19 | 'String' doesn't conform to the bound 'num' of the type parameter 'T'.
  - `type_argument_not_matching_bounds` | offset 129, length 6 | 5:3 | (idem)
  - `type_argument_not_matching_bounds` | offset 236, length 6 | 12:16 | (idem)
  - `type_argument_not_matching_bounds` | offset 258, length 7 | 13:12 | 'dynamic' doesn't conform to the bound 'num' of the type parameter 'T'.
  - `type_argument_not_matching_bounds` | offset 283, length 1 | 14:14 | 'B<Object?>' doesn't conform to the bound 'B<B<Object?>>' of the type parameter 'T'.
  - `could_not_infer` | offset 283, length 1 | 14:14 | Couldn't infer type parameter 'T'.⏎⏎Tried to infer 'B<Object?>' for 'T' which doesn't work:⏎  Type parameter 'T' is declared to extend 'B<T>' producing 'B<B<Object?>>'.⏎⏎Consider passing explicit type argument(s) to the generic.⏎⏎ (⏎ = quebra de linha)
  - `type_argument_not_matching_bounds` | offset 290, length 6 | 15:3 | 'String' … 'num' … 'T'.
  - `type_argument_not_matching_bounds` | offset 343, length 6 | 17:14 | (idem, `g<String>()`)
  - `type_argument_not_matching_bounds` | offset 356, length 7 | 17:27 | 'dynamic' … (`g<dynamic>()`)
  - `type_argument_not_matching_bounds` | offset 379, length 6 | 17:50 | (`<A<String>>[]`)
  - `type_argument_not_matching_bounds` | offset 409, length 6 | 17:80 | (`print(A<String>)`, literal de tipo)
  - `wrong_number_of_type_arguments` | offset 421, length 15 | 18:1 | The type 'A' is declared with 1 type parameters, but 2 type arguments were given.
  - `non_type_as_type_argument` | offset 443, length 9 | 19:3 | The name 'Undefined' isn't a type, so it can't be used as a type argument.

  (Sem relato: linhas 6, 7, 8 (`A<dynamic>`, `A<Object?>`, `A<Never>` em anotação), 9, 10, 11.)

##### `could_not_infer` — complemento (perda 45: FN 26, msg 16, pos 3)
- **Supressões e ordem** (`GenericInferrer.chooseFinalTypes`, `analyzer/lib/src/dart/element/generic_inferrer.dart`):
  - `failAtError` (inferência tentativa) → devolve `null` sem relatar (`:288-291`, `:312-315`, `:354-357`);
  - `errorReporter == null` → não relata (o relator é opcional; `:293`, `:311`, `:363`);
  - as três variantes são relatadas **por parâmetro de tipo**, na ordem do laço: restrições não
    satisfeitas (`:287-300`), candidato tipo de função genérico sem `generic-metadata` (`:308-328`),
    limite recursivo (`hasError[i]`, `:353-372`); cada uma marca `hasErrorReported`, e a checagem final
    dos limites (variante 4 de §A) só roda sem erro anterior (texto de §A; linha não conferida nesta rodada);
  - o mesmo ponto pode sair junto de `type_argument_not_matching_bounds` (exemplo acima, linha 14: os
    dois no nome do tipo em `new B()`).
- **Confere com §A:** posição (`errorEntity`) e mensagem certas; o texto de `{1}` começa e termina com
  quebras de linha (ver exemplo acima) — a comparação do placar é com o texto inteiro.
- **No DartForge:** ver §A (sem mudança nesta rodada).

##### `not_enough_positional_arguments` — complemento (perda 41: FN 4, FP 2, pos 35)
- **Supressões e ordem** (`ResolverVisitor.resolveArgumentsToParameters`, `analyzer/lib/src/generated/resolver.dart`):
  - condição `positionalArgumentCount < requiredParameterCount && noBlankArguments` (`:4319`):
    `noBlankArguments` é falso quando há argumento sintético vazio da recuperação do parser — aí nem este
    código nem `extra_positional_arguments` saem;
  - é exclusivo com `extra_positional_arguments` (`else if`, `:4331`);
  - sai **depois** dos relatos por argumento nomeado do laço anterior (`undefined_named_parameter`,
    `:4292-4307`), mas o JSON ordena por offset; os dois podem cair no mesmo offset (exemplo, linhas 8 e 10:
    o token do relato é o rótulo do 1º nomeado);
  - alvo não resolvido (`undefinedFn()`) ou `dynamic` (`d()`): não há lista de parâmetros → nada (linhas 16, 18);
  - `errorReporter == null` ou `argumentList.parent == null` → nada (`:4321`).
- **Confere com §A:** token (o seguinte ao último posicional; sem posicionais, o seguinte ao `(`) e as 4
  variantes de mensagem conferem com o oráculo vivo (abaixo).
- **Exemplo (oráculo vivo 3.6.2):**
  ```dart
  class A { A(int a, int b); A.n(int a); }
  void f(int a, int b) {}
  void g(int a) {}
  void h(void Function(int) cb, dynamic d) {
    f();
    f(1);
    f(1, );
    f(b: 2);
    g();
    g(x: 1);
    new A();
    new A(1);
    A.n();
    cb();
    (cb)();
    d();
    f(1, 2, 3);
    undefinedFn();
  }
  class B extends A { B() : super(1); B.k() : super.n(); }
  ```
  - `not_enough_positional_arguments` | offset 129, length 1 | 5:5 | 2 positional arguments expected by 'f', but 0 found.
  - `not_enough_positional_arguments` | offset 137, length 1 | 6:6 | 2 positional arguments expected by 'f', but 1 found.
  - `not_enough_positional_arguments` | offset 145, length 1 | 7:6 | (idem; o token é a `,` final)
  - `not_enough_positional_arguments` | offset 154, length 1 | 8:5 | 2 positional arguments expected by 'f', but 0 found.
  - `undefined_named_parameter` | offset 154, length 1 | 8:5 | The named parameter 'b' isn't defined.
  - `not_enough_positional_arguments` | offset 165, length 1 | 9:5 | 1 positional argument expected by 'g', but 0 found.
  - `not_enough_positional_arguments` | offset 172, length 1 | 10:5 | (idem) + `undefined_named_parameter` 'x' no mesmo offset
  - `not_enough_positional_arguments` | offset 187, length 1 | 11:9 | 2 positional arguments expected by 'A.new', but 0 found.
  - `not_enough_positional_arguments` | offset 199, length 1 | 12:10 | 2 positional arguments expected by 'A.new', but 1 found.
  - `not_enough_positional_arguments` | offset 208, length 1 | 13:7 | 1 positional argument expected by 'n', but 0 found.
  - `not_enough_positional_arguments` | offset 216, length 1 | 14:6 | 1 positional argument expected by 'cb', but 0 found.
  - `not_enough_positional_arguments` | offset 226, length 1 | 15:8 | 1 positional argument expected, but 0 found.
  - `extra_positional_arguments` | offset 246, length 1 | 17:11 | Too many positional arguments: 2 expected, but 3 found.
  - `undefined_function` | offset 252, length 11 | 18:3 | The function 'undefinedFn' isn't defined.
  - `not_enough_positional_arguments` | offset 302, length 1 | 20:34 | 2 positional arguments expected by 'A.new', but 1 found.
  - `not_enough_positional_arguments` | offset 321, length 1 | 20:53 | 1 positional argument expected by 'n', but 0 found.

##### `wrong_number_of_type_arguments` — complemento (perda 32: FN 31, FP 1)
- **Condição exata:** `NamedTypeResolver._buildTypeArguments(node, argumentList, parameterCount)`
  (`analyzer/lib/src/dart/resolver/named_type_resolver.dart:134-147`): `argumentCount != parameterCount`
  → relato no `NamedType` inteiro com `[node.name2.lexeme, parameterCount, argumentCount]` e **todos** os
  argumentos do tipo viram `InvalidType` (`List.filled(parameterCount, InvalidTypeImpl.instance)`).
  Literal de tipo/tear-off com `<…>` (`FunctionReferenceResolver._resolveDirectTypeLiteral`,
  `function_reference_resolver.dart:316-327`, e `_resolveTypeAlias`, `:821-835`): via
  `_checkTypeArguments(typeArguments, nome, typeParameters, WRONG_NUMBER_OF_TYPE_ARGUMENTS)`.
- **Supressões e ordem:** os argumentos de tipo são visitados **antes** do nome (ver §F,
  `undefined_class`): erros dentro deles saem de qualquer forma. Depois deste código não há
  `type_argument_not_matching_bounds` no mesmo tipo (argumentos `InvalidType`). Nome que não resolve →
  `undefined_class`, sem este código. Tipo sem parâmetros com argumentos (`int<String>`) é este mesmo
  código com `{1}` = 0.
- **Exemplo:** `A<String, int>? w1;` → `wrong_number_of_type_arguments` | offset 421, length 15 | 18:1 |
  The type 'A' is declared with 1 type parameters, but 2 type arguments were given. (exemplo de
  `type_argument_not_matching_bounds` acima.)

### A.R4-2 Complementos da família A (rodada 4, lote 2)

Completa os códigos da família A que a auditoria da rodada 4 achou sem algum dos seis campos
(lista `E:\dftemp\analise\spec-r4\tail-A-01.txt`). Fonte: 3.6.2 (`analyzer/lib/src/...`). Os exemplos
foram rodados no oráculo vivo (`C:\tools\dartsdk-3.6.2\bin\dart analyze --format=json`), casos em
`corpus\especificacao\a2\`; cada linha de saída é
`código | offset, length | linha:coluna | mensagem`. "Hoje" no DartForge = o que está no disco
(há trabalho não commitado em `crates/types`).

##### `doc_directive_has_extra_arguments` — complemento (perda 4: FN 4)
- **Emissão:** duas fontes. (1) `DocCommentVerifier.validateArgumentCount`
  (`analyzer/lib/src/error/doc_comment_verifier.dart:52-108`, relato em `:99-108`), chamado por
  `docDirectiveTag` (`:29-32`), por sua vez chamado do `BestPracticesVerifier` ao visitar o comentário
  (fase de avisos). (2) `DocCommentBuilder` (parser de comentários, fase de parse), para diretivas **de
  fecho** (`{@end-…}`) que trazem texto depois do nome: `analyzer/lib/src/fasta/doc_comment_builder.dart:1222-1242`.
- **Condição exata:** (1) `!tag.type.restParametersAllowed && positionalArgumentCount > requiredCount`
  (`:93-99`; `category`/`subCategory` aceitam resto e saem antes, `:93-97`). (2) depois do nome da
  diretiva de fecho ainda há caracteres antes do `}`.
- **Posição:** (1) do início do 1º posicional excedente (`tag.positionalArguments[requiredCount].offset`)
  ao fim do último posicional (`:100-101`). (2) do 1º caractere depois do nome+espaço até antes do `}`
  (`extraArgumentsOffset`, `:1222`, `:1237`).
- **Mensagem:** `The '{0}' directive has '{1}' arguments, but only '{2}' are expected.`
  (`messages.yaml:23489`); (1) `{0}` = `tag.type.name`, `{1}` = nº de posicionais, `{2}` = nº exigido
  (inteiros). (2) é emitido **sem argumentos** (`:1238-1242`) e o texto sai com os valores do verificador
  (1) na prática — ver exemplo, linha 7 (um relato só, com `'1'`/`'0'`): **não verificado** qual das duas
  fontes produziu esse relato.
- **Supressões e ordem:** antes, no mesmo `validateArgumentCount`, a falta de argumentos
  (`DOC_DIRECTIVE_MISSING_ONE/TWO/THREE_ARGUMENT(S)`, `:57-91`) — exclusiva com o excesso. Argumentos
  nomeados não contam como posicionais (seguem para `DOC_DIRECTIVE_HAS_UNEXPECTED_NAMED_ARGUMENT`,
  `:110-119`); os dois códigos podem sair na mesma diretiva. Depois, `validateArgumentFormat`
  (`DOC_DIRECTIVE_ARGUMENT_WRONG_FORMAT`) roda sempre (`:31`).
- **No DartForge:** inexistente (nenhum uso da constante em `crates/`); o léxico guarda os comentários
  de documentação, mas não há leitura de diretivas `{@…}`. Os 4 FN (`DocDirectiveHasExtraArguments__*`)
  são a regra inteira. Mudança: um leitor de diretivas em `crates/analise` (tipos e aridades de
  `analyzer/lib/src/dart/ast/doc_comment.dart`, `DocDirectiveType`), com os três relatos de aridade.
- **Exemplo (oráculo vivo 3.6.2):**
  ```dart
  /// {@youtube 600 400 https://www.youtube.com/watch?v=x extra}
  /// {@macro name extra}
  /// {@category a b c}
  /// {@animation 600 400 http://x.mp4 foo=bar}
  /// {@macro name foo=bar}
  /// {@animation 600 400 http://x.mp4 id=a}
  /// {@end-inject-html extra}
  class A {}
  ```
  - `doc_directive_has_extra_arguments` | offset 56, length 5 | 1:57 | The 'youtube' directive has '4' arguments, but only '3' are expected.
  - `doc_directive_has_extra_arguments` | offset 80, length 5 | 2:18 | The 'macro' directive has '2' arguments, but only '1' are expected.
  - `doc_directive_has_unexpected_named_argument` | offset 146, length 7 | 4:38 | The 'animation' directive has an unexpected named argument, 'foo'.
  - `doc_directive_has_unexpected_named_argument` | offset 172, length 7 | 5:18 | The 'macro' directive has an unexpected named argument, 'foo'.
  - `doc_directive_missing_opening_tag` | offset 228, length 25 | 7:5 | Doc directive is missing an opening tag.
  - `doc_directive_has_extra_arguments` | offset 246, length 5 | 7:23 | The 'end-inject-html' directive has '1' arguments, but only '0' are expected.

  (`{@category a b c}` não relata: resto permitido. `id=a` em `animation` é nomeado declarado: não relata.)

##### `doc_directive_has_unexpected_named_argument` — complemento (perda 2: FN 2)
- **Emissão:** `DocCommentVerifier.validateArgumentCount`, laço final
  (`analyzer/lib/src/error/doc_comment_verifier.dart:110-119`); mesma cadeia do código anterior.
- **Condição exata:** para cada `tag.namedArguments`: `!tag.type.namedParameters.containsNamed(arg.name)`.
  Só é alcançado se o tipo **não** aceita resto (o `return` de `:93-97` vem antes).
- **Posição:** o argumento nomeado inteiro, `nome=valor` (`namedArgument.offset` a `.end`, `:113-114`).
- **Mensagem:** `The '{0}' directive has an unexpected named argument, '{1}'.` (`messages.yaml:23498`);
  `{0}` = nome da diretiva, `{1}` = nome do argumento.
- **Supressões e ordem:** sai depois do relato de aridade da mesma diretiva (os dois podem sair); um
  relato por argumento nomeado desconhecido.
- **No DartForge:** inexistente (ver código anterior). 2 FN: `DocDirectiveHasUnexpectedNamedArgument__{915ee163,a278ded0}.dart`.
- **Exemplo:** linhas 4 e 5 do exemplo anterior.

##### `extension_override_argument_not_assignable` — complemento (perda 4: FN 4)
- **Supressões e ordem** (o que faltava; `ExtensionMemberResolver.resolveOverride`,
  `analyzer/lib/src/dart/resolver/extension_member_resolver.dart:171-244`), nesta ordem:
  1. contexto inválido (`!_isValidContext(node)` e não é alvo de cascata) → `EXTENSION_OVERRIDE_WITHOUT_ACCESS`
     no nó, e **continua** (`:177-185`);
  2. `arguments.length != 1` → `INVALID_EXTENSION_ARGUMENT_COUNT` na lista de argumentos e **`return`**
     (`:187-196`): nenhuma checagem de tipo do argumento;
  3. `node.isNullAware` (`E(x)?.m()`): o tipo do receptor é promovido a não nulo antes da comparação
     (`:201-206`) — `E(n)?.m()` com `n` anulável **não** relata;
  4. inferência dos argumentos de tipo e `_checkTypeArgumentsMatchingBounds` (`:207-225`): com limite
     violado sai `COULD_NOT_INFER`/`TYPE_ARGUMENT_NOT_MATCHING_BOUNDS` e, como o tipo estendido passa a
     usar o tipo corrigido, este código pode não sair (exemplo, linha 11);
  5. receptor `void` → `USE_OF_VOID_RESULT` no receptor, **no lugar** deste código (`:227-231`);
  6. `!isAssignableTo(receiverType, extendedType)` → este código, com as mensagens de contexto de
     "por que não promoveu" (`:232-243`). `dynamic` é atribuível: não relata.
- **Confere com §A:** emissão, posição (a expressão do argumento, sem `getErrorNode`) e mensagem
  (`{0}`,`{1}` DartType → display com alias) estão certas.
- **No DartForge:** corrige §A ("infere o argumento e não checa"): no disco há emissão em
  `crates/types/src/inferencia/chamadas.rs:770` (trabalho não commitado; não medido no placar). Os 4 FN
  do `placar-r7` (`ExtensionOverrideArgumentNotAssignable__{0883e8ab,20a9eedd,a44c913c}.dart`,
  `why_not_promoted/argument_type_not_assignable_nullability_error_test.dart:609:7`) são anteriores a ela.
- **Exemplo (oráculo vivo 3.6.2):**
  ```dart
  class A {}
  class B {}
  extension E on A { void m() {} }
  extension G<T extends num> on List<T> { void g() {} }
  void f(B b, void v, dynamic d, A? n, List<String> ls) {
    E(b).m();
    E(v).m();
    E(d).m();
    E(n).m();
    E(n)?.m();
    G(ls).g();
    E(b, b).m();
  }
  ```
  - `extension_override_argument_not_assignable` | offset 169, length 1 | 6:5 | The type of the argument to the extension override 'B' isn't assignable to the extended type 'A'.
  - `use_of_void_result` | offset 181, length 1 | 7:5 | This expression has a type of 'void' so its value can't be used.
  - `extension_override_argument_not_assignable` | offset 205, length 1 | 9:5 | The type of the argument to the extension override 'A?' isn't assignable to the extended type 'A'.
  - `could_not_infer` | offset 228, length 1 | 11:3 | Couldn't infer type parameter 'T'. (…)
  - `invalid_extension_argument_count` | offset 242, length 6 | 12:4 | Extension overrides must have exactly one argument: the value of 'this' in the extension method.

##### `extension_type_inherited_member_conflict` — complemento (perda 4: FN 4)
- **Supressões e ordem:** `_checkForExtensionTypeMemberConflicts`
  (`analyzer/lib/src/generated/error_verifier.dart:3450-3488`) percorre `interface.conflicts` de
  `InheritanceManager3.getInterface(element)` e relata **um diagnóstico por conflito** (por nome de
  membro), todos no nome do extension type, com mensagens de contexto `Inherited from '<classe>'` por
  candidato (`:3455-3465`). Três espécies de conflito, todas com o mesmo código (`:3476-3486`):
  `CandidatesConflict`, `HasNonExtensionAndExtensionMemberConflict`, `NotUniqueExtensionMemberConflict`.
  Não sai quando o extension type **redeclara** o membro (a declaração local tira o nome dos
  conflitos — exemplo, `D`). Não há supressão por erro anterior.
- **Classificação dos candidatos (`_getInterfaceExtensionType`, `inheritance_manager3.dart:784-797`):** é o
  bit `isExtensionTypeMember` do elemento, posto só nos membros **declarados** num extension type
  (`element_builder.dart:582`). O membro sintético do `_topMerge` (`combineSignatures(doTopMerge: true)`,
  dono = o extension type que combinou) nasce sem o bit e conta como **não-extensão**: em
  `extension type F<T>(C<T> c) implements A<T>, B<T> {}` e `extension type G<T>(E<T> e) implements F<T>, D<T> {}`,
  o `method` combinado de `A`/`B` em `F` se combina com o de `D` em `G`, sem conflito (oráculo:
  `ExtensionTypeInheritedMemberConflictTes_0c10b04b.dart` sem diagnóstico). No DartForge,
  `heranca.rs::de_tipo_de_extensao` usa `!m.sintetico && dono é extension type`.
- **Confere com §A:** emissão (linhas corretas: `:3450-3488`, não `:3448-3485`), posição e mensagem certas.
- **No DartForge:** inexistente (nenhum uso da constante). 4 FN: `ExtensionTypeInheritedMemberConflictTes_{1564a084,984a81f0,bf46982c,dc9cb5df}.dart`.
- **Exemplo (oráculo vivo 3.6.2):**
  ```dart
  extension type A(int it) { void foo() {} }
  extension type B(int it) { void foo() {} }
  extension type C(int it) implements A, B {}
  extension type D(int it) implements A, B { void foo() {} }
  class K { void bar() {} }
  extension type F(K it) { void bar() {} }
  extension type H(K it) implements K, F {}
  extension type N1(Never it) {}
  extension type N2(Never? it) {}
  extension type N3<T extends Never>(T it) {}
  ```
  - `extension_type_inherited_member_conflict` | offset 101, length 1 | 3:16 | The extension type 'C' has more than one distinct member named 'foo' from implemented types.
  - `extension_type_inherited_member_conflict` | offset 271, length 1 | 7:16 | The extension type 'H' has more than one distinct member named 'bar' from implemented types.
  - `extension_type_representation_type_bottom` | offset 316, length 5 | 8:19 | The representation type can't be a bottom type.
  - `extension_type_representation_type_bottom` | offset 396, length 1 | 10:36 | The representation type can't be a bottom type.

##### `extension_type_representation_type_bottom` — complemento (perda 4: FN 4)
- **Supressões e ordem:** `_checkForExtensionTypeRepresentationTypeBottom`
  (`analyzer/lib/src/generated/error_verifier.dart:3502-3513`) não tem guarda: relata sempre que
  `element.representation.type.isBottom`. `Never?` não é bottom (exemplo acima, `N2`: sem relato);
  parâmetro de tipo com limite `Never` é bottom (`N3`: relato no `T` do campo, length 1). Independe
  dos outros erros da declaração.
- **Confere com §A:** emissão, condição, posição (`node.representation.fieldType`) e mensagem certas.
- **No DartForge:** inexistente. 4 FN: `ConflictingGenericInterfaces__extensionType.dart:4:18`,
  `ExtensionTypeRepresentationTypeBottom__{a1114717:1:35, never:1:18}`,
  `extension_type/extension_type_representation_type_test.dart:17:20`.
- **Exemplo:** acima (`N1`, `N3`).

##### `instantiate_abstract_class` — complemento (perda 4: FN 4)
- **Supressões e ordem:** `ErrorVerifier.visitInstanceCreationExpression`
  (`analyzer/lib/src/generated/error_verifier.dart:1101-1122`): só entra se o tipo do `NamedType` é
  `InterfaceType` (`:1105`); ordem dos relatos no mesmo nó: `_checkForConstOrNewWithAbstractClass`
  (`:1106`) → `_checkForInvalidGenerativeConstructorReference` (`:1107`) → `_checkForConstOrNewWithMixin`
  (`:1108`) → argumentos obrigatórios (`:1109`) → `const`: `_checkForConstWithNonConst`,
  `_checkForConstWithUndefinedConstructor`, `_checkForConstDeferredClass` (`:1112-1116`) / não const:
  `_checkForNewWithUndefinedConstructor` (`:1118`). Dentro de `_checkForConstOrNewWithAbstractClass`
  (`:2950-2973`): só `ClassElement` abstrata (`abstract mixin class` conta; `mixin` não — é
  `MixinElement`); **construtor não resolvido** (`staticElement == null`) → não relata (sai
  `NEW_WITH_UNDEFINED_CONSTRUCTOR`); **factory** → não relata; implícita (`A()` sem `new`) e explícita
  relatam igual (`:2958-2970`, os dois ramos são idênticos).
- **Confere com §A:** certo. Posição = o `NamedType` (com prefixo e argumentos de tipo, se houver).
- **No DartForge:** `crates/types/src/inferencia/chamadas.rs:680`, `:1545`, `:1620`; código publicado
  (`crates/analise/verificados.txt`). Os 4 FN são `dot_shorthands/constructor/*` (`.new()` com contexto
  de classe abstrata — sintaxe 3.10, oráculo 3.13).
- **Exemplo (oráculo vivo 3.6.2):**
  ```dart
  abstract class A { A(); factory A.f() = B; const A.c(); }
  class B extends A { B() : super(); }
  mixin M {}
  abstract mixin class AM {}
  void f() {
    new A();
    A();
    A.f();
    const A.c();
    new A.nada();
    new M();
    M();
    M.named();
    const M();
    new AM();
  }
  ```
  - `instantiate_abstract_class` | offset 150, length 1 | 6:7 | Abstract classes can't be instantiated.
  - `instantiate_abstract_class` | offset 157, length 1 | 7:3 | (idem)
  - `instantiate_abstract_class` | offset 179, length 1 | 9:9 | (idem)
  - `new_with_undefined_constructor` | offset 194, length 4 | 10:9 | The class 'A' doesn't have a constructor named 'nada'.
  - `mixin_instantiate` | offset 208, length 1 | 11:7 | Mixins can't be instantiated.
  - `mixin_instantiate` | offset 215, length 1 | 12:3 | (idem)
  - `undefined_method` | offset 224, length 5 | 13:5 | The method 'named' isn't defined for the type 'M'.
  - `mixin_instantiate` | offset 241, length 1 | 14:9 | (idem)
  - `const_with_undefined_constructor_default` | offset 241, length 1 | 14:9 | The class 'M' doesn't have an unnamed constant constructor.
  - `instantiate_abstract_class` | offset 252, length 2 | 15:7 | (idem)

##### `mixin_instantiate` — complemento (perda 2: FN 2)
- **Emissão:** `ErrorVerifier._checkForConstOrNewWithMixin`
  (`analyzer/lib/src/generated/error_verifier.dart:2976-2984`), de `visitInstanceCreationExpression` (`:1108`).
- **Condição exata:** `type.element is MixinElement`, sem outra guarda (não olha o construtor).
- **Posição:** o `NamedType`. **Mensagem:** `Mixins can't be instantiated.` (`messages.yaml:10651`), sem argumentos.
- **Supressões e ordem:** corrige §A ("também a implícita `M()`/`M.n()`"): no oráculo vivo `M()` relata,
  mas **`M.named()` sem `new` não** — dá `undefined_method` ('named' … for the type 'M'), isto é, a
  reescrita para criação de instância não acontece quando o mixin não tem o construtor (exemplo acima,
  linha 13). Com `new`/`const` explícito relata sempre. Não suprime os códigos seguintes do mesmo nó:
  `const M()` dá também `const_with_undefined_constructor_default` no mesmo intervalo; `new M()` **não**
  dá `new_with_undefined_constructor_default` (só este código) — exemplo, linhas 11 e 14.
- **No DartForge:** inexistente (nenhum uso da constante). 2 FN: `MixinInstantiate__namedConstructor.dart:8:7`,
  `MixinInstantiate__unnamedConstructor.dart:4:7`. Entra em `chamadas.rs` (criação com classe de espécie
  mixin), antes do relato de construtor indefinido, que deve ficar mudo no `new`.

##### `wrong_number_of_type_arguments_extension` — complemento (perda 4: FN 4)
- **Emissão:** `ExtensionMemberResolver._inferTypeArguments`
  (`analyzer/lib/src/dart/resolver/extension_member_resolver.dart:321-346`), chamado de `resolveOverride` (`:207`).
- **Condição exata:** `node.typeArguments != null && arguments.length != typeParameters.length` (`:329-345`);
  recuperação: todos os argumentos de tipo viram `dynamic` (`_listOfDynamic`, `:345`).
- **Posição:** a lista `<…>` inteira (`typeArguments`, do `<` ao `>`).
- **Mensagem:** `The extension '{0}' is declared with {1} type parameters, but {2} type arguments were given.`
  (`messages.yaml:18376`); `{0}` = `element.name`, `{1}`/`{2}` inteiros (sem plural especial: "1 type arguments").
- **Supressões e ordem:** não é alcançado quando o override tem nº de argumentos ≠ 1
  (`INVALID_EXTENSION_ARGUMENT_COUNT` + `return`, `:187-196`). Depois dele, com tudo `dynamic`, a
  checagem de limites e `EXTENSION_OVERRIDE_ARGUMENT_NOT_ASSIGNABLE` (`:220-243`) seguem com o tipo
  estendido substituído por `dynamic` nos parâmetros de tipo.
- **No DartForge:** corrige §A: no disco há emissão em `crates/types/src/inferencia/chamadas.rs:734`
  (não commitado, não medido). Os 4 FN (`WrongNumberOfTypeArgumentsExtension__{notGeneric,tooFew,tooMany}.dart`,
  `WrongNumberOfTypeArguments__messageText_68a242a6.dart`) são do placar anterior a ela.
- **Exemplo (oráculo vivo 3.6.2):**
  ```dart
  extension E on int { void m() {} }
  extension G<T, U> on T { void g() {} }
  void f() {
    E<int>(1).m();
    G<int>(1).g();
    G<int, int, int>(1).g();
    G<int, String>(1).g();
  }
  ```
  - `wrong_number_of_type_arguments_extension` | offset 88, length 5 | 4:4 | The extension 'E' is declared with 0 type parameters, but 1 type arguments were given.
  - `wrong_number_of_type_arguments_extension` | offset 105, length 5 | 5:4 | The extension 'G' is declared with 2 type parameters, but 1 type arguments were given.
  - `wrong_number_of_type_arguments_extension` | offset 122, length 15 | 6:4 | The extension 'G' is declared with 2 type parameters, but 3 type arguments were given.

##### `assignment_to_const` — complemento (perda 3: FN 3)
- **Supressões e ordem:** dois emissores que não se sobrepõem:
  - atribuição (`=`, composta, `??=`): `AssignmentVerifier.verify`
    (`analyzer/lib/src/error/assignment_verifier.dart:30-112`), na resolução. Ordem: `requested != null`
    (o alvo resolveu para algo gravável): se é `VariableElement` const → este código, e `return`
    (`:36-45`); senão, pelo `recovery`: função → `ASSIGNMENT_TO_FUNCTION`, método →
    `ASSIGNMENT_TO_METHOD` (`:61-65`), prefixo → `PREFIX_IDENTIFIER_NOT_FOLLOWED_BY_DOT` (`:66-71`),
    getter: variável const → este código (`:77-81`), campo sintético → `ASSIGNMENT_TO_FINAL_NO_SETTER`
    (`:82-87`), senão `ASSIGNMENT_TO_FINAL` (`:88-93`); `MultiplyDefinedElement` → nada (`:95-96`);
  - `++`/`--` e `for (x in …)`: `ErrorVerifier._checkForAssignmentToFinal`
    (`analyzer/lib/src/generated/error_verifier.dart:2138-2196`), chamado em `:363` (lado esquerdo de
    atribuição — mas sai logo, `:2143-2145`: "already handled in the assignment resolver"), `:929`
    (`visitForEachPartsWithIdentifier`), `:1316` (pós-fixo), `:1339` (prefixo `++`/`--`). Só para
    `SimpleIdentifier` (`:2140`).
  - Variável **local** `final` não é este código nem `ASSIGNMENT_TO_FINAL`: é
    `ASSIGNMENT_TO_FINAL_LOCAL` (fluxo). Local `const` é este código (exemplo, linhas 7–11).
  - Um relato por nó; `x += 1` com `x` const dá um só.
- **Confere com §A:** posição (o identificador; em `A.sc = 3` só o `sc`, length 2) e mensagem certas.
- **No DartForge:** emissão por texto convertida em `crates/paridade/src/ponte.rs:87`, `:221`, `:227`;
  código publicado. FN: `AssignmentToConst__localVariable_inForEach.dart:3:8` (`for (x in …)` com `x`
  const — falta o ramo de `visitForEachPartsWithIdentifier`) e
  `switch/case_expression_with_assignment{,_legacy}_test.dart:18:10` (`case ERROR_B = 1:` em biblioteca
  2.x, onde o `case` é expressão — ver §E, `expected_token`).
- **Exemplo (oráculo vivo 3.6.2):**
  ```dart
  const c = 1;
  final fi = 1;
  class A { static const sc = 1; final int ff = 0; int get g => 0; void m() { ff = 1; g = 2; } }
  void f(List<int> l) {
    const x = 0;
    final y = 0;
    x = 1;
    x++;
    --x;
    x += 1;
    for (x in l) {}
    y = 1;
    for (y in l) {}
    c = 2;
    c++;
    fi = 2;
    fi++;
    A.sc = 3;
    (x) = 1;
    print(y);
  }
  ```
  - `assignment_to_final` | offset 103, length 2 | 3:77 | 'ff' can't be used as a setter because it's final.
  - `assignment_to_final_no_setter` | offset 111, length 1 | 3:85 | There isn't a setter named 'g' in class 'A'.
  - `unused_local_variable` | offset 152, length 1 | 5:9 | The value of the local variable 'x' isn't used.
  - `assignment_to_const` | offset 176, length 1 | 7:3 | Constant variables can't be assigned a value.
  - `assignment_to_const` | offset 185, length 1 | 8:3 | (idem)
  - `assignment_to_const` | offset 194, length 1 | 9:5 | (idem)
  - `assignment_to_const` | offset 199, length 1 | 10:3 | (idem)
  - `assignment_to_const` | offset 214, length 1 | 11:8 | (idem)
  - `assignment_to_final_local` | offset 227, length 1 | 12:3 | The final variable 'y' can only be set once.
  - `assignment_to_final_local` | offset 241, length 1 | 13:8 | (idem)
  - `assignment_to_const` | offset 254, length 1 | 14:3 | (idem)
  - `assignment_to_const` | offset 263, length 1 | 15:3 | (idem)
  - `assignment_to_final` | offset 270, length 2 | 16:3 | 'fi' can't be used as a setter because it's final.
  - `assignment_to_final` | offset 280, length 2 | 17:3 | (idem)
  - `assignment_to_const` | offset 290, length 2 | 18:5 | (idem)

  (Linha 19, `(x) = 1;` — atribuição por padrão a `x` const: nenhum diagnóstico no 3.6.2.)

##### `assignment_to_final` — complemento (perda 3: FN 3)
- **Supressões e ordem:** mesma cadeia de `assignment_to_const` (acima): só sai quando o alvo não tem
  setter (`requested == null`) e a recuperação é um getter cuja variável não é const e não é
  sintética (`assignment_verifier.dart:72-94`; `error_verifier.dart:2162-2184` para `++`/`--`/for-in).
  Getter explícito (variável sintética) → `ASSIGNMENT_TO_FINAL_NO_SETTER`; variável local → o fluxo
  (`ASSIGNMENT_TO_FINAL_LOCAL`).
- **Confere com §A:** posição (o identificador; `highlightedNode`), mensagem (`{0}` = `variable.name`) certas.
- **No DartForge:** `crates/paridade/src/ponte.rs:71`, `:223`, `:230`; publicado. Os 3 FN de
  `extension_methods/static_extension_internal_basename_shadowing_error_test.dart:402/406/410` eram da classe
  `A11 extends A10` com `static final int extensionFieldSetter`, sombreando o setter da extensão `E9 on A10`.
  O `LexicalLookup.resolveSetter` acha o getter estático, que não é `isInstanceMember`; ele vira recuperação, sem
  `ThisLookup.lookupSetter`, e sai `ASSIGNMENT_TO_FINAL`. Nós buscávamos pelo `this` e achávamos o setter da
  extensão. Corrigido em `tipo_de_escrita_nome` (`inferencia/expr.rs`): só o membro de instância de classe segue
  para o `this`. O de extensão também é recuperação (`extensions.dart:109-119`).
- **Exemplo:** acima (linhas 3, 16, 17).

##### `async_for_in_wrong_context` — complemento (perda 3: FN 3)
- **Supressões e ordem:** relato do parser (`parseForInLoopPartsRest`,
  `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:8543-8545`: `awaitToken != null && !inAsync` →
  `messageAwaitForNotAsync` no token `await`), convertido por nome em
  `analyzer/lib/src/fasta/error_converter.dart:34-40`. `inAsync` é o estado do parser para o corpo de
  função mais interno: closure síncrona dentro de função `async` relata (exemplo, linha 8); `sync*`
  relata. Nada o suprime; o laço segue sendo lido como `await for`.
- **Confere com §A:** posição (token `await`, length 5) e mensagem certas.
- **No DartForge:** inexistente (nenhum uso da constante). 3 FN: `AsyncForInWrongContext__syncFunction.dart:2:3`,
  `identifier/known_usage_error_test.dart:69:3` e `:83:3`.
- **Exemplo (oráculo vivo 3.6.2):**
  ```dart
  void f(Stream<int> s) {
    await for (var x in s) {}
  }
  void g(Stream<int> s) sync* {
    await for (var x in s) {}
  }
  void h(Stream<int> s) async {
    () { await for (var x in s) {} };
  }
  ```
  - `async_for_in_wrong_context` | offset 26, length 5 | 2:3 | The async for-in loop can only be used in an async function.
  - `unused_local_variable` | offset 41, length 1 | 2:18
  - `illegal_sync_generator_return_type` | offset 54, length 4 | 4:1
  - `async_for_in_wrong_context` | offset 86, length 5 | 5:3 | (idem)
  - `unused_local_variable` | offset 101, length 1 | 5:18
  - `async_for_in_wrong_context` | offset 151, length 5 | 8:8 | (idem)
  - `unused_local_variable` | offset 166, length 1 | 8:23

##### `extension_override_with_cascade` — complemento (perda 3: FN 3)
- **Emissão:** três resolvedores, conforme a seção da cascata: método — `MethodInvocationResolver`
  (`analyzer/lib/src/dart/resolver/method_invocation_resolver.dart:431-437`, `node.isCascaded`);
  propriedade (leitura/escrita) — `PropertyElementResolver._resolveTargetExtensionOverride`
  (`property_element_resolver.dart:606-612`, `target.parent is CascadeExpression`); tear-off com
  argumentos de tipo — `FunctionReferenceResolver` (`function_reference_resolver.dart:370-376`).
- **Condição exata:** o alvo da cascata é um `ExtensionOverride` (`E(x)..m()`, `..g`, `..s = v`).
- **Posição:** o nome da extensão (`override.name`, token; length = nome), não a cascata.
- **Mensagem:** `Extension overrides have no value so they can't be used as the receiver of a cascade expression.` (`messages.yaml:5270`).
- **Supressões e ordem:** no ramo de método, membro inexistente → `UNDEFINED_EXTENSION_METHOD` e
  `return` antes deste relato (`method_invocation_resolver.dart:416-421`); membro estático →
  `EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER` e segue (os dois saem). `resolveOverride` não relata
  `EXTENSION_OVERRIDE_WITHOUT_ACCESS` para alvo de cascata (`extension_member_resolver.dart:177-185`).
  No oráculo vivo, `E(3)..g..m()` (duas seções) dá um relato só (exemplo, linha 4) — o motivo
  (relatos idênticos no mesmo offset × só uma seção relatar) não verificado.
- **No DartForge:** inexistente. 3 FN: `ExtensionOverrideWithCascade__{getter,method,setter}.dart:5:3`.
- **Exemplo (oráculo vivo 3.6.2):**
  ```dart
  extension E on int { void m() {} int get g => 0; }
  void f() {
    E(3)..m();
    E(3)..g..m();
    E(3);
    var x = E(3);
  }
  extension type A(int it) { A.n(this.it) : super(); A.k(int x) : it = x, super(); }
  extension type B(int it) implements B {}
  extension type C(int it) implements D {}
  extension type D(int it) implements C {}
  ```
  - `extension_override_with_cascade` | offset 64, length 1 | 3:3 | Extension overrides have no value so they can't be used as the receiver of a cascade expression.
  - `extension_override_with_cascade` | offset 77, length 1 | 4:3 | (idem)
  - `extension_override_without_access` | offset 93, length 4 | 5:3 | An extension override can only be used to access instance members.
  - `unused_local_variable` | offset 105, length 1 | 6:7
  - `extension_override_without_access` | offset 109, length 4 | 6:11 | (idem)
  - `extension_type_constructor_with_super_invocation` | offset 159, length 5 | 8:43 | Extension type constructors can't include super initializers.
  - `extension_type_constructor_with_super_invocation` | offset 189, length 5 | 8:73 | (idem)
  - `extension_type_implements_itself` | offset 215, length 1 | 9:16 | The extension type can't implement itself.
  - `extension_type_implements_itself` | offset 256, length 1 | 10:16 | (idem)
  - `extension_type_implements_itself` | offset 297, length 1 | 11:16 | (idem)

##### `extension_override_without_access` — complemento (perda 1: FN 1)
- **Emissão:** `ExtensionMemberResolver.resolveOverride`
  (`analyzer/lib/src/dart/resolver/extension_member_resolver.dart:177-185`).
- **Condição exata:** `!_isValidContext(node) && !_isCascadeTarget(node)`; contexto válido
  (`:435-443`): operando esquerdo de binária, função de `FunctionExpressionInvocation`, alvo de índice,
  alvo de `MethodInvocation`, operando de `PrefixExpression`, alvo de `PropertyAccess`.
- **Posição:** o override inteiro (`E(3)`, do nome ao `)`). **Mensagem:** `An extension override can only be used to access instance members.`
- **Estado em 2026-10-05 (escrito, não compilado):** `chamadas.rs::override_sem_acesso` decide o contexto
  pelo pai sintático (`BodyInferrer::pai_de`, o `Pais` da unidade sob demanda). A cascata sai em
  `expr.rs` (`ExprKind::Cascade`), um relato por cascata, no nome da extensão. `extension_as_expression` sai
  em `expr.rs::extensao_como_expressao`, no identificador ou no `p.E`, com o nó `dynamic`.
- **Supressões e ordem:** o tipo do override vira `dynamic` (`:184`); a resolução continua (pode sair
  também `INVALID_EXTENSION_ARGUMENT_COUNT` ou `EXTENSION_OVERRIDE_ARGUMENT_NOT_ASSIGNABLE` no mesmo override).
- **No DartForge:** inexistente. 1 FN. Exemplo: acima, linhas 5 e 6.

##### `extension_type_constructor_with_super_invocation` — complemento (perda 3: FN 3)
- **Supressões e ordem:** `ErrorVerifier._checkForExtensionTypeConstructorWithSuperInvocation`
  (`analyzer/lib/src/generated/error_verifier.dart:3394-3403`), ao visitar cada
  `SuperConstructorInvocation`; única condição: `_enclosingClass is ExtensionTypeElement`. Um relato por
  `super(...)` na lista de inicializadores; sem guarda por outros erros.
- **Confere com §A:** posição = token `super` (length 5); mensagem sem argumentos.
- **No DartForge:** inexistente. 3 FN. Exemplo: acima, linha 8 (dois construtores, dois relatos).

##### `extension_type_implements_itself` — complemento (perda 3: FN 3)
- **Emissão:** `ErrorVerifier._checkForExtensionTypeImplementsItself`
  (`analyzer/lib/src/generated/error_verifier.dart:3438-3448`), chamado de
  `visitExtensionTypeDeclaration` (`:833`).
- **Condição exata:** `element.hasImplementsSelfReference` — marca posta pelo linker
  (`analyzer/lib/src/summary2/extension_type.dart:87`) em todos os extension types de um ciclo de
  `implements` (direto ou indireto: `C implements D`, `D implements C` marca os dois — exemplo).
- **Posição:** o nome do extension type (token). **Mensagem:** `The extension type can't implement itself.`
- **Supressões e ordem:** a marca vem do linker; o que ele faz com a lista de interfaces dos membros do
  ciclo — não verificado além do exemplo (não há cascata de outros códigos nas linhas 9–11).
- **No DartForge:** inexistente. 3 FN. Exemplo: acima, linhas 9–11.

##### `main_first_positional_parameter_type`, `main_has_too_many_required_positional_parameters`, `main_has_required_named_parameters` — complemento (perdas 3, 3, 2: todas FN)
- **Emissão:** `ErrorVerifier._checkForMainFunction2`
  (`analyzer/lib/src/generated/error_verifier.dart:4143-4187`), de `visitFunctionDeclaration`.
- **Condição exata** (em ordem; os três relatos são independentes e podem sair juntos):
  ```
  if nome != 'main' → return                                  // :4144
  if parent is! CompilationUnit → return                      // :4148 (método `main` de classe: nada)
  if parameters == null → return                              // :4152-4155 (getter `main`)
  positional = parâmetros posicionais (obrigatórios e opcionais)
  if nº de posicionais OBRIGATÓRIOS > 2 → MAIN_HAS_TOO_MANY_REQUIRED_POSITIONAL_PARAMETERS no nome   // :4162-4167
  if algum nomeado required → MAIN_HAS_REQUIRED_NAMED_PARAMETERS no nome                             // :4169-4174
  if positional não vazia e !isSubtypeOf(List<String>, tipo do 1º posicional)
       → MAIN_FIRST_POSITIONAL_PARAMETER_TYPE em first.notDefault.typeOrSelf                         // :4176-4186
  ```
- **Posição:** os dois primeiros no token do nome `main` (length 4); o terceiro no **tipo escrito** do
  1º posicional (`int` em `main(int a)`; em `[int? a, …]` o `int?` com o `?`, length 4); sem tipo
  escrito o parâmetro é `dynamic` e a condição não dispara.
- **Mensagem:** textos fixos, sem argumentos (`messages.yaml:9706`, `:9773`, `:9740`).
- **Supressões e ordem:** `main` que não é função (variável, getter) é outro código,
  `MAIN_IS_NOT_FUNCTION`, relatado por outro caminho (exemplo `m6`); `main` com parâmetro `this.a` ainda
  conta como posicional obrigatório (exemplo `m7`: sai junto de `field_initializer_outside_constructor`).
  `List<Object>` como 1º parâmetro é aceito (supertipo de `List<String>`).
- **No DartForge:** inexistentes (nenhum uso das constantes em `crates/`). FN:
  `MainFirstPositionalParameter__positiona_{190cac1f,2f89c8e2,dca9bcf6}.dart`,
  `MainHasTooManyRequiredPositionalParamet_{33422a3e,5676a295,a32afec7}.dart:1:6`,
  `MainHasRequiredNamedParameters__namedRequired.dart:1:6` (+ `…33422a3e.dart:1:6`, que tem os dois).
  Entram em `crates/analise` (declarações de topo), com o tipo do parâmetro vindo do outline.
- **Exemplos (oráculo vivo 3.6.2; um arquivo por linha):**
  - `void main(int a) {}` → `main_first_positional_parameter_type` | offset 10, length 3 | 1:11 | The type of the first positional parameter of the 'main' function must be a supertype of 'List<String>'.
  - `void main(a, b, c) {}` → `main_has_too_many_required_positional_parameters` | offset 5, length 4 | 1:6 | The function 'main' can't have more than two required positional parameters.
  - `void main({required List<String> a}) {}` → `main_has_required_named_parameters` | offset 5, length 4 | 1:6 | The function 'main' can't have any required named parameters.
  - `void main([int? a, int b = 0]) {}` → `main_first_positional_parameter_type` | offset 11, length 4 | 1:12
  - `void main(List<Object> a, [x]) {} class A { void main(int a, b, c, {required d}) {} }` → nenhum diagnóstico.
  - `var main = 1; get g => 0;` → `main_is_not_function` | offset 4, length 4 | 1:5 | The declaration named 'main' must be a function.
  - `void main(this.a, int b, c, d) {}` → `main_has_too_many_required_positional_parameters` | offset 5, length 4 | 1:6; `field_initializer_outside_constructor` | offset 10, length 6 | 1:11.

##### `super_in_extension_type` — complemento (perda 3: FN 3)
- **Emissão:** `ElementResolver.visitSuperExpression`
  (`analyzer/lib/src/generated/element_resolver.dart:404-424`), na resolução.
- **Condição exata:** `SuperContext.of(node)` devolve `extensionType` (o `super` está em membro de
  instância de `ExtensionTypeDeclaration`). Os outros contextos: `annotation`/`static` →
  `SUPER_IN_INVALID_CONTEXT` (`:407-412`), `extension` → `SUPER_IN_EXTENSION` (`:413-417`); `valid` → nada.
- **Posição:** o nó `super` (length 5). **Mensagem:** `The 'super' keyword can't be used in an extension type because an extension type doesn't have a superclass.` (`messages.yaml:15958`).
- **Supressões e ordem:** o contexto estático vence (`static void s() { super.m(); }` num extension type
  dá `super_in_invalid_context`, não este código — exemplo, linha 3). Um relato por `super`. A resolução do
  membro continua: no oráculo vivo `super.toString()`, `super + 1`, `super.x`, `super.x = 1` não dão
  segundo diagnóstico, mas `super[0]` dá também `undefined_super_member` (`The operator '[]' isn't
  defined in a superclass of 'A'.`).
- **No DartForge:** inexistente. 3 FN: `SuperInExtensionType__{binaryOperator,methodInvocation,propertyAccess}.dart:3:5`.
- **Exemplo (oráculo vivo 3.6.2):**
  ```dart
  extension type A(int it) {
    void m() { super.toString(); super + 1; super.x; super.x = 1; super[0]; }
    static void s() { super.m(); }
  }
  ```
  - `super_in_extension_type` | offset 40, length 5 | 2:14
  - `super_in_extension_type` | offset 58, length 5 | 2:32
  - `super_in_extension_type` | offset 69, length 5 | 2:43
  - `super_in_extension_type` | offset 78, length 5 | 2:52
  - `super_in_extension_type` | offset 91, length 5 | 2:65
  - `undefined_super_member` | offset 96, length 3 | 2:70 | The operator '[]' isn't defined in a superclass of 'A'.
  - `super_in_invalid_context` | offset 123, length 5 | 3:21 | Invalid context for 'super' invocation.

##### `undefined_named_parameter` — complemento (perda 3: FP 3)
- **Supressões e ordem** (`ResolverVisitor.resolveArgumentsToParameters`,
  `analyzer/lib/src/generated/resolver.dart`, laço em `:4292-4307`): para cada `NamedExpression`, busca
  `namedParameters[name]`; ausente → este código no **rótulo** (`argument.name.label`, sem o `:`).
  Antes do laço, em construtor, `verifySuperFormalParameters` (`:4280-4290`) acrescenta os nomes de
  `super.x` nomeados a `usedNames` — argumento explícito com o mesmo nome dá `DUPLICATE_NAMED_ARGUMENT`,
  não este código. Quando o alvo da chamada não resolve (função/construtor indefinido, `dynamic`), a
  função não é chamada com parâmetros e nada sai. `errorReporter` nulo (chamadas de reanálise) silencia.
- **Confere com §A:** posição e mensagem certas (`{0}` = nome como escrito, inclusive `_x`).
- **No DartForge:** `crates/types/src/inferencia/chamadas.rs:184`, conversão em `crates/paridade/src/ponte.rs:106`.
  3 FP: `DuplicateNamedArgument__constructor_sup_c6e7b4a1.dart:6:33` (nome vindo de `super.a` nomeado:
  deve ser `duplicate_named_argument`), `ExperimentalMemberUse__incorrectlyNeste_0e44e3bf.dart:16:13`
  (`const Experimental(x: …)` — a assinatura vista é a do SDK do DartForge; ver T3),
  `UseOfPrivateParameterName__andVoidLhsError.dart:8:5` (`_x` nomeado privado — arquivo de sintaxe 3.13,
  private-named-parameters; ver T2).

##### `use_of_native_extension` — complemento (perda 3: FN 3)
- **Supressões e ordem:** `_reportImportDirectiveErrors`
  (`analyzer/lib/src/dart/analysis/library_analyzer.dart:671-…`): `state is LibraryImportWithUri` e a URI
  **selecionada** (depois das configurações `if (dart.library.x)`) começa com `dart-ext:` → este código, e
  o `else if` seguinte (`URI_DOES_NOT_EXIST` / `…_IN_DOC_IMPORT`, `:683-…`) **não** roda — este código
  substitui o de URI inexistente. Export: `_resolveLibraryExportDirective` (`:909-915`), mesmo esquema.
  `@docImport` passa pelo mesmo `_reportImportDirectiveErrors`.
- **Confere com §A:** posição = o literal da URI com as aspas (length 12 para `'dart-ext:a'`); mensagem fixa.
- **No DartForge:** inexistente. 3 FN: `UseOfNativeExtension__{docImport:1:16, export:1:8, import:1:8}.dart`.
  Entra em `crates/analise/src/importacoes.rs`, antes de `uri_does_not_exist`.
- **Exemplo (oráculo vivo 3.6.2):**
  ```dart
  import 'dart-ext:a';
  export 'dart-ext:b';
  /// @docImport 'dart-ext:c';
  library;
  ```
  - `use_of_native_extension` | offset 7, length 12 | 1:8 | Dart native extensions are deprecated and aren't available in Dart 2.15.
  - `use_of_native_extension` | offset 28, length 12 | 2:8 | (idem)
  - `use_of_native_extension` | offset 57, length 12 | 3:16 | (idem)
  - `library_directive_not_first` | offset 71, length 7 | 4:1 | The library directive must appear before all other directives.

##### `wrong_number_of_type_arguments_enum` — complemento (perda 3: FN 3)
- **Emissão:** `TypeArgumentsVerifier.checkEnumConstantDeclaration`
  (`analyzer/lib/src/error/type_arguments_verifier.dart:93-112`), chamado do `ErrorVerifier` ao visitar
  cada `EnumConstantDeclaration`.
- **Condição exata:** `node.constructorElement != null` (`:94-97`: construtor não resolvido → nada) e a
  constante tem lista de argumentos de tipo com `length != enum.typeParameters.length` (`:104-106`).
  Sem lista escrita (`d()`) não relata.
- **Posição:** a lista `<…>` da constante. **Mensagem:** `The enum is declared with {0} type parameters, but {1} type arguments were given.` (`messages.yaml:18329`), inteiros.
- **Supressões e ordem:** depois do relato o método continua para a checagem de limites
  (`:114-…`; com contagem errada os tipos inferidos são os de recuperação). Independe do construtor ser const.
- **No DartForge:** inexistente. 3 FN: `WrongNumberOfTypeArgumentsEnum__{tooFew,tooMany}.dart:2:4`,
  `WrongNumberOfTypeArguments__messageText_enum.dart:2:4`.
- **Exemplo (oráculo vivo 3.6.2):**
  ```dart
  enum E<T, U> { a<int>(), b<int, int, int>(), c<int, String>(), d() }
  enum F { x<int>() }
  ```
  - `wrong_number_of_type_arguments_enum` | offset 16, length 5 | 1:17 | The enum is declared with 2 type parameters, but 1 type arguments were given.
  - `wrong_number_of_type_arguments_enum` | offset 26, length 15 | 1:27 | The enum is declared with 2 type parameters, but 3 type arguments were given.
  - `wrong_number_of_type_arguments_enum` | offset 79, length 5 | 2:11 | The enum is declared with 0 type parameters, but 1 type arguments were given.

##### `await_in_late_local_variable_initializer` — complemento (perda 2: FN 2)
- **Supressões e ordem:** `_checkForAwaitInLateLocalVariableInitializer`
  (`analyzer/lib/src/generated/error_verifier.dart:2205-2212`): `_isInLateLocalVariable.last`. A pilha é
  reiniciada ao entrar em corpo de função (closure dentro do inicializador **não** relata — exemplo,
  linha 5); campo `late` de classe não é "local": sai `await_in_wrong_context` (linha 9). Um relato por
  `await` (linha 6: dois). Não interage com `await_in_wrong_context` em função `async`.
- **Confere com §A:** posição = token `await` (length 5); mensagem fixa.
- **No DartForge:** inexistente. 2 FN: `AwaitInLateLocalVariableInitializer__{cl_dea16eee:3:19, late_await:2:16}.dart`.
- **Exemplo (oráculo vivo 3.6.2):**
  ```dart
  Future<int> g() async => 1;
  Future<void> f() async {
    late var a = await g();
    var b = await g();
    late var c = () async => await g();
    late var d = [await g(), await g()];
    print([a, b, c, d]);
  }
  class C { late var x = await g(); }
  ```
  - `await_in_late_local_variable_initializer` | offset 68, length 5 | 3:16 | The 'await' expression can't be used in a 'late' local variable's initializer.
  - `await_in_late_local_variable_initializer` | offset 154, length 5 | 6:17 | (idem)
  - `await_in_late_local_variable_initializer` | offset 165, length 5 | 6:28 | (idem)
  - `await_in_wrong_context` | offset 225, length 5 | 9:24 | The await expression can only be used in an async function.

##### `class_instantiation_access_to_member` — complemento (perda 2: FN 1, FP 1)
- **Emissão:** `ConstructorReferenceResolver.resolve`
  (`analyzer/lib/src/dart/resolver/constructor_reference_resolver.dart:40-90`), na resolução, quando o
  nome depois de `C<…>.` não é construtor. Três constantes com o mesmo nome publicado
  (`sharedName: CLASS_INSTANTIATION_ACCESS_TO_MEMBER`, `messages.yaml:1856/1864/1873`).
- **Condição exata:** construtor não resolvido e recurso `constructor-tearoffs` habilitado (`:51-52`:
  sem o recurso nada é relatado aqui). `enclosingElement` = elemento do tipo (alias desdobrado para a
  classe, `:54-58`). Se é `InterfaceElement` e tem método/getter/setter com o nome (`:62-66`): estático →
  `…_STATIC_MEMBER`, de instância → `…_INSTANCE_MEMBER`; senão `…_UNKNOWN_MEMBER`.
- **Posição:** o nó `ConstructorReference` inteiro (`C<int>.s`: do `C` ao fim do nome).
- **Mensagem:** estático: `The static member '{0}' can't be accessed on a class instantiation.`; instância:
  `The instance member '{0}' can't be accessed on a class instantiation.`; desconhecido:
  `The class '{0}' doesn't have a constructor named '{1}'.` (`{0}` = nome da classe, `{1}` = nome).
- **Supressões e ordem:** só para a forma **sem chamada**: `C<int>.s()` é criação de instância e dá
  `new_with_undefined_constructor` no nome (exemplo, linha 9). Getter estático conta como "static member".
- **No DartForge:** existe (8 acertos; nenhum uso direto da constante achado por grep em
  `crates/{analise,types,frontend,paridade}/src` — a emissão é por texto/ponte: ponto exato não verificado).
  FN `explicit_type_instantiation_parsing_test.dart:265:3` (`X<X>.any` — forma "unknown");
  FP `nonfunction_type_aliases/generic_usage_class_error_test.dart:36:3` (via alias; ver §A).
- **Exemplo (oráculo vivo 3.6.2):**
  ```dart
  class C<T> { C(); C.n(); static void s() {} void i() {} static int get g => 0; }
  typedef F<T> = void Function();
  void f() {
    C<int>.s;
    C<int>.i;
    C<int>.g;
    C<int>.nada;
    C<int>.n;
    C<int>.s();
    var l = <int, int>[];
    var m = <int>{1: 2};
    var m3 = <int, int, int>{};
    var s = <int, int>{1};
    var e = <int, int>{};
    const cl = <int, int>[];
    int x = 0;
    x<int>;
    f<int>;
    print([l, m, m3, s, e, cl]);
  }
  extension E on int {}
  var a = E;
  var b = [E];
  void g() { E; E.toString(); print(E); }
  ```
  - `class_instantiation_access_to_member` | offset 126, length 8 | 4:3 | The static member 's' can't be accessed on a class instantiation.
  - `class_instantiation_access_to_member` | offset 138, length 8 | 5:3 | The instance member 'i' can't be accessed on a class instantiation.
  - `class_instantiation_access_to_member` | offset 150, length 8 | 6:3 | The static member 'g' can't be accessed on a class instantiation.
  - `class_instantiation_access_to_member` | offset 162, length 11 | 7:3 | The class 'C' doesn't have a constructor named 'nada'.
  - `new_with_undefined_constructor` | offset 196, length 1 | 9:10 | The class 'C' doesn't have a constructor named 's'.
  - `expected_one_list_type_arguments` | offset 211, length 10 | 10:11 | List literals require one type argument or none, but 2 found.
  - `map_entry_not_in_map` | offset 241, length 4 | 11:17 | Map entries can only be used in a map literal.
  - `expected_two_map_type_arguments` | offset 259, length 15 | 12:12 | Map literals require two type arguments or none, but 3 found.
  - `expression_in_map` | offset 299, length 1 | 13:22 | Expressions can't be used in a map literal.
  - `expected_one_list_type_arguments` | offset 340, length 10 | 15:14 | (idem, 2 found)
  - `disallowed_type_instantiation_expression` | offset 369, length 1 | 17:3 | Only a generic type, generic function, generic instance method, or generic constructor can have type arguments.
  - `wrong_number_of_type_arguments_function` | offset 380, length 5 | 18:4 | The function 'f' is declared with 0 type parameters, but 1 type arguments were given.
  - `extension_as_expression` | offset 450, length 1 | 22:9 | Extension 'E' can't be used as an expression.
  - `extension_as_expression` | offset 462, length 1 | 23:10 | (idem)
  - `extension_as_expression` | offset 477, length 1 | 24:12 | (idem)
  - `undefined_extension_method` | offset 482, length 8 | 24:17 | The method 'toString' isn't defined for the extension 'E'.
  - `extension_as_expression` | offset 500, length 1 | 24:35 | (idem)

##### `expected_one_list_type_arguments`, `expected_one_set_type_arguments`, `expected_two_map_type_arguments` — complemento (perdas 1, 1, 2: FN)
- **Emissão:** `TypeArgumentsVerifier.checkListLiteral` (`analyzer/lib/src/error/type_arguments_verifier.dart:159-173`),
  `checkSetLiteral` (`:208-222`), `checkMapLiteral` (`:175-189`), chamados pelo `ErrorVerifier` em
  `visitListLiteral`/`visitSetOrMapLiteral` (conforme `node.isMap`/`isSet` já decidido pela resolução);
  todos terminam em `_checkTypeArgumentCount(typeArguments, n, código)`.
- **Condição exata:** há lista de argumentos de tipo e o nº ≠ `n` (lista 1, set 1, mapa 2).
- **Posição:** a lista `<…>` inteira. **Mensagem:** `List literals require one type argument or none, but {0} found.` /
  `Set literals require one type argument or none, but {0} were found.` /
  `Map literals require two type arguments or none, but {0} found.` — `{0}` inteiro.
- **Supressões e ordem:** a espécie do literal `{…}` é decidida **antes**, pelo nº de argumentos de tipo
  escritos: 1 → set, 2 → mapa, outro nº → pelos elementos (vazio → mapa). Por isso (exemplo): `<int>{1: 2}`
  é set e dá `map_entry_not_in_map` (não erro de contagem); `<int, int>{1}` é mapa e dá
  `expression_in_map`; `<int, int, int>{}` é mapa com 3 → `expected_two_map_type_arguments`; `<int, int>{}`
  nada. `expected_one_set_type_arguments` só sai com ≥ 3 argumentos e elementos que fazem do literal um
  set (a amostra `ExpectedOneSetTypeArguments__multiple_t_d68b4fa9.dart`; a regra de decisão está na
  resolução do literal — arquivo:linha não verificado). Em literal `const`, antes da contagem, cada
  argumento passa por `_checkTypeArgumentConst` (`INVALID_TYPE_ARGUMENT_IN_CONST_LIST/SET/MAP`, `:162-168`).
- **No DartForge:** inexistentes. FN: `ExpectedOneListTypeArguments__two_type_arguments.dart:2:3`,
  `ExpectedOneSetTypeArguments__multiple_t_d68b4fa9.dart:2:3`, `ExpectedTwoMapTypeArguments__three_type_{3e547b16,5284ef18}.dart:2:3`.
  Entram em `crates/types/src/inferencia/colecoes.rs` ao ler os argumentos explícitos.
- **Exemplo:** acima, linhas 10–15.

##### `disallowed_type_instantiation_expression` — complemento (perda 2: FN 2)
- **Emissão:** `FunctionReferenceResolver` (`analyzer/lib/src/dart/resolver/function_reference_resolver.dart`),
  ramos em que a função de `expr<T>` não é genérica nem tipo: `:270-278` (conferido) e os outros pontos de
  uso da constante no mesmo arquivo (`emissores.json`).
- **Condição exata:** `expr<…>` em que o tipo de `expr` não é `FunctionType` (nem tipo/construtor), com
  `isConstructorTearoffsEnabled` (`:271-273`: sem o recurso não relata; tipo vira `dynamic`/`InvalidType`).
- **Posição:** `node.function` (a expressão antes do `<`; no exemplo o `x`, length 1).
- **Mensagem:** texto fixo (`messages.yaml:3799`).
- **Supressões e ordem:** função **não genérica** com argumentos de tipo é outro código
  (`WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION`, via `_checkTypeArguments`, `:258-263`; exemplo, linha 18).
  O nó recebe `InvalidType` (`:278`), o que silencia erros de tipo a jusante.
- **No DartForge:** inexistente. FN `WrongNumberOfTypeArguments__functionTyp_7f76006c.dart:2:3` e
  `dot_shorthands/type_parameter/type_parameter_error_test.dart:37:57` (3.13).
- **Exemplo:** acima, linha 17.

##### `extension_as_expression` — complemento (perda 1: FN 1)
- **Emissão:** `SimpleIdentifierResolver` (`analyzer/lib/src/dart/resolver/simple_identifier_resolver.dart`, ver §A) e
  `PrefixedIdentifierResolver` (`prefixed_identifier_resolver.dart:160-184`, conferido).
- **Condição exata:** identificador (ou `p.E`) que resolve para `ExtensionElement` e cujo pai **não** é:
  referência em comentário, alvo de `MethodInvocation`, prefixo de `PrefixedIdentifier`, alvo de
  `PropertyAccess` (`:167-173`).
- **Posição:** o identificador (com prefixo, o `p.E` inteiro). **Mensagem:** `Extension '{0}' can't be used as an expression.` (`{0}` = `node.name`).
- **Supressões e ordem:** o nó recebe tipo `dynamic` (`:181-184`); `E.toString()` não é este código — é
  acesso a membro estático da extensão (`undefined_extension_method`, exemplo linha 24).
- **No DartForge:** inexistente. FN `ExtensionAsExpression__simpleIdentifier.dart:2:9`.
- **Exemplo:** acima, linhas 22–24.

##### `invalid_extension_argument_count` — complemento (perda 2: FN 2)
- **Supressões e ordem:** `resolveOverride` (`analyzer/lib/src/dart/resolver/extension_member_resolver.dart:189-198`):
  `arguments.length != 1` → relato em `node.argumentList` (do `(` ao `)`), argumentos de tipo e tipo
  estendido viram `dynamic`, e `return`: nenhum dos relatos seguintes (contagem de argumentos de tipo,
  limites, atribuibilidade do receptor) sai. Antes dele pode sair `EXTENSION_OVERRIDE_WITHOUT_ACCESS` (`:179-187`).
  Argumento nomeado conta como argumento (a contagem é de `arguments`).
- **No DartForge:** corrige §A: há emissão no disco em `crates/types/src/inferencia/chamadas.rs:705` (não
  commitado, não medido). FN do placar: `InvalidExtensionArgumentCount__{many,zero}.dart:5:4`.
- **Exemplo:** ver `extension_override_argument_not_assignable` (linha 12: offset 242, length 6).

##### `deferred_import_of_extension` — complemento (perda 2: FN 2)
- **Supressões e ordem:** `_checkForDeferredImportOfExtensions`
  (`analyzer/lib/src/generated/error_verifier.dart:3056-3067`): percorre `importElement.namespace.definedNames`
  — o namespace **depois** dos combinators — e relata **uma vez** (`return` no primeiro `ExtensionElement`).
  `hide X` ou `show K` tiram a extensão do namespace: sem relato (exemplo, linhas 2 e 3). Extensão sem
  nome não entra no namespace (não tem nome exportável) — não verificado com exemplo.
- **Confere com §A:** posição = a URI (com aspas); mensagem fixa.
- **No DartForge:** inexistente. FN `ConstInitializedWithNonConstantValueFro_d5ca985c.dart:1:8`,
  `extension_methods/static_extension_deferred_import_error_test.dart:12:8`.
- **Exemplo (oráculo vivo 3.6.2; `lib.dart` declara `extension X on int` e `class K`):**
  ```dart
  import 'lib.dart' deferred as p;
  import 'lib.dart' deferred as q hide X;
  import 'lib.dart' deferred as r show K;
  void f() { p.loadLibrary(); q.loadLibrary(); r.loadLibrary(); }
  ```
  - `deferred_import_of_extension` | offset 7, length 10 | 1:8 | Imports of deferred libraries must hide all extensions.
  - `unused_import` | offset 7, length 10 | 1:8 | Unused import: 'lib.dart'.
  - `unused_import` | offset 40, length 10 | 2:8 | (idem)
  - `unused_import` | offset 80, length 10 | 3:8 | (idem)

  (Nota para §F: `p.loadLibrary()` não conta como uso do import.)

##### `generic_function_type_cannot_be_type_argument` — complemento (perda 2: FN 2)
- **Emissão:** `TypeArgumentsVerifier` (`analyzer/lib/src/error/type_arguments_verifier.dart:302-310`, tipo
  nomeado — conferido; e o ramo de invocação citado em §A).
- **Condição exata:** argumento de tipo `FunctionType` com `typeFormals` não vazio **e** biblioteca sem
  `Feature.generic_metadata` (linguagem < 2.14) (`:302-303`); `continue` — o limite desse argumento não é checado.
- **Posição:** `_typeArgumentErrorNode(namedType, i)` — o argumento de tipo `i` escrito.
- **Mensagem:** `A generic function type can't be a type argument.` (`messages.yaml:6731`).
- **Supressões e ordem:** com linguagem ≥ 2.14 nunca sai. Em literal/inferência, o tipo inferido genérico
  dá `could_not_infer` (exemplo, 4:38).
- **Estado em 2026-10-05 (escrito, não compilado):** `limites_simples::verificar`, em biblioteca anterior à
  2.14, nos argumentos de todo tipo nomeado escrito que são tipo de função genérico, direto ou por alias.
- **No DartForge:** inexistente. FN `generic/function_type_as_type_argument_test.dart:20:8`,
  `generic_methods/generic_function_result_test.dart:14:6` (arquivos `// @dart` antigos).
- **Exemplo (oráculo vivo 3.6.2):**
  ```dart
  // @dart=2.12
  class G<T> {}
  G<void Function<T>(T)>? g;
  void h() { List<T Function<T>()> l = []; print(l); }
  ```
  - `generic_function_type_cannot_be_type_argument` | offset 30, length 19 | 3:3 | A generic function type can't be a type argument.
  - `generic_function_type_cannot_be_type_argument` | offset 71, length 15 | 4:17 | (idem)
  - `could_not_infer` | offset 92, length 2 | 4:38 | Couldn't infer type parameter 'E'. Inferred candidate type T Function<T>() has type parameters [T], but a function with type parameters cannot be used as a type argument.

##### `extension_type_constructor_with_super_formal_parameter` — complemento (perda 2: FN 2)
- **Supressões e ordem:** `ErrorVerifier.visitSuperFormalParameter`
  (`analyzer/lib/src/generated/error_verifier.dart:1456-1467`): depois de visitar os filhos, se
  `_enclosingClass is ExtensionTypeElement` → relato no token `super` e **`return`** — as demais checagens
  de `super.x` do mesmo método (`:1469-…`: fora de construtor, sem parâmetro associado etc.) não rodam.
  Um relato por parâmetro `super.x`, posicional ou nomeado.
- **Confere com §A:** posição = `node.superKeyword` (length 5); mensagem fixa (`messages.yaml:5312`).
- **No DartForge:** inexistente. FN `ExtensionTypeConstructorWithSuperFormal_{2344e265,6044357c}.dart`.
- **Exemplo (oráculo vivo 3.6.2):**
  ```dart
  class A { A(int a, [int? b]); A.n({int? x}); }
  class B extends A { B(super.a) : super(0); B.k(super.a, super.b) : super(1); B.j(int a, {super.x}) : super.n(); B.ok(super.a); }
  extension type E(int it) { E.n(super.it); E.m({super.x}) : it = 0; }
  enum G<T extends G<T>> { a }
  enum H<T extends num> { h }
  void f(List<int> l) { for (const x in l) {} for (const int y in 3) {} }
  ```
  - `positional_super_formal_parameter_with_positional_argument` | offset 75, length 1 | 2:29 | Positional super parameters can't be used when the super constructor invocation has a positional argument.
  - `positional_super_formal_parameter_with_positional_argument` | offset 100, length 1 | 2:54 | (idem)
  - `positional_super_formal_parameter_with_positional_argument` | offset 109, length 1 | 2:63 | (idem)
  - `final_not_initialized_constructor` | offset 203, length 1 | 3:28 | All final variables must be initialized, but 'it' isn't.
  - `extension_type_constructor_with_super_formal_parameter` | offset 207, length 5 | 3:32 | Extension type constructors can't declare super formal parameters.
  - `extension_type_constructor_with_super_formal_parameter` | offset 223, length 5 | 3:48 | (idem)
  - `type_argument_not_matching_bounds` | offset 270, length 1 | 4:26 | 'G<Object?>' doesn't conform to the bound 'G<G<Object?>>' of the type parameter 'T'.
  - `for_in_with_const_variable` | offset 329, length 5 | 6:28 | A for-in loop variable can't be a 'const'.
  - `unused_local_variable` | offset 335, length 1 | 6:34
  - `unused_local_variable` | offset 361, length 1 | 6:60
  - `for_in_of_invalid_type` | offset 366, length 1 | 6:65 | The type 'int' used in the 'for' loop must implement 'Iterable'.

##### `positional_super_formal_parameter_with_positional_argument` — complemento (perda 2: FN 2)
- **Supressões e ordem:** `verifySuperFormalParameters`
  (`analyzer/lib/src/error/super_formal_parameters_verifier.dart:11-37`): percorre os parâmetros do
  construtor; para cada `super.x` **posicional** com `hasExplicitPositionalArguments` (o `super(...)`
  explícito tem ao menos um argumento posicional — `resolver.dart:4281-4285`) relata no **nome** do
  parâmetro (`parameter.name`, `:27-31`). Um relato por parâmetro (exemplo: `B.k(super.a, super.b)` dá
  dois). `super.x` nomeado nunca relata aqui (vai para `namedArgumentNames`, `:22-23`). `errorReporter`
  é opcional: a mesma função é chamada sem relator em outros pontos (só para contar) — sem duplicata.
  Não depende de o construtor do super existir.
- **Confere com §A:** posição e mensagem (`messages.yaml:13648`, fixa) certas.
- **No DartForge:** corrige §A ("inexistente"): há emissão no disco em
  `crates/types/src/inferencia/funcoes.rs:303` (não commitado, não medido). FN do placar:
  `PositionalSuperFormalParameterWithPosit_{3f8bf0ed,a3e770b9}.dart`.
- **Exemplo:** acima, linha 2.

##### `extension_type_implements_representation_not_supertype` — complemento (perda 2: FN 2)
- **Supressões e ordem:** `ResolutionVisitor` (cláusula `implements` de extension type,
  `analyzer/lib/src/dart/resolver/resolution_visitor.dart:1794-1829`), em ordem, com `return` em cada
  relato: (1) tipo não permitido como superinterface → `EXTENSION_TYPE_IMPLEMENTS_DISALLOWED_TYPE`
  (`:1794-1801`); (2) a representação declarada é subtipo do tipo implementado → ok, `return`
  (`:1803-1806`; caso "implementa um supertipo da representação"); (3) o implementado é extension type
  (`type.representationType != null`): representação declarada não é subtipo da representação dele →
  **este código** (`:1809-1829`); senão ok; (4) caso contrário → `EXTENSION_TYPE_IMPLEMENTS_NOT_SUPERTYPE`.
- **Confere com §A:** posição = o `NamedType` da cláusula; mensagem com 4 argumentos — `{0}` e `{2}` são
  `DartType` (display com alias), `{1}` e `{3}` nomes de elemento.
- **No DartForge:** inexistente. FN `ExtensionTypeImplementsRepresentationNo_4b138b7b.dart:2:37`,
  `primary_constructors/type_inference/extension_type_error_test.dart:41:37` (3.13).
- **Exemplo (amostra do corpus, anotação do próprio arquivo — não rodado à parte):**
  `extension type A(String it) {}` / `extension type B(int it) implements A {}` →
  `'String', the representation type of 'A', is not a supertype of 'int', the representation type of 'B'.` no `A` da cláusula.

##### `enum_instantiated_to_bounds_is_not_well_bounded` — complemento (perda 1: FN 1)
- **Supressões e ordem:** `_checkForEnumInstantiatedToBoundsIsNotWellBounded`
  (`analyzer/lib/src/generated/error_verifier.dart:3239-3256`): usa o tipo do campo sintético `values`
  (`List<E<…instanciado aos limites…>>`) e `isWellBounded(arg, allowSuperBounded: true)`; relata no nome do
  enum. Limite F-limitado simples (`enum G<T extends G<T>>`) **não** dispara este código (o oráculo dá
  `type_argument_not_matching_bounds` no nome — exemplo acima, 4:26); o caso que dispara é limite por
  alias de função invariante (amostra).
- **No DartForge:** inexistente. FN `EnumInstantiatedToBoundsIsNotWellBounde_5319625b.dart:3:6`.
- **Estado em 2026-10-05 (escrito, não compilado):** `limites::enum_instanciado_aos_limites`, chamado pela
  paridade depois do `conflicting_generic_interfaces`: instancia os parâmetros do enum aos limites
  (`ops::instanciar_para_limites`), aplica o `replaceTopAndBottom` sobre os tipos resolvidos
  (`trocar_topo_e_fundo`, com o alias pela decoração de exibição e a variância do parâmetro no alvo) e
  confere só o invertido, como o `isWellBounded(allowSuperBounded: true)` do 3.6.2 (que descarta o
  resultado regular). Sem a decoração de alias (tabela sem exibição preservada) a função entra pela
  estrutura e o caso da amostra não dispara.
- **Exemplo (oráculo vivo 3.6.2):**
  ```dart
  typedef A<X> = X Function(X);

  enum E<T extends A<T>, U> {
    v<Never, int>()
  }
  ```
  - `enum_instantiated_to_bounds_is_not_well_bounded` | offset 36, length 1 | 3:6 | The result of instantiating the enum to bounds is not well-bounded.

##### `for_in_with_const_variable` — complemento (perda 1: FN 1)
- **Supressões e ordem:** `visitForEachPartsWithDeclaration`
  (`analyzer/lib/src/generated/error_verifier.dart:911-923`): só relata se `_checkForEachParts` devolveu
  `true` — isto é, se o iterável tem tipo válido; com `FOR_IN_OF_INVALID_TYPE` o `const` **não** é relatado
  (exemplo acima, 2º laço: só `for_in_of_invalid_type`). O parser aceita `const` na variável do for-in
  (o código `ForInLoopWithConstVariable` do fasta tem este `analyzerCode`, mas o relato do analyzer vem do
  `ErrorVerifier`).
- **Confere com §A:** posição = token `const` (length 5); mensagem fixa.
- **No DartForge:** inexistente. FN `ForInWithConstVariable__forEach_loopVariable.dart:2:8`.
- **Exemplo:** acima, linha 6.

##### `non_sized_type_argument`, `argument_must_be_native`, `must_return_void`, `non_native_function_type_argument_to_pointer` — complemento (FFI; perdas 2, 3, 1, 1: FN)
- **Emissão:** `FfiVerifier` (`analyzer/lib/src/generated/ffi_verifier.dart`), rodado pelo `LibraryAnalyzer`
  junto dos verificadores de erro:
  - `NON_SIZED_TYPE_ARGUMENT` — validação dos campos de `Struct`/`Union`, ramo `declaredType.isArray`
    (`:1473-1488`): `!_isSized(typeArg)`. Posição: o 1º argumento de tipo escrito do campo
    (`Array<Void>` → `Void`), ou o tipo do campo se não houver. Argumentos: `['Array', typeArg]` —
    na mensagem `{1}` (tipo) vem antes de `{0}` (`messages.yaml:20541`).
  - `ARGUMENT_MUST_BE_NATIVE` — `Native.addressOf<T>(arg)` (`:1672-1755`): sai do método sem relatar
    se a invocação não tem exatamente 1 argumento de tipo e 1 argumento (`:1672-1676`, "other diagnostics
    reported"); `referencedElement` só para `Identifier` (`:1682-1685`); procura na metadata do elemento
    uma anotação cujo tipo é `Native` (`:1687-1746`); nenhum alvo válido → relato **no argumento**
    (`:1749-1754`). Com alvo `@Native` válido os relatos possíveis são `MUST_BE_A_NATIVE_FUNCTION_TYPE` /
    `MUST_BE_A_SUBTYPE` no nó da invocação.
  - `MUST_RETURN_VOID` — `NativeCallable.listener(f)` (`:1764-1840`): ordem — nº de argumentos errado →
    `return` sem relato (`:1764-1769`); tipo `T` inválido → `MUST_BE_A_NATIVE_FUNCTION_TYPE` em
    `constructorName` e `return` (`:1771-1779`); assinatura incompatível → `MUST_BE_A_SUBTYPE` em `f` e
    `return` (`:1781-1790`); por fim, no ramo `listener` (não `isolateLocal`), retorno nativo ≠ `Void` →
    este código em `f`, argumento `natRetType` (DartType) (`:1831-1839`).
  - `NON_NATIVE_FUNCTION_TYPE_ARGUMENT_TO_POINTER` — `Pointer<T>.asFunction<F>()` (`:1196-1226`):
    `errorNode` = o 1º argumento de tipo de `asFunction` se houver, senão a invocação (`:1197`); `T` não é
    `NativeFunction` → `return` sem relato (`:1207-1209`); argumento do `NativeFunction` é parâmetro de
    tipo → `NON_CONSTANT_TYPE_ARGUMENT` no alvo (`:1211-1218`); assinatura C inválida → este código em
    `errorNode`, argumento `T` (o `NativeFunction<…>` inteiro) (`:1219-1226`).
- **Supressões e ordem:** as de cada item acima (os `return` antecipados). Nenhum depende de
  `InvalidType` explicitamente — não verificado o comportamento com tipos não resolvidos.
- **No DartForge:** inexistentes (nenhum uso das constantes; `crates/analise/src/nativos.rs` é o lugar).
  FN: `NonSizedTypeArgument__invalid_{struct,union}.dart:5:18`; `ffi_native/AddressOf__invalid_{Lambda:3:39,
  NotAConstant:9:26, String:3:39}.dart`; `FfiNativeCallableListenersMustReturnVoi_5a4b8147.dart:4:50`;
  `NonNativeFunctionTypeArgumentToPointer__5c6a9635.dart:6:18`.
- **Exemplo (oráculo vivo 3.6.2):**
  ```dart
  import 'dart:ffi';
  @Native<Void Function()>()
  external void nat();
  void notNative() {}
  final class S extends Struct {
    @Array(8)
    external Array<Void> a;
    @Array(8)
    external Array<Int8> b;
  }
  final class U extends Union {
    @Array(2)
    external Array<Handle> h;
  }
  int cb(int a) => a;
  void use(Pointer<NativeFunction<Int32 Function(int)>> p, Pointer<NativeFunction<Int32 Function(Int32)>> q) {
    Native.addressOf<NativeFunction<Void Function()>>(nat);
    Native.addressOf<NativeFunction<Void Function()>>(notNative);
    Native.addressOf<NativeFunction<Void Function()>>(() {});
    Native.addressOf<NativeFunction<Void Function()>>('x');
    NativeCallable<Int32 Function(Int32)>.listener(cb);
    NativeCallable<Void Function(Int32)>.listener((int a) {});
    p.asFunction<int Function(int)>();
    q.asFunction<int Function(int)>();
  }
  ```
  - `non_sized_type_argument` | offset 147, length 4 | 7:18 | The type 'Void' isn't a valid type argument for 'Array'. The type argument must be a native integer, 'Float', 'Double', 'Pointer', or subtype of 'Struct', 'Union', or 'AbiSpecificInteger'.
  - `non_sized_type_argument` | offset 255, length 6 | 13:18 | The type 'Handle' isn't a valid type argument for 'Array'. (…)
  - `argument_must_be_native` | offset 507, length 9 | 18:53 | Argument to 'Native.addressOf' must be annotated with @Native
  - `argument_must_be_native` | offset 571, length 5 | 19:53 | (idem)
  - `argument_must_be_native` | offset 631, length 3 | 20:53 | (idem)
  - `must_return_void` | offset 686, length 2 | 21:50 | The return type of the function passed to 'NativeCallable.listener' must be 'void' rather than 'Int32'.
  - `non_native_function_type_argument_to_pointer` | offset 767, length 17 | 23:16 | Can't invoke 'asFunction' because the function signature 'NativeFunction<Int32 Function(int)>' for the pointer isn't a valid C function signature.

### A.R4-3 Família A — guardas dos emissores (rodada 4, gerado)

Esta parte foi **gerada por script** (`E:\dftemp\analise\spec-r4\final\guardas.py`) para os códigos que a auditoria (§0.A) achou sem o campo "Supressões e ordem" e que não receberam complemento escrito à mão. Para cada código ela traz os pontos de emissão (arquivo:linha na tag 3.6.2, com a classe e o método que os contêm) e os **desvios do método emissor que antecedem o relato** (`if`, `return`, `continue`, `case`), copiados da fonte com o número da linha. É a matéria-prima do campo "Supressões": cada `return` antes do relato é uma condição em que o código não sai. O script não interpreta os desvios; quem implementar o código lê o trecho citado. As supressões gerais (tipo inválido, ordem das fases, homônimos) estão nos planos T1, T4 e T5 (§G).

##### `assignment_to_primary_constructor_parameter` — guardas do emissor (perda 40: FN 40, FP 0, msg 0, pos 0)
- **Emissão:** o código não existe no analyzer 3.6.2 (nenhuma constante com esse nome; auditoria §0.A, item 2). É do oráculo 3.13.4; a regra, tirada do checkout main, está na seção `assignment_to_primary_constructor_parameter` do grupo 3 (feito, 40/40).

##### `invocation_of_non_function` — guardas do emissor (perda 28: FN 28, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/method_invocation_resolver.dart:267` em `MethodInvocationResolver._reportInvocationOfNonFunction` (começa em `:258`).
  Nenhum desvio entre o início do método e o relato: o relato é incondicional dentro dele; a supressão está em quem o chama.

##### `new_with_undefined_constructor_default` — guardas do emissor (perda 26: FN 26, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/method_invocation_resolver.dart:925` em `MethodInvocationResolver._resolveReceiverTypeLiteral` (começa em `:890`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    897: if (node.isCascaded) {
    902: if (element != null) {
    903: if (element is ExecutableElement) {
    905: if (element is PropertyAccessorElement) {
    906: return _rewriteAsFunctionExpressionInvocation(
    911: } else {
    915: return null;
    920: if (nameNode.name == 'new') {
    922: if (_resolver.isConstructorTearoffsEnabled) {
  ```
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:4623` em `ErrorVerifier._checkForNewWithUndefinedConstructor` (começa em `:4596`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   4601: if (constructorName.staticElement != null) {
   4602: return;
   4605: if (type is InterfaceType) {
   4607: if (element is EnumElement || element is MixinElement) {
   4609: return;
   4614: if (name != null) {
   4620: } else {
  ```
- **Fase e supressões gerais:** fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `extra_positional_arguments_could_be_named` — guardas do emissor (perda 21: FN 21, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/resolver.dart:4339` em `ResolverVisitor.resolveArgumentsToParameters` (começa em `:4229`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   4266: if (argument is SimpleIdentifier && argument.name.isEmpty) {
   4270: if (unnamedIndex < unnamedParameterCount) {
   4272: } else {
   4280: if (enclosingConstructor != null) {
   4287: if (result.namedArgumentNames.isNotEmpty) {
   4292: for (int i = 0; i < argumentCount; i++) {
   4294: if (argument is NamedExpressionImpl) {
   4298: if (element == null) {
   4304: } else {
   4309: if (!usedNames.add(name)) {
   4319: if (positionalArgumentCount < requiredParameterCount && noBlankArguments) {
   4321: if (errorReporter != null && parent != null) {
   4332: } else if (positionalArgumentCount > unnamedParameterCount &&
   4337: if (namedParameterCount > namedArgumentCount) {
  ```
- **Fase e supressões gerais:** fase 3c (`ResolverVisitor`); operandos de tipo `InvalidType` não chegam ao relato (T4).

##### `ambiguous_extension_member_access` — guardas do emissor (perda 20: FN 19, FP 0, msg 1, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/extension_member_resolver.dart:122` em `ExtensionMemberResolver.findExtension` (começa em `:89`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
     98: if (extensions.isEmpty) {
     99: return ResolutionResult.none;
    102: if (extensions.length == 1) {
    107: return instantiated.asResolutionResult;
    111: if (mostSpecific.length == 1) {
    116: return instantiated.asResolutionResult;
  ```

##### `for_in_of_invalid_type` — guardas do emissor (perda 17: FN 17, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/shared_type_analyzer.dart:168` em `SharedTypeAnalyzerErrors.patternForInExpressionIsNotIterable` (começa em `:161`).
  Nenhum desvio entre o início do método e o relato: o relato é incondicional dentro dele; a supressão está em quem o chama.
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:3125` em `ErrorVerifier._checkForEachParts` (começa em `:3104`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   3105: if (checkForUseOfVoidResult(node.iterable)) {
   3106: return false;
   3113: if (parent is ForStatement) {
   3115: } else if (parent is ForElement) {
   3122: if (iterableType is DynamicType && strictCasts) {
  ```
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:3161` em `ErrorVerifier._checkForEachParts` (começa em `:3104`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   3105: if (checkForUseOfVoidResult(node.iterable)) {
   3106: return false;
   3113: if (parent is ForStatement) {
   3115: } else if (parent is ForElement) {
   3122: if (iterableType is DynamicType && strictCasts) {
   3128: return false;
   3132: if (typeSystem.isNullable(iterableType)) {
   3133: return false;
   3138: if (variableElement is VariableElement) {
   3140: } else {
   3141: return false;
   3153: if (typeSystem.isTop(iterableType)) {
   3157: if (!typeSystem.isAssignableTo(iterableType, requiredSequenceType,
  ```
- **Fase e supressões gerais:** fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `return_of_invalid_type_from_closure` — guardas do emissor (perda 17: FN 16, FP 0, msg 1, pos 0)
- **Emissão:** `analyzer/lib/src/error/return_type_verifier.dart:164` em `ReturnTypeVerifier._checkReturnExpression` (começa em `:134`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    135: if (!enclosingExecutable.hasLegalReturnType) {
    140: return;
    143: if (enclosingExecutable.isGenerator) {
    146: return;
    155: if (enclosingExecutable.catchErrorOnErrorReturnType != null) {
    161: } else if (enclosingExecutable.isClosure) {
  ```

##### `assignment_to_final_no_setter` — guardas do emissor (perda 16: FN 16, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/assignment_verifier.dart:85` em `AssignmentVerifier.verify` (começa em `:30`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
     36: if (requested != null) {
     37: if (requested is VariableElement) {
     38: if (requested.isConst) {
     45: return;
     48: if (recovery is DynamicElementImpl ||
     56: } else if (recovery is FunctionElement) {
     61: } else if (recovery is MethodElement) {
     66: } else if (recovery is PrefixElement) {
     72: } else if (recovery is PropertyAccessorElement && recovery.isGetter) {
     74: if (variable == null) {
     75: return;
     77: if (variable.isConst) {
     82: } else if (variable is FieldElement && variable.isSynthetic) {
  ```
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:2175` em `ErrorVerifier._checkForAssignmentToFinal` (começa em `:2138`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   2140: if (expression is! SimpleIdentifier) return;
   2143: if (expression.parent is AssignmentExpression) {
   2144: return;
   2150: if (expression is PrefixedIdentifier) {
   2155: if (element is VariableElement) {
   2156: if (element.isConst) {
   2162: } else if (element is PropertyAccessorElement && element.isGetter) {
   2164: if (variable == null) {
   2165: return;
   2167: if (variable.isConst) {
   2172: } else if (variable is FieldElement && variable.isSynthetic) {
  ```
- **Fase e supressões gerais:** fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `throw_of_invalid_type` — guardas do emissor (perda 16: FN 16, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:5344` em `ErrorVerifier._checkForThrowOfInvalidType` (começa em `:5336`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   5340: if (!typeSystem.isAssignableTo(type, typeSystem.objectNone,
  ```
- **Fase e supressões gerais:** fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `for_in_of_invalid_element_type` — guardas do emissor (perda 14: FN 14, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:3197` em `ErrorVerifier._checkForEachParts` (começa em `:3104`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   3128: return false;
   3132: if (typeSystem.isNullable(iterableType)) {
   3133: return false;
   3138: if (variableElement is VariableElement) {
   3140: } else {
   3141: return false;
   3153: if (typeSystem.isTop(iterableType)) {
   3157: if (!typeSystem.isAssignableTo(iterableType, requiredSequenceType,
   3164: return false;
   3173: if (sequenceType != null) {
   3178: if (sequenceElementType == null) {
   3179: return true;
   3182: if (!typeSystem.isAssignableTo(sequenceElementType, variableType,
   3194: if (implicitCallMethod == null) {
  ```
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:3229` em `ErrorVerifier._checkForEachParts` (começa em `:3104`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   3140: } else {
   3141: return false;
   3153: if (typeSystem.isTop(iterableType)) {
   3157: if (!typeSystem.isAssignableTo(iterableType, requiredSequenceType,
   3164: return false;
   3173: if (sequenceType != null) {
   3178: if (sequenceElementType == null) {
   3179: return true;
   3182: if (!typeSystem.isAssignableTo(sequenceElementType, variableType,
   3194: if (implicitCallMethod == null) {
   3200: } else {
   3205: if (_featureSet?.isEnabled(Feature.constructor_tearoffs) ?? true) {
   3220: if (typeArguments.isNotEmpty) {
   3225: if (!typeSystem.isAssignableTo(tearoffType, variableType,
  ```
- **Fase e supressões gerais:** fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `list_element_type_not_assignable` — guardas do emissor (perda 14: FN 14, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/constant/constant_verifier.dart:1210` em `_ConstLiteralVerifier._validateListExpression` (começa em `:1205`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1207: if (!verifier._runtimeTypeMatch(value, listElementType)) {
  ```
- **Emissão:** `analyzer/lib/src/error/literal_element_verifier.dart:60` em `LiteralElementVerifier._checkAssignableToElementType` (começa em `:54`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
     57: if (!typeSystem.isAssignableTo(type, elementType!,
  ```
- **Emissão:** `analyzer/lib/src/error/literal_element_verifier.dart:219` em `LiteralElementVerifier._verifySpreadForListOrSet` (começa em `:176`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    178: if (expressionType is DynamicType) {
    179: if (_errorVerifier.strictCasts) {
    185: return;
    188: if (typeSystem.isSubtypeOf(expressionType, NeverTypeImpl.instance)) {
    189: return;
    192: if (typeSystem.isSubtypeOf(expressionType, typeSystem.nullNone)) {
    193: if (isNullAware) {
    194: return;
    200: return;
    207: if (iterableType == null) {
    208: return errorReporter.atNode(
    216: if (!typeSystem.isAssignableTo(iterableElementType, elementType!,
  ```
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:4096` em `ErrorVerifier._checkForLateFinalFieldWithConstConstructor` (começa em `:4068`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   4069: if (node.isStatic) return;
   4072: if (!variableList.isFinal) return;
   4075: if (lateKeyword == null) return;
   4078: if (enclosingClass == null) {
   4080: return;
   4085: if (!hasGenerativeConstConstructor) return;
  ```
- **Fase e supressões gerais:** fase 5a (`ConstantVerifier`); ver D.R4-2; fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `new_with_undefined_constructor` — guardas do emissor (perda 14: FN 14, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:4595` em `ErrorVerifier._checkForNativeFunctionBodyInNonSdkCode` (começa em `:4579`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   4580: if (!_isInSystemLibrary) {
  ```
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:4617` em `ErrorVerifier._checkForNewWithUndefinedConstructor` (começa em `:4596`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   4601: if (constructorName.staticElement != null) {
   4602: return;
   4605: if (type is InterfaceType) {
   4607: if (element is EnumElement || element is MixinElement) {
   4609: return;
   4614: if (name != null) {
  ```
- **Fase e supressões gerais:** fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `undefined_super_member` — guardas do emissor (perda 13: FN 13, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/binary_expression_resolver.dart:448` em `BinaryExpressionResolver._resolveUserDefinableElement` (começa em `:396`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    403: if (leftOperand is ExtensionOverride) {
    406: if (member == null) {
    417: return;
    422: if (identical(leftType, NeverTypeImpl.instance)) {
    427: return;
    430: if (promoteLeftTypeToNonNull) {
    444: if (result.needsGetterError) {
    445: if (leftOperand is SuperExpression) {
  ```
- **Emissão:** `analyzer/lib/src/dart/resolver/method_invocation_resolver.dart:785` em `MethodInvocationResolver._resolveReceiverSuper` (começa em `:730`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    738: if (SuperContext.of(receiver) != SuperContext.valid) {
    741: return null;
    752: if (target != null) {
    754: if (target is PropertyAccessorElement) {
    755: return _rewriteAsFunctionExpressionInvocation(node, target.returnType,
    760: return null;
    767: if (target != null) {
    777: return null;
  ```
- **Emissão:** `analyzer/lib/src/dart/resolver/postfix_expression_resolver.dart:148` em `PostfixExpressionResolver._resolve1` (começa em `:124`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    127: if (identical(receiverType, NeverTypeImpl.instance)) {
    132: return;
    144: if (result.needsGetterError) {
    145: if (operand is SuperExpression) {
  ```
- **Emissão:** `analyzer/lib/src/dart/resolver/prefix_expression_resolver.dart:197` em `PrefixExpressionResolver._resolve1` (começa em `:150`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    153: if (operatorType.isUserDefinableOperator ||
    157: if (operand is ExtensionOverride) {
    160: if (member == null) {
    170: return;
    174: if (readType is InvalidType) {
    175: return;
    177: if (identical(readType, NeverTypeImpl.instance)) {
    182: return;
    193: if (result.needsGetterError) {
    194: if (operand is SuperExpression) {
  ```
- Outros pontos de uso: `analyzer/lib/src/dart/resolver/property_element_resolver.dart:121`, `analyzer/lib/src/dart/resolver/property_element_resolver.dart:131`, `analyzer/lib/src/dart/resolver/property_element_resolver.dart:827`, `analyzer/lib/src/dart/resolver/property_element_resolver.dart:873`.

##### `wrong_number_of_type_arguments_function` — guardas do emissor (perda 13: FN 13, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/constant/constant_verifier.dart:722` em `ConstantVerifier._reportError` (começa em `:634`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    635: if (error.avoidReporting) {
    636: return;
    645: if (identical(
  ```
- **Emissão:** `analyzer/lib/src/dart/constant/constant_verifier.dart:726` em `ConstantVerifier._reportError` (começa em `:634`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    635: if (error.avoidReporting) {
    636: return;
    645: if (identical(
  ```
- **Emissão:** `analyzer/lib/src/dart/constant/evaluation.dart:2319` em `DartObjectComputer.typeInstantiate` (começa em `:2308`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   2315: if (rawType is FunctionType) {
   2316: if (typeArguments.length != rawType.typeFormals.length) {
   2317: if (node is SimpleIdentifier) {
   2318: return InvalidConstant.forEntity(typeArgumentsErrorNode,
  ```
- **Emissão:** `analyzer/lib/src/dart/constant/evaluation.dart:2329` em `DartObjectComputer.typeInstantiate` (começa em `:2308`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   2315: if (rawType is FunctionType) {
   2316: if (typeArguments.length != rawType.typeFormals.length) {
   2317: if (node is SimpleIdentifier) {
   2318: return InvalidConstant.forEntity(typeArgumentsErrorNode,
   2326: return InvalidConstant.forEntity(
  ```
- Outros pontos de uso: `analyzer/lib/src/dart/resolver/function_reference_resolver.dart:124`, `analyzer/lib/src/dart/resolver/function_reference_resolver.dart:126`, `analyzer/lib/src/dart/resolver/function_reference_resolver.dart:262`, `analyzer/lib/src/dart/resolver/function_reference_resolver.dart:295`.
- **Fase e supressões gerais:** fase 5a (`ConstantVerifier`); ver D.R4-2.

##### `invocation_of_non_function_expression` — guardas do emissor (perda 12: FN 12, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/function_expression_invocation_resolver.dart:86` em `FunctionExpressionInvocationResolver.resolve` (começa em `:35`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
     40: if (function is ExtensionOverrideImpl) {
     43: return;
     47: if (_checkForUseOfVoidResult(function, receiverType)) {
     50: return;
     53: if (receiverType is FunctionType) {
     60: return;
     63: if (identical(receiverType, NeverTypeImpl.instance)) {
     70: return;
     82: if (callElement == null) {
     83: if (result.needsGetterError) {
  ```
- **Emissão:** `analyzer/lib/src/dart/resolver/function_expression_invocation_resolver.dart:99` em `FunctionExpressionInvocationResolver.resolve` (começa em `:35`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
     40: if (function is ExtensionOverrideImpl) {
     43: return;
     47: if (_checkForUseOfVoidResult(function, receiverType)) {
     50: return;
     53: if (receiverType is FunctionType) {
     60: return;
     63: if (identical(receiverType, NeverTypeImpl.instance)) {
     70: return;
     82: if (callElement == null) {
     83: if (result.needsGetterError) {
     93: return;
     96: if (callElement.kind != ElementKind.METHOD) {
  ```
- **Emissão:** `analyzer/lib/src/generated/resolver.dart:2022` em `ResolverVisitor.visitAugmentedInvocation` (começa em `:1975`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1988: for (var argument in node.arguments.arguments) {
   1996: if (augmentationTarget is PropertyAccessorElementImpl) {
   1997: if (augmentationTarget.isSetter) {
   2006: return;
   2019: if (functionType == null) {
  ```
- **Fase e supressões gerais:** fase 3c (`ResolverVisitor`); operandos de tipo `InvalidType` não chegam ao relato (T4).

##### `map_key_type_not_assignable` — guardas do emissor (perda 12: FN 12, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/constant/constant_verifier.dart:1290` em `_ConstLiteralVerifier._validateMapLiteralEntry` (começa em `:1268`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1270: if (config == null) return false;
   1284: if (keyValue is DartObjectImpl) {
   1287: if (!verifier._runtimeTypeMatch(keyValue, config.keyType)) {
  ```
- **Emissão:** `analyzer/lib/src/error/literal_element_verifier.dart:153` em `LiteralElementVerifier._verifyMapLiteralEntry` (começa em `:130`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    132: if (mapKeyType is! VoidType &&
    134: return;
    138: if (mapValueType is! VoidType &&
    140: return;
    146: if (entry.keyQuestion != null) {
    149: if (!typeSystem.isAssignableTo(keyType, mapKeyType,
  ```
- **Emissão:** `analyzer/lib/src/error/literal_element_verifier.dart:311` em `LiteralElementVerifier._verifySpreadForMap` (começa em `:267`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    269: if (expressionType is DynamicType) {
    270: if (_errorVerifier.strictCasts) {
    276: return;
    279: if (typeSystem.isSubtypeOf(expressionType, NeverTypeImpl.instance)) {
    280: return;
    283: if (typeSystem.isSubtypeOf(expressionType, typeSystem.nullNone)) {
    284: if (isNullAware) {
    285: return;
    291: return;
    298: if (mapType == null) {
    299: return errorReporter.atNode(
    307: if (!typeSystem.isAssignableTo(keyType, mapKeyType!,
  ```
- **Fase e supressões gerais:** fase 5a (`ConstantVerifier`); ver D.R4-2.

##### `map_value_type_not_assignable` — guardas do emissor (perda 12: FN 12, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/constant/constant_verifier.dart:1316` em `_ConstLiteralVerifier._validateMapLiteralEntry` (começa em `:1268`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1270: if (config == null) return false;
   1284: if (keyValue is DartObjectImpl) {
   1287: if (!verifier._runtimeTypeMatch(keyValue, config.keyType)) {
   1296: if (!keyValue.hasPrimitiveEquality(featureSet)) {
   1305: if (existingKey != null) {
   1307: } else {
   1312: if (valueValue is DartObjectImpl) {
   1313: if (!verifier._runtimeTypeMatch(valueValue, config.valueType)) {
  ```
- **Emissão:** `analyzer/lib/src/error/literal_element_verifier.dart:168` em `LiteralElementVerifier._verifyMapLiteralEntry` (começa em `:130`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    132: if (mapKeyType is! VoidType &&
    134: return;
    138: if (mapValueType is! VoidType &&
    140: return;
    146: if (entry.keyQuestion != null) {
    149: if (!typeSystem.isAssignableTo(keyType, mapKeyType,
    161: if (entry.valueQuestion != null) {
    164: if (!typeSystem.isAssignableTo(valueType, mapValueType,
  ```
- **Emissão:** `analyzer/lib/src/error/literal_element_verifier.dart:322` em `LiteralElementVerifier._verifySpreadForMap` (começa em `:267`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    269: if (expressionType is DynamicType) {
    270: if (_errorVerifier.strictCasts) {
    276: return;
    279: if (typeSystem.isSubtypeOf(expressionType, NeverTypeImpl.instance)) {
    280: return;
    283: if (typeSystem.isSubtypeOf(expressionType, typeSystem.nullNone)) {
    284: if (isNullAware) {
    285: return;
    291: return;
    298: if (mapType == null) {
    299: return errorReporter.atNode(
    307: if (!typeSystem.isAssignableTo(keyType, mapKeyType!,
    318: if (!typeSystem.isAssignableTo(valueType, mapValueType!,
  ```
- **Fase e supressões gerais:** fase 5a (`ConstantVerifier`); ver D.R4-2.

##### `missing_required_argument` — guardas do emissor (perda 12: FN 2, FP 0, msg 0, pos 10)
- **Emissão:** `analyzer/lib/src/error/required_parameters_verifier.dart:128` em `RequiredParametersVerifier._check` (começa em `:111`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    117: if (parameters == null) {
    118: return;
    121: for (ParameterElement parameter in parameters) {
    122: if (parameter.isRequiredNamed) {
    124: if (!_containsNamedExpression(
  ```

##### `super_formal_parameter_without_associated_positional` — guardas do emissor (perda 12: FN 12, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:1489` em `ErrorVerifier.visitSuperFormalParameter` (começa em `:1457`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1460: if (_enclosingClass is ExtensionTypeElement) {
   1466: return;
   1470: if (!(constructor is ConstructorDeclaration &&
   1476: return;
   1482: if (superParameter == null) {
  ```
- **Fase e supressões gerais:** fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `wrong_number_of_type_arguments_constructor` — guardas do emissor (perda 12: FN 12, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/ast_rewrite.dart:441` em `AstRewriter._instanceCreation_prefix_type_name` (começa em `:423`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    433: if (constructorElement == null) {
    434: return node;
    438: if (typeArguments != null) {
  ```
- **Emissão:** `analyzer/lib/src/dart/resolver/ast_rewrite.dart:607` em `AstRewriter._toInstanceCreation_type_constructor` (começa em `:590`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    599: if (constructorElement == null) {
    600: return node;
    604: if (typeArguments != null) {
  ```
- **Emissão:** `analyzer/lib/src/dart/resolver/function_reference_resolver.dart:59` em `FunctionReferenceResolver.resolve` (começa em `:39`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
     43: if (function is SimpleIdentifierImpl) {
     45: } else if (function is PrefixedIdentifierImpl) {
     47: } else if (function is PropertyAccessImpl) {
     49: } else if (function is ConstructorReferenceImpl) {
     51: if (typeArguments != null) {
  ```
- **Emissão:** `analyzer/lib/src/dart/resolver/function_reference_resolver.dart:249` em `FunctionReferenceResolver._resolve` (começa em `:225`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    230: if (rawType == null) {
    234: if (rawType is InvalidType) {
    236: return;
    239: if (rawType is TypeParameterTypeImpl) {
    246: if (rawType is FunctionType) {
  ```
- Outros pontos de uso: `analyzer/lib/src/dart/resolver/named_type_resolver.dart:343`.

##### `undefined_setter` — guardas do emissor (perda 11: FN 7, FP 4, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/property_element_resolver.dart:474` em `PropertyElementResolver._resolve` (começa em `:383`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    410: if (aliasedType is InterfaceType) {
    411: return _resolveTargetInterfaceElement(
    427: if (target is Identifier) {
    429: if (targetElement is ExtensionElement) {
    430: return _resolveTargetExtensionElement(
    441: if (propertyName.name == FunctionElement.CALL_METHOD_NAME) {
    442: if (targetType is FunctionType || targetType.isDartCoreFunction) {
    443: return PropertyElementResolverResult(
    449: if (targetType is VoidType) {
    454: return PropertyElementResolverResult();
    457: if (isNullAware) {
    461: if (target is TypeLiteral && target.type.type is FunctionType) {
    465: if (hasRead) {
    471: } else {
  ```
- **Emissão:** `analyzer/lib/src/error/assignment_verifier.dart:28`.
  Nenhum desvio entre o início do método e o relato: o relato é incondicional dentro dele; a supressão está em quem o chama.
- **Emissão:** `analyzer/lib/src/error/assignment_verifier.dart:104` em `AssignmentVerifier.verify` (começa em `:30`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
     56: } else if (recovery is FunctionElement) {
     61: } else if (recovery is MethodElement) {
     66: } else if (recovery is PrefixElement) {
     72: } else if (recovery is PropertyAccessorElement && recovery.isGetter) {
     74: if (variable == null) {
     75: return;
     77: if (variable.isConst) {
     82: } else if (variable is FieldElement && variable.isSynthetic) {
     88: } else {
     95: } else if (recovery is MultiplyDefinedElementImpl) {
     97: } else {
     98: if (node.isSynthetic) {
     99: return;
    101: if (receiverType != null) {
  ```

##### `doc_directive_missing_argument` — guardas do emissor (perda 10: FN 10, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/doc_comment_verifier.dart:64` em `DocCommentVerifier.validateArgumentCount` (começa em `:53`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
     58: if (positionalArgumentCount < requiredCount) {
     60: if (gap == 1) {
  ```
- **Emissão:** `analyzer/lib/src/error/doc_comment_verifier.dart:75` em `DocCommentVerifier.validateArgumentCount` (começa em `:53`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
     58: if (positionalArgumentCount < requiredCount) {
     60: if (gap == 1) {
     67: } else if (gap == 2) {
  ```
- **Emissão:** `analyzer/lib/src/error/doc_comment_verifier.dart:87` em `DocCommentVerifier.validateArgumentCount` (começa em `:53`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
     58: if (positionalArgumentCount < requiredCount) {
     60: if (gap == 1) {
     67: } else if (gap == 2) {
     78: } else if (gap == 3) {
  ```

##### `argument_must_be_a_constant` — guardas do emissor (perda 9: FN 9, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/ffi_verifier.dart:1600` em `FfiVerifier._validateFromFunction` (começa em `:1532`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1534: if (argCount < 1 || argCount > 2) {
   1537: return;
   1541: if (!_isValidFfiNativeFunctionType(T)) {
   1544: if (typeArgument != null) {
   1552: return;
   1557: if (!_validateCompatibleFunctionTypes(
   1564: return;
   1569: if (_primitiveNativeType(R) == _PrimitiveDartType.void_ ||
   1573: if (argCount != 1) {
   1580: } else if (argCount != 2) {
   1586: } else {
   1589: if (!_validateCompatibleNativeType(
   1597: if (!_isConst(e)) {
  ```
- **Emissão:** `analyzer/lib/src/generated/ffi_verifier.dart:1618` em `FfiVerifier._validateIsLeafIsConst` (começa em `:1609`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1611: if (args.isNotEmpty) {
   1612: for (var arg in args) {
   1613: if (arg is NamedExpression) {
   1614: if (arg.element?.name == _isLeafParamName) {
   1615: if (!_isConst(arg.expression)) {
  ```
- **Emissão:** `analyzer/lib/src/generated/ffi_verifier.dart:1826` em `FfiVerifier._validateNativeCallable` (começa em `:1759`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1765: if (!(argCount == 1 || (isolateLocal && argCount == 2))) {
   1768: return;
   1772: if (!_isValidFfiNativeFunctionType(typeArg)) {
   1778: return;
   1783: if (!_validateCompatibleFunctionTypes(
   1790: return;
   1794: if (isolateLocal) {
   1795: if (_primitiveNativeType(natRetType) == _PrimitiveDartType.void_ ||
   1799: if (argCount != 1) {
   1806: } else if (argCount != 2) {
   1812: } else {
   1815: if (!_validateCompatibleNativeType(
   1823: if (!_isConst(e)) {
  ```
- **Fase e supressões gerais:** fase 5d (`FfiVerifier`); só roda se a biblioteca usa `dart:ffi`.

##### `no_annotation_constructor_arguments` — guardas do emissor (perda 9: FN 9, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/constant/constant_verifier.dart:121` em `ConstantVerifier.visitAnnotation` (começa em `:103`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    107: if (element is ConstructorElement) {
    109: if (!element.isConst) {
    114: return;
    118: if (argumentList == null) {
  ```
- **Fase e supressões gerais:** fase 5a (`ConstantVerifier`); ver D.R4-2.

##### `super_in_extension` — guardas do emissor (perda 8: FN 8, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/element_resolver.dart:416` em `ElementResolver.visitSuperExpression` (começa em `:404`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    406: switch (context) {
    407: case SuperContext.annotation:
    408: case SuperContext.static:
    413: case SuperContext.extension:
  ```

##### `undefined_extension_operator` — guardas do emissor (perda 8: FN 8, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/binary_expression_resolver.dart:411` em `BinaryExpressionResolver._resolveUserDefinableElement` (começa em `:396`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    403: if (leftOperand is ExtensionOverride) {
    406: if (member == null) {
  ```
- **Emissão:** `analyzer/lib/src/dart/resolver/prefix_expression_resolver.dart:165` em `PrefixExpressionResolver._resolve1` (começa em `:150`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    153: if (operatorType.isUserDefinableOperator ||
    157: if (operand is ExtensionOverride) {
    160: if (member == null) {
  ```
- **Emissão:** `analyzer/lib/src/dart/resolver/property_element_resolver.dart:57` em `PropertyElementResolver.resolveIndexExpression` (começa em `:41`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
     48: if (target is ExtensionOverride) {
     52: if (hasRead && result.getter == null && !result.isAmbiguous) {
  ```
- **Emissão:** `analyzer/lib/src/dart/resolver/property_element_resolver.dart:67` em `PropertyElementResolver.resolveIndexExpression` (começa em `:41`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
     48: if (target is ExtensionOverride) {
     52: if (hasRead && result.getter == null && !result.isAmbiguous) {
     62: if (hasWrite && result.setter == null && !result.isAmbiguous) {
  ```

##### `duplicate_named_argument` — guardas do emissor (perda 7: FN 7, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/resolver.dart:4312` em `ResolverVisitor.resolveArgumentsToParameters` (começa em `:4229`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   4246: } else if (parameter.isOptionalPositional) {
   4249: } else {
   4263: for (int i = 0; i < argumentCount; i++) {
   4265: if (argument is! NamedExpression) {
   4266: if (argument is SimpleIdentifier && argument.name.isEmpty) {
   4270: if (unnamedIndex < unnamedParameterCount) {
   4272: } else {
   4280: if (enclosingConstructor != null) {
   4287: if (result.namedArgumentNames.isNotEmpty) {
   4292: for (int i = 0; i < argumentCount; i++) {
   4294: if (argument is NamedExpressionImpl) {
   4298: if (element == null) {
   4304: } else {
   4309: if (!usedNames.add(name)) {
  ```
- **Fase e supressões gerais:** fase 3c (`ResolverVisitor`); operandos de tipo `InvalidType` não chegam ao relato (T4).

##### `extension_type_declares_member_of_object` — guardas do emissor (perda 7: FN 7, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:3388` em `ErrorVerifier._checkForExtensionDeclaresMemberOfObject` (começa em `:3374`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   3375: if (_enclosingExtension != null) {
   3376: if (node.hasObjectMemberName) {
   3384: if (_enclosingClass is ExtensionTypeElement) {
   3385: if (node.hasObjectMemberName) {
  ```
- **Fase e supressões gerais:** fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `instantiate_type_alias_expands_to_type_parameter` — guardas do emissor (perda 7: FN 7, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/named_type_resolver.dart:436` em `NamedTypeResolver._verifyTypeAliasForContext` (começa em `:420`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    426: if (element.aliasedType is TypeParameterType) {
    428: if (parent is ConstructorName) {
    431: if (constructorUsage is InstanceCreationExpression) {
  ```

##### `leaf_call_must_not_return_handle` — guardas do emissor (perda 7: FN 7, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/ffi_verifier.dart:1420` em `FfiVerifier._validateFfiLeafCallUsesNoHandles` (começa em `:1413`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1415: if (nativeType is FunctionType) {
   1416: if (_primitiveNativeType(nativeType.returnType) ==
  ```
- **Fase e supressões gerais:** fase 5d (`FfiVerifier`); só roda se a biblioteca usa `dart:ffi`.

##### `set_element_type_not_assignable` — guardas do emissor (perda 7: FN 7, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/constant/constant_verifier.dart:1364` em `_ConstLiteralVerifier._validateSetExpression` (começa em `:1356`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1361: if (!verifier._runtimeTypeMatch(value, config.elementType)) {
  ```
- **Emissão:** `analyzer/lib/src/error/literal_element_verifier.dart:61` em `LiteralElementVerifier._checkAssignableToElementType` (começa em `:54`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
     57: if (!typeSystem.isAssignableTo(type, elementType!,
  ```
- **Emissão:** `analyzer/lib/src/error/literal_element_verifier.dart:220` em `LiteralElementVerifier._verifySpreadForListOrSet` (começa em `:176`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    178: if (expressionType is DynamicType) {
    179: if (_errorVerifier.strictCasts) {
    185: return;
    188: if (typeSystem.isSubtypeOf(expressionType, NeverTypeImpl.instance)) {
    189: return;
    192: if (typeSystem.isSubtypeOf(expressionType, typeSystem.nullNone)) {
    193: if (isNullAware) {
    194: return;
    200: return;
    207: if (iterableType == null) {
    208: return errorReporter.atNode(
    216: if (!typeSystem.isAssignableTo(iterableElementType, elementType!,
  ```
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:5275` em `ErrorVerifier._checkForSealedSupertypeOutsideOfLibrary` (começa em `:5235`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   5241: for (NamedType namedType in namedTypes) {
   5243: if (type is InterfaceType) {
   5245: if (element is ClassElement &&
   5258: if (superclass != null) {
   5261: if (withClause != null) {
   5264: if (implementsClause != null) {
   5267: if (onClause != null) {
  ```
- **Fase e supressões gerais:** fase 5a (`ConstantVerifier`); ver D.R4-2; fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `extension_type_implements_not_supertype` — guardas do emissor (perda 6: FN 6, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/resolution_visitor.dart:1834` em `ResolutionVisitor._verifyExtensionElementImplements` (começa em `:1787`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1794: if (!typeSystem.isValidExtensionTypeSuperinterface(type)) {
   1800: return;
   1804: if (typeSystem.isSubtypeOf(declaredRepresentation, type)) {
   1805: return;
   1809: if (type is InterfaceTypeImpl) {
   1811: if (implementedRepresentation != null) {
   1812: if (!typeSystem.isSubtypeOf(
   1828: return;
  ```

##### `instance_member_access_from_factory` — guardas do emissor (perda 6: FN 6, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:3974` em `ErrorVerifier._checkForInvalidGenerativeConstructorReference` (começa em `:3951`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   3953: if (constructorElement != null &&
   3956: if (_currentLibrary.featureSet.isEnabled(Feature.enhanced_enums)) {
   3961: } else {
  ```
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:4031` em `ErrorVerifier._checkForInvalidInstanceMemberAccess` (começa em `:3976`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   3995: return;
   3999: if (enclosingElement is! InterfaceElement &&
   4001: return;
   4005: if (parent is MethodInvocation) {
   4006: if (identical(parent.methodName, identifier) &&
   4008: return;
   4012: if (parent is PropertyAccess) {
   4013: if (identical(parent.propertyName, identifier)) {
   4014: return;
   4017: if (parent is PrefixedIdentifier) {
   4018: if (identical(parent.identifier, identifier)) {
   4019: return;
   4023: if (_enclosingExecutable.inStaticMethod) {
   4028: } else if (_enclosingExecutable.inFactoryConstructor) {
  ```
- **Fase e supressões gerais:** fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `super_formal_parameter_without_associated_named` — guardas do emissor (perda 6: FN 6, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:1487` em `ErrorVerifier.visitSuperFormalParameter` (começa em `:1457`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1460: if (_enclosingClass is ExtensionTypeElement) {
   1466: return;
   1470: if (!(constructor is ConstructorDeclaration &&
   1476: return;
   1482: if (superParameter == null) {
  ```
- **Fase e supressões gerais:** fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `wrong_number_of_type_arguments_method` — guardas do emissor (perda 6: FN 6, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/invocation_inferrer.dart:154` em `FullInvocationInferrer._wrongNumberOfTypeArgumentsErrorCode` (começa em `:153`).
  Nenhum desvio entre o início do método e o relato: o relato é incondicional dentro dele; a supressão está em quem o chama.

##### `doc_directive_argument_wrong_format` — guardas do emissor (perda 5: FN 5, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/doc_comment_verifier.dart:134` em `DocCommentVerifier.validateArgumentFormat` (começa em `:122`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    126: for (var i = 0; i < positionalArgumentCount; i++) {
  ```

##### `leaf_call_must_not_take_handle` — guardas do emissor (perda 5: FN 5, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/ffi_verifier.dart:1427` em `FfiVerifier._validateFfiLeafCallUsesNoHandles` (começa em `:1413`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1415: if (nativeType is FunctionType) {
   1416: if (_primitiveNativeType(nativeType.returnType) ==
   1423: for (var param in nativeType.normalParameterTypes) {
   1424: if (_primitiveNativeType(param) == _PrimitiveDartType.handle) {
  ```
- **Fase e supressões gerais:** fase 5d (`FfiVerifier`); só roda se a biblioteca usa `dart:ffi`.

##### `argument_must_be_native` — guardas do emissor (perda 3: FN 3, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/ffi_verifier.dart:1752` em `FfiVerifier._validateNativeAddressOf` (começa em `:1667`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1687: if (referencedElement != null) {
   1688: for (var annotation in referencedElement.metadata) {
   1692: if (annotationType is InterfaceType &&
   1696: if (nativeType is FunctionType) {
   1700: if (!targetType.isNativeFunction) {
   1706: } else {
   1709: if (!typeSystem.isAssignableTo(nativeType, targetFunctionType,
   1718: } else {
   1723: if (nativeType is DynamicType) {
   1725: if (staticType != null) {
   1728: if (canonical != null) {
   1734: if (!typeSystem.isAssignableTo(nativeType, targetType)) {
   1744: break;
   1749: if (!validTarget) {
  ```
- **Fase e supressões gerais:** fase 5d (`FfiVerifier`); só roda se a biblioteca usa `dart:ffi`.

##### `main_has_too_many_required_positional_parameters` — guardas do emissor (perda 3: FN 3, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:4165` em `ErrorVerifier._checkForMainFunction2` (começa em `:4143`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   4144: if (functionDeclaration.name.lexeme != 'main') {
   4145: return;
   4148: if (functionDeclaration.parent is! CompilationUnit) {
   4149: return;
   4153: if (parameterList == null) {
   4154: return;
   4162: if (requiredPositional.length > 2) {
  ```
- **Fase e supressões gerais:** fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `expected_two_map_type_arguments` — guardas do emissor (perda 2: FN 2, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/type_arguments_verifier.dart:187` em `TypeArgumentsVerifier.checkMapLiteral` (começa em `:175`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    177: if (typeArguments != null) {
    178: if (node.isConst) {
    179: for (var argument in typeArguments.arguments) {
  ```

##### `main_has_required_named_parameters` — guardas do emissor (perda 2: FN 2, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:4172` em `ErrorVerifier._checkForMainFunction2` (começa em `:4143`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   4144: if (functionDeclaration.name.lexeme != 'main') {
   4145: return;
   4148: if (functionDeclaration.parent is! CompilationUnit) {
   4149: return;
   4153: if (parameterList == null) {
   4154: return;
   4162: if (requiredPositional.length > 2) {
   4169: if (parameters.any((e) => e.isRequiredNamed)) {
  ```
- **Fase e supressões gerais:** fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `expected_one_set_type_arguments` — guardas do emissor (perda 1: FN 1, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/type_arguments_verifier.dart:220` em `TypeArgumentsVerifier.checkSetLiteral` (começa em `:208`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    210: if (typeArguments != null) {
    211: if (node.isConst) {
    212: for (var argument in typeArguments.arguments) {
  ```

##### `must_return_void` — guardas do emissor (perda 1: FN 1, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/ffi_verifier.dart:1835` em `FfiVerifier._validateNativeCallable` (começa em `:1759`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1768: return;
   1772: if (!_isValidFfiNativeFunctionType(typeArg)) {
   1778: return;
   1783: if (!_validateCompatibleFunctionTypes(
   1790: return;
   1794: if (isolateLocal) {
   1795: if (_primitiveNativeType(natRetType) == _PrimitiveDartType.void_ ||
   1799: if (argCount != 1) {
   1806: } else if (argCount != 2) {
   1812: } else {
   1815: if (!_validateCompatibleNativeType(
   1823: if (!_isConst(e)) {
   1831: } else {
   1832: if (_primitiveNativeType(natRetType) != _PrimitiveDartType.void_) {
  ```
- **Fase e supressões gerais:** fase 5d (`FfiVerifier`); só roda se a biblioteca usa `dart:ffi`.

##### `non_native_function_type_argument_to_pointer` — guardas do emissor (perda 1: FN 1, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/ffi_verifier.dart:1222` em `FfiVerifier._validateAsFunction` (começa em `:1195`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1198: if (typeArguments != null && typeArguments.length == 1) {
   1199: if (_validateTypeArgument(typeArguments[0], 'asFunction')) {
   1200: return;
   1205: if (targetType is InterfaceType && targetType.isPointer) {
   1207: if (!T.isNativeFunction) {
   1208: return;
   1211: if (pointerTypeArg is TypeParameterType) {
   1217: return;
   1219: if (!_isValidFfiNativeFunctionType(pointerTypeArg)) {
  ```
- **Fase e supressões gerais:** fase 5d (`FfiVerifier`); só roda se a biblioteca usa `dart:ffi`.


## B. Fluxo, nulidade, padrões, código morto

**Estado em 2026-10-04 (escrito, não compilado).** Além do T6 (§G), ganharam emissor nesta data:
em `crates/analise/src/c2_sintaticos.rs` (só a árvore), `break_label_on_switch_member`,
`duplicate_variable_pattern`, `duplicate_pattern_assignment_variable`, `duplicate_pattern_field`,
`rest_element_in_map_pattern`, `empty_map_pattern`, `expected_one_list_pattern_type_arguments`,
`expected_two_map_pattern_type_arguments`, `unnecessary_final` e
`late_final_field_with_const_constructor`; na inferência, `dead_code_catch_following_catch`,
`dead_code_on_catch_subtype` e `nullable_type_in_catch_clause` (`inferencia/instrucoes.rs`, no
`try`) e `not_null_aware_null_spread` (`inferencia/colecoes.rs`). Não feito:
`duplicate_rest_element_in_pattern` (o resto de um padrão de lista não guarda o intervalo), a
supressão de `dead_code` dentro de um `catch` já dado por morto, e os demais códigos sem emissor da
família (`body_might_complete_normally`, `switch_case_completes_normally`, `missing_variable_pattern`,
`refutable_pattern_in_irrefutable_context`, os `relational_pattern_*`, `unnecessary_nan_comparison`,
`unnecessary_question_mark`, `non_nullable_equals_parameter`, `unnecessary_set_literal` e outros).

### Especificação — família B (fluxo, nulidade, padrões) — r3-b

Fonte: `E:\references\dart-sdk-3.6.2\pkg` (analyzer 6.11 = SDK 3.6.2). Citações `analyzer/lib/...:linha` e
`_fe_analyzer_shared/lib/...:linha` desse diretório. Perdas de `E:\dftemp\analise\trab\familia-B.txt`
(colunas oráculo/nosso/acerto/msg/pos/FP/FN). Amostras de `placar-base-r3.txt` (ver com
`python E:/dftemp/r3-b/am.py <codigo>`).

Convenções usadas abaixo:
- **potencialmente anulável** = `!TypeSystemImpl.isNonNullable(T)` (`analyzer/lib/src/dart/element/type_system.dart:1194-1217`, `:1287`):
  `dynamic`, `InvalidType`, `_` (desconhecido), **`void`**, `Null`, `T?`, `FutureOr<S>` com `S` anulável, parâmetro de tipo sem
  limite não anulável (ou promovido para um anulável), tipo de extensão sem `implements` (representação qualquer) — todos são
  potencialmente anuláveis. No DartForge `e_nao_anulavel(t) = t <: Object` (`crates/types/src/inferencia/tipos.rs:59`) é a mesma
  coisa exceto pelo tipo de extensão (ver abaixo) e pelo inválido.
- **dynamic-bounded** = `isDynamicBounded` (`type_system.dart`): `dynamic`, ou parâmetro de tipo/interseção cujo limite
  (transitivamente) é dynamic-bounded. Idem **invalid-bounded**.
- O fluxo de padrões do analyzer (`_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart`) é descrito uma vez em
  **§ Apêndice A** (usado por `dead_code`, `constant_pattern_never_matches_value_type`, `unnecessary_*_pattern` etc.).
- O verificador de código morto do analyzer é descrito em **§ Apêndice B** (usado por `dead_code`).


#### Índice

| código | perda | FN | FP | msg | pos | seção |
|---|---|---|---|---|---|---|
| `unchecked_use_of_nullable_value` | 178 | 151 | 12 | 0 | 15 | §1 Nulidade de receptor |
| `dead_code` | 127 | 118 | 5 | 0 | 4 | §2 Código morto |
| `referenced_before_declaration` | 29 | 29 | 0 | 0 | 0 | §3 Referência antes da declaração |
| `pattern_type_mismatch_in_irrefutable_context` | 22 | 22 | 0 | 0 | 0 | §4 Padrões |
| `relational_pattern_operand_type_not_assignable` | 18 | 18 | 0 | 0 | 0 | §4 Padrões |
| `not_initialized_non_nullable_instance_field` | 15 | 12 | 2 | 0 | 1 | §6 Nulidade e verificadores |
| `pattern_variable_assignment_inside_guard` | 15 | 15 | 0 | 0 | 0 | §4 Padrões |
| `unnecessary_cast_pattern` | 13 | 13 | 0 | 0 | 0 | §4 Padrões |
| `invalid_use_of_null_value` | 12 | 12 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `missing_variable_pattern` | 12 | 12 | 0 | 0 | 0 | §4 Padrões |
| `unnecessary_null_check_pattern` | 12 | 12 | 0 | 0 | 0 | §4 Padrões |
| `unnecessary_set_literal` | 12 | 12 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `assignment_to_final_local` | 11 | 11 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `invalid_pattern_variable_in_shared_case_scope` | 11 | 11 | 0 | 0 | 0 | §4 Padrões |
| `late_final_field_with_const_constructor` | 10 | 10 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `switch_case_completes_normally` | 10 | 10 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `unnecessary_wildcard_pattern` | 10 | 10 | 0 | 0 | 0 | §4 Padrões |
| `body_might_complete_normally` | 8 | 8 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `dead_null_aware_expression` | 8 | 6 | 2 | 0 | 0 | §6 Nulidade e verificadores |
| `equal_keys_in_map_pattern` | 8 | 8 | 0 | 0 | 0 | §4 Padrões |
| `not_assigned_potentially_non_nullable_local_variable` | 8 | 8 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `not_initialized_non_nullable_variable` | 8 | 8 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `pattern_never_matches_value_type` | 8 | 5 | 2 | 1 | 0 | §4 Padrões |
| `unnecessary_null_comparison` | 8 | 8 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `unnecessary_type_check` | 8 | 3 | 5 | 0 | 0 | §6 Nulidade e verificadores |
| `non_nullable_equals_parameter` | 7 | 7 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `null_argument_to_non_null_type` | 7 | 7 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `pattern_assignment_not_local_variable` | 7 | 7 | 0 | 0 | 0 | §4 Padrões |
| `unnecessary_null_assert_pattern` | 7 | 7 | 0 | 0 | 0 | §4 Padrões |
| `cast_from_null_always_fails` | 6 | 6 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `duplicate_pattern_field` | 6 | 6 | 0 | 0 | 0 | §4 Padrões |
| `not_null_aware_null_spread` | 6 | 6 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `unnecessary_nan_comparison` | 5 | 5 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `body_might_complete_normally_nullable` | 4 | 4 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `break_label_on_switch_member` | 4 | 4 | 0 | 0 | 0 | §4 Padrões |
| `case_expression_type_is_not_switch_expression_subtype` | 4 | 4 | 0 | 0 | 0 | §4 Padrões |
| `duplicate_pattern_assignment_variable` | 4 | 4 | 0 | 0 | 0 | §4 Padrões |
| `null_check_always_fails` | 4 | 4 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `nullable_type_in_catch_clause` | 4 | 4 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `nullable_type_in_implements_clause` | 4 | 4 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `refutable_pattern_in_irrefutable_context` | 4 | 4 | 0 | 0 | 0 | §4 Padrões |
| `unnecessary_question_mark` | 4 | 4 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `body_might_complete_normally_catch_error` | 3 | 3 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `dead_code_on_catch_subtype` | 3 | 3 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `definitely_unassigned_late_local_variable` | 3 | 1 | 0 | 0 | 2 | §5 Fluxo e atribuição definitiva |
| `duplicate_variable_pattern` | 3 | 3 | 0 | 0 | 0 | §4 Padrões |
| `unnecessary_final` | 3 | 3 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `cast_from_nullable_always_fails` | 2 | 2 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `expected_two_map_pattern_type_arguments` | 2 | 2 | 0 | 0 | 0 | §4 Padrões |
| `late_final_local_already_assigned` | 2 | 1 | 1 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `read_potentially_unassigned_final` | 2 | 2 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `relational_pattern_operator_return_type_not_assignable_to_bool` | 2 | 2 | 0 | 0 | 0 | §4 Padrões |
| `rest_element_in_map_pattern` | 2 | 2 | 0 | 0 | 0 | §4 Padrões |
| `unnecessary_no_such_method` | 2 | 2 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `dead_code_catch_following_catch` | 1 | 1 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `duplicate_rest_element_in_pattern` | 1 | 1 | 0 | 0 | 0 | §4 Padrões |
| `empty_map_pattern` | 1 | 1 | 0 | 0 | 0 | §4 Padrões |
| `expected_one_list_pattern_type_arguments` | 1 | 1 | 0 | 0 | 0 | §4 Padrões |
| `invalid_null_aware_operator` | 1 | 1 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `non_exhaustive_switch_expression` | 1 | 0 | 0 | 1 | 0 | §4 Padrões |

#### Resumo e ordem sugerida de implementação

1. **`unchecked_use_of_nullable_value` + `invalid_use_of_null_value` (~190):** mecanismo `NullableDereferenceVerifier` para
   iterável/espalhamento/`yield*` (68+8), invocação de valor anulável (38), prefixo/posfixo/composto (~17+5), `this` implícito
   em extensão, escrita em propriedade, campo de padrão objeto; e as supressões que hoje dão FP (`new`, `T extends dynamic`,
   `super` em extensão). Implementação de teste já escrita (revertida): `E:\dftemp\r3-b\meu-instrucoes.diff` e as chamadas em
   `colecoes.rs`/`chamadas.rs`/`expr.rs` descritas no §1.
2. **Fluxo de padrões (Apêndice A) (~100 em vários códigos):** referência do valor casado + `_unmatched`; destrava
   `dead_code` de padrões (~46), `constant_pattern_never_matches_value_type` (casos com tipo promovido), `pattern_never_matches_value_type`,
   `unnecessary_null_check/assert_pattern`, `unnecessary_wildcard_pattern`, `unnecessary_cast_pattern` (tipo casado certo).
3. **`dead_code` no nível de expressão (Apêndice B) (~40):** gancho de "nó inalcançável" em toda expressão e na lista de
   argumentos; operando direito de binário começa no operador; `?.` sobre `Null`; variáveis de condição com literal booleano
   (18); elementos de coleção; `is` com fator `Never` não torna o ramo falso inalcançável (FP).
4. **Códigos de padrão não implementados (~110 somados):** `pattern_type_mismatch_in_irrefutable_context`,
   `relational_pattern_operand_type_not_assignable`, `pattern_variable_assignment_inside_guard`, `missing_variable_pattern`,
   `invalid_pattern_variable_in_shared_case_scope`, `equal_keys_in_map_pattern`, `pattern_assignment_not_local_variable`,
   `duplicate_*`, `refutable_pattern_in_irrefutable_context`… — cada um local em `padroes::tipar`.
5. **`referenced_before_declaration` (29, publicado):** pré-escaneamento de `PatternVariables` no bloco e dos corpos de
   `case`; nome de tipo que é local adiante; variáveis de padrão referidas no próprio padrão.
6. **Fluxo/atribuição:** `for (x in …)` com `x` final (sempre relata), formais `this.x`/`super.x` são `final`, `??=` escreve só
   no ramo nulo (FP de `late_final_local_already_assigned`), leitura em código inalcançável mantém o estado de atribuição,
   `T` sem limite é potencialmente não anulável; `body_might_complete_normally*` e `switch_case_completes_normally` (legado)
   não implementados.
7. **Achado transversal:** escrita de valor de tipo inválido promove o local ao tipo de interesse não anulável (§6).

#### §1 Nulidade de receptor

##### `unchecked_use_of_nullable_value` (perda 178: FN 151, FP 12, msg 0, pos 15)

Nome compartilhado de 8 códigos (`analyzer/messages.yaml`, `CompileTimeErrorCode.UNCHECKED_*`):
`UNCHECKED_INVOCATION_OF_NULLABLE_VALUE` ("The function can't be unconditionally invoked because it can be 'null'.", sem
argumentos), `UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE` ("The method '{0}' …"), `UNCHECKED_OPERATOR_INVOCATION_OF_NULLABLE_VALUE`
("The operator '{0}' …"), `UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE` ("The property '{0}' …"), `…_AS_CONDITION`,
`…_AS_ITERATOR`, `…_IN_SPREAD`, `…_IN_YIELD_EACH` (sem argumentos). `{0}` é o nome do membro como String (`'unary-'`, `'[]'`,
`'+'` para `++`, `'-'` para `--`).
- **Feito em 2026-10-07 (FP 16 → 6):** (a) o parâmetro de tipo de limite `dynamic` (`isDynamicBounded`,
  `type_property_resolver.dart:81-90`) não exige checagem de nulo e só acha membros de `Object`, o resto é
  dinâmico (`field1 * field2` com `T extends dynamic`); (b) o receptor de `?.`/`?..`/`?[` é o
  `promoteToNonNull` do alvo (o parâmetro de tipo de limite anulável vira `T & NonNull(limite)`); (c) a
  promoção pelo alvo de cascata e `x.new` (ver INFERENCIA §8.6 e `undefined_setter`).

- **Emissão (dois mecanismos):**
  1. `NullableDereferenceVerifier.expression` → `_check` (`analyzer/lib/src/error/nullable_dereference_verifier.dart:33-37`,
     `:72-90`): relata no **nó da expressão inteira**. Chamado por:
     - `for-in` comum: `ForResolver._forEachParts` (`analyzer/lib/src/dart/resolver/for_resolver.dart:164-169`), depois de
       inferir o iterável com contexto `Iterable<T>`/`Stream<T>`; `for` com padrão: `_analyzePatternForIn` (`:85-91`). Vale
       para instrução e para elemento de coleção (o mesmo resolvedor).
     - espalhamento sem `?`: `ResolverVisitor.visitSpreadElement` (`analyzer/lib/src/generated/resolver.dart:3673-3692`,
       só `if (!node.isNullAware)`).
     - `yield*`: `YieldStatementResolver._resolve_generator` (`analyzer/lib/src/dart/resolver/yield_statement_resolver.dart:171-175`,
       só com `star`).
     - invocação de expressão cujo tipo é `FunctionType`: `FunctionExpressionInvocationResolver.resolve`
       (`analyzer/lib/src/dart/resolver/function_expression_invocation_resolver.dart:52-58`) → `UNCHECKED_INVOCATION…` no nó
       `function` (ex.: `f()` com `f` local; `this._f()` → `this._f`; `super._f()` → `super._f`; `x.first()` → `x.first`).
       Antes: `_checkForUseOfVoidResult` (`:47`, `void` sai como `use_of_void_result` e mais nada).
     - condição: `BoolExpressionVerifier` (`analyzer/lib/src/error/bool_expression_verifier.dart:61`, já implementado).
     `_check` (`:77-80`): sai sem relatar se o tipo é `DynamicType`, `InvalidType` ou **não** potencialmente anulável.
     `report` (`:39-64`): se o tipo do receptor é exatamente `Null` (`typeProvider.nullType`; `Never?` NÃO é) troca para
     `INVALID_USE_OF_NULL_VALUE` sem argumentos.
  2. `TypePropertyResolver.resolve` (`analyzer/lib/src/dart/resolver/type_property_resolver.dart:62-179`), chamado em toda
     busca de membro: invocação de método (`method_invocation_resolver.dart:805`, entidade = nome), leitura/escrita de
     propriedade (`property_element_resolver.dart:481`, entidade = nome da propriedade), índice (`property_element_resolver.dart:109`,
     entidade = `[`), binário (`binary_expression_resolver.dart:434`, entidade = operador), prefixo `-x ~x ++x --x`
     (`prefix_expression_resolver.dart:185-191`, entidade = token do operador), posfixo `x++ x--`
     (`postfix_expression_resolver.dart:136-141`, entidade = operador), atribuição composta `x += 1`
     (`assignment_expression_resolver.dart:241-247`, entidade = operador), `this` implícito (`this_lookup.dart:28`, `:69`,
     receptor `null`, entidade = o identificador), `call` de valor não-função (`function_expression_invocation_resolver.dart:73-79`,
     entidade = `function`), padrão relacional (`generated/resolver.dart:1627-1634`, entidade = operador, `parentNode: node`),
     campo de padrão objeto (`generated/resolver.dart:1574-1580`, receptor `null`, entidade = `objectPattern.type`), referência
     a função (`function_reference_resolver.dart:683`).
- **Condição exata** (`type_property_resolver.dart`):
  ```
  if name == 'new' → needsGetterError, sem relato de nulo (:75-79)
  if isDynamicBounded(R) || isInvalidBounded(R) → busca em Object, sem erro (:81-91)
  isNullable = R.isExtensionType ? R.nullabilitySuffix == '?' : isPotentiallyNullable(R)   (:93-97)
  if isNullable:
     acha em Object (getter/setter/método do nome)  → OK, sem relato (:100-103)
     acha em extensão aplicável a R (o tipo anulável) → OK (:105-108)
     parentNode ??= receiver?.parent ?? (propertyErrorEntity as AstNode).parent   (:110-122)
     if parentNode is CascadeExpression: parentNode = cascadeSections.first        (:128-130)
     código = parentNode is BinaryExpression|RelationalPattern → OPERATOR [name]  (:131-134)
              parentNode is MethodInvocation|MethodReferenceExpression → METHOD [name] (:135-139)
                 (MethodReferenceExpression = Assignment, Index, Prefix, Postfix, ImplicitCallReference — ast.dart:850,9789,10128,14230,14441)
              parentNode is FunctionExpressionInvocation → INVOCATION []          (:140-143)
              senão → PROPERTY [name]                                             (:144-147)
     nullableDereferenceVerifier.report(código, propertyErrorEntity, R)  (Null → INVALID_USE_OF_NULL_VALUE)  (:165-166)
     recuperação: R = resolveToBound(R); se interface, busca nela (:172-176) — por isso não sai undefined_* depois
  ```
  `void` é potencialmente anulável: `-x` com `x` `void` dá `UNCHECKED_METHOD…('unary-')` no `-` **e** `use_of_void_result` no
  operando (amostra `use_of_void_result/UseOfVoidResult__prefixExpression_minus_2247a990.dart`, `void/void_type_usage_test.dart:180`).
  Leitura de propriedade/invocação em `void` sai antes como `use_of_void_result` (outros caminhos), sem este código.
  `Never` (não anulável) vai para `RECEIVER_OF_TYPE_NEVER` antes (`postfix…:128`, `prefix…:176`); `Never?` não é `Never` nem
  `Null` → este código (amostras `receiver_of_type_never/InvalidUseOfNever__*neverQ*`).
- **Posição:** o `propertyErrorEntity` acima (nome do método/propriedade; `[` do índice, inclusive em cascata `x..[0] = 1`;
  token do operador em binário/prefixo/posfixo/composto; identificador do `this` implícito; tipo do padrão objeto
  `case A(foo: 0)` → `A`); para o mecanismo 1, a expressão inteira (iterável, operando do espalhamento, valor do `yield*`,
  função invocada).
- **Mensagem:** só nome (String); nenhuma formatação de tipo.
- **Supressões e ordem:** `name == 'new'`; receptor `dynamic`/inválido/dynamic-bounded (inclusive `T extends dynamic`);
  `Object` ou extensão aplicável ao tipo anulável tem o membro; sobreposição explícita de extensão (`E(x).m`, não passa por
  aqui); `super` numa extensão já é `super_in_extension` com tipo inválido (sem este código); receptor `Never`.
- **No DartForge:**
  - Mecanismo 2 existe parcialmente em `BodyInferrer::exige_checagem_de_nulo` (`crates/types/src/inferencia/membros.rs:343-356`),
    usado em `expr::propriedade` (leitura), `chamadas::chamada` (método), `expr::operador_binario` (binário e índice),
    `expr::verificar_bool` (condição). Causas das divergências:
    - **FN iterável/espalhamento/`yield*` (68):** mecanismo 1 não implementado. Entrar em `instrucoes::cabecalho_for_in`
      (logo depois de `inferir` do iterável — serve instrução, elemento de coleção e `for` com padrão), em
      `colecoes::visitar` ramo `CollectionElement::Spread` quando `!null_aware`, e em `instrucoes` `StmtKind::Yield{star:true}`.
      Regra: `t` não `dynamic`, não desconhecido, não inválido e `!e_nao_anulavel(t)` → `aviso_de_nulo(t, código, span da expressão)`.
      Amostras: `patterns/for_in_nullable_error_test.dart:9:19`, `why_not_promoted/nullable_spread_error_test.dart`,
      `nnbd/operator_type_test.dart:115:12`. A confirmar com a sonda: `for (x in v)` com `v` `void` (o analyzer conta `void`
      como anulável aqui, sem `_checkForUseOfVoidResult` antes no `for`). (Implementação de teste já escrita e guardada em
      `E:\dftemp\r3-b\meu-instrucoes.diff`, revertida do repositório.)
    - **FN invocação de função anulável (38):** `chamadas::invocar_valor` não relata. Regra: se o tipo `t` do alvo é função
      (após tirar `?`) e potencialmente anulável → relatar no span do alvo (`ExprKind::Call.target`), código INVOCATION;
      se não é função e é potencialmente anulável (`Function?`, `C?` com `call`, `T` com limite anulável, `Never?`, `Null`) e
      nenhuma extensão aplicável ao tipo anulável tem `call` → idem. Amostras `inference_update_2/super_this_distinction_error_test.dart:26:5`
      (`_f()`), `:31:5` (`this._f()`, length 7), `:36:5` (`super._f()`), `use_of_nullable_value/UncheckedUseOfNullableValue__member_pot_7cd99204.dart`
      (`x.first()` com `T?`), `receiver_of_type_never/InvalidUseOfNever__functionExpressionIn_d97a3872.dart` (`Never?`),
      `why_not_promoted/nullable_expression_call_error_test.dart` (5).
    - **FN prefixo/posfixo/composto (~17):** `expr::unario` (`UnaryOp::Neg/BitNot` e `PrefixInc…PostfixDec`) não checa nulo.
      Regra: depois de inferir o operando (leitura), se não há sobreposição de extensão no operando, tipo não `dynamic`/`Never`/
      desconhecido e potencialmente anulável **incluindo `void`**, e nem `Object` nem extensão aplicável ao tipo anulável tem o
      operador → `aviso_de_nulo(t, METHOD, token, [nome])`, token = `[início, início+len)` (prefixo: 1 para `-`/`~`, 2 para
      `++`/`--`) ou `[fim-2, fim)` (posfixo); a busca segue no tipo não anulável (recuperação). Amostras
      `UncheckedUseOfNullableValue__operatorPo_020f2a12.dart` (`x++`), `null_aware/prefix_not_shortening_test.dart:13:3`
      (`-c?.e()`: o `?.` não encurta através do prefixo), `void/void_type_usage_test.dart:180:3` (6 vezes nesse arquivo),
      `null_safety_read_write/ReadWrite__lateFinalNullable_*` (4), `super/conditional_operator_test.dart:48:5`.
      Atribuição composta (`x += 1`, `a.b += 1`, `a[i] += 1`): conferir `expr::atribuicao` — o operador usa METHOD
      (pai é `AssignmentExpression`), hoje `PosicoesDeOperador.composta`.
    - **FN `this` implícito em extensão sobre tipo anulável (5):** `RefNome::ThisImplicito` (`expr.rs`, leitura e escrita) não
      passa por `exige_checagem_de_nulo`. Código pelo pai do identificador: chamada `foo()` → METHOD; `foo = 0`
      (pai `AssignmentExpression`) → METHOD; `foo;` → PROPERTY. Amostras `UncheckedUseOfNullableValueInsideExtens_6b6a2f6f.dart:9:5`
      (`foo = 0` → "The method 'foo'"), `_c616a27b.dart:9:5`, `_8231c303.dart:7:5`, `_7fd67550.dart:14:9` (`this[0]` → `[`),
      `_2fa23de1.dart:7:5` (`-this`, prefixo).
    - **FN escrita em propriedade de receptor anulável:** `b.a.x = 2` (`UncheckedUseOfNullableValue__assignment_b2ba37a8.dart:13:7`)
      — o caminho de escrita (`expr::atribuicao` → alvo `Property`) não chama `exige_checagem_de_nulo(…, setter=true)`.
      PROPERTY no nome (pai `PropertyAccess`). Cascata `x..[0] = 1` (`…__cascade_nu_b9551aeb.dart:2:6`): METHOD `'[]'` no `[`.
    - **FN padrão objeto sobre tipo anulável:** `case A(foo: 0)` com `A` alias de tipo anulável ou `T?` — em
      `padroes::tipar` `PatternKind::Object` a busca dos campos não checa nulo. PROPERTY [campo] no span do tipo do padrão.
      Amostras `patterns/issue52202_error_test.dart:11:10`, `patterns/nullable_object_pattern_error_test.dart:13:10`, `:25:10`.
    - **FN de inferência (tipo do receptor errado no DartForge; consertar a inferência, não este código):**
      `class/override_inference_error_test.dart:237` (parâmetro sem tipo herdando `Q` → `Object?`),
      `extension_type/combined_member_signature_error_test.dart:46:35` (campos de registro da assinatura combinada →
      `Object?`), `primary_constructors/type_inference/extension_type_error_test.dart:16:20` (representação sem tipo = `Object?`),
      `records/type_inference_error_test.dart:132:20`, `:139:20` (`(d,)` com `d` dynamic em contexto `(List<_>,)` → `List<Object?>`),
      `extension_methods/static_extension_this_not_promoted_error_test.dart:21:9` (**`this` nunca é promovido** por
      `this != null`; o DartForge promove — `expr::alvo_de_promocao`/`Base::This`).
    - **FP (12):** `t.new` (`UndefinedGetter__new_*`: nome `new` nunca exige checagem); `super + 1` numa extensão
      (`SuperInExtension__binaryOperator_withGe_2100d7fc.dart`: `super` tem tipo inválido depois de `super_in_extension`);
      `T extends dynamic` (`type_variable/bound_access_test.dart:11:22`: dynamic-bounded não exige checagem);
      local inicializado de expressão de tipo inválido (`InvalidNullAwareOperator__invalid_nonNullable.dart:5:5`: `int? i =
      o.nonNull` com `o` de classe indefinida — a variável fica promovida ao tipo inválido e nada sai depois);
      cascata `c?.._field.f(…)` (`inference_update_2/cascaded_field_promotion_unnecessary_null_aware_error_test.dart:107:15`:
      o alvo da cascata guarda a promoção do campo, mesmo com escrita em `c` dentro da cascata).
    - **Conferido com o oráculo vivo** (`E:\dftemp\r3-r3-b\c1\a.dart`): `for (final x in n)` com `n: Null` →
      `invalid_use_of_null_value` (3:19); `<int,int>{...null}` → `invalid_use_of_null_value` **e** `not_null_aware_null_spread`
      no `null`; `yield* l` com `l` anulável → este código **e** `yield_of_invalid_type` no mesmo nó; `f()` (1 col), `(f)()`
      (3 cols, os parênteses entram), `c.cb()` (4), `c.gg()` getter (4), `cc()` com `C?` que tem `call` (2), `(c.cb)()` (6);
      `x++`/`++x` com `int?` → METHOD '+' no `++` (e a escrita promove `x` para `int`, por isso `-x` logo depois não relata);
      `-this` numa extensão em `int?` → METHOD 'unary-'; `foo()` implícito → METHOD 'foo'; `foo;` implícito → PROPERTY 'foo'
      **e também `undefined_identifier`** quando o tipo não anulável também não tem o membro (o `this_lookup` não acha getter
      e o `SimpleIdentifierResolver` relata o nome indefinido; na invocação não há `undefined_method`).
    - **pos (15) + FP top_merge (6):** `nnbd/top_merge/*_error_test.dart`: a interface de `A` numa classe que implementa `A<X>`
      e `A<Y>` com `X`, `Y` mutuamente subtipos é `NNBD_TOP_MERGE(NORM(X), NORM(Y))` (`analyzer/lib/src/dart/element/class_hierarchy.dart:116-212`,
      `top_merge.dart:27`); o DartForge escolhe uma das duas e erra o tipo de `deconstruct(this)` (sai `dynamic`/`void` onde é
      `Object?` e vice-versa). Não é deste código: consertar no cálculo das superinterfaces (crates/types, hierarquia).
      As 3 `posição` de `nnbd/operator_type_test.dart:98/103/110` são o mesmo FN de iterável/espalhamento pareado com outra linha.
- **Estado (2026-10-06):** operadores de prefixo e sufixo (`-x`, `~x`, `++x`, `x--`) relatam no token do operador
  (INFERENCIA §8.5); `this._f()` com `_f` promovido usa o tipo promovido da propriedade (o `PropertyAccess` da
  `FunctionExpressionInvocation`). 356 de 384, FP 15.

- **Correções de 2026-10-06 (375/384, FN 28 para 9):** quatro pontos do `TypePropertyResolver` (`type_property_resolver.dart:80-150`: receptor potencialmente anulável, sem o membro em `Object` nem extensão aplicável, relata sempre, e o código sai do pai do receptor) que faltavam:
  - operador unário (`-x`, `~x`, `++x`) em `void`: `void` é potencialmente anulável e não é limitado por `dynamic`, então sai `UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE` no operador além do `USE_OF_VOID_RESULT` no operando (`expr.rs::nulo_em_unario`; `exige_checagem_de_nulo` continua falso para `void` nos outros usos);
  - chamada de valor que não é tipo de função (`F?` com `F extends Function`, `Function?`, classe com `call` anulável): `FunctionExpressionInvocationResolver.resolve` (`function_expression_invocation_resolver.dart:72-78`) passa o `call` ao `TypePropertyResolver`, cujo pai é `FunctionExpressionInvocation`: `UNCHECKED_INVOCATION_OF_NULLABLE_VALUE` na função, com as mensagens de não promoção (`chamadas.rs::invocar_valor`);
  - padrão relacional fora da igualdade: `UNCHECKED_OPERATOR_INVOCATION_OF_NULLABLE_VALUE` no operador (o pai é `RelationalPattern`), em `padroes.rs::padrao_relacional`;
  - escrita indexada (`x[0] = 1`, `x..[0] = 1`, `Never?`): `resolveIndexExpression` (`property_element_resolver.dart:80-115`) usa o nome `[]` (procurando `[]` e `[]=`), e o `IndexExpression` é `MethodReferenceExpression`: `UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE` `['[]']` no `[` (`expr.rs::escrita_indice`).
- **Restam:** a promoção de campo do alvo de cascata (`c?.._field()` depois de `c._field as int Function()`, `cascaded_field_promotion_unnecessary_null_aware_error_test.dart`), membros de extensão por `this` implícito, `call` de extensão, padrões objeto anuláveis e records (`records/type_inference_error_test.dart`).
#### §2 Código morto
- **Estado em 2026-10-07 (placar 380/384):** três correções.
  * O campo de padrão objeto sobre tipo potencialmente anulável (`case A(isEven: true)` com `typedef A = int?`)
    relata `UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE` no tipo do padrão e busca no tipo sem `?`. É o
    `resolveObjectPatternPropertyGet` com `propertyErrorEntity` no tipo (`resolver.dart:1562-1580`).
  * O `super` dentro de `extension` tem tipo inválido (`static_type_analyzer.dart:240-252`). Nada sai de
    `super + 1` além do `super_in_extension`.
  * O campo `dynamic` de record com contexto conhecido vale o fecho maior do contexto
    (`record_literal_resolver.dart:135-148`, INFERENCIA §3.4).
  * Restam: a promoção separada de `super._f` (`inference_update_2/super_this_distinction*`), o `toRadixString` de
    `nnbd/operator_type_test`, os operadores prefixos sobre `super` em `super/conditional_operator_test` e a
    invocação de valor `Never?`.

##### `dead_code` (perda 127: FN 118, FP 5, msg 0, pos 4)
- **Ramo morto dentro das atualizações mortas (2026-10-08):** `for (; false; true ? x : x) {}` relata o corpo e
  as atualizações (`_reportForUpdaters`) e nada dentro delas: o trecho morto das atualizações fica aberto
  enquanto elas são inferidas. `instrucoes.rs` o abria com o sentinela `usize::MAX`, que o `sair_fluxo` do ramo
  `then` do `?:` fechava (fecha todo trecho de profundidade `>=` à corrente), e o `else` morto abria outro
  relato (`void/void_type_usage_test.dart:1215:40`). Agora o trecho fica na profundidade corrente.
- **Cascata `?..` com alvo `Null` (2026-10-08):** o `nullAwareAccess_rightBegin` marca o temporário do alvo como não
  nulo (`tryMarkNonNullable`); de `Null` sai `Never`, e as seções ficam inalcançáveis desde o começo; o fim da
  cascata junta com o caminho do atalho nulo e volta a alcançar. O primeiro nó morto é a primeira seção, que
  começa no operador `?..`/`..` (`null?..(_) => 1;`: 2:7, 12, até o `;`). No DartForge, o braço da cascata
  (`expr.rs`) faz o inalcançável antes das seções, relata a seção a partir do operador e junta no fim.

**Feito em 2026-10-07 (247/255):** (a) o `default` que não é o último membro do `switch`: o `parseSwitchBlock`
(`parser_impl.dart:8975-9030`) mantém o `defaultKeyword` nos grupos seguintes e o `AstBuilder.endSwitchCase`
(`ast_builder.dart:3210-3247`) acrescenta ao fim de cada grupo de `case` depois dele um `SwitchDefault` com a
**mesma palavra** e as instruções do grupo; o `default` original casa sempre, os membros seguintes ficam
inalcançáveis e o `flowEnd` de cada um relata na palavra dele (`dead_code_verifier.dart:228-235`): o `case`
e, pela cópia, o `default` original (`switch/switch1_test.dart` 13:5 e 14:5). No DartForge, o parser emite o
membro sintético (`parse_switch_cases`). (b) A condição do `do … while` como primeiro nó morto (o corpo não
completa): o `flowEnd` com pai `DoStatement` (`dead_code_verifier.dart:281-301`) relata do `do` ao `{` do corpo
bloco (ou só o `do`), e do `}` do corpo bloco (ou do `while`) ao `;`; o resto vai do token depois do `;` ao
fim do nó que fecha o fluxo (o membro do `switch` pelo `handleMergedStatementCase`), e não sai com `break` no
laço (`hasBreakStatement`). No DartForge, `instrucoes.rs`, braço `DoWhile`.

**Estado em 2026-10-05 (escrito, não compilado), a variante `DEAD_CODE_LATE_WILDCARD_VARIABLE_INITIALIZER`:**
o `DeadCodeVerifier.visitVariableDeclaration` (`dead_code_verifier.dart:114-128`) está em
`instrucoes::declaracao_de_variaveis`: local `late` chamado `_` com curingas (3.7) e inicializador relata no
inicializador, com a mensagem própria do código.
- **Feito em 2026-10-07 (241/255):** (a) `for (init; c; u)` com a inicialização que não completa: o primeiro nó
  morto é a condição, e o `flowEnd` das partes vai dela ao fim da última atualização
  (`dead_code_verifier.dart:309-310`); sem atualizações o `updaters.last` lança e o verificador não relata mais
  nada no corpo (o trecho fica aberto até o fim dele); (b) `x?.m(args)` com o lado direito morto começa na lista
  de argumentos (o `(`); (c) `for (var x in e)` com `e` que não completa (`Never`) começa na variável do laço.

- **Emissão:** `WarningCode.DEAD_CODE` ("Dead code.", sem argumentos; correção "Try removing the code, or fixing the code
  before it so that it can be reached."), só pelo `NullSafetyDeadCodeVerifier` durante a resolução — modelo completo no
  **Apêndice B** (`analyzer/lib/src/error/dead_code_verifier.dart:186-475`). Também `_verifyUnassignedSimpleIdentifier`
  (`:443-475`). (O `DeadCodeVerifier` antigo de `:28-184` só cuida de rótulos/`ignore`, não emite este código em código
  null-safe.)
- **Condição exata:** trecho [primeiro nó inalcançável, fim do nó que fecha o bloco básico] (Apêndice B); a alcançabilidade é a
  da análise de fluxo (inclui expressões `Never`, variáveis de condição, `?.` sobre `Null` e padrões — Apêndice A).
- **Posição:** offset do primeiro nó morto (ou do operador `&&`/`||`/padrão `||`; ou o token `case`/`default`), até o fim do
  nó que fecha, aparado na última instrução do bloco/membro. Casos especiais: `do-while`, inicializador de construtor,
  atualizadores de `for` (`_reportForUpdaters`), mensagem de `assert` (nunca relatada).
- **Mensagem:** fixa.
- **Supressões e ordem:** um trecho por vez (enquanto `_firstDeadNode` existe, nada novo abre); dentro de `catch` morto já
  relatado (`dead_code_on_catch_subtype`/`…_following_catch`) não sai; sem fluxo (código fora de corpo) não sai.
- **No DartForge:** `Corpo::trecho_morto`/`fins_de_fluxo`/`origem_do_morto` (`crates/types/src/inferencia/corpo.rs`),
  aberto só no nível de **instrução** (`instrucoes::inferir_instrucao` `StmtKind::Block`, `ramo_de_fluxo`) e em
  `expr::operando_de_fluxo` (ramos do condicional e operando direito de `&&`/`||`); fechado por `instrucoes::sair_fluxo`.
  Grupos de FN (118) e a mudança de cada um:
  1. **Expressão `Never` no meio de uma expressão (~24):** `x.foo(1 + 2)`/`x()`/`x[0]`/`x[0] += 1`/`x.foo = 0`/`(throw '') + (1+2)`
     com `x: Never` (`receiver_of_type_never/InvalidUseOfNever__*`, `void/issue_flutter_161479_test.dart:26:13`,
     `nnbd/never/never_error_test.dart:73:26`). Falta o gancho de "nó inalcançável" no nível de expressão: ao começar a
     inferir **qualquer** expressão (`expr::inferir_no`) e **a lista de argumentos** de qualquer invocação (antes do primeiro
     argumento; offset = `Arguments.span.start`, inclusive em todos os caminhos de recuperação de `chamadas.rs` que fazem
     `inferir_livre` dos argumentos), se `!fluxo.alcancavel && trecho_morto.is_none()`: abrir trecho de `início` até
     `fins_de_fluxo.last()` (o fim aparado do bloco básico de fora) e marcar `trecho_morto`. Exceção: operando direito de
     binário (não lógico) começa no **operador** (`x + (1+2)` → `+ (1 + 2);`; `x == 1 + 2` → `== 1 + 2;`). Amostras:
     `InvalidUseOfNever__methodInvocation_never.dart` (25, 8: `(1 + 2);`), `__indexExpression_never_read.dart` (22, 3: `0];`),
     `__binaryExpression_plus.dart` (24, 9), `__functionExpressionIn_cc1f7897.dart` (`x();` → `();`).
  2. **`?.`/`?[` com alvo do tipo `Null` (6):** o lado direito é inalcançável (`nullAwareAccess_rightBegin`,
     `flow_analysis.dart:5041-5052`): `n?[i]`, `n?.foo(i)`, `n?.p = i`, `(n)?.p = i`, `x?[0]` com `x: Never?`… Trecho a partir
     do primeiro nó do lado direito (índice, argumentos, valor atribuído) até o fim do bloco básico. Amostras
     `dead_code/DeadCode__nullAware*.dart`, `InvalidUseOfNever__indexExpression_neve_ca39e5f4.dart`. No DartForge as cadeias
     (`Corpo::cadeias`, `expr::fechar_cadeia`) não tornam o fluxo inalcançável quando o alvo é `Null`.
  3. **Variável de condição com literal booleano (18):** `bool x = true; x || false` (`|| false` morto), `!x`, `Object b =
     true; b ? 1 : 2` (`2` morto), `bool condition = true; condition ? A() : B()` (10 em `variance/variance_upper_lower_bounds_error_test.dart`),
     `for (…; b; i++)`/`while (b)` (`implicit_downcast_during/*`), `const isTrue = true; if (isTrue) … else …`
     (`NonConstantMapElement__ifElementWithEls_988e819a.dart`). Regra (`_initialize` `flow_analysis.dart:5931-5960`,
     `_write` `:6133-6146`, `variableRead` `:5536-5552`): toda escrita/inicialização de local **não `late`** guarda no nó SSA a
     `ExpressionInfo` não trivial do valor escrito (literal `true`/`false` tem uma: o ramo oposto inalcançável;
     `booleanLiteral`); a leitura posterior sem escrita no meio a reaplica (`rebaseForward`). Vale para qualquer tipo
     declarado (`Object`, `bool`, `const`, `final`). No DartForge `instrucoes::declaracao_de_variaveis` só guarda
     (`Corpo::condicoes`) quando `expr::e_forma_de_condicao(init)` — falta o literal booleano (e parênteses dele) e a
     atribuição simples `x = true` (`expr::atribuicao`), e a leitura só é consultada em `expr::condicao` (correto).
  4. **Padrões (~46):** (a) operando direito de padrão `||` inalcançável porque o esquerdo sempre casa (`int() || 0` sobre
     `int`, `_ || 0`, `final a || final a`, `var (_ || _) = 0`): trecho do operador `||` até o fim do operando direito
     (`dead_code/DeadCode__deadPattern_ifCase_logicalOrP_*`, `missing_variable_pattern/*`, `patterns/switch_trivial_exhaustiveness_error_test.dart:274:12`);
     (b) `case`/`default` de `switch` instrução inalcançável (um caso anterior sempre casa, ou `null` depois de `case null`, ou
     tipo selado já coberto): só o token `case`/`default` (length 4/7) — e, se nenhum membro do grupo é alcançável, o corpo
     do grupo da 1ª à última instrução (`unreachable_switch_case/UnreachableSwitchCaseTest_SwitchStateme_10b7df07.dart`:
     `case` 64/4, `case` 195/4, `break;` 328/6; `patterns/exhaustiveness/sealed_class_switch_test.dart:168:5` + `:169:7`);
     (c) caso de `switch` expressão inalcançável: o caso inteiro `padrão => expr` (`DeadCode__deadPattern_switchExpression__c29a5c55.dart`:
     `int() => 1` 110/10, `_ => 2` 260/6); (d) campo de padrão objeto de tipo `Never` (`DeadCode__objectPattern_neverTypedGetter.dart`:
     o `then` `{}` morto). Requer o Apêndice A em `padroes.rs` (hoje `irrefutavel`/`tipo_casado`) e, em `instrucoes`
     (`StmtKind::Switch`) e `padroes::expressao_switch`, o gancho antes de cada `case` (token: depois dos rótulos; `SwitchCase.span`
     começa nos rótulos) e o relato do corpo morto do grupo (hoje o laço do corpo não relata nada: a variável `avisou` não é
     usada).
  5. **Elementos de coleção (9):** `if (false)` elemento (`spread_collections/null_spread_test.dart:77:34`: o `throw 1` do
     `then`), `[if (false) (a) = 0]`, elemento `for` com condição `false` (`why_not_promoted/argument_type_not_assignable_nullability_error_test.dart:429:32`),
     `[for (; ; y) 0]` (`nnbd/definite_assignment/definite_assignment_error_test.dart:330:5`), `[for (var (i) = throw 0; true; 1) 0]`
     (`DeadCode__flowEnd_forElementParts_initi_e2dbb486.dart`). `colecoes::visitar` (ramos `If`/`For`/`ForIn`) não abre trecho:
     falta o equivalente de `ramo_de_fluxo` (abrir no elemento se inalcançável; `flowEnd` nos ramos do `if` elemento
     `resolver.dart:1027-1040`).
  6. **`for` com declaração por padrão e `throw` no inicializador** (`DeadCode__flowEnd_forParts_initializer__3779cb5c.dart:2:27`):
     a condição/atualizadores ficam mortos (regra `ForParts`: até o último atualizador).
  7. **Legado e `do-while` (8):** `do break; while (true);` (`switch/case_fallthrough_legacy_error_test.dart:219`, `@dart=2.19`):
     regra `DoStatement` do Apêndice B (relata `do` e `while (…);` separadamente); `switch/switch1_test.dart:13:5` (`default`
     seguido de `case`, com erro de sintaxe) e `label/label8_test.dart:31:21` (`while (false) {` dentro de laço rotulado) — a
     conferir com a sonda.
  8. **Mensagem de `assert` com `throw` na interpolação** (`ConstEvalThrowsException__assertInitial_5db8e50b.dart:2:45`):
     o pedaço `}'` depois do `throw` (nó `InterpolationString`) é o primeiro morto; trecho até o fim do construtor.
  9. **Experimento `anonymous-methods`** (5, `DeadCodeTest_AnonymousMethodsExperiment_*`): sintaxe experimental (`never.=> 1`);
     fora de escopo (depende do parser).
  **FP (5):** (i) `is/not_class2_test.dart:25:5` e `nnbd/syntax/nullable_type_ambiguous_test.dart:32:31`: `expr::teste_de_tipo`
  promove o ramo falso para `fator(v, t)` com `promover_testado`, que torna o fluxo inalcançável quando o fator é `Never`; o
  analyzer (`tryPromoteForTypeCheck`, `flow_analysis.dart:2540-2551`) **não promove** quando o fator é `Never` (nem quando o
  tipo testado é inválido) — o ramo falso continua alcançável. (ii) `DeadCode__forLoop_noUpdaters.dart` (2 FP): `for (foo(); (i = 42) < 0;) {}`
  com `foo()` `Never` — o oráculo 3.6.2 **não** relata nada (o comentário do teste espera, mas o oráculo é a verdade); conferir
  com a sonda se o inicializador de `for` de expressão não propaga o `Never` (provável: `ForPartsWithExpression` analisa o
  inicializador sem `handleExit`). (iii) `void/void_type_usage_test.dart:1215:40`: `for (true ? x : x; false; true ? x : x) {}`
  — o oráculo relata `x` (1215:19), os atualizadores inteiros (1215:29, `_reportForUpdaters`) e `{}` (1215:43); o DartForge
  abre um trecho a mais dentro dos atualizadores (o condicional do atualizador não pode abrir trecho enquanto o dos
  atualizadores está aberto).
  **pos (4):** `DeadCode__deadBlock_conditionalElse_rec_ba8a97a0.dart` (`true ? p.x : p.y` com `p` registro: oráculo 44/4 =
  `p.y;` — o `PrefixedIdentifier` `p.y` é **reescrito** para `PropertyAccess` (campo de registro), o `flowEnd(elseExpression)`
  recebe o nó novo e não acha o primeiro morto (o antigo), então o trecho só fecha no fim do bloco, aparado na última
  instrução; o mesmo fenômeno que `expr::operando_de_fluxo` já trata para `f()` com `f` local (`chamada_de_local`) — estender
  a `p.campo` de registro (e demais reescritas de `PrefixedIdentifier`); o DartForge hoje erra o fim);
  `ForInOfInvalidType__forIn_never.dart`/`__awaitForIn_never.dart` (iterável `Never`: o primeiro morto é a **variável do laço**
  `var id` (`visitDeclaredIdentifier`), e o `flowEnd(body)` não a contém → o trecho vai até o fim do bloco de fora: 20/77;
  o DartForge começa no corpo); `never_error_test.dart:73:26` (argumentos `()` de `x.neverMethod()`, grupo 1).
- **Correções e estado (2026-10-06):** o `checkUnreachableNode` vale para **toda expressão** visitada num fluxo
  inalcançável (não só instruções e operandos de `&&`/`||`/`?:`): ela é o primeiro nó morto, e o trecho vai até o fim do
  bloco básico em curso (sem bloco básico aberto, o fim da última instrução do bloco, com o `;`). Casos: a **lista de
  argumentos** depois de receptor `Never` (`invocation_inferrer.dart:530`, do `(`); o **índice** de `x[i]`; o operando
  direito de binário, relatado do **operador** ao fim do operando direito (`dead_code_verifier.dart:316-318`, pai
  `BinaryExpression`); os ramos de `if` de coleção fecham bloco básico (`visitIfElement`, `flowEnd(ifTrue/ifFalse)`);
  e a variável de condição inicializada com literal booleano (INFERENCIA §7.10). 237 de 255; restam `for` de coleção,
  `switch` legado, recuperação de parser e `void`.

#### §3 Referência antes da declaração
- **Estado em 2026-10-07:** o padrão de tipo conhecido inválido (`case Unresolved():`) não cobre o tipo casado.
  `promoteForPattern` (`flow_analysis.dart:5196-5199`) junta o estado atual ao "não casou" e devolve falso
  (`casamento::promover_para_padrao`). Os casos seguintes deixam de ser código morto, o que tirou 3 FP de
  `unreachable_switch_case/*`.

##### `referenced_before_declaration` (perda 29: FN 29, FP 0, msg 0, pos 0) — publicado

- **Emissão:** `CompileTimeErrorCode.REFERENCED_BEFORE_DECLARATION` ("Local variable '{0}' can't be referenced before it is
  declared.", correção "Try moving the declaration to before the first use, or renaming the local variable so that it
  doesn't hide a name from an enclosing scope."), construída por `DiagnosticFactory.referencedBeforeDeclaration`
  (`analyzer/lib/src/diagnostic/diagnostic_factory.dart:341-368`, com mensagem de contexto "The declaration of '{0}' is
  here."). Dois chamadores:
  1. `ErrorVerifier._checkForReferenceBeforeDeclaration` (`analyzer/lib/src/generated/error_verifier.dart:5155-5169`), de
     `visitSimpleIdentifier` (`:1410-1420`) e `visitImportPrefixReference` (`:1080-1085`), fase ErrorVerifier (depois da
     resolução): o elemento resolvido do identificador está em `_hiddenElements`.
  2. `NamedTypeResolver` (`analyzer/lib/src/dart/resolver/named_type_resolver.dart:610-621`), fase de resolução de tipos:
     um **nome de tipo** que resolve para uma variável local ou função local (elemento `LocalVariableElement` ou
     `FunctionElement` cujo dono é um executável) — depois das checagens de `as`/`is`/redirecionamento/argumento de tipo
     (`:544-608`, que dão `CAST_TO_NON_TYPE`, `TYPE_TEST_WITH_NON_TYPE`… antes) e fora de `extends`/`implements`/`with`.
- **Condição exata (`_hiddenElements`):**
  ```
  visitBlock / visitSwitchCase / visitSwitchDefault / visitSwitchPatternCase:      (error_verifier.dart:404-409, 1505-1534)
     HiddenElements(outer, statements) = BlockScope.elementsInStatements(statements)  (analyzer/lib/src/dart/resolver/scope.dart:19-42)
        = variáveis de `VariableDeclarationStatement`, variáveis de `PatternVariableDeclarationStatement`
          (`var [v] = …`), funções locais; atravessa um `LabeledStatement`
  declara (deixa de esconder): função local ao visitar a declaração (:950-953, antes do corpo), variável comum ao
     visitar a `VariableDeclaration` (:1617-1625 — depois do inicializador), variáveis de declaração por padrão DEPOIS de
     visitar a instrução inteira (:1300-1306 — `var [v] = [v]` relata)
  visitGuardedPattern (:1057-1062): HiddenElements.forGuardedPattern = TODAS as variáveis do padrão do `case`/`if-case`
     (guardedPattern.variables), escondidas durante a visita do padrão inteiro e NUNCA declaradas dentro dele; a guarda
     `when` é visitada fora (sem esconder)
  relata quando o identificador (leitura OU escrita) resolve para um elemento escondido
  ```
  Como a resolução de nomes do analyzer já põe no escopo do bloco todas as declarações do bloco, um nome usado antes da sua
  declaração local resolve para o local (e não para o de fora) e cai aqui.
- **Posição:** o token do identificador (`nameToken`); no tipo, `node.name2` (só o nome, sem prefixo/argumentos).
- **Mensagem:** `{0}` = lexema do identificador.
- **Supressões e ordem:** referência dentro de `extends`/`implements`/`with` (outra regra); no tipo, os erros de `as`/`is`
  vêm antes (`CAST_TO_NON_TYPE`, `TYPE_TEST_WITH_NON_TYPE`); `when` não é coberto pelo esconder do padrão.
- **No DartForge:** `expr.rs` (`RefNome::Local` com `n.span.start < local.offset`; `RefNome::Adiante` vindo de
  `Corpo::declarar_adiante`) e `crates/types/src/scope.rs:242-256` (outro resolvedor). Os nomes adiante só são
  pré-declarados em `instrucoes::inferir_instrucao` `StmtKind::Block` para `StmtKind::Variables` e `StmtKind::Function`.
  Causas das 29 FN:
  - **declaração por padrão no bloco** (`ReferencedBeforeDeclaration__block_patt_1d1cf29a.dart:3:3`: `v; var [v] = [0];`):
    o pré-escaneamento do bloco não inclui `StmtKind::PatternVariables` (precisa coletar os nomes das variáveis do padrão);
  - **corpo de `case`/`default` de `switch`** (8 amostras `ReferencedBeforeDeclaration__hideInSwit_*.dart`, `v;` antes de
    `void v() {}`/`var v` no mesmo `case`): `instrucoes` `StmtKind::Switch` infere `cases[j].body` sem o pré-escaneamento
    (fazer o mesmo de `StmtKind::Block` sobre `body`, num escopo por grupo);
  - **nome de tipo que é um local declarado adiante** (`ReferencedBeforeDeclaration__type_localVariable.dart:2:3`,
    `__type_localFunction.dart:2:3`, `variable/ref_before_declaration_test.dart:88:3`: `String s = ''; … var String = '';`):
    a resolução da anotação de tipo de uma declaração local (`tipo_de_anotacao`) não consulta os locais/adiante do bloco;
    relatar no nome do tipo quando ele resolve para `Nome::Local`/`Nome::Adiante` (local variável ou função local);
  - **variável de padrão referida dentro do próprio padrão** (16 em `patterns/pattern_variable_constant_scope_test.dart`:
    `case var a && == a`, `case == b && var b`, também em `if-case`, switch expressão e elementos `if-case`): o operando do
    relacional/constante que nomeia uma variável do mesmo padrão (antes OU depois dela) relata; hoje `padroes::tipar` não
    tem a lista das variáveis do padrão em curso — coletar as variáveis do padrão do `case` antes de tipá-lo e, ao
    resolver um identificador dentro do padrão (fora da guarda), relatar se ele é uma delas;
  - `prefix_shadowed_by_local_declaration/PrefixShadowedByLocalDeclaration__local_cdc6ef0b.dart:5:3` (`a.Future? x` com
    `a` local declarado adiante e prefixo de import `a`): o caminho 1 via `visitImportPrefixReference` — o nome do prefixo
    resolve para o local escondido.
  Não há FP hoje; manter a condição estrita (o local tem de estar no mesmo bloco/grupo/padrão e ainda não declarado).
- **Correções e estado (2026-10-06):** quatro fontes, todas portadas (51 de 51):
  1. `ErrorVerifier._checkForReferenceBeforeDeclaration` com `HiddenElements`
     (`generated/error_verifier.dart:5155-5170`, `:6551-6598`): os elementos de um bloco **e das instruções de cada
     membro de `switch`** (`_withHiddenElements(node.statements)`, `:1507-1529`) ficam escondidos até a declaração —
     inclusive as variáveis de **declaração de padrão** (`var [v] = …`, `BlockScope.elementsInStatements`);
  2. `HiddenElements.forGuardedPattern` (`:6570-6575`, `visitGuardedPattern:1057-1062`): as variáveis de um padrão
     guardado ficam escondidas durante o **padrão inteiro** (nunca são "declaradas" nele) e reaparecem na guarda —
     `case var a && == a` e `case == b && var b` relatam;
  3. `NamedTypeResolver` (`dart/resolver/named_type_resolver.dart:611-621`): tipo escrito cujo elemento é variável
     local ou função local, **declarada ou não**, depois dos códigos de `as`/`is`/argumento de tipo/criação
     (`:544-600`), que têm prioridade (`a is aa` com `aa` local é `type_test_with_non_type`);
  4. `visitImportPrefixReference` (`:1080-1086`): o prefixo `a` de `a.Future` que é um local ainda escondido.

- **Tipo da leitura adiantada:** o nome liga ao elemento local, cujo tipo neste ponto é o que o `ResolutionVisitor`
  pôs (`localElement.type = varList.type?.type ?? _dynamicType`, `resolution_visitor.dart:1397`): o tipo escrito, ou
  `dynamic` sem tipo; o implícito só é inferido na declaração. `Function f = () { x = f; };` (com `int x`) dá também
  `invalid_assignment` de `Function` (`variable/ref_before_declaration_test.dart:71:11`). No DartForge, o ramo
  `RefNome::Adiante` de `identificador` (`inferencia/expr.rs`) resolve a anotação da declaração sem relatar.

#### §4 Padrões

##### `pattern_type_mismatch_in_irrefutable_context` (perda 22: FN 22)

- **Emissão:** `SharedTypeAnalyzerErrors.patternTypeMismatchInIrrefutableContext` (`analyzer/lib/src/dart/resolver/shared_type_analyzer.dart:174-185`),
  chamado pelo analisador de padrões compartilhado (`_fe_analyzer_shared/lib/src/type_inference/type_analyzer.dart`) durante a
  resolução, só quando `context.irrefutableContext != null` (declaração `var/final p = e`, atribuição por padrão `p = e`,
  `for (var p in e)`; nunca em `case`/`if-case`).
- **Condição exata** (com `M` = tipo casado **promovido**, `flow.getMatchedValueType()`):
  - variável declarada `T x` (`type_analyzer.dart:499-512`): `M` não `dynamic`/inválido e `!(M <: T)` (subtipo, não
    atribuível); variável sem tipo nunca (T = M);
  - variável atribuída `(x) = e` (`:330-341`): idem contra o tipo declarado da variável;
  - curinga com tipo `T _` (`:2078-2089`): `!isAssignableTo(M, T)`;
  - lista (`:845-855`), mapa (`:1140-1150`), objeto (`:1312-1322`), registro (`:1630-1640`): `!isAssignableTo(M, requerido)`
    (requerido = `List<E>`, `Map<K,V>`, o tipo do objeto, o formato do registro com `Object?` nos campos sem tipo:
    `(Object?,)`); avaliado DEPOIS dos subpadrões.
  `isAssignableTo` aceita `dynamic` (e não relata quando `M` é `dynamic`).
- **Posição:** o nó do padrão (`atNode(pattern)`): `int a` em `var (int a) = x`; a lista `[a]`; o objeto `String(length: a)`
  inteiro; em `(a) = x` o `a` (a variável atribuída).
- **Mensagem:** "The matched value of type '{0}' isn't assignable to the required type '{1}'." — `{0}` = `M`, `{1}` =
  requerido, ambos `DartType` (exibição padrão; tipo de função `int Function(int)`, registro `({int foo})`/`(Object?,)`).
- **Supressões e ordem:** em contexto irrefutável, um padrão refutável (`?`, constante, relacional, `||`) relata
  `refutable_pattern_in_irrefutable_context` e o resto do padrão é analisado como refutável (sem este código abaixo dele);
  tipo requerido inválido.
- **No DartForge:** não implementado. Entrar em `padroes::tipar` quando `!cx.padrao_refutavel` (chamado por
  `padroes::declaracao`, `declarar_por_tipo` (for-in), `atribuicao_de_padrao`): nos ramos `Variable{ty}`, `Wildcard{ty}`,
  `List`, `Map`, `Object`, `Record` e na atribuição a variável. Amostras: `PatternTypeMismatchInIrrefutableContext_0f7a306d.dart`
  (`var (int a) = x` com `num`), `_192863be.dart` (`var (a,) = x` com `({int foo})` → requerido `(Object?,)`), `_45088ae5.dart`
  (`(a) = x`).

##### `relational_pattern_operand_type_not_assignable` (perda 18: FN 18)

- **Emissão:** `relationalPatternOperandTypeNotAssignable` (`shared_type_analyzer.dart:196-207`), de
  `TypeAnalyzer.analyzeRelationalPattern` (`type_analyzer.dart:1691-1760`), resolução.
- **Condição exata:** `operator = resolveRelationalPatternOperator(node, M)` (`analyzer/lib/src/generated/resolver.dart:1610-1660`:
  busca `==` (para `==` e `!=`) ou o operador no tipo casado `M` via `TypePropertyResolver`); `parameterType` = tipo do 1º
  parâmetro do operador, feito anulável se é igualdade; o operando é inferido com esse contexto; se `operator != null` e
  `parameterType != null` e `!isAssignableTo(tipoDoOperando, parameterType)` → relata.
- **Posição:** o operando (`pattern.operand`, expressão inteira).
- **Mensagem:** "The constant expression type '{0}' is not assignable to the parameter type '{1}' of the '{2}' operator." —
  `{0}` tipo do operando, `{1}` = `operator.parameterType` **não anulável** (o original, sem o `?` acrescentado), `{2}` lexema
  do operador do padrão (`==`, `!=`, `>`, `>=`, `<`, `<=`).
- **Supressões:** operador não resolvido (outro erro: `undefined_operator` no operador); o operando que não é constante
  (`constant_pattern_with_non_constant_expression`/`non_constant_relational_pattern_expression`, independentes).
- **No DartForge:** `padroes::tipar` `PatternKind::Relational` já busca o membro e infere o operando com o contexto do
  parâmetro, mas não confere. Acrescentar a conferência (`atribuivel(tipo_do_operando, anulável-se-igualdade(param))`).
  Amostras `RelationalPatternArgumentTypeNotAssigna_00a67d12.dart` (`case > 0` com `A.operator >(A)`), `extension_type/relational_pattern_error_test.dart:29:19`
  (6, operando de tipo de extensão `E` contra `int`).
- **Estado (2026-10-06):** 18 de 18 (e `relational_pattern_operator_return_type_not_assignable_to_bool`, 2 de 2). O
  `analyzeRelationalPattern` vale em **todo** contexto de padrão, inclusive os `case` (o caminho refutável do casamento,
  que antes só inferia o operando).

##### `pattern_variable_assignment_inside_guard` (perda 15: FN 15)

- **Emissão:** `ScopeResolverVisitor.visitSimpleIdentifier` (`analyzer/lib/src/generated/resolver.dart:5151-5199`, relato em
  `:5186-5193`), fase de resolução de escopo (antes da inferência).
- **Condição exata:** o identificador está em contexto de escrita (`inSetterContext`: alvo de `=`, `op=`, `++`/`--`
  prefixo/posfixo) e resolve para um `PatternVariableElementImpl` com `isVisitingWhenClause` — ligado em
  `visitGuardedPattern` (`:4988-5005`) para TODAS as variáveis do padrão guardado (`node.variables`, inclusive as unidas de
  `||`) só durante a visita da cláusula `when` (inclui closures e `if-case` aninhados dentro da guarda).
- **Posição:** o identificador.
- **Mensagem:** "Pattern variables can't be assigned inside the guard of the enclosing guarded pattern." (sem argumentos).
- **No DartForge:** não implementado. Em `padroes::caso`, durante `expr::condicao_verificada(guarda)`, marcar no `Corpo` o
  conjunto de `LocalId` das variáveis do padrão (`Corpo::variaveis_em_guarda`, pilha); em `expr::atribuicao`/`unario`
  (inc/dec) ao escrever num `Local` desse conjunto, relatar no span do identificador. Amostras
  `PatternVariableAssignmentInsideGuard__e_22ae0963.dart` (`when (a = 1) > 0`), `__c_3eec891b.dart` (`a = 0;` dentro de
  closure na guarda), `patterns/guard_error_test.dart:8:22`.

##### `unnecessary_cast_pattern` (perda 13: FN 13)

- **Emissão:** `matchedTypeIsSubtypeOfRequired` (`shared_type_analyzer.dart:140-150`), de `analyzeCastPattern`
  (`type_analyzer.dart:370-395`).
- **Condição exata:** `isSubtypeOf(M, T)` e `T` não inválido, com `M` o tipo casado promovido (`p as T`). Em contexto
  refutável e irrefutável.
- **Posição:** o token `as`.
- **Mensagem:** "Unnecessary cast pattern." (WarningCode).
- **No DartForge:** não implementado; `padroes::tipar` `PatternKind::Cast` (precisa do offset do `as`: entre o fim do
  subpadrão e o início do tipo). Amostras `UnnecessaryCastPattern__matchedIsSameAsRequired.dart`, `__matchedIsSubtyp_edba307c.dart`,
  `CastFromNullAlwaysFails__castPattern_Nu_67892450.dart` (`(m as int?) = n` em atribuição), `patterns/exhaustiveness/nullable_cast_error_test.dart`
  (5, com `FutureOr`/typedef `Nullable<T> = T?` — `M` é o tipo do subpadrão objeto anterior via `&&`/caso).

##### `missing_variable_pattern` (perda 12: FN 12)

- **Emissão:** `_VariableBinderErrors.logicalOrPatternBranchMissingVariable` (`analyzer/lib/src/dart/resolver/resolution_visitor.dart:1992-2004`),
  chamado por `VariableBinder` (`_fe_analyzer_shared/lib/src/type_inference/variable_bindings.dart:100-140`), na
  `ResolutionVisitor` (antes da inferência).
- **Condição exata:** num `p1 || p2`, cada variável declarada só em `p1` relata no `p2`; cada uma só em `p2` relata no `p1`
  (lados = conjuntos de nomes de variáveis declaradas, já unidos recursivamente: `a || b || c` é `(a || b) || c`).
- **Posição:** o operando do lado onde falta (o nó inteiro do padrão: `2` em `final int a || 2`; `1` em `1 || final a`).
- **Mensagem:** "Variable pattern '{0}' is missing in this branch of the logical-or pattern." — `{0}` nome.
- **No DartForge:** não implementado; `padroes::tipar` `PatternKind::Or` (coletar nomes declarados de cada lado).
  Amostras `MissingVariablePattern__ifCase_logicalOr2_left.dart:2:29`, `…logicalOr3_1.dart` (dois relatos: `2` e `3`).

##### `unnecessary_null_check_pattern` / `unnecessary_null_assert_pattern` (perda 12 / 7: FN 12 / 7)

- **Emissão:** `matchedTypeIsStrictlyNonNullable` (`shared_type_analyzer.dart:120-138`), de `analyzeNullCheckOrAssertPattern`
  (`type_analyzer.dart:1215-1245`).
- **Condição exata:** `flow.nullCheckOrAssertPattern_begin` devolve `true` ⇔ o tipo casado `M` é **não anulável** pela
  classificação do fluxo (`_nullCheckPattern` devolve `null`, `flow_analysis.dart:5998-6010`; `InvalidType` conta como não
  anulável — por isso `getValue() case final x?` com retorno de classe indefinida relata). E: para `?`, só fora de contexto
  irrefutável (no irrefutável é `refutable_pattern_in_irrefutable_context` e nada mais); para `!`, também em contexto
  irrefutável (`(x!) = 2`).
- **Posição:** o token `?`/`!` do padrão.
- **Mensagem:** "The null-check pattern will have no effect because the matched type isn't nullable." /
  "The null-assert pattern will have no effect because the matched type isn't nullable." (StaticWarningCode).
- **No DartForge:** não implementado; `padroes::tipar` `PatternKind::NullCheck`/`NullAssert` (token = último caractere do
  span do padrão). Usar o tipo casado promovido (Apêndice A). Amostras `UnnecessaryNullCheckPattern__interfaceT_1800d04e.dart`,
  `__typeParame_9799ccca.dart` (`T extends Object`), `__invalidTyp_57995ed9.dart` (tipo inválido),
  `patterns/relational_pattern_expression_precedence_error_test.dart:52:22` (`== true ? 1 : 2` é `(== true)?` seguido de erro
  de sintaxe — depende do parser produzir o padrão `?`), `UnnecessaryNullAssertPattern__interface_e948cb68.dart`,
  `AssignmentToPrimaryConstructorParameter_fcdbb29b.dart:2:13` (`(x!) = 2`).

##### `invalid_pattern_variable_in_shared_case_scope` (perda 11: FN 11)

- **Emissão:** `ResolverVisitor.finishJoinedPatternVariable` (`analyzer/lib/src/generated/resolver.dart:878-915`): códigos
  `PATTERN_VARIABLE_SHARED_CASE_SCOPE_NOT_ALL_CASES` ("The variable '{0}' is available in some, but not all cases that share
  this body."), `…_HAS_LABEL` ("…is not available because there is a label or 'default' case."),
  `…_DIFFERENT_FINALITY_OR_TYPE` ("…doesn't have the same type and/or finality in all cases that share this body."), todos com
  nome compartilhado `INVALID_PATTERN_VARIABLE_IN_SHARED_CASE_SCOPE` (`analyzer/messages.yaml:13443`, `:13597`, `:13605`).
- **Condição exata:** para `switch` instrução com vários `case` dividindo um corpo (`location == sharedCaseScope`): a
  variável de junção com nome `x` tem inconsistência: falta em algum `case` do grupo (ou o grupo tem `default`) →
  NOT_ALL_CASES; há rótulo → HAS_LABEL; tipos (exatos) ou `final` diferentes → DIFFERENT_FINALITY_OR_TYPE. Relata em **cada
  referência** à variável no corpo (`variable.references`, coletadas em `visitSimpleIdentifier` `resolver.dart:5196-5198`).
- **Posição:** cada identificador que usa a variável no corpo (leitura ou escrita).
- **Mensagem:** `{0}` = nome.
- **No DartForge:** não implementado. Em `instrucoes` `StmtKind::Switch` o bloco "Casos que dividem o corpo" já declara as
  variáveis de junção; precisa guardar, por nome, a consistência (presença em todos os casos, tipo, `final`, rótulo,
  `default`) e relatar em cada uso no corpo. Amostras `unused_local_variable/UnusedLocalVariable__switchStatement_sh_20f8b113.dart:7:7`
  (`case 0: case [var a]: a = 1;`), `patterns/shared_case_variable_error_test.dart:12:13` (tipos diferentes).

##### `unnecessary_wildcard_pattern` (perda 10: FN 10)

- **Emissão:** `unnecessaryWildcardPattern` (`shared_type_analyzer.dart:241-253`), de `analyzeWildcardPattern`
  (`type_analyzer.dart:2068-2110`).
- **Condição exata:** o curinga é operando direto (através de parênteses) de um `&&` (`withUnnecessaryWildcardKind(logicalAndPatternOperand)`,
  `analyzeLogicalAndPattern`, `type_analyzer.dart:912-935`; anulado em subpadrões de lista/mapa/registro/objeto/`?`/`!`/`as`/`||`) e **sempre casa**:
  sem tipo, ou com tipo `T` e `promoteForPattern(M, T)` devolve `covers` (`M <: T`).
- **Posição:** o padrão curinga (`_` ou `int _`).
- **Mensagem:** "Unnecessary wildcard pattern." (WarningCode).
- **No DartForge:** não implementado; `padroes::tipar` `PatternKind::And` (marcar os operandos). Amostras
  `UnnecessaryWildcardPattern__ifCase_notR_b90d4636.dart` (`_ && 0`), `…_9a6928b7.dart` (`int _ && > 0`), `…_09d48b9f.dart` (`(_) && 0`).

##### `equal_keys_in_map_pattern` (perda 8: FN 8)

- **Emissão:** `ConstantVerifier.visitMapPattern` (`analyzer/lib/src/dart/constant/constant_verifier.dart:311-358`) →
  `DiagnosticFactory.equalKeysInMapPattern` (`analyzer/lib/src/diagnostic/diagnostic_factory.dart:247-262`), fase do
  verificador de constantes.
- **Condição exata:** cada chave avaliada como constante (sem erro); duas chaves são iguais se `identical` (`isIdentical2`) ou,
  com igualdade primitiva nas duas, `==` (`:310-325`). Relata a chave repetida (a posterior), com contexto na original.
- **Posição:** a chave duplicada (expressão).
- **Mensagem:** "Two keys in a map pattern can't be equal." (sem argumentos).
- **No DartForge:** não implementado (o equivalente `equal_keys_in_const_map` existe para literais — reaproveitar a igualdade
  de constantes). Entrar em `padroes::tipar` `PatternKind::Map` ou no verificador de constantes (`crates/types/src/constantes`).
  Amostras `EqualKeysInMapPattern__identical_int.dart` (`{0: 1, 0: 2}`), `__identical_double.dart`, `__identical_type.dart`
  (`int`/`int`), `__identical_type_e_9e60da74.dart` (`int` e typedef `E = int`), `__recordType_primi_38d24c91.dart` (`()`).

##### `pattern_never_matches_value_type` (perda 8: FN 5, FP 2, msg 1)

**Feito em 2026-10-07:** no `canBeSubtypeOf` (3.6.2 `type_system.dart:221-229`), o enum à esquerda casa se o tipo de
alguma **constante** (`v1<String>()` é `A<String>`, `v<int>()` é `E<int>`) é subtipo do da direita; não o tipo
do enum (`A<dynamic>`, `E<T>`). No DartForge, `padroes::pode_ser_subtipo` pelos `enum_constants`.
Os padrões que conferem são cast, variável declarada, objeto, curinga, lista, **mapa** (`visitMapPattern`,
`resolver.dart:1549-1554`, no padrão inteiro) e **registro** (`RecordPatternImpl.resolvePattern`, `ast.dart:14858`,
exigido `(Object?, …)`, só sem campo nomeado repetido): `x case <int, String>{0: _}` com `x: int` e
`r case (_, _)` com `r: (int,)` relatam.

- **Emissão:** `ResolverVisitor.checkPatternNeverMatchesValueType` (`analyzer/lib/src/generated/resolver.dart:614-640`),
  chamado na resolução de padrão objeto, variável declarada, curinga, cast, lista, mapa e registro (só fora de contexto
  irrefutável).
- **Condição exata:** `!typeSystem.canBeSubtypeOf(M, requerido)` (`analyzer/lib/src/dart/element/type_system.dart:147-310`)
  com `M` promovido.
- **Posição:** o tipo escrito para cast, variável declarada, objeto e curinga; senão o padrão inteiro (lista, mapa,
  registro).
- **Mensagem:** "The matched value type '{0}' can never match the required type '{1}'." (DartType).
- **No DartForge:** `padroes::nunca_casa` + `pode_ser_subtipo` (já implementado). Divergências: FN de mapa
  (`PatternNeverMatchesValueType__refutable_81d6a7ca.dart:2:14`, o ramo `Map` de `tipar` não chama `nunca_casa`), de registro
  (`…_df47f7f4.dart:3:10`, `(int f1, int f2)` contra `(int,)`: o ramo `Record` não chama), `null_type_test.dart:24:5` e
  `switch_trivial_exhaustiveness_error_test.dart:264:19`/`491:10` (tipo casado promovido: `int _ && String _` — o `String _`
  vê `M = int`; `Null _` depois de `int`… — Apêndice A); FP `PatternNeverMatchesValueType__matchedEn_9107b52c.dart`
  (`x: A` enum com `v2<int>()` e `A<T> implements R<T>`, padrão `R<num> _`: `canBeSubtypeOf` de enum testa o **tipo de cada
  constante** (`A<String>`, `A<int>`), não `A<dynamic>`, `type_system.dart:219-228`).

##### `pattern_assignment_not_local_variable` (perda 7: FN 7)

- **Emissão:** `ResolutionVisitor.visitAssignedVariablePattern` (`analyzer/lib/src/dart/resolver/resolution_visitor.dart:190-210`).
- **Condição exata:** num padrão de atribuição, o nome resolve (escopo léxico) para algo que não é variável local nem
  parâmetro (campo, getter, variável de topo, classe, tipo, função, parâmetro de tipo); se não resolve: `undefined_identifier`.
- **Posição:** o nome.
- **Mensagem:** "Only local variables can be assigned in pattern assignments."
- **No DartForge:** não implementado; `padroes::tipar` com `atribuicao` (ramo `Variable`): hoje só procura `Nome::Local`. Amostras
  `PatternAssignmentNotLocalVariable__class.dart` (`(int) = 0`), `__dynamic.dart`, `__function.dart`, `…__clas_1ef17e2e.dart`
  (campo `x`), `…__clas_41aeb1f2.dart` (`T`).

##### `duplicate_pattern_field` (perda 6: FN 6)

- **Emissão:** `duplicateRecordPatternField` (`shared_type_analyzer.dart:63-81`) → `DiagnosticFactory.duplicatePatternField`
  (`diagnostic_factory.dart:151-178`), de `_reportDuplicateRecordPatternFields` (`type_analyzer.dart:2571-2590`) em padrões
  registro e objeto.
- **Condição exata:** dois campos com o mesmo nome (explícito `foo:` ou implícito `:var foo`).
- **Posição:** o campo duplicado (nome; para `:var foo`, o nó do campo).
- **Mensagem:** "The field '{0}' is already matched in this pattern."
- **No DartForge:** não implementado; ramos `Record`/`Object` de `padroes::tipar`. Amostras `DuplicatePatternField__objectPattern.dart`,
  `__recordPattern_dy_6adabf74.dart` (`(:var foo, foo: 1)`).

##### `refutable_pattern_in_irrefutable_context` (perda 4: FN 4)

- **Emissão:** `refutablePatternInIrrefutableContext` (`shared_type_analyzer.dart:187-194`), em `analyzeConstantPattern`
  (`type_analyzer.dart:426-433`), `analyzeLogicalOrPattern` (`:964-971`), `analyzeNullCheckOrAssertPattern` (só `?`, `:1229-1235`),
  `analyzeRelationalPattern` (`:1700-1705`), quando `irrefutableContext != null`; depois o contexto vira refutável (sem
  cascata).
- **Posição:** o padrão refutável.
- **Mensagem:** "Refutable patterns can't be used in an irrefutable context."
- **No DartForge:** não implementado; `padroes::tipar` em contexto de declaração/atribuição. Amostras `RefutablePatternInIrrefutableContext__d_850ea7cf.dart`
  (`var (0) = 0`), `__d_98ce1c3c.dart` (`var (_?) = x`), `__d_b8d27d9f.dart` (`var (_ || _) = 0`, que também tem `dead_code`
  no `|| _`), `__d_c219e5ec.dart` (`var (> 0) = 0`).

##### `duplicate_pattern_assignment_variable` (perda 4: FN 4)

- **Emissão:** `duplicateAssignmentPatternVariable` (`shared_type_analyzer.dart:47-61`) → `diagnostic_factory.dart:22-45`,
  de `analyzeAssignedVariablePattern` (`type_analyzer.dart:314-322`).
- **Condição exata:** a mesma variável atribuída duas vezes no mesmo padrão de atribuição (`(a, a) = …`).
- **Posição:** a segunda ocorrência (o padrão de variável atribuída).
- **Mensagem:** "The variable '{0}' is already assigned in this pattern."
- **No DartForge:** não implementado; `padroes::atribuicao_de_padrao` (conjunto de `LocalId` já atribuídos).

##### `duplicate_variable_pattern` (perda 3: FN 3)
- **Sem `duplicate_definition` junto (2026-10-08):** a segunda ocorrência no mesmo padrão não vira elemento
  (`VariableBinder`), então o `DuplicateDefinitionVerifier` do bloco só vê a primeira: `var [a, a] = …` tem só
  este código. `analise::duplicatas` passa ao escopo só a primeira de cada nome do padrão.

- **Emissão:** `_VariableBinderErrors.duplicateVariablePattern` (`resolution_visitor.dart:1974-1989`) →
  `DiagnosticFactory.duplicateDefinitionForNodes`.
- **Condição exata:** o mesmo nome declarado duas vezes no mesmo padrão fora de lados opostos de `||` (`var (a, a)`, `[a, a]`).
- **Posição:** o nome da segunda declaração.
- **Mensagem:** "The variable '{0}' is already defined in this pattern."
- **No DartForge:** não implementado; coletar nomes em `padroes::tipar` (por padrão de topo; `||` junta).

##### `expected_two_map_pattern_type_arguments` / `expected_one_list_pattern_type_arguments` (perda 2 / 1)

- **Emissão:** `ResolverVisitor.resolveMapPattern…` (`analyzer/lib/src/generated/resolver.dart:1520-1538`) e
  `ListPatternResolver` (`analyzer/lib/src/dart/resolver/list_pattern_resolver.dart:20-33`).
- **Condição:** lista de argumentos de tipo presente com número ≠ 2 (mapa) / ≠ 1 (lista).
- **Posição:** a lista de argumentos de tipo `<…>`.
- **Mensagem:** "Map patterns require two type arguments or none, but {0} found." / "List patterns require one type argument
  or none, but {0} found." — `{0}` = número (int).
- **No DartForge:** não implementado; ramos `Map`/`List` de `padroes::tipar` (hoje usa `type_args.first()` / exige `len()==2`).

##### `relational_pattern_operator_return_type_not_assignable_to_bool` (perda 2: FN 2)

- **Emissão:** `shared_type_analyzer.dart:209-219`, de `analyzeRelationalPattern` (`type_analyzer.dart:1741-1748`).
- **Condição:** operador resolvido cujo tipo de retorno não é atribuível a `bool`.
- **Posição:** o token do operador. **Mensagem:** "The return type of operators used in relational patterns must be
  assignable to 'bool'."
- **No DartForge:** não implementado; `padroes::tipar` `Relational` (o retorno de `m.tipo`).

##### `rest_element_in_map_pattern` / `duplicate_rest_element_in_pattern` / `empty_map_pattern` (perda 2 / 1 / 1)

- **Emissão:** `restPatternInMap` (`shared_type_analyzer.dart:221-230`, de `analyzeMapPattern` `type_analyzer.dart:1100-1110`,
  na posição do elemento `...`); `duplicateRestPattern` (`shared_type_analyzer.dart:83-96` → `diagnostic_factory.dart:181-200`,
  de `analyzeListPattern` `type_analyzer.dart:816-825`, no segundo `...`); `emptyMapPattern` (`shared_type_analyzer.dart:98-106`,
  `type_analyzer.dart:1152-1155`, no padrão `{}` inteiro, depois dos elementos).
- **Mensagens:** "A map pattern can't contain a rest pattern." / "At most one rest element is allowed in a list or map
  pattern." / "A map pattern must have at least one entry."
- **No DartForge:** não implementado; ramos `Map`/`List` de `padroes::tipar` (o parser precisa preservar o `...` em mapa).

##### `non_exhaustive_switch_expression` (perda 1: msg 1)

- **Emissão:** `ConstantVerifier._validateSwitchExhaustiveness` (`analyzer/lib/src/dart/constant/constant_verifier.dart:940-985`,
  código em `:969`). **Mensagem:** "The type '{0}' is not exhaustively matched by the switch cases since it doesn't match
  '{1}'." — `{1}` é o espaço testemunha formatado pelo motor de exaustividade.
- **No DartForge:** `crates/types/src/constantes/verificador.rs` + `exaustividade.rs` (do coordenador). A 1 mensagem errada é a
  formatação da testemunha; fora desta família.

##### `case_expression_type_is_not_switch_expression_subtype` (perda 4: FN 4)

- **Emissão:** `caseExpressionTypeMismatch` (`shared_type_analyzer.dart:32-45`), de `analyzeConstantPattern`
  (`type_analyzer.dart:436-457`) **só sem padrões** (`!options.patternsEnabled`, linguagem < 3.0) e num `switch`.
- **Condição:** com null safety: `!(tipoDoCase <: tipoDoEscrutínio)`; sem: `!isAssignableTo`.
- **Posição:** a expressão do `case`. **Mensagem:** "The switch case expression type '{0}' must be a subtype of the switch
  expression type '{1}'." (DartType).
- **No DartForge:** não implementado; `instrucoes` `StmtKind::Switch` quando a biblioteca é `@dart < 3.0`.

##### `break_label_on_switch_member` (perda 4: FN 4)

- **Emissão:** `ErrorVerifier.visitBreakStatement` (`analyzer/lib/src/generated/error_verifier.dart:424-436`).
- **Condição:** `break L;` cujo rótulo `L` está num membro de `switch` (`case`/`default`, `LabelElementImpl.isOnSwitchMember`).
- **Posição:** o rótulo no `break`. **Mensagem:** "A break label resolves to the 'case' or 'default' statement."
- **No DartForge:** não implementado; `instrucoes::alvo_de_salto` (os rótulos de `SwitchCase.labels` não são alvos de
  `break`; hoje não são registrados como tal).

**Estado em 2026-10-05 (escrito, não compilado), §4.** No caminho do analisador
(`crates/types/src/inferencia/padroes/casamento.rs`, `tipar`): `refutable_pattern_in_irrefutable_context`
(constante, relacional, `||` e `?`, que tornam o resto refutável por `Corpo::refutavel_forcado`, sem
o `unnecessary_null_check_pattern` no `?` relatado); `pattern_type_mismatch_in_irrefutable_context`
(variável declarada com tipo e variável atribuída por subtipo, fora de `dynamic`/inválido; curinga
com tipo, lista, mapa, registro e objeto por atribuível, depois dos subpadrões);
`missing_variable_pattern` (as variáveis de cada lado do `||`, na ordem do `VariableBinder`);
`pattern_assignment_not_local_variable` e o `undefined_identifier` do padrão de atribuição (pelo
escopo léxico: membro herdado só pelo `this` não está nele); `duplicate_rest_element_in_pattern`
(cada `...` depois do primeiro, achado no texto entre os elementos);
`case_expression_type_is_not_switch_expression_subtype` (sem padrões, < 3.0);
`pattern_variable_assignment_inside_guard` (`Corpo::variaveis_em_guarda` durante a guarda; a
escrita por `=`, `op=` e `++`/`--` em `expr.rs`). Em `instrucoes.rs`: as variáveis de junção dos
casos que dividem o corpo (`juncoes_do_grupo`: `…_HAS_LABEL` com rótulo ou `default`,
`…_NOT_ALL_CASES`, `…_DIFFERENT_FINALITY_OR_TYPE`), relatadas em cada referência no corpo, e
`switch_case_completes_normally` (< 3.0, grupo que não é o último, na palavra do primeiro membro).
`equal_keys_in_map_pattern` no verificador de constantes (`constantes/verificador.rs`, `padrao`): a
chave igual a uma anterior (a igualdade do `DartObjectImpl`), relatada depois de todas as chaves.

#### §5 Fluxo e atribuição definitiva

##### `invalid_use_of_null_value` (perda 12: FN 12)

- **Emissão:** `NullableDereferenceVerifier.report` (`analyzer/lib/src/error/nullable_dereference_verifier.dart:39-64`): é a
  troca de qualquer `UNCHECKED_*` (§ `unchecked_use_of_nullable_value`) quando o tipo do receptor/expressão é **exatamente**
  `Null` (`receiverType == typeProvider.nullType`, `:43-46`); sem argumentos.
- **Mensagem:** "An expression whose value is always 'null' can't be dereferenced." (correção "Try changing the type of the
  expression.").
- **Posição:** a mesma do `UNCHECKED_*` que ela substitui (expressão inteira para iterável/espalhamento; token do operador
  para `++`/`--`/`-`).
- **No DartForge:** `BodyInferrer::aviso_de_nulo` (`crates/types/src/inferencia/mod.rs:528-541`) já faz a troca; as 12 FN são
  dos mecanismos que faltam no `unchecked_use_of_nullable_value`: espalhamento `[...null]`, `[...a]` com `Null a`, em lista,
  conjunto e mapa (6, `not_null_aware_null_spread/*` — sai junto com `not_null_aware_null_spread`), `for (var y in x)` com
  `x: Null` (`InvalidUseOfNullValue__forLoop.dart:3:17`), e prefixo/posfixo `x++`, `x--`, `++x`, `--x`, `-x` com `x: Null`
  (`undefined_operator/UndefinedOperator__{postfixInc,postfixDec,prefixInc,prefixDec,unaryMinus}_null.dart`, no token
  do operador). Conferido no oráculo vivo (`c1/a.dart`: `for (final x in n)` e `{...null}`).

##### `assignment_to_final_local` (perda 11: FN 11) — publicado

- **Emissão:** `AssignmentExpressionShared.checkFinalAlreadyAssigned` (`analyzer/lib/src/dart/resolver/assignment_expression_resolver.dart:355-385`)
  — chamado por atribuição (simples, composta, `??=`), prefixo/posfixo `++`/`--` e identificador de `for-in`
  (`for_resolver.dart:121-124`, `isForEachIdentifier: true`); e `resolveAssignedVariablePattern`
  (`analyzer/lib/src/generated/resolver.dart:1365-1393`) para atribuição por padrão.
- **Condição exata:** alvo `SimpleIdentifier` cujo elemento é `PromotableElement` (variável local **ou parâmetro**), `final`,
  não `late`, e (`isForEachIdentifier` **ou** não definitivamente não atribuído). Parâmetros `final` incluem os formais de
  campo `this.x` e formais de super `super.a` (são implicitamente `final`), e o parâmetro já está atribuído → toda escrita
  relata (inclusive dentro de closure no inicializador).
- **Posição:** o identificador alvo. **Mensagem:** "The final variable '{0}' can only be set once." — `{0}` nome.
- **Supressões:** `late final` vai para `late_final_local_already_assigned`.
- **No DartForge:** `expr::check_final_local` (`crates/types/src/inferencia/expr.rs`), usado na atribuição e no inc/dec.
  FN: (a) `for (i in …)` com `i` `final` (`AssignmentToFinalLocal__localVariable_forEach.dart:3:8`, `__inForEach.dart`,
  `null_safety_read_write/ReadWrite__final_definitelyAssigned_wri_2a7a09cf.dart`) — `instrucoes::cabecalho_for_in`
  `ForInTarget::Expression` escreve sem checar (e a regra é "sempre", não "se atribuída"); (b) parâmetro formal de campo
  `this.x`/super `super.a` (`AssignmentToFinalLocal__parameter_fieldFormal.dart:5:5`, `__parameter_superFormal.dart:6:26`,
  `initializing_formal/final_test.dart:13:9`) — o `Local` do parâmetro é declarado com `final_: p.final_` (o escrito), devia
  ser `true` para `this.`/`super.`; (c) `??=` sobre `final` possivelmente atribuída (`if_null/assignment_behavior_test.dart:187:5`,
  `nnbd/definite_assignment/read_error_test.dart:890:5`, `write_error_test.dart:550:5`) — o caminho de `AssignOp::IfNull`
  não passa por `check_final_local`.
- **Feito em 2026-10-07 (60/60; `late_final_local_already_assigned` 19/19):** (a) o identificador de `for-in`
  que é local `final` relata sempre (`late final`: `LATE_FINAL_LOCAL_ALREADY_ASSIGNED`); (b) `this.x` e `super.x`
  entram como locais finais na lista de inicializadores; (c) `??=` confere o final antes do lado direito, e a
  escrita fica só no ramo do nulo: depois da junção (com o alvo promovido a não nulo no outro ramo) o local
  `late final` antes não atribuído fica só possivelmente atribuído, sem o FP da escrita seguinte.

##### `late_final_local_already_assigned` (perda 2: FN 1, FP 1)

- **Emissão:** mesmos dois pontos (`assignment_expression_resolver.dart:366-374`, `resolver.dart:1375-1383`).
- **Condição:** `late final` e (`isForEachIdentifier` ou **definitivamente atribuída**); padrão: `flow.isAssigned`.
- **Posição:** o identificador. **Mensagem:** "The late final local variable is already assigned." (sem argumentos).
- **No DartForge:** `expr::check_final_local`. FN `LateFinalLocalAlreadyAssigned__localVar_db2b2bf2.dart:3:8` (`for (i in …)`
  com `late final i` — regra "sempre" do for-each). FP `nnbd/definite_assignment/definite_assignment_error_test.dart:465:5`:
  depois de `y ??= 3;` com `y` só **potencialmente** atribuída, a escrita do `??=` acontece só no ramo nulo — o fluxo depois
  do `??=` é a junção (não atribuída ∪ atribuída) = não definitivamente atribuída; o DartForge marca atribuída
  incondicionalmente na escrita de `??=` (modelar `x ??= e` como `x == null ? x = e : x`: `ifNull_rightBegin`/`ifNull_end`).

##### `not_assigned_potentially_non_nullable_local_variable` (perda 8: FN 8) — publicado

- **Emissão:** `ResolverVisitor.checkReadOfNotAssignedLocalVariable` (`analyzer/lib/src/generated/resolver.dart:643-692`,
  código em `:683-688`), na resolução de cada `SimpleIdentifier` lido.
- **Condição exata:** em contexto de leitura (`inGetterContext`), elemento `VariableElement` (local ou parâmetro) não `late`,
  **não definitivamente atribuído** (pelo estado de fluxo corrente — **também em código inalcançável**: o estado de
  atribuição sobrevive ao `setUnreachable`), não `final` (senão é `read_potentially_unassigned_final`), e
  `isPotentiallyNonNullable(tipo)` = **não anulável** (`T` sem limite, `T extends Object?`, `FutureOr<T>`… são
  potencialmente não anuláveis: só `dynamic`, `void`, `Null`, `T?` e parâmetro de tipo com limite anulável **e** sufixo `?`
  ficam de fora).
- **Posição:** o identificador. **Mensagem:** "The non-nullable local variable '{0}' must be assigned before it can be used."
- **No DartForge:** existe (publicado). FN: (a) parâmetro de tipo sem limite (`NotInitializedPotentiallyNonNullableLoc_fff714db.dart:3:3`,
  `ReadWrite__potentiallyNonNullable_defin_1c74dbe1.dart:3:3`, `…_neither_read.dart:4:3`: `T v; v;`) — a condição usa
  "não anulável" estrito (`e_nao_anulavel`) em vez de `!isNullable` (potencialmente não anulável); (b) leitura em código
  inalcançável (`NotInitializedPotentiallyNonNullableLoc_50076a65.dart:7:3`, depois de `while (true) {}`;
  `nnbd/definite_assignment/definite_assignment_error_test.dart:50:9` depois de `return;`): o DartForge não relata quando
  `!fluxo.alcancavel` (o modelo deve manter `atribuida`/`nao_atribuida` ao tornar inalcançável e consultá-los mesmo assim).
  Cuidado: no analyzer `join` de um ramo inalcançável devolve o outro (`juntar` já faz).
- **Feito em 2026-10-07 (128/129, FP 0):** (a) a leitura no código inalcançável também confere (o código morto
  herda o estado dos caminhos que levam a ele: `return; use(x);` e o que vem depois de `while (true) {}`
  relatam; vale para o `late` e o `final`); (b) no padrão `p1 || p2`, a variável de junção (a cópia do
  direito, que o escopo enxerga) recebe no ramo do esquerdo o estado da cópia do esquerdo antes da junção
  (`casamento.rs`): com o direito morto (`var a || var a`), fica atribuída.

- **Correção de 2026-10-06:** a condição é `isPotentiallyNonNullable` (`!(Null <: T)`), não `T <: Object`: o parâmetro de tipo de limite `Object?` (`T v; v;`) e o tipo de extensão sem `Object` também relatam (`expr.rs::ler_local`).

##### `read_potentially_unassigned_final` (perda 2: FN 2) — publicado

**Feito em 2026-10-08:** a leitura de um local `final` antes da declaração (o nome já está no escopo do
bloco, `REFERENCED_BEFORE_DECLARATION`) também não está atribuída: relata este código no nome. O `const` e o
`late` não. No DartForge, o braço `RefNome::Adiante` de `expr::identificador`.

- **Emissão:** o mesmo `checkReadOfNotAssignedLocalVariable` (`resolver.dart:671-679`).
- **Condição:** leitura de local `final` não `late` não definitivamente atribuído (qualquer tipo).
- **Posição/Mensagem:** o identificador; "The final variable '{0}' can't be read because it's potentially unassigned at this
  point."
- **No DartForge:** FN `variable/ref_before_declaration_test.dart:43:11` e `:96:13`: o identificador refere-se a um local
  `final` **declarado adiante** no mesmo bloco (`use(x); … final x = …;`, `math.pi` com `final math` adiante) — o analyzer
  relata **as duas coisas**: `referenced_before_declaration` e `read_potentially_unassigned_final` (o local existe e não está
  atribuído). Hoje o caminho `RefNome::Adiante` (`expr.rs`) só relata o primeiro. Regra: se o local adiante é `final` (não
  `const`, não `late`) → também este código no mesmo span; se não é `final` e o tipo **escrito** é potencialmente não
  anulável → `not_assigned_potentially_non_nullable_local_variable` (conferir com a sonda; com `var x = …` o tipo ainda não
  foi inferido).

##### `definitely_unassigned_late_local_variable` (perda 3: FN 1, pos 2)
- **`for` de coleção (2026-10-08):** `[for (; y < 0;) y = 0]` com `late int y`: o `for_conditionBegin` do
  elemento faz a junção conservadora com tudo o que o laço escreve, **o corpo inclusive** (o `AssignedVariables`
  do nó inteiro, como no comando `for`), e o `forEach_bodyBegin` do `for-in` de coleção, com o que o corpo
  escreve. `colecoes.rs` contava só a condição e as atualizações (e nada no `for-in`): o `y` da condição saía
  definitivamente não atribuído (`nnbd/definite_assignment/definite_assignment_error_test.dart:799:13`).

- **Emissão:** `checkReadOfNotAssignedLocalVariable` (`resolver.dart:660-668`).
- **Condição:** leitura de local `late` **definitivamente não atribuído** (também em código inalcançável).
- **Posição/Mensagem:** o identificador; "The late local variable '{0}' is definitely unassigned at this point."
- **No DartForge:** existe. FN `nnbd/definite_assignment/definite_assignment_error_test.dart:315:14` (`for (; ; y) {}`: o
  atualizador é lido com o estado do fim do corpo (inalcançável → herda "não atribuída"); o DartForge não infere/relata
  atualizadores inalcançáveis — `instrucoes` `StmtKind::For` marca `trecho_morto` e/ou pula). As 2 `posição` (54:9, 252:11)
  são o mesmo fenômeno de código inalcançável (relato numa linha, falta na outra): ler `use(y)` depois de `return;`.

##### `body_might_complete_normally` / `body_might_complete_normally_nullable` / `body_might_complete_normally_catch_error` (perda 8 / 4 / 3: só FN)

**Feito em 2026-10-07:** não conferem o setter (o elemento tem retorno `void`; `bool set x(v) {}` só dá
`NON_VOID_RETURN_FOR_SETTER`) nem a factory com modificador (`factory C() async* {}`: o gerador sai cedo e o
`async` tem o tipo imposto ilegal, `resolver.dart:542-563`), que só dá `NON_SYNC_FACTORY`.

- **Emissão:** `ResolverVisitor.checkForBodyMayCompleteNormally` (`analyzer/lib/src/generated/resolver.dart:522-603`), chamado
  ao fim de declaração de função (`:2800-2803`, `errorNode: node.name`), método (`:3194-3197`, `node.name`), construtor
  **factory** (`:2333-2337`, `errorNode: node` → `atConstructorDeclaration`, `analyzer/lib/src/error/listener.dart:72-95`:
  do tipo de retorno ao fim do nome, `C.named`) e expressão de função (`function_expression_resolver.dart:63-66`,
  `errorNode: body` → `{`). `_CATCH_ERROR` em `_checkForFutureCatchErrorOnError` (`resolver.dart:3971-4006`).
- **Condição exata:** fim do corpo **alcançável**; o corpo tem contexto de retorno (`bodyContext.contextType`); corpo de
  bloco, não gerador; se `async`, o tipo imposto aceita `Future<Never>`; então `BODY_MIGHT_COMPLETE_NORMALLY` se o tipo de
  retorno (de contexto: `flatten` para async) é **potencialmente não anulável**; senão, se a base `futureOrBase` não é
  `dynamic`/inválido/`_`/`void`/`Null` → `BODY_MIGHT_COMPLETE_NORMALLY_NULLABLE` (WarningCode). Sem contexto de retorno e o
  corpo é o `onError` de `Future<T>.catchError`: `_CATCH_ERROR` com `futureOrBase(FutureOr<T>)` se não é
  `dynamic`/`_`/`void`/`Null`.
- **Mensagens:** "The body might complete normally, causing 'null' to be returned, but the return type, '{0}', is a
  potentially non-nullable type." / "This function has a nullable return type of '{0}', but ends without returning a value."
  / "This 'onError' handler must return a value assignable to '{0}', but ends without returning a value." (`{0}` DartType).
- **Posição:** nome da função/método/getter (para `get b, c` recuperado, o nome `b`); construtor factory: do tipo de retorno
  ao fim do nome (`C`, `E.named`); expressão de função: o `{`.
- **Supressões:** retorno inválido (`async` com retorno não-`Future`); corpo `=>`; gerador; fim inalcançável; `external`
  NÃO suprime (amostras `ConstructorBody__class_secondaryConstru_5085df45.dart`: `external factory C() {}` relata).
- **No DartForge:** não implementado (nenhum emissor). Entrar em `funcoes::corpo_de_funcao`/`funcoes::expressao_de_funcao`
  (já sabem `cx.fluxo.alcancavel` no fim e o retorno declarado/de contexto). Amostras: `constructor/unresolved_in_factory_test.dart:8:11`,
  `type_variable/static_context_test.dart:8:14`, `BodyMightCompleteNormallyNullable__func_6872f662.dart` (`int? f() {}` → no
  nome `f`), `…_a6a31a8d.dart` (`FutureOr<int?> f(Future f) async {}`), `getter/syntax_get_set_syntax_test.dart:113:13`,
  `BodyMayCompleteNormallyCatchError__noRe_67f177f2.dart:2:29` (`future.catchError((e, st) {})` → `{`).
- **Estado em 2026-10-05 (escrito, não compilado):** `funcoes.rs::corpo_completa_normalmente` porta o
  `checkForBodyMayCompleteNormally`, com o fim do bloco alcançável (`cx.fluxo.alcancavel` depois do corpo).
  Funções e métodos declarados relatam no nome, com o imposto sendo o retorno do outline e o contexto o
  `contexto_de_retorno_declarado`. A factory relata do nome da classe ao fim do nome. A função local
  relata no nome. A expressão de função relata no `{`, com o imposto sendo o retorno escrito ou o do
  contexto. Sem imposto, `catch_error_sem_retorno` olha o pai sintático: argumento posicional de
  `x.catchError(...)` com `x` um `Future<T>`.

##### `switch_case_completes_normally` (perda 10: FN 10)

- **Emissão:** `switchCaseCompletesNormally` (`analyzer/lib/src/dart/resolver/shared_type_analyzer.dart:232-239`), de
  `analyzeSwitchStatement` (`_fe_analyzer_shared/lib/src/type_inference/type_analyzer.dart:2005-2012`).
- **Condição exata:** null safety ligado **e padrões desligados** (biblioteca com `@dart` < 3.0); o grupo de `case` não é o
  último (`caseIndex < numCases - 1`); o fim do corpo do grupo é alcançável a partir do seu começo
  (`!flow.switchStatement_afterCase()` = `locallyReachable`). Grupo vazio que cai no seguinte (`case 1: case 2:`) é o mesmo
  grupo (não relata). Em 3.0+ não existe (o `case` pode completar).
- **Posição:** a palavra-chave `case`/`default` do **primeiro** membro do grupo (`node.members[caseIndex].keyword` — o
  `caseIndex` é o índice do grupo; conferir com as amostras: `case 9:` 203:9).
- **Mensagem:** "The 'case' shouldn't complete normally." (sem argumentos).
- **No DartForge:** não implementado; `instrucoes` `StmtKind::Switch` (laço dos grupos, `cx.fluxo.alcancavel` depois do corpo)
  quando a versão da linguagem da biblioteca < 3.0. Amostras `switch/case_fallthrough_legacy_error_test.dart:203:9` (7),
  `switch/fallthru_legacy_test.dart:15:5`, `switch/empty_block_case_test.dart:20:7` (`{}` vazio completa),
  `switch/fallthru_test.dart:16:5` — conferir a versão (`@dart=2.19`).
- **Estado em 2026-10-07 (placar 10/10):** o 3.6.2 tem um defeito aqui que nós reproduzimos.
  * O `analyzeSwitchStatement` passa o índice do grupo de casos (`caseIndex`), e o
    `switchCaseCompletesNormally` o usa na lista de membros (`node.members[caseIndex].keyword`,
    `shared_type_analyzer.dart:232-239`).
  * Com casos que dividem o corpo antes (`case 0: case 1:`), o relato cai num membro anterior: o grupo do `case 10`
    relata no `case 9`.
  * No DartForge: o contador `grupo` em `instrucoes.rs`, com a palavra de `cases[grupo]`.

##### `dead_code_on_catch_subtype` / `dead_code_catch_following_catch` (perda 3 / 1: só FN)

- **Emissão:** `NullSafetyDeadCodeVerifier.verifyCatchClause` → `_CatchClausesVerifier.nextCatchClause`
  (`analyzer/lib/src/error/dead_code_verifier.dart:373-378`, `:509-544`), chamado de `ResolverVisitor.visitTryStatement`
  (`analyzer/lib/src/generated/resolver.dart:3828-3847`).
- **Condição exata:** percorrendo os `catch` em ordem: um `catch` sem `on` ou `on Object` que não é o último →
  `DEAD_CODE_CATCH_FOLLOWING_CATCH` do `catch` seguinte até o fim do último, e para; senão, se o tipo `on T` é subtipo de um
  `on S` anterior → `DEAD_CODE_ON_CATCH_SUBTYPE` [T, S] deste `catch` até o fim do último, e para. Tipo não resolvido é
  `InvalidType` e conta (`on Unavailable` depois de `on String` → `'InvalidType' is a subtype of 'String'`).
- **Posição:** do início do `catch` problemático (o `on`/`catch`) ao fim do último `catch` do `try`.
- **Mensagens:** "Dead code: This on-catch block won't be executed because '{0}' is a subtype of '{1}' and hence will have been
  caught already." / "Dead code: Catch clauses after a 'catch (e)' or an 'on Object catch (e)' are never reached." (correções
  "Try reordering the catch clauses so that this block can be reached, or removing the unreachable catch clause." / "Try
  reordering the catch clauses so that they can be reached, or removing the unreachable catch clauses.").
- **No DartForge:** não implementado; `instrucoes` `StmtKind::Try` (os tipos de `c.on_type` já são resolvidos) e o intervalo
  vira "catch morto" (nenhum `dead_code` dentro). Amostras `exception/try_catch_test.dart:37:7`, `:168:7`,
  `exception/on_catch_malformed_type_test.dart:16:5`, `:36:7` (exibição `InvalidType`).

#### §6 Nulidade e verificadores

**Estado em 2026-10-05 (escrito, não compilado).** Ganharam emissor: `unnecessary_nan_comparison`
(`double.nan` pela forma escrita, na igualdade e no padrão constante), `null_check_always_fails`,
`cast_from_null_always_fails` no `as`, `cast_from_nullable_always_fails`,
`null_argument_to_non_null_type` (`Future.value` com e sem `new`, `Completer.complete` também em
cascata) em `crates/types/src/inferencia/expr.rs`; `unnecessary_set_literal` (o parâmetro de tipo de
função que recebe a closure em `chamadas::invocar`, e o retorno escrito de função de topo e local em
`funcoes.rs`); `non_nullable_equals_parameter`, `unnecessary_no_such_method` (o `super.noSuchMethod`
pelos mixins e superclasses, método de classe que não é `Object`) e `unnecessary_question_mark` em
`crates/types/src/boas_praticas.rs`, ligados em `crates/paridade/src/analise.rs`.


##### Achado transversal: local inicializado/atribuído com valor de tipo **inválido**

Vários FN/FP abaixo e no `unchecked_use_of_nullable_value` vêm da mesma regra do fluxo: escrever num local de tipo declarado
`T?` um valor de tipo `InvalidType` (expressão que não resolve: `o.nonNull` com `o` de classe indefinida, chamada a função
com retorno de classe indefinida) **promove** o local ao tipo de interesse `NonNull(T?) = T`, porque `InvalidType` é subtipo
de tudo (`_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart:5931-5960` `_initialize` → `FlowModel.write` com
`promoteToTypeOfInterest`). Efeitos no oráculo: `int? i = o.nonNull; i.isEven;` não relata `unchecked…`
(`InvalidNullAwareOperator__invalid_nonNullable.dart`); `i == null` relata `unnecessary_null_comparison` "never null"
(`UnnecessaryNullComparisonTrue__equal_in_5aade068.dart`, 8 FN); `i?.isEven` relata `invalid_null_aware_operator`
(`InvalidNullAwareOperator__invalid_nullable.dart:5:4`). No DartForge, `BodyInferrer::escrever_fluxo`
(`crates/types/src/inferencia/fluxo.rs`) trata o escrito `dynamic` como "o declarado" (sem promoção); o tipo inválido é
representado como `dynamic` + a marca `UnitBodyTypes::tipos_invalidos`/`locais_invalidos`. Regra a implementar: se a
expressão escrita está marcada inválida, o tipo escrito efetivo é `Never`-like para a promoção: promover ao primeiro tipo de
interesse (`nao_nulo_promocao(declarado)` quando o declarado é anulável).

##### `not_initialized_non_nullable_instance_field` (perda 15: FN 12, FP 2, pos 1)

- **Emissão:** dois códigos com o mesmo nome compartilhado: `NOT_INITIALIZED_NON_NULLABLE_INSTANCE_FIELD` em
  `ErrorVerifier._checkForNotInitializedNonNullableInstanceFields` (`analyzer/lib/src/generated/error_verifier.dart:4837-4864`),
  chamado de `_checkForFinalNotInitializedInClass` (`:3625-3645`) **só quando a classe não tem construtor gerador não
  sintético**; e `…_CONSTRUCTOR` em `ConstructorFieldsVerifier.reportNotInitializedNonNullable`
  (`analyzer/lib/src/error/constructor_fields_verifier.dart:175-194`), no `returnType` do construtor, um por campo (nomes
  ordenados).
- **Condição (sem construtor):** campo não `static`, não `late`, não `final` (o `const` não é `final`: `const int v;` de
  instância relata), classe não é `Struct`/`Union` do FFI, campo não `abstract` nem `external`, sem inicializador, tipo
  potencialmente não anulável (`T`, `int`, `var v` com tipo inferido/herdado não anulável).
- **Posição:** o `VariableDeclaration` (nome até o fim do declarador); no construtor: o nome do tipo de retorno (`P` em `P._()`).
- **Mensagem:** "Non-nullable instance field '{0}' must be initialized." / "…_CONSTRUCTOR": "Non-nullable instance field '{0}'
  must be initialized." (correção diferente).
- **No DartForge:** `crates/analise/src/inicializacao.rs` (~l. 51 e ~202). FN: `T v;` (`VariableNotInitialized__class_instanceF_29bac040.dart`),
  `const int v;` de instância em classe/enum/extensão (`…_aa07ca15.dart`, `…enum_instanceFi_f37cbf12.dart`, `…extension_insta_0857cd32.dart`),
  `var v;` herdando tipo não anulável (`…_ac43854d.dart`), `augment abstract int foo;` (3, augmentations — fora de escopo),
  `could_not_infer/CouldNotInfer__constructors_inferenceFBounded.dart:7:3` (dois campos `t`, `u` no construtor `P._()`).
  Ver as 2 FP e a posição no placar (não analisadas aqui: fora do fluxo).
- **Feito em 2026-10-06 (35/35):** `_checkForNotInitializedNonNullableInstanceFields` (`error_verifier.dart:4837-4862`, chamado por `_checkForFinalNotInitializedInClass` em classe, enum, extensão e mixin sem construtor gerador explícito) decide pelo **tipo do elemento** (`isPotentiallyNonNullable`), não pela forma escrita: parâmetro de tipo (`T v;`), campo `const` (o `isFinal` da lista é só a palavra `final`), campo sem tipo inferido pela sobrescrita (`var v;` sobre `int get v`), extensão. `crates/analise/src/inicializacao.rs::finais_nao_inicializados_com` recebe o predicado (`!(Null <: T)` pelo tipo escrito ou inferido); a paridade o chama depois da inferência dos corpos. Sem o predicado (LSP, testes), a evidência sintática de antes.

##### `unnecessary_set_literal` (perda 12: FN 12)

- **Emissão:** `BestPracticesVerifier._checkForUnnecessarySetLiteral` (`analyzer/lib/src/error/best_practices_verifier.dart:1283-1318`),
  de `visitFunctionExpression`.
- **Condição:** corpo `=> e` cujo `e` é um literal de **conjunto** (`SetOrMapLiteral.isSet`); o tipo de retorno (do parâmetro
  de função ao qual a closure é passada — `staticParameterElement.type` — ou o escrito da `FunctionDeclaration`) é `void`,
  `Future<void>` ou `FutureOr<void>`.
- **Posição:** o literal `{…}`. **Mensagem:** "Braces unnecessarily wrap this expression in a set literal." (WarningCode).
- **No DartForge:** não implementado; entrar em `funcoes::expressao_de_funcao`/`inferir_funcao_declarada` (corpo `=>`), com
  o literal já decidido conjunto. Amostras `UnnecessarySetLiteral__functionDeclarat_14130680.dart` (`void f() => {1, 2};`),
  `__expressionFuncti_12dcc4e6.dart` (`g(() => {1, 2})` com parâmetro `void Function()`).

##### `late_final_field_with_const_constructor` (perda 10: FN 10)

- **Emissão:** `ErrorVerifier._checkForLateFinalFieldWithConstConstructor` (`analyzer/lib/src/generated/error_verifier.dart:4068-4091`).
- **Condição:** campo de instância `late final` numa classe/enum (não extensão) com algum construtor `const` não factory.
- **Posição:** a palavra `late`. **Mensagem:** "Can't have a late final field in a class with a generative const constructor."
- **No DartForge:** não implementado (crates/analise, junto das regras de campo). Amostras `VariableNotInitialized__class_instanceF_0b3e114e.dart:2:3`,
  enums (construtores de enum são `const`), `InvalidReferenceToThis__enum_instanceFi_2e3283ae.dart:3:3`.

##### `dead_null_aware_expression` (perda 8: FN 6, FP 2)

- **Emissão:** `ErrorVerifier._checkForDeadNullCoalesce` (`analyzer/lib/src/generated/error_verifier.dart:3045-3052`), de
  `visitAssignmentExpression` (`??=`, `:361`, com `node.readType`) e `visitBinaryExpression` (`??`, `:394`).
- **Condição:** tipo do lado esquerdo **estritamente não anulável** (`TypeSystemImpl.isStrictlyNonNullable`,
  `analyzer/lib/src/dart/element/type_system.dart:1290-1309`: falso para `dynamic`, inválido, `_`, `void`, `Null`, qualquer
  `?`; `FutureOr<T>` segue `T`; **tipo de extensão só se tem `implements`** (`interfaces.isNotEmpty`); parâmetro de tipo
  segue o limite; o resto — inclusive `Never` — verdadeiro).
- **Posição:** o lado direito. **Mensagem:** "The left operand can't be null, so the right operand is never executed."
- **No DartForge:** `expr.rs` (`DEAD_NULL_AWARE_EXPRESSION`, ~l. 1274). FN: `dot_shorthands/*_if_null_error_test.dart` (6) — o
  lado esquerdo é um atalho de ponto cujo tipo é um **tipo de extensão com `implements num`** (`extension type IfNullConstructorExt(int x) implements num`):
  estritamente não anulável pela regra acima; o DartForge trata tipo de extensão como anulável. FP: `x[0] ??= 1` com `x: Never`
  (`InvalidUseOfNever__indexExpression_neve_e8cb085e.dart:4:12`): com receptor `Never` o `PropertyElementResolver` sai cedo
  (`receiver_of_type_never`) sem tipo de leitura — o `readType` não é `Never` e nada sai; `C ??= null` com `C` classe
  (`if_null/assignment_behavior_test.dart:195:9`): alvo não é variável (erro de atribuição) e não há `readType` válido.
- **Estado em 2026-10-07 (placar 35/35):** duas correções.
  * `expr::estritamente_nao_anulavel` é agora o `isStrictlyNonNullable` inteiro (`type_system.dart:1290-1310`).
    Antes era `T <: Object`, com o tipo de extensão sempre de fora.
    * Fora: `dynamic`, inválido, `_`, `void`, `Null` e qualquer `?`.
    * `FutureOr<T>` vale pelo `T`.
    * O tipo de extensão vale quando tem `implements`.
    * O parâmetro de tipo vale pelo limite, o promovido ou o declarado.
    * Saíram os 6 FN de atalho de ponto com tipo de extensão.
  * No `a ??= b`, o `readType` do 3.6.2 (`resolver.dart:1662-1692`) só existe para getter, variável e o `[]`
    resolvido. O índice sobre receptor `Never` (`resolver_ao_limite`) e o nome que é classe, alias, função,
    método, parâmetro de tipo, construtor ou prefixo (`C ??= null`) não relatam.

##### `not_initialized_non_nullable_variable` (perda 8: FN 8)

- **Emissão:** `ErrorVerifier._checkForNotInitializedNonNullableVariable` (`analyzer/lib/src/generated/error_verifier.dart:4875-4915`),
  para variáveis de topo e campos `static` (`_checkForNotInitializedNonNullableStaticField`, `:4867-4872`).
- **Condição:** não `const`, (topo) não `final`, não `late`, não `external`, tipo **escrito** potencialmente não anulável, sem
  inicializador. `abstract` NÃO suprime aqui (`static abstract int foo;` relata).
- **Posição:** o nome. **Mensagem:** "The non-nullable variable '{0}' must be initialized."
- **No DartForge:** `crates/types/src/sobrescritas.rs:1196-1241`. FN: os 8 são `static abstract int foo;`/`static abstract final int foo;`
  (`executable_body/ExecutableBody__class_staticField_abstr_*`): o DartForge pula campos `abstract` (só deve pular `external`).

##### `unnecessary_null_comparison` (perda 8: FN 8)
- **Feito em 2026-10-07 (54/54):** a escrita de `InvalidType` promove ao não anulável do declarado (INFERENCIA
  §7.8), e `===`/`!==` (`UNSUPPORTED_OPERATOR`) não são comparação de igualdade para esta regra.

- **Emissão:** `BestPracticesVerifier._checkForInvariantNullComparison` (`analyzer/lib/src/error/best_practices_verifier.dart:1059-1092`;
  "never null" `NEVER_NULL_TRUE/FALSE`) e `BinaryExpressionResolver` (`analyzer/lib/src/dart/resolver/binary_expression_resolver.dart:123-150`;
  "always null").
- **Condição/posição:** `null == e` com `e` estritamente não anulável → de `null` até o operador; `e == null` → do operador
  até o fim de `null` (`!=` → `…_TRUE`).
- **Mensagem:** "The operand can't be 'null', so the condition is always 'false'." / "…'true'.".
- **No DartForge:** `expr.rs` (~l. 3248-3280). As 8 FN são o achado transversal (local promovido ao tipo de interesse por
  escrita de valor inválido).

##### `unnecessary_type_check` (perda 8: FN 3, FP 5)

- **Emissão:** `BestPracticesVerifier._checkAllTypeChecks` (`analyzer/lib/src/error/best_practices_verifier.dart:771-820`).
- **Condição:** esquerdo inválido → nada; `is dynamic` → relata; direito inválido → nada; `is Null` → `type_check_is_null`
  (só literal `null is Null` é desnecessário); senão `isSubtypeOf(esquerdo, direito)` → relata.
- **Posição:** a expressão `is` inteira. **Mensagem:** "Unnecessary type check; the result is always 'true'." / "…'false'."
- **No DartForge:** `expr::teste_de_tipo_desnecessario` (~l. 3383-3430). FN `function/type_alias6_test.dart:24:18`,
  `prefix/prefix16_test.dart:29:17`/`30:17` (tipos de função via `typedef` e closures: o tipo estático da closure
  `(Object x) => x` é subtipo do typedef); FP `generic/function_typedef2_test.dart:54:17`, `function_typedef3_test.dart:46:17`
  (`foo is M` com typedef genérico — subtipagem de funções genéricas/limites), `interface/injection1_test.dart:21:18`,
  `injection2_test.dart:19:18` (`new C() is S` — hierarquia com erro), `nnbd/syntax/nullable_type_ambiguous_test.dart:32:13`
  (`a is bool?` com `a` promovido a `bool` pela atribuição `a = true` de `dynamic`… o analyzer não promove `dynamic` por
  atribuição: promoção só a tipos de interesse — `escrever_fluxo`).

##### `non_nullable_equals_parameter` (perda 7: FN 7)

- **Emissão:** `BestPracticesVerifier._checkForNullableEqualsParameterType` (`best_practices_verifier.dart:1150-1184`).
- **Condição:** método `operator ==` com exatamente 1 parâmetro (de qualquer espécie: `[a]`, `{a}`, genérico `==<T>(a)`), cujo
  tipo é `Object?` ou `dynamic` (inclusive herdado: `==(other)` sem tipo herda `dynamic`/`Object?` da superclasse).
- **Posição:** o nome do operador (`==`). **Mensagem:** "The parameter type of '==' operators should be non-nullable."
- **No DartForge:** não implementado (crates/analise ou `sobrescritas.rs`, onde o tipo herdado do parâmetro já é calculado).

##### `null_argument_to_non_null_type` (perda 7: FN 7)

- **Emissão:** `NullSafeApiVerifier` (`analyzer/lib/src/error/null_safe_api_verifier.dart:29-80`): `methodInvocation`
  (`Completer<T>.complete`, alvo real, inclusive cascata) e `instanceCreation` (`Future<T>.value`).
- **Condição:** no máximo 1 argumento; `T` não anulável; argumento ausente ou de tipo `Null`-equivalente.
- **Posição:** o argumento, ou a invocação inteira se não há argumento (`Completer<int>().complete()` → do início do alvo;
  em cascata `c..complete()` → a seção `..complete()`). **Mensagem:** "'{0}' shouldn't be called with a 'null' argument for
  the non-nullable type argument '{1}'." — `{0}` "Completer.complete"/"Future.value", `{1}` `T.getDisplayString()`.
- **No DartForge:** não implementado; `chamadas::chamada`/`instanciacao` (classe de `dart:async`).

##### `cast_from_null_always_fails` (perda 6: FN 6)

- **Emissão:** `BestPracticesVerifier.visitAsExpression` (`best_practices_verifier.dart:135-150`) e `visitCastPattern` (`:168-182`).
- **Condição:** tipo alvo não anulável (`isNonNullable`: inclui `Never`, `T extends Object`) e o operando (ou o tipo casado
  do padrão) é `Null`.
- **Posição:** a expressão `as` inteira / o padrão cast inteiro. **Mensagem:** "This cast always throws an exception because
  the expression always evaluates to 'null'."
- **No DartForge:** não implementado; `expr.rs` `ExprKind::As` e `padroes::tipar` `Cast`.

##### `not_null_aware_null_spread` (perda 6: FN 6)

- **Emissão:** `LiteralElementVerifier` (`analyzer/lib/src/error/literal_element_verifier.dart:180-200` lista/conjunto,
  `:275-292` mapa).
- **Condição:** espalhamento sem `?` cujo tipo é subtipo de `Null` (e não de `Never`).
- **Posição:** a expressão espalhada. **Mensagem:** "The Null-typed expression can't be used with a non-null-aware spread."
- **No DartForge:** não implementado; `colecoes::visitar` `Spread` (sai junto com `invalid_use_of_null_value`, conferido no
  oráculo vivo).

##### `unnecessary_nan_comparison` (perda 5: FN 5)

- **Emissão:** `BestPracticesVerifier._checkForInvariantNanComparison` (`best_practices_verifier.dart:1030-1057`) e no padrão
  constante (`:250-265`, `case double.nan`).
- **Condição/posição:** um operando é `double.nan` (`isDoubleNan`: `double.nan` prefixado/identificador do `dart:core`);
  `==` → `…_FALSE`, `!=` → `…_TRUE`; intervalo do `double.nan` até o operador (nan à esquerda) ou do operador até o fim
  (nan à direita); padrão: a constante.
- **Mensagem:** "A double can't equal 'double.nan', so the condition is always 'false'." / "…'true'."
- **No DartForge:** não implementado; `expr::binario` (igualdade) e `padroes::tipar` `Constant`.

##### `null_check_always_fails` (perda 4: FN 4)

- **Emissão:** `BestPracticesVerifier.visitPostfixExpression` (`best_practices_verifier.dart:686-697`).
- **Condição:** `e!` com tipo de `e` exatamente `Null` (`null!`, `(null)!`, `g()!` com retorno `Null`, `(await g())!`).
- **Posição:** a expressão `e!` inteira. **Mensagem:** "This null-check will always throw an exception because the
  expression will always evaluate to 'null'."
- **No DartForge:** não implementado; `expr::unario` `UnaryOp::NullAssert`.

##### `nullable_type_in_catch_clause` (perda 4: FN 4)

- **Emissão:** `BestPracticesVerifier._checkForNullableTypeInCatchClause` (`best_practices_verifier.dart:1186-1204`).
- **Condição:** `on T` com `T` não inválido e potencialmente anulável (`dynamic`, `T?`, função `?`, parâmetro de tipo de
  limite anulável).
- **Posição:** a anotação de tipo. **Mensagem:** "A potentially nullable type can't be used in an 'on' clause because it
  isn't valid to throw a nullable expression."
- **No DartForge:** não implementado; `instrucoes` `StmtKind::Try` (o tipo de `c.on_type` já é resolvido).

##### `nullable_type_in_implements_clause` (perda 4: FN 4) — publicado (`nullable_type_in_implements_clause`)

- **Emissão:** `NamedTypeResolver._verifyNullability` (`analyzer/lib/src/dart/resolver/named_type_resolver.dart:381-410`).
- **Condição:** o tipo resolvido (já com o alias expandido) tem sufixo `?` — inclusive por um `typedef X = A?`.
- **Posição:** o tipo nomeado. **Mensagem:** "A class, mixin, or extension type can't implement a nullable type."
- **No DartForge:** `crates/analise/src/clausulas.rs` (~l. 1247): FN em **tipo de extensão** (`extension type E(A _) implements A?`,
  `implements B` com `typedef B = A?`; `ExtensionTypeImplementsDisallowedType___c43dde67.dart`) — o ramo dos tipos de
  extensão não confere a nulidade (nem a do alias).
- **Estado em 2026-10-07 (placar 10/10):** o ramo do tipo de extensão em `clausulas::verificar` confere cada tipo do
  `implements` como o das classes. A regra é `anulavel`, que segue o alias, com o tipo resolvido a uma
  classe, mixin, enum ou tipo de extensão. O relato vai no nó inteiro.

##### `unnecessary_question_mark` (perda 4: FN 4)

- **Emissão:** `BestPracticesVerifier.visitNamedType` (`best_practices_verifier.dart:654-675`).
- **Condição:** tipo nomeado com `?` cujo tipo é `Null` (o da `dart:core`) ou `dynamic` escrito literalmente, e sem alias.
- **Posição:** o token `?`. **Mensagem:** "The '?' is unnecessary because '{0}' is nullable without it." — `{0}` = nome
  qualificado escrito.
- **No DartForge:** não implementado (resolução de anotações, crates/types `resolve`).

##### `unnecessary_final` (perda 3: FN 3)

- **Emissão:** `BestPracticesVerifier._checkFinalParameter` (`best_practices_verifier.dart:828-835`), de
  `visitFieldFormalParameter`/`visitSuperFormalParameter`.
- **Condição:** `final this.x` / `final super.x`. **Posição:** a palavra `final`. **Mensagem:** "The keyword 'final' isn't
  necessary because the parameter is implicitly 'final'."
- **No DartForge:** não implementado (crates/analise, parâmetros de construtor).

##### `cast_from_nullable_always_fails` (perda 2: FN 2)

- **Emissão:** `ResolverVisitor.visitAsExpression` (`analyzer/lib/src/generated/resolver.dart:1866-1886`).
- **Condição:** `x as T` com `x` identificador de local/parâmetro (`PromotableElement`) de tipo declarado anulável, tipo de
  `x` no ponto não `Null`, `T` não anulável, e `x` **definitivamente não atribuído** (o valor é sempre `null`).
- **Posição:** o identificador `x`. **Mensagem:** "This cast will always throw an exception because the nullable local
  variable '{0}' is not assigned."
- **No DartForge:** não implementado; `expr.rs` `ExprKind::As` (o fluxo já sabe `nao_atribuida`).

##### `unnecessary_no_such_method` (perda 2: FN 2)

- **Emissão:** `BestPracticesVerifier._checkForUnnecessaryNoSuchMethod` (`best_practices_verifier.dart:1234-1280`).
- **Condição:** método `noSuchMethod` cujo corpo é só `super.noSuchMethod(x)` (=> ou `{ return …; }`) e o `noSuchMethod`
  chamado não é o de `Object`.
- **Posição:** o nome do método. **Mensagem:** "Unnecessary 'noSuchMethod' declaration."
- **No DartForge:** não implementado.

##### `invalid_null_aware_operator` (perda 1: FN 1) — publicado

- **Emissão:** `ErrorVerifier._checkForUnnecessaryNullAware` (`analyzer/lib/src/generated/error_verifier.dart:5553-5600`).
- **No DartForge:** `expr.rs` (~l. 1319-1345). A 1 FN (`InvalidNullAwareOperator__invalid_nullable.dart:5:4`) é o achado
  transversal (`int? i = o.nullable` com `o` inválido → `i` promovido a `int`).

#### §7 Extra

##### (extra, fora da lista B) `constant_pattern_never_matches_value_type` (perda 52: FN 52) — publicado

- **Emissão:** `ConstantVerifier.visitConstantPattern` (`analyzer/lib/src/dart/constant/constant_verifier.dart:130-160`) com
  `_canBeEqual` (`:517-546`).
- **Condição:** a constante avalia sem erro; `value.hasPrimitiveEquality` (`analyzer/lib/src/dart/constant/value.dart`:
  bool/int/double/String/Null/Symbol/Type/função/lista/mapa/conjunto sempre; registro se todos os campos; instância genérica
  se `==` e `hashCode` concretos vêm de `Object`, `:1496-1523`); tipo da constante = **tipo do valor** (`A<num>` em
  `const A<num>()`, `bool` em `const (1 == 2)`/`!false`, `Symbol`, `Type`, `List<dynamic>` em `const []`); tipo casado =
  `matchedValueType` (o **promovido** pelo fluxo de padrões, Apêndice A) com apagamento de tipo de extensão; e
  `!_canBeEqual(c, M)`: `M` interface → `c <: eliminateToGreatest(M)` (e `int` × `double` pode); `M` parâmetro de tipo →
  usa `promotedBound ?? bound` (com `?` se `M` tem `?`) quando o limite não referencia parâmetros (cobre `T & int`);
  `M` função → só `null` pode (se `M` anulável); outros → pode.
- **Posição:** o padrão constante. **Mensagem:** "The matched value type '{0}' can never be equal to this constant of type
  '{1}'." (DartType).
- **No DartForge:** `padroes::constante_nunca_casa`/`pode_ser_igual` (`crates/types/src/inferencia/padroes.rs`), restrito a
  literais e enums. FN: (a) `T & int` (interseção) não tratada em `pode_ser_igual` (`ConstantPatternNeverMatchesValueType__b_412c86ec.dart`,
  `patterns/invalid_const_pattern_binary_test.dart` 11×) — usar o `bound` da interseção; (b) tipo casado promovido pelos
  casos anteriores (`case null:` depois de `case null:` sobre `bool?`, `null => …` depois de `Null _ => …`:
  `bool_switch_test.dart:165:10`, `enum_switch_test.dart:249:10`, `null_type_test.dart:33:5`, `object_pattern_switch_test.dart:204:10`)
  — Apêndice A; (c) constantes não literais: `const A()`/`const A<num>()`/`const G()`, `const []`, `const {}`, `const #a`,
  `G<int>` (literal de tipo → `Type`), `!false`, `1 == 2` (`ConstantPatternNeverMatchesValueType__c_*`, `patterns/invalid_const_pattern_test.dart`
  ~30×) — o tipo do valor (estático da criação/literal) e a igualdade primitiva por classe (busca concreta de `==`/`hashCode`).
  Cuidado (publicado): só relatar quando a constante avalia sem erro (o avaliador `crate::constant::ConstantEvaluator`).
- **Estado (2026-10-06):** a regra saiu da inferência para o verificador de constantes
  (`constantes/verificador.rs::padrao`), como no `ConstantVerifier.visitConstantPattern`: o tipo da constante é o
  `value.type` do valor **avaliado** (não o estático da expressão), a guarda é o `hasPrimitiveEquality` do valor, o tipo
  casado é o `matchedValueType` (`tipos_casados`) sem tipos de extensão, e `_canBeEqual` usa o
  `PatternGreatestClosureHelper.eliminateToGreatest` (parâmetro de tipo → `Object?` covariante / `Never`
  contravariante) e o `promotedBound` de `X & T`. Relatado, o padrão não tem os filhos visitados. 71 de 71.

#### Apêndice A — análise de fluxo de padrões (o que o DartForge não modela)

Fonte: `_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart` (implementação `_FlowAnalysisImpl`) e as chamadas em
`_fe_analyzer_shared/lib/src/type_inference/type_analyzer.dart`.

**Estado.** Durante um casamento há (a) o fluxo corrente `_current` = "o padrão casou até aqui", (b) o acumulador
`_unmatched` = junção de todos os estados em que o casamento já falhou, (c) a **referência do valor casado** (uma variável
temporária com chave de promoção própria, `_pushScrutinee` `:6077-6100`, `createReference`), cujo tipo promovido é o
`matchedValueType` que os padrões veem (`flow.getMatchedValueType()`), e (d) `_scrutineeReference`: se o escrutínio é uma
variável local (ou propriedade promovível) **e** `allowScrutineePromotion` (só `switch` e `if-case`; não em declaração,
atribuição por padrão nem `for` com padrão), as promoções do valor casado de topo também valem para ela enquanto a versão SSA
for a mesma (`:5217-5245`).

**Entrada/saída.** `_pushPattern` (`:6060-6066`): `_unmatched = _current.setUnreachable()`. `_popPattern(guard)`
(`:6037-6050`): devolve `_unmatched` (com o ramo falso da guarda juntado); o `_current` vira o ramo verdadeiro da guarda.
Subpadrões de lista/mapa/registro/objeto ganham **referência nova** (tipo = tipo do elemento/campo); `&&`, `||`, `(p)`, `p?`,
`p!`, `p as T` reusam a mesma.

**Regras por padrão** (com `t` = tipo promovido corrente da referência):
- `promoteForPattern(matchedType: t, knownType: K, matchFailsIfWrongType = true, matchMayFailEvenIfCorrectType = false)`
  (`:5191-5260`): se `K` inválido → `_unmatched ∪= _current`, devolve false. Se `t` não anulável, `K = promoteToNonNull(K)`.
  `covers = t <: K` (com apagamento de tipo de extensão). `ifTrue/ifFalse = tryPromoteForTypeCheck(ref, K)` (`:2525-2560`:
  `ifTrue` promove para `K` (ou `t & K`); `ifFalse` promove para `factor(t, K)` **exceto quando o fator é `Never` ou igual a
  `t`: então não promove nem torna inalcançável**). O mesmo no escrutínio (se aplicável). `_current = ifTrue`; se
  `matchFailsIfWrongType && !covers` → `_unmatched ∪= ifFalse`; se `matchMayFailEvenIfCorrectType` → `_unmatched ∪= ifTrue`.
  Chamado por: variável com tipo (`type_analyzer.dart:515`), curinga com tipo (`:2094`), objeto (`:1301`), registro
  (`:1606`, depois `:1645` com `matchFailsIfWrongType: false`), lista (`:810`, `matchMayFail… = !(um único elemento e ele é
  resto)`), mapa (`:1097`, `matchMayFail… = true`), cast (`:377`, `matchFailsIfWrongType: false`), variável atribuída
  (`:344`, `matchFailsIfWrongType: false`).
- variável/curinga sem tipo: nada (sempre casa).
- `p?` (`nullCheckOrAssertPattern_begin`, `:5084-5110`): `_unmatched ∪= _current` **sempre** (mesmo com `t` não anulável);
  `ifNotNull = _nullCheckPattern` (`:5998-6035`: `null` se `t` não anulável; senão promove a referência (e o escrutínio)
  para não nulo, e se `t` é `Null` torna inalcançável); `_current = ifNotNull ?? _current`. `p!`: igual sem o `∪=`.
- constante `c` e `== c`/`!= c` (`constantPattern_end` `:4489-4505`, `equalityRelationalPattern_end` `:4626-4631` →
  `_handleEqualityCheckPattern` `:5832-5915` com `_equalityCheck` `:5642-5673`): se `t` e `c` são ambos Null-equivalentes →
  garantidamente igual (constante: nada muda; `!=`: `_unmatched ∪= _current`, `_current` inalcançável); se um é Null e o outro
  não anulável → sem informação; se `c` é o literal `null` (isNull) e `t` anulável → é checagem de nulo: `ifNotNull` como
  acima; constante (`==`): `_unmatched ∪= ifNotNull`; `!=`: `_unmatched ∪= _current`, `_current = ifNotNull`. Qualquer
  outro caso: sem informação → `_unmatched ∪= _current`. (Para bibliotecas sem padrões, `constantPattern_end` faz sempre
  `_unmatched ∪= _current`.)
- relacional `<`, `<=`, `>`, `>=` (`nonEqualityRelationalPattern_end` `:5016-5021`): `_unmatched ∪= _current`.
- `p1 && p2`: `p1` depois `p2`, mesma referência.
- `p1 || p2` (`logicalOrPattern_begin/afterLhs/end` `:4995-5014`, `:4978-4993`): guarda `prev = _unmatched`;
  `_unmatched = _current.setUnreachable()`; analisa `p1`; `lhsCasou = _current`; `_current = _unmatched` (falhas de `p1`);
  `_unmatched = prev`; **`checkUnreachableNode(p2)`** (`analyzer/lib/src/generated/resolver.dart:1062-1065`); analisa `p2`;
  `_current = lhsCasou ∪ _current`; `flowEnd(p2)` (`analyzer/lib/src/dart/ast/ast.dart:11595`).

**`switch` instrução** (`type_analyzer.dart:1907-2035`, flow `:5335-5400`): o escrutínio vira a referência; para cada
`case`/`default` de cada grupo: `_current = _unmatched do switch` (estado "nenhum caso anterior casou");
`checkUnreachableNode(member)`; padrão; `_unmatched do switch = _popPattern(guard)`; `flowEnd(member)` (cabeça). O corpo do
grupo começa na junção dos "casou" dos membros (`switchStatement_endAlternatives`); com rótulo, junção conservadora do estado
antes do `switch`. `default` casa sempre (`_unmatched` fica inalcançável). No fim (`switchStatement_end` `:5350-5374`):
saída = breaks ∪ fim dos corpos que completam ∪ (`_unmatched` se não exaustivo).
**`switch` expressão** (`type_analyzer.dart:1779-1900`): igual, com `checkUnreachableNode(case)` antes e `flowEnd(case)` depois de cada caso.
**`if-case`/elemento `if-case`**: `ifCaseStatement_afterExpression` (`flow_analysis.dart:4747-4765`, `type_analyzer.dart:592`, `:657`) empurra escrutínio (com promoção) e
padrão; `thenBegin(guard)`: then = casou ∧ guarda; else = `_unmatched`.

**Consequências observáveis** (todas FN hoje):
1. tipo casado mais estreito nos casos seguintes: `switch (b) { case true: … case false: … case null: … case null: }` com
   `b` `bool?` → o 4º `null` é casado contra `bool` → `constant_pattern_never_matches_value_type`;
2. trechos mortos: `int() || 0` sobre `int` (o `|| 0` é inalcançável); `case 1:` depois de um caso que sempre casa (o `case`
   é morto e o seu corpo também, se só ele entra no corpo);
3. variáveis de padrão com o tipo promovido (`case int? x?` → `x: int`; `case num() && var y` → `y: num`).

**No DartForge:** `padroes::caso` (`crates/types/src/inferencia/padroes.rs`) usa `irrefutavel` (aproximação: curinga,
variável, cast, `!`, `&&`, `||`) para decidir o "não casou" e `tipo_casado` para promover só o escrutínio no ramo "casou";
`tipar` usa o tipo não promovido `t` em todos os subpadrões; `instrucoes` (`StmtKind::Switch`) e
`padroes::expressao_switch` passam o mesmo `t` a todos os casos. Proposta: uma referência sintética por casamento
(`Corpo::declarar_sintetico` com o tipo do valor casado), um campo `Corpo::padrao_nao: Option<Fluxo>` com o `_unmatched`, e
`tipar` aplicando as regras acima a cada nó (promoção via `promover`/`fator` com a exceção do fator `Never`), mais
`Corpo::escrutinio: Option<(LocalId, versão)>` para a promoção do escrutínio. O tipo `t` de cada caso passa a ser
`fluxo.tipo_atual(ref)`.

#### Apêndice B — o verificador de código morto (`NullSafetyDeadCodeVerifier`)

`analyzer/lib/src/error/dead_code_verifier.dart:186-493`. Estado: `_firstDeadNode` (o primeiro nó inalcançável do trecho em
curso). Dois ganchos, chamados pelo `ResolverVisitor` durante a resolução (com a análise de fluxo em curso):

1. `visitNode(node)` (`:392-419`), chamado por `ResolverVisitor.checkUnreachableNode` (`analyzer/lib/src/generated/resolver.dart:694-696`):
   se já há `_firstDeadNode`, nada; se `flow.isReachable`, nada; se `node` está num intervalo de `catch` morto já relatado
   (`_deadCatchClauseRanges`), nada; senão `_firstDeadNode = node`.
   **Quem chama** (é o que decide onde o trecho COMEÇA):
   - quase todo `visitX` de expressão e instrução no começo da visita (`resolver.dart`: literais, identificadores
     (`simple_identifier_resolver.dart:37`), `this`, `super`, `as`, `is`, `await`, `throw`, `rethrow`, atribuição, binário,
     índice, invocação de método, acesso a propriedade, prefixo/posfixo, criação de instância, listas/conjuntos/mapas,
     entradas de mapa, espalhamento, registro, interpolação e cada pedaço de string (`visitInterpolationString`), `switch`,
     condicional, bloco, instruções, `catch`, etc.);
   - **a lista de argumentos** inteira (`analyzer/lib/src/dart/resolver/invocation_inferrer.dart:530`, `_visitArguments`), em
     TODA invocação resolvida pelo `InvocationInferrer` (método, função, construtor, inclusive receptor `Never`/inválido) —
     por isso `x.foo(1 + 2)` com `x` `Never` começa no `(`; `x()` começa no `(`;
   - lista de argumentos de tipo (`visitTypeArgumentList`);
   - operando direito de `&&`/`||` (`binary_expression_resolver.dart:231`, `:260`), ramos do condicional
     (`resolver.dart:2266`, `:2277`), operando direito de padrão `||` (`resolver.dart:1062-1065`,
     `handle_logicalOrPattern_afterLhs`), cada `case`/`default` de `switch` (instrução: o `SwitchMember`; expressão: o
     `SwitchExpressionCase`) antes do padrão (`resolver.dart:1144-1157`, `handleSwitchBeforeAlternative`).
   Padrões (fora o `||`) não chamam: o trecho que começa num padrão começa no `case`/no próximo nó de expressão/instrução.

2. `flowEnd(node)` (`:214-337`) — fecha o trecho **se `node` contém `_firstDeadNode`** (`_containsFirstDeadNode`, sobe pelos
   pais). Chamado em (o que decide onde o trecho TERMINA):
   - `then`/`else` de `if` instrução e elemento (`resolver.dart:1027-1060`); `then`/`else` do condicional (`:2270`, `:2289`);
     operando direito de `&&`/`||` (`binary_expression_resolver.dart:239`, `:268`); operando direito de padrão `||`
     (`analyzer/lib/src/dart/ast/ast.dart:11595`); cabeça de `case`/`default` (`resolver.dart:1083`, `:1098`); corpo do grupo
     de `case` (`:1126`, `handleMergedStatementCase` → último membro); caso de expressão `switch` (`:874`); corpo de `for`/`for-in`
     (`:2754`); corpo de `while` (`:3951`); corpo do `try` (`:3827`) e de cada `catch` (`:3842`); declaração de função
     (`:2814`), método (`:3201`), construtor (`:2341`); expressão de função (`function_expression_resolver.dart:68`);
     inicializador de variável de topo/campo (`variable_declaration_resolver.dart:69`).
   - **NÃO** chamam: `do-while` (o trecho do corpo segue até o `flowEnd` de fora), bloco solto `{ }`, rótulo, instrução de
     expressão, `return`, inicializador de local.

   Cálculo do intervalo quando fecha (`:252-333`):
   ```
   offset = firstDead.offset
   if node != firstDead:            // fechou num nó que CONTÉM o primeiro morto
      node = corpo (FunctionDeclaration/FunctionExpression/MethodDeclaration → body; BlockFunctionBody → block)
      if node is Block com instruções: node = última instrução
      if node is SwitchMember com instruções: node = última instrução
   else if parent(firstDead) is BinaryExpression: offset = operador          // `&&`/`||`: começa no operador
   if parent is SwitchMember && node == firstDead → relata o token `case`/`default` (keyword) e para (:228-235)
   casos especiais pelo pai do primeiro morto:
      Assertion e firstDead é a mensagem → não relata (:238-241)
      ConstructorDeclaration e firstDead é `;` (EmptyFunctionBody) ou bloco vazio → não relata (:242-250)
      ConstructorInitializer → relata o inicializador inteiro e offset = node.end (:278-284)
      DoStatement → relata `do` (ou `do {`) e `}` while…`;` separados; o resto começa depois do `;` (:285-308)
      ForParts (firstDead é a condição/atualizador) → node = último atualizador (:309-310)
      ForStatement (firstDead é o corpo) → `_reportForUpdaters` (:311-312, `:428-441`: relata do 1º ao último atualizador)
      Block cujo pai é ForStatement → `_reportForUpdaters` (:313-317)
      BinaryExpression → offset = operador, node = operando direito (:318-320)
      LogicalOrPattern e firstDead é o direito → offset = operador (:321-324)
   length = node.end - offset; relata DEAD_CODE se length > 0
   ```
   Depois de relatar, `_firstDeadNode = null`: o próximo nó inalcançável abre outro trecho.

3. `catch`: `tryStatementEnter`/`verifyCatchClause` (`:339-378`, `_CatchClausesVerifier` `:495-545`) — ver
   `dead_code_on_catch_subtype` e `dead_code_catch_following_catch`. O intervalo relatado vai do `catch` problemático até o
   fim do último `catch`, e entra em `_deadCatchClauseRanges` (nenhum `dead_code` dentro dele).

4. `?.`/`?[`/`?..` sobre variável **definitivamente não atribuída** de tipo anulável (`_verifyUnassignedSimpleIdentifier`,
   `:443-475`): relata DEAD_CODE no acesso inteiro (subindo por cadeias de invocação/propriedade/índice).

**Alcançabilidade (de onde vem o "inalcançável" dentro de expressões):** além de `return`/`throw`/`break`/`continue`/
`rethrow`, a análise de fluxo marca inalcançável depois de qualquer expressão de tipo estático `Never` (invocação, leitura de
variável ou getter `Never`, `x!` com `x` `Null`…), no ramo `false` de condição que é literal `true` ou variável cujo valor
escrito foi um literal booleano/condição (§ `dead_code`), no lado direito de `?.`/`?[`/`?..` quando o alvo é do tipo `Null`
(`nullAwareAccess_rightBegin`, `_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart:5041-5055`), em padrões
(Apêndice A), e depois de `switch` exaustivo sem saída.

### Rodada 4 — família B

Complementos da rodada 4 (2026-10-04) para os códigos da família B que a auditoria (§0.A) achou sem algum dos seis campos: os emissores e os desvios do método emissor, copiados da fonte por script. O fluxo de padrões, que cobre boa parte da família, está escrito à mão no plano T6 (§G).

### B.R4-1 Família B — guardas dos emissores (rodada 4, gerado)

Esta parte foi **gerada por script** (`E:\dftemp\analise\spec-r4\final\guardas.py`) para os códigos que a auditoria (§0.A) achou sem o campo "Supressões e ordem" e que não receberam complemento escrito à mão. Para cada código ela traz os pontos de emissão (arquivo:linha na tag 3.6.2, com a classe e o método que os contêm) e os **desvios do método emissor que antecedem o relato** (`if`, `return`, `continue`, `case`), copiados da fonte com o número da linha. É a matéria-prima do campo "Supressões": cada `return` antes do relato é uma condição em que o código não sai. O script não interpreta os desvios; quem implementar o código lê o trecho citado. As supressões gerais (tipo inválido, ordem das fases, homônimos) estão nos planos T1, T4 e T5 (§G).

##### `not_initialized_non_nullable_instance_field` — guardas do emissor (perda 15: FN 12, FP 2, msg 0, pos 1)
- **Emissão:** `analyzer/lib/src/error/constructor_fields_verifier.dart:191` em `_Constructor.reportNotInitializedNonNullable` (começa em `:176`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    180: if (notInitNonNullableFields.isEmpty) {
    181: return;
    187: for (var name in names) {
  ```
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:4859` em `ErrorVerifier._checkForNotInitializedNonNullableInstanceFields` (começa em `:4837`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   4840: if (fieldDeclaration.isStatic) return;
   4843: if (fields.isLate) return;
   4844: if (fields.isFinal) return;
   4846: if (_isEnclosingClassFfiStruct) return;
   4847: if (_isEnclosingClassFfiUnion) return;
   4849: for (var field in fields.variables) {
   4851: if (fieldElement.isAbstract || fieldElement.isExternal) continue;
   4852: if (field.initializer != null) continue;
   4855: if (!typeSystem.isPotentiallyNonNullable(type)) continue;
  ```
- **Fase e supressões gerais:** fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `pattern_variable_assignment_inside_guard` — guardas do emissor (perda 15: FN 15, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/resolver.dart:5192` em `ScopeResolverVisitor.visitSimpleIdentifier` (começa em `:5151`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   5162: return;
   5167: if (parent is FieldFormalParameter) {
   5168: return;
   5169: } else if (parent is ConstructorDeclaration && parent.returnType == node) {
   5170: return;
   5171: } else if (parent is ConstructorFieldInitializer &&
   5173: return;
   5175: if (parent is Label) {
   5176: return;
   5180: if (element is! VariableElement) {
   5181: return;
   5185: if (kind == ElementKind.LOCAL_VARIABLE || kind == ElementKind.PARAMETER) {
   5187: if (node.inSetterContext()) {
   5188: if (element is PatternVariableElementImpl &&
  ```
- **Fase e supressões gerais:** fase 3c (`ResolverVisitor`); operandos de tipo `InvalidType` não chegam ao relato (T4).

##### `unnecessary_cast_pattern` — guardas do emissor (perda 13: FN 13, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/shared_type_analyzer.dart:148` em `SharedTypeAnalyzerErrors.matchedTypeIsSubtypeOfRequired` (começa em `:141`).
  Nenhum desvio entre o início do método e o relato: o relato é incondicional dentro dele; a supressão está em quem o chama.

##### `invalid_use_of_null_value` — guardas do emissor (perda 12: FN 12, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/nullable_dereference_verifier.dart:44` em `NullableDereferenceVerifier.report` (começa em `:39`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
     43: if (receiverType == _typeSystem.typeProvider.nullType) {
  ```

##### `missing_variable_pattern` — guardas do emissor (perda 12: FN 12, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/resolution_visitor.dart:2001` em `_VariableBinderErrors.logicalOrPatternBranchMissingVariable` (começa em `:1993`).
  Nenhum desvio entre o início do método e o relato: o relato é incondicional dentro dele; a supressão está em quem o chama.

##### `unnecessary_null_check_pattern` — guardas do emissor (perda 12: FN 12, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/shared_type_analyzer.dart:133` em `SharedTypeAnalyzerErrors.matchedTypeIsStrictlyNonNullable` (começa em `:121`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    125: if (pattern is NullAssertPattern) {
    130: } else if (pattern is NullCheckPattern) {
  ```

##### `unnecessary_set_literal` — guardas do emissor (perda 12: FN 12, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/best_practices_verifier.dart:1314` em `BestPracticesVerifier._checkForUnnecessarySetLiteral` (começa em `:1283`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1285: if (body is ExpressionFunctionBodyImpl) {
   1289: if (parameterType is FunctionType) {
   1291: } else {
   1293: if (parent is! FunctionDeclaration) return;
   1296: if (returnType == null) return;
   1299: if (returnType is VoidType) {
   1301: } else if (returnType is ParameterizedType &&
   1306: } else {
   1309: if (isReturnVoid) {
   1311: if (expression is SetOrMapLiteralImpl && expression.isSet) {
  ```

##### `invalid_pattern_variable_in_shared_case_scope` — guardas do emissor (perda 11: FN 11, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/resolver.dart:896` em `ResolverVisitor.finishJoinedPatternVariable` (começa em `:878`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    889: if (location == JoinedPatternVariableLocation.sharedCaseScope) {
    890: for (var reference in variable.references) {
    891: if (variable.inconsistency ==
  ```
- **Emissão:** `analyzer/lib/src/generated/resolver.dart:903` em `ResolverVisitor.finishJoinedPatternVariable` (começa em `:878`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    889: if (location == JoinedPatternVariableLocation.sharedCaseScope) {
    890: for (var reference in variable.references) {
    891: if (variable.inconsistency ==
    899: } else if (variable.inconsistency ==
  ```
- **Emissão:** `analyzer/lib/src/generated/resolver.dart:911` em `ResolverVisitor.finishJoinedPatternVariable` (começa em `:878`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    889: if (location == JoinedPatternVariableLocation.sharedCaseScope) {
    890: for (var reference in variable.references) {
    891: if (variable.inconsistency ==
    899: } else if (variable.inconsistency ==
    906: } else if (variable.inconsistency ==
  ```
- **Fase e supressões gerais:** fase 3c (`ResolverVisitor`); operandos de tipo `InvalidType` não chegam ao relato (T4).

##### `late_final_field_with_const_constructor` — guardas do emissor (perda 10: FN 10, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:4089` em `ErrorVerifier._checkForLateFinalFieldWithConstConstructor` (começa em `:4068`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   4069: if (node.isStatic) return;
   4072: if (!variableList.isFinal) return;
   4075: if (lateKeyword == null) return;
   4078: if (enclosingClass == null) {
   4080: return;
   4085: if (!hasGenerativeConstConstructor) return;
  ```
- **Fase e supressões gerais:** fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `switch_case_completes_normally` — guardas do emissor (perda 10: FN 10, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/shared_type_analyzer.dart:237` em `SharedTypeAnalyzerErrors.switchCaseCompletesNormally` (começa em `:233`).
  Nenhum desvio entre o início do método e o relato: o relato é incondicional dentro dele; a supressão está em quem o chama.

##### `unnecessary_wildcard_pattern` — guardas do emissor (perda 10: FN 10, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/shared_type_analyzer.dart:250` em `SharedTypeAnalyzerErrors.unnecessaryWildcardPattern` (começa em `:242`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    246: switch (kind) {
    247: case UnnecessaryWildcardKind.logicalAndPatternOperand:
  ```

##### `dead_null_aware_expression` — guardas do emissor (perda 8: FN 6, FP 2, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:3049` em `ErrorVerifier._checkForDeadNullCoalesce` (começa em `:3045`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   3046: if (typeSystem.isStrictlyNonNullable(lhsType)) {
  ```
- **Fase e supressões gerais:** fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `equal_keys_in_map_pattern` — guardas do emissor (perda 8: FN 8, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/diagnostic/diagnostic_factory.dart:253` em `DiagnosticFactory.equalKeysInMapPattern` (começa em `:247`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    249: return AnalysisError.tmp(
  ```

##### `not_assigned_potentially_non_nullable_local_variable` — guardas do emissor (perda 8: FN 8, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/resolver.dart:685` em `ResolverVisitor.checkReadOfNotAssignedLocalVariable` (começa em `:643`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    647: if (flowAnalysis.flow == null) {
    648: return;
    651: if (!node.inGetterContext()) {
    652: return;
    655: if (element is VariableElement) {
    660: if (element.isLate) {
    661: if (unassigned) {
    668: return;
    671: if (!assigned) {
    672: if (element.isFinal) {
    678: return;
    681: if (typeSystem.isPotentiallyNonNullable(element.type)) {
  ```
- **Fase e supressões gerais:** fase 3c (`ResolverVisitor`); operandos de tipo `InvalidType` não chegam ao relato (T4).

##### `not_initialized_non_nullable_variable` — guardas do emissor (perda 8: FN 8, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:4909` em `ErrorVerifier._checkForNotInitializedNonNullableVariable` (começa em `:4872`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   4881: if (node.isLate) {
   4882: return;
   4886: if (parent is FieldDeclaration) {
   4887: if (parent.externalKeyword != null) {
   4888: return;
   4890: } else if (parent is TopLevelVariableDeclaration) {
   4891: if (parent.externalKeyword != null) {
   4892: return;
   4896: if (node.type == null) {
   4897: return;
   4901: if (!typeSystem.isPotentiallyNonNullable(type)) {
   4902: return;
   4905: for (var variable in node.variables) {
   4906: if (variable.initializer == null) {
  ```
- **Fase e supressões gerais:** fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `pattern_never_matches_value_type` — guardas do emissor (perda 8: FN 5, FP 2, msg 1, pos 0)
- **Emissão:** `analyzer/lib/src/generated/resolver.dart:636` em `ResolverVisitor.checkPatternNeverMatchesValueType` (começa em `:615`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    621: if (context.irrefutableContext == null) {
    622: if (!typeSystem.canBeSubtypeOf(matchedValueType, requiredType)) {
    624: if (pattern is CastPatternImpl) {
    626: } else if (pattern is DeclaredVariablePatternImpl) {
    628: } else if (pattern is ObjectPatternImpl) {
    630: } else if (pattern is WildcardPatternImpl) {
  ```
- **Fase e supressões gerais:** fase 3c (`ResolverVisitor`); operandos de tipo `InvalidType` não chegam ao relato (T4).

##### `unnecessary_null_comparison` — guardas do emissor (perda 8: FN 8, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/binary_expression_resolver.dart:128` em `BinaryExpressionResolver._resolveEqual` (começa em `:92`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    101: if (!leftExtensionOverride) {
    111: if (!leftExtensionOverride) {
  ```
- **Emissão:** `analyzer/lib/src/dart/resolver/binary_expression_resolver.dart:129` em `BinaryExpressionResolver._resolveEqual` (começa em `:92`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    101: if (!leftExtensionOverride) {
    111: if (!leftExtensionOverride) {
  ```
- **Emissão:** `analyzer/lib/src/error/best_practices_verifier.dart:1063` em `BestPracticesVerifier._checkForInvariantNullComparison` (começa em `:1060`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1062: if (node.operator.type == TokenType.BANG_EQ) {
  ```
- **Emissão:** `analyzer/lib/src/error/best_practices_verifier.dart:1065` em `BestPracticesVerifier._checkForInvariantNullComparison` (começa em `:1060`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1062: if (node.operator.type == TokenType.BANG_EQ) {
   1064: } else if (node.operator.type == TokenType.EQ_EQ) {
  ```

##### `unnecessary_type_check` — guardas do emissor (perda 8: FN 3, FP 5, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/best_practices_verifier.dart:771` em `BestPracticesVerifier.visitTopLevelVariableDeclaration` (começa em `:748`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    751: if (!_invalidAccessVerifier._inTestDirectory) {
    752: for (var decl in node.variables.variables) {
  ```
- **Emissão:** `analyzer/lib/src/error/best_practices_verifier.dart:772` em `BestPracticesVerifier.visitTopLevelVariableDeclaration` (começa em `:748`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    751: if (!_invalidAccessVerifier._inTestDirectory) {
    752: for (var decl in node.variables.variables) {
  ```
- **Emissão:** `analyzer/lib/src/error/best_practices_verifier.dart:784` em `BestPracticesVerifier._checkAllTypeChecks` (começa em `:773`).
  Nenhum desvio entre o início do método e o relato: o relato é incondicional dentro dele; a supressão está em quem o chama.
- **Emissão:** `analyzer/lib/src/error/best_practices_verifier.dart:785` em `BestPracticesVerifier._checkAllTypeChecks` (começa em `:773`).
  Nenhum desvio entre o início do método e o relato: o relato é incondicional dentro dele; a supressão está em quem o chama.

##### `non_nullable_equals_parameter` — guardas do emissor (perda 7: FN 7, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/best_practices_verifier.dart:1181` em `BestPracticesVerifier._checkForNullableEqualsParameterType` (começa em `:1151`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1152: if (node.name.type != TokenType.EQ_EQ) {
   1153: return;
   1157: if (parameters == null) {
   1158: return;
   1161: if (parameters.parameters.length != 1) {
   1162: return;
   1167: if (parameterElement == null) {
   1168: return;
   1172: if (!type.isDartCoreObject && type is! DynamicType) {
   1175: return;
   1178: if (_typeSystem.isNullable(parameterElement.type)) {
  ```

##### `null_argument_to_non_null_type` — guardas do emissor (perda 7: FN 7, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/null_safe_api_verifier.dart:73` em `NullSafeApiVerifier._checkTypes` (começa em `:56`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
     61: if (args.arguments.length > 1 || !_typeSystem.isNonNullable(type)) return;
     66: if (argument != null && argumentType == null) return;
     70: if (argumentIsNull) {
  ```

##### `pattern_assignment_not_local_variable` — guardas do emissor (perda 7: FN 7, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/resolution_visitor.dart:206` em `ResolutionVisitor.visitAssignedVariablePattern` (começa em `:189`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    196: if (element == null) {
    202: } else if (!(element is LocalVariableElement ||
  ```

##### `unnecessary_null_assert_pattern` — guardas do emissor (perda 7: FN 7, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/shared_type_analyzer.dart:128` em `SharedTypeAnalyzerErrors.matchedTypeIsStrictlyNonNullable` (começa em `:121`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    125: if (pattern is NullAssertPattern) {
  ```

##### `cast_from_null_always_fails` — guardas do emissor (perda 6: FN 6, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/best_practices_verifier.dart:148` em `BestPracticesVerifier.visitAsExpression` (começa em `:135`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    136: if (isUnnecessaryCast(node, _typeSystem)) {
    143: if (type != null &&
  ```
- **Emissão:** `analyzer/lib/src/error/best_practices_verifier.dart:179` em `BestPracticesVerifier.visitCastPattern` (começa em `:170`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    173: if (type != null &&
  ```

##### `duplicate_pattern_field` — guardas do emissor (perda 6: FN 6, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/diagnostic/diagnostic_factory.dart:165` em `DiagnosticFactory.duplicatePatternField` (começa em `:151`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    161: return AnalysisError.tmp(
  ```

##### `not_null_aware_null_spread` — guardas do emissor (perda 6: FN 6, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/literal_element_verifier.dart:198` em `LiteralElementVerifier._verifySpreadForListOrSet` (começa em `:176`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    178: if (expressionType is DynamicType) {
    179: if (_errorVerifier.strictCasts) {
    185: return;
    188: if (typeSystem.isSubtypeOf(expressionType, NeverTypeImpl.instance)) {
    189: return;
    192: if (typeSystem.isSubtypeOf(expressionType, typeSystem.nullNone)) {
    193: if (isNullAware) {
    194: return;
  ```
- **Emissão:** `analyzer/lib/src/error/literal_element_verifier.dart:289` em `LiteralElementVerifier._verifySpreadForMap` (começa em `:267`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    269: if (expressionType is DynamicType) {
    270: if (_errorVerifier.strictCasts) {
    276: return;
    279: if (typeSystem.isSubtypeOf(expressionType, NeverTypeImpl.instance)) {
    280: return;
    283: if (typeSystem.isSubtypeOf(expressionType, typeSystem.nullNone)) {
    284: if (isNullAware) {
    285: return;
  ```

##### `unnecessary_nan_comparison` — guardas do emissor (perda 5: FN 5, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/best_practices_verifier.dart:262` em `BestPracticesVerifier.visitConstantPattern` (começa em `:258`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    259: if (node.expression.isDoubleNan) {
  ```
- **Emissão:** `analyzer/lib/src/error/best_practices_verifier.dart:1054` em `BestPracticesVerifier._checkForInvariantNanComparison` (começa em `:1031`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1046: if (node.leftOperand.isDoubleNan) {
   1048: } else if (node.rightOperand.isDoubleNan) {
   1053: if (node.operator.type == TokenType.BANG_EQ) {
  ```
- **Emissão:** `analyzer/lib/src/error/best_practices_verifier.dart:1056` em `BestPracticesVerifier._checkForInvariantNanComparison` (começa em `:1031`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1046: if (node.leftOperand.isDoubleNan) {
   1048: } else if (node.rightOperand.isDoubleNan) {
   1053: if (node.operator.type == TokenType.BANG_EQ) {
   1055: } else if (node.operator.type == TokenType.EQ_EQ) {
  ```

##### `break_label_on_switch_member` — guardas do emissor (perda 4: FN 4, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:431` em `ErrorVerifier.visitBreakStatement` (começa em `:424`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    426: if (labelNode != null) {
    428: if (labelElement is LabelElementImpl && labelElement.isOnSwitchMember) {
  ```
- **Fase e supressões gerais:** fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `case_expression_type_is_not_switch_expression_subtype` — guardas do emissor (perda 4: FN 4, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/shared_type_analyzer.dart:42` em `SharedTypeAnalyzerErrors.caseExpressionTypeMismatch` (começa em `:33`).
  Nenhum desvio entre o início do método e o relato: o relato é incondicional dentro dele; a supressão está em quem o chama.

##### `duplicate_pattern_assignment_variable` — guardas do emissor (perda 4: FN 4, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/diagnostic/diagnostic_factory.dart:32` em `DiagnosticFactory.duplicateAssignmentPatternVariable` (começa em `:22`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
     28: return AnalysisError.tmp(
  ```

##### `null_check_always_fails` — guardas do emissor (perda 4: FN 4, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/best_practices_verifier.dart:694` em `BestPracticesVerifier.visitPostfixExpression` (começa em `:688`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    690: if (node.operator.type == TokenType.BANG &&
  ```

##### `nullable_type_in_catch_clause` — guardas do emissor (perda 4: FN 4, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/best_practices_verifier.dart:1200` em `BestPracticesVerifier._checkForNullableTypeInCatchClause` (começa em `:1186`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1188: if (typeNode == null) {
   1189: return;
   1193: if (typeObj is InvalidType) {
   1194: return;
   1197: if (_typeSystem.isPotentiallyNullable(typeObj)) {
  ```

##### `nullable_type_in_implements_clause` — guardas do emissor (perda 4: FN 4, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/named_type_resolver.dart:400` em `NamedTypeResolver._verifyNullability` (começa em `:388`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    389: if (identical(node, classHierarchy_namedType)) {
    390: if (type.nullabilitySuffix == NullabilitySuffix.question) {
    392: if (parent is ExtendsClause || parent is ClassTypeAlias) {
    397: } else if (parent is ImplementsClause) {
  ```

##### `refutable_pattern_in_irrefutable_context` — guardas do emissor (perda 4: FN 4, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/shared_type_analyzer.dart:192` em `SharedTypeAnalyzerErrors.refutablePatternInIrrefutableContext` (começa em `:188`).
  Nenhum desvio entre o início do método e o relato: o relato é incondicional dentro dele; a supressão está em quem o chama.

##### `unnecessary_question_mark` — guardas do emissor (perda 4: FN 4, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/best_practices_verifier.dart:672` em `BestPracticesVerifier.visitNamedType` (começa em `:659`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    663: if (question != null) {
    667: if ((type is InterfaceType && type.element == _nullType.element ||
  ```

##### `dead_code_on_catch_subtype` — guardas do emissor (perda 3: FN 3, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/dead_code_verifier.dart:535` em `_CatchClausesVerifier.nextCatchClause` (começa em `:509`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    514: if (currentType == null || currentType.isDartCoreObject) {
    515: if (catchClause != catchClauses.last) {
    525: return;
    530: for (var type in _visitedTypes) {
    531: if (_typeSystem.isSubtypeOf(currentType, type)) {
  ```

##### `definitely_unassigned_late_local_variable` — guardas do emissor (perda 3: FN 1, FP 0, msg 0, pos 2)
- **Emissão:** `analyzer/lib/src/generated/resolver.dart:664` em `ResolverVisitor.checkReadOfNotAssignedLocalVariable` (começa em `:643`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    647: if (flowAnalysis.flow == null) {
    648: return;
    651: if (!node.inGetterContext()) {
    652: return;
    655: if (element is VariableElement) {
    660: if (element.isLate) {
    661: if (unassigned) {
  ```
- **Fase e supressões gerais:** fase 3c (`ResolverVisitor`); operandos de tipo `InvalidType` não chegam ao relato (T4).

##### `duplicate_variable_pattern` — guardas do emissor (perda 3: FN 3, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/resolution_visitor.dart:1983` em `_VariableBinderErrors.duplicateVariablePattern` (começa em `:1975`).
  Nenhum desvio entre o início do método e o relato: o relato é incondicional dentro dele; a supressão está em quem o chama.

##### `unnecessary_final` — guardas do emissor (perda 3: FN 3, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/best_practices_verifier.dart:832` em `BestPracticesVerifier._checkFinalParameter` (começa em `:828`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    829: if (node.isFinal) {
  ```

##### `cast_from_nullable_always_fails` — guardas do emissor (perda 2: FN 2, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/resolver.dart:1883` em `ResolverVisitor.visitAsExpression` (começa em `:1852`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1873: if (staticType != null && expression is SimpleIdentifier) {
   1876: if (element is PromotableElement &&
  ```
- **Fase e supressões gerais:** fase 3c (`ResolverVisitor`); operandos de tipo `InvalidType` não chegam ao relato (T4).

##### `expected_two_map_pattern_type_arguments` — guardas do emissor (perda 2: FN 2, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/resolver.dart:1535` em `ResolverVisitor.resolveMapPattern` (começa em `:1513`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1523: if (typeArgumentsList != null) {
   1527: if (length == 2) {
   1532: } else {
  ```
- **Fase e supressões gerais:** fase 3c (`ResolverVisitor`); operandos de tipo `InvalidType` não chegam ao relato (T4).

##### `late_final_local_already_assigned` — guardas do emissor (perda 2: FN 1, FP 1, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/assignment_expression_resolver.dart:372` em `AssignmentExpressionShared.checkFinalAlreadyAssigned` (começa em `:354`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    359: if (flow == null) return;
    361: if (left is SimpleIdentifier) {
    363: if (element is PromotableElement) {
    367: if (element.isFinal) {
    368: if (element.isLate) {
    369: if (isForEachIdentifier || assigned) {
  ```
- **Emissão:** `analyzer/lib/src/generated/resolver.dart:1380` em `ResolverVisitor.resolveAssignedVariablePattern` (começa em `:1364`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1369: if (element is! PromotableElement) {
   1370: return PatternResult(
   1374: if (element.isFinal) {
   1376: if (element.isLate) {
   1377: if (flow.isAssigned(element)) {
  ```
- **Fase e supressões gerais:** fase 3c (`ResolverVisitor`); operandos de tipo `InvalidType` não chegam ao relato (T4).

##### `read_potentially_unassigned_final` — guardas do emissor (perda 2: FN 2, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/generated/resolver.dart:675` em `ResolverVisitor.checkReadOfNotAssignedLocalVariable` (começa em `:643`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    647: if (flowAnalysis.flow == null) {
    648: return;
    651: if (!node.inGetterContext()) {
    652: return;
    655: if (element is VariableElement) {
    660: if (element.isLate) {
    661: if (unassigned) {
    668: return;
    671: if (!assigned) {
    672: if (element.isFinal) {
  ```
- **Fase e supressões gerais:** fase 3c (`ResolverVisitor`); operandos de tipo `InvalidType` não chegam ao relato (T4).

##### `relational_pattern_operator_return_type_not_assignable_to_bool` — guardas do emissor (perda 2: FN 2, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/shared_type_analyzer.dart:217` em `SharedTypeAnalyzerErrors.relationalPatternOperatorReturnTypeNotAssignableToBool` (começa em `:210`).
  Nenhum desvio entre o início do método e o relato: o relato é incondicional dentro dele; a supressão está em quem o chama.

##### `rest_element_in_map_pattern` — guardas do emissor (perda 2: FN 2, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/shared_type_analyzer.dart:228` em `SharedTypeAnalyzerErrors.restPatternInMap` (começa em `:222`).
  Nenhum desvio entre o início do método e o relato: o relato é incondicional dentro dele; a supressão está em quem o chama.

##### `unnecessary_no_such_method` — guardas do emissor (perda 2: FN 2, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/best_practices_verifier.dart:1259` em `BestPracticesVerifier._checkForUnnecessaryNoSuchMethod` (começa em `:1234`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1235: if (node.name.lexeme != FunctionElement.NO_SUCH_METHOD_METHOD_NAME) {
   1236: return false;
   1239: if (invocation is MethodInvocation &&
   1243: if (name.name == FunctionElement.NO_SUCH_METHOD_METHOD_NAME) {
   1246: return methodElement is MethodElement &&
   1251: return false;
   1255: if (body is ExpressionFunctionBody) {
   1256: if (isNonObjectNoSuchMethodInvocation(body.expression)) {
  ```
- **Emissão:** `analyzer/lib/src/error/best_practices_verifier.dart:1271` em `BestPracticesVerifier._checkForUnnecessaryNoSuchMethod` (começa em `:1234`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   1235: if (node.name.lexeme != FunctionElement.NO_SUCH_METHOD_METHOD_NAME) {
   1236: return false;
   1239: if (invocation is MethodInvocation &&
   1243: if (name.name == FunctionElement.NO_SUCH_METHOD_METHOD_NAME) {
   1246: return methodElement is MethodElement &&
   1251: return false;
   1255: if (body is ExpressionFunctionBody) {
   1256: if (isNonObjectNoSuchMethodInvocation(body.expression)) {
   1261: return true;
   1263: } else if (body is BlockFunctionBody) {
   1265: if (statements.length == 1) {
   1267: if (returnStatement is ReturnStatement &&
  ```

##### `dead_code_catch_following_catch` — guardas do emissor (perda 1: FN 1, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/error/dead_code_verifier.dart:520` em `_CatchClausesVerifier.nextCatchClause` (começa em `:509`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    514: if (currentType == null || currentType.isDartCoreObject) {
    515: if (catchClause != catchClauses.last) {
  ```

##### `duplicate_rest_element_in_pattern` — guardas do emissor (perda 1: FN 1, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/diagnostic/diagnostic_factory.dart:190` em `DiagnosticFactory.duplicateRestElementInPattern` (começa em `:181`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    186: return AnalysisError.tmp(
  ```

##### `empty_map_pattern` — guardas do emissor (perda 1: FN 1, FP 0, msg 0, pos 0)
- **O resto conta como elemento (2026-10-08):** `case {...}:` tem o `RestPatternElement` em `MapPattern.elements`, então
  não é vazio: só `rest_element_in_map_pattern`. `c2_sintaticos` relata vazio só sem entradas **e** sem resto.
- **Emissão:** `analyzer/lib/src/dart/resolver/shared_type_analyzer.dart:104` em `SharedTypeAnalyzerErrors.emptyMapPattern` (começa em `:99`).
  Nenhum desvio entre o início do método e o relato: o relato é incondicional dentro dele; a supressão está em quem o chama.

##### `expected_one_list_pattern_type_arguments` — guardas do emissor (perda 1: FN 1, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/dart/resolver/list_pattern_resolver.dart:30` em `ListPatternResolver.resolve` (começa em `:18`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
     23: if (typeArguments != null) {
     27: if (length != 1) {
  ```

##### `invalid_null_aware_operator` — guardas do emissor (perda 1: FN 1, FP 0, msg 0, pos 0)
- **Emissão:** `analyzer/lib/src/diagnostic/diagnostic_factory.dart:276` em `DiagnosticFactory.invalidNullAwareAfterShortCircuit` (começa em `:268`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    271: return AnalysisError.tmp(
  ```
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:5563` em `ErrorVerifier._checkForUnnecessaryNullAware` (começa em `:5554`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   5555: if (target is SuperExpression) {
   5556: return;
   5562: if (operator.type == TokenType.QUESTION) {
  ```
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:5567` em `ErrorVerifier._checkForUnnecessaryNullAware` (começa em `:5554`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   5555: if (target is SuperExpression) {
   5556: return;
   5562: if (operator.type == TokenType.QUESTION) {
   5566: } else if (operator.type == TokenType.QUESTION_PERIOD) {
  ```
- **Emissão:** `analyzer/lib/src/generated/error_verifier.dart:5570` em `ErrorVerifier._checkForUnnecessaryNullAware` (começa em `:5554`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
   5555: if (target is SuperExpression) {
   5556: return;
   5562: if (operator.type == TokenType.QUESTION) {
   5566: } else if (operator.type == TokenType.QUESTION_PERIOD) {
   5569: } else if (operator.type == TokenType.QUESTION_PERIOD_PERIOD) {
  ```
- Outros pontos de uso: `analyzer/lib/src/generated/error_verifier.dart:5573`, `analyzer/lib/src/generated/error_verifier.dart:5633`.
- **Fase e supressões gerais:** fase 5c (`ErrorVerifier`, sobre a AST resolvida; sem porta de sintaxe — T5).

##### `non_exhaustive_switch_expression` — guardas do emissor (perda 1: FN 0, FP 0, msg 1, pos 0)
- **Emissão:** `analyzer/lib/src/dart/constant/constant_verifier.dart:969` em `ConstantVerifier._validateSwitchExhaustiveness` (começa em `:883`).
  Desvios do método antes do relato (fonte, com o número da linha):
  ```dart
    911: for (var caseNode in caseNodes) {
    912: if (caseNode is SwitchCase) {
    914: } else if (caseNode is SwitchDefault) {
    916: } else if (caseNode is CaseNodeImpl) {
    922: } else {
    934: if (!patternConverter.hasInvalidType) {
    938: for (var caseUnreachability in caseUnreachabilities) {
    949: if (nonExhaustiveness != null) {
    950: if (reportNonExhaustive) {
    959: for (var witness in nonExhaustiveness.witnesses) {
    962: if (correctionDataBuffer.isComplete) {
  ```
- **Fase e supressões gerais:** fase 5a (`ConstantVerifier`); ver D.R4-2.


## C. Declarações, herança, construtores, limites, FFI

**Estado em 2026-10-04 (escrito, não compilado).** Ganharam emissor nesta data, em
`crates/analise/src/c2_sintaticos.rs` (só a árvore; testes de unidade no arquivo, não rodados):
`rethrow_outside_catch`, `return_in_generator`, `label_undefined`, `label_in_outer_scope`,
`continue_label_invalid`, `invalid_modifier_on_setter`, `uri_with_interpolation` (diretivas; o
`@docImport` não) e `main_is_not_function`. Não feitos por dependerem do parser, que descarta o dado:
`non_sync_factory` e `invalid_modifier_on_constructor` (o modificador do corpo do construtor) e
`yield_in_non_generator` (fora de gerador o `yield` não vira comando). O resto da família não foi
tocado nesta data.

**Estado em 2026-10-05 (escrito, não compilado).** `non_sync_factory` e
`invalid_modifier_on_constructor` saem do parser (`parser/declarations.rs`, `parse_constructor_resto`).
`yield_in_non_generator`: o parser só faz do `yield` comando onde o fasta faz (`looksLikeYieldStatement`)
e então relata como o `parseYieldStatement`, no token `yield` (`YIELD_IN_NON_GENERATOR` também com
`*`); o `c2_sintaticos` relata de novo no comando inteiro, como o `YieldStatementResolver`
(`YIELD_EACH_IN_NON_GENERATOR` com `*`). São dois diagnósticos, de intervalos diferentes, como no
analyzer.

### Parte A_heranca — herança, sobrescrita e conflitos de membros (analyzer 3.6.2)

Fonte citada: `E:\references\dart-sdk-3.6.2\pkg\analyzer\lib` (prefixo `analyzer/lib/` omitido abaixo:
`src/...:linha`). DartForge: `E:\MyRustProjects\dartforge\crates\...`. Placar base:
`E:\dftemp\analise\trab\placar-base-r3.txt`.

#### 0. Achados transversais (valem para quase todos os códigos do grupo)

##### 0.1 Declarações homônimas compartilham caches no 3.6.2 (`ElementImpl.==` por localização)

`src/dart/element/element.dart:2872` — `ElementImpl.operator ==` compara `kind` e `location`
(`hashCode` = `location.hashCode`, `:2472`). Dois `class A`/`enum E`/`mixin M` de mesmo nome na
mesma biblioteca (o caso de todo `augment class X ...` **sem** o experimento: o parser recupera
`augment` como variável de topo — `expected_token` + `missing_const_final_var_or_type` — e o resto
vira uma **segunda declaração** com `duplicate_definition`) são elementos **iguais**. Consequências:

- `InheritanceManager3._interfaces` (`src/dart/element/inheritance_manager3.dart:69`, consulta em
  `getInterface` `:239`) devolve para a 2ª declaração a interface já calculada para a 1ª (na prática
  a 1ª — prova: `unused_element/UnusedElement__parameter_isUsed_overrid_83bf2fbf.dart`: o 1º `B` relata
  `non_abstract_class_inherits_abstract_member` 'A._m' e o 2º, que declara `_m` concreto, relata
  `concrete_class_with_abstract_member` no próprio `_m` — a interface/implemented é a do 1º).
  Isso gera, **na posição do nome da 2ª declaração**, cópias de `non_abstract_class_inherits_abstract_member`,
  `inconsistent_inheritance_getter_and_method`, `conflicting_field_and_method`, `conflicting_method_and_field`,
  `conflicting_static_and_instance` etc., calculadas com a interface da 1ª mas com os **membros (AST)
  da 2ª**.
- `ClassHierarchy._map` (`src/dart/element/class_hierarchy.dart:14,44`) idem (`conflicting_generic_interfaces`).
- `_checkForRecursiveInterfaceInheritance` (`src/error/inheritance_override.dart:592`) compara
  `classElement == element`: `augment class A extends A {}` (2º A referindo o 1º, porque o escopo
  resolve o nome para a **primeira** declaração) é "A herda de si mesmo" — ver §recursive.
- `DuplicationDefinitionContext._instanceElementContexts` (`src/error/duplicate_definition_verifier.dart:307`)
  junta membros das duas (o DartForge já faz isso em `analise/src/duplicatas.rs` com `juntar`).
- O que **não** é cache por elemento continua separado: `element.augmented.constants`
  (`enum_without_constants`) usa só as constantes de cada declaração.

**No DartForge** quase todo verificador de herança pula essas classes, e a maioria dos FN do grupo vem
daí:
- `crates/analise/src/clausulas.rs:1114` (`verificador_de_heranca_prossegue`): classe com nome repetido
  na biblioteca fica fora da lista `classes` usada por `sobrescritas_invalidas`, `membros_de_enum`,
  `membros_abstratos`, `getters_e_setters` (`crates/paridade/src/analise.rs:503-523`) — inclusive a **1ª**
  declaração (FN em `NonAbstractClassInheritsAbstractMember__9a2979d9.dart:5:7`,
  `InconsistentInheritance__enum_returnTyp_221d07c1.dart:9:6`).
- `crates/types/src/sobrescritas.rs:1358-1364` (`membros_em_conflito`) e `:1525-1534`
  (`estaticos_de_enum`): pulam a classe se **qualquer linha da unidade** começa com `augment ` ou se o
  nome é repetido.
- `crates/types/src/sobrescritas.rs:1029-1042` (`membros_abstratos`): pula se algum membro tem
  `augment` ou se o texto da declaração tem linha `augment `.
- `getters_e_setters` (`sobrescritas.rs:699-710`): campo com `augment` → biblioteca inteira fora.

Proposta: (1) ligar esses filtros só quando a biblioteca tem `Feature::Augmentations`/`Macros`
(sem o experimento `augment` nem é modificador — `frontend/src/parser/declarations.rs:269`
`parse_augment_opt` já devolve `false`); (2) para homônimos, calcular a interface/hierarquia uma vez
pela **primeira** declaração (chave = (biblioteca, nome, espécie)) e reutilizá-la para as seguintes,
relatando no nome de cada uma com os membros AST dela; (3) referências de tipo a um nome repetido
resolvem para a primeira declaração (hoje `clausulas.rs:731-736` marca `incerta`).

##### 0.2 Ordem dos verificadores e supressões gerais

`src/dart/analysis/library_analyzer.dart:420-456` (`_computeVerifyErrors`): `ConstantVerifier` →
`InheritanceOverrideVerifier.verifyUnit` (`:432`) → `ErrorVerifier` (`:441-451`). Antes disso, por
biblioteca, `MemberDuplicateDefinitionVerifier.checkLibrary`
(`src/error/duplicate_definition_verifier.dart:841`: todos os `_checkUnit` de instância `:750`, depois
todos os `_checkUnitStatic` `:773`). `OverrideVerifier` roda em `_computeWarnings` (`:483`).

`InheritanceOverrideVerifier.verifyUnit` (`src/error/inheritance_override.dart:38-129`) só visita
`ClassDeclaration`, `ClassTypeAlias`, `EnumDeclaration`, `MixinDeclaration` (não extension type nem
extension). Fragmento de augmentation (`element.isAugmentation`, só com o experimento) roda só
`_checkDirectSuperTypes` e para (`:58-61`, `:77-80`, `:96-99`, `:115-118`). `_ClassVerifier.verify`
(`:187`) **para** (retorna `true`, sem nada depois) em: supertipo direto proibido/`Enum` em classe
concreta/mixin de enum com campo (`_checkDirectSuperTypes` `:486`), `CONCRETE_CLASS_HAS_ENUM_SUPERINTERFACE`
transitivo (`:196-205`), herança recursiva (`:207`). Depois disso não para mais: conflitos (`:215`),
membros de mixins (`:233-239`), membros declarados (`:245-274`), enum (`:276-277`),
`GetterSetterTypesVerifier.checkInterface` (`:279-282`), membros abstratos/implementação (`:284-349`).

Os registros do oráculo são deduplicados (`crates/paridade/src/main.rs:383` `regs.dedup()`), e o
nosso lado também (`crates/paridade/src/analise.rs:640-655`, por código+intervalo+mensagem): dois
relatos iguais do analyzer contam um.

---

##### `enum_without_constants` (perda 62: FN 62, FP 0, msg 0, pos 0)
- **Emissão:** `ErrorVerifier.visitEnumDeclaration` (`src/generated/error_verifier.dart:670`), em
  `:702-708`.
- **Condição exata:**
  ```
  if (!element.isAugmentation) {             // fragmento declaração
    if (element.augmented.constants.isEmpty) // constantes da declaração + das augmentations REAIS
      report ENUM_WITHOUT_CONSTANTS at node.name
  }
  ```
  Sem o experimento `augmentations`, `augment enum E {...}` não é augmentation: é um 2º `enum E`
  (duplicado) com as **próprias** constantes (`augmented` não é cache por igualdade, ver §0.1). Cada
  enum não-augmentation sem constantes próprias (e sem augmentations reais com constantes) relata.
- **Posição:** o token do nome do enum (`node.name`), comprimento do nome.
- **Mensagem:** 3.6.2: "The enum must have at least one constant." (correção "Try declaring a
  constant."), sem argumentos. **Oráculo 3.13.4** (arquivos de `sintaxe-nova.json`): "The enum must have
  at least one enum constant." / "Try declaring an enum constant." (7 casos no corpus).
- **Supressões e ordem:** nenhuma específica; sai mesmo com erro de sintaxe no arquivo (todos os 55
  casos têm `expected_token`/`missing_const_final_var_or_type` na linha do `augment`). Sai junto de
  `duplicate_definition` no mesmo nome.
- **No DartForge:** `crates/analise/src/enums.rs` `sem_constantes`, chamado em
  `crates/paridade/src/analise.rs:329`. Classificação dos 62 FN:
  - **55** (corpus 3.6.2): arquivos com `augment enum E ...` sem o experimento (o 2º `E`, vazio, relata;
    ou o 1º vazio quando só o 2º tem constantes, ex. `EnumWithoutConstants__hasConstants_inAu_eadf4494.dart:1:6`).
    Causa: o código antigo juntava constantes por **nome** do enum. A correção já no working tree
    (`git diff crates/analise/src/enums.rs`: só `decl.augment` contribui constantes ao alvo; todo enum
    não-augment com 0 constantes próprias relata) está **de acordo com o fonte**: sem o experimento o
    parser (`declarations.rs:269`) põe `augment = false` e a 2ª declaração não entra em
    `com_constantes`. Fica correta também com o experimento ligado (augmentation real com constantes
    completa o alvo). Ressalva: `com_constantes` é por nome, então com o experimento um `augment enum E`
    para um `E` **duplicado** completaria os dois — irrelevante no corpus.
  - **3** (sintaxe nova, também `augment enum` duplicado: `AugmentationModifierExtra__enum_constru_a5e1b2c2.dart`,
    `ConstructorBody__enum_primaryConstructo_b4bc584e/c6f5a1c0.dart`): a correção acima os acha, mas a
    **mensagem** tem de ser a do 3.13 numa biblioteca de `libs_com_sintaxe_nova`
    (`analise.rs:297-327`; `Diagnostic.message` é público, dá para sobrescrever depois de
    `sem_constantes` quando `sintaxe_nova`).
  - **4** (sintaxe nova, `linguagem/primary_constructors/syntax/empty_body_error_test.dart:8,13,18,26`):
    `enum E1;`, `enum E2(final int x);`, `enum const E3;`, `enum const E4(final int x);` — corpo vazio
    `;` (construtores primários, 3.13). Posição: o nome (`E1`, col 6; com `const`, col 12). Precisa o
    parser aceitar o corpo `;` de enum e `sem_constantes` vê-lo com 0 constantes, e a mensagem 3.13.
- **Estado (2026-10-06):** a correção descrita entrou (`enums.rs`: só `augment enum` completa as constantes do
  alvo); 63 de 67. Faltam os 4 de corpo `;` (construtores primários, 3.13).
- **2026-10-07 (66/67):** o enum de corpo vazio `;` (3.13, `enum E1;`, `enum E2(final int x);`) é lido pelo parser
  sem constantes nem membros, com `EXPERIMENT_NOT_ENABLED` no `;` (`exigir_no_ast`); antes saía `EXPECTED_TOKEN`.

##### `conflicting_static_and_instance` (perda 55: FN 55, FP 0, msg 0, pos 0)
- **`static name` em enum (2026-10-08):** o `name` de um enum é a extensão `EnumName` do `dart:core` (o `Enum` só
  declara `_name`), então `enum E { a; static int name = 1; }` não conflita. O modelo sintetiza um getter `name` de
  instância em todo enum (`outline.rs`, para os back-ends); o `_checkEnumStatic` (`sobrescritas.rs`,
  `estaticos_de_enum`) o ignora.
- **Emissão (três caminhos):**
  1. Classe/mixin/tipo de extensão, membro local: `MemberDuplicateDefinitionVerifier._checkClassStatic`
     (`src/error/duplicate_definition_verifier.dart:411-461`), fase `_checkUnitStatic` (`:773`).
  2. Enum (local **e** herdado): `_checkEnumStatic` (`:647-685`), mesma fase.
  3. Classe/mixin/tipo de extensão, herdado: `ErrorVerifier._checkForConflictingClassMembers`
     (`src/generated/error_verifier.dart:2364`; chamado de `visitClassDeclaration:510`,
     `visitMixinDeclaration:1254`, `visitExtensionTypeDeclaration:824`; **não** de enum nem de alias de classe).
- **Condição exata:**
  - (1) para cada `FieldDeclaration` estático (cada variável) e `MethodDeclaration` estático (método,
    getter ou setter) do fragmento: `instanceGetters.containsKey(name) || instanceSetters.containsKey(name)`
    (os mapas do contexto do elemento, já com **todos** os membros de instância de todos os fragmentos /
    homônimos) e `declarationElement is InterfaceElement` → relata `[className, name, className]`.
  - (2) `for accessor in fragment.accessors` (inclui os getters sintéticos das **constantes** e dos campos
    estáticos, e o getter `values`) com `accessor.source == unidade atual` e `accessor.isStatic`:
    `instance = _getInterfaceMember(declaration, baseName)` (`:827`: `getMember2(element, Name(lib, n))`
    ou, se nulo, `getMember2(element, Name(lib, 'n='))` — a **interface inteira**: declarados + herdados de
    `Enum`/`Object`/mixins/interfaces); relata se `instance != null && baseName != 'values'`. Depois
    `for method in fragment.methods` estáticos: mesmo teste, **sem** a exceção de `values`. Argumentos
    `[declarationName, baseName, declarationName]`.
  - (3) `getter = getInherited2(enclosingClass, Name(lib, name))`, `setter = getInherited2(…, 'name=')`
    (`getInheritedMap2` = combinação mais específica dos candidatos dos supertipos diretos, sem o que a
    classe declara; para tipo de extensão sobre `interface.redeclared`, `inheritance_manager3.dart:215-230`).
    Métodos estáticos (`fragment.methods`, só os da unidade): `getter ?? setter` herdado → relata
    `[enclosingClass.displayName, name, inherited.enclosingElement3.displayName]` e `continue`.
    Acessores (`fragment.accessors`, inclui os sintéticos de campos): `inherited = getInherited2(name) ??
    getInherited2('name=')`; `accessor.isStatic && inherited != null` → relata e marca o nome em
    `conflictingDeclaredNames`. Mixin sem `on` tem restrição implícita `Object`
    (`_getInterfaceMixin` `inheritance_manager3.dart:930-952`), então `Object.runtimeType`/`toString`
    são herdados.
- **Posição:** (1) token do nome do campo/método; (2)(3) `atElement` → `nameOffset`/`nameLength` do
  elemento: para getter sintético de constante ou campo, o nome da constante/variável; para métodos e
  acessores explícitos, o nome. Campo estático não-final gera getter e setter → dois relatos idênticos
  (deduplicados).
- **Mensagem:** "Class '{0}' can't define static member '{1}' and have instance member '{2}.{1}' with the
  same name." — strings (sem alias). Em enum, `{0}` e `{2}` são **ambos** o nome do enum, mesmo quando o
  membro de instância vem de `Enum`/`Object`/mixin (`'E.runtimeType'`, `'E.index'`). Em classe/mixin/
  tipo de extensão com herdado, `{2}` é o **dono** do membro escolhido pelo `getInherited2` ('A.foo',
  'Object.runtimeType'); com membro local (caminho 1) `{2}` é a própria classe. Os dois caminhos podem
  relatar no mesmo token com mensagens diferentes (local e herdado).
- **Supressões e ordem:** nenhuma por erro de sintaxe. Caminho (1) não roda para enum (enum usa só o 2).
- **No DartForge:** hoje `analise/src/duplicatas.rs:735-765` (caminho 1, enum excluído, correto) e
  `analise/src/heranca.rs:15` `estatico_contra_super` (caminho 3 parcial: só classe/mixin, cadeia linear,
  ≤1 mixin, ≤1 interface, ≤1 `on`, e pula o nome que a classe também declara de instância).
  Classificação dos 55 FN: **44 enums** (24 `ConflictingStaticAndInstanceEnum__*`, 11
  `enum/enhanced_enums_error_test.dart`, 6 `executable_body/ExecutableBody__enum_static*`,
  2 `const_variable_augmentation/*enum*`, 1 `AugmentationOfDifferentDeclarationKind__dab386cb`),
  **9 tipos de extensão** (8 `ConflictingStaticAndInstanceExtensionTy_*`, 1
  `ConflictingInheritedMethodAndSetter__ex_f50c5959`), **2 mixins** (`Object.runtimeType`/`toString` via
  restrição implícita). Dos 44 enums, **9** estão em arquivos com `augment` (6 executable_body + 2
  const_variable_augmentation, nos quais a recuperação de `augment static int get foo => 0;` cria um campo
  de instância `static` do tipo `augment` e um **getter de instância** `foo`; e 1 enum duplicado `A`
  cujo `getMember2` usa a interface do 1º `A` — §0.1).

  **Código novo não validado** (working tree):
  - `crates/types/src/sobrescritas.rs:1508` `estaticos_de_enum` (ligado em `paridade/src/analise.rs:518`):
    para cada enum não-augment da biblioteca, junta constantes, campos estáticos e métodos/acessores
    estáticos; para cada um (exceto acessor `values`), procura `nome` e depois `nome_=` em
    `cx.na_interface(este, k, 0)` (declarado ou herdado de `Enum` via `superclasse()`
    `sobrescritas.rs:937-947`, mixins e interfaces, com `combineSignatures`) e relata com o nome do enum nos
    dois lugares. Fiel a `_checkEnumStatic`. Riscos: (a) os filtros "linha `augment `" e "nome repetido"
    (`:1525-1534`) deixam os 9 FN de augment/duplicado sem relato (§0.1); (b) `na_interface` devolve
    `None` quando `supertype_of`/`tipo_visto` falha (`?` em `:192,213,234`) — só gera FN, não FP;
    (c) nome privado: filtro `_` de outra biblioteca correto (`Name` com URI);
    (d) `na_interface` com candidatos de espécies diferentes devolve `None` (`:246-248`), igual ao
    `getMember2` (o `GetterMethodConflict` não entra no `map`), ok; (e) nenhuma checagem de
    `accessor.source == unidade` — irrelevante sem partes que dividam o enum.
  - `crates/types/src/sobrescritas.rs:1337` `membros_em_conflito`, agora emite `CONFLICTING_STATIC_AND_INSTANCE`
    para classe/mixin/tipo de extensão (método estático: `getter.or(setter)` herdado, `:1404-1415`;
    acessor estático: `herdado(nome).or(herdado(nome_=))`, `:1451-1458`) e, para mixin sem `on`, cai em
    `Object` quando `herdado` não acha (`:1375-1384`). Fiel ao caminho (3). Riscos:
    1. **Sobreposição com `analise::heranca`**: os dois relatam o caminho 3. Quando o dono escolhido
       difere (ex.: `class C extends A implements I` com `A.foo(int)` e `I.foo(num)`: o
       `combineSignatures` com `doTopMerge: false` escolhe o 1º *valid override*, `I.foo`; `heranca.rs`
       diz `A`), sai um FP com mensagem diferente no mesmo token. Recomendo remover o
       `CONFLICTING_STATIC_AND_INSTANCE` de `heranca.rs` depois de conferir no placar que nada se perde
       (atenção: `heranca.rs` não tem o filtro de `augment`, então hoje acerta casos em arquivos com
       `augment` que `membros_em_conflito` pula — tirar os filtros do §0.1 primeiro).
    2. O fallback para `Object` também alimenta `CONFLICTING_METHOD_AND_FIELD`/`FIELD_AND_METHOD`/
       `INHERITED_METHOD_AND_SETTER` em mixins sem `on` (ex.: `mixin M { int hashCode() => 0; }` →
       `conflicting_method_and_field` 'Object.hashCode'). É o que o analyzer faz, mas é FP novo possível
       se `herdado` já incluísse `Object` por outro caminho com outro dono.
    3. Tipo de extensão sem `implements`: `getInherited2` usa `redeclared` das interfaces declaradas
       (`_getInterfaceExtensionType`, `inheritance_manager3.dart:784-797`), **sem** `Object`. Conferir que
       `herdado` não devolve membros de `Object` para extension type sem `implements` (senão
       `static String toString()` num extension type vira FP).
    4. O filtro "linha `augment `"/nome repetido (`:1358-1364`) mantém os FN do §0.1.
- **Estado (2026-10-06):** (3) em `types::sobrescritas::membros_em_conflito` (o `getInherited2` do porte do
  `InheritanceManager3`) e (2) em `types::sobrescritas::estaticos_de_enum` (`getMember2`); o recorte antigo
  `analise::heranca` (cadeia linear, uma interface) saiu. 262 de 262; 1 FP num arquivo de construtor primário (3.13).

##### `concrete_class_with_abstract_member` (perda 26: FN 26)
- **Emissão:** `_ClassVerifier._reportConcreteClassWithAbstractMember`
  (`src/error/inheritance_override.dart:809-845`), chamado do laço de `verify()` `:288-300`.
  (`src/fasta/error_converter.dart:63-68` também converte o código do CFE, sem argumentos — não ocorre
  no corpus.)
- **Condição exata:** classe (`ClassElement`) não abstrata ou enum; para cada `name` de
  `interface.map` acessível (`name.isAccessibleFor(libraryUri)`) sem `interface.implemented[name]`:
  ```
  for member in members (AST do fragmento, em ordem):
    MethodDeclaration: memberName = lexeme (+ '=' se setter); se == name → relata
    FieldDeclaration: para cada variável: se name == v → relata; se !isFinal e name == 'v=' → relata
  ```
  **Sem filtrar `static`** nem abstrato: o 1º membro AST cujo nome bate é o relatado, mesmo estático ou
  concreto (casos `executable_body/*static*`, em que a recuperação do `augment` criou um getter de
  instância abstrato `foo`, e o relato cai no `static int get foo` anterior; e o 2º `B` homônimo de
  `UnusedElement__…83bf2fbf` relata no seu `_m` concreto). Relatou → `continue` (o nome não entra em
  `non_abstract_class_inherits_abstract_member`). Enum → código `enum_with_abstract_member`.
  A classe só é "não abstrata" se `!isAbstract`; **`sealed` com o recurso desligado é descartado pelo
  AstBuilder** (`src/fasta/ast_builder.dart:261-268`: "Pretend that 'sealed' didn't occur"), então a classe
  fica concreta.
- **Posição:** `atNode(member)`: o `MethodDeclaration`/`FieldDeclaration` inteiro, do 1º token depois do
  comentário/metadados (comentário de documentação incluído no início — `inicio_com_documentacao` do
  DartForge) até o `;`/fim do corpo. Modificador que o parser descarta não entra (`const factory();` →
  começa em `factory`, `constructor_body/…e4682f2c:3:9`).
- **Mensagem:** "'{0}' must have a method body because '{1}' isn't abstract." — `{0}` = lexeme do nome
  (sem `=`; nome sintético vazio dá `''`, `factory/redirection3_cyclic_test.dart:21:26`), `{1}` =
  `classElement.name`.
- **Supressões:** só roda se `verify()` não parou antes (§0.2).
- **No DartForge:** `crates/types/src/sobrescritas.rs:1076-1108` (`membros_abstratos`). Divergências:
  1. **22 FN** em arquivos com `augment` (filtros `:1029-1042` e nome repetido em
     `clausulas.rs:1114`): `augmentation_return_type_mismatch`(2), `const_variable_augmentation`(3),
     `constructor_body`(2), `deprecated_factory_method`(2), `duplicate_definition`(4),
     `executable_body`(8), `unused_element`(1).
  2. O `find` em `:1079-1095` exige `!af.static_`/`!vl.static_` — o analyzer não filtra (necessário para
     os `executable_body/*static*`). Retirar o filtro de `static`.
  3. `sealed_class/sealed_class_syntax_disabled_error_test.dart:17,21` (2 FN, `@dart = 2.19`): o parser
     (`frontend/src/parser/declarations.rs:964-982`) põe `modifiers.sealed = true` mesmo com
     `exigir_versao("sealed-class", 3, 0)` falhando; `membros_abstratos` (`:1021`) e
     `superclasse_concreta` (`:1058`) tratam como abstrata. Sem o recurso, `sealed` deve sumir dos
     modificadores (idem `base`/`interface`/`final`/`mixin` para `class-modifiers`, que o AstBuilder
     também descarta em seguida, `ast_builder.dart:271+`).
  4. `factory/redirection3_cyclic_test.dart:21:26` (nome sintético vazio após recuperação de
     `factory C.foo() = C.bar();`) e `abstract/factory_constructor_test.dart:27:3` (`method();` numa
     classe `A2` que também tem `A2.make() {}`): o filtro `recuperado` (`:1029-1034`, "método cujo nome
     vem depois de `.`") descarta a classe inteira por causa do construtor `A2.make` lido como método? —
     conferir; e o método de nome vazio precisa existir no modelo com nome `""`.

**Estado em 2026-10-05 (escrito, não compilado), herança e mixins.**
- `crates/types/src/fase_heranca.rs::inconsistencias`, ligada antes de `sobrescritas_invalidas`, relata os
  conflitos da interface da `Heranca`: `inconsistent_inheritance_getter_and_method` e
  `inconsistent_inheritance` (candidatos `Classe.nome (tipo)`, sem alias, um relato por texto). Relata também
  `no_combined_super_signature`, para método ou operador de instância com tipo omitido cujos sobrescritos
  (todos métodos) o `combinar` não junta. `sobrescritas_invalidas` pula esses métodos, como o `continue` do
  `verify()`.
- `crates/types/src/fase_genericos.rs::con
... [truncated for diff preview]