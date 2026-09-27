# Matriz de suporte por programa, perfil e sistema

Gerada por `scripts/matriz.sh` (que roda o `dartforge-diferencial --matriz`
em cada corpus e perfil) e `scripts/matriz.py`; não edite à mão. Data da
geração: 2026-09-27.

Cada par (programa, perfil) tem **um** estado, contra a referência do
programa (a VM do SDK que o programa pede; o `dartdevc` quando o programa
só existe na web):

* **suportado** (✓) — compilou e o stdout e o código de saída são os da referência;
* **recusado** (R) — o perfil recusou na compilação, com diagnóstico: igual à
  referência ("como a referência", ex.: biblioteca que não existe na
  plataforma) ou não (o recurso falta no perfil);
* **divergente** (D) — executou e a saída difere (inclusive os listados em
  `PENDENTES`);
* **não medido** (·) — o perfil não rodou o programa neste sistema, ou a
  referência não pôde ser obtida. Nada é afirmado sobre ele.

Os percentuais de um corpus medem **aquele corpus**, não a linguagem: um
programa exercita um conjunto de operações, e operação que nenhum programa
exercita não aparece aqui. As APIs públicas do nativo levantadas fora do
corpus estão na última seção.

## Resumo

| sistema | corpus | perfil | programas | suportado | recusado (como a ref.) | recusado (falta) | divergente | não medido |
|---|---|---|---:|---:|---:|---:|---:|---:|
| linux-x86_64 | js | js-dev | 234 | 234 | 0 | 0 | 0 | 0 |
| linux-x86_64 | js | js-prod | 234 | 234 | 0 | 0 | 0 | 0 |
| linux-x86_64 | js | aot | 234 | 231 | 3 | 0 | 0 | 0 |
| linux-x86_64 | js | aot-gc-stress | 234 | 231 | 3 | 0 | 0 | 0 |
| linux-x86_64 | js | jit | 234 | 231 | 3 | 0 | 0 | 0 |
| linux-x86_64 | moderno | js-dev | 26 | 26 | 0 | 0 | 0 | 0 |
| linux-x86_64 | moderno | js-prod | 26 | 26 | 0 | 0 | 0 | 0 |
| linux-x86_64 | nativo | aot | 36 | 36 | 0 | 0 | 0 | 0 |
| linux-x86_64 | nativo | aot-gc-stress | 36 | 36 | 0 | 0 | 0 | 0 |
| linux-x86_64 | nativo | jit | 36 | 36 | 0 | 0 | 0 | 0 |

Sistemas sem TSV aqui não foram medidos por esta matriz (o CI deles tem o
placar próprio, que não separa os estados).

## Recusados por falta e divergentes

Nenhum: todo programa medido é suportado ou recusado como a referência.

## Por programa

✓ suportado · R recusado · D divergente · `·` não medido.

### linux-x86_64 — corpus/js

