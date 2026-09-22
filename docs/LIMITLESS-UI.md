# limitless_ui: o exemplo ngdart com suíte de comportamento

Alvo: `references/limitless_ui` (ngdart 8.0.0-dev.4, ngforms 5.0.0-dev.3,
ngrouter 4.0.0-dev.3, `dart:html`, `package:js`, `dart:js_util`), cuja
`example/` é uma galeria com 24 componentes (datatable, pickers, carousel,
treeview, select, overlays, editor Quill, visualizador de PDF…) e — o que a
torna melhor alvo que o `new_sali/frontend` — traz **suíte e2e de
comportamento** em `ui_test/e2e/`: testes Puppeteer que dirigem um navegador
real contra `UI_EXAMPLE_BASE_URL` e afirmam o que a aplicação **faz** (clicar
num item do `li-select` muda o `data-value`; o painel aberto recebe mesmo o
clique em `elementFromPoint`; a tabela **para** de se redesenhar).

O critério de aceite é direto: o mesmo suite que passa contra o build oficial
tem de passar contra a nossa saída.

## Estado (2026-09-22)

| lado | JavaScript servido | e2e | sonda de rotas |
|------|--------------------|-----|----------------|
| oficial | `build_web_compilers --release` (dart2js), um `main.dart.js` de 4,5 MB | **26/26** | **52/53** |
| DartForge | `dartforge compile-js`, 483 módulos ES6 + `dart_sdk.js` do DDC | **26/26** | **52/53** |

A aplicação compilou, subiu e passou nos 26 testes na primeira execução. A
suíte, porém, não cobre tudo: uma sonda própria percorreu as **53 rotas** da
galeria recolhendo `window.onerror`, `unhandledrejection`, `console.error` e
as exceções do runtime do Dart, e achou **uma falha real** que nenhuma
asserção pegava (`/person-registration`) — corrigida, ver
"[A falha achada](#a-falha-achada-tipo-cru-na-receita-rti)". Depois da
correção a sonda dá o mesmo resultado dos dois lados: 52/53, com a única
reprovação sendo `/target-alert`, que **também reprova no oficial** porque a
página não tem `.demo-page`/`.content` (o seletor de montagem da sonda é que
não serve para ela), e portanto não conta contra nós.

Compilação (Windows, 8 GB, release): 782 unidades de usuário/pacotes + 172 do
SDK, 781 bibliotecas, **483 módulos**, ~10,5 s de compilação (carga 4,2 s;
outline 0,5 s; corpos 0,7 s; emissão 4,2 s) e 233 s de **escrita** em disco —
a escrita domina e é I/O, não compilador. Recompilar depois da correção
reescreveu 58 dos 484 arquivos.

## Preparar

`references/` é ignorada pelo git; os comandos abaixo rodam sobre o clone que
já está lá. O cache do SDK grava em `target/dartforge/` relativo ao cwd quando
o binário está fora de `target/`: os scripts apontam `DARTFORGE_CACHE_DIR` para
`work/lui/cache` para não sujar o repositório.

```powershell
pwsh scripts/limitless-ui.ps1 -Preparar
```

Isso faz, em ordem:

1. `dart pub get` em `references/limitless_ui` e em `.../example`;
2. `dart run build_runner build --delete-conflicting-outputs` no `example` —
   gera os `.template.dart` do ngdart em
   `example/.dart_tool/build/generated/<pacote>/lib/…`, que o nosso carregador
   sobrepõe ao `lib/` do pacote (`PackageConfig::generated_root`,
   `crates/elements/src/config.rs`). São 290 templates: 166 do
   `limitless_ui_example` e 124 do `limitless_ui` (dependência por `path`, cujos
   gerados caem no `.dart_tool` do **example**, não no do pacote);
3. `dart run build_runner build --release --output web:build` — o build oficial
   (dart2js) em `example/build`, que é o teto da comparação.

**Não rodar `compile-js` enquanto o `build_runner` roda**: o
`--delete-conflicting-outputs` apaga a árvore inteira de gerados antes de
reconstruir, e no meio disso os `.template.dart` não existem — o carregador
cai no `lib/` do pacote e falha com dezenas de "não foi possível ler
…/x.template.dart". Não é bug nosso; é corrida com o build_runner.

## Rodar os dois lados

O servidor é o **mesmo** nos dois casos — `tool/serve_example.dart` do próprio
projeto, que carrega tudo para a memória já comprimido —, para que a única
diferença entre as medições seja o JavaScript servido. A aplicação usa rotas
por hash (`#/rota`), então não é preciso fallback de SPA.

