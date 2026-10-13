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
