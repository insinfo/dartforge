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
              │  │analisador │  │  trait Analisador (parser + verificadores locais)
              │  └───────────┘  │
              └──┬─────────▲────┘
                 │ pedido  │ resultado (uri, versão, diagnósticos)
              ┌──▼─────────┴────┐
              │     tipado      │  thread 3: análise do `dartforge analyze`
              └────────┬────────┘  por pacote (ver "Diagnósticos tipados")
                       │
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
trabalho relevante. A análise tipada (seção seguinte) também não usa
debounce: roda noutra thread, coalesce as edições que chegam enquanto está
ocupada e para no próximo ponto de cancelamento quando o texto muda.

## Diagnósticos tipados (fluxo contínuo)

Cada `didOpen`/`didChange` produz **duas** publicações para o documento:

1. **Imediata**, no próprio `bombear`: o parser na versão de linguagem do
   arquivo mais os verificadores de `crates/analise` que olham só o arquivo
   (duplicatas, locais não usados, `external`, aridade de operador…), já
   pela regra de publicação e pelos `// ignore:`. Custa milissegundos.
2. **Tipada**, em segundo plano (`crates/lsp/src/tipado.rs`): a mesma
   análise do `dartforge analyze` — `dartforge_paridade::analise::Motor`
   (carga, outline, inferência de `crates/types`, todos os verificadores de
   `crates/analise`, imports não usados, diretivas sem alvo) seguida de
   `dartforge_paridade::publicaveis` (sintaxe sempre; semântica só com
   código em `crates/analise/verificados.txt`; `analysis_options.yaml` e
   `// ignore:`). Nenhuma regra é reescrita no LSP: a publicação tipada é a
   sintaxe do parser mais a semântica publicada do motor. Conferido com o
   exemplo de medição abaixo: nos arquivos medidos, a lista publicada é
   idêntica (código, linha, coluna) à do `Motor` + `diagnosticos_json`,
   que é o caminho do `dartforge analyze`.

Regras do fluxo:

* **Um programa por pacote.** O pedido marca o pacote do documento (a raiz
  com `pubspec.yaml`, como no `analyze`). A análise inclui **todos** os
  documentos abertos daquele pacote (e a biblioteca dona de cada parte
  aberta) num único `Motor::analisar_com`, com o texto aberto valendo mais
  que o disco — inclusive para arquivos só importados. Por isso editar um
  arquivo importado (aberto, sem salvar) republica os diagnósticos de quem o
  importa, direta ou transitivamente, se estiver aberto. Fechar um documento
  reanalisa os outros abertos do pacote com o texto do disco.
* **Versão.** Cada resultado leva a versão do texto analisado. O servidor só
  publica se ela ainda é a vigente; senão descarta
  (`Servidor::tipados_descartados`). Os resultados são drenados depois das
  notificações do mesmo `bombear`, então uma mudança já recebida descarta o
  resultado que ela tornou velho antes de ele chegar ao editor. Documento
  fechado também descarta.
* **Edições rápidas.** Marcar um pacote já marcado não enfileira outra
  análise: uma rajada vira uma análise da última versão. A análise em curso
  consulta, entre as fases do motor (depois da carga, antes dos tipos e
  antes de cada biblioteca na inferência de corpos), se o pacote foi marcado
  de novo; se foi, para e recomeça com o texto novo.
* **Memória.** O trabalhador retém o `Motor` (layout do SDK e nomes das
  bibliotecas) e uma cópia do texto vigente de cada documento aberto (a
  cópia sai no `didClose`). Programa, árvores e tabela de tipos vivem uma
  análise. O motor usa o cache do SDK em disco (`SdkCache`, o mesmo do
  `analyze`, em `DARTFORGE_CACHE_DIR` ou `target/dartforge`).
* **Sem SDK** (`AnalisadorSemantico::novo(None)`, ou SDK não descoberto) ou
  com `DARTFORGE_LSP_TIPADO=0`, só a publicação imediata existe. Pânico na
  análise tipada é contido: fica a imediata.
