# Brief — servidor LSP em Rust e extensão VS Code (`crates/lsp`, `editors/vscode`)

Você está no repositório DartForge (`D:\Projects\dartforge`), um compilador
Dart → JavaScript em Rust. A motivação deste trabalho está medida no
PLANO.md: no projeto de referência do proprietário
(`C:/MyDartProjects/new_sali`, ngdart + angel3, 1.258 arquivos, 8,5 MiB de
Dart) o **LSP do Dart chega a ~6 GB** e a memória só sobe a cada edição até
ser preciso matar o processo. O front-end novo do DartForge analisa o
projeto inteiro em 254 ms retendo 132 MiB. Falta o servidor que expõe isso
ao editor — e a medição que prova que ele não repete o defeito.

Leia antes de tocar em código, nesta ordem:

1. `PLANO.md` — "Meta de projeto — cadeia de ferramentas própria" inteira
   (as duas "Precisões obrigatórias" e "O LSP é a peça mais difícil"), e
   "O que falta para não repetir webdev, analyzer e LSP do Dart", itens 3,
   5, 6, 9 e 10.
2. `crates/lsp/src/lib.rs` — o que existe: `diagnose()` e `DocumentStore`
   (dono por documento; versão N substitui N−1 no lugar; `close` remove).
   `crates/lsp/tests/plato_documentos.rs` — o teste de platô K×K, e o
   comentário sobre por que testes de memória vivem num só `#[test]`.
3. `crates/frontend/src/parser/mod.rs` — `parse(&str, &mut Interner) ->
   Parsed { unit, ast, diagnostics }`: o parser completo de Dart 3.6, que
   aceita 100% do SDK, do corpus pub e do `new_sali`, e **acumula** todos
   os diagnósticos sintáticos com recuperação por declaração.
4. `crates/instrument/src/lib.rs` — alocador contador (`live_bytes`,
   `peak_bytes`) e `crates/frontend/examples/memoria.rs` como modelo de
   medição.
5. `references/rust-analyzer/crates/rust-analyzer/src/` — `main_loop.rs`,
   `global_state.rs`, `mem_docs.rs`, `op_queue.rs`, e `lsp/utils.rs`
   (conversão de posições UTF-16). É a prova de existência da meta;
   copie o desenho, não o código.
6. `references/rust-analyzer/editors/code/` — o cliente TypeScript fino.

## Regra de coexistência — o que você NÃO toca

`crates/frontend` está com outro agente (reduzindo o tamanho dos nós da
AST; a API `parse` não muda). `crates/elements` e `crates/types` estão com
o agente da inferência de corpos. Você consome só `dartforge_frontend::
parser::parse` e `dartforge_diagnostics`. **Não** chame
`dartforge_compiler::compile_diagnostics` para arquivos reais: é o
compilador do subconjunto antigo e recusaria quase todo arquivo do
`new_sali` com erros que não são erros. A `diagnose()` atual pode ficar
como está para os testes antigos, mas o servidor usa o parser novo.

Semântica (nomes não resolvidos, erros de tipo) chega depois, do
`crates/types`. Deixe a costura pronta: um `trait Analisador { fn
diagnosticar(&mut self, uri, texto) -> Vec<Diagnostic> }` com a
implementação sintática hoje, para que a semântica entre sem tocar no
transporte.

## O que entregar

### A. Transporte JSON-RPC por stdio

Sem runtime assíncrono: uma thread lê `stdin` (cabeçalho
`Content-Length`, corpo JSON), enfileira; a thread principal despacha;
respostas e notificações saem por `stdout` sob um `Mutex` (uma mensagem
inteira por vez). **`stdout` é só protocolo**: qualquer log vai para
`stderr` — um `println!` esquecido corrompe a sessão, e o teste de
aceite verifica isso. `serde_json` para o JSON; `lsp-types` (crates.io)
é aceitável só para os tipos de mensagem; o transporte é escrito aqui
(é pequeno e precisamos controlar cancelamento e memória).

Ciclo de vida: `initialize` (capacidades: `textDocumentSync` incremental,
`diagnosticProvider` não — usa `publishDiagnostics`), `initialized`,
`shutdown`, `exit` (código 0 após `shutdown`, 1 sem). `$/cancelRequest`:
requisições em fila cancelam com `RequestCancelled`; a que está em
execução checa uma flag de cancelamento em pontos seguros.

### B. Sincronização incremental de documentos com UTF-16 correto

`didOpen`/`didChange`/`didClose` sobre o `DocumentStore`. `didChange`
**incremental**: cada `contentChange` traz `range` em (linha, coluna) com
coluna em **unidades UTF-16**; converter para offset de bytes UTF-8 exige
tabela de linhas por documento (`Vec<u32>` de inícios de linha, recalculada
só do ponto editado em diante). Reenviar o arquivo inteiro a cada tecla é
proibido pelo PLANO.md. O `DocumentStore` passa a guardar texto + tabela
de linhas + versão, e continua sendo o único dono: N substitui N−1.

Diagnósticos: após cada mudança, analisar e publicar
`textDocument/publishDiagnostics` com `range` convertido de volta para
UTF-16, `severity`, `source: "dartforge"`. Um documento fechado recebe uma
publicação vazia. Debounce curto (por exemplo 50 ms) para digitação
rápida, medido, não chutado.

### C. Binário e extensão

* Binário `dartforge-lsp` (novo `[[bin]]` em `crates/lsp` ou no
  `crates/cli`, o que for mais simples; `--version` e `--stdio`).
