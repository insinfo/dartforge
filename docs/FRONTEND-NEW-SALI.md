# new_sali/frontend no navegador: verificação de fluxo real

Alvo: `C:/MyDartProjects/new_sali/frontend` (ngdart 8.0.0-dev.4, `dart:html`,
`package:js`, OIDC), entrada `web/main.dart`, `package_config.json` em
`frontend/.dart_tool/`. Os `.template.dart` gerados pelo `build_runner` ficam em
`.dart_tool/build/generated/<pacote>/lib/…` e o carregador os sobrepõe ao `lib/`
(`PackageConfig::generated_path`).

    work/bin/dartforge.exe compile-js C:/MyDartProjects/new_sali/frontend/web/main.dart \
        -o work/fe/out --packages C:/MyDartProjects/new_sali/frontend/.dart_tool/package_config.json
    pwsh scripts/servir.ps1 -Dir work/fe/out -Web C:/MyDartProjects/new_sali/frontend/web -Fluxo

Estado: **616 módulos emitidos, `node --check` 616/616**, os 11 passos do fluxo
abaixo percorridos no Edge headless **sem nenhuma exceção não tratada**, e duas
divergências de comportamento em relação ao `dartdevc` (fila no fim).

## O verificador de fluxo (`scripts/servir.ps1 -Fluxo` → `scripts/fluxo.mjs`)

`servir.ps1` continua servindo o diretório e despejando o DOM (`-Headless`);
com `-Fluxo` ele delega ao `scripts/fluxo.mjs` (Node, sem dependências — o
Node 22+ tem `WebSocket` global), que:

* serve o diretório emitido com **fallback de SPA** apenas para navegações
  (`Accept: text/html`), necessário porque o ngdart usa `routerProviders` com
  `<base href="/">`; XHR/`fetch` de API continuam a receber 404, que é o que a
  aplicação veria;
* injeta no `index.html` o coletor `window.onerror` + `unhandledrejection` +
  `console.error` e troca o `main.dart.js` pelo `main.mjs` emitido;
* abre o Edge com `--headless=new --remote-debugging-port` e conduz os passos
  pelo **CDP** por WebSocket (`Page.navigate`, `Runtime.evaluate`, cliques com
  `userGesture`), esperando a aplicação montar entre um passo e outro;
* **bloqueia e registra** as navegações e pedidos para fora do servidor local
  (`Fetch.requestPaused` → `failRequest`): o interesse é a reação da aplicação,
  e a lista de URLs pedidas é por si um resultado (mostra até onde o código
  chegou);
* junta, por passo, `Runtime.exceptionThrown`, `Runtime.consoleAPICalled`,
  `Log.entryAdded` e o coletor da página, separando **erros da aplicação**,
  **ambiente** (404 de recurso, conexão recusada) e os **`print` da própria
  aplicação**, que são o rastro que se compara entre compiladores.

Modos de comparação:

| modo | comando | o que roda |
|------|---------|-----------|
| DartForge | `-Fluxo` | os módulos de `compile-js` (`main.mjs`) |
| `dartdevc` oficial | `--build <…/.dart_tool/build/generated>` | a saída do `build_web_compilers` do próprio projeto (um módulo DDC por biblioteca + `require.js` + `main.dart.bootstrap.js`), como no `webdev serve` |
| `dartdevc` módulo único | `--ddc` | `dartdevc --modules=es6` do programa inteiro num arquivo só |

    node scripts/fluxo.mjs --dir work/fe/out --web …/frontend/web --json work/fe/fluxo.json
    node scripts/fluxo.mjs --dir work/fe/ddc --web …/frontend/web \
        --build …/frontend/.dart_tool/build/generated --rotulo "dartdevc oficial"

## Fluxo verificado (2026-09-22)

