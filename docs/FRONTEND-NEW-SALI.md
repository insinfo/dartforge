# new_sali/frontend no navegador: verificação de fluxo real

Alvo: `C:/MyDartProjects/new_sali/frontend` (ngdart 8.0.0-dev.4, `dart:html`,
`package:js`, OIDC), entrada `web/main.dart`, `package_config.json` em
`frontend/.dart_tool/`. Os `.template.dart` gerados pelo `build_runner` ficam em
`.dart_tool/build/generated/<pacote>/lib/…` e o carregador os sobrepõe ao `lib/`
(`PackageConfig::generated_path`).

    work/bin/dartforge.exe compile-js C:/MyDartProjects/new_sali/frontend/web/main.dart \
        -o work/fe/out --packages C:/MyDartProjects/new_sali/frontend/.dart_tool/package_config.json
    pwsh scripts/servir.ps1 -Dir work/fe/out -Web C:/MyDartProjects/new_sali/frontend/web -Fluxo

Estado: **616 módulos emitidos, `node --check` 616/616**, e o fluxo abaixo
percorrido no Edge headless **sem nenhuma exceção da aplicação**.

## O verificador de fluxo (`scripts/servir.ps1 -Fluxo` → `scripts/fluxo.mjs`)

`servir.ps1` continua servindo o diretório e despejando o DOM (`-Headless`);
com `-Fluxo` ele delega ao `scripts/fluxo.mjs` (Node, sem dependências — o
Node 22+ tem `WebSocket` global), que:

* serve o diretório emitido com **fallback de SPA** (rota desconhecida devolve o
  `index.html`), necessário porque o ngdart usa `routerProviders` com
  `<base href="/">` — um servidor estático simples devolveria 404 em `/sobre`;
* injeta no `index.html` o coletor `window.onerror` + `unhandledrejection` +
  `console.error` e troca o `main.dart.js` pelo `main.mjs` emitido (`-Ddc` usa
  `import { main } from './main.js'`, a saída do `dartdevc`);
* abre o Edge com `--headless=new --remote-debugging-port` e conduz os passos
  pelo **CDP** por WebSocket (`Page.navigate`, `Runtime.evaluate`, cliques com
  `userGesture`), esperando a aplicação montar entre um passo e outro;
* **bloqueia e registra** as navegações para fora do servidor local
  (`Fetch.requestPaused` → `failRequest`): o interesse é a reação da aplicação,
  não o servidor de identidade;
* junta, por passo, `Runtime.exceptionThrown`, `Runtime.consoleAPICalled`
  (error/warning), `Log.entryAdded` e o coletor da página, separando os 404 de
  recurso (ambiente) dos erros da aplicação.

Saída: relatório por passo (rota, componentes montados, botões/entradas/alertas
encontrados, erros) no terminal e, com `-Json`, o mesmo em JSON.

## Fluxo verificado (DartForge, 2026-09-22)

| # | passo | o que se afirma | resultado |
|---|-------|-----------------|-----------|
| 1 | carga inicial `/` | monta `my-app` → `router-outlet` → rota padrão | `pre-login-page` montada, 593 chars de texto |
| 2 | clique no carrossel (Slide 3) | interop com o Bootstrap via `dart:html` reage | slide ativo 0 → 2 |
| 3 | `/login?error_code=GOVBR_BRONZE` | `onActivate` + `queryParameters` + `*ngIf` renderizam o alerta | alerta "Elevar nível da conta Gov.br: confiabilidades.acesso.gov.br" |
| 4 | submeter "Entrar" | `doLogin()` → `OidcService.login()` monta o pedido de autorização | 1 navegação externa bloqueada: `http://localhost:3350/oauth2/authorize?response_type=code&client_id=sali-front-end&redirect_uri=…` **com `code_challenge` (PKCE)** |
| 5 | rota `/sobre` | o router troca o componente | `sobre-page` montada ("Sobre o SALI", "Acesso e Contato") |
| 6 | rota `/sessao-expirou` | idem | `session-expired-comp` montada |
| 7 | rota restrita `/restrito/home` sem sessão | a guarda devolve ao login | termina em `/login` com `pre-login-page` |
| 8 | volta a `/login` | reentrada na rota | `pre-login-page` montada |
| 9 | clique em link interno | navegação pelo router sem recarregar | a página pública não tem `<a href="/…">` (só links externos); nada a clicar |

**9 passos, 0 com erro da aplicação.** Nenhum `onerror`, nenhuma rejeição não
tratada, nenhuma exceção do runtime do Dart, nenhum `console.error`.

Sobre o passo 4: a aplicação não tem formulário de usuário/senha — o login é
OIDC (Gov.br / servidor de identidade em `localhost:3350`), e o único controle
do formulário é o botão `#submit`. "Credenciais inválidas" é, portanto, o
`error_code` que o IdP devolve na query (passo 3), que é o caminho que a
aplicação de fato trata e o que o passo 3 afirma.

## Fila para o emissor

Nada. O fluxo não revelou nenhuma exceção nem divergência atribuível ao
DartForge. Quando algum passo falhar, o formato desta fila é: construto Dart
(`arquivo:linha`), forma que o `dartdevc` emite, forma que emitimos.

## Achados de ambiente/programa (não são do emissor)

* **`/style.css` 404**: `web/index.html:172` referencia `style.css`, que não
  existe em `web/` (é produzido pelo pipeline de SCSS do `build_runner`). O
  mesmo 404 aparece com qualquer compilador; contado à parte, em "ambiente".
* **Servidor de identidade ausente**: `localhost:3350` não está no ar no
  ambiente de verificação, então o passo 4 confere o *pedido* de autorização, e
  não o retorno do IdP. O callback (`/callback`) precisa de um `code` válido e
  fica fora do fluxo automatizado.
* **Rotas restritas** exigem sessão; a verificação cobre a guarda (passo 7), não
  as telas internas.
