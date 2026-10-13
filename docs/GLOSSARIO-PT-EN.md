# Glossário português → inglês da migração do código

O código-fonte do DartForge passa para inglês (decisão de 2026-10-12; ver `AGENTS.md`).
Este glossário fixa os nomes que mais de um crate usa, para todos os agentes traduzirem
igual. Nomes internos de um crate só podem ser escolhidos por quem o traduz, seguindo o
espírito daqui. Acrescente aqui todo nome público novo (fase 3).

## Crates e pastas

| antes | depois |
|---|---|
| `crates/analise` (`dartforge-analise`) | `crates/analysis` (`dartforge-analysis`) |
| `crates/formatar` (`dartforge-formatar`) | `crates/formatter` (`dartforge-formatter`) |
| `crates/gerador_ng` (`dartforge-gerador-ng`) | `crates/ng_generator` (`dartforge-ng-generator`) |
| `crates/emit_js_producao` (`dartforge-emit-js-producao`) | `crates/emit_js_production` (`dartforge-emit-js-production`) |
| `crates/mundo` (`dartforge-mundo`) | `crates/world` (`dartforge-world`) |
| `crates/paridade` (`dartforge-paridade`) | `crates/parity` (`dartforge-parity`) |
| `crates/diferencial` (`dartforge-diferencial`) | `crates/differential` (`dartforge-differential`) |
| `crates/runtime_estatico` | `crates/runtime_static` |
| `pacotes/` | `packages/` |
| `sdk_nativo/` | `native_sdk/` |
| `bench/desempenho/` | `bench/performance/` |
| `bench/comparativo/` | `bench/comparison/` |
| `bench/resultados/` | `bench/results/` |
| `corpus/nativo/` | `corpus/native/` |
| `crates/emit_native/src/otimizar/` | `crates/emit_native/src/optimize/` |
| `crates/emit_native/src/lower/` (mantém) | — |

## Opções da CLI e valores

| antes | depois |
|---|---|
| `--memoria tracing\|arc` | `--memory tracing\|arc` |
| `--excecoes checagem\|tabelas` | `--exceptions check\|tables` |
| `--raizes sombra\|mapas` | `--roots shadow\|maps` |
| `--rastro simbolico\|nenhum` | `--stack-trace symbolic\|none` |
| `--depuracao` | `--debug` |
| `--versao-linguagem` | `--language-version` |
| `--sdk-no-grupo` | `--sdk-in-group` |

## Variáveis de ambiente

| antes | depois |
|---|---|
| `DARTFORGE_MEMORIA` | `DARTFORGE_MEMORY` |
| `DARTFORGE_EXCECOES` | `DARTFORGE_EXCEPTIONS` |
| `DARTFORGE_RAIZES` (`sombra`/`mapas`) | `DARTFORGE_ROOTS` (`shadow`/`maps`) |
| `DARTFORGE_RAIZES_CONFERIR` | `DARTFORGE_ROOTS_CHECK` |
| `DARTFORGE_RAIZES_NO_CHAMADO` | `DARTFORGE_ROOTS_IN_CALLEE` |
| `DARTFORGE_ARC_PREPARADO` | `DARTFORGE_ARC_PREPARED` |
| `DARTFORGE_ARC_CONFERIR` | `DARTFORGE_ARC_CHECK` |
| `DARTFORGE_ARC_BERCARIO` | `DARTFORGE_ARC_NURSERY` |
| `DARTFORGE_ARC_CICLOS` (`sempre`/`nunca`) | `DARTFORGE_ARC_CYCLES` (`always`/`never`) |
| `DARTFORGE_ARC_TRACO` | `DARTFORGE_ARC_TRACE` |
| `DARTFORGE_RASTRO` | `DARTFORGE_CALL_TRACE` |
| `DARTFORGE_RASTRO_VM` (`simbolico`/`nenhum`) | `DARTFORGE_STACK_TRACE` (`symbolic`/`none`) |
| `DARTFORGE_RASTRO_CRU` | `DARTFORGE_STACK_TRACE_RAW` |
| `DARTFORGE_EFEITOS` (`conferir`) | `DARTFORGE_EFFECTS` (`check`) |
| `DARTFORGE_GC_PERCURSO` (`conferir`) | `DARTFORGE_GC_WALK` (`check`) |
| `DARTFORGE_GC_VENENO` | `DARTFORGE_GC_POISON` |
| `DARTFORGE_GC_MEMORIA` | `DARTFORGE_GC_MEMORY` |
| `DARTFORGE_GC_AGENDA` | `DARTFORGE_GC_SCHEDULE` |
| `DARTFORGE_MANTER_SIMBOLOS` | `DARTFORGE_KEEP_SYMBOLS` |
| `DARTFORGE_ORCAMENTO_MAPAS` | `DARTFORGE_MAPS_BUDGET` |
| `DARTFORGE_SABOTAGEM` | `DARTFORGE_SABOTAGE` |
| `DARTFORGE_LTO_NIVEL` | `DARTFORGE_LTO_LEVEL` |
| `DARTFORGE_SEM_*` | `DARTFORGE_NO_*` |
| demais `DARTFORGE_<termo em português>` | a tradução direta pelo vocabulário abaixo |

