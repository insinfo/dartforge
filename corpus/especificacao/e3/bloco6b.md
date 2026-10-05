
#### E.3.9 No DartForge: onde está cada decisão e a diferença

Arquivos em `crates/frontend/src/parser/` (linhas do disco em 2026-10-04, com o trabalho não
commitado). Diferença de fundo: o fasta **nunca falha** — relata, insere token sintético e segue,
devolvendo sempre um nó; o parser Rust devolve `Err` e sincroniza pulando tokens
(`recover_top_level`, `recover_member`). Toda regra "relata e continua" do fasta que no Rust termina
em `Err` perde os relatos seguintes do mesmo membro e pode criar relatos que o fasta não dá.

| decisão do fasta | no DartForge | diferença |
|---|---|---|
| laço de topo, sem progresso (`pi:403-437`) | `declarations.rs:132-145`, `recover_top_level` `:326` | com progresso, pula até `;`/`}` no nível zero ou até um início de declaração; o fasta não pula: cada ramo já deixou tokens sintéticos |
| `parseInvalidTopLevelDeclaration` (`pi:9468`) | `:169-177` (`;` → `UNEXPECTED_TOKEN`; resto → `EXPECTED_EXECUTABLE`), `can_start_declaration` `:286` | `{…}` solto não é consumido como bloco (um relato a mais no `}`); operador + `(` (`TOP_LEVEL_OPERATOR`) não tratado aqui |
| rota de topo (`pi:556-636`) | `rota_de_topo` `:811`, `prefixos_de_mixin_e_enum` `:891` | equivalente (porte direto) |
| modificadores antes de palavra de topo (`mc:122-300`) | `modificadores_antes_de_palavra` `:855`, `modificadores.rs:481` | equivalente; `augment` fora das fichas (lido por `parse_augment_opt` `:269`) |
| ordem das diretivas (`directive_context.dart`) | `conferir_ordem_de_diretiva` `:192` | equivalente; falta a troca de `ab:203` (`NON_PART_OF_DIRECTIVE_IN_PART` → `DIRECTIVE_AFTER_DECLARATION` sem diretivas) — não verificado se há caso |
| campo × método no topo (`pi:3613-3618`) | `parse_function_or_variables` `:2063-2064` | falta o `.` (nome seguido de `.` é método): FN de `missing_function_parameters`/`missing_function_body` |
| identificador + reservada + (`;` `=` `(` `{` `=>` `<`) → tipo + nome (`pi:3540`, `4743`) | `:1952-1957` | lê sempre o identificador como campo sem tipo: FP `expected_class_member`, `missing_const_final_var_or_type` |
| `get`/`set` + reservada (`pi:4609`) | `accessor_follows` `:1862` | exige identificador: FP `missing_method_parameters` |
| `parseGetterOrFormalParameters` (`pi:1541`) | `:2068-2075`, `:2160-2187` | equivalente para `{`/`=>`; outros tokens depois do nome caem em campo |
| `parseFunctionBody` (`pi:5415`) | `parse_body_after_modifier` `:3143`, `conferir_corpo_vazio` `:3091`, `permite_abstrato` `:3081` | equivalente |
| cabeçalho de classe + recuperação (`pi:2715-2863`) | `parse_class` `:954-1049` | cláusulas só na ordem fixa; **sem** `parseClassHeaderRecovery`: `IMPLEMENTS_BEFORE_EXTENDS`, `WITH_BEFORE_EXTENDS`, `IMPLEMENTS_BEFORE_WITH`, `MULTIPLE_*_CLAUSES`, `EXPECTED_INSTEAD`, token solto no cabeçalho não existem (sem amostra no placar) |
| cabeçalho de mixin (`pi:2939-3046`) | `parse_mixin` `:1405` | só o token solto antes de `on`/`implements`/`{` (`:1429`); sem `MULTIPLE_ON_CLAUSES`, `IMPLEMENTS_BEFORE_ON`, `MIXIN_WITH_CLAUSE` |
| enum (`pi:2367-2509`) | `parse_enum` `:1458` | sem `{`: `expect_op` (`:1472`) dá `EXPECTED_TOKEN '{'` em vez de `MISSING_ENUM_BODY`; sem recuperação de cláusulas (`MULTIPLE_CLAUSES`, `OUT_OF_ORDER_CLAUSES`, `UNEXPECTED_TOKENS`) |
| extension (`pi:3108`) | `parse_extension` `:1564` | `on` que falta: equivalente; sem o laço de `,`/`extends`/`implements`/`with` depois do tipo |
| `ensureBlock` de declaração (`pi:4200`) | `parse_class_body_ou_vazio` `:1094-1118` | equivalente (último token lido) |
| laço de membros (`pi:4376`) | `parse_member_list` `:2382`, `recover_member` `:427` | recuperação por pulo com tentativa especulativa; o fasta não pula |
| `recoverFromInvalidMember` (`pi:9325`) | `parse_member_dentro` `:2414-2428` | só as linhas 5 (`=>`) e 6 da tabela de §E.3.3; `CLASS_IN_CLASS`, `ENUM_IN_CLASS`, `TYPEDEF_IN_CLASS`, `MISSING_KEYWORD_OPERATOR` não existem |
| construtor × método, decidido no fim (`pi:4990`) | `constructor_follows` `:2630`, `constructor_com_retorno` `:2588`, `metodo_sem_parametros` `:2555` — decidido **antes**, por lookahead | cobre `Nome(`, `Nome.x(`, `X(...) :`, `T Nome(`, `T X.Y(`; não cobre `T nome(...) :`, `get X.y`, `GETTER_/SETTER_CONSTRUCTOR` |
| erros de construtor (`pi:5009-5029`) | `parse_constructor` `:2693-2713`, `parse_constructor_resto` `:2821` | `CONSTRUCTOR_WITH_RETURN_TYPE` cobre o tipo inteiro (fasta: 1º token) |
| factory (`pi:5104`) | `parse_constructor` `:2668-2692`, `:2834-2858`, `conferir_corpo_externo` `:3044` | falta `EXTERNAL_FACTORY_REDIRECTION`, `parseModifiersAfterFactory` (`factory static`, `factory const`…), `MISSING_FUNCTION_PARAMETERS` |
| AstBuilder de construtor (`ab:5897-5920`) | `:2708-2712` (parâmetros de tipo), `:2881-2895` (`const` com corpo) | falta `EXTERNAL_CONSTRUCTOR_WITH_FIELD_INITIALIZERS` |
| `_parseModifiers` e `_parseX` (`mc:325-639`) | `modificadores.rs:151-337` | equivalente (porte linha a linha) |
| caminho rápido de topo e de membro | `modificadores.rs:368-390`, `:397-423` | equivalente |
| caminho rápido de parâmetro + M3 (`pi:1944-2009`) | `types.rs:690-696` | **sem** caminho rápido: tudo pelo contexto, `opcional_nomeado` fixo → falta o `EXTRANEOUS_MODIFIER` de `required` (M3) |
| modificadores de variável local (`pi:8023-8056`, `mc:303`) | `statements.rs:452-484` | não portado: `static int x`, `final final x`, `var late y` em bloco não seguem o fasta (sem amostra no placar) |
| método: `abstract`, `late`, `static` operador, `covariant`, `const`, `var`, `final` (`pi:4823-4877`) | `relatar_modificadores_de_metodo` `declarations.rs:2243` | equivalente |
| extension type, cabeçalho (`pi:3196`) | `parse_extension_type` `:1614` | sem `parseExtensionTypeHeaderRecovery` (`EXTENSION_TYPE_EXTENDS`, `EXTENSION_TYPE_WITH` não existem) |
| representação 3.6.2 (`ab:2843`) | `parse_representacao` `:1679-1757` | equivalente, salvo `final`/`var` sem tipo (não relata o tipo, `:1728`) |
| representação 3.13.4 ([main] `error_verifier.dart:5597`) | mesma função, guardas `versao() > PISO` (`:1708`, `:1736`) | posições novas (nome, `this`, `super`, delimitador), mensagem fixa e retorno cedo não implementados; a guarda devia ser "oráculo 3.13.4", não a versão |
| parte `this` de construtor primário ([main] `parser_impl.dart:5559-5605`, `3912`) | `parse_parte_primaria` `:2771`, `elaborar_construtor_primario` `:1159` | modificadores antes de `this` não relatados; extension type não elaborado |

**Não verificado** nesta parte: (1) comportamento do binário 3.13.4 — tudo o que está marcado
[main] foi lido no checkout 3.14 e conferido só contra o oráculo gravado das amostras citadas;
(2) `recoverFromStackOverflow` e `MISSING_FUNCTION_PARAMETERS` em função local/expressão de função,
sem exemplo rodado; (3) a saída atual do DartForge para os casos `a/b/c` (não há binário atualizado:
as diferenças da tabela vêm da leitura do código e do placar r7); (4) a ordem de **emissão** dos
diagnósticos dentro do analyzer (as saídas estão ordenadas por offset).
