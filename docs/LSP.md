# Servidor LSP do DartForge (`crates/lsp`, `editors/vscode`)

Servidor da linguagem Dart em Rust, por stdio, com sincronização incremental
e colunas UTF-16 corretas. Expõe o front-end novo (parser completo de Dart
3.6, que aceita 100% do `new_sali`) ao editor, e existe para provar que não
repete o defeito medido no PLANO.md: o LSP do Dart chega a ~6 GB no projeto
de referência e a memória só sobe a cada edição até ser preciso matar o
processo.

## Desenho

```text
                    stdin (bytes)
                       │
                 ┌─────▼──────┐
                 │  leitora   │  thread 1: `ler_mensagem` quadro a quadro
                 └─────┬──────┘  (Content-Length + JSON), enfileira Values
                       │ mpsc
              ┌────────▼────────┐
              │    Servidor     │  thread principal: dono de tudo,
              │  ┌───────────┐  │  passado por `&mut` ao despacho
              │  │ fila      │  │  VecDeque de mensagens em ordem
              │  │ docs      │  │  DocumentStore: 1 entrada por documento
              │  │ cancel.   │  │  ids na fila (teto 4096, limpa ao responder)
              │  │analisador │  │  trait Analisador (sintático hoje)
              │  └───────────┘  │
              └────────┬────────┘
                       │ respostas e publishDiagnostics,
                       │ uma mensagem inteira por vez sob Mutex
                 ┌─────▼──────┐
                 │   stdout   │  SÓ protocolo. Log vai para stderr.
                 └────────────┘
```

Cópia do desenho do rust-analyzer, não do código (`references/rust-analyzer`):
documentos em memória versionados (`mem_docs.rs` → `DocumentStore`),
operações com cancelamento (`op_queue.rs` → fila + `$/cancelRequest`),
conversão ampla de posições (`lsp/utils.rs` → `utf16.rs`), cliente fino
(`editors/code` → `editors/vscode`).

Cada `bombear` faz, nesta ordem: (1) extrai e aplica **todos** os
`$/cancelRequest` da fila, estejam antes ou depois da requisição alvo —
sem isso o cancelamento seria condição de corrida entre a leitora e o
despacho; (2) processa as notificações líderes (`didOpen`/`didChange`/`didClose`,
cada mudança publica diagnósticos); (3) executa **uma** requisição, ou
responde `RequestCancelled` (-32800) se ela foi cancelada. Requisição em
execução checa cancelamento em pontos seguros (o gancho de teste
`dartforge/dormir` dorme em fatias de 10 ms; a análise real checará entre
documentos, nunca no meio de uma frase do parser).

Por documento, o único estado retido é texto + tabela de linhas + versão; a
chegada da versão N substitui a N−1 no lugar e `didClose` remove. Nenhum
cache sem teto, nenhum histórico de versões, nenhum log em memória; o
conjunto de cancelamentos só guarda ids de requisições ainda na fila e é
limpo ao responder. Nada de `Rc<RefCell<…>>` no estado global.
O analisador esquece a versão de linguagem associada à URI em `didClose` e
recarrega o `package_config.json` na próxima abertura; reabrir um documento
já aberto também invalida esse estado.
O caminho local da URI é decodificado como URL de arquivo, inclusive nomes
Unicode percent-encodados, antes de descobrir o pacote e sua versão.

## Sincronização incremental e UTF-16

Capacidade anunciada: `textDocumentSync: 2` (incremental),
`positionEncoding: "utf-16"`. Cada documento guarda `Vec<usize>` de inícios
de linha, recalculada só do ponto editado em diante (`recalc_a_partir`).
Coluna do protocolo (unidades UTF-16) vira offset de bytes na entrada e o
span de bytes do diagnóstico vira posição na saída; quebras `\n`, `\r\n` e
`\r`; saturação sem `panic` fora do documento; coluna no meio de um par
substituto trunca para a borda.

Sem debounce: medido, não chutado. A medição abaixo dá ~0,35 ms por
diagnose (transporte + análise + publicação, média em 2.516 mudanças sobre
o `new_sali`); um debounce de 50 ms só adicionaria latência sem economizar
trabalho relevante. A decisão será revista quando a análise semântica
entre — o custo por tecla será remedido então.