| programa | js-dev | js-prod | aot | aot-gc-stress | jit |
|---|:---:|:---:|:---:|:---:|:---:|
| `01_print` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `02_strings_interpolacao` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `03_strings_escapes` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `04_strings_surrogates` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `05_strings_multilinha_raw` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `06_strings_metodos` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `07_string_buffer` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `08_strings_comparacao_igualdade` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `09_bool_null_literais` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `10_int_basico` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `11_int_truncdiv_modulo_negativos` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `12_int_bits_32` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `12b_int_bits_divergencia_web` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `13_double_basico` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `13b_double_tostring_divergencia_web` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `14_int_grandes_2p53` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `15_num_parse` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `16_num_metodos_conversao` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `17_operadores_compostos_precedencia` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `18_math` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `19_num_literais_formatos` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `20_if_else_encadeado` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `21_while_do_while` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `22_for_classico_forin` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `23_break_continue_rotulos` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `24_switch_classico` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `25_condicional_ternario` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `26_assert` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `27_operadores_logicos_curto_circuito` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `28_ordem_avaliacao` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `29_fluxo_return_em_laco_e_switch` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `30_funcoes_parametros` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `31_closures_captura_em_laco` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `32_tearoffs` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `33_funcoes_genericas` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `34_funcoes_locais_recursao` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `35_funcao_como_valor_typedef` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `36_funcoes_arrow_callable_class` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `37_closure_estado_contador` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `38_funcoes_parametros_funcao_e_nulos` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `39_funcoes_recursivas_e_iteradores` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `40_classes_basico` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `41_classes_ctor_nomeado` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `42_classes_ctor_factory_redirect` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `43_classes_ctor_const_canonico` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `44_classes_campos_inicializadores` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `45_classes_estaticos` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `46_classes_getters_setters` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `47_classes_operadores` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `48_classes_equals_hashcode` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `49_classes_heranca_super` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `50_classes_abstratas_interfaces` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `51_mixins` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `52_enums_basico` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `53_enums_membros` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `54_genericas_bounds` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `55_is_as_runtimetype` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `56_late` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `57_nosuchmethod` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `58_tostring_override_e_interpolacao` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `59_class_modifiers_e_privados` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `60_list_basico` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `61_list_metodos` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `62_iterable_map_where_fold` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `63_iterable_expand_reduce_lazy` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `64_map_ordem_insercao` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `65_set_operacoes` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `66_colecoes_literais_spread_if_for` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `67_colecoes_tipadas_generics` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `68_iterator_manual_e_iterable_custom` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `69_list_de_lists_e_matriz` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `70_try_catch_basico` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `71_try_on_catch_finally` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `72_rethrow` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `73_excecoes_customizadas` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `74_finally_return_break` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `75_stacktrace_existencia` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `76_erros_do_core_tipos` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `77_throw_em_expressoes` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `78_excecoes_em_closures_e_lacos` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `79_error_objects_toString_proprio` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `80_async_await_basico` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `81_microtarefas_ordem` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `82_future_delayed_ordem` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `83_completer` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `84_stream_listen` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `85_await_for` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `86_async_star` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `87_sync_star` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `88_future_wait_then_catcherror` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `89_async_excecoes` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `89b_async_em_metodos_e_closures` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `90_dynamic_chamadas` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `91_dynamic_nosuchmethod_erros` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `92_cascatas` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `93_null_aware` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `94_null_safety_promocao` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `95_dynamic_operadores_e_conversoes` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `100_records_basico` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `101_records_desestruturacao` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `102_switch_padroes` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `103_if_case` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `104_sealed_exaustivo` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `105_padroes_lista_map` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `106_padroes_objeto_guardas` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `107_switch_expressao` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `108_records_igualdade_hash` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `109_padroes_em_declaracoes_e_foreach` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `110_extension_methods` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `111_extension_genericas_e_estaticas` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `112_typedef` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `113_imports_prefixo` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `114_export_show_hide` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `115_part` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `116_bibliotecas_privados` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `117_biblioteca_ciclo` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `118_top_level_lazy_init` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `119_library_com_nome_e_dart_async_import` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `119b_import_deferred` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `120_convert_json` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `121_convert_utf8_base64` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `122_collection_queue` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `123_collection_linkedhashmap_splaytreemap` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `124_typed_data` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `125_datetime_utc` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `126_duration` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `127_regexp` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `128_uri` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `129_comparable_sort` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `130_string_split_trim_pad_avancado` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `131_string_codeunits_runes` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `132_int_parse_double_parse_erros` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `133_iterable_num_sum_media` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `134_object_identical_equals` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `135_convert_json_classes` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `136_string_interpolacao_avancada` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `137_math_random_seed` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `138_collection_hashmap_ordenado` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `139_string_tostring_de_tudo` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `140_antigo_annotations` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `141_antigo_arithmetic` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `142_antigo_boolean` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `143_antigo_class_effects` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `144_antigo_classes` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `145_antigo_colecoes` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `146_antigo_colecoes_bits` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `147_antigo_collections_closures` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `148_antigo_constants` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `149_antigo_enhanced_enums_switch` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `150_antigo_evaluation_order` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `151_antigo_expression_bodies` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `152_antigo_extension_dispatch` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `153_antigo_extensions` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `154_antigo_for_scope` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `155_antigo_functions` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `156_antigo_generics_constants` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `157_antigo_interfaces_enums` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `158_antigo_iterable_format` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `159_antigo_loop_evaluation` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `160_antigo_loops` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `161_antigo_merge_bindings` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `162_antigo_null_flow` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `163_antigo_null_safety` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `164_antigo_numeric_edges` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `165_antigo_parameter_shadow` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `166_antigo_recursion` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `167_antigo_scopes` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `168_antigo_sealed_patterns` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `169_antigo_string_escapes` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `170_antigo_strings` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `171_antigo_nativo_calls` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `172_antigo_nativo_coalesce_promotion` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `173_antigo_nativo_control_flow` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `174_antigo_nativo_edge_cfg` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `175_antigo_nativo_gc_root_slots` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `176_antigo_nativo_managed_lifetimes` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `177_antigo_nativo_managed_objects` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `178_antigo_nativo_mixins` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `179_antigo_nativo_null_flow_loops` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `180_antigo_nativo_root_slots` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `181_antigo_async23` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `182_antigo_cascades20` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `183_antigo_classes_adv` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `184_antigo_closures` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `185_antigo_colecoesops` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `186_antigo_constructors17` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `187_antigo_cycle` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `188_antigo_export_cycle` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `189_antigo_features13` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `190_antigo_fluxo26` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `191_antigo_interfaces` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `192_antigo_maps21` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `194_antigo_modifiers14` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `195_antigo_namespaces` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `196_antigo_package_exports` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `197_antigo_private_inheritance` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `198_antigo_records19` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `199_antigo_reified18` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `200_antigo_strings25` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `201_antigo_nativo_managed` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `202_antigo_nativo_packages` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `203_antigo_unmodifiable_async` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `204_super_equals` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `205_super_params_tipo` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `206_enum_factory_e_classe_so_factory` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `207_factory_redirecionada_tearoff` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `208_literais_de_tipo_especiais` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `209_js_interop` | ✓ | ✓ | R | R | R |
| `210_constantes_de_ambiente` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `211_membros_nativos_renomeados` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `212_rti_de_superclasse_generica` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `213_interop_de_tipos_de_extensao` | ✓ | ✓ | R | R | R |
| `214_tipo_cru_generico` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `215_membros_reservados_js` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `215_poda_dinamico_por_nome` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `216_poda_nosuchmethod` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `217_poda_tearoffs` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `218_poda_protocolos_do_sdk` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `219_poda_tipos_so_em_is` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `220_poda_estaticos_preguicosos` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `221_poda_mixins_super` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `222_poda_interop` | ✓ | ✓ | R | R | R |
| `223_nosuchmethod_argumentos` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `224_poda_por_tipo_do_receptor` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `225_extensoes_genericas_reificadas` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `226_acessores_de_topo_prefixados` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `227_campos_por_interface` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `228_super_setter_e_tearoff_de_extensao` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `229_nome_indefinido_e_local_duplicado_erro` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `230_tipos_de_extensao_membros` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `231_tipos_de_extensao_escrita_composta` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `232_tipos_de_extensao_implements` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `233_geradores_tipo_inferido` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `234_this_interpolado_e_coalescencia_dinamica` | ✓ | ✓ | ✓ | ✓ | ✓ |

