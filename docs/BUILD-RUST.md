# `dartforge build` — o motor de geração de código em Rust

Plano para substituir o `build_runner` no ciclo de desenvolvimento, sem
quebrar o ecossistema. Escrito em 2026-09-22 a partir de medição dos
projetos reais do proprietário, não de suposição.

## 0. Onde está (2026-09-24)

A Fase 1 existe: `crates/build` (contrato em `docs/BUILD-MOTOR.md`,
protocolo do executor Dart em `docs/BUILD-PROTOCOLO.md`), ligado ao
`dartforge dev`/`serve`/`compile-js` e ao subcomando `dartforge build`.
Números medidos no ESTADO.md §1.8. Em uma linha por passo do plano:

* **B0** — contratos escritos (BUILD-MOTOR, BUILD-PROTOCOLO).
* **B1** — `corpus/builders/`: 10 casos com o oráculo do `build_runner`
  oficial (2.4.15), `scripts/corpus-builders.ps1`; `go_router_builder` fora
  (exige Flutter, D-B4).
* **B2** — o plano de fases do `build_runner` em Rust: **igual** ao
  `.dart_tool/build/entrypoint/build.dart` nos 9 casos com builders, no
  `new_sali/frontend` e no `limitless_ui/example` (lidos, nunca gerados).
* **B3** — grafo de saídas, consultas, impressão digital, agenda
  determinística, executores (nativo, Dart injetável com cliente `dfexec/1`,
  apoio; o processo Dart real ainda não existe): toda saída
  dos manifestos é prevista pelo plano (menos a de um pós-processador), 0
  diferentes, determinismo 1/4/8 e incremental = do zero nas edições.
* **B4** — sessão, `serve`, `compile-js` (o `emit_js` não depende mais do
  `gerador_ng`), `dartforge build`, custo zero com os três portões.
* **B5** — ngdart nativo no estágio A (ação de pacote). Sass nativo publica
  `compressed` sem mapas, após medição byte a byte; registra os módulos
  importados na ação e recusa `expanded` e mapas de fonte até que sejam
  reproduzidos byte a byte. Saídas CSS pedidas sob demanda usam o gerador
  nativo elegível antes do apoio.
* **B6** — o estágio B do ngdart depende do que está pedido ao `gerador_ng`
  em `docs/BUILD-PEDIDOS-GERADOR-NG.md`.

## 1. O que o `build_runner` custa hoje, medido

`new_sali/frontend` (ngdart 8.0.0-dev.4, 284 arquivos Dart):

```
dart run build_runner build --delete-conflicting-outputs
[INFO] Succeeded after 2m 59s with 9879 outputs (23569 actions)
.dart_tool/build = 205 MB
```

Composição dos 9.879 artefatos gerados:

| Artefato | Quantidade | Quem gera | Precisamos? |
| --- | ---: | --- | --- |
| `*.ddc.js` / `*.ddc.dill` | 934 | `build_web_compilers` | **não** — é o compilador que estamos substituindo |
| `*.template.dart` | 773 | `ngdart` | sim, até termos o compilador de templates |
| `*.css` (de `.scss`) | 192 | `sass_builder` | sim |
| resto (módulos, metadados, cópias) | ~7.980 | `build_web_compilers` e bookkeeping | **não** |

**A maior parte do tempo e do disco é do `build_web_compilers`** — ele
compila o projeto inteiro para JavaScript com o DDC, que é exatamente o
que o `dartforge compile-js` já faz (616 módulos, 10 s). Ou seja: no
nosso fluxo, ~80% do trabalho do `build_runner` **desaparece por não ser
executado**, não por ser reescrito.

Os builders que sobram, no fecho dos dois projetos reais
(`new_sali/frontend` e `limitless_ui/example`), são **três**:

```
ngdart        1 factory   compila @Component → *.template.dart
i18n          1 factory   mensagens traduzidas
sass_builder  1 factory   .scss → .css
```