## Capacidades

`textDocument/documentSymbol` lista declarações de topo e membros a partir da
AST do documento aberto. Usa `DocumentSymbol[]` hierárquico quando o cliente
anuncia `hierarchicalDocumentSymbolSupport`; caso contrário devolve
`SymbolInformation[]` plano, como exige o contrato do LSP. Os intervalos são
UTF-16. A árvore é descartada depois de cada pedido, mantendo o platô de
memória por edição. Edições `didChange` com versão antiga são ignoradas sem
republicar diagnósticos. Testes direcionados: `cargo test -p dartforge-lsp
--test simbolos --locked`.

`workspace/symbol` busca nos **documentos abertos** (sem índice em disco),
com comparação sem distinguir maiúsculas e minúsculas. As URIs são ordenadas
antes da análise, e cada AST é liberada antes da próxima: uma consulta não
mantém cópias das versões já substituídas. O resultado é sempre
`SymbolInformation[]`, no intervalo do nome.

`textDocument/definition` navega do literal de URI em `import`, `export`,
`part`, `part of` e `import augment` para um **arquivo relativo existente**.
Também navega de uma anotação de tipo sem prefixo (`Caixa x`) para uma única
declaração de tipo homônima no mesmo arquivo. Para esse segundo caso, só
responde quando não há diretivas que tragam outros nomes nem parâmetro de
tipo homônimo em qualquer escopo da unidade; assim evita apontar para uma
classe sombreada. A posição de entrada e o intervalo de destino são UTF-16.
Referências de expressão a uma variável de topo única também navegam quando
nenhum parâmetro, variável local, membro ou padrão pode sombrear o nome.
O mesmo vale para uma função de topo única; uma função local homônima impede
a navegação até haver resolução por escopo.
URIs `dart:`/`package:` e demais nomes aguardam resolução de bibliotecas e
elementos. A regra de ativação do literal segue a navegação de diretivas do
analyzer (`analyzer_plugin/.../navigation_dart.dart`): só existe alvo quando
o arquivo existe. Teste: `cargo test -p dartforge-lsp --test navegacao
--locked`.

`textDocument/hover` mostra a descrição sintática de um tipo local resolvido
pela regra conservadora da definição acima: `class C`, `enum E`, `mixin M`
ou `extension type X`, quando não genérico. Usa Markdown se o cliente o
anuncia; caso contrário devolve texto simples. O intervalo cobre apenas o
nome sob o cursor. Tipos genéricos e typedefs aguardam a formatação de
assinatura do modelo de elementos, e referências importadas aguardam
resolução semântica. Para uma variável de topo única com tipo primitivo
escrito (`int`, `double`, `num`, `bool`, `String`, `Object`, `dynamic`), mostra
`tipo nome` e `Type: tipo`, como a descrição do `VariableElement` no analyzer.
Nomes locais homônimos desligam esse hover até existir resolução por escopo.
Funções de topo com até dois parâmetros posicionais obrigatórios, todos com
tipos primitivos escritos, e retorno primitivo/`void` escrito recebem hover
com assinatura `tipo nome(tipo parâmetro, ...)`. Getters de topo com retorno
primitivo escrito mostram `tipo get nome` e `Type: tipo`, seguindo o formato
dos testes de hover do servidor Dart. A navegação para a declaração funciona
mesmo quando a assinatura não pode ser mostrada; nesses casos o hover fica
vazio. Funções genéricas, parâmetros opcionais/nomeados e retorno inferido
aguardam a formatação completa da assinatura.
Teste: `cargo test -p dartforge-lsp --test hover --locked`.

Implementadas: `initialize` (com `serverInfo`), `initialized`, `shutdown`,
`exit` (0 após `shutdown`, 1 sem), `$/cancelRequest`,
`textDocument/didOpen`/`didChange` (incremental e integral)/`didClose`,
`textDocument/publishDiagnostics` (`severity`, `source: "dartforge"`,
`version`), `dartforge/dormir` (gancho de teste do cancelamento em
execução; clientes reais nunca enviam).

