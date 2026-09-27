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
Também resolve literais `package:` pelo `package_config.json` descoberto a
partir do arquivo aberto, somente se o pacote estiver mapeado e o destino
existir. Não usa o fallback de pacotes de referência do compilador.
Também navega de uma anotação de tipo sem prefixo (`Caixa x`) para uma única
declaração de tipo homônima no mesmo arquivo. Para esse segundo caso, só
responde quando não há diretivas que tragam outros nomes nem parâmetro de
tipo homônimo em qualquer escopo da unidade; assim evita apontar para uma
classe sombreada. A posição de entrada e o intervalo de destino são UTF-16.
Referências de expressão a uma variável de topo única também navegam quando
nenhum parâmetro, variável local, membro ou padrão pode sombrear o nome.
O mesmo vale para uma função de topo única; uma função local homônima impede
a navegação até haver resolução por escopo. Entre documentos abertos, o nome
resolve para o documento que o declara quando o arquivo só tem imports
relativos simples e um único aberto declara o nome sozinho num espaço (tipo
ou valor); prefixo, `show`/`hide`, `export`, `part`, `part of`, augmentations,
`dart:`/`package:`, padrões, sombras e dono fora dos abertos devolvem vazio
em vez de destino errado.
URIs `dart:` e os demais nomes importados aguardam cobertura de navegação.
A regra de ativação do literal segue a navegação de diretivas do
analyzer (`analyzer_plugin/.../navigation_dart.dart`): só existe alvo quando
o arquivo existe. Teste: `cargo test -p dartforge-lsp --test navegacao
--locked`.

`textDocument/hover` mostra a descrição sintática de um tipo local resolvido
pela regra conservadora da definição acima: `class C`, `enum E`, `mixin M`
ou `extension type X`, quando não genérico. Usa Markdown se o cliente o
anuncia; caso contrário devolve texto simples. O intervalo cobre apenas o
nome sob o cursor. Tipos genéricos e typedefs aguardam a formatação de
assinatura do modelo de elementos. Para uma variável de topo única com tipo primitivo
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
aguardam a formatação completa da assinatura. Entre documentos abertos, o
hover resolve como a definição (só import relativo simples, sem prefixo,
`show`/`hide`, `export`, `part`, `dart:`/`package:`) e formata a descrição a
partir do documento dono, com as mesmas regras do hover local; o intervalo
continua no arquivo do cursor. Dono não-aberto, símbolo ambíguo ou
assinatura sem formato fiel devolvem hover vazio (null), nunca texto errado.
Teste: `cargo test -p dartforge-lsp --test hover --locked`.

O binário também carrega o SDK descoberto por `SdkLayout::discover` e resolve
variáveis, funções e getters de topo importados em `definition` e `hover`,
com nome simples ou prefixo explícito (`p.nome`).
`elements` escolhe o vínculo no namespace da biblioteca, e `types` resolve a
anotação explícita para o hover (`int resposta`, `Type: int`) e a assinatura
de funções com até dois parâmetros posicionais obrigatórios de tipos primitivos
escritos (`int soma(int a, int b)`) ou getters de topo com retorno primitivo
escrito (`int get resposta`, `Type: int`). A referência
precisa ser uma expressão identificadora, sem declaração local ou parâmetro
homônimo em qualquer escopo da unidade. Vínculos ambíguos, aliases,
funções genéricas ou com parâmetros opcionais/nomeados, tipos inferidos de
inicializador e prefixos sombreados por nomes locais ainda não geram esse
resultado. Sem SDK, permanecem as respostas sintáticas anteriores. Cada
requisição carrega o texto vigente do editor por geração em memória;
`Program`, `Interner`, AST e `TypeTable` são descartados ao responder.
Documentos importados também abertos entram na mesma geração com seus textos
vigentes; as outras dependências são lidas do disco. Nenhuma cópia dessas
fontes fica retida depois da consulta.
`didChange` antigo e `didClose` preservam as garantias de versão. O intervalo
de definição em outro arquivo é convertido com as linhas **desse arquivo**.
Teste: `cargo test -p dartforge-lsp --test semantica --locked`.

