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
