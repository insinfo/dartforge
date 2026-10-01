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

Remedido em 2026-10-01 no Windows (release, SDK 3.6.2, 159 códigos
publicados; mesmo exemplo), com a publicação idêntica à do `dartforge
analyze` em todos (24, 0, 81 e 604 semânticos):

| Arquivo (corpus) | Imediata (mediana) | Tipada após edição (mediana; mín–máx) | Rajada de 20 |
|---|---|---|---|
| `linguagem/mixin/superclass_test.dart` | 0,5 ms | 55 ms (49–63) | 1 publicação tipada, 91 ms |
| `pacotes/expect/lib/expect.dart` | 2,0 ms | 57 ms (50–63) | 1 publicação tipada, 114 ms |
| `linguagem/void/void_type_usage_test.dart` | 3,3 ms | 51 ms (48–58) | 1 publicação tipada, 198 ms |
| `linguagem/generic/super_bounded_types_error_test.dart` | 14 ms | 68 ms (62–82) | 1 publicação tipada, 329 ms |

Vivo acima da base depois de fechar os documentos: 0,04 MiB.

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
| membro por instância, `this` implícito, `super`, cascata | o membro resolvido para o receptor (a sobrescrita, não a família) | assinatura completa (`void met(int x, [int y = 0])`; com três parâmetros ou mais, um por linha, como o `multiline` do analyzer); numa chamada, `Type:` com o tipo da invocação (`void Function(int, [int])`); campo e getter com `Type:` já substituído (`T valor` + `Type: int`) |
| construtor nomeado (`A.nome()`, `new A.nome()`) e sem nome escrito (`A()`) | o construtor; sem construtor escrito, a classe | `Caixa<T> Caixa.vazia(T v)` |
| classe, mixin, enum, extension type, extensão, typedef | a declaração | `abstract class B<T> extends A with M implements I` (modificadores, parâmetros de tipo, supertipos como no `ElementDisplayStringBuilder`) |
| variável, função, getter de topo; importados com ou sem prefixo | a declaração (outro arquivo, aberto ou no disco, ou o SDK) | `int x` + `Type: int`; `int soma([int a = 0])`; `int get g` + `Type: int` |
| constante de enum, membro de extensão | a declaração | `Cor azul` + `Type: Cor` |
| prefixo de import | o `as p` da diretiva | — (o Dart também não mostra) |
| parâmetro de tipo | o parâmetro mais interno com o nome | `T extends Bound` |
| referência `[nome]`, `[A.b]` num comentário de documentação | resolvida no escopo da declaração documentada (parâmetros, parâmetros de tipo, membros da classe, biblioteca) | como a declaração |

O hover inclui a documentação (`///` ou `/** */`, limpa, depois de `---` em
Markdown); um membro sobrescrito sem documentação própria mostra a do
primeiro membro sobrescrito que a tem, como o analyzer. Em Markdown, o
texto segue o `toHover` do Dart 3.6.2 (`lsp/handlers/handler_hover.dart`):
o bloco `dart` com a descrição (`(deprecated) ` antes, se a declaração tem
`@deprecated`/`@Deprecated`; `(new) ` antes do construtor de uma criação
sem `new`/`const`), `Type: \`T\``, a biblioteca do elemento não local em
itálico (`*package:x/y.dart*`, `*dart:core*`, ou o caminho relativo à raiz
para um arquivo fora de `lib/`, como o `_libraryInfo` do analyzer) e, depois
de `---`, a documentação. Um parâmetro mostra os delimitadores do tipo dele
(`[int b = 0]`, `{required int c}`); a declaração de uma constante de enum
não tem hover; os nomes de `show`/`hide` resolvem para o elemento exportado
(hover, definição, referências e renomear). Em texto puro, sem a linha da
biblioteca. Partes entram pela
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
| `analyzer-7.7.1/lib/src/dart/element/display_string_builder.dart` (18 KB; projeto de 1.607 arquivos), 10 posições | sem sessão | 59,9; 103,8 ms | 58,6 ms | 1.358,7 ms | 71,4 ms | 0 | 0 | 0 | 24 / 0 |
| | com sessão | 6,8; 84,8 ms | 7,6 ms | 1.174,8 ms | 70,0 ms | 0 (projeto acima do orçamento) | 19,4 MiB | 0,0 MiB | 5 / 19 |