```powershell
# oficial na 8081
pwsh scripts/limitless-ui.ps1 -Servir oficial -Porta 8081

# DartForge: compila, monta o diretório e serve na 8082
pwsh scripts/limitless-ui.ps1 -Montar
pwsh scripts/limitless-ui.ps1 -Servir dartforge -Porta 8082

# a suíte, contra qualquer um dos dois
pwsh scripts/limitless-ui.ps1 -E2e -Porta 8081
pwsh scripts/limitless-ui.ps1 -E2e -Porta 8082
```

`-Montar` roda

```
dartforge compile-js example/web/main.dart -o work/lui/out \
    --packages example/.dart_tool/package_config.json
```

e depois monta o diretório servido: troca o `<script defer src="main.dart.js">`
do `index.html` por `<script type="module" src="main.mjs">`, injeta o coletor
`window.__erros` (`onerror` + `unhandledrejection` + `console.error`) e copia os
estáticos do projeto (`assets/`, `scrollbar.css`, `themes/`) mais o `style.css`
que o `sass_builder` gera do `style.scss`.

`-E2e` exporta `RUN_EXAMPLE_E2E=true` (sem ele os testes se declaram `skip`),
`UI_EXAMPLE_BASE_URL`, `UI_HEADLESS=true` e `PUPPETEER_EXECUTABLE_PATH` para o
Chrome instalado — sem isso o `package:puppeteer` baixa um Chromium próprio na
primeira execução. A suíte abre um navegador por teste, então `-j 1`.

O servidor lê os arquivos **uma vez**, para a memória: depois de recompilar é
preciso reiniciá-lo, senão ele continua servindo o JavaScript antigo.

## A suíte, por arquivo

São 26 testes em três arquivos (o de overlays gera 5 deles num laço, um por
contêiner hospedeiro).

| arquivo | testes | oficial | DartForge | o que exercita |
|---------|-------:|--------:|----------:|----------------|
| `ui_test/e2e/datatable_layout_loop_test.dart` | 3 | 3 | 3 | laço infinito de layout do `li-datatable` com zoom em 110%: os contadores de redesenho **param** de subir |
| `ui_test/e2e/overlay_layers_test.dart` | 9 | 9 | 9 | escala de camadas; 19 overlays ancorados × 5 contêineres (página, modal, modal empilhado, offcanvas, modal sobre offcanvas) recebendo o clique por `elementFromPoint`; diálogo/alerta/toast acima do modal |
| `ui_test/e2e/puppeteer_test.dart` | 14 | 14 | 14 | select, multi-select, datatable-select, tags, token field, date/date-range/time pickers (desktop e mobile fullscreen), typeahead, treeview, rating, dropdown menus, checkbox/radio/toggle/slider/color picker, SweetAlert |
| **total** | **26** | **26** | **26** | |

Tempos: 3m33s no oficial, 4m11s na nossa saída (abrir um navegador por teste
domina; a diferença é o carregamento de 483 módulos contra um arquivo só).

## A falha achada: tipo cru na receita rti

A suíte passava inteira e ainda assim `/person-registration` estava quebrada:
nenhum dos 26 testes visita essa rota. A sonda de rotas pegou

```
Assertion failed: rti.dart:3676  length == _Utils.arrayLength(tArgs)
  _areArgumentsSubtypes → _isInterfaceSubtype → _isSubtype → _isFunctionSubtype
  [dartx.add] ← Validators._removeNullValidators ← Validators.compose
  ← setUpControl ← NgModel.ngOnInit
```

O ngforms tem

```dart
abstract class AbstractControl<T> { … }                     // genérica
typedef ValidatorFn = Map<String, dynamic>? Function(AbstractControl c);  // crua!
static List<T> _removeNullValidators<T>(List<T?> validators) { … r.add(v) … }
```

`Validators.compose` chama `_removeNullValidators` com `T` inferido como
`ValidatorFn`, e o `add` de dentro faz um `as T` contra a receita desse tipo
função. Nós emitíamos o `AbstractControl` **cru**, sem argumentos:

```js
// errado
"core|Map<core|String,@>?(ngforms__src__model|AbstractControl)"
// certo (o que o dartdevc emite)
"core|Map<core|String,@>?(ngforms__src__model|AbstractControl<@>)"
```

Regra, conferida no oráculo (`dartdevc --modules=es6` sobre o programa mínimo
do corpus): **uma receita de tipo interface traz sempre um argumento por
parâmetro declarado** — um tipo cru é `C<@,…>`, nunca `C`. O nome sem
argumentos só é legítimo como **chave** de `addRules` e no `addRtiResources`,
que não são receitas de tipo. O `rti` confere a aridade em
`_areArgumentsSubtypes` e a receita crua derruba o primeiro `is`/`as` que a
atravesse — o que aqui só acontecia numa página que a suíte não abre.

Correção em `crates/emit_js`, nas quatro ocorrências do mesmo problema:

