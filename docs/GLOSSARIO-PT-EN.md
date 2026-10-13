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

## Interface com usuário e CI (fase 3c)

Opções, subcomandos, variáveis de ambiente, features, nomes de scripts e de
workflows, linhas de relatório lidas pelo CI, protocolos e formatos em disco
passaram para inglês **sem aliases**: os nomes antigos deixam de funcionar.
Esta seção substitui as notas "fica" das seções anteriores sobre
`verificados.txt`, `dartforge empacotar`, os diretórios `target/prova-*` e a
linha `média … ms/edição`. Ficam para a fase 4: as APIs públicas dos pacotes
Dart (`packages/`, `native_sdk/`) e o esquema do modelo, do resultado e da
codificação de argumentos das macros (`macro.*` só traduziu o envelope), os
nomes de programas e pastas do corpus, o conteúdo dos fixtures, os nomes
internos (variáveis, comentários, mensagens) dos scripts e os nomes das pastas
e arquivos de `tools/` (as opções de `tools/tamanho/por-que.py` e `quebra.py`
já são `--all --summary --map` e `--section --groups --symbols`).

### Features do Cargo

| antes | depois |
|---|---|
| `nativo` (dartforge-cli) | `native` |
| `nativos` (dartforge-build) | `native-generators` |
| `llvm-embutido` (emit-native, differential) | `embedded-llvm` |

### Subcomandos, binários e valores de opção

| antes | depois |
|---|---|
| `dartforge empacotar` | `dartforge pack` |
| `dartforge macros --forma 3.6\|atual` | `--shape 3.6\|current` |
| `--excecoes checagem\|tabelas` | `--exceptions check\|tables` |
| `--raizes sombra\|mapas` | `--roots shadow\|maps` |
| `--rastro simbolico\|nenhum` | `--stack-trace symbolic\|none` |
| `compile-native --resumo` | `--digest` |
| `dartforge-differential contrato\|verificar\|determinismo` | `contract\|verify\|determinism` |
| `dartforge-parity placar\|determinismo\|oraculo\|sintaxe-nova\|gerar-corpus\|projetos\|saida\|encurtar` (`_lote`) | `scoreboard\|determinism\|oracle\|new-syntax\|generate-corpus\|projects\|output\|shorten` (`_batch`) |
| binário `dartforge-executar-ir` | `dartforge-run-ir` |
| `scripts/native-corpus.sh tracing\|arc\|preparado\|tabelas` (`igual`/`diferente`, `resumo.tsv`) | `tracing\|arc\|prepared\|tables` (`equal`/`different`, `summary.tsv`) |
| `scripts/ci.ps1 -Frente -Suite -Ref -Fragmentos -Acompanhar -Run -Placar -Manter -Listar` | `-Front -Suite -Ref -Shards -Follow -Run -Scoreboard -Keep -List` |
| `scripts/ci-scoreboard.ps1 -Relatorio -Titulo -Esperados -Linha` | `-Report -Title -Expected -Line` |
| `scripts/zero-cost.ps1 -Atual -Base -Rodadas -Tolerancia -Entrada -Packages -Alvo -Edicoes` | `-Current -Base -Rounds -Tolerance -Entry -Packages -Target -Edits` |
| `scripts/no-ai-trailer.sh --historico` | `--history` |
| `scripts/pub-scoreboard.py --projeto --trabalho --limite-testes --tempo-compilacao --tempo-execucao --memoria-compilacao --memoria-execucao --filtro` | `--project --work --test-limit --compile-time --run-time --compile-memory --run-memory --filter` |
| `scripts/prove-typed-fields-aot.py --saida` | `--output` |
| `bench-http.py`, `compare-http.py`, `compare-performance.py`, `measure-performance-modes.py`: `--repeticoes --duracao --aquecer --conexoes --df-antes --so --saida --prazo --sem-jit --sem-dart --afinidade --modos` | `--repetitions --duration --warmup --connections --df-before --only --output --timeout --no-jit --no-dart --affinity --modes` |
| `flow.mjs --porta --visivel --rotulo` | `--port --visible --label` |
| parâmetros PowerShell: `limitless-ui.ps1 -Preparar -Montar -Producao -Servir -Testes -Porta -Raiz`; `serve.ps1 -Porta -Fluxo -Visivel -Rotulo -Espera`; `clean.ps1 -Limpar -Tudo -AlertaGB`; `corpus-builders.ps1 -Caso -Atualizar -Limpar`; `measure-generation.ps1 -Projeto -Repeticoes -SemMotor`; `measure-js-production.ps1`/`generate-dart-sdk.ps1 -Saida`; `measure-zero-cost.ps1 -Entrada -Rodadas -Tolerancia -ReusarBase`; `measure-lsp.ps1 -Edicoes -AmostraACada -EsperaFinalSeg`; `verify-js-pruning.ps1 -Todos -Entrada -Pacotes` | `-Prepare -Assemble -Production -Serve -Tests -Port -Root`; `-Port -Flow -Visible -Label -Wait`; `-Clean -All -AlertGB`; `-Case -Update -Clean`; `-Project -Repetitions -NoEngine`; `-Output`; `-Entry -Rounds -Tolerance -ReuseBase`; `-Edits -SampleEvery -FinalWaitSec`; `-All -Entry -Packages` |
| controles dos exemplos ARC (`sem-liberar`, `sem-retencao`, `sem-owner`, `sem-drop-*`, `layout-obsoleto`, `copia-no-erro`…; caminho `erro`; `automatico`, `simples`, `misto`, `quadros`, `retoma`, `forcado`, `retorno-mortal-*`) | `no-release`, `no-retain`, `no-owner`, `no-*-drop`, `stale-layout`, `copy-on-error`…; `error`; `automatic`, `simple`, `mixed`, `frames`, `resume`, `forced`, `mortal-return-*` |
| sabotagens (`folha:`, `pouso_sem_topo`, `quadro_sujo`, `bruto_no_mapa`, `sem_uso_ficticio`, `callback_sem_pouso`, `sem_porta`, `folga`, `sem_nop`, `tipo_sem_raiz`) | `leaf:`, `pad_without_top`, `dirty_frame`, `raw_in_map`, `no_fake_use`, `callback_without_pad`, `no_ports`, `slack`, `no_nop`, `unrooted_type` |
| valores `DARTFORGE_GC_STATS=exigir`, `DARTFORGE_JSPROD_STRINGS=todas`, `DARTFORGE_JSPROD_SIGNATURES=todas`, `DARTFORGE_GENERATED=nenhum` | `require`, `all`, `all`, `none` |