No `analyzer` (projeto grande), a biblioteca cabe no orçamento e é
reaproveitada (a mediana de ~7 ms é quase toda a conferência da chave: a
lista de 1.607 arquivos e as datas dos lidos); o projeto inteiro passa de 8
MiB de fonte, então cada `references` recarrega (~1,2 s) e nada dele fica
retido — o orçamento funcionando como limite, não como cache.

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

`textDocument/codeAction` (`codeActionKinds: ["quickfix", "refactor"]`,
honra `context.only`). As correções respondem aos diagnósticos **publicados**
da versão vigente: os imediatos (recalculados no pedido) e os tipados que o
fluxo contínuo publicou para essa versão — o servidor guarda a última
publicação tipada de cada documento aberto (substituída a cada publicação,
removida no `didClose`) e não a usa se a versão mudou. As edições saem em
`documentChanges` com a versão do documento quando o cliente anuncia
`workspace.workspaceEdit.documentChanges`, senão em `changes`; cada ação de
correção leva o diagnóstico que corrige.

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

* Correções dos códigos semânticos publicados, com os títulos e espécies do
  servidor do Dart 3.6.2 (títulos e ids conferidos no
  `analysis_server.dart.snapshot` do SDK):

  | Código publicado | Ação | Espécie |
  | --- | --- | --- |
  | `unused_local_variable` | `Remove unused local variable` (a declaração, ou só a variável numa lista, e os comandos que só atribuem a ela) | `quickfix.remove.unusedLocalVariable` |
  | `unused_element` (função local, declaração de topo, membro de classe) | `Remove unused element` (as linhas da declaração, com o `///` de cima) | `quickfix.remove.unusedElement` |
  | `unnecessary_cast` | `Remove unnecessary cast` (e o parêntese que sobraria em volta de uma primária) | `quickfix.remove.unnecessaryCast` |
  | `unnecessary_non_null_assertion` | `Remove the '!'` | `quickfix.remove.nonNullAssertion` |
  | `invalid_null_aware_operator` | `Replace with '.'` / `Replace with '['` | `quickfix.replace.withNotNullAware` |
  | `instance_access_to_static_member` | `Change access to static using 'C'` (`p.C` se a classe é vista por prefixo) | `quickfix.change.toStaticAccess` |
  | `record_literal_one_positional_no_trailing_comma` | `Add trailing comma` | `quickfix.add.trailingComma` |
  | `assignment_to_final` (campo) | `Make field 'x' not final` (tira o `final` da declaração, ou troca por `var` sem tipo, no arquivo dela) | `quickfix.makeFieldNotFinal` |
  | `abstract_field_initializer` | `Remove initializer` e `Remove the 'abstract' keyword` | `quickfix.remove.initializer`, `quickfix.remove.abstract` |
  | `non_bool_condition` | `Add != null` (como o `AddNeNull` do Dart, depois da condição) | `quickfix.add.neNull` |
  | `uri_does_not_exist` (URI relativa `.dart`) | `Create file 'x.dart'`: operação `create` no `WorkspaceEdit`, vazio ou com o `part of` para uma parte; só ao cliente que anuncia `documentChanges` e `resourceOperations: create` | `quickfix.create.file` |

  Cada edição é conferida contra a árvore do texto vigente: diagnóstico que
  não corresponde ao nó esperado não gera ação.
* Assistência `Add type annotation` (`refactor.add.typeAnnotation`) num
  local `var x = e;` ou `final x = e;` (cursor na palavra-chave ou no nome),
  com o tipo da inferência comum; não quando o tipo é `dynamic` nem quando
  algum nome do tipo não é visível na biblioteca (a anotação não
  compilaria).