`DARTFORGE_GC_STRESS`, `DARTFORGE_KEEP_IR`, `DARTFORGE_CLANG`, `DARTFORGE_SDK_LIB` e as
outras já em inglês ficam.

## Vocabulário

| português | inglês |
|---|---|
| acao, acoes | action, actions |
| alocar, alocacao | allocate, allocation |
| alvo | target |
| anterior | previous |
| argumento | argument |
| armazenamento | storage |
| arquivo | file |
| aresta | edge |
| arvore | tree |
| atual | current |
| bloco | block |
| caminho | path |
| campo | field |
| chamada, chamar | call |
| chamado / chamador | callee / caller |
| chave | key |
| classe | class |
| codigo | code |
| coleta, coletar, coletor | collection, collect, collector |
| conferir, conferencia | check (verify) |
| conjunto | set |
| constante | constant |
| construtor | constructor |
| contexto | context |
| contrato | contract |
| corpo | body |
| dados | data |
| declaracao | declaration |
| depuracao, depurar | debug |
| derivado | derived |
| desenrolar, desenrolamento | unwind, unwinding |
| destino | destination / target |
| diagnostico | diagnostic |
| dono | owner |
| efeito | effect |
| emprestimo, emprestado | borrow, borrowed |
| entrada | entry / input |
| erro | error |
| escopo | scope |
| espaco | space |
| estado | state |
| estatico | static |
| excecao, excecoes | exception, exceptions |
| expressao | expression |
| fim, inicio | end, start |
| fonte | source |
| funcao | function |
| gerador | generator |
| gravar, gravacao | write |
| indice | index |
| lacos, laco | loops, loop |
| laco de eventos | event loop |
| ler, leitura | read |
| lista | list |
| mapa, mapas de pilha | map, stack maps |
| membro | member |
| memoria | memory |
| mensagem | message |
| metodo | method |
| modulo | module |
| mundo (fechado) | (closed) world |
| nativo | native |
| nome | name |
| novo, nova | new |
| objeto | object |
| otimizar | optimize |
| pacote | package |
| padrao | default / pattern (conforme o caso) |
| parametro | parameter |
| pendencia, pendente | pending |
| pilha, pilha-sombra | stack, shadow stack |
| plano | plan |
| poda, podar | prune |
| ponto seguro | safepoint |
| porta | port |
| pouso | landing pad |
| preparado, preparacao | prepared, preparation |
| programa | program |
| quadro | frame |
| raiz, raizes | root, roots |
| rastro | stack trace (rastro de pilha) / call trace (`DARTFORGE_RASTRO`) |
| registro | record / registration (conforme o caso) |
| reter / soltar | retain / release |
| resultado | result |
| retorno | return |
| saida | exit / output |
| seletor | selector |
| sitio | site |
| slot | slot |
| tabela, tabelas | table, tables |
| tamanho | size |
| texto, textos | text / string |
| tipo, tipos | type, types |
| token | token |
| tracing (coletor) | tracing |
| unidade | unit |
| valor | value |
| variavel | variable |
| verificar | verify |
| vivo, vivos | live |