| # | passo | DartForge | dartdevc oficial |
|---|-------|-----------|------------------|
| 1 | carga inicial `/` | `my-app` -> `router-outlet` -> `pre-login-page`, 378 chars | igual |
| 2 | clique no carrossel (Slide 3) | slide ativo 0 -> 2 | igual |
| 3 | `/login?error_code=GOVBR_BRONZE` | alerta com titulo e mensagem, 685 chars | igual |
| 4 | submeter "Entrar" | autorizacao OIDC com PKCE, bloqueada | igual |
| 5 | rota `/sobre` | `sobre-page` montada | igual |
| 6 | rota `/sessao-expirou` | `session-expired-comp` montada | igual |
| 7 | `/restrito/home` sem sessao | a guarda devolve a `/login` | igual |
| 8 | volta a `/login` | `pre-login-page` montada | igual |
| 9 | clique em link interno | a pagina publica nao tem `<a href="/...">` | igual |
| 10 | `/callback?code=invalido` | `POST /oauth2/token` e depois `/oauth2/logout` | igual |
| 11 | sessao forjada -> `/restrito/home` | pede os 5 endpoints da API e trata a conexao recusada | igual |

**11 passos, 0 erros da aplicacao nos dois compiladores, e os 11 passos batem
componente a componente** (mesmos componentes montados, mesmo tamanho de texto,
mesmos alertas, mesmos `print` da aplicacao). Os 7 pedidos externos bloqueados
sao exatamente os mesmos: `/oauth2/token`, `/oauth2/logout`,
`/api/v1/administracao/menu/12345`,
`/api/v1/administracao/permissoes/user/logado/todas`,
`/api/v1/administracao/usuario-preferencias/me`,
`/api/v1/protocolo/acoes/favoritas/cgm/12345`,
`/api/v1/protocolo/notificacoes/last/12345`.

Sobre o passo 4: a aplicacao nao tem formulario de usuario/senha — o login e
OIDC (servidor de identidade em `localhost:3350`) e o unico controle do
formulario e o botao `#submit`. "Credenciais invalidas" e o `error_code` que o
IdP devolve na query, que e o caminho do passo 3.

## O que foi corrigido no emissor (fila de 2026-09-22, fechada)

1. **`@JSName` na simbolizacao de membro nativo** (`_isSymbolizedMember`,
   compiler.dart:3311): membro nativo que e campo ou `external` so acede
   propriedade direta quando **nao** e null-checkable **e nao** foi renomeado;
   o nome vem da anotacao lida do outline (mapa por membro da hierarquia
   nativa), nao de lista. `Node.text` (`@JSName('textContent')`) volta a ser
   `dartx`, e com isso a interpolacao `{{...}}` do ngdart aparece no DOM e os
   estilos de componente sao aplicados. Corpus `211_membros_nativos_renomeados`
   (via `dart:_native_typed_data`: `lengthInBytes`/`offsetInBytes`/
   `elementSizeInBytes`), que roda na VM e no Node.
2. **Constantes de ambiente na compilacao**: `const bool/int/String.fromEnvironment`
   e `bool.hasEnvironment` sao avaliadas ao compilar (sem `-D`, valem o
   `defaultValue`) e o ramo morto de um `if` com condicao constante nao e
   emitido, como o dartdevc faz. Com isso `package:http` cria o `BrowserClient`
   e as chamadas HTTP saem. Corpus `210_constantes_de_ambiente`.
3. **Interop de `dart:js_interop`** (revelada pela correcao 2, que destravou o
   caminho HTTP): tipos de extensao de interop (package:web, `dart:js_interop`)
   deixam de ser apagados no tipo — construtor `external` vira
   `new dart.global.X(...)`, construtor so com nomeados vira literal de objeto
   (compiler.dart:6948) e os membros `external` viram propriedade direta —,
   enquanto nas **receitas rti** o apagamento passa a ser para o tipo de
   representacao (`JSArray<JSString>` -> `_interceptors|JSArray<core|Object?>`,
   `JSString` -> `core|String`), que e o que o DDC emite. A interop e calculada
   antes da hierarquia, para `implements JSAny` valer nas extensoes do SDK
   (`toDart`, `toJS`, `jsify`, `dartify`). Corpus
   `213_interop_de_tipos_de_extensao` (`diverge-ddc`).