`textDocument/references` devolve a declaração primeiro e depois os usos em
ordem de (URI, offset), honrando `context.includeDeclaration`. No próprio
documento valem os mesmos casos seguros da definição acima (tipo, variável,
função ou getter de topo únicos, sem diretivas que tragam outros nomes, sem
padrões e sem sombras). Entre documentos abertos, o símbolo resolve para o
único dono importado por import relativo simples, sem prefixo, `show`/`hide`,
`deferred`, condição, `export`, `part` ou `dart:`/`package:`; cada aberto
contribui com os usos só quando nada mais pode trazer o nome — arquivo em
dúvida (diretiva complexa, padrão, sombra, declaração local homônima, import
simples para fora dos abertos ou outro aberto que declare o nome) é pulado,
e dono zero ou duplo devolve vazio. Cada árvore é temporária por pedido e
liberada antes da próxima, como em `workspace/symbol`: só os documentos
abertos são lidos, nunca o disco por tecla, mantendo o platô de memória por
edição. Teste: `cargo test -p dartforge-lsp --test referencias --locked`.

### Consultas semânticas por requisição (completar, renomear, ações)

As três capacidades abaixo leem tipos e resoluções da **inferência comum**
de `crates/types` (`BodyInferrer`, tabelas `BodyTypes`: `get_type`,
`get_resolved`), nunca de uma inferência paralela no LSP. Cada requisição
monta uma `Consulta` transitória (`crates/lsp/src/consulta.rs`): programa
carregado com os textos vigentes dos documentos abertos (os demais arquivos
vêm do disco), outline, tabela de tipos e corpos inferidos só das bibliotecas
necessárias; tudo é descartado ao responder. Um arquivo `part of` entra
pela biblioteca dona (URI escrita, ou o arquivo do projeto que declara o
`part` na forma `part of nome;`), o que vale também para `definition` e
`hover` semânticos; parte que a dona não declara entra sozinha. Dois ganchos da inferência,
desligados no compilador, existem para o LSP: `sonda_escopo` captura o
escopo léxico (locais, parâmetros, parâmetros de tipo, classe/extensão
envolvente, `this`) no identificador pedido, e `registrar_locais` guarda,
para cada expressão que lê ou escreve um local, o offset da declaração.
Sem SDK, completar responde `null`, renomear recusa e as ações ficam só nas
sintáticas. Custo medido (binário release, projeto pequeno com o SDK 3.6.2
real, processo novo por medida): completar ~70–100 ms, renomear ~70–100 ms,
ação de importar ~135 ms na primeira vez (inclui o índice de nomes do SDK);
o tempo é dominado pela carga das bibliotecas `dart:`.

`textDocument/completion` (`triggerCharacters: ["."]`). O ponto de
digitação quase nunca analisa (`a.` sem nome, `a.ca` sem `;`), e o parser
recupera por comando: o comando incompleto some da árvore. Antes da
análise, o nome sob o cursor vira um identificador sentinela e, se preciso,
recebe um fecho curto (`;`, `)`, `);`, `));`, `]`, `});` …); vale a
variante que põe o sentinela numa expressão com o menor número de
diagnósticos. Então:

* `alvo.▮` — membros de instância pelo tipo estático do alvo
  (`get_type`), pela busca de membros de `crates/types`
  (`MemberResolver::lookup_member`), com genéricos substituídos
  (`List<String>.first` mostra `String`), herdados, de mixins, interfaces
  e de extensões aplicáveis; parâmetro de tipo usa o limite; `dynamic` usa
  `Object`; registros mostram `$1`… e os nomeados. Nome de classe
  (`A.▮`): estáticos e construtores nomeados. Prefixo de import
  (`p.▮`): o espaço de nomes do prefixo.