Dos códigos semânticos publicados (`crates/analise/verificados.txt`), os
três de enum (`enum_constant_same_name_as_enclosing`, `enum_with_name_values`,
`values_declaration_in_enum`) não têm correção rápida no `dart
language-server` 3.6.2 (conferido), e este servidor também não. Teste:
`cargo test -p dartforge-lsp --test acoes --locked` (inclui os negativos:
fora do intervalo, variável usada, publicação tipada de versão velha,
`dynamic`, cursor no inicializador, `context.only`).

Implementadas: `initialize` (com `serverInfo`), `initialized`, `shutdown`,
`exit` (0 após `shutdown`, 1 sem), `$/cancelRequest`,
`textDocument/didOpen`/`didChange` (incremental e integral)/`didClose`,
`textDocument/publishDiagnostics` (`severity`, `source: "dartforge"`,
`version`), `textDocument/documentSymbol`, `workspace/symbol`,
`textDocument/definition`, `textDocument/references`, `textDocument/hover`,
`textDocument/completion`, `textDocument/prepareRename`,
`textDocument/rename`, `textDocument/codeAction`, `completionItem/resolve`,
`textDocument/diagnostic` (só com o cliente que anuncia
`textDocument.diagnostic`), `textDocument/signatureHelp`,
`textDocument/documentHighlight`, `textDocument/implementation`,
`textDocument/typeDefinition`, `textDocument/foldingRange`,
`textDocument/selectionRange`, `textDocument/inlayHint`,
`textDocument/semanticTokens/full` e `/range`,
`textDocument/prepareTypeHierarchy`, `typeHierarchy/supertypes`,
`typeHierarchy/subtypes`, `textDocument/prepareCallHierarchy`,
`callHierarchy/incomingCalls`, `callHierarchy/outgoingCalls` (regras em
"Paridade com o servidor do Dart (3.6.2)"), `dartforge/dormir`
(gancho de teste do cancelamento em execução; clientes reais nunca enviam).

Pendentes nas ações: das assistências do Dart (`refactor.*`: extrair
método ou variável, embutir, converter corpo, mover para arquivo…), só a
anotação de tipo de local existe; das ações de fonte, só `Organize Imports`
(sem remover imports não usados: `unused_import` não é publicado ainda);
`Sort Members` e `Fix All` não existem.

**Diagnósticos puxados (LSP 3.17).** O cliente que anuncia
`textDocument.diagnostic` recebe o `diagnosticProvider`
(`interFileDependencies: true`, `workspaceDiagnostics: false`) e o servidor
deixa de empurrar `publishDiagnostics` para ele. `textDocument/diagnostic`
devolve o resultado tipado da versão vigente, senão o imediato; o
`resultId` é a versão (`"3"`, ou `"3.t"` quando já é o tipado), e um
`previousResultId` igual volta `unchanged` sem recalcular. Quando chega um
resultado tipado, o servidor pede `workspace/diagnostic/refresh` (se o
cliente anunciou `workspace.diagnostics.refreshSupport`) e o cliente puxa de
novo; a resposta do cliente é aceita em silêncio. Documento fechado não tem
diagnóstico. Cliente sem a capacidade continua no fluxo empurrado.
Teste: `tipados::diagnosticos_puxados_no_lugar_dos_empurrados`.

**Formatação: decisão (L08).** `documentFormattingProvider` não é
anunciado. O formatador de referência é o `dart_style` (`dart format`), com
regras que mudam por versão de linguagem ("tall style" a partir da 3.7); um
formatador que não seja byte a byte o do SDK reescreveria arquivos de forma
diferente do `dart format` do projeto e do CI dele. A capacidade só entra com
um porte do `dart_style` conferido contra ele num oráculo (como os demais
componentes); até lá, a formatação fica com o `dart format`. Os
diagnósticos semânticos publicados (os códigos de `verificados.txt`,
inclusive os que dependem de tipos) chegam pelo fluxo tipado descrito em
"Diagnósticos tipados"; os não verificados continuam fora do editor.

## Paridade com o servidor do Dart (3.6.2)