### Workflows e ações do GitHub

| antes | depois |
|---|---|
| `pesado.yml` ("Pesado") | `heavy.yml` ("Heavy") |
| `depurar.yml` | `debug.yml` |
| `pub-placar.yml` | `pub-scoreboard.yml` |
| `sem-toolchain.yml` | `no-toolchain.yml` |
| suítes `todos, js, producao, limitless-ui, nativo, jit, unix, determinismo, moderno, macros, macros-aot-sonda, custo-zero, analise`; input `fragmentos`; variável `FRAGMENTOS_NATIVO` | `all, js, production, limitless-ui, native, jit, unix, determinism, modern, macros, macros-aot-probe, zero-cost, analysis`; `shards`; `NATIVE_SHARDS` |
| jobs `compilar`, `producao`, `moderno`, `nativo*`, `sdk-fonte`, `determinismo*`, `analise`, `custo-zero`, `mensagens` | `build`, `production`, `modern`, `native*`, `sdk-source`, `determinism*`, `analysis`, `zero-cost`, `messages` |
| artefatos `binarios`, `relatorio-*`, `placar-*`, `sonda-macros-aot`, `prova-*`; arquivos `relatorio.txt`, `progresso.txt`, `placar.md`, `evidencia.json` | `binaries`, `report-*`, `scoreboard-*`, `macros-aot-probe`, `proof-*`; `report.txt`, `progress.txt`, `scoreboard.md`, `evidence.json` |
| inputs da ação `harness`: `nome, titulo, nativo, aceitar-placar-parcial, exige, preparar` | `name, title, native, accept-partial-scoreboard, requires, prepare` |
| input `gerar-dart-sdk-js` (ação `dart`), `desenvolvimento` (ação `llvm-windows`) | `generate-dart-sdk-js`, `development` |
| chaves de cache `oraculos-*`, `pesado-release`, `pesado-unix`, `custo-zero` | `oracles-*`, `heavy-release`, `heavy-unix`, `zero-cost` |
| diretórios `target/prova-<nome>` dos exemplos ARC | `target/proof-<name>` (`proof-native-port-owned`, `proof-source-return`, `proof-zeroed-instance`, `proof-typed-fields`, `proof-local-fields`, `proof-field-allocation`, `proof-field-keepalive`, `proof-keepalive-pending`, `proof-loop-cleanup`, `proof-arc-resume`, `proof-foreign-unwind`…) |

### Linhas de relatório lidas pelo CI e por scripts