Explicitamente fora deste brief: completion, definição de variáveis/funções locais e nomes importados, referências,
rename, code actions, formatação e `diagnosticProvider` por
requisição (o servidor empurra diagnósticos; não atende pull). Semântica
(nomes não resolvidos, erros de tipo) chega depois via `crates/types`,
pela costura `trait Analisador { fn diagnosticar(&mut self, uri, texto) }`
— o transporte não muda.

## Testes

`crates/lsp/tests/protocolo.rs` (processo filho, bytes no fio):
`sessao_completa` (2 erros → correção incremental → 1 → fechar → 0 →
código 0), `utf16_correto` (emoji 4 bytes/2 unidades, CRLF, escape de
surrogate solto), `stdout_limpo` (com `RUST_LOG=debug`, tudo decodifica
quadro a quadro), `cancelamento` (resposta -32800).
`tests/plato_protocolo.rs` (`plato_pelo_protocolo`, K=24 × K=24 pelo
transporte com arquivos reais): binário separado de propósito — o alocador
contador é global e os testes de um binário dividem o processo, então a
medição precisa do processo só para ela.

## Medição D.2 — mesmo projeto, mesma sequência

Comando: `scripts/medir-lsp.ps1` (sem flag: DartForge; com `-Dart`: o LSP
do Dart). Sequência idêntica: `initialize`, 1.258 `didOpen`, 200 edições
incrementais distribuídas, `shutdown` + `exit`; RSS via `Get-Process` a
cada 50 mensagens. Projeto `C:/MyDartProjects/new_sali` (ngdart + angel3,
1.258 arquivos, 8,52 MiB). Máquina e data da rodada: Windows x64,
2026-09-22, binário release.

| | DartForge (`dartforge-lsp`) | Dart (`dart language-server --protocol=lsp`) |
| --- | --- | --- |
| Mensagens enviadas | 1.462 | 1.462 |
| Quadros recebidos | 1.460 (diagnóstico por abertura e edição) | 1 (só o `initialize`; nada publicado em 2 min) |
| Tempo total | 2,5 s | 131,9 s (com 120 s de espera final) |
| Pico RSS | 21,3 MiB | 640,5 MiB |
| Platô RSS (média das 10 últimas) | 19,0 MiB | 633,7 MiB |
| Saída | 0 | 0 |

In-process (alocador contador, `crates/lsp/examples/memoria_lsp.rs`, tudo retido como num editor
com o projeto aberto): 8,52 MiB de fonte → 19,94 MiB vivos (2,34×),
pico 20,43 MiB, 893 ms, 1,37 M alocações; após fechar tudo, 1.024 bytes
acima da base (o `Vec` de caminhos do próprio exemplo ainda vivo na hora
da impressão; o teste de platô, sem esse resíduo, volta ao nível exato).

Leitura honesta: o número de 6 GB do PLANO.md é o relato de campo do
proprietário em edição interativa, não reproduzido aqui — o Dart, nesta
sequência em lote, estabilizou em ~634 MiB sem publicar diagnósticos na
janela (possível causa: sem `package_config` no projeto, a resolução de
pacotes fica prejudicada, ou a análise priorizada nunca alcança 1.258
arquivos abertos de uma vez). O que a comparação prova, sem extrapolar:
na mesma sequência, o DartForge publica diagnóstico para cada abertura e
edição retendo ~20 MiB, e N edições estabilizam em platô (afirmado pelo
teste) em vez de crescer com N.

Prova de ponta a ponta com arquivo real (binário release, mesma invocação
da extensão: `dartforge-lsp --stdio`; `local_signer_server.dart` do
`new_sali` + `void quebrado( { }` inserido): 1 diagnóstico, linha 127
coluna 0, `source: "dartforge"`, mensagem `esperava ')', encontrou o fim
do arquivo`. No VS Code isso aparece como sublinhado vermelho na linha 128
com a mensagem no hover e no painel Problemas; o canal *Output ›
DartForge* mostra o ciclo de vida. (Janela de desenvolvimento via F5 com a
extensão compilada — passos em `editors/vscode/README.md`; sem captura de
tela neste ambiente.)
