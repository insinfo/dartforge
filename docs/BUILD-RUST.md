# `dartforge build` — o motor de geração de código em Rust

Plano para substituir o `build_runner` no ciclo de desenvolvimento, sem
quebrar o ecossistema. Escrito em 2026-09-22 a partir de medição dos
projetos reais do proprietário, não de suposição.

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

> **Regra governante (proprietário, 2026-09-22): compatibilidade
> obrigatória com o ecossistema.** O nosso compilador, a nossa VM, o
> nosso analisador e o nosso LSP têm de funcionar com os pacotes que o
> ecossistema Dart usa — `json_serializable`, `freezed`, `drift`,
> `mockito`, `source_gen`, `build`, `analyzer` e os demais. Um projeto
> real que compila com a toolchain oficial tem de compilar com a nossa.
> Não construímos um fork incompatível do Dart.

É a mesma regra que governa a linguagem ("qualquer projeto Dart 3.6
válido"), aplicada às ferramentas. A consequência prática está na §3: a
compatibilidade vem **na Fase 1**, porque o worker executa os builders do
ecossistema como Dart de verdade, com o `package:analyzer` de verdade. Os
geradores nativos das fases seguintes são **aceleração opcional**, nunca
substituição obrigatória: cada um só entra se produzir saída **idêntica**
à do builder oficial, e o motor cai de volta no builder Dart sempre que
não houver gerador nativo ou a saída divergir.

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

### Fase 1 — motor em Rust, builders Dart por worker persistente

Entrega o grafo, o cache e o agendamento; os builders continuam sendo os
do ecossistema, mas sem pagar a inicialização a cada build.

* `crates/build`: leitura de `build.yaml` (targets, `builders:`,
  `generate_for`, `include`/`exclude`), grafo de assets e de builders,
  observação do sistema de arquivos, execução paralela.
* **Impressão digital** por saída, não `mtime`:
  `blake3(fonte + versão do builder + opções + dependências semânticas)`.
  Igual ⇒ não executa nada. É a mesma disciplina do `crates/dev`, que já
  provou valer (edição de corpo: 1 unidade reanalisada, 1 módulo escrito).
* **Worker Dart persistente**: um processo `dart` vivo enquanto o
  `dartforge dev` estiver rodando, com os builders já carregados, falando
  por linhas JSON no stdin/stdout (`executar <builder> <asset>` →
  `saída <caminho> <hash>`). Sem `dart` novo por build.
* **Qualquer builder do ecossistema roda aqui**, sem alteração: o worker
  é Dart de verdade, carrega `package:build`, `package:source_gen` e o
  `package:analyzer` como o `build_runner` faz. É isto que cumpre a regra
  de compatibilidade desde a primeira fase.
* `build_web_compilers` é a única exceção, e por substituição, não por
  incompatibilidade: a compilação para JavaScript é nossa.

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

É trabalhoso, e **é obrigatório** pela regra de compatibilidade — o que
não é obrigatório é a ordem: enquanto ele não existir, o worker Dart da
Fase 1 já roda esses builders com o `analyzer` oficial, então nenhum
projeto fica de fora. A Fase 3 troca o analyzer pelo nosso banco para
ganhar velocidade e deixar de analisar o programa duas vezes, com o
corpus de compatibilidade como rede: se um `ClassElement` nosso divergir
do oficial num gerador real, o teste acusa.

### Fase 4 — compilar os próprios builders com o DartForge nativo

O marco técnico proposto ("compilar `package:analyzer` +
`package:source_gen` + `json_serializable` com o DartForge e executá-los
em LLVM") é atraente e **fica por último**, por uma razão medida: o
backend nativo está em ~6/202 do corpus básico; o `package:analyzer`
sozinho passa de 200 mil linhas de Dart e usa `dart:io`, `dart:isolate`,
`dart:mirrors`-adjacentes e todo o `dart:async`. É um alvo de outra
ordem de grandeza, e **não está no caminho crítico**: pelas Fases 1 e 2,
os três builders que os projetos reais usam já estão resolvidos sem ele.

Quando o backend nativo amadurecer, isto vira o teste de maturidade
definitivo da implementação da linguagem — e aí o worker Dart deixa de
precisar da VM oficial. Até lá, usar a VM oficial no worker **não é
concessão**: é o que garante que nenhum projeto real fique de fora
enquanto a nossa pilha cresce.

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
6. Fase 3 (resolver sobre o nosso banco) — obrigatória, medida pelo mesmo
   corpus; depois a Fase 4.