* nome simples — locais e parâmetros visíveis (os declarados adiante e os
  de blocos fechados não aparecem), parâmetros de tipo, membros da classe
  envolvente (de instância só fora de contexto estático), declarações de
  topo, importados sem prefixo, os prefixos e palavras-chave pelo contexto
  (comando, expressão, membro de classe, topo). Numa lista de argumentos,
  os parâmetros nomeados ainda não passados (`nome: `) vêm primeiro.
* privado de outra biblioteca nunca aparece; nada em comentário, texto de
  string ou número (interpolação é código).

Os itens seguem o formato do servidor do Dart: `label` `met(…)`/`met()`
para funções, `detail` `(int x, {String? nome}) → void` ou o tipo, `kind`
(2 método, 3 função, 4 construtor, 5 campo, 6 variável, 7 classe, 9
prefixo, 10 getter, 13 enum, 14 palavra-chave, 20 constante de enum, 25
parâmetro de tipo), `textEdit` sobre o prefixo digitado. A ordem é estável
— grupo (nomeados, locais, membros, biblioteca, importados, prefixos,
palavras-chave) e nome — e `sortText` a repete; o filtro é pelo prefixo,
sem diferenciar maiúsculas de minúsculas. Teste: `cargo test -p
dartforge-lsp --test completar --locked`.

`textDocument/prepareRename` e `textDocument/rename` (`renameProvider:
{prepareProvider: true}` quando o cliente anuncia `prepareSupport`). O
projeto é o diretório mais próximo com `pubspec.yaml` (sem ele, o do
arquivo); todos os `.dart` dele (sem ocultos, `build` nem subpacotes), menos
as partes, entram como bibliotecas de entrada de uma só carga
(`load_lenient_entradas` em `crates/elements`), para que as bibliotecas que
*importam* a declaração também sejam vistas; os corpos do projeto são
inferidos com `registrar_locais`. O que o cursor denota vem de
`get_resolved`/`declaracao_local`:

* local, parâmetro ou função local — a declaração e as expressões que a
  referem (inclusive em interpolação e como alvo de atribuição); num
  parâmetro nomeado, também os rótulos `nome:` nas chamadas da função;
* membro de classe — a família ligada por sobrescrita na hierarquia (sobe e
  desce por `extends`, `with`, `implements` e `on` até fechar), getter e
  setter juntos, cada uso resolvido para um membro da família (por
  instância, `this` implícito, `super`, cascata), parâmetros `this.x`,
  inicializadores `x = e` e os rótulos dos `this.x` nomeados; membro de
  extensão e estático são só os do dono; constantes de enum inclusas;
* declaração de topo — a declaração (getter e setter homônimos juntos), os
  usos resolvidos com ou sem prefixo, as anotações de tipo (resolvidas pelo
  escopo da biblioteca, respeitando parâmetros de tipo homônimos), os
  construtores escritos com o nome da classe, `show`/`hide` e metadados
  `@nome`.

Recusa com o código `-32010` (o `RenameNotValid` do servidor do Dart):
identificador malformado, palavra reservada, identificador embutido como
nome de tipo, elemento do SDK ou de pacote fora da raiz (inclusive a
sobrescrita de um membro deles, como `toString`), local que colidiria com
outro do mesmo corpo ou sombrearia um uso, membro já existente na família,
nome já declarado na biblioteca (ou numa que usa o elemento) e nome
público que viraria privado com usos em outra biblioteca. Construtores
nomeados, prefixos de import e parâmetros de tipo ainda são recusados.
`prepareRename` em espaço, palavra-chave ou literal devolve `null`. As
edições saem como `WorkspaceEdit.changes`, convertidas com as linhas do
texto aberto ou, para arquivo fechado, do arquivo no disco. Uso dentro de
um comando que não analisa não é visto (a recuperação do parser o
descarta). Teste: `cargo test -p dartforge-lsp --test renomear --locked`.