| antes | depois |
|---|---|
| `FALHA` (differential) | `FAIL` |
| `DartForge Nativo:` / `DartForge desenvolvimento:` / `DartForge produção:` | `DartForge native:` / `DartForge development:` / `DartForge production:` |
| `Pendentes (PENDENTES): N ainda falham, M passaram` | `Pending (PENDENTES): N still fail, M passed` |
| `DDC×VM: a/b batem (sem contar n …)` | `DDC×VM: a/b match (not counting n …)` |
| `Falhas por primeira linha do stderr …` | `Failures by first stderr line …` |
| `JIT × AOT: … idênticos, N divergentes`, `Tempos pós-IR`, `razão` | `… identical, N divergent`, `Post-IR times`, `ratio` |
| `(N programas em X s)`, `N programas: … com IR` | `(N programs in X s)`, `N programs: … with IR` |
| `determinismo: …`, `determinismo (IR): …`, `trabalhador(es)`, `linha N:`, `aviso:` | `determinism: …`, `determinism (IR): …`, `worker(s)`, `line N:`, `warning:` |
| `placar paridade:` e o cabeçalho `código` da tabela do placar | `parity scoreboard:` e `code` |
| estados do TSV da matriz `suportado, recusado, divergente, não medido`; perfil `aot-otimizado` | `supported, rejected, divergent, not measured`; `aot-optimized` (o `scripts/matrix.py` continua escrevendo a `docs/MATRIZ.md` em português) |
| `vivos após … média N ms/edição` (exemplo `measure`) | `live after … mean N ms/edit` (o `zero-cost.ps1` lê os dois, para binários antigos do `main`) |
| anotação `::notice title=Placar …` | `::notice title=Scoreboard …` |
| `scripts/pub-scoreboard.py`: `relatorio.txt`, `resultados.json`, `PLACAR:`, chaves `estado`/`erro`, estados `vm-invalido, memoria, compilacao, aot-diferente, vm-instavel, divergente` | `report.txt`, `results.json`, `SCOREBOARD:`, `state`/`error`, `vm-invalid, memory, compile, aot-different, vm-unstable, divergent` |
| texto sintético do oráculo: `erro de compilação na linha N`, `[saída truncada pelo harness …]` | `compile error at line N`, `[output truncated by the harness …]` (novo cache de oráculos em `target/differential/`) |

### Protocolos

| antes | depois |
|---|---|
| handshake `dfexec/1`: `{t: ola, protocolo, servicos, motor\|executor, abi}`, `fim`, `erro {mensagem}` | `{t: hello, protocol, services, engine\|executor, abi}`, `end`, `error {message}` |
| `build.carregar/carregado`, `rodada`, `extensoes`, `entradas_pos`, `executar`, `posprocessar`, `resultado`, `resposta`, `ler`, `existe`, `escrever` | `build.load/loaded`, `round`, `extensions`, `post_inputs`, `run`, `post_process`, `result`, `response`, `read`, `exists`, `write` (`glob`, `digest`, `log`, `resolver.*` ficam) |
| campos do `build.*`: `chave, fabrica, opcoes, fase, entrada, saidas_permitidas, saidas, falhou, apagados, entradas, extensoes, padrao, sim, nivel (fino/aviso/severo), mensagem, aplicacoes, fabricas, chave_de_cache`; erros `SaidaNaoPermitida`, `indisponivel` | `key, factory, options, phase, input, allowed_outputs, outputs, failed, deleted, inputs, extensions, pattern, exists, level (fine/warning/severe), message, applications, factories, cache_key`; `OutputNotAllowed`, `unavailable` |
| `macro.instanciar/instancia/executar/resultado/consulta/resposta/descartar` | `macro.instantiate/instance/execute/result/query/response/discard` |
| campos do envelope `macro.*`: `construtor(es), argumentos, instancia, fase, alvo, modelo, execucao, tipo, valor, resultado, erro {tipo, mensagem}` | `constructor(s), arguments, instance, phase, target, model, execution, kind, value, result, error {kind, message}` (o conteúdo de `arguments`, `model` e `result` é a fase 4) |
| comentário do bootstrap gerado "Gerado pelo DartForge…" | "Generated by DartForge…" |
| recarga do `dev`: `{tipo: ola\|recarregar\|erro, protocolo, geracao, mensagem}`, rota `/__dartforge_recarga` | `{type: hello\|reload\|error, protocol, generation, message}`, `/__dartforge_reload` |

### Arquivos de dados e formatos em disco