Esses dois projetos não usam `json_serializable`, `freezed`, `drift` nem
`mockito` — mas isso **não** autoriza a deixar o ecossistema de fora.

> **Regra governante (proprietário, 2026-09-22): tudo nosso, compatível
> com o ecossistema.** O compilador, o runtime, o analisador e o LSP são
> nossos — a VM oficial **não faz parte do produto**. E têm de ser
> compatíveis com o ecossistema: `json_serializable`, `freezed`, `drift`,
> `mockito`, `source_gen`, `build`, `analyzer` e os demais pacotes que os
> projetos reais usam. Um projeto que compila com a toolchain oficial tem
> de compilar com a nossa. Não construímos um fork incompatível, e também
> não dependemos da implementação oficial para funcionar.

O `dart` oficial continua tendo **um** papel, e só ele: **oráculo de
verificação**, como `dart run` é hoje o oráculo do compilador JavaScript
(213 programas comparados byte a byte). Oráculo é o que se compara, não o
que se embute.

Isso torna o alvo maior e o torna explícito: **rodar um builder do
ecossistema é compilar e executar esse builder com a nossa pilha**. O que
isso exige, medido nos pacotes instalados nesta máquina:

| Pacote | Arquivos | Linhas | O que puxa |
| --- | ---: | ---: | --- |
| `package:build` | 21 | 1.904 | `dart:async`, `dart:convert` |
| `package:source_gen` | 14 | 2.116 | `build`, `analyzer` |
| `package:mockito` (gerador) | 10 | 4.447 | `analyzer`, `build` |
| `package:build_runner` | 36 | 4.779 | `dart:io`, `dart:isolate` |
| **`package:analyzer`** | **438** | **227.252** | `dart:io` (11), `dart:isolate` (2), `dart:ffi` (1), `dart:typed_data` (35), `dart:collection` (31), `dart:async` (15), `dart:convert` (12), `dart:math` (13), `dart:_internal` |

O `analyzer` é o alvo dominante: 227 mil linhas de Dart exercitando quase
toda a plataforma. Compilá-lo e executá-lo **é** o teste de maturidade da
nossa implementação, e é o que define a prioridade do backend nativo
(hoje em ~6/202 do corpus básico). Não há atalho honesto: ou o nosso
runtime roda isso, ou não somos compatíveis com o ecossistema.

Os geradores nativos em Rust continuam sendo **aceleração opcional**: só
entram com saída **byte a byte igual** à do builder oficial, e o motor
executa o builder de verdade sempre que não houver gerador nativo ou a
saída divergir.

## 2. Arquitetura

```
                       dartforge dev  (um processo)
                                │
   ┌────────────────────────────┼────────────────────────────┐
   │                            │                            │
watcher + grafo            banco semântico              emissão
(notify, BLAKE3)       (Program/Outline/Body)        (JS por lib)
   │                            │                            │
   └──────────► geradores ◄─────┘                            ▼
                    │                                    navegador
        ┌───────────┴────────────┐
        ▼                        ▼
  nativos (Rust)          worker Dart (IPC)
  ngdart, i18n, sass      builders do ecossistema
```

O ganho principal **não é "Rust é mais rápido que a Dart VM"**. É
**analisar o programa uma vez só**: hoje o `analysis_server` (IDE), o
`build_runner` (analyzer) e o `webdev` (DDC) analisam o mesmo código em
três processos, o que é a causa medida dos gigabytes que travam a máquina
(PLANO.md, "Evidência de campo"). O `dartforge dev` já tem o banco
semântico vivo e o platô de memória verificado (+0,00 MB em 20 edições);
os geradores passam a ler esse banco em vez de refazer a análise.

## 3. Fases, em ordem de valor medido

### Fase 1 — motor em Rust (grafo, cache, agendamento)

Entrega a parte que não depende de executar Dart: leitura do
`build.yaml`, grafo de assets e de builders, impressão digital,
observação do sistema de arquivos, paralelismo. É a orquestração que o
`build_runner` faz, independente de qual runtime executa os builders.