* `editors/vscode/`: cliente TypeScript **fino** com
  `vscode-languageclient` — só localiza o binário (configuração
  `dartforge.serverPath`, senão `PATH`) e o inicia; ativa em
  `onLanguage:dart`; nenhuma lógica no cliente. `package.json`,
  `tsconfig.json`, `src/extension.ts`, `README.md` com os passos de
  `npm install && npm run compile` e como rodar em modo de
  desenvolvimento (F5). A extensão é TypeScript por necessidade da
  plataforma — está registrado no PLANO.md como precisão obrigatória.

### D. Medição — o que este trabalho existe para provar

`crates/lsp/examples/memoria.rs` (alocador contador) e um script de RSS:

1. **Platô sob o protocolo**: abrir K = 24 documentos reais do `new_sali`
   pelo transporte (mensagens JSON de verdade, não chamadas diretas),
   aplicar K edições incrementais em cada um, afirmar `live_bytes` em
   platô e retorno ao nível inicial após fechar. É o
   `plato_documentos.rs` de hoje, elevado ao servidor inteiro.
2. **RSS do processo no `new_sali`**: script PowerShell em
   `scripts/medir-lsp.ps1` que inicia `dartforge-lsp`, envia `initialize`,
   abre os 1.258 arquivos, faz 200 edições distribuídas, e lê o RSS via
   `Get-Process` a cada 50 mensagens. Registrar pico e platô em
   `docs/LSP.md`. O mesmo script, com a flag `-dart`, roda o LSP do Dart
   (`dart language-server --protocol=lsp`) na mesma sequência — a
   comparação honesta é **no mesmo projeto, com a mesma sequência**, e o
   PLANO.md diz que nenhum número pode ser afirmado antes dela existir.

## Critério de aceite

`crates/lsp/tests/protocolo.rs`:

1. `sessao_completa`: transcrição `initialize` → `initialized` → `didOpen`
   (arquivo com dois erros de sintaxe) → recebe `publishDiagnostics` com
   **dois** diagnósticos → `didChange` incremental corrigindo um → recebe
   um → `didClose` → recebe zero → `shutdown` → `exit` com código 0.
   Tudo por bytes no `stdin`/`stdout` do servidor em processo filho, não
   por chamada de função.
2. `utf16_correto`: linha com emoji (`'👭'` ocupa 2 unidades UTF-16 e 4
   bytes) antes do erro — a coluna publicada é a UTF-16, e um
   `didChange` com `range` em UTF-16 depois do emoji edita o byte certo.
   Inclua também CRLF e uma linha com surrogate solto via escape.
3. `stdout_limpo`: com `RUST_LOG=debug` (ou o mecanismo de log que você
   escolher), a sessão inteira ainda decodifica mensagem a mensagem;
   nada fora de `Content-Length` sai em `stdout`.
4. `cancelamento`: enfileirar duas requisições e cancelar a segunda antes
   de a primeira terminar; a resposta da segunda é `RequestCancelled`.
5. `plato_pelo_protocolo`: item D.1, num só `#[test]` (pela razão
   documentada em `plato_documentos.rs`).
6. Extensão: `npm run compile` sem erro e um vídeo/GIF não é exigido —
   descreva na resposta final o que apareceu no VS Code ao abrir um
   arquivo do `new_sali` com um erro de sintaxe inserido.
7. `docs/LSP.md` com: desenho (threads, fila, dono dos documentos), as
   capacidades implementadas e as não implementadas (hover, completion,
   definição etc. ficam explicitamente fora deste brief), e a tabela de
   medição D.2 com DartForge e Dart lado a lado.

## Disciplina

* Documentação Rust em português, com contrato e exemplo.
* Nenhum cache sem teto; nenhuma estrutura viva que cresça com o número
  de mensagens (histórico de versões, fila sem consumo, log em memória).
  Se precisar guardar algo por documento, o dono é o `DocumentStore`.
* Nada de `Rc<RefCell<…>>` para o estado global: um `struct Servidor`
  dono de tudo, passado por `&mut` ao despacho.
* `cargo fmt`, `cargo clippy -p dartforge-lsp --all-targets` limpos,
  `cargo test -p dartforge-lsp` verde; a saída de D.2 colada na resposta.
* **Commit ao fim de cada bloco verificado** (só os seus arquivos:
  `crates/lsp`, `editors/vscode`, `scripts/medir-lsp.ps1`, `docs/LSP.md`;
  mensagem em português; sem push) — instrução do proprietário para não
  perder trabalho. No PLANO.md, atualize só o "Estado em 2026-09-21" dos
  itens 6 e 9 e a linha de medição do item 10.

Nota de eficiência (proprietário): quando o passo seguinte é executar o binário, use `cargo build` direto — `cargo check` seguido de `build` tipa duas vezes e não reaproveita nada. `check` só quando for corrigir erros sem executar. Faça `git merge main` para receber `.cargo/config.toml` com `target/` compartilhado (D:/Projects/dartforge/target): a máquina tem 8 GB e os builds das worktrees se enfileiram em vez de recompilar tudo.

**Princípio 10 (proprietário, 2026-09-22):** referência antes de código — localize a regra na referência (DDC/dartdevc, CONTRATO-DDC.md, especificação/CFE, runtime/lib/*.cc, analyzer), leia o código próprio envolvido inteiro, corrija todas as ocorrências de uma vez e compile uma vez para confirmar. Tentativa-e-erro compilando/executando para descobrir o que fazer é proibido.