| antes | depois |
|---|---|
| `crates/analysis/verificados.txt` | `crates/analysis/verified.txt` |
| `lib/dartforge-distribuicao.json` (distribuição) | `lib/dartforge-distribution.json` |
| `corpus/inferencia/*.esperado.tsv`, `divergencias.txt` | `*.expected.tsv`, `divergences.txt` |
| `corpus/diagnosticos/*/oraculo.jsonl`, `oraculo.json`, `grupo.json`; `sintaxe-nova.json`, `projetos.json`, `mutacoes.jsonl` | `oracle.jsonl`, `oracle.json`, `group.json`; `new-syntax.json`, `projects.json`, `mutations.jsonl` |
| chaves JSON da paridade `arquivo, arquivos, diagnosticos, excluidos, pacotes, origem, nome, caminho, oraculo, projeto, tipo, inicio, fim, inserir`; tipos de mutação `nome`/`tipo` | `file, files, diagnostics, excluded, packages, origin, name, path, oracle, project, kind, start, end, insert`; `name`/`type` |
| estado do motor `.dart_tool/dartforge/build/estado/estado.json` (`formato, chave, codigo_dart, acoes, origem nativo/apoio/omitida, motivo, consultas arquivo/existe/glob_ativos, saidas, do_apoio, apagados`) | `…/build/state/state.json` (`format, key, dart_code, actions, origin native/support/skipped, reason, queries file/exists/glob_assets, outputs, from_support, deleted`) |
| `target/diferencial/` (`nativo/`, `producao/`, `contrato/`, `determinismo/`, `saida.js`) | `target/differential/` (`native/`, `production/`, `contract/`, `determinism/`, `output.js`) |
| `target/paridade-cache-dart`, `target/scratch-paridade/projetos` | `target/parity-cache-dart`, `target/scratch-parity/projects` |
| diretórios `nativo/` (cache nativo do usuário, executores nativos de macros e do `compile-js`) | `native/` |
| arquivo `versao-do-sdk` do sysroot macOS | `sdk-version` |
| chaves do SDK compilado `rastro=simbolico`, `excecoes=tabelas`, `raizes=mapas_v2`, `conferir`, `efeitos=conferir`, `sabotagem=`, `memoria=arc_v1`, `arc=preparado_v1`, `dartforge-sdk-fonte` | `stack_trace=symbolic`, `exceptions=tables`, `roots=maps_v2`, `check`, `effects=check`, `sabotage=`, `memory=arc_v1`, `arc=prepared_v1`, `dartforge-sdk-source` |
| sessão gravada `corpus/macros/410_json_codable/esperado/sessao.dfexec` | mesmo arquivo, com o envelope novo (regravada pela VM 3.6.2 byte a byte igual) |
| `scripts/prove-arc-aot.py`: `evidencia.json` (`fonte, exemplo, execucoes, resultado sucesso/falha…`) | `evidence.json` (`source, example, runs, result success/failure…`) |

### Variáveis de ambiente (lista completa)