* `crates/build`: leitura de `build.yaml` (targets, `builders:`,
  `generate_for`, `include`/`exclude`), grafo de assets e de builders,
  observação do sistema de arquivos, execução paralela.
* **Impressão digital** por saída, não `mtime`:
  `blake3(fonte + versão do builder + opções + dependências semânticas)`.
  Igual ⇒ não executa nada. É a mesma disciplina do `crates/dev`, que já
  provou valer (edição de corpo: 1 unidade reanalisada, 1 módulo escrito).
* **Worker persistente, no nosso runtime**: o builder é compilado uma vez
  (AOT, resultado em cache por `blake3(fontes + versões + versão do
  DartForge + ABI)`) e fica vivo enquanto o `dartforge dev` rodar,
  falando com o motor por linhas JSON (`executar <builder> <asset>` →
  `saída <caminho> <hash>`). Sem inicialização por build e sem VM oficial.
* `build_web_compilers` não é executado: a compilação para JavaScript é
  nossa, por substituição.
* **Enquanto o nosso runtime não executar um builder**, a limitação é
  declarada e o projeto roda `dart run build_runner` à parte, uma vez,
  como já acontece hoje com os `.template.dart` do ngdart. É **limitação
  conhecida, não arquitetura**: nada no desenho depende do `dart`.

Aceite, em duas partes:
1. **Projetos do proprietário**: `new_sali/frontend` e
   `limitless_ui/example` geram os mesmos `.template.dart`/`.css` que o
   `build_runner` gera (byte a byte), e editar um componente regenera
   **só** o template dele. Meta: abaixo de 500 ms por edição, contra os
   2m59s de build completo.
2. **Corpus de compatibilidade do ecossistema** (`corpus/builders/`): um
   projeto pequeno por gerador — `json_serializable`, `freezed`,
   `drift`, `mockito`, `built_value`, `riverpod_generator`,
   `go_router_builder` —, cada um com `pubspec.yaml`, `build.yaml` e
   código que exercite o gerador. Critério: a saída do `dartforge build`
   é **byte a byte igual** à do `dart run build_runner build`, e o
   programa gerado compila e executa igual pelos dois caminhos. É o
   `crates/diferencial` aplicado à geração de código.

Nota de campo: o `mockito` 5.4.5 quebra com o `analyzer` 7.x (o gerador
de mocks usa tipos internos do analyzer) — está anotado nos `pubspec` de
três projetos do proprietário. O corpus precisa fixar a versão que
funciona, e essa incompatibilidade **é do ecossistema**, não nossa; o
nosso papel é reproduzir o mesmo resultado que a toolchain oficial dá.

### Fase 2 — geradores nativos para o que esses projetos usam

Cada gerador nativo lê o banco semântico (`Program`, `OutlineTypes`,
`BodyTypes`) — nenhuma reanálise, nenhum `package:analyzer`.

```rust
pub trait Gerador {
    fn interessa(&self, ctx: &Ctx, decl: DeclId) -> bool;   // pela anotação
    fn gerar(&self, ctx: &Ctx, lib: LibraryId) -> Vec<(String, String)>;
}
```

1. **`sass_builder`** — o mais simples e sem dependência do modelo Dart
   (`grass` ou `rsass` em Rust compilam SCSS). Entrega rápida, mede o
   motor da Fase 1 de ponta a ponta.
2. **`ngdart`** — é a Fase 5 do PLANO.md (compilador de templates), agora
   com um destino claro: em vez de emitir `*.template.dart` para o
   `build_runner` recompilar, o gerador nativo pode emitir **direto para
   a nossa IR/JS**, eliminando uma ida e volta inteira por Dart. Enquanto
   a equivalência não estiver provada, emitir `*.template.dart` idêntico
   ao do `ngdart` é o passo intermediário verificável (o `limitless_ui`
   tem suíte e2e: 26/26 é o critério).