O alvo é o `dart language-server` do SDK 3.6.2 (o mesmo do oráculo dos
diagnósticos). As regras abaixo saem da fonte do `analysis_server` na tag
`3.6.2` do checkout de referência (`git show 3.6.2:pkg/analysis_server/…`,
caminhos relativos a `pkg/analysis_server/lib/src/`) e foram conferidas no
oráculo; onde o servidor atual (pós-3.6) mudou o comportamento e a mudança
é melhor para o editor, a escolha está dita.

### Oráculo de paridade

`crates/lsp/oraculo/oraculo.py` abre os mesmos arquivos nos dois servidores
por stdio (capacidades de cliente como as do VS Code: Markdown, snippets,
`documentChanges`, `prepareSupport`, `lineFoldingOnly`,
`hierarchicalDocumentSymbolSupport`), espera a análise inicial do Dart e pede,
em posições amostradas de identificadores (fora de comentários e strings, 12
por arquivo, espalhadas): `hover`, `definition`, `typeDefinition`,
`implementation`, `references`, `documentHighlight`, `prepareRename`,
`rename`, `codeAction` (assistências no cursor), `selectionRange`,
`prepareCallHierarchy`, `prepareTypeHierarchy` e `completion` (no meio do
nome, com até dois caracteres digitados, ou logo depois do `.` de um acesso a
membro); `signatureHelp` logo depois do `(` de chamadas (disparo automático)
e depois da primeira vírgula (invocado); por arquivo, `documentSymbol`,
`foldingRange`, `semanticTokens/full`, `inlayHint`, `documentLink` e
`formatting`; e `codeAction` no intervalo de cada diagnóstico que o Dart
publica. `crates/lsp/oraculo/resumo.py` normaliza (URIs sem
percent-encoding nem caixa, intervalos, conjuntos) e imprime a tabela.

Uso (o binário do DartForge com `DARTFORGE_SDK_LIB` apontando o SDK 3.6.2):

```text
python crates/lsp/oraculo/oraculo.py saida.jsonl target/release/dartforge-lsp.exe <projeto> …
python crates/lsp/oraculo/resumo.py saida.jsonl
```

Projetos medidos: cópias de `args-2.7.0`, `path-1.9.1` e
`string_scanner-1.4.1` do pub-cache (só `lib/`, `dev_dependencies` fora,
`dart pub get --offline`) e um projeto `app` com erros comuns (método,
getter e função indefinidos, argumento obrigatório faltando, `switch` não
exaustivo, membros abstratos sem implementação, variável não usada, cast
desnecessário).

Resultado (4 projetos, 36 arquivos, 410 posições; "igual" é o resultado
normalizado idêntico ao do Dart; antes = o servidor no início deste
trabalho, depois = o atual):

| Recurso | Antes | Depois | Observação |
|---|---|---|---|
| documentSymbol | 53% | 97% | espécies e nomes (`A.nome`) como o Dart |
| foldingRange | 0% | 100% | |
| semanticTokens (por token) | 0% | 100% | |
| inlayHint (por arquivo) | 17% | 50% | falta `InvalidType` e argumentos de tipo de chamada genérica |
| codeAction: correções p/ diag. do Dart | 17% | 75% | faltam `Add missing switch cases` e `Add required argument` |
| codeAction: ações de fonte | 1% | 0% | `Organize Imports` igual; `Fix All` e `Sort Members` não existem |
| hover (texto inteiro) | 17% | 86% | |
| hover (assinatura) | 86% | 97% | |
| definition | 95% | 96% | |
| typeDefinition | 28% | 95% | |
| implementation | 71% | 77% | o Dart procura no SDK inteiro |
| references | 56% | 58% | o Dart procura no SDK inteiro (aqui, o projeto carregado) |
| documentHighlight | 0% | 95% | |
| prepareRename | 95% | 96% | |
| rename | 88% | 89% | |
| codeAction: assistências no cursor | 8% | 26% | faltam Extract/Inline Method, Move to file, Convert to… |
| selectionRange | 0% | 48% | nós intermediários da árvore do analyzer |
| prepareCallHierarchy | 68% | 86% | |
| prepareTypeHierarchy | 22% | 76% | `range` do Dart é o nome; aqui a declaração |
| completion: alvo presente | 95% | 98% | |
| completion: mesmo top-1 | 46% | 63% | |
| completion: top-5 ≥ 60% comum | 48% | 52% | |
| signatureHelp (rótulo) | 53% | 99% | |
| workspace/symbol | 0% | 0% | o Dart inclui o SDK inteiro |

