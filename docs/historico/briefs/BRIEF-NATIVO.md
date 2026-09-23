# Brief — backend nativo do DartForge (AOT LLVM e JIT)

Você vai trabalhar no **compilador nativo** do DartForge, em paralelo com
outro agente que está no gerador do ngdart e no `dartforge serve`. Este
documento é o seu escopo: o que existe, o que fazer, como verificar e o que
não encostar.

---

## 0. Regras inegociáveis

1. **Worktree própria.** Não trabalhe em `D:/Projects/dartforge` direto.
   A worktree `D:/Projects/dartforge-nativo` **já existe**, na branch
   `nativo-excecoes-aot`, com um commit de preservação (`wip: estado
   intermediário do backend nativo`) do agente anterior: harness
   diferencial com `--nativo`, mexidas em `hir`/`llvm`/`lower` e no
   runtime. **Esse estado não compila** —
   `crates/emit_native/src/lower/mod.rs` tem delimitador aberto na linha
   258. Comece por ler esse diff (`git show HEAD`), decidir o que
   aproveitar e deixar o crate compilando outra vez; não recomece do zero
   sem olhar.
   ```
   cd D:/Projects/dartforge-nativo && git log --oneline -3 && git show --stat HEAD
   ```
   Commite lá, rebaseie e integre em `main` por fast-forward só quando
   compilar. Isto não é burocracia: com dois agentes na mesma árvore, um
   estado intermediário que não compila é revertido por quem "conserta o
   build" com `git checkout`, e já custou uma conversão inteira de trabalho
   (PLANO.md, "Regra de trabalho aprendida a custo").

2. **Não toque nestes arquivos** — são do outro agente, e conflito aqui
   custa caro: `crates/gerador_ng/**`, `crates/elements/src/gerado.rs`,
   `crates/dev/**`, `crates/emit_js/**`, `crates/cli/src/main.rs`.
   Seu território: `crates/emit_native/**`, `crates/runtime/**`,
   `crates/jit/**`, `crates/cranelift-jit/**`, `crates/native/**`,
   `crates/abi/**`, `crates/cli/src/nativo.rs`, `docs/NATIVO*.md`,
   `docs/JIT.md`, `docs/CRANELIFT.md`.
   Se precisar mexer em `crates/elements` ou `crates/types`, **só
   acrescente**; mudança de representação some com o trabalho de quem está
   lendo a mesma estrutura.

3. **Estudar antes de implementar.** Não descubra regra por tentativa e
   erro: leia a referência, entenda o algoritmo, escreva o plano, e só
   então escreva código. Compilar é **confirmação**, não método de
   descoberta — cada ciclo custa minutos e memória da máquina.

4. **Princípio 9 — funcionar primeiro.** Não gaste um minuto com
   formatação, estilo ou reorganização estética. O que conta é programa
   real compilando e executando com a saída certa.

5. **Princípio 10 — referência antes de código.** Antes de implementar uma
   forma, leia como o Dart oficial a define e como a VM a implementa. Nada
   de tentativa e erro: compilar, rodar e ver o que deu é **confirmação**,
   não método de descoberta. Referências no disco:
   - SDK 3.6.2 em `C:/tools/dartsdk-3.6.2/lib` (fonte do `dart:core`,
     `dart:io`, `dart:async` — inclusive a seção `vm` do `libraries.json`)
   - `references/dart-sdk`, `references/dart-language`, `references/asmjit`,
     `references/llvm-dartino`, `references/dartino-llvm`
   - `docs/AOT-REFERENCIAS.md`, `docs/AOT-NULL-SAFETY.md`,
     `docs/ABI-FFI-WASM.md`, `docs/GENERICS-REIFIED-REFERENCIAS.md`

6. **Regra governante: tudo nosso, compatível com o ecossistema.** A VM
   oficial **não faz parte do produto**. O `dart` oficial existe só como
   **oráculo** (`dart run --enable-asserts x.dart` dá a saída de
   referência). Nada de "chamar a VM para resolver isso".