* O laço do binário acorda pelo gancho `Servidor::ao_ter_diagnosticos`;
  testes e medições usam `Servidor::aguardar_diagnosticos(limite)`.

Latência medida (2026-09-27, Linux, release, SDK 3.6.2, cache do SDK
quente; `cargo run --release -p dartforge-lsp --example
latencia_diagnosticos -- <arquivos>`: pelo protocolo, abre o arquivo,
espera a publicação tipada, aplica 10 edições incrementais esperando cada
uma e uma rajada de 20 sem esperar):

| Arquivo (corpus) | Tamanho | Imediata (mediana) | Tipada após edição (mediana; mín–máx) | Rajada de 20 |
|---|---|---|---|---|
| `linguagem/mixin/superclass_test.dart` | 6,5 KB, 218 linhas | 0,4 ms | 28 ms (28–35) | 1 publicação tipada, 48 ms |
| `pacotes/expect/lib/expect.dart` | 36 KB, 1.027 linhas | 2,2 ms | 34 ms (32–40) | 1 publicação tipada, 80 ms |
| `linguagem/void/void_type_usage_test.dart` | 74 KB, 2.256 linhas | 2,0 ms | 35 ms (34–47) | 1 publicação tipada, 106 ms |
| `linguagem/generic/super_bounded_types_error_test.dart` | 196 KB, 3.186 linhas | 11 ms | 56 ms (54–63) | 1 publicação tipada, 297 ms |

A primeira análise da sessão com o cache do SDK frio (construído na hora)
levou 227 ms no arquivo de 6,5 KB. Depois de fechar os documentos, o vivo
acima da base ficou em 0,03 MiB (alocador contador). Na rajada, o tempo é
dominado pelas 20 publicações imediatas (o parser do arquivo inteiro a cada
tecla), não pela tipada. Os arquivos do corpus não importam pacotes
grandes; num projeto com muitas dependências a carga cresce com elas.

Limitações: o custo de cada análise cresce com o número de documentos
abertos do pacote (todos entram no programa, e cada biblioteca aberta tem
os corpos inferidos); dependentes **fechados** não são analisados (não há
publicação para eles); um arquivo importado de outro pacote (dependência
por caminho) editado sem salvar vale para a carga, mas não marca o pacote
dos que o importam; a publicação imediata de uma nova versão substitui a
tipada anterior até a nova tipada chegar (os códigos que dependem de tipos
somem por algumas dezenas de ms a cada edição); as ações de código usam só
os diagnósticos imediatos.

## Capacidades

`textDocument/documentSymbol` lista declarações de topo e membros a partir da
AST do documento aberto. Usa `DocumentSymbol[]` hierárquico quando o cliente
anuncia `hierarchicalDocumentSymbolSupport`; caso contrário devolve
`SymbolInformation[]` plano, como exige o contrato do LSP. Os intervalos são
UTF-16. A árvore é descartada depois de cada pedido, mantendo o platô de
memória por edição. Edições `didChange` com versão antiga são ignoradas sem
republicar diagnósticos. Testes direcionados: `cargo test -p dartforge-lsp
--test simbolos --locked`.

`workspace/symbol` busca nos documentos abertos **e** nos arquivos `.dart`
dos projetos do workspace: as raízes do `initialize` (`workspaceFolders`,
senão `rootUri`/`rootPath`) e o projeto (diretório com `pubspec.yaml`) de
cada documento aberto, sem diretórios ocultos, `build` nem subpacotes. O
texto aberto vale mais que o disco; cada árvore é descartada antes da
próxima (nada fica indexado entre pedidos). O casamento é aproximado
(`crates/lsp/src/aproximado.rs`, no espírito do `FuzzyMatcher` do Dart):
prefixo, contém e iniciais de palavras (`cR` acha `CarregadorRemoto`); os
que contêm a consulta vêm antes das subsequências, e entre iguais a ordem é
(URI, posição). O resultado é `SymbolInformation[]`, no intervalo do nome.

### Definição, hover e referências pela identidade da declaração