Latência (mediana no oráculo, ms, Dart / DartForge): hover 1,4 / 3,5 (era
3,5), completion 5,0 / 69,5 (era 67,6), signatureHelp 1,1 / 3,3,
documentHighlight 1,4 / 4,1, semanticTokens 2,1 / 4,0, inlayHint 1,7 / 3,4,
foldingRange 1,4 / 0,4. Com `examples/latencia_recursos.rs` (release,
Windows, 20 pedidos em `args/lib/src/arg_parser.dart`,
`args/lib/command_runner.dart` e `path/lib/src/context.dart`): hover parado
2,5–3,5 ms, completion parado 67–82 ms, signatureHelp 2,6–2,8 ms,
documentHighlight 3,2–4,1 ms, foldingRange 0,5–1,6 ms, hover logo após
edição 63–73 ms, completion logo após edição 74–88 ms. O hover e o
completar não pioraram; o completar continua dominado pela reinferência da
biblioteca a cada pedido.

### Regras por recurso

**Completar** (`lsp/handlers/handler_completion.dart`,
`services/completion/dart/*`). Além do descrito em "Consultas semânticas
por requisição":

* *relevância* — a do `RelevanceComputer`/`FeatureComputer` do Dart
  (`crates/lsp/src/relevancia.rs`): média ponderada das características
  tipo de contexto (o tipo que a posição espera — parâmetro do argumento,
  alvo da atribuição, tipo escrito da variável, retorno da função, `bool`
  da condição — contra o tipo do item: igual 1,0, subtipo 0,40, supertipo
  0,02, sem relação 0,13), espécie do elemento no local do completar (as
  tabelas estatísticas `relevance_tables.g.dart` do 3.6.2, geradas em
  `relevancia_tabelas.rs` por `crates/lsp/oraculo/gerar_tabelas.py`; o local
  é o papel da posição no nó pai: `Block_statement`,
  `ArgumentList_method_unnamed`, `PropertyAccess_propertyName`,
  `ReturnStatement_expression`, `VariableDeclaration_initializer`…; locais
  pela proximidade e membros pela distância de herança, `0,9^d`),
  palavra-chave (o alto da faixa no local), não importado (−1), nome com `$`
  e `noSuchMethod`; `⌊((média + 1) / 2) × 1000⌋`, e os argumentos nomeados
  com 900 (950 obrigatório). A ordem é a relevância (o digitado como
  prefixo antes do casamento aproximado; entre iguais, o grupo e o nome);
* nada no nome de uma declaração (variável com `var`/`final`/tipo,
  parâmetro com tipo, função, classe…), como o Dart;
* `Cor.▮` lista as constantes do enum; `show`/`hide` lista o que a
  biblioteca da diretiva exporta; `this.▮` num construtor lista os campos
  ainda não inicializados e `super.▮` os parâmetros do construtor da
  superclasse ainda não repassados;
* com `completionItem.labelDetailsSupport`, o rótulo é só o nome e
  `labelDetails` traz a assinatura curta (`(…) → int`, ` String`) e a
  biblioteca a importar, como o Dart faz com esse cliente.

Divergências conhecidas do completar: o Dart sugere também o que o SDK
inteiro declara sem import (`InternetAddress`… com `isNotImported`); aqui só
as bibliotecas `dart:` importáveis que o índice conhece. A latência
(~70 ms contra ~5 ms) vem da inferência da biblioteca refeita a cada pedido.