7. **Commite e empurre com frequência.** Trabalho não commitado é trabalho
   que some.

---

## 1. Onde o código está — e qual trilha é a viva

O repositório tem duas trilhas. **A trilha nova é a única que interessa.**

| trilha | crates | estado |
| --- | --- | --- |
| **nova (viva)** | `frontend` → `elements` → `types` → `emit_native` / `emit_js` | é onde tudo acontece |
| velha (morta) | `lexer`, `parser`, `syntax`, `semantic`, `hir`, `compiler`, `codegen`, `llvm`, `optimizer`, `linker` | experimento inicial |

Consequência direta para você: **`crates/jit` (ORCv2) e
`crates/cranelift-jit` consomem `dartforge_hir::Module`, que é da trilha
velha.** Os dois são experimentos medidos, não produto. O compilador
nativo de verdade é `crates/emit_native`, que lê `Program`/`OutlineTypes`/
`BodyTypes` da trilha nova, baixa para uma HIR própria
(`crates/emit_native/src/hir.rs`), emite LLVM IR textual e chama o Clang.

`crates/runtime` (3.038 linhas) é o runtime em Rust: heap com GC por
tracing preciso, strings UTF-16, e a fronteira com o IR.

Leia, nesta ordem: `docs/NATIVO.md` (arquitetura, layout de objeto, ABI,
despacho por seletor e vtable), `docs/NATIVO-PRODUCAO.md`, `docs/JIT.md`,
`docs/CRANELIFT.md`.

---

## 2. Estado medido (não é chute)

- **Corpus nativo: ~6 de 202.** Passam: strings, classes, interpolação,
  coleções básicas, `StringBuffer`, `for-in`, runas.
- O corpus é o mesmo do backend JS: `corpus/js/` (213 programas já
  verificados contra a VM). O backend JS passa 214/214; o nativo, 6.
- O Clang e a ligação dominam o tempo de compilação; há cache de objeto do
  runtime por hash (`crates/emit_native/src/cache.rs`), mas não do módulo.

**Sobre o Cranelift**: `docs/CRANELIFT.md` registra um experimento já feito
e medido, cuja recomendação é **não adotar**. Leia antes de investir nele.
Se for mexer em JIT, o trabalho útil não é repetir o experimento: é
**rebasear o JIT na HIR do `emit_native`** (a da trilha nova), para que
desenvolvimento e produção compartilhem uma representação só. Hoje o
`crates/jit` ORCv2 e o AOT compartilham o LLVM IR, e há um teste que exige
saída idêntica pelos dois caminhos (`crates/jit/tests/execucao.rs`) — esse
contrato é bom e deve sobreviver.

---

## 3. O que fazer, em ordem

A ordem não é arbitrária: ela é a que desbloqueia mais programas do corpus
por unidade de trabalho, e termina no alvo governante.

### 3.1 Exceções (`throw`, `try`/`catch`/`finally`, `rethrow`)

Bloqueia uma fatia grande do corpus e é pré-requisito de quase tudo (o
`dart:core` lança). Decida e documente o mecanismo: *landing pads* do LLVM
(`invoke`/`landingpad`, com o modelo de personalidade do alvo) ou retorno
por valor com verificação. Meça as duas em programas sem exceção — o custo
no caminho feliz é o que decide. Referência obrigatória: como a VM do Dart
trata `finally` com `return` dentro, e a ordem de execução de
`catch`/`finally` aninhados.

**Aceite**: os programas de exceção do `corpus/js/` passam com saída
idêntica à da VM, `finally` incluído.

### 3.2 `async`/`await` e o laço de eventos

A HIR já faz desugaring de `async` em máquina de estados
(`crates/emit_native/src/hir.rs`); falta o laço de eventos e os
`Future`/`Completer`/`Zone` do runtime. Sem isso nenhum programa real roda.