## Nomes públicos dos crates de base (fase 3b-1)

| antes | depois |
|---|---|
| `diagnostics::Codigo`, `InfoCodigo`, `codigos`, `codigos_g` | `Code`, `CodeInfo`, `codes`, `codes_g` |
| `Severidade`, `TipoErro` | `Severity`, `ErrorType` |
| `Referencia` (analyzer de referência, 3.6/3.13) | `AnalyzerReference` |
| `Variante313` (`de`, `para`, `exige`) | `Variant313` (`from`, `to`, `requires`) |
| `Contexto` (mensagem de contexto do diagnóstico), campo `contexto` | `ContextMessage`, campo `context` |
| `InfoCodigo { nome, unico, mensagem, correcao, tipo, severidade, documentado }` | `CodeInfo { name, unique, message, correction, error_type, severity, documented }` |
| `com_codigo`, `com_contexto`, `na_referencia` | `with_code`, `with_context`, `for_reference` |
| `por_nome`, `por_unico`, `todos` | `by_name`, `by_unique`, `all` |
| `e_erro_de_compilacao` | `is_compile_error` |
| `formatar` (molde do analyzer) | `format_list` |
| exemplo `gerar_codigos` | exemplo `generate_codes` |
| `frontend::comentarios::Comentarios` (`de`, `todos`, `antes_de`) | `comments::Comments` (`scan`, `all`, `before`) |
| `frontend::doc_referencias` (`referencias`, `DocReference { novo, partes }`) | `doc_references` (`references`, `DocReference { has_new, parts }`) |
| `frontend::pais::{Pais, Pai}` (`novo`, `pai`, `em_contexto_constante`, `todas_as_anotacoes`) | `parents::{Parents, Parent}` (`new`, `parent`, `in_constant_context`, `all_annotations`) |
| `Pai::{Fronteira, Anotacao, ConstanteDeEnum, Variavel, PadraoConstante}` | `Parent::{Boundary, Annotation, EnumConstant, Variable, ConstantPattern}` |
| `frontend::fonte` (`de_tipo`, `de_expr`, `de_padrao`, `pular_brancos`, `palavra_em`) | `source` (`from_type`, `from_expr`, `from_pattern`, `skip_whitespace`, `word_at`) |
| `LanguageVersion::{MINIMA, PISO, ATUAL}` | `LanguageVersion::{MINIMUM, FLOOR, CURRENT}` |
| `LibraryFeatures::{atual, piso, tem, versao}` | `LibraryFeatures::{current, floor, has, version}` |
| `Feature::{TODOS, DESCONHECIDOS_NO_3_6, nome, do_nome}` | `Feature::{ALL, UNKNOWN_IN_3_6, name, from_name}` |
| `features::marcador_versao`, `parser::parse_com` | `features::version_marker`, `parser::parse_with` |
| `Parsed { referencia, pulados }` | `Parsed { reference, skipped }` |
| `Ast::{metadados_locais, invocacoes_de_assert, criacoes_implicitas, raiz_de_atalho}` | `Ast::{local_metadata, assert_invocations, implicit_creations, shorthand_root}` |
| `Parameter::{declarante, nome_externo}`, `parte_primaria` | `Parameter::{declaring, external_name}`, `primary_body_part` |
| `ExprKind::NaoBinario`, `StrFlags::aberta` | `ExprKind::NotBinary`, `StrFlags::unterminated` |
| `elements::unidades::CacheUnidades` (`nova`, `invalidar`, `alterados`, `recolher`, `reaproveitadas`) | `unit_cache::AnalyzedUnitCache` (`new`, `invalidate`, `changed`, `reclaim`, `reused`) |
| `unidades::Marca` (`ler`, `tamanho`) | `unit_cache::FileStamp` (`read`, `size`) |
| `elements::gerado` (`Geracao`, `Construtor`, `chave`, `por`, `concluir`, `do_build_runner`) | `generated` (`Generation`, `GenerationBuilder`, `key`, `put`, `finish`, `from_build_runner`) |
| `GeneratedSource { conteudo, entradas, gerador }` | `GeneratedSource { content, inputs, generator }` |
| `elements::distribuicao` (`MARCA`, `raiz`, `em_lib`, `sdk_do_dart`, `sobreposicao_nativa`, `ferramenta_llvm`, `cache_do_usuario`) | `distribution` (`MARKER`, `root`, `in_lib`, `dart_sdk`, `native_overlay`, `llvm_tool`, `user_cache`) |
| `incremental::{Recusa, Anterior, substituir_unidade, restaurar_unidade}` | `incremental::{Refusal, Previous, swap_unit, restore_unit}` |
| `PackageConfig { gerados, ocultos }`, `sem_verbatim`, `gerado_no_lugar_de` | `PackageConfig { generated_sources, hidden }`, `without_verbatim`, `generated_in_place_of` |
| `load_lenient_{com_cache, entradas, entradas_ocultando, gerados}`, `load_com_sobreposicao`, `load_como_analyzer` | `load_lenient_{with_cache, entries, entries_hiding, generated}`, `load_with_overlay`, `load_as_analyzer` |
| `sdk::Linguagem` (`versao_corrente`, `experimentos`, `ler_opcao`) | `sdk::LanguageOptions` (`current_version`, `experiments`, `read_option`) |
| `model::TemposCarga`, `Program::tempos` | `model::LoadTimes`, `Program::times` |
| `lookup_na_unidade`, `lookup_prefixed_na_unidade`, `prefixos_na_unidade` | `lookup_in_unit`, `lookup_prefixed_in_unit`, `prefixes_in_unit` |
| `dono_da_classe`, `dono_do_grupo`, `representante(_da_classe)`, `publico`, `declaracao_publica` | `class_owner`, `group_owner`, `representative`/`class_representative`, `public`, `public_declaration` |
| `Library { pais, escopos_de_unidade }`, `instancia_sobrepostos`, `augmentacoes` | `Library { unit_parents, unit_scopes }`, `shadowed_instance_members`, `augmentations` |
| `types::fase_*` (`acesso`, `deprecado`, `heranca`, `genericos`, `requeridos`, `resultado`, `nao_guardar`, `nao_usados`, `estrita`…) | `types::phase_*` (`access`, `deprecated`, `inheritance`, `generics`, `required`, `result`, `do_not_store`, `unused`, `strict`…) |
| `types::lints_tipados{,2..9}`, `achados` | `types::typed_lints{,2..9}`, `findings` |
| `types::constantes` (`valor`, `avaliador`; `Motor`, `Constante`, `Valor`, `Estado`) | `types::const_eval` (`value`, `evaluation`; `Engine`, `Constant`, `Value`, `InstanceState`) |
| `types::heranca::{Heranca, Membro, Nome, ProvedorDoOutline}` | `types::inheritance::{InheritanceManager, InterfaceMember, MemberName, OutlineProvider}` |
| `types::exibicao::Exibidor`, `table::Exibicao`, `TypeTable::exibicao` | `types::display::Displayer`, `table::DisplayDecoration`, `TypeTable::decoration` |
| `types::{despejo, limites, sobrescritas, anotacoes, auto_referencia, boas_praticas, parametros, tipos_de_extensao, variancia}` | `types::{dump, bound_checks, override_checks, annotations, self_reference, best_practices, parameters, extension_types, variance}` |
| `EscopoSondado`, `infer_all_com_sonda`, `sonda_escopo` | `ProbedScope`, `infer_all_with_probe`, `scope_probe` |
| `UnitBodyTypes::{tipos_escritos, tipos_de_anotacoes, tipos_casados, tipo_local, declaracao_local, …}` | `UnitBodyTypes::{written_types, annotation_types, matched_types, local_type, local_declaration, …}` |
| `infer_bodies_das_bibliotecas(_com_unidades)`, `resolve_outline_com_unidades`, `inferir_corpos`, `corpos_da_unidade`, `corpo_no_offset` | `infer_library_bodies(_with_units)`, `resolve_outline_with_units`, `infer_bodies`, `unit_bodies`, `body_at_offset` |
| `e_<x>` (predicados “é x”) | `is_<x>` |
| exemplos `despejo_tipos`, `comparar_tipos`, `gravar_corpus`, `avisos`, `memoria_types`; testes `corpus_inferencia`, `inferencia_grupos` | `dump_types`, `compare_types`, `record_corpus`, `warnings`, `memory_types`; `corpus_inference`, `inference_groups` |
| `analysis::arvore_analyzer::{Arvore, No, Marca, Ligacao, PartesDeFor}` (`nos`, `inicio`, `fim`, `pai`, `filhos`, `especie`) | `analysis::analyzer_tree::{AnalyzerTree, Node, Mark, Binding, ForParts}` (`nodes`, `start`, `end`, `parent`, `children`, `kind`) |
| `arvore_analyzer::{construir, construir_resolvida, localizar, e_comando, e_expressao}` | `analyzer_tree::{build, build_resolved, locate, is_statement, is_expression}` |
| `analysis::{publicacao, duplicatas, externos, importacoes, locais, saida, inicializacao, operadores, naodart}` | `analysis::{publication, duplicates, externals, imports, locals, exit_detector, initialization, operators, non_dart}` |
| `analysis::Unidade { fonte }`, `Pulados { trechos }` | `analysis::Unit { source }`, `Skipped { spans }` |
| `publicacao::{VERIFICADOS, verificados, publicado}` | `publication::{VERIFIED, verified, is_published}` (o arquivo `verificados.txt` fica) |
| `lints::{CodigoLint, InfoRegra, EstadoDaRegra, Conjunto, REGRAS, TODOS}`, `codigos_g`, `tabela_g` | `lints::{LintCode, RuleInfo, RuleState, RuleSet, RULES, ALL}`, `codes_g`, `table_g` |
| `naodart::{CodigoNaoDart, NOMES_DE_CODIGO}`, `yaml::{No, Valor, ler}` | `non_dart::{NonDartCode, CODE_NAMES}`, `yaml::{Node, Value, parse}` |
| `parity::analise::{Motor, Analise, Arquivo}` (`descobrir`, `analisar`, `analisar_com`) | `parity::analysis::{Engine, AnalysisResult, FileAnalysis}` (`discover`, `analyze`, `analyze_with`) |
| `parity::filtros::{Opcoes, Ignorados}` (`ler`, `ler_arquivo`, `de_texto`, `de_subpasta`, `ignoravel`, `ignora`, `excluido`) | `parity::filters::{AnalysisOptions, IgnoreComments}` (`read`, `read_file`, `from_text`, `from_subfolder`, `is_ignorable`, `ignores`, `is_excluded`) |
| `parity::json::{Faixa, Ponto, Linhas}` (`ponto`, `ordenar`, `escrever`) | `parity::json::{JsonRange, JsonPosition, LineIndex}` (`position`, `sort`, `write`) |
| `parity::{oraculo, projetos}`, `diagnosticos_json`, `diagnosticos_do_pubspec`… | `parity::{oracle, projects}`, `json_diagnostics`, `pubspec_diagnostics`… (as chaves dos JSON gravados — `arquivo`, `nome`, `caminho`, `pacotes`… — ficam, via `#[serde(rename)]`) |
| exemplo `sonda_arquivos`; testes `sonda`, `regras_do_analyzer` | `probe_files`; `probe`, `analyzer_rules` |
| `formatter::{formatar, Opcoes { largura, recuo, fim_de_linha }}` | `formatter::{format, FormatOptions { page_width, indent, line_ending }}` |
| `formatter::{ErroFormatacao::{Sintaxe, SaidaInesperada}, ErroSintaxe { posicao, comprimento, mensagem }}` | `formatter::{FormatError::{Syntax, UnexpectedOutput}, SyntaxError { offset, length, message }}` |
| `formatter::edicoes::{Edicao, edicoes_minimas}` | `formatter::edits::{Edit, minimal_edits}` |