**Ajuda de assinatura** (`computer/computer_signature.dart`,
`lsp/handlers/handler_signature_help.dart`, `lsp/mapping.dart`
`toSignatureHelp`). `signatureHelpProvider` com `triggerCharacters: ["("]`
e `retriggerCharacters: [","]` (`lsp/constants.dart`). A lista é a
`ArgumentList` mais interna que contém o cursor (do `(` exclusive ao `)`
inclusive); uma `FunctionExpression` entre o cursor e a lista encerra a
busca (cursor no corpo de um *closure* argumento não mostra a chamada de
fora). Só chamadas de método ou função (`MethodInvocation`), criações de
instância (`InstanceCreationExpression`, com ou sem `new`) e invocações de
expressão cujo alvo é um identificador; getter ou campo que devolve função
não tem assinatura. Rótulo `nome(p1, [p2], {p3})`, com `nome` o nome do
método, ou o nome qualificado do tipo criado (`p.A.nome`), e cada
parâmetro `[required ]tipo nome[ = padrão]`, tipo como o elemento o vê (num
receptor genérico, substituído: `List<int>.add` → `add(int value)`; numa
criação, pelos argumentos do tipo criado) e o padrão como escrito. Um só
`SignatureInformation`, `activeSignature: 0`; documentação do elemento (a do
hover, Markdown se o cliente aceita). Disparo automático (`triggerKind: 2`,
`isRetrigger: false`) só responde quando o cursor está logo depois do `(`
que abre a lista. Escolha: o 3.6.2 manda `activeParameter: -1` (nenhum
parâmetro destacado); o servidor atual calcula o parâmetro ativo (o do
argumento sob o cursor, nomeado pelo nome, posicional pela posição; entre
argumentos, o próximo posicional) e manda o tamanho da lista quando não há
(`null` ao cliente com `noActiveParameterSupport`). Seguimos o atual: no VS
Code, o parâmetro sendo digitado fica destacado. Assinatura de argumentos de
tipo (`List<▮>`) não existe aqui.

**Destaques no documento** (`lsp/handlers/handler_document_highlights.dart`,
`DartUnitOccurrencesComputer`): as ocorrências do elemento sob o cursor no
próprio arquivo, a declaração inclusive, sem `kind`. Aqui, pela identidade
do `references` (`Projeto::ocorrencias`) restrita à unidade; num membro de
instância a família inteira (o Dart separa o membro sobrescrito do que
sobrescreve).

**Implementações** (`lsp/handlers/handler_implementation.dart`,
`search/type_hierarchy.dart` `TypeHierarchyComputerHelper`): o elemento sob
o cursor; se é classe (ou construtor dela), os subtipos transitivos; se é
membro de instância de classe, a declaração do membro em cada subtipo que o
declara (ou o recebe de um mixin; não os intermediários sem declaração);
local, topo que não é classe e membro de extension type não têm
implementação (lista vazia). Os locais são o nome de cada declaração. A
busca cobre o projeto carregado (as bibliotecas do projeto e o que elas
importam).

**Definição do tipo** (`lsp/handlers/handler_type_definition.dart`): o nó
sob o cursor dá o tipo — nome de tipo, a própria classe; declaração de
variável, de parâmetro ou de `for-in`, o tipo declarado; expressão, o tipo
estático (com promoção); método ou função, nada (é tipo função) — e só tipo
de interface (classe, enum, mixin, extension type; `int?` vai para `int`)
tem destino: o nome da declaração do tipo. Sem destino, lista vazia. Sem
`LocationLink` (o cliente do oráculo não anuncia `linkSupport`).