4. **`rti` de classe com superclasse generica**: a factory recebe `_ti` sempre
   que a classe *ou uma superclasse* e generica (antes so com parametros
   proprios) e o tearoff estatico passa o rti ao construtor — era o que
   quebrava `ByteStream.fromBytes` do `package:http`. Corpus
   `212_rti_de_superclasse_generica`.

Harness apos as correcoes: **212/212**.

## Achados de ambiente/programa (não são do emissor)

* **`/style.css` 404**: `web/index.html:172` referencia `style.css`, que não
  existe em `web/` (sai do pipeline de SCSS do `build_runner`). Acontece com os
  dois compiladores.
* **`/packages/build_web_compilers/src/dev_compiler_stack_trace/stack_trace_mapper.dart.js` 404**
  no modo `--build`: o mapeador de pilha não está na saída gerada; é só
  diagnóstico do DDC.
* **Servidor de identidade e API ausentes** (`localhost:3350`, `ws://localhost:3351`):
  o passo 4 confere o *pedido* de autorização, e o passo 11 confere que a
  aplicação trata a conexão recusada. Com a API no ar, o passo 11 renderizaria o
  shell autenticado — é o próximo salto de cobertura.
* **Rotas restritas** exigem sessão *e* permissões da API; a guarda
  (`AuthGuard.canActivate`) espera `permissionsReady` com watchdog de 20 s e,
  sem backend, cancela a navegação nos dois compiladores.

## Inferência de tipos contra o oráculo (2026-09-23)

O `dart analyze` oficial dá 0 diagnósticos no `new_sali`: todo aviso da
nossa inferência (`crates/types`) é um tipo que o analyzer infere e nós não.
A medição compara, expressão a expressão, o tipo estático nosso com o do
`package:analyzer` 6.11 (o que o próprio `new_sali` resolve):

    # nosso despejo (ordenado por arquivo/offset/comprimento, offsets UTF-16)
    cargo run --release -p dartforge-types --example despejo_tipos --         C:/MyDartProjects/new_sali/frontend/web/main.dart         --packages C:/MyDartProjects/new_sali/frontend/.dart_tool/package_config.json         -o fe.tsv --arquivos fe_arquivos.txt --avisos fe_avisos.txt
    # oráculo: compilado uma vez (não recompila o analyzer a cada rodada)
    dart compile exe --packages=C:/MyDartProjects/new_sali/frontend/.dart_tool/package_config.json         tools/oraculo_tipos/oraculo.dart -o oraculo.exe
    oraculo.exe C:/MyDartProjects/new_sali/frontend fe_arquivos.txt fe_oraculo.tsv
    # comparação em fluxo, agrupada pela causa
    cargo run --release -p dartforge-types --example comparar_tipos -- fe.tsv fe_oraculo.tsv

Uma divergência é **causa** quando a expressão diverge e nenhuma divergência
está estritamente contida nela (a divergência começa ali; as que a contêm são
cascata). Entradas: core = `test/arvore_processo_item_test.dart` (2.136
unidades, 634 mil expressões comparáveis), frontend = `web/main.dart` (3.183
unidades, 1,26 milhão).

**Memória das ferramentas** (pico do working set, medido no core): o
comparador antigo em Python carregava os dois despejos inteiros — **949 MB**
no core (1,6–2 GB no frontend, visto no Gerenciador de Tarefas); o
`comparar_tipos` faz merge-join arquivo a arquivo sobre os dois despejos
ordenados pela mesma chave e fica em **45 MB** (2,6 s contra 7,4 s), com o
mesmo agrupamento (os 19 grupos e as contagens idênticos; só o `` dos
trechos de exemplo muda). O oráculo rodado pelo `dart` sobre a fonte
compilava o analyzer a cada vez e guardava tudo o que resolvia: **1.206 MB**
no core; compilado com `dart compile exe`, com os resumos num cache em disco
(32 MB em memória) e o modelo de elementos limpo a cada 150 unidades, fica em
**427–477 MB** no core e **827 MB** no frontend, com a saída byte a byte igual.

