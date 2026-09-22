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
| 1 | carga inicial `/` | monta `my-app` → `router-outlet` → `pre-login-page` | igual |
| 2 | clique no carrossel (Slide 3) | slide ativo 0 → 2 | igual |
| 3 | `/login?error_code=GOVBR_BRONZE` | alerta renderizado, **mas os dois `{{…}}` saem vazios** | alerta com título e mensagem |
| 4 | submeter "Entrar" | pedido de autorização OIDC com PKCE, bloqueado | igual |
| 5 | rota `/sobre` | `sobre-page` montada | igual |
| 6 | rota `/sessao-expirou` | `session-expired-comp` montada | igual |
| 7 | `/restrito/home` sem sessão | a guarda devolve a `/login` | igual |
| 8 | volta a `/login` | `pre-login-page` montada | igual |
| 9 | clique em link interno | a página pública não tem `<a href="/…">` | igual |
| 10 | `/callback?code=inválido` | tenta **só** `POST /oauth2/logout` | tenta `POST /oauth2/token` e depois `/oauth2/logout` |
| 11 | sessão forjada → `/restrito/home` | `HomePage@ngOnInit` roda, mas os componentes falham com `Unsupported operation: bool.fromEnvironment…`; **nenhuma chamada à API** | mesmos componentes pedem `…/administracao/menu/12345`, `…/permissoes/user/logado/todas`, `…/usuario-preferencias/me`, `…/acoes/favoritas/cgm/12345` e tratam a conexão recusada |

Nos dois compiladores: **11 passos, 0 erros não tratados**. As divergências
estão no que a aplicação consegue fazer, não em exceções soltas.

Sobre o passo 4: a aplicação não tem formulário de usuário/senha — o login é
OIDC (servidor de identidade em `localhost:3350`) e o único controle do
formulário é o botão `#submit`. "Credenciais inválidas" é o `error_code` que o
IdP devolve na query, que é o caminho do passo 3.

## Fila para o emissor (revelada pelo fluxo, não corrigida)

### 1. `const bool.fromEnvironment` avaliado em tempo de execução

* **Construto**: `package:http/src/browser_client.dart:28` (e
  `io_client.dart:15`): `if (const bool.fromEnvironment('no_default_http_client')) { throw StateError(…); }`
* **dartdevc** (`.dart_tool/build/generated/http/lib/http.ddc.js:1389`): avalia a
  constante de ambiente na compilação (ausente → `false`) e some com o `if`:
  `createClient = function createClient() { ; return new browser_client.BrowserClient.new(); };`
* **DartForge**: emite `if (dart.const(core.bool.fromEnvironment("no_default_http_client")))`,
  e o `dart_sdk.js` lança de propósito nesse construtor
  (`bool.fromEnvironment can only be used as a const constructor`, dart_sdk.js:125824).
* **Efeito**: `createClient()` sempre lança → **nenhuma chamada HTTP acontece**;
  `MainMenuComponent@loadMenus`, `ListNotificationComp@load`,
  `AcaoFavoritaComp@loadFavoritos` e a troca de código do OIDC morrem aí.
* **O que falta**: avaliar `const bool/int/String.fromEnvironment` e
  `bool.hasEnvironment` na compilação (sem `-D`, o valor é o `defaultValue`), como
  a CFE faz, em vez de emitir a chamada.
* **Reprodução mínima** (sem `dart:html`; `dart run` imprime
  `false/true/7/padrão/false`, o DartForge lança na primeira linha) — vira
  programa de corpus junto com a correção:

      const bool depuracao = bool.fromEnvironment('modo_depuracao');
      void main() {
        if (const bool.fromEnvironment('sem_cliente')) print('nunca');
        print(depuracao);
        print(const bool.fromEnvironment('x', defaultValue: true));
        print(const int.fromEnvironment('n', defaultValue: 7));
        print(const String.fromEnvironment('s', defaultValue: 'padrão'));
        print(const bool.hasEnvironment('nao_existe'));
      }

### 2. `@JSName` em membro nativo é ignorado (nome JS errado)

* **Construto**: `package:ngdart/src/runtime/text_binding.dart:28`
  (`element.text = newValue`, onde `element` é um `Text` de `dart:html`) e
  `package:ngdart/src/core/linker/style_encapsulation.dart:170`
  (`final styleElement = StyleElement()..text = styles;`).
* **Regra na referência**: `compiler.dart:3311` `_isSymbolizedMember` — num
  receptor nativo, o membro encaminhado que é campo ou `external` só escapa do
  símbolo `dartx` quando **não** é null-checkable **e não é renomeado**:

      if (_isNullCheckableNative(member!)) return true;
      var jsName = _annotationName(member, isJSName);
      return jsName != null && jsName != name;   // renomeado → simbolizado

  `Node.text` é `external` com `@JSName('textContent')`
  (`html_dart2js.dart:23486/23489`), portanto **simbolizado**; o `dart_sdk.js`
  implementa `get [S.$text]() { return this.textContent; }` e
  `set [S.$text](value) { this.textContent = value; }` (dart_sdk.js:66597-66602).
* **DartForge**: `Ctx::is_ext_member` (patch da regra `_isSymbolizedMember`)
  ainda não olha o `@JSName`, então emite `this.element.text = newValue` —
  escreve numa propriedade JS inexistente, sem erro.
* **Efeito**: toda interpolação `{{…}}` do ngdart é silenciosamente perdida
  (passo 3) e os estilos dos componentes não são aplicados (`StyleElement.text`).
  `dart:html` tem **614** membros com `@JSName`, então a classe de falhas é
  ampla (`innerHtml`, `_getContext`, `_toDataUrl`…).
* **O que falta**: no cálculo de `is_ext_member`, simbolizar quando o membro
  nativo tem `@JSName('x')` com `x != nome`.
* Programa mínimo de corpus: precisa de `dart:html` (o harness roda em Node);
  dá para cobrir com um `@Native`/`@JSName` próprio num programa de teste.

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