**Dobras** (`computer/computer_folding.dart`,
`lsp/handlers/handler_folding.dart`): as regiões em ordem de visita da
árvore — anotações (do fim do nome da primeira ao fim da última), corpo de
classe e de mixin (do fim do nome ao `}`), de enum, extensão e extension
type (entre as chaves), construtor, método e função (do fim do nome ao
fim), listas de parâmetros e de argumentos (entre os parênteses; se a linha
do `(` já tem região, a partir do primeiro parâmetro ou argumento), literais
de lista, conjunto, mapa e registro, blocos de `if`/`else`/`while`/`do` (do
`{` ao fim do último comando, ou do último comentário antes do `}`), `for`
e corpo de *closure* (do `{` ao `}`), `switch` e cada caso (do `:` ao fim),
`switch` de expressão e cada caso (do `=>` ao fim), `assert` e strings de
várias linhas —, depois as diretivas (da palavra-chave da primeira ao fim da
última, `imports`) e os comentários (`/* */` do fim da primeira linha;
`//`/`///` consecutivos do mesmo tipo e sem linha em branco, do fim do
primeiro ao fim do último; `comment`). Só regiões de mais de uma linha;
duas não começam na mesma linha. Ordenadas pelo início; com
`lineFoldingOnly`, a que termina na linha em que a seguinte começa (sem
contê-la) termina na linha anterior, e some se ficar com uma linha.

**Faixas de seleção** (`computer/computer_selection_ranges.dart`): os nós
que contêm o cursor, do mais interno ao mais externo, sem repetir intervalo;
a declaração inclui o comentário de documentação. Aqui, dos nós da árvore do
parser (nomes, expressões, comandos, tipos, padrões, parâmetros, listas de
argumentos, `nome: valor`, `x = e`, declarações, diretivas); a árvore do
analyzer tem nós intermediários que esta não tem (`ExpressionStatement` com
o `;`, `VariableDeclarationList`, `FormalParameterList`), então alguns
degraus diferem.

**Dicas embutidas** (`computer/computer_inlay_hint.dart`): o arquivo inteiro
(o intervalo pedido é ignorado, como no Dart); tipo antes do nome de
variável sem tipo escrito (local, campo, topo, `for-in`, padrão), antes do
nome de parâmetro sem tipo (de *closure* inclusive; não `this.x`/`super.x`)
e de função ou método sem tipo de retorno (antes do `get` num getter; não
em setter); `nome:` antes de cada argumento posicional de chamada
resolvida; argumentos de tipo inferidos (`<int>`) antes do `[`/`{` de
literal sem argumentos escritos e depois do nome da classe numa criação sem
argumentos escritos. `label` em partes (sem `location`), `paddingRight`
como o Dart. Falta: argumentos de tipo inferidos de chamada genérica
(`map<String>(…)`), que pedem os argumentos inferidos da inferência comum.

**Tokens semânticos** (`computer/computer_highlights.dart`,
`lsp/semantic_tokens/{mapping,legend,encoder}.dart`): a legenda é a do
3.6.2, na mesma ordem (tipos `annotation, keyword, class, comment, method,
variable, parameter, enum, enumMember, type, source, property, namespace,
boolean, number, string, function, typeParameter`; modificadores
`documentation, constructor, declaration, importPrefix, instance, static,
escape, annotation, control, label, interpolation, void, wildcard`). Palavras
reservadas e embutidas viram `keyword` (`control` nas de fluxo), `true`/
`false` `boolean`, `void` `keyword`+`void`; números, strings (cada trecho
de uma interpolada; `${…}`/`$x` como `source`+`interpolation`; escapes como
`string`+`escape`) e comentários (`documentation` em `///`/`/**`); cada nome
pelo elemento que denota — declarações com `declaration` (classe,
extension type, método, getter/setter, campo e variável), referências a
getter, campo e variável de topo como `property`, campo declarado como
`variable` (com `instance` ou `static`), métodos com `instance`/`static`,
construtores (`class`/`method` + `constructor`), locais, parâmetros
(`label` nos rótulos `nome:`), parâmetros de tipo, constantes de enum,
prefixos (`importPrefix`), e `source` para o não resolvido. Anotações:
`annotation` do `@` ao `(` (ou ao fim) e no `)`, com `annotation` nos
nomes. Sobreposições divididas pelo de cima (`splitOverlappingTokens`);
sem `multilineTokenSupport`, um token por linha.