Com SDK, `textDocument/definition`, `textDocument/hover` e
`textDocument/references` saem do mesmo modelo do renomear
(`crates/lsp/src/projeto.rs`): o que a posição denota vem das tabelas da
inferência comum (`get_resolved`, `declaracao_local`, `tipos_de_locais`) e
dos namespaces de `crates/elements` (`lookup`, `lookup_prefixed`), nunca de
uma resolução paralela por nome. Formas cobertas (matriz em
`tests/navegacao_semantica.rs`):

| Forma | Definição | Hover |
| --- | --- | --- |
| local, parâmetro (promovido) | a declaração | `num n` + `Type: int` (tipo da referência, com promoção) |
| função local | a declaração | `int dobro(int v)` |
| membro por instância, `this` implícito, `super`, cascata | o membro resolvido para o receptor (a sobrescrita, não a família) | assinatura completa (`void met(int x, {int y = 0, required String nome})`); campo e getter com `Type:` já substituído (`T valor` + `Type: int`) |
| construtor nomeado (`A.nome()`, `new A.nome()`) e sem nome escrito (`A()`) | o construtor; sem construtor escrito, a classe | `Caixa<T> Caixa.vazia(T v)` |
| classe, mixin, enum, extension type, extensão, typedef | a declaração | `abstract class B<T> extends A with M implements I` (modificadores, parâmetros de tipo, supertipos como no `ElementDisplayStringBuilder`) |
| variável, função, getter de topo; importados com ou sem prefixo | a declaração (outro arquivo, aberto ou no disco, ou o SDK) | `int x` + `Type: int`; `int soma([int a = 0])`; `int get g` + `Type: int` |
| constante de enum, membro de extensão | a declaração | `Cor azul` + `Type: Cor` |
| prefixo de import | o `as p` da diretiva | — (o Dart também não mostra) |
| parâmetro de tipo | o parâmetro mais interno com o nome | `T extends Bound` |
| referência `[nome]`, `[A.b]` num comentário de documentação | resolvida no escopo da declaração documentada (parâmetros, parâmetros de tipo, membros da classe, biblioteca) | como a declaração |

O hover inclui a documentação (`///` ou `/** */`, limpa, depois de `---` em
Markdown); um membro sobrescrito sem documentação própria mostra a do
primeiro membro sobrescrito que a tem, como o analyzer. Partes entram pela
biblioteca dona e a definição cruza parte ↔ dona; o texto aberto vale mais
que o disco (inclusive para o dono da declaração). Definição e hover
carregam só a biblioteca do documento (e o que ela importa) e inferem só os
corpos dela; o literal de URI de diretiva continua pela regra sintática
(arquivo existente). Prefixo que colide com declaração de topo
(`prefix_collides_with_top_level_member`) não é resolvido. Sem SDK, ou se a
posição não resolve, valem as respostas sintáticas conservadoras descritas
acima. Testes: `cargo test -p dartforge-lsp --test navegacao_semantica
--test semantica --locked`.

`textDocument/references` carrega o projeto inteiro (como o renomear) e
devolve a declaração (se `context.includeDeclaration`) e os usos do mesmo
elemento em **todas** as bibliotecas do projeto, abertas ou só no disco,
ordenados por (URI, offset); intervalos de arquivo fechado vêm do texto do
disco. A identidade é a da declaração: homônimos de outras bibliotecas,
locais que sombreiam, e parâmetros ou membros de mesmo nome não entram; um
membro de instância abrange a família ligada por sobrescrita (como o
`getHierarchyMembers` do analyzer), e as declarações dos outros membros da
família não contam como usos. Entram também `show`/`hide`, metadados,
rótulos `nome:` de parâmetros nomeados e referências `[nome]` de
documentação. Política de dependências: um elemento do SDK ou de pacote tem
a declaração devolvida (no arquivo do SDK/pacote), mas os usos só são
procurados no projeto. Sem SDK, vale a regra conservadora dos documentos
abertos descrita acima.

### Consultas semânticas por requisição (completar, renomear, ações)