## Nomes públicos dos crates JS e de ferramentas (fase 3b-2)

| antes | depois |
|---|---|
| `world::{Mundo, Raizes, Entrada, calcular}` | `world::{World, Roots, WorldInput, compute}` |
| `Mundo::{funcao, variavel, estat}` | `World::{is_live_function, is_live_variable, stats}` |
| `Raizes { funcoes, classes_instanciadas, seletores }` | `Roots { functions, instantiated_classes, selectors }` |
| `Statistics { classes_instanciadas, funcoes_usuario, funcoes_vivas, seletores }` | `Statistics { instantiated_classes, user_functions, live_functions, selectors }` |
| `dfexec::{Canal, CanalDeProcesso, PROTOCOLO, escrever_quadro, ler_quadro}` | `dfexec::{Channel, ProcessChannel, PROTOCOL, write_frame, read_frame}` (o valor `dfexec/1` fica) |
| `Canal::{enviar, receber}`, `CanalDeProcesso::{iniciar, com_prazo}` | `Channel::{send, receive}`, `ProcessChannel::{start, with_deadline}` |
| `sass::VersaoDartSass` (`do_lock`) | `sass::DartSassVersion` (`from_lock`) |
| `sass_builder::Leitor` (`existe`, `ler`), `Compiled { lidos, sondados }` | `sass_builder::AssetReader` (`exists`, `read`), `Compiled { read_files, probed_files }` |
| teste `formas_166` (a pasta de dados `tests/formas_166/` fica) | `forms_166` |
| `ng_generator::resolucao::Resolvedor` (`novo`) | `ng_generator::resolution::Resolver` (`new`) |
| `ng_generator::Indice` (`novo`, `do_programa`, `atualizar`, `remover`) | `ng_generator::NgIndex` (`new`, `from_program`, `update`, `remove`) |
| `ng_generator::Pacote { nome, raiz, folhas_geradas }` | `ng_generator::Package { name, root, generated_sheets }` |
| `Scoreboard { gerados, pendentes, conjuntos }` | `Scoreboard { generated, pending, refusal_sets }` |
| `caminho_do_template`, `gerar_folha`, `gerar_com_apoio`, `analisar_arquivo`, `gerar_arquivo` | `template_path`, `generate_sheet`, `generate_with_fallback`, `analyze_file`, `generate_file` |
| `FileOutput { entradas }`, `metadata::ler` | `FileOutput { inputs }`, `metadata::read_directive` |
| `ng_generator::sass::{Estilo::{Expandido, Comprimido}, compilar_ativo}`, `Sheet { mapa }` | `sass::{Style::{Expanded, Compressed}, compile_asset}`, `Sheet { map }` |
| `expr::Action::Complexa` | `expr::Action::Complex` |
| teste `sass_formas` (a pasta de dados `tests/sass_formas/` fica) | `sass_forms` |
| teste `ng_edicao_dart_em_cada_arquivo_igual_ao_do_zero` (`--skip ng_edicao_dart_em_cada_arquivo` no `ci.yml`) | `ng_dart_edit_in_each_file_equals_from_scratch` (`--skip ng_dart_edit_in_each_file`) |
| motivos `pós-processador…` e aviso `estado do motor não gravado` do motor de build | `post-processor…` e `engine state not saved` |
| `dartforge-dev`: testes `custo_zero`, `plato`; exemplos `medir`, `medir_geracao` | `zero_cost`, `plateau`; `measure`, `measure_generation` (a linha `média … ms/edição` que o `custo-zero.ps1` lê fica) |
| `lsp::especies_g::ESPECIES` (gerado por `scripts/gerar-especies-de-acao.py`) | `lsp::action_kinds_g::KINDS` |
| método de teste `dartforge/dormir`, sentinela `dartforge__completar`, arquivo sintético `.dartforge_conhecidos.dart` | `dartforge/sleep`, `dartforge__complete`, `.dartforge_known.dart` |