**Hierarquia de tipos** (`lsp/handlers/handler_type_hierarchy.dart`,
`computer/computer_lazy_type_hierarchy.dart`): `prepareTypeHierarchy` no
nome de um tipo (ou dentro de uma classe, mixin, enum ou extension type)
devolve o item da classe (`kind: 5`, nome com os parâmetros de tipo,
`range` a declaração, `selectionRange` o nome); `supertypes`: superclasse
(`Object` quando não há `extends`), restrições `on`, interfaces e mixins, com
os argumentos de tipo escritos (`Base<int>`); `subtypes`: as classes do
projeto que citam o tipo em `extends`, `implements`, `on` ou `with`.

**Ações rápidas**: além das descritas em `textDocument/codeAction` acima,
`crates/lsp/src/correcoes.rs` traz as dos códigos publicados que ainda não
tinham correção, com título, espécie e edição dos produtores do 3.6.2
(`services/correction/dart/*.dart`, ligados em `fix_internal.dart`):
`non_abstract_class_inherits_abstract_member` (`Create N missing
override(s)` — os membros sem implementação concreta na cadeia da classe,
ordenados pelo nome, getter antes de setter, par getter/setter como campo,
tipos substituídos pelos argumentos com que a classe vê o supertipo,
corpo `// TODO: implement x` + `throw UnimplementedError();`, depois do
último membro —, `Create 'noSuchMethod' method`, `Make class 'C'
abstract`), `concrete_class_with_abstract_member` (as duas últimas),
`unused_field`, `unused_catch_clause`, `unused_catch_stack`,
`assignment_to_final_local`, `missing_default_value_for_parameter` (`Add
'required' keyword` num nomeado, `Make 'x' nullable`),
`await_in_wrong_context` (`Add 'async' modifier`, com `Future<…>` no
retorno escrito), `nullable_type_in_*_clause` (`Remove the '?'`),
`const_instance_field` (`Add 'static' modifier`), `non_final_field_in_enum`
(`Make final`), `extension(_type)_declares_member_of_object` (`Remove method
declaration`) e `assert_in_redirecting_constructor` (`Remove the
assertion`). As correções valem para os diagnósticos das linhas pedidas (o Dart filtra
por linha), as assistências para o cursor; `Ignore 'x' for this line`/`for
the whole file` vêm por último para os diagnósticos que não são erro.
Assistências de reescrita (`crates/lsp/src/assistencias.rs`): `Convert to
async function body`, `Convert to block body`, `Convert to expression
body`, `Remove type annotation`, `Split variable declaration`, `Use curly
braces`, `Assign value to new local variable`, `Inline Local Variable`,
`Extract Local Variable`; criação de membros (`crates/lsp/src/criar.rs`):
`Change to 'x'` (distância de edição < 3), `Create method/function/class/
mixin/getter/field/local variable`, `Create extension method/getter`.
`source.organizeImports` segue o `ImportOrganizer` (dart:, package:,
relativos; exports depois; duplicados removidos). Faltam: `Fix All`, `Sort
Members`, `Add missing switch cases`, `Add required argument`, `Extract
Method`, `Inline Method`, `Move to file`.

**Hierarquia de chamadas** (`computer/computer_call_hierarchy.dart`,
`lsp/handlers/handler_call_hierarchy.dart`): `prepareCallHierarchy` num
executável escrito (função, método, operador, getter/setter escritos,
construtor; o sem nome implícito fica na classe), com nome exibido (`get
x`, `set x`, `A.nome`), espécie (12 função, 6 método, 9 construtor, 7
propriedade), `detail` o contêiner (classe ou arquivo), `range` a
declaração e `selectionRange` o nome; `incomingCalls`: as referências (a
família de sobrescrita inclusa) agrupadas pelo executável que as contém
(ou pela classe, num inicializador de campo, ou pelo arquivo);
`outgoingCalls`: chamadas, criações e leituras de getter escrito no corpo,
agrupadas pelo chamado. As recebidas procuram no projeto carregado.

**Fora (e por quê)**: formatação (decisão L08 acima); `documentLink` (no
3.6.2 só liga `** See code in examples/api/…` de comentários do Flutter);
`codeLens`, `colorProvider` e `inlineValue` (desligados no 3.6.2 sem
configuração do cliente).

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