`textDocument/codeAction` (`codeActionKinds: ["quickfix"]`, honra
`context.only`):

* `Insert ';'` (`quickfix.insertSemicolon`) para o `expected_token`
  "Expected to find ';'." publicado, inserindo no fim do intervalo do
  diagnóstico, com o diagnóstico na ação — como o `dart.fix.insertSemicolon`.
* `Import library '…'` para um nome indefinido no intervalo: identificador
  sem resolução na inferência e fora do escopo da biblioteca, ou nome de
  tipo não encontrado (sem parâmetro de tipo homônimo). Candidatas: as
  bibliotecas públicas do SDK que o declaram (`quickfix.import.librarySdk`;
  índice de nomes de topo das bibliotecas sem `_` e das suas partes, montado
  na primeira vez e de tamanho fixo) e as do projeto
  (`quickfix.import.libraryProject1`): import relativo, ou `package:` quando
  o arquivo está fora de `lib/` e o alvo dentro. A diretiva entra na ordem
  (`dart:`, `package:`, relativas), depois de `library`, ou no topo.

Dos códigos semânticos publicados (`crates/analise/verificados.txt`), os
três de enum (`enum_constant_same_name_as_enclosing`, `enum_with_name_values`,
`values_declaration_in_enum`) não têm correção rápida no `dart
language-server` 3.6.2 (conferido), e este servidor também não. A lista
cresceu para 50 códigos em 2026-09-26 (entre eles `unused_local_variable` e
`unused_element`, que o LSP calcula sem tipos); as correções que o servidor
oficial oferece para eles ainda não existem aqui. Teste:
`cargo test -p dartforge-lsp --test acoes --locked`.

Implementadas: `initialize` (com `serverInfo`), `initialized`, `shutdown`,
`exit` (0 após `shutdown`, 1 sem), `$/cancelRequest`,
`textDocument/didOpen`/`didChange` (incremental e integral)/`didClose`,
`textDocument/publishDiagnostics` (`severity`, `source: "dartforge"`,
`version`), `textDocument/documentSymbol`, `workspace/symbol`,
`textDocument/definition`, `textDocument/references`, `textDocument/hover`,
`textDocument/completion`, `textDocument/prepareRename`,
`textDocument/rename`, `textDocument/codeAction`, `dartforge/dormir`
(gancho de teste do cancelamento em execução; clientes reais nunca enviam).

Pendentes no completar: sugestões de nomes ainda não importados (o Dart as
oferece com import automático), `completionItem/resolve` com documentação,
snippets, ordenação por relevância (o Dart pondera por uso), filtro
aproximado (só prefixo aqui) e contexto de tipo (em posição de tipo a lista
traz também valores). No renomear: construtores nomeados, prefixos de
import, parâmetros de tipo, rótulos de parâmetros nomeados de sobrescritas,
comentários de documentação (`[nome]`), renomear o arquivo junto com a
classe e a detecção completa de conflitos por escopo (a de hoje é
conservadora por corpo). Nas ações: as demais correções e assistências do
Dart (criar classe, remover variável não usada…), que dependem de
diagnósticos ainda não publicados.

Explicitamente fora deste brief: definição de variáveis/funções locais e demais nomes importados,
formatação e `diagnosticProvider` por
requisição (o servidor empurra diagnósticos; não atende pull). Diagnósticos
semânticos (nomes não resolvidos, erros de tipo) chegam depois via `crates/types`,
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
`tests/completar.rs`, `tests/renomear.rs` e `tests/acoes.rs` falam
JSON-RPC com o `Servidor` semântico sobre um projeto temporário
(`pubspec.yaml`, `package_config.json`) e um SDK mínimo em disco
(`tests/comum`), sem depender do Dart instalado; cobrem código incompleto
(`a.` solto, `a.ca` sem `;`, chamada sem `)`, comando quebrado no corpo).

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