### linux-x86_64 — corpus/moderno

| programa | js-dev | js-prod |
|---|:---:|:---:|
| `300_curingas_locais` | ✓ | ✓ |
| `301_curingas_membros` | ✓ | ✓ |
| `302_curingas_versao_36` | ✓ | ✓ |
| `303_curingas_ler_erro` | ✓ | ✓ |
| `310_null_aware_elementos` | ✓ | ✓ |
| `311_null_aware_versao_37` | ✓ | ✓ |
| `320_nomeados_privados` | ✓ | ✓ |
| `321_nomeados_privados_versao_311` | ✓ | ✓ |
| `322_nomeados_privados_super_erro` | ✓ | ✓ |
| `323_nomeados_privados_sem_publico` | ✓ | ✓ |
| `330_atalhos_ponto` | ✓ | ✓ |
| `331_atalhos_versao_39` | ✓ | ✓ |
| `332_atalhos_sem_contexto` | ✓ | ✓ |
| `340_construtores_primarios` | ✓ | ✓ |
| `341_primarios_escopo` | ✓ | ✓ |
| `342_new_factory` | ✓ | ✓ |
| `343_factory_metodo_312` | ✓ | ✓ |
| `344_final_parametro_erro` | ✓ | ✓ |
| `345_primarios_versao_312` | ✓ | ✓ |
| `346_primarios_tipos_omitidos` | ✓ | ✓ |
| `347_primarios_extension_type` | ✓ | ✓ |
| `348_primario_generativo_erro` | ✓ | ✓ |
| `349_parte_this_sem_primario_erro` | ✓ | ✓ |
| `350_inferencia_bounds` | ✓ | ✓ |
| `351_fluxo_solido` | ✓ | ✓ |
| `352_fluxo_solido_versao_38` | ✓ | ✓ |