**Aceite**: `async`/`await`, `Future.delayed`, `Stream` básico e a ordem
de microtarefas iguais à da VM (a ordem é observável e o corpus compara
saída byte a byte).

### 3.3 Genéricos reificados

`is`/`as` com argumentos de tipo, `List<int>` contra `List<dynamic>`,
`runtimeType`. O layout já reserva `metadata_ptr` para o vetor canônico de
argumentos (`docs/NATIVO.md` §2); falta usar. Leia
`docs/GENERICS-REIFIED-REFERENCIAS.md` e como o dart2js faz a receita de
rti — o backend JS já acertou isso e o defeito mais sutil que apareceu lá
(`raw|Caixa<@>`, nunca `raw|Caixa`) vale como aviso.

### 3.4 `dart:core` da seção `vm`, a partir da fonte

Hoje o `dart:core` nativo é parcial. O `libraries.json` do SDK tem a seção
`vm` com os `@patch` certos; carregue-os como o backend JS carrega os do
`dartdevc`. Isso é o que faz `int` de 64 bits, `String` UTF-16 e as
coleções pararem de ser casos especiais do compilador.

### 3.5 `dart:io`

É o que falta para dois alvos reais do proprietário:
- `new_sali/core`: 7 dos 14 testes de `package:test` não rodam no navegador
  porque leem arquivo e fontes de PDF;
- `new_sali/backend` (angel3): `dart:io`, `dart:isolate`, `dart:ffi`.

Comece por `File`, `Directory`, `Platform`, `stdout`/`stderr`, `Process`.
`HttpServer` depois.

### 3.6 Isolates

`Isolate.spawn`, `SendPort`/`ReceivePort`. Sem memória compartilhada, como
a VM: cada isolate tem seu heap. Isto e `dart:io` são o que o
`package:build`/`source_gen` exigem.

### 3.7 Cache de módulo e tempo de compilação

Hoje o Clang/link domina. Com o resto de pé, cachear o objeto por módulo
(hash do IR) é o que torna o AOT usável no ciclo de desenvolvimento.

### Alvo governante (o fim da estrada)

**Compilar e executar `package:analyzer`** — 438 arquivos, 227.252 linhas,
com `dart:io`, `dart:isolate`, `dart:ffi`, `typed_data`. É dele que
dependem `json_serializable`, `freezed`, `drift`, `mockito` e `source_gen`,
e a regra do projeto é que eles rodem **no nosso runtime**. Não é o
primeiro passo; é o critério de pronto.

---

## 4. Como verificar

```powershell
# compilar o CLI com o backend nativo (o padrão não inclui, para não puxar LLVM)
cargo build -p dartforge-cli --features nativo

# um programa
target\debug\dartforge.exe compile-native corpus\js\001_hello.dart -o saida.exe
.\saida.exe

# AOT de produção, com tempos
target\debug\dartforge.exe aot entrada.dart saida.exe --optimize --timings

# JIT em memória
target\debug\dartforge.exe run entrada.dart

# o oráculo — a saída que tem de sair igual
dart run --enable-asserts corpus\js\001_hello.dart
```

O harness diferencial (`crates/diferencial`) hoje roda o corpus pelo
backend JS. **Estender esse harness para o nativo é a primeira coisa que
vale a pena fazer**: sem placar automático você vai medir no olho, e o
número "6/202" precisa virar um comando que qualquer um roda. Espelhe o
que já existe — ele já tem cache de oráculo, execução paralela e
comparação byte a byte de `stdout`.

---

## 5. O que entregar em cada passo

- Um commit por forma implementada, com mensagem em português explicando
  **por que** a decisão foi essa (qual referência, qual medição).
- O número do corpus atualizado em `ESTADO.md` §1.5 e §2.5 — é o placar do
  projeto.
- Nada de "quase funciona": se um programa passa, ele passa com saída
  idêntica à da VM, byte a byte.