| antes | depois |
|---|---|
| `DARTFORGE_AJUDANTES_FORA` | `DARTFORGE_OUTLINED_HELPERS` |
| `DARTFORGE_ALOCADOR_SEM_CACHE` | `DARTFORGE_ALLOCATOR_NO_CACHE` |
| `DARTFORGE_ARC_BERCARIO` | `DARTFORGE_ARC_NURSERY` |
| `DARTFORGE_ARC_CICLOS` | `DARTFORGE_ARC_CYCLES` |
| `DARTFORGE_ARC_CONFERIR` | `DARTFORGE_ARC_CHECK` |
| `DARTFORGE_ARC_PREPARADO` | `DARTFORGE_ARC_PREPARED` |
| `DARTFORGE_ARC_TRACO` | `DARTFORGE_ARC_TRACE` |
| `DARTFORGE_AVISOS` | `DARTFORGE_WARNINGS` |
| `DARTFORGE_AVISOS_RESUMO` | `DARTFORGE_WARNINGS_SUMMARY` |
| `DARTFORGE_BUILD_COMPILANDO_EXECUTOR` | `DARTFORGE_BUILD_COMPILING_EXECUTOR` |
| `DARTFORGE_BUILD_ESTADO` | `DARTFORGE_BUILD_STATE` |
| `DARTFORGE_BUILD_NATIVO` | `DARTFORGE_BUILD_NATIVE` |
| `DARTFORGE_BUILD_PRAZO` | `DARTFORGE_BUILD_TIMEOUT` |
| `DARTFORGE_CACHE_NATIVO` | `DARTFORGE_NATIVE_CACHE` |
| `DARTFORGE_CAMPOS_POR_CHAMADA` | `DARTFORGE_FIELDS_BY_CALL` |
| `DARTFORGE_CASOS` | `DARTFORGE_CASES` |
| `DARTFORGE_COMPILADOR` | `DARTFORGE_COMPILER` |
| `DARTFORGE_COMPILADOR_NATIVO` | `DARTFORGE_NATIVE_COMPILER` |
| `DARTFORGE_CONFERIR_RS4GC` | `DARTFORGE_CHECK_RS4GC` |
| `DARTFORGE_CONFERIR_SOMBRA` | `DARTFORGE_CHECK_SHADOW` |
| `DARTFORGE_CRONO` | `DARTFORGE_MODULE_TIMES` |
| `DARTFORGE_DEPURAR` | `DARTFORGE_DEBUG` |
| `DARTFORGE_DESPEJAR_MODULO` | `DARTFORGE_DUMP_MODULE` |
| `DARTFORGE_DEV_SAIDA` | `DARTFORGE_DEV_OUTPUT` |
| `DARTFORGE_EFEITOS` | `DARTFORGE_EFFECTS` |
| `DARTFORGE_EMISSOR_HASH` | `DARTFORGE_EMITTER_HASH` |
| `DARTFORGE_EXCECOES` | `DARTFORGE_EXCEPTIONS` |
| `DARTFORGE_EXECUTAR_IR` | `DARTFORGE_RUN_IR` |
| `DARTFORGE_FONTE_NAO_SUPORTADO` | `DARTFORGE_SOURCE_UNSUPPORTED` |
| `DARTFORGE_GC_AGENDA` | `DARTFORGE_GC_SCHEDULE` |
| `DARTFORGE_GC_CRESCIMENTO` | `DARTFORGE_GC_GROWTH` |
| `DARTFORGE_GC_JOVEM_KB` | `DARTFORGE_GC_YOUNG_KB` |
| `DARTFORGE_GC_MEMORIA` | `DARTFORGE_GC_MEMORY` |
| `DARTFORGE_GC_PERCURSO` | `DARTFORGE_GC_WALK` |
| `DARTFORGE_GC_RASTRO` | `DARTFORGE_GC_TRACE` |
| `DARTFORGE_GC_VENENO` | `DARTFORGE_GC_POISON` |
| `DARTFORGE_GC_VERIFICAR` | `DARTFORGE_GC_VERIFY` |
| `DARTFORGE_GERADOR` | `DARTFORGE_GENERATOR` |
| `DARTFORGE_GERADOS` | `DARTFORGE_GENERATED` |
| `DARTFORGE_GERADOS_DEBUG` | `DARTFORGE_GENERATED_DEBUG` |
| `DARTFORGE_GERADOS_PKGS` | `DARTFORGE_GENERATED_PKGS` |
| `DARTFORGE_GRAVAR_SESSAO` | `DARTFORGE_RECORD_SESSION` |
| `DARTFORGE_IDENTIDADE_REF` | `DARTFORGE_IDENTITY_REF` |
| `DARTFORGE_IO_RASTRO` | `DARTFORGE_IO_TRACE` |
| `DARTFORGE_IR_PARALELO_MAX` | `DARTFORGE_IR_PARALLEL_MAX` |
| `DARTFORGE_JSPROD_ACHATAR` | `DARTFORGE_JSPROD_FLATTEN` |
| `DARTFORGE_JSPROD_ACHATAR_POR` | `DARTFORGE_JSPROD_FLATTEN_WHY` |
| `DARTFORGE_JSPROD_ASSINATURAS` | `DARTFORGE_JSPROD_SIGNATURES` |
| `DARTFORGE_JSPROD_CAMINHO` | `DARTFORGE_JSPROD_PATH` |
| `DARTFORGE_JSPROD_COMPRIMIR` | `DARTFORGE_JSPROD_COMPRESS` |
| `DARTFORGE_JSPROD_CONFERIR` | `DARTFORGE_JSPROD_CHECK` |
| `DARTFORGE_JSPROD_CURAS` | `DARTFORGE_JSPROD_FIXES` |
| `DARTFORGE_JSPROD_DESPEJO` | `DARTFORGE_JSPROD_DUMP` |
| `DARTFORGE_JSPROD_ESTRITO` | `DARTFORGE_JSPROD_STRICT` |
| `DARTFORGE_JSPROD_FORMAS` | `DARTFORGE_JSPROD_SHAPES` |
| `DARTFORGE_JSPROD_GATILHO` | `DARTFORGE_JSPROD_TRIGGER` |
| `DARTFORGE_JSPROD_NOMEADOS` | `DARTFORGE_JSPROD_NAMED` |
| `DARTFORGE_JSPROD_NOMES` | `DARTFORGE_JSPROD_NAMES` |
| `DARTFORGE_JSPROD_NOMES_PROPRIOS` | `DARTFORGE_JSPROD_OWN_NAMES` |
| `DARTFORGE_JSPROD_POR` | `DARTFORGE_JSPROD_WHY` |
| `DARTFORGE_JSPROD_PROPS_POR` | `DARTFORGE_JSPROD_PROPS_WHY` |
| `DARTFORGE_JSPROD_QUEM` | `DARTFORGE_JSPROD_WHO` |
| `DARTFORGE_JSPROD_RASTRO` | `DARTFORGE_JSPROD_TRACE` |
| `DARTFORGE_JSPROD_RECEITAS_FIXAS` | `DARTFORGE_JSPROD_FIXED_RECIPES` |
| `DARTFORGE_JSPROD_RECEITA_CURTA` | `DARTFORGE_JSPROD_SHORT_RECIPE` |
| `DARTFORGE_JSPROD_REGRAS` | `DARTFORGE_JSPROD_RULES` |
| `DARTFORGE_JSPROD_RODADAS` | `DARTFORGE_JSPROD_ROUNDS` |
| `DARTFORGE_JSPROD_VERIFICAR` | `DARTFORGE_JSPROD_VERIFY` |
| `DARTFORGE_JS_CONFERIR_TIPOS` | `DARTFORGE_JS_CHECK_TYPES` |
| `DARTFORGE_LIGAR_COM_CLANG` | `DARTFORGE_LINK_WITH_CLANG` |
| `DARTFORGE_LSP_AMPLO_MIB` | `DARTFORGE_LSP_WIDE_MIB` |
| `DARTFORGE_LSP_SESSAO_MIB` | `DARTFORGE_LSP_SESSION_MIB` |
| `DARTFORGE_LSP_TIPADO` | `DARTFORGE_LSP_TYPED` |
| `DARTFORGE_LTO_DISTRIBUIDA` | `DARTFORGE_LTO_DISTRIBUTED` |
| `DARTFORGE_LTO_NIVEL` | `DARTFORGE_LTO_LEVEL` |
| `DARTFORGE_LTO_PERFIL` | `DARTFORGE_LTO_PROFILE` |
| `DARTFORGE_MACROS_NATIVO` | `DARTFORGE_MACROS_NATIVE` |
| `DARTFORGE_MANTER_SIMBOLOS` | `DARTFORGE_KEEP_SYMBOLS` |
| `DARTFORGE_MAPAS_REGISTRADORES` | `DARTFORGE_REGISTER_MAPS` |
| `DARTFORGE_MAPA_DA_LIGACAO` | `DARTFORGE_LINK_MAP` |
| `DARTFORGE_MEDICAO_N` | `DARTFORGE_MEASUREMENT_N` |
| `DARTFORGE_MEDIR_SAIDA` | `DARTFORGE_MEASURE_OUTPUT` |
| `DARTFORGE_MEMORIA` | `DARTFORGE_MEMORY` |
| `DARTFORGE_MOTOR_TEMPOS` | `DARTFORGE_ENGINE_TIMES` |
| `DARTFORGE_OPTSIZE_PROGRAMA` | `DARTFORGE_OPTSIZE_PROGRAM` |
| `DARTFORGE_ORACULO_3_13` | `DARTFORGE_ORACLE_3_13` |
| `DARTFORGE_ORACULO_3_6` | `DARTFORGE_ORACLE_3_6` |
| `DARTFORGE_ORCAMENTO_MAPAS` | `DARTFORGE_MAPS_BUDGET` |
| `DARTFORGE_OTIMIZAR` | `DARTFORGE_OPTIMIZE` |
| `DARTFORGE_OTIMIZAR_HIR` | `DARTFORGE_OPTIMIZE_HIR` |
| `DARTFORGE_PARIDADE_AMOSTRAS` | `DARTFORGE_PARITY_SAMPLES` |
| `DARTFORGE_PARIDADE_TETO_MB` | `DARTFORGE_PARITY_CAP_MB` |
| `DARTFORGE_PARTES_PARALELAS` | `DARTFORGE_PARALLEL_PARTS` |
| `DARTFORGE_PARTE_MB` | `DARTFORGE_PART_MB` |
| `DARTFORGE_PIPELINE_OBJETO` | `DARTFORGE_OBJECT_PIPELINE` |
| `DARTFORGE_POR_QUE` | `DARTFORGE_WHY` |
| `DARTFORGE_PRODUCAO_SEM_LTO` | `DARTFORGE_PRODUCTION_NO_LTO` |
| `DARTFORGE_PROGRAMA` | `DARTFORGE_PROGRAM` |
| `DARTFORGE_RAIZES` | `DARTFORGE_ROOTS` |
| `DARTFORGE_RAIZES_CONFERIR` | `DARTFORGE_ROOTS_CHECK` |
| `DARTFORGE_RAIZES_NO_CHAMADO` | `DARTFORGE_ROOTS_IN_CALLEE` |
| `DARTFORGE_RASTRO` | `DARTFORGE_CALL_TRACE` |
| `DARTFORGE_RASTRO_CRU` | `DARTFORGE_STACK_TRACE_RAW` |
| `DARTFORGE_RASTRO_VM` | `DARTFORGE_STACK_TRACE` |
| `DARTFORGE_RELATORIO_MAPAS` | `DARTFORGE_MAPS_REPORT` |
| `DARTFORGE_RUNTIME_DIR_DO_BUILD` | `DARTFORGE_RUNTIME_BUILD_DIR` |
| `DARTFORGE_RUNTIME_PRINCIPAL` | `DARTFORGE_RUNTIME_MAIN` |
| `DARTFORGE_RUNTIME_PRODUCAO` | `DARTFORGE_RUNTIME_PRODUCTION` |
| `DARTFORGE_RUNTIME_SEM_PRECOMPILAR` | `DARTFORGE_RUNTIME_NO_PRECOMPILE` |
| `DARTFORGE_SABOTAGEM` | `DARTFORGE_SABOTAGE` |
| `DARTFORGE_SDK_COMPILAR` | `DARTFORGE_SDK_COMPILE` |
| `DARTFORGE_SDK_DA_FONTE` | `DARTFORGE_SDK_FROM_SOURCE` |
| `DARTFORGE_SDK_NATIVO` | `DARTFORGE_NATIVE_SDK` |
| `DARTFORGE_SDK_RASTRO` | `DARTFORGE_SDK_TRACE` |
| `DARTFORGE_SEM_CACHE_RTI` | `DARTFORGE_NO_RTI_CACHE` |
| `DARTFORGE_SEM_CACHE_THINLTO` | `DARTFORGE_NO_THINLTO_CACHE` |
| `DARTFORGE_SEM_CLASSE_EM_LINHA` | `DARTFORGE_NO_INLINE_CLASS` |
| `DARTFORGE_SEM_EFEITOS_NA_CONFERENCIA` | `DARTFORGE_NO_EFFECTS_IN_CHECK` |
| `DARTFORGE_SEM_MAPA_COMPACTO` | `DARTFORGE_NO_COMPACT_MAP` |
| `DARTFORGE_SEM_PODA_DE_TABELAS` | `DARTFORGE_NO_TABLE_PRUNING` |
| `DARTFORGE_SEM_PODA_DO_PROGRAMA` | `DARTFORGE_NO_PROGRAM_PRUNING` |
| `DARTFORGE_SEM_PODA_FFI` | `DARTFORGE_NO_FFI_PRUNING` |
| `DARTFORGE_SEM_PODA_HIR` | `DARTFORGE_NO_HIR_PRUNING` |
| `DARTFORGE_SEM_QUADRO_NA_ENTRADA` | `DARTFORGE_NO_FRAME_AT_ENTRY` |
| `DARTFORGE_SEM_VERSIONAR_TIPADAS` | `DARTFORGE_NO_TYPED_VERSIONING` |
| `DARTFORGE_SHIM_GERADO` | `DARTFORGE_SHIM_GENERATED` |
| `DARTFORGE_SHIM_ORACULO` | `DARTFORGE_SHIM_ORACLE` |
| `DARTFORGE_SHIM_RAIZES` | `DARTFORGE_SHIM_ROOTS` |
| `DARTFORGE_TESTES_MAPAS` | `DARTFORGE_MAPS_TESTS` |
| `DARTFORGE_VALIDAR_HANDLES` | `DARTFORGE_VALIDATE_HANDLES` |
| `MEDIR_COMPONENTE` | `MEASURE_COMPONENT` |
| `MEDIR_SEM_ANGULAR` | `MEASURE_NO_ANGULAR` |
| `MEDIR_DEPURAR` | `MEASURE_DEBUG` |
| `EMITIR_CORPUS_EXECUTAVEL` | `EMIT_CORPUS_EXECUTABLE` |
| `DF_ARC_SEMENTES` | `DF_ARC_SEEDS` |
| `ATUALIZAR_DIVERGENCIAS` | `UPDATE_DIVERGENCES` |
| `NAVEGADOR` | `BROWSER` |
| `SONDA_CLICK` | `PROBE_CLICK` |