As três capacidades abaixo leem tipos e resoluções da **inferência comum**
de `crates/types` (`BodyInferrer`, tabelas `BodyTypes`: `get_type`,
`get_resolved`), nunca de uma inferência paralela no LSP. Cada requisição
usa uma `Consulta` (`crates/lsp/src/consulta.rs`): programa carregado com os
textos vigentes dos documentos abertos (os demais arquivos vêm do disco),
outline, tabela de tipos e corpos inferidos só das bibliotecas necessárias.
A do completar (texto com sentinela) é descartada ao responder; a de
definição, hover, referências, renomear e ações fica na sessão limitada
descrita abaixo. Um arquivo `part of` entra
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

### Sessão semântica limitada

`crates/lsp/src/sessao.rs` guarda **um** programa carregado (com outline,
tipos e corpos) e o reaproveita nas consultas seguintes enquanto nada do
que ele leu mudou: `hover` e `definition` (escopo da biblioteca),
`references`, `prepareRename` e `rename` (projeto inteiro, que também serve a
uma consulta de biblioteca do mesmo projeto) e as ações. A política de
memória é explícita:

* no máximo uma entrada (não há cache por arquivo nem por versão);
* `didOpen`, `didChange` e `didClose` descartam a entrada na hora (o `drop`
  acontece no despacho da notificação) — nenhuma árvore de versão velha
  fica presa;
* antes de reaproveitar, a chave é conferida: versão e tamanho de cada
  documento aberto, data e tamanho de cada arquivo lido do disco (SDK
  incluído) e a lista de `.dart` do projeto (arquivo novo pode satisfazer um
  import); disco alterado por outra ferramenta recarrega;
* só retém programa cuja fonte somada cabe no orçamento
  `DARTFORGE_LSP_SESSAO_MIB` (MiB de fonte; padrão 8; `0` desliga). Acima
  dele, o programa vale só para a consulta e cai com a resposta.

Medida (2026-09-27, Linux, release, SDK 3.6.2 real; `cargo run --release -p
dartforge-lsp --example sessao_semantica -- <arquivo> 20`: 20 posições de
identificador com `hover` + `definition`, 3 `references`, uma edição e a
primeira consulta depois dela; latência e memória viva do alocador
contador, juntas):

| Arquivo (pub-cache) | Modo | hover (mediana; máx) | definition | references | 1ª após edição | fonte retida | vivo retido | vivo após edição | cargas / reusos |
|---|---|---|---|---|---|---|---|---|---|
| `args-2.7.0/lib/src/arg_parser.dart` (15 KB) | sem sessão | 81,6; 102,6 ms | 72,6 ms | 73,0 ms | 124,1 ms | 0 | 0 | 0 | 44 / 0 |
| | com sessão | 0,3; 97,9 ms | 0,5 ms | 0,4 ms | 82,2 ms | 2,5 MiB | 21,1 MiB | 0,0 MiB | 3 / 41 |
| `collection-1.19.1/lib/src/iterable_extensions.dart` (32 KB) | sem sessão | 58,1; 80,3 ms | 57,5 ms | 120,1 ms | 69,2 ms | 0 | 0 | 0 | 44 / 0 |
| | com sessão | 0,9; 87,8 ms | 1,5 ms | 0,7 ms | 112,0 ms | 2,7 MiB | 25,9 MiB | 0,0 MiB | 3 / 41 |

Leitura: a sessão troca ~60–120 ms por consulta por ~1 ms enquanto o texto
não muda, ao custo de ~8–10× a fonte carregada em memória viva **enquanto**
a versão vale (o orçamento padrão de 8 MiB de fonte limita isso a algumas
dezenas de MiB); a primeira consulta depois de uma edição paga a carga
inteira de novo, e o vivo volta à base (0,0 MiB) a cada edição. Os três
"máximos" altos são as três cargas (biblioteca, projeto, após a edição).
Testes: `cargo test -p dartforge-lsp --test sessao --test sessao_memoria
--locked` (reuso, invalidação por edição, por arquivo alterado e por
arquivo novo, orçamento zero; retenção e volta à base em 20 edições).

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
* posição de tipo (`Str▮ x`, `List<▮>`, `void f(▮ a)`, `p.▮ x`) — só
  tipos: classes, mixins, enums, extension types e typedefs do escopo (ou
  do prefixo), parâmetros de tipo em escopo, prefixos, `dynamic`/`void`, e
  as palavras-chave de declaração quando o tipo abre uma declaração ou um
  comando; nunca valores (locais, variáveis, funções, `null`…);