### linux-x86_64 — corpus/nativo

| programa | aot | aot-gc-stress | jit |
|---|:---:|:---:|:---:|
| `01_io_sincrono` | ✓ | ✓ | ✓ |
| `02_io_assincrono` | ✓ | ✓ | ✓ |
| `03_processos` | ✓ | ✓ | ✓ |
| `04_soquetes_http` | ✓ | ✓ | ✓ |
| `05_udp_sinais` | ✓ | ✓ | ✓ |
| `06_escuta_compartilhada` | ✓ | ✓ | ✓ |
| `07_isolados` | ✓ | ✓ | ✓ |
| `08_ffi` | ✓ | ✓ | ✓ |
| `09_simd` | ✓ | ✓ | ✓ |
| `10_colecoes_e_referencias_fracas` | ✓ | ✓ | ✓ |
| `11_zlib` | ✓ | ✓ | ✓ |
| `12_ffi_structs` | ✓ | ✓ | ✓ |
| `13_ffi_callbacks` | ✓ | ✓ | ✓ |
| `14_finalizadores` | ✓ | ✓ | ✓ |
| `15_ffi_api_nativa` | ✓ | ✓ | ✓ |
| `16_ffi_structs_por_valor` | ✓ | ✓ | ✓ |
| `17_ffi_varargs` | ✓ | ✓ | ✓ |
| `18_ffi_handle` | ✓ | ✓ | ✓ |
| `19_ffi_native_variaveis` | ✓ | ✓ | ✓ |
| `20_listas_tipadas_acesso_direto` | ✓ | ✓ | ✓ |
| `21_listas_do_nucleo_acesso_direto` | ✓ | ✓ | ✓ |
| `22_listas_validade_do_buffer` | ✓ | ✓ | ✓ |
| `23_resto_por_constante` | ✓ | ✓ | ✓ |
| `24_simd_sem_caixa` | ✓ | ✓ | ✓ |
| `25_int_double_em_linha` | ✓ | ✓ | ✓ |
| `26_for_in_de_lista` | ✓ | ✓ | ✓ |
| `27_listas_covariancia` | ✓ | ✓ | ✓ |
| `28_closures_tipadas` | ✓ | ✓ | ✓ |
| `29_divisao_por_zero` | ✓ | ✓ | ✓ |
| `30_listas_cabecalho_add_filled` | ✓ | ✓ | ✓ |
| `31_udp_multicast` | ✓ | ✓ | ✓ |
| `32_soquete_sincrono` | ✓ | ✓ | ✓ |
| `33_mensagens_de_controle` | ✓ | ✓ | ✓ |
| `34_observar_arquivos` | ✓ | ✓ | ✓ |
| `35_interromper_laco` | ✓ | ✓ | ✓ |
| `36_tipos_de_extensao` | ✓ | ✓ | ✓ |

## APIs públicas do levantamento de natives (backend nativo)

As operações que ficaram fora do nativo até a auditoria de 2026-09-27, com o
estado declarado em `docs/NATIVOS-PENDENTES.md` §1 (o detalhe e a evidência
de cada uma estão lá; "ausente" é `UnsupportedError("não suportado no
backend nativo: …")` em execução, no AOT e no JIT):

| API | estado no nativo |
|---|---|
| `dart:io` — observação de sistema de arquivos | implementado no Linux |
| `dart:io` — multicast de UDP | implementado |
| `dart:io` — mensagens de controle e `ResourceHandle` (Unix domain sockets) | implementado |
| `dart:io` — `RawSynchronousSocket` | implementado |
| `dart:developer` — `NativeRuntime.writeHeapSnapshotToFile` | ausente |