### Opções de linha de comando (todos os binários e exemplos)

| antes | depois |
|---|---|
| `--aceitar-pendentes` | `--accept-pending` |
| `--anulavel` | `--nullable` |
| `--argumento` | `--argument` |
| `--arquivos` | `--files` |
| `--avisos` | `--warnings` |
| `--bloco` | `--block` |
| `--captura` | `--capture` |
| `--captura-vizinho` | `--capture-neighbor` |
| `--celula` | `--cell` |
| `--ciclo` | `--cycle` |
| `--comparar` | `--compare` |
| `--contexto` | `--context` |
| `--crescer` | `--grow` |
| `--depuracao` | `--debug` |
| `--detalhes` | `--details` |
| `--escrever-cache` | `--write-cache` |
| `--estado` | `--state` |
| `--estrito` | `--strict` |
| `--excecoes` | `--exceptions` |
| `--executar` | `--run` |
| `--exemplos` | `--examples` |
| `--filho` | `--child` |
| `--filtro` | `--filter` |
| `--forma` | `--shape` |
| `--fragmento` | `--shard` |
| `--gravar` | `--write` |
| `--grupo` | `--group` |
| `--intervalo` | `--interval` |
| `--inventario-externs` | `--externs-inventory` |
| `--limite` | `--limit` |
| `--limite-exec` | `--exec-limit` |
| `--livre` | `--free` |
| `--lote` | `--batch` |
| `--materializar` | `--materialize` |
| `--matriz` | `--matrix` |
| `--memoria` | `--memory` |
| `--misto` | `--mixed` |
| `--mutacoes` | `--mutations` |
| `--nativo` | `--native` |
| `--omitir-checagens` | `--omit-checks` |
| `--oraculo` | `--oracle` |
| `--otimizar` | `--optimize` |
| `--perfil` | `--profile` |
| `--plano` | `--plan` |
| `--porta` | `--port` |
| `--preservar-estado` | `--preserve-state` |
| `--producao` | `--production` |
| `--quadro` | `--frame` |
| `--raiz` | `--root` |
| `--raizes` | `--roots` |
| `--rastro` | `--stack-trace` |
| `--referencias` | `--references` |
| `--regravar` | `--rerecord` |
| `--reiniciar` | `--restart` |
| `--resumo` | `--digest` |
| `--sdk-no-grupo` | `--sdk-in-group` |
| `--sdk-proprio` | `--own-sdk` |
| `--sem-cache` | `--no-cache` |
| `--sem-forge` | `--no-forge` |
| `--sem-membros` | `--no-members` |
| `--sem-minificar` | `--no-minify` |
| `--sem-poda` | `--no-prune` |
| `--sem-poda-usuario` | `--no-user-prune` |
| `--silencioso` | `--quiet` |
| `--tipada` | `--typed` |
| `--tipada-erro` | `--typed-error` |
| `--tipos` | `--types` |
| `--todos` | `--all` |
| `--trabalhadores` | `--workers` |
| `--uma-vez` | `--once` |
| `--verificar-stub` | `--verify-stub` |
| `--versao-linguagem` | `--language-version` |
| `--version-linguagem` | `--language-version` |
| `--vizinho` | `--neighbor` |