* `Ctx::args_na_aridade` / `Ctx::class_arity` (`ctx.rs`) — o preenchimento com
  `dynamic`, num lugar só;
* `Ctx::ty_of` (`ctx.rs`), nos ramos `Type::Interface` e `Type::ExtensionType`
  de interop — a porta de entrada dos tipos da `TypeTable`, para que receitas,
  regras, substituições e busca de membros vejam a mesma forma;
* `FnEmitter::recipe` (`body.rs`) e `rule_recipe` (`module.rs`) — os dois
  emissores de receita, como última linha de defesa para os `Ty::iface(c)`
  construídos sem contexto;
* `emit_rules` (`module.rs`) — o vetor de argumentos do supertipo no
  `addRules` tem a mesma exigência: `implements Caixa` vale
  `"c|Caixa":["@"]` mais a entrada `"Caixa.T":"@"`, e antes saía `[]` sem
  entrada nenhuma (segundo defeito, latente).

Corpus: `corpus/js/214_tipo_cru_generico.dart` (tipo cru em typedef, em
supertipo, em campo, em `List`/`Map`, em `is`/`as` e inferido como argumento de
tipo de uma função genérica).

## Sonda de rotas

`scripts/limitless-ui.ps1` não cobre isto; a sonda foi um programa Puppeteer de
uso único que visita as 53 rotas de
`example/lib/src/shared/routes/route_paths.dart`, espera a montagem
(`.demo-page, .content`) e recolhe o que o coletor da página juntou. Vale
manter o hábito: foi ela, e não a suíte, que achou o defeito acima. Rodar
sempre **contra os dois lados** — `/target-alert` "reprova" nos dois e é só o
seletor de montagem que não serve para aquela página.

## O que o exemplo exercita do emissor

O que faz deste alvo um teste do compilador, e não só do runtime:

* **ngdart**: os `.template.dart` gerados são código Dart denso — classes de
  view com centenas de campos, `NgFactory` estáticas, `ComponentView<T>`
  genéricas, ciclos de import entre bibliotecas do mesmo pacote (que o emissor
  funde num módulo só) e `dart:html` em cada linha;
* **`dart:html`**: 115 imports. Todo o tratamento de membros de receptor nativo
  (`_isSymbolizedMember`: propriedade JS direta vs. símbolo `dartx`,
  `@JSName`, `checkNativeNonNull`) está em uso o tempo todo;
* **interop**: `dart:js_util` (`callMethod`, `getProperty`, `jsify`,
  `promiseToFuture`) no exportador do datatable e no dropdown; `package:js`
  (`@JS`, `@anonymous`) nas ligações do PDF.js;
* **assíncrono**: 87 imports de `dart:async`, `Stream`/`StreamController` em
  todo componente, `async`/`await` e o `Zone` do ngdart;
* **pacotes**: `essential_core`, `popper`, `intl`, `i18n`, `built_value`,
  `collection` — 481 dos 483 módulos são de pacote (política de um módulo por
  pacote do pub cache, um por biblioteca do projeto).

## O que falta

* **Tamanho**: a soma dos módulos é 48 MB contra 4,5 MB de um `main.dart.js` do
  dart2js. É esperado — o nosso é saída de **desenvolvimento**, um módulo por
  biblioteca sem tree shaking, e o oficial é release com programa inteiro. A
  comparação justa de tamanho é contra o `build_web_compilers` em modo debug
  (DDC), não contra o dart2js; medir isso continua pendente.
* **Escrita em disco**: 233 s para 484 arquivos domina o tempo de ponta a
  ponta. É I/O, não compilação (a compilação inteira são 10,5 s), mas torna o
  ciclo inviável sem o `dartforge dev`, que só regrava os módulos cujo texto
  mudou — a recompilação depois da correção reescreveu 58 arquivos e foi rápida.
* **74.332 avisos de inferência de corpos** (`crates/types`, de outro agente).
  Cada um é um ponto onde o emissor não sabe o tipo estático e recua para
  `dart.dsend`/`dload`/`dput` — correto, mas mais lento e maior que o
  necessário. Pedido detalhado (os quatro grupos, por frequência) registrado em
  [EMISSAO-DDC.md](EMISSAO-DDC.md).
* A sonda cobre a montagem de cada rota, não a interação em cada uma. O
  `pdf-viewer` e o `quill-text-editor` dependem de bibliotecas JS carregadas
  pelo `index.html` (`pdf.js`, `quill`) e só foram exercitados pela sonda — a
  suíte oficial também não os cobre.
* A sonda ainda não está no `scripts/limitless-ui.ps1`; transformá-la num modo
  do script (`-Rotas`) é o próximo passo óbvio, porque o valor dela já se
  provou.