## Crates nativos e ABI do runtime (fase 3b-3)

Os símbolos da ABI mudaram de nome sem aliases: o cache do SDK compilado é chaveado pelo
hash das fontes do emissor e do runtime (e dos catálogos), então objetos antigos não são
reaproveitados. As versões (`_v1`) ficam.

| antes | depois |
|---|---|
| `crates/runtime/efeitos.tsv` (`nome coleta lanca roda_dart`) | `crates/runtime/effects.tsv` (`name collects throws runs_dart`); as marcas não mudaram |
| `ownership.tsv`: `vetor:native`, `tabela:native` | `vector:native`, `table:native` |
| `dartforge_runtime::{simbolos::{NOMES, FRAGMENTOS, tabela}, efeitos::EFEITOS}` | `dartforge_runtime::{symbols::{NAMES, FRAGMENTS, table}, effects::EFFECTS}` |
| `ownership::{Contrato, CONTRATOS, ModoParametro, ModoResultado, contrato}` (`nome`, `parametros`, `resultado`, `retencao_persistente`, `invalida_borrows`, `pode_falhar`, `chama_dart`) | `ownership::{Contract, CONTRACTS, ParamMode, ResultMode, contract}` (`name`, `parameters`, `result`, `persistent_retention`, `invalidates_borrows`, `may_fail`, `calls_dart`) |
| `dartforge_nativo_<Nome>` | `dartforge_native_<Name>` (os nomes da VM, como `Double_add`, ficam) |
| `DartForge_<x>` (natives da sobreposição `native_sdk/`) | tradução direta: `DartForge_porta_enviar` → `DartForge_port_send`, `DartForge_ffi_gravar_*` → `DartForge_ffi_store_*`, `DartForge_ffi_carregar_*` → `DartForge_ffi_load_*` |
| sobreposição `dartforge_nativo` (`native_sdk/libraries.json`) | `dartforge_native` |
| `dartforge_arc_quadro_*`, `dartforge_arc_lista_*`, `dartforge_arc_gravar_campo_*`, `dartforge_arc_ler_campo_*` | `dartforge_arc_frame_*`, `dartforge_arc_list_*`, `dartforge_arc_write_field_*`, `dartforge_arc_read_field_*` |
| `dartforge_arc_escopo_nativo_{abrir,fechar}_v1` | `dartforge_arc_native_scope_{open,close}_v1` |
| `dartforge_texto_*`, `df.texto_*` | `dartforge_string_*`, `df.string_*` (`dartforge_texto_novo` → `dartforge_string_allocate`) |
| `dartforge_record_novo` | `dartforge_record_from_refs` |
| `dartforge_rastro_{entrar,sair,texto}` / `dartforge_rastro_{entrada,saida}` | `dartforge_stack_trace_{enter,exit,string}` / `dartforge_call_trace_{entry,exit}` |
| `dartforge_personalidade[_cleanup_itanium]`, `dartforge_objeto_de_desenrolamento` | `dartforge_personality[_cleanup_itanium]`, `dartforge_unwind_object` |
| `dartforge_contexto`, `dartforge_ponto_seguro`, `dartforge_laco_de_eventos` | `dartforge_context`, `dartforge_safepoint`, `dartforge_event_loop` |
| `df.manter_vivo`, `df.contexto`, `df.tipada_*`, `df.nucleo_*`, `df.caixa_*`, `df.desencaixa_*` | `df.keep_alive`, `df.context`, `df.typed_*`, `df.core_*`, `df.box_*`, `df.unbox_*` |
| `df.classe`, `df.corpo`, `df.barreira[_elemento]`, `df.filho_jovem`, `df.e_objeto`, `df.nova_instancia` | `df.class`, `df.body`, `df.barrier`/`df.element_barrier`, `df.young_child`, `df.is_object`, `df.new_instance` |
| `df.registrar.*`, `df.$registro.*`, `df.$tipo.*`, `; df.classe`, `; df.campos` | `df.register.*`, `df.$record.*`, `df.$type.*`, `; df.class`, `; df.fields` |
| sufixos `$novo`, `$primario`, `$quadro`, `$indice`; parâmetros `$tipos`, `$tipos_de_fora` | `$new`, `$primary`, `$frame`, `$index`; `$types`, `$outer_types` |
| `runtime/src/{textos, tipadas, tabela_rti, ffi_api_nativa, io_observador, io_windows_eventos}.rs` | `{text, typed_lists, rti_table, ffi_native_api, io_watcher, io_windows_events}.rs` |
| `textos::{Texto, TextoRef::{Um, Dois}}`, `layout::{desl, estado}` | `text::{Text, TextRef::{OneByte, TwoByte}}`, `layout::{offsets, state}` |
| `layout::CAMPOS_EM_LINHA`, `DESLOCAMENTO_DO_HANDLE`, `flags::{FORA, FORMA, BRUTO, INSTANCIA, CARTOES}` | `INLINE_FIELDS`, `HANDLE_OFFSET`, `flags::{OUT_OF_LINE, FORMAT, RAW, INSTANCE, CARDS}` |
| `heap::EstadoDoArc`, `Heap::coletar` | `ArcHeapState`, `Heap::run_collection` |
| `emit_native/src/{alvo, gerador, ligador, ligador_macos, ligador_windows, resumo, sdk_modulo}.rs` | `{target, generator, linker, linker_macos, linker_windows, summary, sdk_module}.rs` |
| `alvo::{Sistema, sistema}`, `gerador::Cpu::do_nome`, `resumo::ResumoIr::de` | `target::{Os, target_os}`, `generator::Cpu::from_name`, `summary::IrSummary::of` |
| `compilar`, `emitir_ir`, `construtos_do_erro`, `objeto_do_cache`, `sdk_do_dart` | `compile`, `emit_ir`, `error_constructs`, `cache_object`, `dart_sdk` |
| `CompileOptions { versao_linguagem, experimentos, depuracao, gerador }` | `CompileOptions { language_version, experiments, debug, generator }` |
| mensagens `erro de compilação`, `não suportado no backend nativo: …`, `bug do compilador` | `compile error`, `not supported by the native backend: …`, `compiler bug` (o `differential` casa os novos textos) |
| guarda de layout ARC: `layout escalar ARC incompatível`, `classe/tamanho fora dos layouts Ref` | `incompatible ARC scalar layout`, `class/size outside the Ref layouts` (lidas por `scripts/provar-arc-aot.py`) |
| `cli/src/empacotar.rs` (o subcomando `empacotar` fica) | `cli/src/pack.rs` |
| testes da CLI `memoria_cli`, `reload_estado`, `jit_programa`, `io_regressao`, `mapas_dirigidos`, `depuracao` | `memory_cli`, `reload_state`, `jit_program`, `io_regression`, `directed_maps`, `debugging` |
| testes do emit_native `contrato`, `depuracao`; binário `diferencial` | `contract`, `debugging`; `differential` |
| exemplos `arc_alocacao_campos`, `arc_campos_locais`, `arc_cleanup_laco`, `arc_instancia_zerada`, `arc_keepalive_campos`, `arc_porta_nativa_owned`, `arc_retorno_da_fonte`, `arc_retornos_guardados`, `arc_unwind_estrangeiro`… | `arc_field_allocation`, `arc_local_fields`, `arc_loop_cleanup`, `arc_zeroed_instance`, `arc_field_keepalive`, `arc_native_port_owned`, `arc_source_return`, `arc_guarded_returns`, `arc_foreign_unwind`… (os diretórios `target/prova-*` do `provar-arc-aot.py` ficam) |