3. **`i18n`** — pequeno, mecânico.
4. **`json_serializable`** e depois `freezed` — os mais usados do
   ecossistema. Aqui a regra de compatibilidade é dura: o gerador nativo
   só substitui o oficial quando a saída for **byte a byte igual** no
   corpus de compatibilidade; qualquer divergência é defeito nosso e o
   motor volta a executar o builder Dart. `drift` e `mockito`, que
   dependem fundo do `analyzer`, ficam no caminho do worker até a Fase 3
   estar madura.

### Fase 3 — `BuildStep.resolver` servido pelo nosso banco semântico

Para builders do ecossistema que exigem o analyzer. Um builder Dart
chama `buildStep.resolver.libraryFor(...)` e recebe `LibraryElement`,
`ClassElement`, `DartType`. Servir isso a partir do nosso banco exige
uma camada de compatibilidade que **imite a API do analyzer** — objetos
Dart no worker cujos métodos consultam o Rust por IPC.

É trabalhoso e é o que tira as 227 mil linhas do `analyzer` do caminho
quente: os objetos que o gerador recebe executam no nosso runtime e
consultam o nosso banco em vez de reanalisar o programa. O corpus de
compatibilidade é a rede: se um `ClassElement` nosso divergir do oficial
num gerador real, o teste acusa.

### Fase 4 — executar os builders do ecossistema no nosso runtime

**É requisito, não ambição opcional**, pela regra governante. O caminho,
em degraus verificáveis, cada um com corpus próprio:

1. `dart:async` com event loop, `dart:convert`, `dart:collection`,
   `dart:typed_data`, `dart:math` — já exercitados pelos 213 programas do
   corpus; o backend nativo precisa alcançá-los (hoje ~6/202).
2. `dart:io` (arquivo, diretório, processo, `Platform`) e `dart:isolate`:
   o `build_runner` e o `analyzer` dependem dos dois.
3. Compilar e executar `package:build` + `package:source_gen` (4 mil
   linhas somadas) com um gerador trivial escrito por nós.
4. Compilar e executar **`package:analyzer`** (227 mil linhas) — o marco.
   A partir dele, `json_serializable`, `freezed`, `drift` e `mockito`
   rodam por consequência, porque é dele que dependem.
5. Trocar o `analyzer` pelo nosso banco (Fase 3) e deixar de analisar o
   programa duas vezes.

O `dart` oficial nunca entra no produto: só como oráculo, comparando
saída byte a byte, como já fazemos com `dart run` no compilador JS.

## 4. O que descartar explicitamente

* **Não** recompilar os builders com LLVM `-O3` a cada ciclo: o custo do
  LLVM excede o que se economiza num gerador de 100 ms. Se um dia forem
  compilados por nós, é uma vez após `pub get`, com o resultado em cache
  por `blake3(fontes + versões + versão do DartForge + ABI)`.
* **Não** reimplementar o `build_web_compilers`: ele é o compilador que
  estamos substituindo.
* **Não** transformar gerador nativo em obrigação: ele é atalho medido,
  com saída idêntica verificada; sem isso, executa-se o builder oficial.
  A promessa é compatibilidade com o ecossistema inteiro, e velocidade
  onde ela for demonstrável.

## 5. Ordem de execução recomendada

1. **Corpus de compatibilidade** (`corpus/builders/`) antes do motor: um
   projeto por gerador do ecossistema, com a saída do `build_runner`
   oficial gravada como referência. Sem ele, "compatível" é opinião.
2. Fase 1 (motor + worker), medida nos dois projetos reais **e** verde no
   corpus de compatibilidade.
3. `sass_builder` nativo (prova o desenho de gerador, sem tocar no
   ecossistema).
4. `ngdart` nativo = Fase 5 do PLANO, com o e2e do `limitless_ui` (26/26)
   como critério.
5. `json_serializable` nativo, com igualdade byte a byte no corpus.
6. Fase 4 em degraus (`dart:io` → `dart:isolate` → `package:build` +
   `source_gen` → `package:analyzer`), que é o que tira a VM oficial do
   caminho, e a Fase 3 quando o resolver próprio valer a pena.