* nomes públicos de bibliotecas **ainda não importadas** que casam com o
  digitado (não com a lista vazia): do SDK (índice das bibliotecas públicas,
  `dart:core` fora) e do projeto (índice incremental por arquivo em
  `crates/lsp/src/indice.rs`, um projeto por vez, só nomes e offsets), com
  `detail` `Auto import from 'dart:math'` e `additionalTextEdits` com a
  diretiva no lugar certo (a mesma regra da ação de importar: `dart:`,
  `package:`, relativas; relativo ou `package:` conforme `lib/`). Em posição
  de tipo, só tipos. Numa parte não há (o `import` iria para a dona). Acima
  de 200 desses itens, a lista sai com `isIncomplete: true`;
* privado de outra biblioteca nunca aparece; nada em comentário, texto de
  string ou número (interpolação é código).

Os itens seguem o formato do servidor do Dart: `label` `met(…)`/`met()`
para funções, `detail` `(int x, {String? nome}) → void` ou o tipo, `kind`
(2 método, 3 função, 4 construtor, 5 campo, 6 variável, 7 classe, 9
prefixo, 10 getter, 13 enum, 14 palavra-chave, 20 constante de enum, 25
parâmetro de tipo), `textEdit` sobre o prefixo digitado. O filtro é aproximado
(`crates/lsp/src/aproximado.rs`): prefixo, contém, iniciais de palavras
(`vt` acha `valorTotal`) e subsequência que abre como o nome. A ordem é por
relevância e estável, e `sortText` a repete: o que começa com o digitado
(sem diferenciar maiúsculas) antes do que só casa por aproximação; depois o
grupo (nomeados, locais, membros — os herdados de `Object` por último —,
biblioteca, importados, prefixos, não importados, palavras-chave) e o nome.

`completionItem/resolve` (`resolveProvider: true`): cada item com declaração
conhecida leva `data` (`arquivo`, `inicio`); o resolve lê o comentário de
documentação da declaração (texto aberto ou disco; `///` ou `/** */`) e o
devolve em `documentation`, em Markdown se o cliente anuncia
`documentationFormat: ["markdown"]`. Sem `data`, o item volta como veio.

Snippets: quando o cliente anuncia `completionItem.snippetSupport` (e não
passa `completeFunctionCalls: false` nas `initializationOptions`), funções,
métodos e construtores entram com os parênteses e os parâmetros
obrigatórios como marcadores (`met(${1:x})$0`, nomeados obrigatórios como
`req(n: ${1:n})$0`, sem parâmetros `nada()$0`, parâmetros desconhecidos de
um nome não importado `f($0)`), com `insertTextFormat: 2`; se já há `(`
depois do nome, só o nome. Campos, getters e variáveis nunca levam
parênteses. Teste: `cargo test -p dartforge-lsp --test completar
--locked`.

`textDocument/prepareRename` e `textDocument/rename` (`renameProvider:
{prepareProvider: true}` quando o cliente anuncia `prepareSupport`). O
projeto é o diretório mais próximo com `pubspec.yaml` (sem ele, o do
arquivo); todos os `.dart` dele (sem ocultos, `build` nem subpacotes), menos
as partes, entram como bibliotecas de entrada de uma só carga
(`load_lenient_entradas` em `crates/elements`), para que as bibliotecas que
*importam* a declaração também sejam vistas; os corpos do projeto são
inferidos com `registrar_locais`. O que o cursor denota e as ocorrências vêm
do modelo comum (`crates/lsp/src/projeto.rs`, o mesmo das referências):