| passo (commit) | avisos core | avisos frontend | divergentes core | divergentes frontend |
|---|---:|---:|---:|---:|
| antes (infer.rs) | 13.431 | 57.883 | 137.428 | 316.756 |
| motor reescrito pela especificação (`inferencia/`) | 1.657 | 9.319 | 3.396 | 8.372 |
| tipo cru do outline instanciado para os limites | 1.093 | 8.312 | 1.625 | 5.876 |
| setter implícito de `late final` sem inicializador | 1.079 | 1.322 | 1.625 | 2.454 |
| tear-off genérico no alvo, `C.nome()`, `?.`, `$this`, campos de extensão | 977 | 1.129 | 646 | 1.096 |
| parte de arquivo de patch é patch (`BigInt` duplicada) — `crates/elements` | 916 | 1.003 | 480 | 743 |
| `?` do parâmetro-função da forma antiga — `crates/frontend` | 860 | 963 | 436 | 569 |
| sobreposição pela posição, extensão genérica, `-1`, closure | 799 | 883 | 314 | 389 |
| conflito de import `dart:` × pacote — `crates/elements` | 783 | 862 | 232 | 279 |
| variável de tipo promovida `X & B` (`Type::Intersection`) | 724 | 783 | 197 | 248 |

SDK compilado da fonte pelo backend nativo (`infer_bodies_das_bibliotecas`
com as sete bibliotecas, sobreposição `vm`): **4.447 → 275** diagnósticos
(`core` 193, `convert` 37, `async` 26, `_internal` 7, `collection` 5,
`_compact_hash` 4, `math` 3).

Grupos restantes no new_sali (causas; frontend / core):

| grupo | causas | situação |
|---|---:|---|
| nulabilidade de local/campo depois de promoção de campo privado final (`_matcher`, `_el`: promoção de campo, Dart 3.2) | 69 / 45 | a fazer: promoção de `this._x` |
| tipo mais específico por promoção de campo (`_el` → `InputElement`) | 48 / 40 | idem |
| inteiro em contexto `double` dentro de argumentos genéricos (`color.dart`) | 14 / 14 | a investigar |
| tear-off genérico cujos parâmetros de tipo o analyzer renomeia (`T₀`) | 10 / – | exibição: o analyzer renomeia parâmetros que colidem |
| `runZoned`, `compareTo` em receptor de variável de tipo promovida | 7 / 7 | a investigar |
| atribuição `a?.b = null` (tipo `Null?`) | 7 / – | null-shorting em atribuição |
| construtor de fábrica genérico redirecionado (`Stream.empty()`) | 5 / 5 | inferência pelo contexto de construtor redirecionador |

### Observação arquitetural: o `emit_js` tem inferência própria

O emissor JavaScript (`crates/emit_js`: `ctx.rs` `ty_of`/`lookup_member`/
`lub`, `expr.rs` `infer_elements_ty`…) **não lê** os tipos de corpo do
`crates/types` (`BodyTypes`): ele reconstrói, durante a emissão, a sua própria
inferência paralela sobre a AST. Quem consome `BodyTypes` hoje é o mundo
fechado da produção (`crates/mundo`), o backend nativo e o LSP. Consequência:
os ganhos desta frente não mudam o JS de desenvolvimento (tamanho e despacho
dinâmico continuam decididos pela inferência do emissor), e há duas
inferências a manter coerentes. A unificação — o `emit_js` consumindo
`BodyTypes` — fica para depois de os avisos zerarem.