### Scripts (`scripts/`)

| antes | depois |
|---|---|
| `analisar-efeitos-runtime.py` | `analyze-runtime-effects.py` |
| `caminho_dos_casos.py` | `cases_path.py` |
| `ci-placar.ps1` | `ci-scoreboard.ps1` |
| `comparar-desempenho.py` | `compare-performance.py` |
| `comparar-http.py` | `compare-http.py` |
| `comparar-linguagens.py` | `compare-languages.py` |
| `comparar-placares.py` | `compare-scoreboards.py` |
| `conferir-tipos-js.mjs` | `check-js-types.mjs` |
| `copiar_sondas_saida.py` | `copy_output_probes.py` |
| `corpus-nativo.sh` | `native-corpus.sh` |
| `corpus-ngdart-dependente.sh` | `corpus-ngdart-dependent.sh` |
| `corpus-perfis.py` | `corpus-profiles.py` |
| `custo-zero.ps1` | `zero-cost.ps1` |
| `fluxo.mjs` | `flow.mjs` |
| `gerar-dart-sdk.ps1` | `generate-dart-sdk.ps1` |
| `gerar-efeitos.py` | `generate-effects.py` |
| `gerar-especies-de-acao.py` | `generate-action-kinds.py` |
| `gerar-regexp-unicode.py` | `generate-regexp-unicode.py` |
| `gerar-tabelas-analise.py` | `generate-analysis-tables.py` |
| `harness-com-gerador.ps1` | `harness-with-generator.ps1` |
| `limitless-ui-producao.sh` | `limitless-ui-production.sh` |
| `limpar.ps1` | `clean.ps1` |
| `matriz.py` | `matrix.py` |
| `matriz.sh` | `matrix.sh` |
| `medir-custo-zero.ps1` | `measure-zero-cost.ps1` |
| `medir-geracao.ps1` | `measure-generation.ps1` |
| `medir-js-producao-tokens.py` | `measure-js-production-tokens.py` |
| `medir-js-producao.ps1` | `measure-js-production.ps1` |
| `medir-lsp.ps1` | `measure-lsp.ps1` |
| `medir-mapas.sh` | `measure-maps.sh` |
| `medir-modos-desempenho.py` | `measure-performance-modes.py` |
| `test_medir_modos_desempenho.py` | `test_measure_performance_modes.py` |
| `oraculo_augmentation.dart` | `oracle_augmentation.dart` |
| `paridade-pub-cache.py` | `parity-pub-cache.py` |
| `preparar-macros-410.ps1` | `prepare-macros-410.ps1` |
| `provar-arc-aot.py` | `prove-arc-aot.py` |
| `provar-campos-tipados-aot.py` | `prove-typed-fields-aot.py` |
| `pub-placar.py` | `pub-scoreboard.py` |
| `remover-funcao.py` | `remove-function.py` |
| `sass-formas-166.sh` | `sass-forms-166.sh` |
| `sass-formas.sh` | `sass-forms.sh` |
| `sem-trailer-ia.sh` | `no-ai-trailer.sh` |
| `servir.ps1` | `serve.ps1` |
| `sondar-limitless-ui.mjs` | `probe-limitless-ui.mjs` |
| `sondar-macros-aot.ps1` | `probe-macros-aot.ps1` |
| `testar-macros-410-materializado.ps1` | `test-macros-410-materialized.ps1` |
| `testar-macros-412.ps1` | `test-macros-412.ps1` |
| `testar-macros-413.ps1` | `test-macros-413.ps1` |
| `testar-macros-414.ps1` | `test-macros-414.ps1` |
| `testar-macros-415.ps1` | `test-macros-415.ps1` |
| `testar-macros-416.ps1` | `test-macros-416.ps1` |
| `testar-macros-418.ps1` | `test-macros-418.ps1` |
| `testar-macros-419.ps1` | `test-macros-419.ps1` |
| `testar-macros-420.ps1` | `test-macros-420.ps1` |
| `testar-macros-421.ps1` | `test-macros-421.ps1` |
| `testar-macros-422.ps1` | `test-macros-422.ps1` |
| `testar-macros-builder.ps1` | `test-macros-builder.ps1` |
| `verificar-poda-js.ps1` | `verify-js-pruning.ps1` |
| `verificar-recarga.mjs` | `verify-reload.mjs` |
| `dados/efeitos-runtime/efeitos.tsv` | `data/runtime-effects/effects.tsv` |