* local, parâmetro ou função local — a declaração e as expressões que a
  referem (inclusive em interpolação e como alvo de atribuição); num
  parâmetro nomeado, os rótulos `nome:` nas chamadas e, num método de
  instância, o parâmetro homônimo de todas as sobrescritas (com os usos e
  rótulos delas);
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
  `@nome`;
* construtor nomeado — a declaração, `A.nome()`, `new A.nome()`,
  `this.nome()`, `super.nome()` (pela superclasse), `factory … = A.nome`,
  constantes de enum `a.nome()` e metadados `@A.nome()`;
* prefixo de import — o `as p` de cada import com ele na biblioteca e cada
  `p.` escrito nela (expressões, tipos, metadados);
* parâmetro de tipo — a declaração e os usos cujo parâmetro mais interno
  com o nome é ele (um `<T>` de método sombreia o da classe);
* em todos os casos, as referências `[nome]`/`[A.nome]` dos comentários de
  documentação que resolvem para o elemento.

Com `renameFilesWithClasses: "always"` nas `initializationOptions` e o
cliente aceitando `documentChanges` com a operação `rename`, renomear uma
classe cujo arquivo segue o nome dela (`MinhaClasse` em `minha_classe.dart`)
renomeia o arquivo (operação depois das edições de texto) e corrige as
diretivas `import`/`export`/`part`/`part of` do projeto que o citam
(relativas ou `package:`); destino existente deixa o arquivo como está.

Recusa com o código `-32010` (o `RenameNotValid` do servidor do Dart):
identificador malformado, palavra reservada, identificador embutido como
nome de tipo ou prefixo, elemento do SDK ou de pacote fora da raiz
(inclusive a sobrescrita de um membro deles, como `toString`), e conflitos
por escopo léxico (bloco, `for`, `catch`, função; o bloco do corpo é o
escopo dos parâmetros): local duplicado no mesmo escopo, referência que
ficaria no escopo de outro local aninhado com o nome novo, uso de `novo`
(identificador ou tipo) no escopo que passaria a denotar o local, uso solto
de um topo ou membro que cairia no escopo de um local `novo`, uso de topo
dentro de uma classe com membro `novo`, `novo` solto no corpo da família que
passaria a denotar o membro, membro já existente na família, nome já
declarado na biblioteca (ou numa que usa o elemento), construtor ou estático
homônimo, prefixo que colidiria com nome visível, parâmetro de tipo irmão
ou tipo usado no escopo, e nome público que viraria privado com usos em
outra biblioteca. `prepareRename` em espaço, palavra-chave ou literal
devolve `null`. As edições saem como `WorkspaceEdit.documentChanges` (cada
documento com a versão vigente; fechado, `version: null`) quando o cliente
anuncia `workspace.workspaceEdit.documentChanges`, senão como `changes`,
convertidas com as linhas do texto aberto ou, para arquivo fechado, do
arquivo no disco. Uso dentro de um comando que não analisa não é visto (a
recuperação do parser o descarta). Teste: `cargo test -p dartforge-lsp
--test renomear --locked`.

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

Pendentes no completar: a relevância não pondera pelo tipo esperado nem
pelo uso (o Dart usa as duas coisas). Nas ações: as demais correções e
assistências do Dart (criar classe, remover variável não usada…), que
dependem de diagnósticos ainda não publicados.

Explicitamente fora deste brief: formatação e `diagnosticProvider` por
requisição (o servidor empurra diagnósticos; não atende pull). Os
diagnósticos semânticos publicados (os códigos de `verificados.txt`,
inclusive os que dependem de tipos) chegam pelo fluxo tipado descrito em
"Diagnósticos tipados"; os não verificados continuam fora do editor.

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
`tests/tipados.rs` usa o mesmo apoio para o fluxo tipado: código publicado
que depende de tipos (`non_bool_condition`) só na publicação tipada, só
códigos publicados e `// ignore:` respeitado, resultado de versão velha
descartado, rajada de edições coalescida numa análise, arquivo importado
editado republicando quem o importa, e `didClose` descartando resultado
pendente.

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
