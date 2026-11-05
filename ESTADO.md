# Estado do DartForge — 2026-11-05

## Prioridade: LSP mais rápido e mais leve que o servidor oficial

**Meta (decisão do proprietário, 01/11/2026):** o `dartforge-lsp` precisa ser melhor que
o `dart language-server` em todos os pedidos: mais rápido, com menos CPU e menos memória,
com a mesma resposta. Depois da décima sétima rodada, ganha no `new_sali/frontend` a memória em
repouso de uma sessão com pedidos (as consultas de biblioteca saem do residente), os pedidos sem
edição e quase todos depois de uma, e as três cargas da sessão longa no macOS. Perde a edição de
corpo da sessão longa no Windows (76 × 60 ms), e `references` logo depois de uma edição não ganha
de forma estável.

1. **Reanalisar só as bibliotecas afetadas por uma mudança de API.** No `new_sali/frontend`,
   o diagnóstico do arquivo editado depois de uma mudança de API leva **1,3–1,4 s contra
   84–89 ms do Dart**. Os dependentes levam de 7 a 22× o tempo do Dart.
   - **Causa:** `ClassId`, `FunctionElementId` e os demais identificadores são índices
     densos, renumerados a cada carga. Por isso nada do modelo de elementos e do de tipos
     sobrevive a uma mudança de assinatura.
   - **O que pede:**
     - identidade estável de elementos em `crates/elements`;
     - um `OutlineTypes` por biblioteca em `crates/types`;
     - refazer só as bibliotecas cujo *outline* depende do que mudou.
2. **Índice de referências resolvidas.** `references` dos tipos do SDK no frontend leva
   **9,6 s × 344 ms**, com o mesmo resultado (cerca de 15 s de CPU por busca de `String`
   ou `bool`), porque cada busca infere os corpos de todos os arquivos que citam o nome.
   - **O que pede:**
     - o verificador registrar o alvo de cada resolução, com uma identidade comum à de
       `crates/lsp/src/project.rs`;
     - o residente manter esse índice.
   - **Restrição:** um índice parcial não serve, porque quebraria a paridade de 100% do
     `references` no oráculo.
   - **Feito na décima quinta rodada** (`crates/lsp/src/reference_index.rs`): `references`,
     `implementation` e `prepareRename` respondem do índice. No `new_sali/frontend`, sem edição,
     ficam em 2–3 ms contra 7–20 ms do Dart.
   - **Feito na décima sexta rodada:** o registro do documento editado sai do residente, e os
     três pedidos ganham do Dart também depois de uma edição.
3. **Memória estável em sessões longas.** É o problema que o proprietário vive com o
   servidor oficial no VS Code.
   - **Cenário:** o `new_sali` aberto inteiro, com frontend, core e backend. São cerca de
     1 milhão de linhas com as dependências, das quais 200 mil são do próprio projeto.
     Enquanto isso, rodam `dart run build_runner` / `webdev build` / `webdev serve` e há
     muitas edições de template e de código.
   - **Sintoma no Dart:** o *analysis server* cresce até **~10 GB**. O `build_runner`
     reescreve em massa os arquivos gerados (`.g.dart`, `.template.dart`, `.dart_tool/`),
     e cada onda dispara reanálise e acumula memória.
   - **Meta:** a memória do `dartforge-lsp` fica limitada e plana ao longo da sessão.
     Depois de cada onda de arquivos gerados, volta ao patamar de repouso, bem abaixo do
     Dart nas mesmas condições.
   - **O que pede:**
     - um cenário de sessão longa no driver de medição (`crates/lsp/examples/editing_latency.rs`):
       horas simuladas de edições, mais ondas de reescrita dos arquivos gerados como as
       do `build_runner`, mais `didChangeWatchedFiles` em massa;
     - a RSS dos dois servidores amostrada ao longo do tempo;
     - o pico e a inclinação (MiB por hora) como métricas;
     - nenhuma estrutura que cresça com o número de recargas. A alta de 6% do
       `limitless_ui` com o prefixo congelado é desse tipo e tem de sumir.
   - **Agrupar as ondas:** uma rajada de arquivos gerados é tratada como uma recarga só,
     com espera curta, e não arquivo a arquivo.
   - **Comportamento de vazamento:** no Dart, a memória não volta sozinha. O proprietário
     precisa matar o processo do *analysis server* para ela voltar ao normal.
   - **Critério de aceite:** depois de N ondas e de muitas edições, a memória em repouso
     do `dartforge-lsp` fica a poucos por cento da de um servidor recém-iniciado no mesmo
     estado do projeto. Ninguém deve precisar reiniciar nada.
   - **Rede de segurança:** se a memória passar de um orçamento, o servidor recicla os
     próprios processos filhos (residente e buscas) de forma transparente, sem perder o
     estado do editor. Não pode depender disso para passar no critério de aceite.

**Como validar:** cada passo com teste contra uma análise do zero, o oráculo de paridade a
100% e a medição revezada com o servidor oficial (com a coluna de CPU) nos três projetos.
Nenhum projeto pode piorar. O item 3 se mede no `new_sali` inteiro, na sessão longa, com
a curva de memória dos dois servidores.
Para `workspace/symbol`, a comparação agora inclui a lista completa em ordem, com todos
os campos, e consultas que chegam ao limite de 500; igualdade só do conjunto não basta.

**Dono:** agente LSP, primeiro da fila de desempenho a partir de 01/11/2026.

**Andamento (04–05/11), décima oitava rodada: paridade de `workspace/symbol` ampliada,
mas ainda incompleta nos projetos grandes e em sessões com várias pastas:**
- O oráculo antigo comparava conjuntos de `(nome, URI)` em 24 consultas. Seus 100% não
  verificavam a ordem nem os demais campos: a resposta completa coincidia em **4/24**.
- Acrescentadas as consultas amplas `a` e `e`, agora são 32. Antes da correção, coincidiam
  **24/32 conjuntos** e **4/32 respostas completas em ordem**; depois, **32/32 nos dois
  critérios**, também comparando espécie, intervalo, contêiner e demais campos JSON.
- Corrigidos a ordem de descoberta dos arquivos do projeto, a descoberta inicial de
  `dart:core` e suas referências, as dependências alcançadas antes da varredura geral e o
  nome de elemento `unary-` do operador unário. A ordem alfabética das cargas semânticas
  não foi alterada.
- **Sessões e primeira consulta:** as duas matrizes do ciclo de vida passaram com
  **112/112 respostas completas iguais**, incluindo criação antes da primeira busca;
  essa variante dava 38/56 antes do ajuste. A descoberta inicial agora acompanha a
  abertura dos contextos, e os eventos preservam a ordem mesmo sem índice consultado.
  Os quatro projetos pequenos foram repetidos na árvore atual: **32/32**. O índice não
  conserva símbolos ao fechar o último documento com `onlyAnalyzeProjectsWithOpenFiles`.
- **Descoberta de dependências corrigida:** os ciclos descobrem imports antes de exports;
  o bootstrap de `dart:core` mantém sua ordem própria. A enumeração do SDK usa a variante
  selecionada pelo analisador, sem duplicar declarações de parts compartilhadas. Arquivos
  abertos têm prioridade na descoberta; imports editados sem salvar invalidam o grafo,
  preservando a ordem já conhecida. URIs relativas de packages agora resolvem `..` e
  escapes percentuais. Catálogos substituídos liberam suas bibliotecas antigas.
- **Catálogos por contexto:** a descoberta agora conserva o catálogo do arquivo que
  faz o import, inclusive através de dependências compartilhadas. URIs adicionadas
  mantêm o primeiro dono; URIs conhecidas recebem o último, sem trocar de posição.
  A varredura segue o `HashMap` de drivers da VM 3.6.2 e a ordem de raízes do catálogo.
  Gerados também são resolvidos em `test/` e `web/`; imports relativos partem da URI
  canônica, sem ocultar a biblioteca real por um caminho inexistente no diretório gerado.
- **URI válida preservada ao revalidar o disco:** as três declarações de `Ponto` estavam
  no índice na primeira consulta, mas sumiam na seguinte. A limpeza esquecia um import
  inexistente do pai, que então retomava a URI do gerado válido do filho. Agora essa
  colisão fica registrada enquanto houver uma biblioteca válida; imports inválidos sem
  vínculo válido são descartados. O teste mantém a resposta após 128 mudanças de import,
  sem acumular os estados e índices antigos.
- **Histórico da tabela de drivers (05/11):** a sessão agora preserva a capacidade da
  tabela após reconstruir os contextos ou fechar a última pasta. O oráculo novo usa
  sete projetos e dependências não importadas: a consulta de dependências passou de
  **2/6 para 6/6** nas fases de expansão, reconstrução, redução, esvaziamento e reabertura;
  adiando a primeira busca até depois da redução, de **1/4 para 4/4**. A capacidade já
  é registrada ao abrir os contextos, antes de qualquer busca. Projetos removidos não
  ficam retidos para preservar esse histórico.
  **Na primeira correção, a matriz inteira continuava reprovada:** **13/18** e **9/12** respostas completas,
  respectivamente, por diferenças na ordem dos símbolos dos próprios projetos.
  As execuções anteriores deram 12/18 e 9/12, em outros caminhos temporários: esses
  totais não isolam o efeito da correção. As dependências têm uma ordem esperada
  calculada pela VM para cada caminho, que é conferida pelo oráculo.
- **Ordem de criação dos contextos (05/11):** o `ContextLocator` primeiro passa as
  raízes por um `HashSet` novo e depois as ordena pelo comprimento UTF-16. Isso agora
  antecede a criação dos drivers e dos arquivos adicionados, mantendo separada a
  capacidade histórica do mapa de drivers. A ordenação também reproduz os movimentos
  entre caminhos do mesmo comprimento no quicksort da VM, a partir de 34 raízes.
  **Matriz completa nos mesmos caminhos:** **13/18 → 18/18**, repetidos duas vezes
  antes e duas depois, com as mesmas respostas do Dart nas quatro execuções.
  Com a primeira busca adiada até depois da redução: **9/12 → 12/12**. Os 27 casos
  capturados da VM verificam raízes duplicadas, comprimentos distintos, pares UTF-16
  e a transição entre insertion sort e quicksort. Isso fecha esses cenários com
  várias pastas; a repetição com cache Dart isolado e frio também manteve **18/18**.
  Os demais históricos de sessão continuam sujeitos ao oráculo.
- **Condicionais e descoberta durante a busca (05/11):** o índice registra as URIs de
  todas as alternativas de import/export, mas percorre primeiro a primária: o servidor
  de análise 3.6.2 não declara variáveis condicionais, nem `dart.library.*`. Ausência
  não equivale a `false` nem a string vazia. A busca resolve as bibliotecas da lista
  capturada, incluindo as que não combinam com o nome; arquivos descobertos nesse
  trabalho só entram na próxima resposta. Cada arquivo mantém seu contexto de origem.
  O oráculo novo passou de **4/16 para 16/16**, repetido com cache quente e frio:
  **48/48** após o ajuste, com respostas da referência idênticas antes/depois.
  Inclui alternativas fora dos catálogos, dependências dessas alternativas, edição
  sem salvar, reversão e fechamento. Depois de estabilizar, buscas confiáveis com
  as mesmas diretivas não percorrem novamente o grafo.
- **URIs canônicas e fontes sombreadas (05/11):** imports relativos de uma biblioteca
  de pacote agora resolvem dentro de `package:`, antes de mapear para o disco.
  `../../x.dart` e `/x.dart` mantêm a raiz de `lib/`; URIs de arquivo mantêm suas
  regras próprias. Corrigidos escapes percentuais, barras escapadas e Unicode;
  fragmentos não vazios são rejeitados, como no SDK. O fallback de gerados
  conserva os escapes, pois o SDK usa esse caminho sem decodificá-lo.
  O oráculo principal passou de **4/12 para 12/12**, também repetido sem watcher.
  A variante com uma fonte real fora de `lib/` sombreada por um gerado passou de
  **7/16 para 16/16** após respeitar a ida e volta caminho → URI → caminho exigida
  por `AnalysisDriver.addFile`. Importar explicitamente essa URI encontra o gerado.
  A variante manteve **16/16 com watcher, sem watcher e com cache Dart frio**;
  as respostas ficaram idênticas. Quatro regressões novas incluem 60 vetores da VM.
- **Projetos grandes, comparação de 04/11:** a dupla deu **9/24 respostas completas no
  cache Dart frio e 24/24 no quente**, com **20/24 e 24/24 conjuntos**, respectivamente.
  Os três projetos deram **8/8 no quente**, incluindo os 373 resultados de `Ponto` no
  `limitless_ui`; outra execução com os mesmos caches manteve **24/24**, sem mudar as
  respostas de nenhum dos servidores. A dupla anterior dava 11/24 e 20/24; a queda no
  frio veio da variação do Dart no core, sem mudar a resposta do DartForge nesse projeto.
  Os manifestos de entrada continuaram iguais e sem raízes ausentes. O core usa a cópia com os 64
  caminhos Windows corrigidos, mantendo versões e fontes. Os 24/24 quentes não fecham
  a paridade no frio, a descoberta condicional, edições nos projetos grandes nem
  todos os históricos de expansão/remoção de contextos.
  **Repetição quente em 05/11:** **21/24 respostas completas e 24/24 conjuntos**:
  `limitless_ui` e core 8/8, frontend 5/8. As 24 respostas do DartForge continuam
  idênticas às da execução quente anterior; o Dart mudou a ordem de `ctx`, `Ponto`
  e `join` no frontend. Repeti-lo no mesmo cache deu 8/8, outra vez mudando só o Dart.
  Fontes e metadados ficaram estáveis, sem raízes ausentes. No core, o digest textual
  do manifesto mudou por `/tmp` versus `/private/tmp`; os arquivos canônicos, conteúdos,
  tamanhos e datas coincidem. As três falhas da primeira repetição são conservadas;
  o novo 8/8 não fecha a paridade entre sessões. Até essa etapa não houve nova dupla
  fria nos projetos grandes. **Após as URIs, duas matrizes quentes mantiveram 21/24 respostas completas
  e 24/24 conjuntos**, com respostas idênticas nos dois servidores entre elas. As
  respostas do DartForge também são idênticas às da etapa anterior, e os manifestos
  permaneceram iguais, sem raízes ausentes. Detalhes e reprodução em `docs/LSP.md`.
- **Preparação das sessões do oráculo (05/11):** os rastros mostraram que a espera
  antiga pelo status do Dart, seguida de um segundo, iniciava as buscas com o
  DartForge ainda analisando nas nove sessões auditadas dos modos `legacy`, `cycle`
  e `resolved`. Acrescentado `ORACLE_READINESS=settled`: resolve os mesmos documentos
  abertos nos dois servidores e observa ambos ociosos durante um segundo. O padrão
  continua `legacy`; a preparação adicional tem metadados próprios e não conta como
  amostra de paridade. Nesse protocolo distinto, os três projetos grandes deram
  **24/24 respostas completas**, e três sessões rastreadas do frontend deram **8/8
  cada**. No protocolo antigo, as três sessões do frontend deram **4/8, 8/8 e 8/8**;
  as respostas do DartForge foram idênticas nas 12 sessões dos quatro modos. Fontes
  e configurações ficaram iguais, mas o conteúdo interno do cache Dart não foi
  congelado: não há prova de que só a espera explique a variação. Os **21/24**
  anteriores continuam reprovados. Nenhum código do servidor mudou nesta etapa.
  Corrigidos também os prazos do cliente: notificações não reiniciam o timeout dos
  pedidos, e a espera pelo intervalo ocioso respeita o prazo total. Passaram
  **11 testes Python** sem SDK; os controles mantiveram **32/32** nos projetos
  pequenos e **16/16** na fonte sombreada sem watcher. Detalhes em `docs/LSP.md`.
- **Buscas sem watcher, latência e CPU medidas em 05/11:** o perfil do binário
  `release` encontrou normalizações repetidas dos longos caminhos de pacotes com
  `..`. O catálogo agora guarda esses caminhos normalizados desde a carga, sem
  mudar a ordem nem dispensar revalidações do disco. Em três sessões por servidor
  e projeto, alternando a ordem e sem compilações/testes concorrentes, as medianas
  quentes de latência caíram de **595,5 para 385,7 ms** no `limitless_ui`, de
  **275,4 para 158,3 ms** no core e de **877,2 para 615,0 ms** no frontend.
  A CPU da árvore caiu na mesma proporção: cerca de **35%, 43% e 30%**.
  **O Dart ainda ganha:** suas medianas foram 17,1, 13,3 e 18,9 ms. A primeira
  busca também melhorou, mas continua mais lenta. As **27 sessões** conservaram
  **216/216 respostas completas** em cada comparação antes/depois/Dart, com
  manifestos idênticos. Este protocolo resolve o documento e espera dez segundos
  adicionais de ociosidade; não fecha as falhas do protocolo antigo nem o cache
  frio. O footprint final ficou menor que o Dart, sem redução consistente após
  esta mudança; no frontend, variou de 586,7 para 589,6 MiB (Dart 712,1 MiB), com
  faixas sobrepostas. Não é uma medição de estabilidade de sessão longa.
  Reprodução, CPU, primeira busca e memória em `docs/LSP.md`.
- **Parts criados sem editar a biblioteca dona (05/11):** uma ausência não entrava
  nos fingerprints da biblioteca. Depois de abrir em memória ou criar um `.g.dart`
  antes inexistente, o índice aquecido podia continuar omitindo suas declarações.
  Agora o índice registra também a ausência, de modo que abertura, fechamento,
  criação, edição, remoção e recriação invalidam a biblioteca quando necessário.
  O novo oráculo passou de **15/27 para 27/27 com watcher e sem watcher**, e repetiu
  **27/27 com cache Dart frio**: **81/81 após a correção**, com as respostas Dart
  idênticas nas cinco execuções. Dentro de cada pedido, as bibliotecas já validadas
  são reutilizadas na descoberta e na coleta; o marcador é fixo por entrada e
  muda a cada busca, sem acumular um histórico. A comparação de desempenho dessa
  etapa fica separada da correção funcional: **54 sessões** revezaram os dois
  binários e o Dart, com **216/216 respostas completas em cada comparação sem
  watcher e outros 216/216 com watcher**, mantendo os manifestos idênticos.
  Sem watcher, as medianas quentes caíram de **381,6 para 366,8 ms**, de **155,1
  para 145,1 ms** e de **607,5 para 590,2 ms** nos três projetos, cerca de 3–6%
  em latência e CPU; ainda perdem do Dart. Com watcher, ficaram próximas do
  baseline em **12,2, 9,1 e 12,7 ms**, abaixo do Dart; a variação de +0,2 ms no
  `limitless_ui` não demonstra ganho geral nesse modo. O footprint final não
  melhorou de forma consistente (core com watcher: 124,3 → 125,7 MiB); a primeira
  busca continua mais lenta que o Dart. O perfil posterior ainda aponta custo
  na resolução de imports e comparação de caminhos. Detalhes em `docs/LSP.md`.
- **Bases relativas e caminhos compartilhados (05/11):** cada unidade agora
  compartilha seu caminho entre imports e exports. Sua base canônica é calculada
  uma vez por contexto e busca, mantendo a escolha de fonte/gerado revalidada no
  disco. A tabela é descartada após o pedido. Duas regressões novas verificam
  contextos diferentes para o mesmo arquivo e mudanças dos alvos gerados durante
  a reutilização da base. Mantidos os 60 vetores de URI da VM, **270/270** controles
  completos e **27/27 com e 27/27 sem watcher** nos parts.
  Nas **54 sessões revezadas**, cada matriz manteve **216/216 respostas completas
  iguais em cada comparação**. Sem watcher, as medianas quentes caíram de **353,9
  para 180,6 ms**, de **142,1 para 72,4 ms** e de **580,1 para 239,4 ms**:
  cerca de **49%, 49% e 59%** em latência e CPU. O footprint final caiu cerca de
  **1,7–2,5 MiB**. Com watcher, não houve ganho quente: o core subiu de **8,9 para
  9,4 ms** (CPU 8,5 → 9,0 ms), regressão pequena ainda aberta; os três continuam
  abaixo do Dart nesse modo. A primeira busca melhorou, mas continua perdendo nos
  comparativos revezados. Os manifestos permaneceram iguais, sem raízes ausentes.
  Uma nova dupla com caches Dart isolados e inicialmente vazios deu **72/72 no
  frio e 72/72 no quente**, com respostas de cada servidor idênticas entre estados.
  A preparação tem dez segundos adicionais de ociosidade: não fecha as falhas do
  protocolo legado, que precisa de nova rodada na árvore atual. Detalhes, CPU,
  memória e artefatos em `docs/LSP.md`.
- **Outlines no índice de símbolos (05/11):** a construção do índice passou a
  pular as instruções dos blocos usando a API de outlines já existente, mantendo
  spans, assinaturas, inicializadores, closures e corpos `=>`. A análise de
  diagnósticos continua usando a leitura completa. A comparação dos metadados
  integrais dos dois modos passou em sete fontes fixas e **17.645 arquivos Dart
  únicos** do corpus; essa expansão é um teste, não 17.645 testes ou uma prova de
  paridade com o Dart. Mantidos **270/270** controles completos e **54/54** nos parts.
  Nas **54 sessões revezadas**, cada matriz manteve **216/216 respostas completas
  iguais em cada comparação**. Sem watcher, a primeira busca caiu de **1.310 para
  1.143 ms**, de **960 para 755 ms** e de **1.592 para 1.413 ms** nos três projetos;
  a CPU caiu cerca de **13%, 15% e 11%**. Com watcher, caiu de **1.290 para 1.146 ms**,
  de **852 para 755 ms** e de **1.590 para 1.406 ms**, com **11–12% menos CPU**.
  O footprint final caiu aproximadamente **1–4,6 MiB**, mas a primeira busca
  continua mais lenta que a do Dart. As buscas quentes ficaram próximas do
  baseline; a regressão do core com watcher continua aberta (9,0 → 9,3 ms,
  CPU 8,5 → 8,9 ms nesta matriz). Manifestos idênticos antes/depois, sem raízes
  ausentes. Os detalhes, o ensaio preliminar mais lento e os limites dessa
  medição estão em `docs/LSP.md`.
- **Abertura de projeto externo e experimento retirado (05/11):** o novo oráculo
  de aliases revelou uma divergência preexistente: abrir um arquivo de outro
  projeto podia colocar seus símbolos antes dos já conhecidos. A abertura e o
  fechamento agora conservam os arquivos sobreviventes e acrescentam os novos,
  mantendo a reconstrução nas mudanças explícitas de configuração. Nos mesmos
  caminhos, **26/27 → 27/27 com watcher e 26/27 → 27/27 sem watcher**, com as
  respostas do Dart inalteradas. As nove fases incluem edições sem salvar,
  catálogos com tipos diferentes/iguais, remoção de dependência e fechamento final.
  Os **270/270** controles anteriores foram mantidos. Foi medido também um cache
  de aliases em **54 sessões**, com **216/216 respostas completas em cada
  comparação de cada matriz**. O cache foi **retirado**: o ganho quente com
  watcher foi pequeno e variável (core 9,0 → 8,8 ms; frontend 12,7 → 12,2 ms),
  o footprint subiu até **3,8 MiB**, e sem watcher o `limitless_ui` piorou de
  **177,3 para 180,3 ms**. A alteração final mantém somente a correção de ordem,
  o oráculo e sua regressão; não reivindica os ganhos do experimento. Detalhes e
  artefatos em `docs/LSP.md`.
- **Travessia de diretivas por pedido (05/11):** a descoberta e a resolução da
  lista de bibliotecas agora compartilham as unidades já processadas, com chave
  contexto + caminho. Isso evita materializar as mesmas diretivas duas vezes,
  sem guardar esse trabalho entre pedidos nem confundir dois drivers que usam
  o mesmo arquivo. A regressão de condicionais foi ampliada: uma edição de
  ramificação não selecionada atualiza a declaração na busca atual e acrescenta
  sua nova dependência somente na seguinte. Passaram os **335 testes do LSP**,
  os 11 testes Python, clippy, documentação estrita e formatação. Mantidos
  **378/378** comparativos completos: 270 controles anteriores, 54 de aliases e
  54 de parts nos dois modos de watcher. Nas **54 sessões revezadas**, cada
  matriz manteve **216/216 respostas completas iguais em cada comparação**.
  Sem watcher, as medianas quentes caíram de **184,5 para 166,5 ms**, de **72,7
  para 65,1 ms** e de **241,6 para 209,0 ms**: cerca de **10–14%** em latência e
  CPU, ainda acima do Dart. Com watcher, ficaram em **11,6 / 8,8 / 12,8 ms**,
  sem ganho geral; o frontend subiu 0,1 ms. O footprint final também não ganhou
  de forma geral: o frontend subiu **0,4 / 0,8 MiB**, e os outros projetos
  caíram até 1,6 MiB. A primeira busca melhorou pouco e continua mais lenta que
  o Dart. Manifestos idênticos antes/depois, sem raízes ausentes. Uma nova
  passada do legado com caches aquecidos deu **24/24 nas duas versões**; não
  apaga as reprovações anteriores nem substitui a dupla fria/quente isolada.
  Os detalhes, CPU, memória e artefatos estão em `docs/LSP.md`.
- **Raízes do catálogo normalizadas uma vez (05/11):** a criação das bases
  relativas e a identificação de package URIs reutilizam as raízes já
  normalizadas pelo carregamento do contexto. Os mapeamentos de gerados também
  passam por essa normalização; catálogos avulsos mantêm o resolvedor geral.
  Uma regressão nova compara os dois caminhos com raízes contendo `..`, unidades
  fonte/geradas, escapes, fragmentos e URI de arquivo. Ela também corrigiu a
  passagem do gerado para uma raiz fonte não normalizada no caminho geral.
  A criação/remoção de fonte e gerado foi verificada nos dois modos de resolução.
  Mantidos os 60 vetores da VM e **378/378** controles completos com o Dart.
  Passaram **336 testes do LSP**, os 11 testes Python, clippy do workspace,
  documentação estrita e formatação. Nas **54 sessões revezadas**, cada matriz
  manteve **216/216 respostas completas iguais em cada comparação**. Sem watcher,
  as medianas quentes caíram de **164,1 para 153,0 ms**, de **64,6 para 61,6 ms**
  e de **206,4 para 189,7 ms**: cerca de **5–8%** em latência e CPU, ainda acima
  do Dart. A primeira busca melhorou cerca de **1–3%**, mas continua perdendo.
  Com watcher, as buscas quentes ficaram próximas do baseline, sem ganho geral;
  o `limitless_ui` subiu 0,1 ms. O footprint final aumentou em quatro das seis
  células, com **+2,0 / +1,0 MiB** no frontend. Manifestos idênticos antes/depois,
  sem raízes ausentes. Repetida no novo binário a dupla com caches Dart isolados
  e inicialmente vazios: **72/72 no frio e 72/72 no quente**, com os arrays de
  cada servidor idênticos entre estados. São 12 sessões adicionais com preparação
  explícita, separadas das tabelas de desempenho e das reprovações do legado.
  Detalhes e medições em `docs/LSP.md`.
- **Extração paralela avaliada e retirada (05/11):** o protótipo antecipou os
  metadados com até quatro trabalhadores na primeira busca, mantendo a
  descoberta sequencial. As **54 sessões** preservaram **216/216 respostas
  completas em cada comparação de cada matriz**, e a primeira resposta caiu
  **27–37%**. Entretanto, a CPU inicial subiu **8–11%** e o footprint aumentou
  **1,0–9,1 MiB** em todas as células. No core, a latência venceu o Dart, mas a
  CPU passou de menor para maior que a dele. O código paralelo foi retirado;
  os artefatos e a comparação de metadados em 17.645 arquivos ficaram registrados
  em `docs/LSP.md`. O oráculo de parts com 320 arquivos adicionais foi mantido,
  verificando **36/36 com e 36/36 sem watcher**, inclusive a ordem dos 320 símbolos.
- **Nomes de contêiner compartilhados (05/11):** os membros de cada classe,
  mixin ou extension type passam a compartilhar seu texto imutável de
  `containerName`. Uma regressão verifica o compartilhamento, a atualização após
  edição, a igualdade com um índice novo e a liberação da cadeia antiga. A
  indexação continua sequencial. Passaram **337 testes do LSP**, **450/450**
  controles completos com o Dart, 11 testes Python, clippy do workspace,
  documentação estrita e formatação. A comparação entre parsers manteve os
  metadados integrais iguais nos 17.645 arquivos do corpus. Nas **54 sessões
  revezadas**, cada matriz manteve **216/216 respostas completas iguais em cada
  comparação**. O footprint final do frontend caiu **8,1 / 7,8 MiB** e o do
  `limitless_ui`, **2,5 / 1,5 MiB**, nos modos sem/com watcher. O core subiu
  **1,0 / 1,2 MiB**. A primeira busca melhorou pouco sem watcher e não teve
  ganho geral com watcher; continua mais lenta que o Dart. As buscas quentes
  ficaram próximas do baseline, com +1,1 ms no frontend sem watcher. Manifestos
  idênticos antes/depois, sem raízes ausentes. Detalhes e medições em `docs/LSP.md`.
- **Metadados de disco por pedido (05/11):** a descoberta e a validação das
  bibliotecas compartilham os metadados ao revalidar um índice sem confiança no
  disco. O mapa sai ao fim do pedido; a primeira indexação e as buscas confiáveis
  ficam sem ele. A resolução entre fonte e gerado continua fresca a cada chamada.
  Passaram **338 testes do LSP**, **450/450 controles completos com o Dart**,
  11 testes Python, clippy do workspace, documentação estrita e formatação.
  Nas **54 sessões revezadas**, cada matriz manteve **216/216 respostas completas
  iguais em cada comparação**. Sem watcher, as medianas quentes caíram
  **150,8 → 141,0 / 61,3 → 54,9 / 184,8 → 175,4 ms**, com **5–11% menos CPU**;
  ainda perdem do Dart. O custo de footprint final foi **+0,6 / +0,5 / +1,2 MiB**.
  A primeira busca subiu **1,1% / 2,1%** nos dois primeiros projetos sem watcher;
  não há ganho geral nela. As buscas quentes com watcher ficaram próximas do
  baseline e continuam ganhando do Dart. Manifestos idênticos antes/depois,
  sem raízes ausentes. A sessão longa e as edições grandes não foram repetidas.
  Detalhes, tabelas e limitações em `docs/LSP.md`.
- **Regressões do servidor:** **338 testes do LSP passaram novamente em 05/11**, incluindo doctests, o platô de
  memória, as duas regressões do histórico da tabela e os 27 casos de ordenação de
  raízes numa regressão nova, duas regressões de descoberta e listas de resposta e
  quatro de URIs, normalização e fontes sombreadas, além das duas novas de criação
  de part e retorno do contador de validação ao início após seu limite.
  As duas regressões das bases relativas foram acrescentadas na continuação.
  O teste de igualdade dos metadados entre os parsers foi acrescentado depois.
  A abertura de projeto externo acrescentou uma regressão de integração que
  falhava antes da correção; as duas específicas do cache retirado não integram a suíte.
  A regressão mais recente cobre o estado de disco compartilhado só dentro de um pedido.
  Clippy do workspace sem avisos, formatação e documentação estrita passaram.
  Após as URIs, mantidos **112/112** no ciclo de vida, **24/24 com e 24/24 sem
  watcher** nos dois contextos, **18/18** na capacidade, **16/16** nas condicionais e
  **32/32** nos quatro projetos pequenos. Sem watcher, eram 15/24 antes de preservar
  a colisão da URI; a matriz
  corrigida já havia passado duas vezes nos dois modos em 04/11 (**96/96**).
  Após normalizar os catálogos, passaram **270/270 controles completos**: 112 do
  ciclo de vida, 48 dos catálogos, 44 das URIs e sombras, 16 condicionais, 18 de
  capacidade e 32 dos projetos pequenos; os gates e os 11 testes Python passaram.
  A mesma matriz de 270 controles passou após a validação por pedido e a correção
  dos parts; os 81 casos novos ficam registrados separadamente acima.
- **Desempenho ainda incompleto:** capturar a ordem inicial exige enumerar os arquivos
  ao criar a coleção; reconstruí-la reenumera as raízes e descarta o índice sintático.
  Contextos existentes não são reenumerados por edições de corpo; o custo de abertura
  e reconstrução nos projetos grandes ainda precisa ser medido, incluindo a nova
  checagem de ida e volta das URIs na adição dos arquivos. As buscas sem watcher
  agora têm a medição isolada acima. Faltam a paridade após edições nesses projetos
  e com dependências condicionais, desempenho nessas mudanças, a primeira busca,
  reduzir a regressão quente do core com watcher e a sessão longa.
- Detalhes e critérios em `docs/LSP.md`, seção de 04/11 e continuação de 05/11. Os problemas de desempenho e de
  memória da décima sétima rodada abaixo continuam abertos.

**Andamento (03/11, 11h), décima sétima rodada: as consultas servidas pelo residente:**
- **As consultas de biblioteca saem do residente** (`hover`, `definition`, `signatureHelp`,
  `documentHighlight`, `prepareRename`, `codeAction`, `completion`, *tokens*, *inlay hints*,
  hierarquias, cores): ele empresta a última análise a um analisador só dele. O servidor não
  carrega mais a biblioteca do documento.
- **O SDK com *patches*:** um membro que um *patch* do DDC substitui responde com a declaração
  original (local, assinatura, documentação), como no analisador.
- **Os registros do índice** que faltam também saem do residente: a primeira edição de uma
  sessão não carrega mais a biblioteca no servidor.
- **O *cache* do índice é mapeado**, e não lido para o *heap* (−53 MiB no servidor).
- **Medida no `new_sali/frontend`** (carga 11–38; mediana em ms):

  | pedido | rodada 16 | agora | Dart |
  |---|---:|---:|---:|
  | `references`, sem / depois de edição | 0,4 / 95,0 | **0,4–0,8** / 36,7–58,6 | 27,3–30,5 / **19,2–25,4** |
  | `implementation`, sem / depois | 0,3 / 2,5 | **0,3 / 2,3–2,4** | 12,3–13,4 / 10,8–32,0 |
  | `prepareRename`, sem / depois | 0,4 / 1,5 | **0,4 / 1,7–1,9** | 1,1 / 2,1–11,5 |
  | `hover`, sem / depois | 0,4 / 0,9 | **0,5–0,6 / 1,2** | 1,2–1,3 / 3,6–24,2 |
  | memória em repouso, sessão de pedidos (MiB) | 1.220 | **791–794** | 814–944 |

- **Sessão longa no macOS** (150 rodadas, 2 pares, carga 11–30): onda **71; 78 × 109; 114 ms**,
  edição de corpo **30; 31 × 34; 39 ms**, API **46; 49 × 100; 112 ms**, pico **579 × 717–726 MiB**,
  inclinação **+2,6; +1,1 × +35,3; +36,9 MiB por 100 cargas**.
- **Windows:** uma amostragem com `xperf` mostrou 10% do residente em `stat` (agora guardados
  entre as cargas, `dartforge_elements::fs_cache`), 16% no *heap* do Windows e ~10% em operações
  de caminho. A sessão longa (2 pares) fica em onda **221–223 × 400–443 ms**, API **124–125 ×
  265–426 ms** e bytes privados **370–373 × 705–710 MiB**. A edição de corpo **ainda perde**:
  76 × 60 ms.
- **Validação:**
  - **oráculo de paridade** com o binário final: a mesma tabela das rodadas 15 e 16;
  - **igualdade do índice** (2.119 alvos, com os registros do índice e com os do residente);
  - **clippy** `-D warnings` e **rustfmt** limpos;
  - **testes em *release*** dos nove *crates*: não terminaram no fechamento (carga 50–96; a compilação passou de 90 min).
- **Ainda não feito:**
  - a inclinação ≤ 0 em toda sessão longa;
  - a edição de corpo no Windows;
  - a recarga no lugar casando os elementos por nome;
  - a ordem do `workspace/symbol`;
  - as consultas de projeto (`rename`, hierarquias completas, buscas sem índice), que ainda
    carregam no servidor.
- Detalhes em `docs/LSP.md`, "Décima sétima rodada".

**Andamento (03/11, 08h), itens 1 a 4 e 6 (décima sexta rodada: o registro do documento no residente):**
- **O registro do documento editado sai do residente**, que já infere a biblioteca: depois dos
  diagnósticos, ele empresta a análise ao construtor do índice e manda o registro ao servidor,
  que espera por ele antes de uma busca. O que o construtor calcula das unidades que não mudaram
  fica entre um empréstimo e outro (~20 ms por registro, contra 0,5–1,3 s sem isso).
- **As tabelas derivadas do índice** são atualizadas no lugar numa edição de corpo, e a
  atualização antes de cada busca só olha os documentos abertos.
- **A sessão confia no *watcher* do cliente**: a comparação com o disco (~2.000 `stat`, 85% de um
  `hover`) só roda depois de um aviso ou a cada 30 s.
- **`workspace/symbol`:** os gerados que as bibliotecas importam entram (paridade: o
  `.template.dart` dava um resultado a menos), e a lista de arquivos fica guardada enquanto o
  *watcher* não avisa.
- **Defeito do residente corrigido** (`45c5f371`): duas bibliotecas "quentes" que se importam
  saíam do *layout*, e a análise seguinte publicava classes indefinidas.
- **Medida no `new_sali/frontend`** (8 posições; mediana em ms; carga 24–34; índice no *cache*):

  | pedido | rodada 15 | agora | Dart |
  |---|---:|---:|---:|
  | `references`, sem edição / depois de edição | 3,7 / 208 | **0,8–1,0 / 3,3–26,9** | 26,5–26,7 / 20,9–53,0 |
  | `implementation`, sem / depois | 3,2 / 45,1 | **0,3 / 2,5–5,5** | 12,5–14,0 / 11,0–11,7 |
  | `prepareRename`, sem / depois | 3,2 / 26,1 | **0,4 / 1,5** | 0,8–1,1 / 2,6–4,0 |
  | `hover`, sem / depois | 10,4 / 15,0 | **0,4 / 0,8** | 1,3–1,4 / 2,2–14,6 |
  | `workspace/symbol`, sem / depois | 162–215 / 201–219 | **22–23 / 23–28** | 39–68 / 1.764–1.869 |
  | memória em repouso, sessão de pedidos (MiB) | 1.085 | 1.218–1.227 | **858–889** |

  Ganhamos todos os pedidos. **Perdemos a memória com pedidos**: a sessão do servidor (376 MiB) e
  o índice (156 MiB) ficam ao lado do residente (~540 MiB).
- **Sessão longa no macOS, 150 rodadas, três pares revezados** (carga 17–30; mediana até
  assentar, CPU entre parênteses):

  | carga | DartForge | Dart |
  |---|---:|---:|
  | onda de 40 `.g.dart` | **72–78 ms (51–57)** | 138–158 ms (228–240) |
  | edição de corpo | **31–32 ms (24–25)** | 41–47 ms (33–34) |
  | mudança de API | **45–51 ms (41–47)** | 121–141 ms (85–88) |
  | pico / repouso (MiB) | **574–576 / 527–574** | 698–757 / 686–756 |
  | inclinação por 100 cargas | **+2,3; −13,2; +2,4** | +30,7 a +41,7 |

  - **A inclinação ≤ 0 em toda sessão não foi atingida** (uma em três). O dado vivo é plano nos
    dois processos (servidor 10,0 MiB, residente 476,4 MiB do começo ao fim); os degraus de
    2–6 MiB são páginas do alocador. Neste macOS (*xzone malloc*) a devolução das páginas
    (`malloc_zone_pressure_relief`) devolve 0 bytes, também num programa C de teste.
- **Windows (i3-1215U; sem `cargo`/`rustc`/`link` durante as sessões; carga 22–79%):**
  - **duas causas da edição de corpo lenta, corrigidas:** a chave de cada caminho era refeita
    componente a componente no Windows (`71959cd4`; o caminho rápido só existia no Unix), e cada
    análise lia a data de cada arquivo do contexto do pacote (`137ace98`);
  - **sessão longa, 150 rodadas, dois pares:** onda **246; 234 contra 392; 433 ms**, mudança de
    API **140; 140 contra 267; 361 ms**, edição de corpo **77; 77 contra 60; 61 ms** (era 139–143;
    **ainda perde**); bytes privados **368–381 MiB do começo ao fim** contra 524 → 734 do Dart;
  - numa sessão só de edições de corpo (só o `frontend`), **29,5 contra 76,7 ms**.
- **Validação:**
  - **igualdade do índice** nos quatro projetos do oráculo (2.119 alvos), com os registros do índice
    e com os do residente: as mesmas respostas da busca sem o índice;
  - **oráculo de paridade** com o binário final: a mesma tabela da rodada 15 (`references`,
    `implementation` e `definition` a 100%; *completion* top-5 a 100% e top-1 a 99%);
  - **testes em *release*** dos nove *crates* de sempre: 853 passaram, nenhum falhou;
  - **clippy** `--workspace --all-targets -D warnings` e **rustfmt** `--check` limpos.
- **Ainda não feito:**
  - **a memória com pedidos abaixo do Dart:** servir as consultas do residente pede o mesmo SDK
    nos dois processos (o residente usa o SDK do DDC com *patches*, o servidor o do analisador);
    é uma decisão sobre o motor de `dartforge analyze`;
  - **a recarga no lugar casando os elementos por nome** (item 1);
  - **`workspace/symbol`:** a ordem dos arquivos conhecidos, que decide os 500 resultados de uma
    consulta ampla;
  - **a edição de corpo da sessão longa no Windows** (77 contra 60–61 ms);
  - **a inclinação ≤ 0 em toda sessão longa no macOS** (+2,3; −13,2; +2,4: fragmentação do
    alocador, com o dado vivo plano).
- Detalhes em `docs/LSP.md`, "Décima sexta rodada".

**Andamento (03/11, 02h), itens 2 e 3 (décima quinta rodada: o índice de referências):**
- **Item 2, o índice de referências resolvidas** (`crates/lsp/src/reference_index.rs`):
  - **o que guarda:** o que cada nome resolve, em cada arquivo conhecido (projeto, `lib/` dos
    pacotes, SDK), por uma chave estável (URI da biblioteca + caminho da declaração). É
    calculado com as mesmas funções da busca;
  - **nos arquivos do projeto**, guarda também o que o `identify` diz de cada identificador e a
    resposta de `prepareRename`. Por isso uma busca não carrega a biblioteca do documento;
  - **o que responde:** `references`, `implementation` e `prepareRename`. O parâmetro nomeado
    ainda vai pelo caminho antigo;
  - **ciclo de vida:** o índice é feito num filho na primeira busca (45 s de CPU no
    `new_sali/frontend`) e fica no *cache* do usuário entre sessões. Os arquivos do projeto que
    mudaram são registrados de novo antes de cada resposta;
  - **sessão semântica:** o orçamento passou de 32 para 48 MiB de fonte. O programa de um
    componente do frontend tinha passado de 32 MiB, e cada `hover` o recarregava (~950 ms).
- **Igualdade com a busca sem o índice:** a busca sem o índice e o índice dão a mesma resposta em 2.119 alvos dos quatro
  projetos do oráculo (`references`, `implementation` e `prepareRename`). Os 159 parâmetros
  nomeados vão pelo caminho antigo, que agora carrega o projeto inteiro (com só a biblioteca,
  faltavam os argumentos de outros arquivos).
- **Medida no `new_sali/frontend`** (8 posições; mediana em ms, sem edição / depois de uma
  edição; índice já no *cache*; carga 20–27):

  | pedido | DartForge antes | DartForge agora | Dart |
  |---|---:|---:|---:|
  | `references` | 2.044 / 2.033 | **3,7** / 289 | 20,7 / **9,0** |
  | `references`, pior caso (`String`) | 9.163 / 9.764 | **106 / 468** | 3.339 / 8.662 |
  | `implementation` | — | **3,1** / 44,9 | 12,5 / **4,5** |
  | `prepareRename` | ~950 | 3,2 / 25,7 | **0,8 / 1,8** |
  | `hover` | 964 / 966 | 10,7 / **11,5** | **1,1** / 35,7 |
  | memória em repouso, todos os processos | 824 MiB | 1.082 MiB | **798 MiB** |

  Ganhamos sem edição em `references` e `implementation` e no pior caso de `references`.
  Perdemos depois de uma edição (o registro do documento é refeito) e, por alguns ms, em
  `prepareRename` e `hover`.
- **Sessão longa, 150 rodadas, dois pares revezados** (carga 20–59, nunca calma):

  | carga (mediana até assentar, CPU) | DartForge | Dart |
  |---|---:|---:|
  | onda de 40 `.g.dart` | **72–79 ms (52–57)** | 141–155 ms (247–390) |
  | edição de corpo | **31–33 ms (25–26)** | 39–73 ms (34–72) |
  | mudança de API | **51–52 ms (44–46)** | 130–188 ms (89–154) |

- **Item 3, memória:**
  - o dado vivo é o mesmo (476,4 MiB);
  - depois de cada análise o residente devolve as páginas livres ao sistema
    (`malloc_zone_pressure_relief`). A inclinação cai de +4,6 para +0,8 e +1,2 MiB por 100
    cargas (Dart: +17,4 e +39,8);
  - **a meta ≤ 0 não foi atingida.** O que sobra são degraus de 1–2 MiB do servidor e do
    trabalhador.
- **Windows (decisão do proprietário sobre o alocador):**
  - **alocador do sistema:** ganhamos na onda (358–373 contra 540–882 ms) e na API (267–270
    contra 408–5.695 ms), mas perdemos na edição de corpo (139–143 contra 75–76 ms);
  - **com o `mimalloc`:** ficou ~12% mais rápido, mas guardou ~190 MiB a mais, e a inclinação
    não ficou abaixo da do sistema nos dois pares (+2,2 contra +0,2);
  - **não foi adotado** (`21843d27` reverte `f9f28d17`);
  - a máquina ficou em 48–100% de CPU durante as sessões, sem `cargo`, `rustc` ou `link`.
- **Validação:**
  - **oráculo de paridade** duas vezes com o binário final: `references`, `implementation` e
    `definition` a 100%; *completion* top-5 a 100% e top-1 a 99%;
  - **testes em *release*** dos nove *crates* de sempre: 852 passaram, nenhum falhou. Os 26 de
    `emit_js`/`dev` só passaram com o `runtime/ddc/dart_sdk.js` copiado para o *worktree*;
  - **clippy** `--workspace --all-targets -D warnings` e **rustfmt** `--check` limpos.
- **Ainda não feito:**
  - **o registro do documento depois de uma edição:** sai da sessão (~20–40 ms quando ela tem a
    biblioteca) ou de um filho (~1,3–1,6 s quando não tem). O residente já infere a biblioteca
    editada; fazer o registro dele é o próximo passo;
  - **o `hover`**, `definition` e `prepareRename` ainda passam do Dart por alguns ms
    (6 contra 1 ms);
  - **a recarga no lugar casando os elementos por nome** (item 1): não começou;
  - **a memória** do servidor e do trabalhador nas sessões longas (degraus de 1–2 MiB).
- Detalhes em `docs/LSP.md`, "Décima quinta rodada".

**Andamento (02/11, 22h), itens 1 e 3 (décima quarta rodada: a etapa 2 e os custos fixos):**
- **Erro do *driver*:** o cronômetro de cada edição (corpo e API) começava antes de amostrar a
  CPU com `pgrep`, e isso somava 25–35 ms a cada edição medida. Pesava mais para o DartForge,
  que tem três processos. Corrigido em `dacb7794`; os números de corpo e de API das rodadas 12
  e 13 carregam esse erro.
- **Etapa 2, recarga no lugar** (`28597b79`): o arquivo cuja API mudou e de que outras
  bibliotecas dependem vai depois dos dependentes no *layout*. A próxima mudança dele recarrega
  só ele.
  - Os dependentes mantidos continuam valendo quando os elementos dele voltam aos mesmos ids
    (declaração acrescentada ou removida no fim, assinatura mudada) e nenhuma assinatura de
    dependente nomeia o que mudou.
  - Os escopos de quem importa ou reexporta são refeitos. Senão, a carga cai no corte de antes.
- **Custos fixos por carga, cortados:**
  - a busca do pacote de cada diretiva (`4b38b89a`);
  - a coleta da tabela de tipos a cada carga (`86434ae6`, `287066bb`);
  - as cópias de *namespaces* nas fases 2 e 3 do *outline* (`ad5050aa`);
  - as tabelas de corpo refeitas a cada passada (`d85fa3a8`);
  - as constantes citadas de dependências inferidas a cada edição (`5877ffc4`);
  - a comparação das chaves de sintaxe (`cea887c8`);
  - a varredura do disco quando o *watcher* já disse o que mudou (`8e18855f`).
- **150 rodadas no macOS, revezadas:**

  | carga (mediana até assentar, CPU) | DartForge | Dart |
  |---|---:|---:|
  | onda de 40 `.g.dart` | **100–101 ms (69–75)** | 130–142 ms (294–338) |
  | edição de corpo | **41–42 ms (32)** | 47–48 ms (39–40) |
  | mudança de API | **64–65 ms (53–57)** | 148–161 ms (116–119) |

  - Dois pares revezados, com a carga em 8–11: o `cargo test` da v21 estava preso a ~390% de CPU.
  - Ganhamos nas três cargas. Na mudança de API, por 2,3–2,5× no relógio, com metade da CPU.
- **Memória:** o dado vivo foi de 474,2 MiB (1 rodada) a 476,4 MiB no fim (+0,46%), o mesmo
  em três sessões. Inclinação do *footprint*: +2,1 e −0,1 MiB por 100 cargas nos pares, e −0,6 e
  0,0 em outras duas sessões. Os degraus das mudanças de API sumiram com as recargas grandes; os
  que restam, de 2–6 MiB, não coincidem com elas e são do alocador. O Dart ficou em +34 a +36.
- **Validação:**
  - testes em *release* dos *crates* tocados e dependentes: 850, sem falhas;
  - `typed_incremental` com as edições de API no lugar;
  - oráculo de paridade igual ao da décima terceira rodada (`references`, `implementation` e
    `definition` a 100%, *completion* top-5 a 100%);
  - clippy `-D warnings`.
- **Ainda não feito:**
  - a recarga no lugar casando os elementos por nome: uma declaração inserida no meio do
    arquivo, ou um arquivo com *parts*, ainda cai no corte de reserva;
  - um par com a máquina calma de fato, ou muito carregada, com o cronômetro corrigido.
- Detalhes em `docs/LSP.md`, "Décima quarta rodada".

**Andamento (02/11, 15h), itens 1 e 3 (décima terceira rodada: ganhar na onda e na API):**
- **Medida das trocas entre processos** (`2bb97f49`): as passagens servidor → trabalhador →
  residente somam ~8 ms por onda. O tempo era o da análise, então juntar processos não
  ajudaria.
- **O que cortou o tempo da análise:**
  - a ordem prefere o que muda por último (`7a67cfc9`): a onda recarrega 41 bibliotecas
    em vez de 394, e a API 409 em vez de 739;
  - uma mudança só no disco não cancela a análise (`d0138bcb`);
  - o índice de elementos sobrevive ao corte (`0aa2712c`);
  - os índices de constantes vêm por unidade (`7d7e18d0`), e a classe de um construtor é
    buscada só na unidade dele (`a0724ed3`);
  - os limites simples são decididos só nas declarações novas (`6793199c`);
  - a inferência de *overrides* roda só depois do corte (`d8be1447`);
  - menos trabalho fixo por carga (`928e899d`).
- **150 rodadas no macOS, revezadas** (mediana até assentar; CPU entre parênteses):

  | carga | máquina carregada (41–46): DartForge | Dart | máquina calma (2–9): DartForge | Dart |
  |---|---:|---:|---:|---:|
  | onda de 40 `.g.dart` | **208 ms (107)** | 311 ms (216) | **79–83 ms (70)** | 102–103 ms (139) |
  | edição de corpo | **86 ms (29)** | 91 ms (35) | 40–43 ms (17) | **30–31 ms** (17) |
  | mudança de API | 573 ms (139) | **250 ms** (93) | 101–105 ms (82) | **90–92 ms** (60) |

  - A onda ganha do Dart nas duas condições. Na rodada 12 ela perdia: 588 contra 476 ms.
  - A mudança de API caiu de 795 para 573 ms com a máquina carregada, mas ainda perde. Falta
    a etapa 2 (identidade estável de elementos), que está projetada em `docs/LSP.md` e não foi
    feita.
  - Com a máquina calma, a edição de corpo ainda fica ~12 ms atrás do Dart.
- **Windows (i3, parado), antes da reserva da máquina:** perdemos nas três cargas (onda 520
  contra 370–587 ms, corpo 125 contra 61 ms, API 505 contra 282–600 ms).
  - A análise fica ~3× mais lenta a partir da quarta onda. Com o `mimalloc` (experimento,
    fora do repositório), o servidor ganha 19–21%.
  - Adotar um alocador próprio no Windows é uma dependência nova e fica para o proprietário
    decidir.
- **Memória:** o dado vivo do residente foi de 474,2 MiB com 1 rodada para 474,5 MiB no fim
  (+0,06%). A inclinação do *footprint* foi −1,0 e −0,2 MiB por 100 cargas nas sessões calmas
  revezadas e +1,2 e +1,5 em outras duas (degraus do alocador). A do Dart foi +36 a +40.
- **Validação:** testes em *release* dos *crates* tocados e dependentes (848, sem falhas);
  `typed_incremental` com as edições de API repetidas; oráculo de paridade com `references`,
  `implementation` e `definition` a 100% e *completion* top-5 a 100%; clippy `-D warnings`;
  publicações do `new_sali` iguais às da décima segunda rodada.
- Detalhes em `docs/LSP.md`, "Décima terceira rodada".

**Andamento (02/11, 05h), itens 1 e 3 (velocidade da sessão longa):**
- **Mudança de API (item 1, pelo nome):** um dependente só é reanalisado se pode ver um nome
  que a mudança alterou (`e4642ed6`). A comparação é declaração por declaração, com o dono do
  membro, as famílias seladas e a propagação pelas assinaturas. Antes eram os 260 dependentes
  de `documento_service.dart`; o Dart também não os reanalisa.
- **O resto:**
  - os verificadores por biblioteca não andam mais pelo programa inteiro;
  - só rodam as regras de *lint* ligadas;
  - o disco e a árvore do pacote não são relidos a cada análise;
  - o *debounce* da onda caiu de 250 para 20 ms.
- **150 rodadas, os dois servidores revezados** (carga 58–63; mediana até assentar, CPU entre
  parênteses):

  | carga | DartForge r11 | DartForge agora | Dart |
  |---|---:|---:|---:|
  | onda de 40 `.g.dart` | 2.309 ms | 588 ms (186) | 476 ms (216) |
  | edição de corpo | 357 ms | **118 ms (36)** | 136 ms (38) |
  | mudança de API | 14.688 ms | 795 ms (185) | 420 ms (103) |

  - Ganhamos na edição de corpo.
  - Na onda gastamos menos CPU, mas com a máquina carregada perdemos no relógio. Com a
    máquina calma (10 rodadas) ficamos à frente ou empatados.
  - A mudança de API ainda é ~1,9× a do Dart: ela recarrega o *outline* de tudo o que vem
    depois do corte.
- **Memória mantida:** fim da sessão 540,6 MiB contra 550,8 de uma rodada, inclinação −0,3 MiB
  por 100 cargas, dado vivo +1,0%. Dart: 731,5 MiB, +44,3 por 100 cargas.
- **Validação:**
  - testes em *release* dos *crates* tocados e dependentes: 846, sem falhas;
  - oráculo de paridade: `references`, `implementation` e `definition` 100%, *completion*
    top-5 100%;
  - clippy `-D warnings`;
  - publicações finais do `new_sali` iguais às da décima primeira rodada.
- **Ainda não feito:** refazer só o *outline* de quem depende da mudança de API (identidade
  estável de elementos).
- Detalhes em `docs/LSP.md`, "Décima segunda rodada".

**Andamento (02/11, 01h), item 3:**
- **Medida:** cada carga da sessão longa agora é medida até a publicação dos diagnósticos
  novos dos arquivos afetados (`6828d91f`); os 1.513 ms fixos da onda sumiram. O servidor
  anuncia a análise que uma mudança no disco começa.
- **Contadores por estrutura** (`dartforge/residentMemory`, `a7ada9e0`). O que se via como
  crescimento era:
  - um degrau único na primeira edição: o contexto do pacote, +53 MiB de dado vivo;
  - a tabela de tipos, que guardava todos os parâmetros de tipo de todas as cargas.
- **Correção:** os parâmetros soltos saem da tabela e os slots são reusados; o lote solta as
  bibliotecas do pacote que não alcança mais.
- **150 rodadas, *footprint* em MiB** (carga 41–130):

  | | DartForge | Dart |
  |---|---:|---:|
  | recém-aberto (só abrir) | 445,9 | 533,1 |
  | recém-aberto + 1 rodada | 506,4 | 450,6 |
  | fim da sessão longa | **537,0** | 886,0 |
  | pico | 598,8 | 1.103,1 |
  | inclinação por 100 cargas | **−0,7** | +62,2 |
  | dado vivo do residente, 1 rodada → fim | 473,2 → 476,9 (**+0,8%**) | — |

- **Critério:** contra o recém-aberto que já fez uma edição, o dado vivo está a +0,8% e o
  *footprint* a +6%. Contra o só aberto, +20%, que é o degrau do contexto. Os +30 MiB de
  *footprint* sobre o dado vivo são memória que o alocador guarda depois dos picos; isso não
  foi atacado.
- **Tempo** (mediana, até assentar): onda 2.309 × 767 ms, edição de corpo 357 × 214 ms,
  mudança de API 14,7 × 3,2 s.
- **Windows:** o teste do prefixo congelado agora compara caminhos sem `\\?\` (`b2ce641f`).
- **Validação:**
  - testes em *release* dos *crates* tocados e dependentes: 845, sem falhas;
  - corpus JS 277/277 em desenvolvimento e em `-O2`;
  - harness nativo 164/164 (`corpus/native`);
  - oráculo de paridade: `references`, `implementation` e `definition` 100%, *completion*
    top-5 100%;
  - clippy `-D warnings`.
- **Ainda não feitos:**
  - a retenção do alocador;
  - o modelo de análise preguiçosa do Dart (item 4 do pedido).
- Detalhes em `docs/LSP.md`, "Décima primeira rodada".

**Andamento (01/11, 23h), item 3:**
- O mimalloc com *purge* no residente foi testado com `--long 150` e **descartado**:
  - recém-aberto → fim: 310 → 410 MiB com o alocador do sistema;
  - 319 → 420 MiB com o mimalloc (os mesmos +100 MiB);
  - Dart: 528 → 682 MiB.
- A medição da onda no *driver* (1.513 ms) é artefato: as janelas de espera somadas ao
  *debounce* de 250 ms. Precisa ser corrigida antes de comparar ondas.
- Ainda não feitos:
  - os contadores de dado vivo por estrutura;
  - o modelo de análise preguiçosa do Dart.
- Detalhes em `docs/LSP.md`, "Décima rodada".

**Andamento (01/11, 22h), item 3, diagnóstico dos +49%** (residente em repouso,
`vmmap`; recém-aberto 319,6 MiB → sessão longa 494,1 MiB, +174,5 MiB):
- Dado vivo do `malloc`: **+89,5 MiB**.
- Fragmentação: **+52,9 MiB** (5% → 16%).
- Fora do `malloc`: +32,1 MiB.
- Tempo por carga (DartForge × Dart):
  - onda de 40 `.g.dart`: **1.513 × 10 ms**;
  - mudança de API: 1.840 × 128 ms;
  - edição de corpo: 77 × 35 ms.
- Ainda não feitos:
  - dividir o dado vivo por estrutura;
  - devolver memória ao sistema;
  - descartar o que o lote não alcança;
  - corrigir o custo das ondas.
- Detalhes em `docs/LSP.md`, "Nona rodada".

**Andamento (01/11, 21h), item 3, sessão longa medida** (`editing_latency --long`):
- Cenário: `new_sali` inteiro, 150 rodadas, 510 cargas (ondas de 40 `.g.dart`, edições de
  corpo e de API), carga 26–69.
- *Footprint* de todos os processos, em MiB:

  | | DartForge | Dart |
  |---|---:|---:|
  | pico | 507 | 845 |
  | crescimento por 100 cargas | **4,7** | **60** |
  | em repouso no fim | 506 | 845 |
  | recém-aberto nos mesmos arquivos | 339 | 429 |
  | razão fim ÷ recém-aberto | 1,49× | 1,97× |

- O Dart cresce sem parar (o "vazamento" do VS Code). O nosso sobe nas primeiras ~100 cargas
  e quase para, mas fica 49% acima do recém-aberto: **o critério ainda não está cumprido**.
- Ainda não medido: o que é dado vivo e o que é retenção do alocador.
- Ainda não feito: descartar do programa mantido as bibliotecas que o lote não alcança.
- Curvas em `bench/results/lsp-long-session/`; detalhes em `docs/LSP.md`, "Oitava rodada".

**Andamento (01/11, 20h), item 3 (memória em sessões longas):**
- O residente coleta a tabela de tipos a cada carga sobre um corte, e a recarga total a cada
  16 cargas saiu.
- Repouso depois de 12 rodadas de mudanças de API contra 1 rodada:
  - backend **plano (−1%)**;
  - frontend +7% (era +16%);
  - `limitless_ui` +19% (era +30%).
- O critério (repouso a poucos % de um servidor recém-aberto) ainda não vale para o frontend
  e o `limitless_ui`. O programa mantido acumula as bibliotecas que os lotes já importaram.
- O cenário de sessão longa com ondas de `build_runner` (3a) não foi escrito.
- O *debounce* das ondas (3c) tem teste que falha sem ele.
- Detalhes em `docs/LSP.md`, "Sétima rodada".

**Andamento (01/11, 18h):**
- Passo 4, primeira parte: as bibliotecas do pacote em ordem de dependência. Uma mudança de
  API recarrega a partir do grupo da biblioteca editada, e não do pacote inteiro.
- Frontend: diagnóstico do arquivo depois de mudança de API privada **265** (era 460) × 19
  ms; exportada 298 × 63 ms.
- `limitless_ui`: exportada **85 × 116 ms (ganha)**.
- Ondas de arquivos gerados: uma análise depois de 250 ms de disco quieto (item 3, parte c).
- Faltam:
  - a coleta da tabela de tipos (memória: em repouso no backend +11% e no `limitless_ui`
    +1,5% nesta rodada);
  - o cenário de sessão longa (item 3, partes a e b);
  - o `simple_bounds` por faixa;
  - o passo 5.
- Detalhes em `docs/LSP.md`, "Sexta rodada".

**Andamento (01/11, 15h):**
- **Item 1, primeira parte, feita** (passos 1–3 do projeto em `docs/LSP.md`, "Projeto:
  reanalisar só o que mudou"). O SDK e as dependências ficam num prefixo congelado do
  programa, cujos IDs, tipos e hierarquia sobrevivem a uma mudança de API. Só o pacote é
  recarregado e tem o *outline* refeito.
- **Frontend:** o diagnóstico do arquivo depois de uma mudança de API caiu de 478–583 para
  226–236 ms na sessão revezada (154–171 ms numa sessão sozinha). O Dart faz 25–50 ms.
  `limitless_ui`: 161/86 ms × 85/53 ms.
- **Falta:**
  - o passo 4: reaproveitar as bibliotecas do pacote que não dependem da editada, e um
    `simple_bounds` por faixa;
  - os corpos dos dependentes (~2 s de CPU × 110 ms no frontend);
  - o item 2 inteiro: o índice de referências (passo 5).

## 04/11/2026: continuação depois da alpha.4

**Base:** `main` em `522bdd32`; a alpha.4 publicada continua em `87cb092b`.
As correções desta seção são posteriores ao pacote publicado.

- **JS de produção: propriedades do Node preservadas pelo minificador.** O conjunto de
  nomes reservados não incluía `exit`, `on`, `stderr` e `write`, usados pelo inicializador
  e pelo verificador de poda. Quando o programa Dart declarava membros com esses nomes,
  o minificador renomeava também os acessos ao Node. Além do `process.exit` registrado
  no teste da alpha.4, a colisão com `on` impedia até o início do programa.
  - Regressão reproduzida antes da correção: `-O1` funciona; `-O2` sai com código 1 e
    `TypeError: process.P is not a function`.
  - Corrigido em `crates/emit_js_production/src/properties.rs`. O teste
    `uncaught_exceptions_keep_the_host_protocol` compila membros com os quatro nomes,
    lança erros síncronos e assíncronos e verifica stdout, stderr completo e saída 255
    em `-O1`, `-O2`, `-O3` e `-O4`: **8 execuções passaram**. Os **90 testes unitários**
    do pacote passaram, incluindo os três que exigem Node. Um deles verifica que o
    verificador de poda minificado escreve o nome no stderr e sai com código 97.
- **CLI:** `dartforge --version` e `dartforge -V` agora imprimem a versão do pacote Cargo,
  com o mesmo atalho do LSP.
- **Distribuição:** `pack` inclui `dartforge-wasm-run` quando construído ao lado do CLI.
  O `Release` passa a construir esse executor e a executar o módulo recém-compilado
  pelo binário empacotado, comparando sua saída. Verifica também `--version` e a
  exceção do JS minificado com colisões de nomes.
- **Pacote local validado no macOS:** binários do perfil debug, com `native,jit`, LSP,
  JS de produção e executor Wasm, preservados em `target/test-tools/`. O roteiro do
  `Release`, adaptado apenas para empacotar esses binários, passou inteiro:
  AOT normal/otimizado, `--version`, execução do Wasm pelo executor incluído, JS dev e
  produção, exceção com saída 255, LSP e `_macros` pelo SDK empacotado. Os comandos do
  pacote usaram `env -i` com `PATH=/usr/bin:/bin`; Node rodou o JS pelo caminho de
  validação. Isso **não** é uma nova medição de desempenho ou tamanho do release.
  `cargo clippy --locked --workspace --all-targets -- -D warnings` e `rustfmt` passaram.
  A primeira suíte do workspace teve cinco alvos reprovados; depois de corrigir o ambiente
  e a concorrência do oráculo DDC, a repetição integral terminou com saída 0 (detalhes
  abaixo). A suíte do CLI
  com `native,jit` revelou dois testes reprovados em `test_runner_cli`, agora corrigidos;
  a nova execução integral passou com **117 testes, zero falhas e zero ignorados**.
  Ainda não há resultado do
  novo workflow nos três sistemas nem nova versão publicada.
  O link estático local avisou que objetos do LLVM instalado têm versão mínima
  macOS 27 acima do alvo 11; o pacote foi executado no sistema desta máquina,
  sem validar compatibilidade com versões antigas do macOS. Para os testes,
  a biblioteca compartilhada local foi conferida pela API como LLVM 22.1.8.
- **Documentação Rust:** a geração completa revelou 24 avisos de links privados ou
  sem resolução. Os privados agora aparecem como nomes de código, e os públicos foram
  qualificados corretamente. A geração de todo o workspace, inclusive itens privados,
  passou com `RUSTDOCFLAGS='-D warnings'` (zero avisos).
- **Pré-requisitos:** o teste ignorado do gzip do Pub exige `DARTFORGE_GZIP_CORPUS`,
  além dos SDKs e do compilador preservado. Requisito registrado no `CONTRIBUTING.md`;
  nesta rodada, cinco entradas e seus `.gz` foram gerados pela VM 3.6.2 do macOS.
  O verificador agora exige também `DARTFORGE_SDK_LIB` explícito e confere os pares do
  corpus gzip. Antes, as variáveis dos dois oráculos passavam no verificador, mas os
  testes de sobreposição do SDK liam `DARTFORGE_SDK_LIB` diretamente e falhavam. A
  ausência de cada uma das duas variáveis novas foi testada: erro antes da compilação.
- **Suíte completa, primeira execução:** terminou com código 101 e cinco alvos reprovados:
  corpus do diferencial, `fresh_list_append`, `native_number_ranges`, contratos nativos
  e corpus do frontend. O log completo ficou em `/tmp/dartforge-workspace-tests.log`.
  - Quatro alvos exigiam `DARTFORGE_SDK_LIB` explícito: repetidos com a biblioteca da
    3.6.2, passaram os **6 testes de JS de produção**, os **23 contratos nativos** e os
    **6 do corpus do frontend**. Os demais executáveis da execução completa passaram;
    o comando continuou até o fim com `--no-fail-fast`.
  - No diferencial, os dois testes compilavam DDC e executavam Node no mesmo diretório.
    Um Node podia ler o módulo antes de outro compilador substituir o import de
    `dart_sdk.js`, falhando ao resolver o pacote. Agora um bloqueio de arquivo cobre a
    compilação e a execução; o cache é publicado por renomeação, sem leitura parcial.
    Falhas de execução inválidas não são reutilizadas, e rejeições previstas e defeitos
    declarados do DDC continuam distinguíveis.
  - Os dois testes concorrentes do corpus passaram com **cache frio (277 programas)**
    e quente. Os **30 testes unitários do diferencial** passaram; uma regressão verifica
    a recuperação de cache inválido e a preservação dos erros declarados.
- **LSP, rodada 18:** **32/32 respostas completas de `workspace/symbol` iguais ao Dart**,
  em ordem, após ampliar o oráculo; resumo no topo. Suíte do LSP com ignorados e doctests,
  clippy dos dois crates alterados e documentação com avisos negados passaram.
  A ampliação para ciclo de vida encontrou **38/56 respostas completas** e **46/56
  conjuntos** iguais antes do ajuste, repetido duas vezes; depois, **56/56 nos dois
  critérios**. O projeto sintético passa do limite de 500 símbolos, com oito fases de
  edição e eventos do disco. A variante com criação antes da primeira busca também
  passou: **56/56**, contra 38/56 antes de capturar a ordem na abertura dos contextos;
  as duas matrizes somam **112/112**. Os quatro projetos iniciais foram repetidos: **32/32**.
  A listagem conserva a ordem dos arquivos sobreviventes e a inserção dos novos; os
  eventos distinguem remoção/recriação mesmo sem consulta entre eles. A opção de só
  analisar projetos abertos impede raízes permanentes e libera o índice de símbolos
  ao fechar o último documento; pedidos com escopo de raiz continuam alcançando os
  projetos abertos. **320 testes do LSP passaram**, incluindo doctests, criação antes
  da primeira busca e pacotes aninhados com/sem configuração própria de packages.
  O teste de versão obsoleta agora espera a análise válida pendente antes de exigir
  ausência de publicações e verifica também que não começou outra análise.
  A primeira ampliação aos três projetos grandes ficou em **10/24 respostas completas
  e 15/24 conjuntos** iguais. A continuação corrigiu a ordem dos ciclos, a escolha de
  variantes do SDK, prioridades abertas e URIs de packages. A auditoria posterior
  separa caches frios/quentes e invalida a conclusão sobre dependências resolvidas do
  core original: sua configuração aponta 64 raízes inexistentes do Windows. A cópia
  corrigida preserva as versões e os 554 demais arquivos; passa de **2/8 no frio para
  8/8 no quente**, com os mesmos hashes nas duas condições. As divergências ainda
  abertas e os limites da comparação estão no topo e em `docs/LSP.md`.
  Na continuação, foram corrigidos os catálogos por contexto, a propriedade das URIs,
  a ordem do mapa de drivers e a resolução dos gerados em `lib/`, `test/` e `web/`.
  O novo `workspace_symbol_contexts.py` passou em **24/24** em três execuções; a dupla
  completa dos projetos grandes passou a **11/24 no frio e 20/24 no quente**, mantendo
  a variação do próprio Dart registrada separadamente. Os hashes de entrada não mudaram.
  A etapa seguinte corrigiu a perda de `Ponto` entre consultas ao revalidar o disco:
  estados de imports inexistentes continuam lembrados quando compartilham uma URI
  válida, sem reter o histórico dos demais imports inválidos. Passaram **24/24 com
  watcher e 24/24 sem watcher do cliente**, repetidos duas vezes, e 128 mudanças de
  import no teste de retenção. A nova dupla dos projetos grandes deu **9/24 no frio e
  24/24 no quente**; a queda no frio veio da variação do Dart no core, com a mesma
  resposta do DartForge. Os três símbolos antes ausentes reapareceram no `limitless_ui`.
  `crates/lsp/oracle/workspace_symbol_lifecycle.py` reproduz os casos e retorna erro se
  houver diferenças. Não há medição de desempenho nessa ampliação.
- **Gates atuais:** `rustfmt`, clippy do workspace com `-D warnings` e documentação de
  todo o workspace com `RUSTDOCFLAGS='-D warnings'` passaram depois das alterações.
  A execução completa que revelou os cinco alvos continua registrada como reprovada.
  A nova execução de `cargo test --locked --workspace --no-fail-fast -- --include-ignored`
  terminou com **saída 0, sem alvos reprovados**, no ambiente corrigido; log completo em
  `/tmp/dartforge-workspace-recheck.log`. O CLI com `native,jit`
  já foi repetido integralmente: **117 testes passaram, zero falhas, zero ignorados**, em
  16 executáveis de teste; log em `/tmp/dartforge-cli-native-recheck.log` (saída 0).
  A execução do workspace começou antes do ajuste de ciclo de vida do LSP; a suíte
  completa desse crate foi repetida separadamente na árvore atual: **320 aprovados,
  zero falhas, zero ignorados**, com log em `/tmp/dartforge-symbol-missing-owner-verified-tests.log`.
  Formatação, clippy e documentação sem avisos passaram novamente depois desse ajuste.
- **CLI `native,jit`, correção do runner:** na primeira execução, os 114 demais testes
  passaram, incluindo geração de fontes, I/O, recarga e orçamentos de tamanho. Os dois
  do runner foram corrigidos:
  - opções explícitas de plataformas/compiladores não suportados, cobertura, depuração,
    watch e seleção por linha/coluna são recusadas com saída 64 antes da busca de
    dependências ou compilação, mesmo fora de um pacote. A leitura preserva os limites
    dos argumentos do `package:test`: valores de filtros que parecem opções não viram
    modos, e `--` continua separando os caminhos;
  - os dois runners agora são comparados pelo mesmo protocolo JSON de `package:test`,
    preservando resultado por suíte/teste, tentativas e testes pulados. O leitor antigo
    esperava o JSON próprio que deixou de existir em 02/11;
  - `--fail-fast` e o relator `github` saíram das recusas desatualizadas e ganharam
    casos positivos contra o oficial. Os **dois testes de integração passaram**:
    **oito cenários JSON**, incluindo interrupção após a primeira falha, e a saída
    completa do relator `github` em uma execução aprovada. Depois do ajuste final dos
    limites de URI, passaram novamente os **68 testes unitários**, incluindo o corpus
    gzip, e a matriz de recusas no binário atualizado.
  A seleção URI (`?line=5&col=2`, inclusive chaves escapadas) também é recusada cedo;
  filtros como `?full-name=foo:5` continuam sendo filtros. A nova execução integral do
  CLI passou. Logs da falha original e da repetição: `/tmp/dartforge-cli-native-tests.log`
  e `/tmp/dartforge-runner-differential.log`.
- **CI, cobertura conferida no código:** o `Heavy` diário roda no Windows, mas não
  executa a suíte de ignorados do workspace. Ele cobre corpora diferenciais e testes
  nativos/JIT selecionados; os ignorados de `dartforge-build`, macros e gerador ngdart
  continuam no `ci.yml`. Transferir o passo inteiro para o `Heavy` exigiria acrescentar
  essa cobertura lá. Nenhum teste foi movido ou retirado nesta rodada.
- **Limpeza anterior conferida:** os 13 *worktrees* antigos de agentes já foram removidos;
  seus 13 branches locais continuam presentes. Os registros Windows `base`/`bissect` e
  o *worktree* de validação `v21` continuam presentes. O `target/` apagado na sessão
  anterior foi recriado pela validação desta rodada.

## Painel: DartForge × Dart oficial (atualizado em 04/11/2026; medições de desempenho sem repetição)

Resumo de cada ferramenta contra a oficial correspondente (SDK 3.6.2), com a medição mais
recente e a data dela. "Não medido" significa que não existe número. A última coluna diz quem
está trabalhando na lacuna. Os detalhes estão nas seções datadas abaixo e nos `docs/`.

### Velocidade e tamanho

| Ferramenta | Velocidade | Tamanho e memória | Data |
|---|---|---|---|
| **JS dev** × DDC/webdev | compilar 1 arquivo: **7,6× mais rápido** (19 × 150 ms). Início do zero, sem `build_runner`: **29,6 × 212,5 s** (`limitless_ui`) e **56,1 × 383,3 s** (`new_sali`). Edição até a página, `limitless_ui`: template **3,3 × 7,6 s**, corpo **2,4 × 8,1 s**, API **8,8 × 51,8 s** (máquina carregada: valem as razões). Código gerado ≈ igual ao DDC (`closures` 1,65×, 13/10) | memória **0,5 × 4,9 GB**; JS em gzip **4,4 × 5,4 MB** (bruto 56 × 49 MB) | 31/10 noite |
| **JS produção** × `dart2js -O4` | ganha em `collections/set_str` (0,54×), `map` (0,79×), `list_sort` (0,85×) e `map_str_int` (13,4 × 15,6 ms); empata em `async_await` (464 × 466 ms), `collatz` (150 × 150), `Set<int>` (16 × 15), `list_methods` (37 × 36), `map_iterate` e `closures`; **perde** em `jsonEncode` 1,0–1,2×. Compila **2–2,8× mais rápido** (-O2) | projetos grandes **0,90–0,94×** bruto, **0,96×** gzip; 16 programas pequenos **0,915×** bruto e **0,963× gzip** (o `json` de `bench/performance` sozinho, 1,034× gzip) | 01/11 |
| **LSP** × servidor de análise | `new_sali/frontend`, revezado, com CPU (décima sétima rodada, carga 11–38). **Pedidos** (mediana, sem edição / depois de edição): `references` **0,4–0,8** / 36,7–58,6 × 27,3–30,5 / **19,2–25,4 ms** (depois de edição não ganha de forma estável), `implementation` **0,3 / 2,3–2,4 × 12,3–13,4 / 10,8–32,0 ms**, `prepareRename` **0,4 / 1,7–1,9 × 1,1 / 2,1–11,5 ms**, `hover` **0,5–0,6 / 1,2 × 1,2–1,3 / 3,6–24,2 ms**; as consultas de biblioteca saem do residente. **Sessão longa** (`new_sali` inteiro, 150 rodadas, 2 pares): onda **71–78 × 109–114 ms**, edição de corpo **30–31 × 34–39 ms**, mudança de API **46–49 × 100–112 ms**. **Windows** (i3, 2 pares de 150 rodadas): onda **221–223 × 400–443 ms**, API **124–125 × 265–426 ms**; a edição de corpo ainda perde (**76 × 60 ms**) | sessão de pedidos: repouso **791–794 × 814–944 MiB (ganha; era 1.218–1.227)**; sessão longa: pico/repouso **579/579 × 717–726/703–714 MiB**, inclinação **+2,6; +1,1 × +35,3; +36,9 MiB por 100 cargas**; Windows: bytes privados no fim **370–373 × 705–710 MiB**. Oráculo de paridade igual ao das rodadas 15 e 16 | 03/11 |
| **`analyze`** × `dart analyze` | `new_sali/backend`: ~5 s × 21,7 s frio (2,6 s com cache); `format`: **0,42 × 1,55 s** | — | 13–14/10 |
| **`test`** / **`pub`** × `dart test` / `dart pub` | **não medido** | — | — |
| **Wasm** × `dart2wasm` (Node 24) | **perde** 1,2–3× em alocação/árvores, JSON, strings e crivo (ganho de só 10–20% em 31/10 noite; o coletor pesa ~25% no JSON); **ganha** 2–8× em `list.add`, `list[i]` e `sort` | módulo **0,68–1,72 MB** sem Binaryen e **0,57–1,43 MB** com o `wasm-opt -Oz` opcional do `--optimize` × 47–114 KB do dart2wasm (hello **565 KB × 47 KB, 12,0×**; era 1,30 MB e 28× em 02/11, 659 KB e 14× na primeira rodada de 03/11, 599 KB e 12,7× na segunda); gzip do hello 208 × 15 KB; memória **2,3–3,4× menor**. Exceções nativas do Wasm (`exnref`) são o padrão | tamanho: 03/11 (seções de 03/11: terceira rodada — registros de classes do SDK como dados da ligação, Ryū próprio no `double.toString`, *heap* nunca destruído no wasm; segunda rodada — tabela RTI só com as classes vivas, mapa compacto do runtime, guarda do `noSuchMethod` corrigida em 3 caminhos; primeira rodada — `noSuchMethod` só com invocação dinâmica, só as bibliotecas carregadas, dados zerados fora do módulo, Binaryen; velocidade igual dentro do ruído nas duas); velocidade: 31/10 noite; contra outras linguagens: seções de 02/11 e 03/11 |
| **AOT nativo** × `dart compile exe` | HTTP `dart:io`: **68–69,5 × 70,2 mil pedidos/s (97–99%)**; ciclos por pedido: tracing 66 mil × 70 mil (melhor), ARC real 205 mil (pior). Trees 12,4 × 11,9 ms; JSON 77,5 × 78 ms; strings 43 × 47 ms. Swift cru: 26 mil ciclos × nosso 38 mil. Compilar `new_sali/backend` com `--optimize` (macOS, objetos frios): **451 × 39 s de CPU (11,6×)**, mediana de três medidas com carga 45–116 (relógio sem significado com essa carga); era ~1 410 s em 31/10 de manhã. Desenvolvimento frio: 177 s de CPU | servidor **4,7 × 5,6 MB**; hello 0,56×; `new_sali/backend` **36,4 × 26,5 MB (1,37× maior**; era 43,5 MB); File Explorer 19,9 × 12,3 MB (1,62×); RSS do servidor 17,5 × 17,2 MB (ARC real 23,8 MB) | 30–31/10 (compilação e tamanho do grande: 01/11) |
| **ARC real** × tracing (mesmo servidor, `DARTFORGE_ARC_PREPARED=1 --memory arc`) | instruções por pedido: `dart:io` **666 × 200 mil (3,3×)**, cru **248 × 123 mil (2,0×)**; ciclos por pedido (carga ~37): `dart:io` 306 × 123 mil (2,5×), cru 98 × 56 mil (1,7×). No início de 01/11: 813 e 361 mil instruções, 343 e 122 mil ciclos | RSS 25 × 18 MB (`dart:io`), 22 × 16 MB (cru) | 01/11 |

### Corretude

| Ferramenta | Resultado | Lacunas abertas |
|---|---|---|
| **Suíte `tests/language`** (2.863 programas) | Medição de 02/11 (sexta rodada), **no Windows**, binário final, referência `vm-win-r6` (a VM oficial com os experimentos de cada teste e os pacotes `meta`/`collection` nas versões da 3.6.2) e, no JS dev, o DDC 3.6.2: **JS dev 2794 iguais, 48 divergências do DDC (o DDC difere da VM do mesmo jeito), 16 recusados pelos dois, 4 passam onde a VM falha, 1 falha**; **nativo AOT** (`dartforge aot --optimize`) **2832 iguais, 16 recusados, 2 passam onde a VM falha, 13 falhas (98,9%)**; **wasm** (`wasm-win-final`, terminada em 03/11 às 03h43) **2828 iguais, 3 iguais só ao dart2wasm, 16 recusados pelos dois, 2 passam onde a VM falha, 14 falhas (98,9%)**: as do nativo menos `stack_trace/full1`, `stack_trace/demangle_ctors` e `augmentation_libraries/class_augmentation`, mais as 4 `canonicalize/hashing_shallow_collision_*` (*hash* de constantes). Wasm no Mac em 03/11 (`wasm-size2-final`, referência `vm-r6`, depois da segunda rodada de tamanho): **2829 iguais, 5 iguais só ao dart2wasm, 16 recusados pelos dois, 2 passam onde a VM falha, 11 falhas**, nenhuma parada na guarda do `noSuchMethod`. Rodada 5 (Mac, nativo e wasm em projeção): JS 2780/13/70, nativo 2810/13/40, wasm 2815/13/35. JS produção: não medido | As 13 do nativo: 9 de pilha simbólica (o AOT sai com `--stack-trace none`), 2 de *hash*, 2 em que o CFE da VM quebra (252). A do JS dev: `least_upper_bound_greatest_closure_2`, em que o CFE quebra na VM e no DDC |
| **`analyze`** | corpus do analisador **99,95%** (23.001/23.012), 14/10 e de novo em 31/10. Erros a mais em `tests/language`: **231 → 107 → 20** (de 70 para 7 arquivos), 31/10, sem novos falsos negativos (os erros dos 1.460 programas de erro ficaram idênticos) | 12 são do *parser*: declarações `augment` sem o experimento `macros`. 6 são *wildcards* fora do modo *wildcard*: com `_` duplicado, o analisador liga à primeira declaração e nós à última. 2 vêm de `X1 extends FutureOr<X1>`, em que o próprio 3.6.2 não diagnostica |
| **`pub`** | solucionador **PubGrub** portado do Pub 3.6.2. `pub get` idêntico ao oficial em **51/51** projetos reais (lockfile byte a byte, `package_config.json`, saída e código), com e sem lockfile. `add`, `remove`, `upgrade`, `downgrade`, `deps` e `outdated` idênticos em 42 cenários (31/10). **`cache`** (`add`/`clean`/`list`/`preload`/`repair`) e **`global`** (`activate` hosted/Git/path, `deactivate`, `list`, `run`, *bin stubs*) em 02/11: 32 comandos de `cache` com transcrição e árvore do cache idênticas; 48 de `global` (dois roteiros) com transcrição, árvore e arquivos do cache idênticos; só a ordem das linhas `Built` pode variar, porque no Pub as compilações são paralelas. **`publish`/`lish`** em 02/11: 116 comandos diferenciais (uso, 25 pacotes montados para acionar cada validador, 12 pacotes reais do *pub cache*, envio contra servidor falso local com 10 modos de falha) com transcrição idêntica (fora a porta local que o próprio Pub muda a cada execução no relato de conexão recusada), 69 requisições ao servidor idênticas (inclusive o SHA-256 do arquivo enviado) e os 20 arquivos `.tar.gz` idênticos byte a byte; o gzip é um porte do zlib do Chromium da VM (o zlib clássico dá outros bytes) | `login` e o envio ao pub.dev sem token (OAuth2; `token`, `logout` e `uploader` idênticos em 31 comandos, 02/11); sugestões depois de falha de resolução; avisos de segurança no `outdated`; `--precompile`; SDKs Flutter e Fuchsia. No `global`, o DartForge guarda executáveis nativos onde o Pub guarda *snapshots*. Matriz em `docs/PUB-E-TEST.md` |
| **`test`** | desde 02/11 roda o próprio runner do package:test (`Configuration`, `Runner`/`Loader` portados, `Engine` e todos os relatores) com as suítes compiladas pelo DartForge em volta do *bootstrap* nativo do test_core. `-r json` com o protocolo do package:test: fluxo de eventos idêntico ao do `dart test` (59 eventos, 4 suítes) fora `time` e quadros de pilha; `compact`, `github`, `failures-only`, `silent`, `--file-reporter` e `--fail-fast` iguais fora tempo e quadros | `line`/`column`/`url` do JSON saem nulos (o executável nativo ainda não tem quadros de pilha); `--coverage` recusado (fidelidade impossível sem o compilador da VM, análise em `docs/PUB-E-TEST.md`); só a plataforma `vm`; seleção por linha/coluna |
| **JS produção** | corpus 277/277 (-O1/-O2), 260/260 (-O3), 256/256 (-O4) | — |
| **Modos do runtime no wasm** | tracing + pilha-sombra; `--exceptions tables` (`exnref`) é o padrão desde 31/10; `--exceptions check` disponível | ARC desligado no wasm32 (medido: mais lento e +6,8% de tamanho); mapas de raízes indisponíveis (o wasm não expõe a pilha) |

### Quem está trabalhando em cada lacuna (desde 31/10, 15h)

| Frente | Agente |
|---|---|
| Wasm: tamanho (hello 12,0× o dart2wasm com Binaryen; runtime Rust ~400 KB é o maior resto) e velocidade (coletor, `List<bool>.filled` não desvirtualizado) | três rodadas de tamanho em 03/11 concluídas (hello 1,30 → 0,78 → 0,72 → 0,68 MB, 0,57 MB com Binaryen); próximos: achar a chamada que o V8 deixa de inlinizar quando só as tabelas vivas são registradas (~16 KB), o `%f` exato do `toString` SIMD (tira os 23 KB de formatação de `f64` do `core::fmt`), cone do tipo estático do receptor na poda (o `_Double`), produtores dos ajudantes de erro pelo grafo de chamadas do runtime, coletor menor |
| Wasm: perfil de compatibilidade (sem `exnref`, sem *tail calls*, sem `extended-const`), com flag própria | **anotado em 03/11, a fazer** (seção de 03/11) |
| Wasm: isolates em threads com memória compartilhada (atomics), flag experimental | **anotado em 03/11, a fazer** (seção de 03/11) |
| `analyze`: falsos positivos do verificador de tipos (107 → 20) | concluído em 31/10; os 12 do *parser* (`augment`) ficam para o *parser* |
| JS dev: subir de um checkout limpo sem `build_runner`; edição de template | concluído em 31/10 (29,6 × 212,5 s; template 3,3 × 7,6 s) |
| JS produção: `jsonEncode` no máximo igual ao `dart2js -O4` (`async_await` e o gzip dos programas pequenos resolvidos em 01/11) | pausado em 01/11: perfil do `jsonEncode` sem ponto concentrado (1,0–1,2×); falta provar no mundo fechado que `withIndent` não é alcançado |
| `pub` (PubGrub, `add`/`remove`/`upgrade`/`outdated`/`deps`) e `test` (relator `expanded`) | concluído em 31/10 (seção abaixo) |
| `test` com o runner do package:test (JSON, `compact`, `github`, `--fail-fast`, `--file-reporter`); `pub cache` e `pub global`; `--coverage` analisado e recusado | concluído em 02/11 (seção abaixo); `pub publish` concluído em 02/11 (seção abaixo); `login` fica aberto |
| Backend nativo (AOT e wasm): as 17 falhas reais restantes da suíte `tests/language` (de 88; mais os 23 de experimento) | agente backend nativo (quinta rodada concluída em 02/11, com a medição nova e os caminhos do Windows) |
| Régua da suíte `tests/language`: Dart oficial medido em 01/11 no notebook Windows (i3-1215U, SDK 3.6.2, `upstream_suite.py` em `/Volumes/ssd_dock/tmp/language-suite`, resultados `vm-win*.json` e `aot-win-flags.json`). **JIT sem opções**: igual ao `vm.json` do Mac em 2857/2863 (os 6 diferem só em caminhos de pilha e *hashes*). **JIT com os `--enable-experiment` de cada teste**: **2821/2863 (98,5%)** saem com 0; 40 recusados viram aprovados; sobram 42 (8 `macros`, 5 `test_runner` feitos para falhar, avulsos). **AOT** (`dart compile exe`, mesmas opções): **2822/2863 (98,6%)**; difere do JIT oficial em 32, dos quais 22 são recusados pelos dois, 6 só caminho/*hash*/tempo e **4 diferenças reais** (2 de pilha em `stack_trace/`, 2 de `static_weak_reference`, que só o AOT implementa). Achado: o `package_config` da suíte não tem `meta` nem `collection`, e 4 programas são recusados por isso em todas as ferramentas. Falta: o modo `dartaot` dentro do `scripts/run-language-suite.py`, um modo para o nosso JIT e comparar nosso AOT/JIT com o oficial com as mesmas opções (depende da leitura de `SharedOptions` em andamento no agente do backend nativo) | medição do oficial feita em 01/11; modo `dartaot` e leitura de `SharedOptions` integrados ao script em 02/11 (quinta rodada do agente backend nativo); falta o modo do nosso JIT |
| AOT nativo: tempo de compilação de programa grande (11,6× a CPU do `dart compile exe`) e tamanho (1,37×); próximo: mapa de campo, `$c`/`$g`/`$tc`, `$async` | em rodízio: agentes de desempenho um por vez desde 01/11 (ordem LSP → ARC → AOT); seção de 31/10 abaixo |
| ARC real: 2,5× os ciclos do tracing no HTTP `dart:io` e 1,7× no cru (eram 3× e 2,2×); próximos passos na seção de 01/11 | em rodízio: agentes de desempenho um por vez (fila: LSP prioridade → AOT → ARC) |
| LSP: edição de corpo no Windows (fases de CPU do residente: *heap* e caminhos), inclinação ≤ 0 da sessão longa, recarga no lugar casando os elementos por nome, ordem dos arquivos conhecidos do `workspace/symbol`, consultas de projeto ainda no servidor | **prioridade** (seção no topo): agente LSP, primeiro da fila de desempenho |
| CI (`CI` e `Heavy`) | 04/11: `CI` verde em `87cb092b` e `v0.1.0-alpha.4` publicada. Falta baixar os 105 min do Windows (seção "fechamento do dia" abaixo) |

## 03/11/2026: fechamento do dia — o que foi feito e o que falta

### Feito em 03/11

- **LSP, décima quinta rodada** (no GitHub, `abc9a187`):
  - `references`, `implementation` e `prepareRename` respondem de um índice: 3,7 ms contra 20,7 do Dart, sem edição;
  - a sessão longa ganha as três cargas no macOS;
  - o mimalloc não foi adotado no Windows (regra do proprietário de 02/11).
- **LSP, décima sexta rodada** (`main` local, validada pela v27):
  - todos os pedidos medidos ganham do Dart, sem edição e depois de edição;
  - a sessão longa ganha as três cargas no macOS, com memória plana (527–574 contra 686–756 MiB);
  - no Windows ganha a onda e a mudança de API, com memória estável;
  - corrigido um defeito real: duas bibliotecas "quentes" que se importam publicavam "Undefined class" em quem as importa.
- **Wasm, primeira rodada de tamanho** (no GitHub, `ed55371b`):
  - hello de 1,30 MB para 0,78 MB (0,66 MB com Binaryen);
  - a regressão de 10× do `863d4159` foi corrigida no wasm e no nativo.
- **Wasm, segunda rodada de tamanho** (`main` local):
  - hello de 0,78 para 0,72 MB (0,60 MB com Binaryen);
  - tabela RTI só com as classes vivas, o que tirou o descompressor;
  - mapa compacto no runtime;
  - 3 furos da guarda do `noSuchMethod` corrigidos;
  - suíte wasm em 2829 iguais e 11 falhas (eram 2826 e 14).
- **Estudo do AssemblyScript** (coletor em ~5 KB) e as anotações do perfil de compatibilidade do wasm e dos isolates com memória compartilhada (seções abaixo).
- **Windows:**
  - lint `is_multiple_of` do clippy corrigido no `space.rs`;
  - os 4 testes de caminhos `\\?\` corrigidos em 02/11.
- **GitHub:** a proteção contra segredos recusava as chaves falsas da AWS dos testes do `leaks.rs`. Os literais agora são partidos com `concat!`.
- **Validações completas no macOS:**
  - v25, v26 e v27 passaram em tudo: nativo dev, `--optimize`, `--gc-stress`, wasm e programas grandes 164/164; JS dev e produção 277/277;
  - suíte de linguagem sem regressão contra a `vm-r6`;
  - `cargo test`, doc e clippy limpos.
- **CI** (`2cabc620`):
  - o teste do `pub global` no Windows lia o nome do *stub* do Unix (no Windows é `<executável>.bat`);
  - dois testes nativos com `--gc-stress` não definiam o registro de tear-offs da biblioteca do SDK (`7af464d1`).

- **LSP, décima sétima rodada** (`main`):
  - as consultas de biblioteca saem do residente: memória em repouso de uma sessão com pedidos em **791–794 contra 814–944 MiB** (era 1.218–1.227; agora ganha);
  - a primeira edição da sessão não carrega mais a biblioteca no servidor;
  - o Windows foi amostrado com `xperf`: `stat` 10%, *heap* 16%, núcleo 19%, caminhos ~10%.
- **Wasm, terceira rodada de tamanho** (`main`):
  - hello de 0,72 para 0,68 MB (0,57 MB com Binaryen), velocidade igual dentro do ruído;
  - registros de classes do SDK como dados da ligação;
  - Ryū próprio no `double.toString`, que corrige uma divergência antiga com a VM (empates para o dígito par);
  - achada a causa da lentidão no V8 quando se registram só as tabelas vivas: a inlinização do TurboFan. Falta saber qual chamada a perde.
- **`Heavy`, corrigido no `main`:**
  - **`253_downcasts_para_parametros_e_campos_de_record` no nativo:** `r.$1(args)` em receptor de tipo record chamava um método `$1` em vez de ler o campo (`dde7d472`);
  - **`273_instanciacao_para_limites` no nativo:** a closure genérica chamada sem argumentos de tipo agora lê os parâmetros da classe do `this` capturado (`abdc09af`);
  - **`corpus/modern` 340 e 341 no JS, regressão do `9bd8e2c6`:** classes com construtor generativo ficam fora do inicializador de mixin legado (`5a2ffdcd`). A primeira versão dessa correção excluía também as classes só com *factory*, e a v29 pegou a regressão em `mixin_legacy/factory_constructor_test` (corrigida em `a629a91e`).
- **Validação final, v30, macOS, `a629a91e`:**
  - nativo dev, `--optimize`, `--gc-stress`, wasm e programas grandes: 164/164;
  - `corpus/js`: JS dev e produção 277/277; nativo 277/277, normal e com `--gc-stress`;
  - `corpus/modern`: 26/26 nos dois perfis;
  - suíte de linguagem no JS dev contra a `vm-r6`: 2794 iguais, 49 falhas, sem regressão;
  - `cargo test --release --workspace`, testes ignorados do `emit_native`, doc e clippy limpos.

  A suíte `tests/language` inteira **não** foi repetida no wasm e no nativo depois das rodadas de hoje (última medição completa do wasm: rodada 2 de tamanho). Os testes do `Heavy` no Windows dependem do CI.

### Madrugada de 04/11: CI verde e versão

- **`CI`, `Heavy` e `No system toolchain` verdes em `526e87d9`.** Faltavam duas correções, nenhuma de regressão do compilador:
  - o runtime wasm herdava o `lto=false` e as 16 unidades do job (758 KB em vez de 675 KB no hello; 759 KB reproduzidos aqui); agora o `cargo` interno ignora esses dois (`crates/emit_native/build.rs`);
  - os testes ignorados do Windows pegavam `matches_a_corpus_of_the_vm`, que exige um corpus da VM.
- **`v0.1.0-alpha.3` publicada e incompleta.** Depois de publicar, testei o pacote baixado: `aot`, `compile-native`, `run`, `pub`, `analyze` e `format` funcionam, mas o defeito é antigo e o `release.yml` só exercitava o AOT:
  - `compile-wasm` não achava o runtime wasm nem um `lld` com esse nome;
  - `compile-js`, `dev` e `serve` procuravam `dart_sdk.js` no caminho da máquina que compilou;
  - `dartforge-lsp` e `dartforge-jsprod` não iam no pacote.

  As notas da alpha.3 avisam disso e apontam para a alpha.4. Corrigido em `7b6b1fbe`. O `release.yml` agora exercita esses compiladores e o LSP, e o `ci.yml` o wasm.
- **Um clang qualquer tomava o lugar do gerador embutido** (`87cb092b`). O passo novo do `ci.yml` reprovou `7f0bfb76` no Linux e no macOS. O pacote não leva clang, a busca terminava no `PATH`, e `Generator::clang_if_any` aceitava qualquer versão para o bitcode de produção. O clang dos runners não conhece as opções de wasm do LLVM 22. Agora só vale um clang da linha do LLVM embutido (22.1).
  - **`env -i PATH=/usr/bin:/bin` não basta como ambiente limpo no macOS:** o Apple clang 21 desta máquina aceita essas opções, e a validação local de `7b6b1fbe` passou usando ele sem aviso. Para reproduzir, usei um `clang` falso no `PATH`, que responde `--version` como 18.1.3 e falha no resto.
- **`v0.1.0-alpha.4` publicada** em `87cb092b` (pré-lançamento, 3 pacotes; o `Release` passou nos três sistemas). Pacote de macOS baixado do GitHub e testado com o clang falso no `PATH`: `aot`, `aot --optimize`, `run`, `compile-wasm` (hello de 667 KB; o módulo rodou no `dartforge-wasm-run`), `compile-js`, `dartforge-jsprod` no Node e `dartforge-lsp --version`.
- **Achados ao testar o pacote alpha.4** (correções posteriores na seção de 04/11):
  - `dartforge-wasm-run` não vai no pacote, então o módulo só roda no navegador;
  - `dartforge --version` responde `unknown command`;
  - no `dartforge-jsprod`, uma exceção não tratada imprime a mensagem, depois um dump de `TypeError` e sai com código 1 em vez de 255: o minificador renomeia `process.exit` para `process.L` (o texto vem de `crates/emit_js_production/src/bundle.rs`).
- **CI mais barato** (`d0ffd294`): mudança só de `docs/` ou `*.md` não dispara o `CI`, e o job do Windows usa o mesmo perfil rápido do Linux e do macOS. Esta segunda parte **quase não ajudou**: ver abaixo.

### Por que o job do Windows levava 107 min

Medido nos logs de `edeacf6c`. **Os testes em si não são mais lentos no Windows:** com os mesmos testes em release, `io_regression` leva 4,2 s contra 4,3 s, `memory_cli` 31,8 s contra 70,1 s e as provas de ARC 112 s contra 164 s. A exceção é o `reload_state`: 53 s com 5 testes contra 3,2 s com 7.

| Gasto | Windows | Linux |
|---|---|---|
| Rodar a suíte em debug (normal 7,7 + ignorados 37,6) | 45 min | não roda este passo |
| Recompilar antes de cada prova de ARC | 21,6 min | 3,8 min |
| Compilar em debug (5,9 + 9,6) | 15,5 min | não compila |
| Compilar em release | 14 min | 7,8 min |

**Medido depois de `d0ffd294`, com cache (`87cb092b`): 105 min.** Tirar o LTO e usar 16 unidades levou a compilação em release de 14 para 12,9 min e quase nada mudou no resto. O run anterior, de 106 min, não serve de comparação: mudar as variáveis `CARGO_*` invalidou o cache. Os passos agora são:

| Passo do Windows | min |
|---|---|
| Testes ignorados em debug | 48,3 |
| Suíte normal em debug (compilação incluída) | 21,7 |
| Recarga JIT pelo CLI (compilação em release incluída) | 12,9 |
| Contratos do SDK | 6,2 |
| Getters e setters | 4,8 |
| Campos tipados | 3,1 |

Dentro dos ignorados, um único binário de teste com 1 teste leva 621 s, e outros levam 487, 293 e 243 s.

**Comparação com o CI do Dart SDK** (`tools/bots/test_matrix.json` e a branch `infra/config`, lidos em 04/11): não roda só `dart/language`, e testa bem mais que o nosso. São 50 builders automáticos antes de cada commit, 115 só sob pedido, cerca de 80 jobs depois do commit e 73 por noite. A espera é curta porque há filtro por caminho (`docs/x.md` dispara 4 builders, `tests/language/` 25, e só `runtime/vm/os_win.cc`, entre os caminhos que simulei, dispara Windows) e o passo principal de testes da VM usa de 16 a 20 máquinas. Mediana de 13,5 min por builder; `vm-win-release-x64-try`, 12,2 min. Nós fazemos push direto no `main`, então nosso CI equivale só à etapa depois do commit: o Windows continua rodando em todo push de código.

### A partir daqui

- Fechar a paridade de `workspace/symbol` nas condições ainda divergentes: a dupla atual
  dá 9/24 no frio e 24/24 no quente; o Dart continua variando mesmo com o mesmo cache.
  Conferir dependências condicionais e descoberta posterior às consultas. Preservar o
  histórico da tabela de drivers quando muitos contextos fecham/reabrem; a VM já
  confirmou uma inversão de ordem após remover 40 chaves sem limpar a tabela.
  Depois medir a abertura dos contextos e repetir CPU/latência/memória. Os gates locais
  foram repetidos com sucesso; detalhes e limites na continuação de 04/11 acima.
- Testes ignorados do Windows em debug (48,3 min), o maior gasto do job: medir em release, aproveitando a compilação que já existe. O `Heavy` roda no Windows diariamente, mas não cobre todos esses testes (conferência de 04/11 acima); antes de transferir o passo, acrescentar e validar a cobertura que falta.
- O perfil sem LTO no Windows economiza só cerca de 1 min: avaliar se vale mantê-lo.
- Validar o pacote com as correções dos três achados listados acima (seção de 04/11), antes de publicar outra versão.
- A suíte `tests/language` inteira continua sem repetição no wasm e no nativo; o gerador usado pelos pacotes (embutido) também não foi medido nela.

### A fazer (próximos dias)

- **LSP:**
  - inclinação ≤ 0 em toda sessão longa (+2,6 e +1,1 MiB por 100 cargas; o Dart, +35 a +37);
  - edição de corpo da sessão longa no Windows (76 contra 60 ms; o residente leva ~50 ms lá contra ~19 no macOS, por causa do *heap* e dos caminhos);
  - `references` logo depois de uma edição (36,7–58,6 contra 19,2–25,4 ms, com poucas amostras);
  - consultas de projeto ainda no servidor (`rename`, hierarquias completas, buscas sem índice);
  - recarga no lugar casando os elementos por nome;
  - `workspace/symbol`: validar a ordem nos projetos grandes e depois de mudanças no
    workspace; os quatro projetos pequenos passaram em 04/11 (rodada 18).
- **Wasm:**
  - chegar perto dos 158 KB do TinyGo no hello: registros do SDK por biblioteca, `double` sem `core::fmt`, coletor menor, cone do tipo estático do receptor (`_Double`), ajudantes de erro pelo grafo de chamadas;
  - velocidade (perde 1,2–3× em alocação, JSON e strings);
  - perfil de compatibilidade com flag própria;
  - isolates em threads com memória compartilhada (flag experimental).
- **Suíte `tests/language`:**
  - nativo, 13 falhas: 9 de pilha simbólica, 2 de *hash*, 2 em que o CFE da VM quebra;
  - wasm, 11 falhas;
  - JS dev, 1 falha;
  - JS produção, não medido.
- **JS produção:** `jsonEncode` 1,0–1,2× o `dart2js -O4`.
- **AOT nativo:**
  - tempo de compilação de programa grande (11,6× a CPU do `dart compile exe`);
  - tamanho (1,37×).
- **ARC real:** 2,5× os ciclos do tracing no HTTP `dart:io`.
- **`pub`, `test` e `analyze`:**
  - `pub login` (OAuth2);
  - `line`/`column`/`url` no JSON do `test`;
  - os 12 falsos positivos do `analyze` vindos do *parser* (`augment`).
- **Limpeza:**
  - os 13 *worktrees* antigos foram removidos em 04/11; os branches locais foram preservados;
  - `backup/main-local` remoto removido na sessão anterior; não há branches locais `backup/*` nesta conferência.

## 03/11/2026: wasm — rodada de tamanho 3: hello de 0,72 para 0,68 MB (0,57 MB com Binaryen); por que o V8 ficou mais lento com as tabelas vivas

Terceira rodada sobre o `compile-wasm --optimize` (macOS; "antes" é `2accdf2e`, o fim da segunda
rodada; "depois" é `29f4811a`). Rodada encurtada a pedido do proprietário (última do dia, antes de
uma *release*): ficou só o que passou na validação e não atrasou o V8. Bytes do `.wasm` e, entre
parênteses, com `gzip -9`. Scripts, módulos e registros em `/Volumes/ssd_dock/tmp/wasm-size3/` (os
da segunda rodada adaptados, mais `regcalls.py`, as chamadas de cada função de registro, e
`ryutest/`, a comparação em massa do formatador novo).

| programa | antes, sem Binaryen | depois, sem Binaryen | antes, com `wasm-opt -Oz` | depois, com `wasm-opt -Oz` | dart2wasm |
|---|---:|---:|---:|---:|---:|
| hello | 718 123 (225 915) | **675 261** (214 744) | 598 885 (218 865) | **565 219** (208 162) | 47 013 (15 369) |
| árvores (`escaping_objects`) | 771 132 (237 853) | 728 182 (226 495) | 643 900 (231 344) | 610 148 (219 900) | 48 482 (15 878) |
| JSON | 1 762 221 (488 178) | 1 720 981 (477 538) | 1 461 886 (473 954) | 1 429 871 (464 003) | 113 505 (38 499) |
| strings | 844 626 (254 721) | 801 836 (243 873) | 707 386 (248 058) | 673 788 (237 365) | 61 453 (19 861) |
| objetos temporários | 771 295 (237 885) | 728 361 (226 443) | 644 109 (231 398) | 610 358 (219 981) | 48 597 (15 917) |
| coleções | 1 219 917 (333 384) | 1 177 167 (321 928) | 1 026 558 (317 886) | 993 021 (306 590) | 67 496 (21 318) |
| `colecoes` | 1 141 549 (316 369) | 1 098 895 (304 650) | 963 737 (302 419) | 930 292 (291 741) | 65 948 (20 643) |

−2,3% (JSON) a −6,0% (hello) cru, −2,2% a −4,9% com gzip; com o Binaryen, −2,2% a −5,6% cru e −2,1%
a −4,9% com gzip. O hello está a 3,6× do TinyGo (158 KB) com o Binaryen (era 3,8×) e a 12,0× do
dart2wasm (era 12,7×).

**O que cada passo tirou do hello** (sem Binaryen):

| passo | commit | hello |
|---|---|---:|
| antes | `2accdf2e` | 718 123 (225 915) |
| dígitos do `double` com Ryū próprio (`double_shortest.rs`) | `e1c28335` | 717 441 (226 414) (sem a correção do `describe_value`, ver abaixo) |
| registros de classes do SDK como dados da ligação, só as classes vivas (`pruning/class_registry.rs`); o *heap* e o universo RTI nunca destruídos no wasm; o `describe_value` com o texto da VM | `2e69c1a8`, `3f860901`, `e1c28335` | **675 261 (214 744)** |
| `wasm-opt -Oz` | — | **565 219 (208 162)** |

Na divisão por grupos do hello: o grupo `register` (27,8 KB de código em 10 funções `df.register.dart:*`)
saiu (ficam 16 nomes de argumento e 13 ajudantes); a inicialização preguiçosa do *heap* caiu de
21 KB para 13 KB (categoria `std thread_local` de 32,1 para 23,7 KB); a seção de código foi de
618 661 para 581 323 bytes e a de dados de 94 761 para 89 265.

1. **Registros de classes do SDK como dados da ligação** (`2e69c1a8`). Cada biblioteca do SDK
   registrava na partida o nome de toda classe instanciável e toda aresta de subtipo direta, viva ou
   não: no hello, 457 chamadas de nome e 1 697 de aresta. No wasm a biblioteca deixa os nomes como
   comentários (`; df.class-name <cid> <nome>`; as arestas já estavam nas linhas `; df.cone-parent`
   do cabeçalho), o resumo os carrega (formato 11, linha `W`), e a ligação escreve uma cadeia de bytes
   (LEB128: nomes, depois arestas) só com as classes que a tabela RTI guarda (`rti_table::kept`:
   raízes, tabelas vivas, classes citadas por texto vivo, supertipos), fechada sobre as arestas; o
   `df.prepare_isolate` do programa a entrega ao runtime (`dartforge_register_classes`), que faz os
   mesmos registros um por um. Correto pelo mesmo argumento da tabela RTI: um objeto só existe por
   código que cita a tabela da sua classe ou com cid fixo, e todo tipo que o código testa está na
   tabela RTI. O nativo não muda.
2. **Ryū próprio para o `double.toString`** (`e1c28335`). Os dígitos mais curtos vinham do `{:e}` do
   Rust (Grisu com o Dragon de reserva), que também ligava os caminhos de precisão exata do
   `core::fmt`. Agora o runtime calcula com Ryū e as tabelas pequenas (multiplicadores de 128 bits a
   cada 26 potências de 5, mais uma correção de 2 bits por índice; licença BSL-1.0, em
   `THIRD_PARTY_NOTICES.md`). Comparado em massa com o `{:e}` (`ryutest/`: 60 milhões de padrões de
   bits aleatórios, subnormais, inteiros, frações de 1/64 e 1/1000, todo expoente binário com as
   mantissas das pontas, todo 1e±p e m·10^p com os vizinhos; ~370 milhões de valores): **nenhuma
   diferença além dos empates**, 14 633 casos em que o valor exato fica no meio de dois candidatos
   mais curtos. Neles o Rust arredonda para cima e a VM (double-conversion) para o par, como o Ryū:
   `1658206780088562.25` imprime `1658206780088562.2` na VM 3.6.2 e agora no DartForge (antes
   `…562.3`) — **uma divergência antiga corrigida** (nos dois alvos; o teste da VM ganhou os casos).
   O `describe_value` (o texto de um `double` cru) usava `{:.1}`/`to_string` do Rust e agora usa o
   texto da VM (`1e+21`, `1e-7`, `Infinity` em vez de `1000000000000000000000.0`, `0.0000001`, `inf`).
   **O ganho de tamanho foi pequeno no hello (−0,7 KB)**: a formatação de `f64` do `core::fmt`
   (23 KB) continua viva pelo `toString` de `Float32x4`/`Float64x2` (`simd.rs`, `percent_f`, o `%f`
   da VM via `{:.6}`), que está vivo porque as classes SIMD têm cid fixo e a tabela delas é
   registrada. Trocar isso pede o `%f` exato com arredondamento par (o do `printf`, não o meio-para-
   cima do `toStringAsFixed`) e não foi feito.
3. **O *heap* e o universo RTI nunca destruídos no wasm** (`3f860901`). O wasm tem uma thread só e
   os *thread-locals* dela nunca são destruídos, mas a inicialização preguiçosa do `HEAP` levava a
   destruição de tudo o que o *heap* tem (148 chamadas de `dealloc`, ~8 KB). No wasm32 os dois ficam
   num `ManuallyDrop` (`IsolateCell`); um isolado nativo continua liberando o seu ao terminar.

**Por que registrar só as tabelas vivas atrasa o V8** (o item revertido da segunda rodada,
`29c56434`). Medido de novo com um binário que gera os dois módulos (variável de experimento, fora
dos commits): `strings/hashes` 13,7 × 16,0 ms no mínimo (+17%), `strings/build` 37,8 × 43,1 ms
(+14%), 8 rodadas alternadas, carga ~21. O que **não** é:
- **arranjo da memória**: com 0, 3 000, 20 000, 70 000 e 300 000 bytes alocados e vazados na partida
  (deslocando tudo o que o `dlmalloc` entrega depois), a diferença fica (13,5–14,2 × 15,0–16,4 ms);
- **o código**: com os nomes guardados, as funções dos dois módulos são iguais byte a byte, a não
  ser pelo LEB128 dos índices de função (−1 byte em ~30 funções; `describe_ref` −72) e pelas funções
  mortas que saíram (getters `df.mt.*`, `df.list_grow`, `df.rti_fill`); o combustível do Wasmtime é
  o mesmo;
- **o Liftoff**: com `--no-liftoff` a diferença fica (14,3 × 16,6 ms).
O que é: **a inlinização do TurboFan**. Com `--no-wasm-inlining` os dois módulos ficam iguais e
lentos (17,1 × 16,3 ms): o módulo com todas as tabelas ganha uma inlinização no laço quente que o
módulo renumerado perde. A decisão do V8 depende de algo que só a numeração/ordem das funções muda
(os candidatos e o orçamento são processados por índice e contagem de chamadas). Não foi isolado
qual chamada perde a inlinização, e por isso as tabelas mortas **continuam registradas**: é uma
decisão frágil do V8 (outra mudança de tamanho pode virá-la para qualquer lado), e o caminho para
ganhar os ~16 KB sem a perda é achar a chamada (`--trace-wasm-inlining` do d8/Node) e torná-la
barata para o LLVM inlinizar antes (ou estável, com `alwaysinline` no lado do runtime). As três
mudanças desta rodada não mudaram `strings/hashes` (13,5 × 13,5 ms, abaixo).

**Velocidade** (Node 24, CPU da thread principal, mediana das iterações quentes e mínimo entre
parênteses, em ms; módulos com Binaryen; 5 rodadas alternadas de todos os programas; carga da
máquina 8–30):

| núcleo | antes | depois |
|---|---:|---:|
| árvores (`trees`) | 27,7 (25,1) | 26,1 (24,6) |
| lista ligada | 17,0 (14,7) | 17,2 (14,8) |
| JSON decode grande | 108,8 (100,9) | 113,0 (100,7) |
| JSON encode grande | 113,3 (103,2) | 114,5 (100,2) |
| JSON `utf8_bytes` | 72,9 (64,4) | 68,6 (62,6) |
| strings `build` | 43,8 (36,9) | 43,8 (38,3) |
| strings `hashes` | 14,8 (13,5) | 14,1 (13,5) |
| objetos temporários (`points`) | 10,8 (9,4) | 9,8 (9,5) |
| crivo | 20,0 (16,3) | 20,0 (17,0) |
| `list_sort` | 75,6 (68,6) | 78,1 (71,4) |
| `map` | 29,7 (21,4) | 27,6 (22,3) |
| `set_str` | 13,9 (13,4) | 14,5 (13,6) |

Tudo dentro do ruído (±10% na mediana nesta máquina compartilhada); `list_sort` e o crivo ficaram
3–4% acima no mínimo, `map`, `points` e `utf8_bytes` abaixo.

**Validação** (macOS, binário final `x2`, o mesmo código dos commits; carga da máquina 40–110
durante quase toda a validação): diferencial do `corpus/native` (164 programas, `--exec-limit 60`):
**wasm sem Binaryen 164/164**, **wasm com Binaryen 164/164**, **wasm `--gc-stress` 164/164**;
**nativo `--optimize --gc-stress` 164/164**, **nativo de desenvolvimento 164/164** e **nativo
`--optimize` 163/164 terminados, todos iguais** — o último, `122_literais_grandes_constantes` (o de
link mais longo), não terminou antes do fim do dia (uma segunda execução do mesmo modo escreveu no
mesmo registro; as duas foram paradas). `cargo test --release` de `dartforge-runtime` e
`dartforge-emit-native`: **1 668 passam, 0 falham** (com os testes novos: o formatador contra o antigo
fora dos empates, todo expoente binário, os empates da VM, a cadeia de bytes do registro de
classes). `size_budget.rs` (`--features native --include-ignored`): passa (hello 675 KB, orçamento
novo de 745 KB). `clippy -D warnings` de `dartforge-emit-native` e `dartforge-runtime`
(`--all-targets`) limpo, e o do runtime para `wasm32-unknown-unknown` também; rustfmt limpo nos
arquivos tocados. **Suíte `tests/language` inteira: não feita** (2 h 13 min sem carga na segunda
rodada; com carga 90 não cabia no dia). No lugar dela, uma lista de risco (`risk3.txt`: os nomes com
`double`, `number`, `to_string`, `runtime_type`, `type_literal`, `subtyp`, `cast`, `hash`, `print`…;
300 arquivos, 210 deles programas da suíte): dos 210, **205 iguais à VM, 2 passam onde a VM falha
(`variance_*_subtyping`), 3 falhas** — as 3 `canonicalize/hashing_shallow_collision_*` de sempre; os
outros 90 são testes de erro estático fora da suíte. `run-language-suite.py compare --wasm` contra
`wasm-size2-final`: nenhuma regressão.

**O que não foi feito** (rodada encurtada):
- registrar só as tabelas do SDK vivas (−15,6 KB com Binaryen no hello; atrasa o V8, acima);
- a formatação de `f64` do `core::fmt` que sobra (SIMD `toString`) e o resto do `core::fmt` e do
  pânico (17,7 + 7,0 KB no hello): o toolchain é o estável 1.98.1 (`panic=immediate-abort` pede
  nightly, e o nightly da máquina não é usado pela compilação);
- coletor menor (92 KB no hello) além do `ManuallyDrop`: candidatos vistos, não feitos — o
  `memory_report` (3,7 KB, estatística), `describe_object` (12 KB), `loop_status` (8 KB);
- o cone do tipo estático do receptor (`_Double` 18 KB) e os produtores dos ajudantes de erro.

## 03/11/2026: wasm — rodada de tamanho 2: hello de 0,78 para 0,72 MB (0,60 MB com Binaryen); guarda do `noSuchMethod` corrigida

Segunda rodada sobre o `compile-wasm --optimize` (macOS; "antes" é `ed55371b`, o fim da primeira
rodada; "depois" é `cba2a5e1`). Bytes do `.wasm` e, entre parênteses, com `gzip -9`. Os programas são
os de `bench/performance` e o `hello` do `crates/cli/tests/size_budget.rs`; `colecoes` é o da
comparação entre linguagens. Scripts, módulos e registros em `/Volumes/ssd_dock/tmp/wasm-size2/`:
`sizes.sh`, `speed.py` (Node 24, CPU da thread), `fuel.sh` (o combustível do Wasmtime: operações
wasm executadas, medida determinística do trabalho), `dbg.sh`, `funcs.py`/`cats.py` (o runtime por
categoria), `cg.py` (grafo de chamadas de um módulo), `rtitab.py` (a tabela RTI de um módulo),
`validate.sh`, `guard.py`.

| programa | antes, sem Binaryen | depois, sem Binaryen | antes, com `wasm-opt -Oz` | depois, com `wasm-opt -Oz` | dart2wasm |
|---|---:|---:|---:|---:|---:|
| hello | 782 615 (258 256) | **718 123** (225 915) | 659 347 (256 841) | **598 885** (218 865) | 47 013 (15 369) |
| árvores (`escaping_objects`) | 835 680 (269 861) | 771 132 (237 853) | 704 433 (268 676) | 643 900 (231 344) | 48 482 (15 878) |
| JSON | 1 821 446 (518 047) | 1 762 221 (488 178) | 1 517 497 (509 933) | 1 461 886 (473 954) | 113 505 (38 499) |
| strings | 908 517 (286 775) | 844 626 (254 721) | 767 281 (285 723) | 707 386 (248 058) | 61 453 (19 861) |
| objetos temporários | 835 859 (269 894) | 771 295 (237 885) | 704 650 (268 745) | 644 109 (231 398) | 48 597 (15 917) |
| coleções | 1 283 416 (364 570) | 1 219 917 (333 384) | 1 086 100 (354 755) | 1 026 558 (317 886) | 67 496 (21 318) |
| `colecoes` | 1 205 408 (348 001) | 1 141 549 (316 369) | 1 023 533 (339 808) | 963 737 (302 417) | 65 948 (20 643) |

−3% (JSON) a −8% (hello) cru, −6% a −13% com gzip; com o Binaryen, −4% a −9% cru e −7% a −15% com
gzip. O TinyGo (158 KB) **não foi alcançado**: o hello está a 3,8× dele com o Binaryen (era 4,2×) e a
12,7× do dart2wasm (era 14×).

**O que cada passo tirou do hello** (sem Binaryen):

| passo | commit | hello |
|---|---|---:|
| antes | `ed55371b` | 782 615 (258 256) |
| tabela RTI só com as classes vivas, primeira forma (ainda comprimida, com as linhas `B`) | `e680ea5d` (intermediário) | 769 767 (244 959) |
| tabela RTI sem as linhas `B` e sem compressão abaixo de 24 KB (sai o `miniz_oxide`), e ordenação por inserção dos nomes de uma receita | `e680ea5d`, `98acbe4d` | 748 200 (231 861) |
| mapa compacto do runtime | `6033919d`, `d990540e` | **718 123 (225 915)** |
| (registrar só as tabelas do SDK vivas: 698 183 (219 622); **revertido**, ver abaixo) | `29c56434` → `ebb7ca14` | — |
| `wasm-opt -Oz` | — | **598 885 (218 865)** |

1. **Tabela RTI só com as classes vivas** (`pruning/rti_table.rs`). A tabela registrava toda classe
   com id das bibliotecas carregadas: no hello, 645 classes, 72 KB de texto, comprimido para 18 KB e
   com o descompressor (`miniz_oxide`, 17 KB) dentro do módulo. Agora a descida guarda a tabela por
   classe (`hir::RtiTable`), o módulo wasm a repete em comentários do IR e a montagem da ligação a
   escreve de novo com o ponto fixo da ligação. Ficam: as raízes (formas do runtime, classes de erro,
   `Object` e as de cid fixo, que o runtime cria sem alocação do programa), as classes com tabela de
   métodos viva (os objetos alocados; uma constante cita a tabela pelo seu getter), as classes citadas
   por texto vivo (`C<id>`; os resumos do SDK passaram a guardar as citações de toda constante, não
   só das receitas `F<`) e, fechando, os supertipos de cada uma e as classes dos argumentos de tipo
   deles. As linhas `B` (biblioteca de cada classe) só ficam com os nativos do `dart:mirrors` vivos;
   no wasm só uma tabela a partir de 24 KB é comprimida (o ganho da compressão, ~3/4 do texto, passa
   o tamanho do descompressor), e a forma comprimida tem uma entrada própria no runtime
   (`dartforge_rti_init_table_compressed`), então o descompressor sai de quem não o usa. Hello: 145
   classes, 16 KB de texto sem compressão. A ligação nativa não muda.
2. **Ordenação por inserção dos parâmetros e campos nomeados de uma receita** (`types.rs`): duas
   instâncias da ordenação estável da biblioteca padrão custavam ~11 KB; a receita tem poucos nomes.
3. **Mapa compacto do runtime no wasm** (`runtime/src/hash.rs`, `hash::compact`). O `HashMap` padrão
   (hashbrown) compilava rehash e inserção de novo para cada um dos ~25 tipos de mapa do runtime
   (~40 KB). No wasm32 o `HashMap`/`HashSet` do runtime são agora um vetor de entradas, um vetor de
   hashes e um índice de endereçamento aberto (sondagem linear, remoção com deslocamento para trás)
   de entradas de 64 bits com o hash e a posição: tudo o que move o índice só lê os hashes e é
   compilado uma vez; cada tipo instancia só a comparação de chave e os movimentos do vetor. A busca
   compara os hashes no índice sem ler as entradas, e a inserção usa a vaga onde a busca parou. A
   remoção move a última entrada para o buraco: a ordem de iteração é determinística, e nada no
   runtime depende dela. O nativo continua com o padrão. O código Rust do hello foi de 460 para
   401 KB (o hashbrown saía inteiro; a inicialização do `Heap` cresceu 4 KB, os mapas têm três
   vetores).
4. **Defeito achado no caminho** (`e0d5dce1`): o resumo tomava o primeiro `c"` da linha como início
   da constante, e um nome entre aspas terminado em `c` (`@"df.s.…95c" = …`, um *hash* em dezesseis)
   dava lixo. Isso já escondia receitas `F<` da poda de FFI; com a tabela RTI, os programas 155 e 165
   do `corpus/native` imprimiram `Class1057` no lugar de `D` e `G`. Corrigido (o `c"` da constante é
   o que vem depois de um espaço), com teste.

**Medido e revertido: registrar só as tabelas do SDK vivas** (`29c56434`, revertido em `ebb7ca14`).
Toda tabela montada é registrada na partida, viva ou não (as mortas vazias): no hello, ~620 tabelas
vazias, seus getters e o vetor do registro, ~20 KB. Registrar só as vivas e as de cid fixo é correto
(um objeto só existe por código que cita a tabela da sua classe, ou com cid fixo), e passou no
harness, mas deixou o núcleo `hashes` de `strings` **16% mais lento no Node** (13,2 × 15,9 ms no
melhor caso, 12 rodadas alternadas; com todas registradas de novo, 13,2), enquanto o combustível do
Wasmtime conta o mesmo trabalho nos dois módulos (12 709 967 648 × 12 709 983 046 operações): um
efeito do arranjo do módulo no V8, não do código. Pela regra de não comprar tamanho com velocidade,
ficou de fora até ser entendido.

**O que não deu para cortar e por quê.**
- **`core::fmt`, formatação de `double` (25 KB: `flt2dec` grisu e dragon, exato e mais curto) e
  pânico.** O toolchain é o estável 1.98: a `std` vem pré-compilada, fora do LTO do módulo. Um
  `{}`/`{:e}` de `f64` liga também os caminhos de precisão (o ramo depende do `Formatter` em tempo de
  execução), e os argumentos de uma mensagem de pânico chegam à `std` opaca. Tirar isso pede
  `panic_immediate_abort`/`build-std` (nightly) ou um formatador de `double` próprio com o algoritmo
  mais curto e correto (o `double.toString` da VM) — não feito.
- **`dlmalloc` (16 KB).** Trocar o alocador do wasm por um menor arrisca fragmentação nas páginas do
  espaço de objetos, que vêm dele; não medido.
- **Coletor menor (item 3 do pedido): não feito.** O coletor no hello é ~92 KB (`run_collection`
  21 KB, `mark_pending` 7 KB, `root_list` 5 KB, inicialização do `Heap` 21 KB). O ARC e os mapas de
  pilha já saem do wasm (`ArcSlot` constante, desde antes). Um TLSF + marcação tricolor como o do
  AssemblyScript pede reescrever a geração jovem, o TLAB, as anexações (finalizadores, `WeakReference`,
  `Expando`) e os instrumentos (`--gc-stress`, `DARTFORGE_GC_SCHEDULE`, verificação) sobre outro
  espaço; o ganho de velocidade da coleção geracional nos *benchmarks* de alocação ficaria em risco.
- **Registros por biblioteca do SDK (27 KB no hello).** `df.register.<lib>` registra o nome de toda
  classe instanciável e toda aresta de subclasse da biblioteca (278 arestas só em `dart:core`), viva
  ou não; são código do objeto pré-compilado do SDK. O caminho é o mesmo da tabela RTI: o objeto do
  SDK passa a descrever os registros no resumo e a ligação emite só os das classes vivas e seus
  supertipos. Não feito nesta rodada.
- **`_Double` (18 KB) e os ajudantes de erro (`FormatException` 7,5 KB, `ArgumentError` 6 KB…) sob
  guarda (item 4).** O `_Double` fica porque a análise de seletores é por nome: `c:+`, `c:<` chamados
  em qualquer receptor deixam vivos os pares de toda classe viva que os tem (a tabela do `_Double` é
  viva porque o runtime cria `double`). O que resolve é a análise com o cone do tipo estático do
  receptor em toda chamada (o TFA do dart2wasm), não só no `this` do SDK. Para os ajudantes de erro,
  a guarda precisa dos produtores, isto é, das funções do runtime que, transitivamente, lançam cada
  erro; com o grafo de chamadas do runtime em bitcode isso é calculável, não feito. As exportações JS
  já não custam nada no hello (o LTO as tira; só ficam vivas no grafo da poda).

**Guarda do `noSuchMethod` (item 5): achou três caminhos.** A suíte `tests/language` inteira no
wasm com o binário de antes (`wasm-size2-base`): 3 programas pararam com "internal error: the
program reached `_dartforgeNoSuchMethod`" (saída 70) em vez do erro da VM:
- `extension_methods/static_extension_resolution_7`: membro de um receptor cuja variável de tipo tem
  limite `dynamic` (`S extends dynamic`) — é invocação dinâmica, e agora leva a marca;
- `external_abstract_fields/external_fields`: um campo `external` de instância sem implementação não
  tem acessor na tabela da classe, e a chamada estática cai no `noSuchMethod`;
- `nonfunction_type_aliases/private_names/private_name_implementation`: uma classe que implementa a
  interface de outra biblioteca não pode ter os membros privados dela, e a chamada estática da outra
  biblioteca erra.
Os dois últimos não são invocação dinâmica: um programa que tem um deles registra os ajudantes sem
guarda (`static_calls_may_miss`, conservador: basta implementar um supertipo de outra biblioteca com
membros privados, ou estender um com membro privado abstrato). Corrigido em `a748982b` e `cba2a5e1`; com o binário
final, os três iguais à VM (a suíte inteira com ele está abaixo).

**Velocidade** (Node 24, CPU da thread principal, mediana das iterações quentes e mínimo entre
parênteses, em ms; módulos com Binaryen; rodadas alternadas, 5 de todos os programas e 10 de
`strings` e coleções; carga da máquina 20–31):

| núcleo | antes | depois |
|---|---:|---:|
| árvores (`trees`) | 35,1 (25,1) | 33,4 (24,9) |
| lista ligada | 24,9 (17,6) | 23,2 (15,1) |
| JSON decode grande | 157,8 (145,2) | 163,2 (127,6) |
| JSON encode grande | 168,1 (138,1) | 161,0 (131,1) |
| JSON `utf8_bytes` | 106,5 (91,0) | 99,3 (80,7) |
| strings `build` | 61,2 (37,6) | 57,7 (38,1) |
| strings `hashes` | 20,0 (13,6) | 21,5 (13,5) |
| objetos temporários (`points`) | 11,1 (9,5) | 11,3 (9,5) |
| crivo | 26,0 (17,1) | 26,1 (17,5) |
| `list_sort` | 100,1 (80,4) | 103,0 (84,8) |
| `map` | 54,1 (22,9) | 49,9 (24,8) |
| `set_str` | 20,6 (13,9) | 21,4 (13,7) |

Tudo dentro do ruído, que nesta máquina compartilhada é de ±10% na mediana. O mapa compacto foi
medido também isolado (com as tabelas todas registradas e com o hashbrown no wasm): igual. O
combustível do Wasmtime conta o mesmo trabalho antes e depois (`strings`: 12 807 120 778 operações
antes, 12 709 937 494 depois, −0,8%).

**Validação** (macOS): diferencial `--wasm` do `corpus/native` (164 programas, `--exec-limit 60`)
com o binário final (`cba2a5e1`): **164/164** sem Binaryen (`DARTFORGE_WASM_OPT=0`), **164/164** com
Binaryen e **164/164** com `--gc-stress`. No binário `s4` (o mesmo runtime, antes da reversão e da
guarda, que só mudam o wasm) os seis modos deram 164/164: os três do wasm e o nativo de
desenvolvimento, `--optimize` e `--optimize --gc-stress` (o runtime nativo também mudou: a ordenação,
o conjunto `INSTANTIATIONS` e o índice de strings estáticas com o hasher Fx). O harness achou no
caminho dois defeitos, corrigidos antes do binário final: `Class1057` em 155 e 165 (o `c"` do nome,
acima) e `Instance of 'Object'` no 161 (a guarda sem registro, `cba2a5e1`). Suíte `tests/language`
inteira no wasm (`--ref vm-r6`): antes (`wasm-size2-base`, 2 h 42 min com carga 30–40) **2826 iguais
à VM, 5 iguais só ao dart2wasm, 16 recusados pelos dois, 2 passam onde a VM falha, 14 falhas** (3
delas as paradas da guarda); depois (`wasm-size2-final`, 2 h 13 min) **2829 iguais à VM, 5 iguais só ao dart2wasm, 16 recusados pelos dois, 2 passam onde a VM falha, 11 falhas**: nenhuma regressão (`run-language-suite.py compare --wasm`), as 3 da guarda agora iguais à VM, e nenhum programa chega a um ajudante deixado de fora (`guard.py`: 0 de 2863 com a parada; antes, 3). Testes: `cargo test --release`
de `dartforge-runtime` e `dartforge-emit-native` (1 666 testes, entre eles os novos do mapa compacto,
das citações de classe no resumo e dos comentários da tabela RTI) passam; o `size_budget.rs`
(`--features native --include-ignored`) passa (hello em 718 KB, orçamento de 790 KB). `clippy -D warnings` de `emit_native`, `runtime`,
`cli` (com `native`) e `wasm_host` limpo, e o do runtime para `wasm32-unknown-unknown` também; rustfmt
limpo nos arquivos tocados (os 4 avisos do `cargo fmt --check` são de `emit_js` e `frontend`, de
antes).

## 03/11/2026: wasm — anotado para depois: perfil de compatibilidade e isolates em threads

Dois itens pedidos pelo proprietário em 03/11. Nenhum dos dois foi começado.

### Recursos do wasm: o que o LLVM gera e o que o DartForge usa

O LLVM 22.1.8 gera os seguintes recursos do wasm: MVP, mutable-globals, sign-ext,
nontrapping-fptoint, multivalue, bulk-memory, reference-types, simd128, relaxed-simd, tail-call,
extended-const, exceptions (`exnref` e o formato antigo), atomics, multimemory, memory64 (alvo
`wasm64`), fp16 e wide-arithmetic. Do WasmGC ele só gera o básico de referências; structs e arrays
não.

O perfil `wasm32-linear` do DartForge:
- usa `-mcpu=generic` com `-msimd128 -mtail-call -mextended-const` e exceções `exnref`
  (`crates/emit_native/src/target.rs`, `crates/emit_native/src/linker_wasm.rs`);
- é conferido por `crates/wasm_gate/src/lib.rs`: `WASM2 | TAIL_CALL | EXTENDED_CONST | EXCEPTIONS`.

**Achado.** Com o padrão atual, o módulo exige um motor tão novo quanto o que o dart2wasm exige, ou
mais novo:
- tail-call chegou ao Safari na mesma versão do WasmGC (18.2);
- `exnref` chegou aos navegadores depois do WasmGC.

Assim, "roda onde o WasmGC não existe" **não vale hoje**. As versões exatas de cada navegador
precisam ser conferidas antes de irem para a documentação.

### 1. Perfil de compatibilidade (flag própria)

- **Flag:** um perfil novo no `compile-wasm` (nome a decidir, por exemplo
  `--wasm-profile compat`), ao lado do `wasm32-linear`, que continua sendo o padrão.
- **O que o perfil tira:**
  - `exnref`: usa `--exceptions check`, que já existe e funciona (`hello` 1 335 709 bytes, +3%);
  - tail-call: as chamadas de cauda que o emissor e o runtime exigem viram chamada comum ou
    trampolim, sem estourar a pilha em recursão profunda nem em `async`;
  - extended-const;
  - SIMD, se a meta for o Safari de 2021: o simd128 só chegou ao Safari 16.4.
- **O que fica:** MVP, mutable-globals, sign-ext, nontrapping-fptoint, multivalue e bulk-memory.
- **Verificação:** um perfil próprio no `wasm_gate` que recuse o resto, e o
  `dartforge-wasm-run` aceitando os dois perfis.
- **Aceitação:**
  - harness wasm 164/164 nesse perfil;
  - rodar em motores sem esses recursos: um Node antigo, wasmtime e wasm3;
  - custo de tamanho e velocidade medido contra o perfil padrão.

### 2. Isolates em threads com memória compartilhada (flag experimental)

**Referência: a VM do Dart.**
- Cada isolate tem o próprio *heap* e roda na própria thread.
- Os isolates trocam mensagens por `SendPort`, com cópia, ou por `TransferableTypedData`, sem
  cópia.
- Memória nativa pode ser compartilhada entre isolates por ponteiro (`dart:ffi`: `malloc` num
  isolate, o endereço passado para outro).

**Proposta para o wasm:**
- **Flag experimental:** nome a decidir, por exemplo `--enable-experiment=wasm-threads`. O padrão
  continua de uma thread só.
- **Compilação:** `-matomics -mbulk-memory`, memória `shared` no `wasm-ld` (`--shared-memory`,
  `--import-memory`) e um perfil do `wasm_gate` com `THREADS`.
- **Um isolate por thread:**
  - Web Worker no navegador;
  - no `dartforge-wasm-run`, uma thread do host (ou `wasi-threads`);
  - todos instanciam o mesmo módulo sobre a mesma memória.
- **Heap por isolate:**
  - uma região própria da memória linear compartilhada;
  - coletor por isolate, sem parar os outros, como na VM;
  - pilha-sombra e TLS por thread (`__tls_base`, `__stack_pointer`).
- **Runtime Rust:** alocador seguro entre threads e estado global por isolate, não por módulo.
- **`Isolate.spawn`/`run`, `SendPort`/`ReceivePort` e `TransferableTypedData`:**
  - mesma semântica da VM;
  - a mensagem é copiada para o *heap* do isolate destino;
  - a espera usa `memory.atomic.wait32`/`notify`, e o laço de eventos de cada isolate acorda por
    ela.
- **Memória nativa compartilhada:**
  - o equivalente ao `dart:ffi` da VM, isto é, `Pointer` e `malloc` sobre a região não coletada da
    memória compartilhada;
  - um ponteiro passado a outro isolate enxerga os mesmos bytes;
  - operações atômicas expostas como na VM.
- **Requisito do navegador:** `SharedArrayBuffer` só existe com `crossOriginIsolated`, isto é, com
  os cabeçalhos COOP/COEP. O `dartforge serve` precisa enviá-los quando a flag estiver ligada.
- **Aceitação:**
  - os testes de isolate da suíte `tests/language` e os do harness, iguais à VM, com a flag;
  - um programa que compartilha memória nativa entre isolates dando o mesmo resultado que na VM;
  - nenhum efeito no módulo sem a flag (tamanho e velocidade iguais).

## 03/11/2026: wasm — rodada de tamanho: hello de 1,30 MB para 0,78 MB (0,66 MB com Binaryen)

Rodada sobre o `compile-wasm --optimize` (macOS, binário de `b7fda46e` como "antes", compilado de
novo numa árvore exportada em `/Volumes/ssd_dock/tmp/wasm-size/base-src`). Bytes do `.wasm` e, entre
parênteses, com `gzip -9`. Os programas são os de `bench/performance` (os da tabela de 31/10) e o
`hello` do `crates/cli/tests/size_budget.rs`. Scripts e módulos em `/Volumes/ssd_dock/tmp/wasm-size/`
(`sizes.sh`, `speed.py`, `dbg.sh`, `why.py`, `livedyn.py`, `wdata.py`, `findaddr.py`, `map.sh`).

| programa | antes (`b7fda46e`) | depois, sem Binaryen | depois, com `wasm-opt -Oz` | dart2wasm |
|---|---:|---:|---:|---:|
| hello | 1 297 303 (390 179) | **782 615** (258 256) | **659 347** (256 841) | 47 013 (15 369) |
| árvores (`escaping_objects`) | 1 350 503 (401 808) | 835 680 (269 874) | 704 434 (268 680) | 48 482 (15 878) |
| JSON | 1 993 677 (567 341) | 1 821 446 (518 009) | 1 517 496 (509 897) | 113 505 (38 499) |
| strings | 1 398 187 (411 628) | 908 517 (286 775) | 767 281 (285 723) | 61 453 (19 861) |
| objetos temporários | 1 351 041 (402 098) | 835 859 (269 897) | 704 650 (268 753) | 48 597 (15 917) |
| coleções | 1 647 289 (457 413) | 1 283 416 (364 570) | 1 086 100 (354 755) | 67 496 (21 318) |
| `colecoes` (comparação entre linguagens) | 1 583 861 (444 082) | 1 205 408 (347 999) | 1 023 533 (339 808) | 65 948 (20 643) |

De −9% (JSON) a −40% (hello) sem o Binaryen; −24% a −49% com ele. O gzip cai de −9% a −34%. O
dart2wasm continua 14× menor no hello (era 28×).

**O que cada passo tirou do hello** (bytes sem Binaryen, gzip entre parênteses):

| passo | commit | hello |
|---|---|---:|
| antes | `b7fda46e` | 1 297 303 (390 179) |
| `noSuchMethod` só com invocação dinâmica | `d58b5ba7` | 907 228 (294 312) |
| só as bibliotecas do SDK que o programa carrega | `f4d1d5fe` | 858 692 (279 024) |
| nomes só das classes instanciáveis | `3938ea4f` | 852 131 |
| tabelas zeradas do runtime fora da seção de dados | `acbec011` | **782 615 (258 256)** |
| `wasm-opt -Oz` quando o Binaryen existe | `a9e307dd` | **659 347 (256 841)** |

1. **`noSuchMethod` só para programas que invocam dinamicamente** (`lower/dynamic_invocation.rs`).
   A raiz era dupla: o seletor `c:noSuchMethod` era raiz da poda (toda classe viva guardava o seu,
   e o `Object.noSuchMethod` cria `NoSuchMethodError`), e o `_dartforgeNoSuchMethod`/`_dartforgeCallError`
   eram registrados na partida pelo `dart:core`. Com eles vinham `NoSuchMethodError` (41 KB),
   `_InvocationMirror` (18 KB), o `LinkedHashMap` do `namedArguments` (`dart:_compact_hash`, 154 KB) e
   o `Iterable.toString`. Uma chamada checada estaticamente não erra o membro nem a aridade; só uma
   invocação dinâmica (membro de receptor `dynamic`, chamada de valor sem tipo de função, padrão
   relacional), um encaminhador de `noSuchMethod`, `Function.apply` e os operadores dinâmicos do
   runtime chegam ao `noSuchMethod` — a mesma premissa do TFA do dart2wasm. No wasm cada invocação
   dinâmica chama uma marca vazia (`dartforge_dynamic_invocation`, que o LTO apaga), e o programa
   registra os dois ajudantes sob um `; df.ref-when` da marca e dos outros produtores: sem nenhum vivo,
   a ligação tira a linha de registro (`pruning.rs`, `assemble`) e a poda da HIR lê a mesma guarda. O
   `_dartforgeRebuildCopiedIndex` (o índice dos mapas de uma mensagem copiada) ficou sob os nativos de
   porta e de isolado, e o runtime só o procura para um objeto copiado. Cada ajudante guardado é
   registrado antes com endereço 0: um caminho que a análise errasse para com "internal error: the
   program reached …" (saída 70) em vez de outro erro que o da VM. `(f as Function)(…)` cujos argumentos
   cabem no tipo de função de `f` não marca (a igualdade customizada do `dart:_compact_hash`). Nos
   programas com `dynamic` de verdade (o JSON: `jsonDecode`, o `toJson()` do codificador) o
   `noSuchMethod` fica, como deve.
2. **Só as bibliotecas que o programa carrega.** O módulo registrava e ligava todas as bibliotecas do
   SDK do wasm: `dart:io`, `dart:_http` e os tipos JS de um hello guardavam nomes, tabelas e o
   registro, e o objeto do `dart:_js_helper` mantinha as exportações JS pelo seu `@llvm.used`. Agora o
   programa só chama o registro das bibliotecas que ele carrega (`sdk_module::sdk_records_of`) e a
   ligação deixa os outros objetos de fora (`driver::loaded_sdk_objects`). O nativo fica igual.
3. **Nomes de classe só das instanciáveis**: o runtime só lê o nome de uma classe para uma instância
   dela; classe abstrata e mixin (sem tabela de métodos) não registram nome no wasm.
4. **Dados zerados.** Pelo mapa da ligação (`map.sh`): o objeto vazio (32 KiB de zeros, memória
   somente leitura no nativo) e a tabela de áreas vazia iam para as constantes, porque o LTO do módulo
   vê que ninguém as escreve; no wasm viraram alocações zeradas feitas uma vez. As memórias de
   subtipo e de `is` começavam com -1 nos espaços vazios; com zero (nenhuma consulta tem alvo 0 nem
   chave 0) ficam em `.bss`, em todos os alvos. E o CRC que identificava a tabela RTI descomprimida,
   com a tabela de 16 KiB do `crc32fast`, saiu do wasm (sem recarga, endereço e tamanho bastam). A
   seção de dados do hello foi de 195 KB para 100 KB.
5. **Binaryen opcional** (`linker_wasm::optimize`): quando acha um `wasm-opt` ≥ 118
   (`DARTFORGE_WASM_OPT`, `0` desliga; senão o do `PATH`; senão o que o SDK do `dart` do `PATH` traz para
   o dart2wasm), o `--optimize` roda `wasm-opt -Oz` só com os recursos do perfil, **antes** de escrever
   os quadros do motor: o orçamento da pilha (`wasm_frames.rs`) é medido no módulo que o Binaryen deixa.
   O Binaryen escreve `x - C` como `x + -C`, então a marca negada também é reconhecida; um módulo em que
   ele dobrasse duas marcas numa constante só (`x - 2C`) é descartado e fica o do LLVM. Sem Binaryen
   nada muda. Ganha −16 a −19% cru e só −0,5 a −2,7% com gzip.

**Medido e descartado:** `opt-level=s` no runtime wasm32 deixa o hello **maior** (907 → 931 KB sem
Binaryen; gzip −2%), e não entrou. Ligar o runtime em bitcode no LTO do módulo já era o caso desde 31/10
(`-Clinker-plugin-lto`).

**Velocidade** (Node 24, CPU da thread principal, mediana das iterações quentes e mínimo entre
parênteses, em ms; três rodadas alternadas dos três módulos, e cinco rodadas para coleções e JSON;
carga da máquina 25–55): tudo dentro do ruído. Exemplos da rodada de cinco: `map` 58,3 (26,6) antes,
52,4 (36,1) depois sem Binaryen, 47,7 (25,4) com Binaryen; `sort` 109,5 / 107,2 / 105,1; JSON decode
grande 162,0 / 161,6 / 165,4; `utf8` 102,0 / 103,1 / 105,6. Da rodada de três: árvores 39,3 (27,5) /
31,6 (26,9) / 35,4 (25,1); strings build 60,1 / 62,9 / 59,0; objetos temporários 10,8 / 11,1 / 10,6.
O tamanho não foi comprado com velocidade, e o `-Oz` do Binaryen não a mudou acima do ruído.

**O que sobra no hello de 782 KB** (nomes da compilação com `DARTFORGE_KEEP_SYMBOLS=1`): código Dart
~225 KB (`dart:core` 125 KB: `_Double` 18 KB, `_GrowableList` 15 KB, `_IntegerImplementation` 13 KB e
os erros que o runtime lança por ajudante, `FormatException`/`RangeError`/`ArgumentError`), runtime
Rust ~470 KB (`abi` 101 KB, o coletor 82 KB, `std`/`hashbrown`/`core::fmt` ~90 KB, o `miniz_oxide`
17 KB da tabela RTI comprimida) e 100 KB de dados (a tabela RTI de 645 classes, 72 KB de texto e
18 KB comprimida, e os nomes). O TinyGo (158 KB) não foi alcançado. Próximos passos, em ordem de
ganho: (1) o runtime Rust — o que o código Dart vivo cita já é podado pelo LTO, então o que resta é
o coletor, a RTI e as estruturas `HashMap` do próprio runtime; (2) a tabela RTI só com as classes
vivas (abaixo de 16 KB deixa de ser comprimida e o `miniz_oxide` sai); (3) a tabela do `_Double`, os
ajudantes de erro e as exportações JS sob guardas como as do item 1.

**Validação** (macOS): diferencial `--wasm` do `corpus/native` (164 programas, `--exec-limit 60`)
**164/164** com Binaryen e sem (`DARTFORGE_WASM_OPT=0`) e **164/164** com `--gc-stress`, nos binários de
cada passo (o final, `30b6a5d2`: com Binaryen, sem ele e com `--gc-stress`); nativo `--optimize`, `--optimize --gc-stress` e desenvolvimento **164/164** cada, com o binário final (o runtime mudou nos três alvos). Suíte
`tests/language` no wasm, 639 programas de risco para estas mudanças (os 555 de `wasm-ae98-risk` e todo
nome com `dynamic`, `such_method`, `nsm`, `apply`, `invocation`, `relational` ou `callable`;
`wasm-size-risk`): **627 iguais à VM, 4 iguais só ao dart2wasm, 1 recusado pelos dois, 7 falhas** — 3
de `wildcard_variables` iguais à referência com experimentos (`vm-r6.json`), e as 4 de sempre (pilhas
simbólicas e o *hash* de `savannah`), iguais às de `wasm-final`; nenhuma regressão. Testes:
`cargo test --release` de `dartforge-runtime` (460 testes) e `dartforge-emit-native` (1 200, entre eles os novos de `pruning` e `wasm_frames`) passam, e o `size_budget.rs` (`--features native --include-ignored`) também. `clippy -D warnings` de `emit_native`, `runtime` e `cli` (com `native`) limpo; rustfmt
limpo nos arquivos tocados. O `size_budget.rs` segura o hello em 860 KB sem Binaryen (`30a6ae04`) (o teste desliga o
passo para o número não depender da máquina).

## 02/11/2026: tamanho do wasm contra outras linguagens; regressão de 10× achada (`863d4159`)

**Regressão.** O `hello` do `compile-wasm` foi de **1 288 823 bytes para 12 753 111 bytes**. `git bisect`
entre `7f35a02f` (31/10, bom) e `f06412db` (ruim), com orçamento de 3 MB, apontou como primeiro commit
ruim o **`863d4159`** ("Close a third round of tests/language gaps of the native backend", 01/11 11:38).

- **O que o pai `47f3ce7a` dá:** 1 288 823 bytes.
- **O que o `main` atual mostra:**
  - na poda, 20 559 de 51 070 símbolos vivos, ~11 mil funções contra ~2,8 mil;
  - 24 s de ligação;
  - o módulo importa a interop JS (`dartforge-wasm-run` o recusa).
- **Por que ninguém viu antes:** o harness wasm continuou 164/164, porque só a saída é comparada, não o tamanho.
- **Corrigida na sexta rodada do backend nativo** (`48d7cdd4`, `0a4532ad`): o SDK não registra mais
  *tear-offs* na partida. O `hello` voltou a 1 297 303 bytes no Mac (1 296 215 no Windows) e o
  executável nativo, que tinha regredido junto sem ninguém ver, de 12,93 para 3,50 MB. O
  `crates/cli/tests/size_budget.rs` segura o `hello` em 1,4 MB no wasm, sem importar `dartforge_js`,
  e em 5 MB no nativo.
- **Para repetir a medição:** os arquivos estão em `/Volumes/ssd_dock/tmp/wasm-cmp/` (`bisect.sh`,
  `bisect.sizes`, `src/`).

**Comparação com outras linguagens que compilam para wasm.** Os mesmos dois programas em cada
linguagem, com a configuração de menor tamanho de cada toolchain:
- `hello`: imprime `Hello, World!`;
- `colecoes`: gera 1000 números por LCG de 64 bits, ordena, conta palavras num mapa e junta 100 textos.

A saída foi conferida em todos: Node 24 (WASI ou o carregador gerado) e Chrome *headless* no Blazor.

A tabela mostra, em bytes, o `.wasm` principal, o mesmo com `gzip -9` e o resultado de
`wasm-opt -Oz` (binaryen 123). Na coluna "coleta":
- WasmGC: a coleta é a do motor;
- própria: coletor em memória linear;
- ARC: contagem de referências.

| linguagem (toolchain, configuração) | coleta | `hello` | gzip | `wasm-opt` | `colecoes` | gzip |
|---|---|---:|---:|---:|---:|---:|
| AssemblyScript 0.28 (`-Oz`) | própria (pequena) | 207 | 204 | 207 | 11 622 | 5 352 |
| Zig 0.14 (`ReleaseSmall`) | sem GC | 2 119 | 1 246 | 1 897 | 25 456 | 11 697 |
| C, wasi-libc (`zig cc -Oz`) | sem GC | 2 760 | 1 357 | 2 190 | — | — |
| Kotlin/Wasm 2.4.21 (wasmWasi, produção) | WasmGC | 9 542 | 4 727 | 9 545 | 34 733 | 15 620 |
| Swift 6.4.2 Embedded (`-Osize`, strip) | ARC | 13 230 | 5 029 | 10 306 | 129 483 | 60 723 |
| Rust 1.98 (`opt-level=z`, LTO, `panic=abort`) | sem GC | 37 429 | 15 320 | 32 773 | 65 885 | 28 002 |
| **dart2wasm 3.6.2** | WasmGC | **47 013** | **15 369** | 46 631 | **65 948** | **20 643** |
| TinyGo 0.39 (`-opt=z`) | própria | 158 291 | 68 014 | 152 169 | 195 664 | 80 961 |
| **DartForge 03/11, 2ª rodada** (`cba2a5e1`; a coluna `wasm-opt` é o `--optimize` com Binaryen) | própria | **718 123** | **225 915** | 598 885 | 1 141 549 | 316 369 |
| **DartForge 03/11** (`30b6a5d2`; a coluna `wasm-opt` é o `--optimize` com Binaryen) | própria | **782 615** | **258 256** | 659 347 | 1 205 408 | 347 999 |
| C# .NET 10, console mínimo (`dotnet.native.wasm`) | própria (Mono) | 1 239 948 | 492 837 | 1 162 589 | igual | igual |
| C# Blazor mínimo (`dotnet.native.wasm`) | própria (Mono) | 1 251 870 | 497 078 | 1 172 812 | igual | igual |
| **DartForge em 31/10** (`7f35a02f`) | própria | **1 258 409** | **378 291** | — | — | — |
| DartForge em 02/11, regressão corrigida (`b7fda46e`) | própria | 1 297 303 | 390 179 | 1 045 436 | 1 583 861 | 444 082 |
| Go 1.24 oficial (`wasip1`, `-s -w`) | própria | 2 377 693 | 685 461 | 2 241 038 | 2 454 896 | 706 693 |
| C# Blazor padrão | própria (Mono) | 2 898 925 | 1 126 025 | 2 735 619 | igual | igual |
| Swift 6.4.2 completo (`-Osize`, strip) | ARC | 5 551 057 | 1 610 679 | 3 365 908 | 5 559 975 | 1 615 439 |
| C# Blazor mínimo + AOT | própria (Mono) | 10 983 693 | 3 510 455 | 8 960 475 | 11 014 955 | 3 524 367 |
| **DartForge hoje** (`f06412db`, regressão) | própria | **12 757 429** | **2 405 842** | 8 332 885 | 12 768 381 | 2 409 530 |

O .NET baixa mais que o `.wasm` principal: os *assemblies* Webcil e o JS. No `hello`, o total
baixado é:
- console mínimo: 2 686 706 bytes (990 154 gzip);
- Blazor mínimo: 4 529 730 (1 738 438);
- Blazor padrão: 7 411 323 (2 736 086);
- Blazor + AOT: 14 271 044 (4 516 711).

**Leitura**
- **Quem não tem coletor fica em bytes ou poucos KB:** C, Zig e Rust.
- **Quem usa o WasmGC do motor (dart2wasm, Kotlin) fica em dezenas de KB.** O DartForge não usa
  WasmGC, por decisão do projeto: um módulo com o coletor dentro, para rodar em qualquer motor (isso
  só vale depois do perfil de compatibilidade; ver a seção de 03/11).
  Por isso a régua justa é o grupo que leva o próprio runtime em memória linear:
  - **AssemblyScript 11,6 KB no `colecoes`, com coletor** (ver o estudo abaixo);
  - TinyGo 158 KB;
  - .NET mínimo 1,24 MB;
  - Go 2,4 MB;
  - Swift completo 5,6 MB.
- **No nosso melhor estado (1,26 MB)** ficamos empatados com o .NET mínimo, abaixo do Go e do
  Swift completo e 8× o TinyGo. Hoje, com a regressão, estamos acima de todos menos o Blazor+AOT.
- **Meta depois da correção:** voltar a 1,26 MB e depois ir na direção do TinyGo. O que sobra no
  `hello` de 1,26 MB está descrito na seção de 31/10:
  - ~560 KB do SDK (`NoSuchMethodError`, `_InvocationMirror`, `LinkedHashMap`);
  - ~400 KB do runtime Rust;
  - 194 KB de dados.
  Mais o `wasm-opt -Oz`, que tiraria ~19% se o orçamento da pilha do motor for refeito depois dele.

### Estudo do AssemblyScript: coletor em memória linear em poucos KB

O AssemblyScript é o caso mais próximo do nosso:
- não usa WasmGC por padrão;
- o runtime (alocador TLSF e coletor) é escrito em AssemblyScript sobre a memória linear;
- esse runtime é compilado junto com o programa, num mundo fechado, e otimizado pelo Binaryen.

**Custo do coletor dele**, medido com `asc -O3z --converge --noAssert`:

| runtime do AssemblyScript | `hello` | `colecoes` |
|---|---:|---:|
| `stub` (só alocação, nunca libera) | 130 | 6 185 |
| `minimal` (TLSF + marcação e varredura simples) | 130 | 7 728 |
| `incremental` (padrão: TLSF + coletor incremental tricolor) | 212 | 11 628 |

O coletor completo custa ~5,4 KB, e o `hello` não paga nada, porque o Binaryen remove o runtime
que o programa não usa.

**De onde vem o nosso `hello`.** Medi com o binário da correção da regressão (WIP do agente de
corretude, 02/11 21h): **1 297 303 bytes**. Na compilação com `--debug`, a seção de nomes deu esta
divisão:
- **Código Dart do SDK, ~575 KB em 1 262 funções:**
  - `dart:core` 255 KB, dos quais `NoSuchMethodError` 41 KB, `_InvocationMirror` 18 KB,
    `Iterable.iterableToShortString` 13 KB e `_Double` 19 KB;
  - `dart:_compact_hash` 154 KB (`LinkedHashMap`/`Set`), num programa que não usa mapa;
  - `register` 46 KB, `dart:_internal` 42 KB, `dart:typed_data` 28 KB, `dart:collection` 28 KB.
- **Runtime Rust, ~400 KB:**
  - `dartforge_runtime::abi` 108 KB;
  - `dartforge_runtime::heap` 86 KB: o coletor, contra 5,4 KB do AssemblyScript;
  - `std`, `hashbrown`, `core::fmt` e `alloc` ~100 KB;
  - `miniz_oxide::inflate` 17 KB, um descompressor dentro do `hello`;
  - `dlmalloc` 7 KB.
- **Dados:** 195 KB.

**`wasm-opt -Oz` (Binaryen 123) no nosso módulo:**
- precisa de `--enable-tail-call --enable-exception-handling --enable-reference-types --enable-simd`
  e dos recursos MVP;
- o resultado roda no `dartforge-wasm-run`;
- `hello`: 1 297 303 → 1 045 436 bytes (−19%), mas com gzip só 390 179 → 378 302 (−3%).
O ganho do otimizador é pequeno depois do gzip. **O que pesa é o que entra no módulo**, não a
qualidade do código gerado.

**O que tirar disso:**
1. **Mundo fechado de verdade no SDK.** `print('…')` não pode puxar `LinkedHashMap`,
   `NoSuchMethodError` e a formatação de `double`. Isso é ~575 KB contra 47 KB do dart2wasm no
   programa inteiro. É o maior ganho; precisa achar as raízes que puxam cada um.
2. **Runtime compilado junto com o programa.** O AssemblyScript remove o runtime que não usa
   porque o runtime faz parte da mesma otimização. O nosso runtime Rust entra inteiro. Caminhos:
   - LTO do runtime em bitcode junto com o módulo;
   - runtime wasm sem `std`, `hashbrown` e `core::fmt`;
   - descompressão só quando há dado comprimido.
3. **Coletor menor:** 86 KB contra 5,4 KB, com o mesmo modelo de memória linear.
4. **`wasm-opt -Oz` opcional no `--optimize`** quando o Binaryen estiver instalado, com o orçamento
   da pilha refeito depois dele: −19% cru, −3% com gzip.

Scripts da divisão por seção e por função: `/Volumes/ssd_dock/tmp/wasm-cmp/study/`.

As toolchains estão em `/Volumes/ssd_dock/tools/wasm-langs/` (7,8 GB; `env.sh` lá) e
`/Volumes/ssd_dock/tmp/wasm-cmp/tools/`. Fontes e scripts estão em `/Volumes/ssd_dock/tmp/wasm-cmp/`
(`src/`, `out/`, `measure-langs/`, `wasi.mjs`).

## 02/11/2026: `dartforge pub publish`

Porte do `command/lish.dart` do Pub 3.6.2 (`crates/cli/src/pub_get/publish*`): lista de
arquivos do `Package.listFiles` com um porte do `ignore.dart` (`.pubignore` vence
`.gitignore` por diretório, regras de base na raiz, caminhada a partir da raiz do
repositório Git), árvore com tamanhos, arquivo `.tar.gz` do `createTarGz` (tar do
package:tar e gzip por um porte do zlib do Chromium que a VM usa: o `gzip.encode` da VM
não é o zlib clássico), os 25 validadores na ordem do Pub com as mensagens do
package:source_span, `--dry-run`, `--force`, `--skip-validation` e as ocultas `--server`,
`--to-archive` e `--from-archive`, e o envio (`versions/new`, `POST` multipart,
finalização) com token, repetições e os relatórios de erro do Pub. O `dartforge analyze`
passou a analisar um `pubspec.yaml` dado como argumento como pubspec e a achar o
`package_config.json` da raiz do workspace; as leituras do `pub get` levam o token do
servidor e o *user agent* do Pub, e o erro de conexão sai como no Pub (só a primeira
linha). Fica de fora: envio ao pub.dev sem token (fluxo OAuth2 do `login`).

Evidências em `bench/results/2026-11-02-pub-publish/README.md`; matriz em
`docs/PUB-E-TEST.md`.

## 02/11/2026: `dartforge test` com o runner do package:test; `pub cache` e `pub global`

**`dartforge test`.** O planejador mais os *workers* com eventos próprios deram lugar a um
anfitrião que é o `dart test` com uma plataforma trocada (`test_runner/host.dart`): o
`Configuration`, o `Runner` e o `Loader` portados, o `Engine` e os relatores do próprio
package:test. Cada suíte é compilada pelo DartForge em volta do
`internalBootstrapNativeTest` do test_core e fala com o anfitrião pelo protocolo de
soquete do `dart test -c exe`. Com isso `-r json` passou a emitir o protocolo do
package:test (ids, `groupIDs`, `hidden`, `skipReason`, `testCount`, `allSuites`) e
`compact`, `github`, `failures-only`, `silent`, `--file-reporter` e `--fail-fast`
passaram a existir, todos iguais ao `dart test` fora tempo e quadros de pilha. O
executável nativo ainda não tem quadros de pilha; o anfitrião detecta isso e as
suítes deixam de coletar rastros de declaração, então `line`/`column`/`url` saem nulos
em vez de quebrar o relator. `--coverage` foi analisado antes de qualquer código e
continua recusado: as linhas e contagens do JSON vêm dos descritores de PC do compilador
da VM para todas as bibliotecas do isolado (223 scripts numa suíte de 2 testes), o que
não se reproduz fora da VM (`docs/PUB-E-TEST.md`).

**`pub cache` e `pub global`.** Portes do Pub 3.6.2 (`cache.rs`, `global.rs`), com os
mesmos textos de ajuda e de erro e o mesmo layout do `PUB_CACHE`; `pub get` passou a
gravar o `README.md` do cache e as listagens `.cache/<pacote>-versions.json`. No
`global`, onde o Pub compila *snapshots* o DartForge compila executáveis nativos e os
*bin stubs* chamam esse binário (ou `dartforge pub global run`).

Evidências em `bench/results/2026-11-02-pub-test-2/README.md`.

## 02/11/2026: backend nativo — sexta rodada; a suíte `tests/language` inteira medida, sem projeção

### Medição: as 2 863, com o binário final

`run-language-suite.py` com `--jobs 4`, **no Windows** (`dfwin-isaque`, Dart 3.6.2 oficial, Node 24),
referência `vm-win-r6` (a medição oficial da VM no Windows com os experimentos, mais os 4 programas
dos pacotes e `closure/cycles_test` rodados de novo; os pacotes nas versões que a 3.6.2 fixa) e,
para o JS dev, o DDC 3.6.2 (`ddc-win-r6`) nos programas em que diferimos da VM; binário final da
rodada (`e5ee436c`; o JS dev, `fa1c3639`, com o mesmo emissor JS). **Números reais, não
projeção**: o Mac perdeu energia duas vezes no meio das medições; tudo foi medido de novo no
Windows, e nenhum resultado parcial de antes conta.

| | iguais à VM | só dart2wasm / divergência do DDC | recusados pelos dois | passam onde a VM falha | falhas |
|---|---:|---:|---:|---:|---:|
| JS dev, rodada 5 (`jsdev-r5c`, Mac, regras antigas) | 2 780 | — | 13 | — | 70 |
| JS dev, `f5c31671` (`jsdev-win-r6`, Windows, regras novas) | 2 782 | 32 (DDC) | 16 | 2 | 31 |
| **JS dev final** (`jsdev-win-final`) | **2 794** | **48** (DDC) | **16** | **4** | **1** |
| nativo, rodada 5 (`f17`, projeção, Mac) | 2 810 | — | 13 | — | 40 |
| **nativo final** (`native-win-final`) | **2 832** | — | **16** | **2** | **13** |
| wasm, rodada 5 (`f17-wasm`, projeção, Mac) | 2 810 | 5 | 13 | — | 35 |
| **wasm final** (`wasm-win-final`) | **2 828** | **3** (dart2wasm) | **16** | **2** | **14** |

Os 16 recusados pelos dois (e não 13) vêm da referência do Windows e das três verificações novas
do *front end* (abaixo): programas que a VM recusa e que antes nós rodávamos.

As 13 falhas do nativo, todas fora do que esta rodada podia igualar: **9 de pilha simbólica**
(`stack_trace/{demangle_ctors,full1,full2,full3,rethrow_nonerror,stack_trace}`, `regress21795`,
`private/mixin_exception_throw` e `private3`, que imprimem a pilha; o AOT sai com
`--stack-trace none`), **2 de *hash*** (`savannah`, `non_interface_object_pattern`) e **2 em que
o CFE da VM quebra** (252: `augmentation_libraries/class_augmentation`, que nós recusamos no
*parser*, e `least_upper_bound_greatest_closure_2`, que nós compilamos e rodamos). Das 17 reais da
rodada 5 sobra só o que já estava fora (pilhas e *hashes*); os 22 de `variance` saíram (20 iguais,
2 passam onde a VM falha).


### Regressão de tamanho do `863d4159` (wasm e nativo)

O hello world em wasm foi de 1 288 823 bytes (`47f3ce7a`) para 12 753 111 (bissecção do
coordenador: `863d4159`), com o interop JS importado e um `.mjs` de companhia; a poda guardava
20 559 de 51 070 símbolos. **O nativo regrediu igual** (3,49 → 12,93 MB no Windows), o que ninguém
tinha visto.

**Causa.** O `863d4159` fez o `join_arc_facts` guardar os nomes dos *tear-offs* e as entradas de
instanciação das funções baixadas em módulos temporários (o SDK é baixado em partes): antes elas
se perdiam, e o `toString` de um *tear-off* do SDK saía sem nome. Com isso, o
`df.register.<biblioteca>.tearoffs` de cada biblioteca do SDK passou a citar **todo** *tear-off*
que o SDK cria em qualquer lugar; a partida chama essas funções, então a poda da ligação tomava
cada entrada como viva — e, por elas, quase todo o SDK, `dart:js_interop` incluído.

**Correção** (`llvm/selectors.rs`): uma biblioteca do SDK não registra mais os seus *tear-offs* na
partida; o programa registra na partida os que ele cria, também os de funções do SDK (declaradas
no módulo do programa: criá-las já as mantém vivas), e o SDK avisa o runtime **onde cria** a
*closure* de uma entrada sua (`note_sdk_tearoff` → `dartforge_note_tearoff`, que registra nome e
identidade no primeiro uso), código que só fica vivo quando quem cria fica. Os ganhos do
`863d4159` ficam todos: o nome no `toString`, a igualdade e o `hashCode` também do *tear-off* que
o SDK cria — `(42 as dynamic).toString` (`closure/tearoff_dynamic`, que a primeira versão desta
correção, só com o registro do programa, voltava a falhar).

Bytes, no Windows (`compile-wasm` e `aot --optimize`; "antes" é o binário de `f5c31671`, com a
regressão; "depois" é o binário final da rodada; os programas são os de `bench/performance`, os da
tabela de 31/10, que foi medida no Mac):

| programa | wasm 31/10 (Mac) | wasm antes | wasm depois | nativo antes | nativo depois |
|---|---:|---:|---:|---:|---:|
| hello | 1 258 409 | 12 756 051 | **1 296 215** | 12 931 584 | **3 501 568** |
| árvores (`escaping_objects`) | 1 307 255 | 12 764 492 | **1 349 351** | 12 938 752 | **3 513 856** |
| JSON | 1 942 692 | 12 809 712 | **1 992 349** | 12 985 856 | **3 805 696** |
| strings | 1 353 205 | 12 766 364 | **1 397 099** | 12 941 312 | **3 532 800** |
| objetos temporários | 1 311 415 | 12 765 010 | **1 349 873** | 12 940 288 | **3 515 904** |
| coleções | 1 603 143 | 12 793 211 | **1 646 201** | 12 965 888 | **3 807 744** |

Depois da correção, nenhum módulo importa `dartforge_js` (só os 4 de `dartforge_host_v1`) nem
tem `.mjs`; antes, 57 importações do interop. No Mac o hello sai com 1 297 303 bytes (o
`47f3ce7a` dava 1 288 823). Os ~6 KB a mais do aviso no ponto de criação, e o que entrou desde 31/10,
dão os 2–3% sobre a tabela de 31/10.

**Guardas** (`crates/cli/tests/size_budget.rs`, no `ci.yml`, job `native-unix`): o hello em wasm
até 1,4 MB e sem importação do módulo `dartforge_js` (lida da seção de importações: os nomes
`dartforge_js_*` que o runtime exporta para o adaptador estão em todo módulo), sem `.mjs`; o
executável do hello até 5 MB (o `dart compile exe` dá ~5,8 MB).

### Correções

- **`typedef/aliased_constructor_tear_off`** (nativo e wasm): `Alias<T…>.new`/`.name` com
  `typedef Alias<…> = C<…>` é o *tear-off* instanciado do construtor de `C` (o tipo estático já
  traz `C<int> Function(int)`), o mesmo de `C<int>.new`; antes era lido como o getter `new` de um
  `Type`. E um alias que é um *proper rename* (`typedef Direct<T> = C<T>`: os parâmetros de `C` em
  ordem e com os mesmos limites) dá, sem instanciação, o próprio *tear-off* genérico de `C`,
  idêntico a `C.new`; `Bounded<T extends num> = C<T>` continua diferente, como na VM.
- **Verificações do *front end* que faltavam** (os programas que a VM recusa e nós rodávamos; nos
  quatro compiladores, `codes::REJECTING_CODES`): `undefined_super_{getter,setter,method,operator}`
  (`super.foo = 3` sem setter acima, `deferred/super_dependency`) e `undefined_named_parameter`
  (`value_class/creates_*`: `@valueClass` sem o experimento não cria o construtor). Antes de
  entrarem na lista, o `dartforge analyze` passou pelos 2 863 programas e pelas 377 bibliotecas
  auxiliares da suíte: nenhuma ocorrência fora dos programas que a VM recusa (e da biblioteca de
  `augmentation_libraries`, que a VM também recusa).
- **Ordem da interpolação** (`string/interpolation_and_buffer`, nativo e wasm): como a VM
  (`StringConcatenation` e depois `_interpolate`), todas as expressões interpoladas são avaliadas
  primeiro e só então cada valor vira texto — `"$t${throw x}"` lança `x` antes do `t.toString()`.
  A VM falha nesse teste (255: ela não segue o texto da especificação), e agora nós falhamos igual.
- ***Mixin* aplicado duas vezes: uma posição por aplicação** (desenho em `docs/NATIVO.md` §6). O
  *layout* segue a cadeia completa, da base para a folha; antes a cadeia sem repetição punha o
  *mixin* depois da superclasse e deslocava os campos dela entre `D` e `E` (`D(5,…)` lia lixo num
  `E`). Leituras e escritas vão à aplicação mais externa (os acessores são virtuais, como na VM, em
  que as duas aplicações veem o campo da externa); o construtor de cada aplicação inicializa as
  suas posições. Medido contra a VM com campos, `super`, `dynamic` e três classes.
- **O mesmo caso no JS** (`mixin/super_2`, `mixin_legacy/super_2`): uma classe (não `mixin`) que
  alguma classe aplica como *mixin* e cujos membros usam `super` ganha também a forma
  `[dart.mixinOn]`, com os aliases de interface nativa (`[$toString]`) da cópia; antes o
  `applyMixin` copiava os métodos com o `super` preso à superclasse da própria classe.
- **Experimento `variance`** (`class C<in T, out U, inout V>`; 24 programas de `variance/`), nos
  dois alvos. *Front end*: o experimento é aceito com `--enable-experiment=variance` (o
  `SharedOptions` dos testes); antes era recusado com o nome. JS: o `addRules` leva as variâncias
  pelo `dart_rti._Universe.addTypeParameterVariances` (os códigos do DDC: 0 legado, 1 `out`, 2 `in`,
  3 `inout`); um parâmetro de classe `in`/`inout` não é covariante pela classe (sem verificação e
  com o tipo declarado no *tear-off*, `Ctx::covariant_param_ids`); os *stubs* de covariância e os
  encaminhadores de `noSuchMethod` são escritos nos parâmetros da própria classe. Nativo e wasm: a
  tabela RTI ganhou a linha `V<id> <dígitos>` (`Universe::variances` no runtime), e o subtipo
  compara cada argumento conforme a variância do parâmetro da classe alvo; `in`/`inout` não tornam
  um parâmetro covariante pela classe (`typed_entry::mentions_covariant_by_class`), mas a entrada
  dinâmica continua verificando. Nativo: 22/24 iguais à VM; os outros dois
  (`variance_{in,inout}_subtyping`) **passam** onde a VM 3.6.2 falha um `Expect` (ela não
  implementa a subtipagem do experimento).
- **`meta` e `collection` da suíte**: as versões que a 3.6.2 fixa — `meta` 1.16.0 (o `pkg/meta` da
  tag) e `collection` 1.19.0 (o `collection_rev` 24b75d85 do DEPS da tag) —, do pub cache ou do
  arquivo do pub.dev (`setup --packages-only`), não mais do checkout novo. A VM dá a mesma saída
  que com as versões novas nos quatro programas.
- **Modo `jit`** no script: `dartforge run` (o JIT ORC sobre o mesmo *lowering* nativo), com os
  experimentos (o `run` ganhou `--language-version`/`--enable-experiment`); uma recusa de
  compilação (código 1, sem saída) de um programa que a VM recusa conta como recusado pelos dois.
  Conferido em 5 programas; a suíte inteira no JIT não foi medida.
- **`canonicalize` no resto do código**: `build` (motor e `natives/ng`), `lsp` (sessão, projeto,
  navegação, mover arquivo), `parity`, `dev` e `emit_native` (`sdk_module`, `cross`) usam
  `dartforge_elements::config::canonicalize`; o `runtime`, que não pode depender de `elements`,
  ganhou `plain_canonicalize` (a regra da VM no `file_win.cc`: tira `\\?\` do disco e transforma
  `\\?\UNC\` em `\\`), usada no `resolveSymbolicLinks` e na detecção de ciclo da listagem de
  diretórios no Windows; o `sass`, que não depende de nada do DartForge, tem a mesma regra local.
  O `differential` já tinha a sua; exemplos e testes que só leem o caminho ficaram como estavam.

### JS dev: as falhas contra o DDC

O JS dev substitui o `webdev serve` (DDC), então cada falha foi conferida contra o próprio DDC 3.6.2
(`run-language-suite.py ddc`, o `dartdevc` do SDK com o `dart_sdk.js` dele, no Node), no Windows.
**Regra nova de contagem** (docstring do script): um programa em que o DDC diverge da VM e o JS dev
diverge do mesmo jeito — o código de saída do DDC, e a saída dele ou a primeira linha diferente da
VM no mesmo lugar (o fonte JS depois de `from:` no `toString` de *closure*, os quadros JS e os
*hashes* vêm do código gerado de cada um) — conta como **divergência do DDC**, não como falha: é a
plataforma web. E, em todos os alvos, um teste que **passa onde a VM falha um `Expect`** (255,
`Unhandled exception:` e `Expect…`; nós com saída 0, sem stderr e com a saída da VM até a falha)
conta à parte, com a lista impressa: a VM 3.6.2 não implementa o que o teste confere.

Das 70 falhas da rodada 5 (com as regras novas, 31 no binário de `f5c31671`), no binário final:

| grupo | programas | o que é |
|---|---:|---|
| experimento `variance` | 22 | implementado (acima): 20 iguais à VM, 2 passam onde a VM falha |
| divergências do DDC | 38 | números da web (`int` de 53 bits, `-0.0`, `double` sem `.0`), `toString` de *closure*, pilhas, *hashes*, `invocation_mirror`, `private3`, asserts ligados, `await_user_defined_future_soundness`… — o DDC dá o mesmo |
| `external` sem interop (`external_constructor`, `external_fields`) | 2 | o DDC recusa ("Only JS interop members may be 'external'"); nós também agora (`emit_js/src/module.rs`, `check_external_members`) |
| `assert/message_test` | (1) | girava para sempre: depois de um `assert` o fluxo é a junção do estado de antes com o ramo verdadeiro da condição (o `assert_end` do CFE; a *closure* saía `Never`); agora diverge como o DDC (asserts ligados) e está entre os 38 |
| `assert/assert_test`, `interpolation_and_buffer` | 2 | passam onde a VM falha (a referência roda sem asserts; a ordem de interpolação da especificação) |
| `least_upper_bound_greatest_closure_2` | 1 | o CFE 3.6.2 estoura a pilha na VM (252) e no DDC; nós compilamos e rodamos — fica como falha, não há referência |

E 8 programas que antes saíam iguais à VM agora são recusados como o DDC recusa: 7 com literais
inteiros não representáveis exatamente em JS ("can't be represented exactly in JavaScript"; fora do
contexto `double` e fora do SDK, `emit_js/src/expr.rs`) e `inference_update_2/external_field`.

Suíte inteira no JS dev, binário final, Windows (`jsdev-win-final`): **2 794/2 863 iguais à VM**,
48 divergências do DDC, 16 recusados pelos dois, 4 passam onde a VM falha, **1 falha** (a de cima).
Contra a rodada 5, nenhuma regressão (`compare --forge`).

### Validação

- harness `corpus/native` (164 programas) nos 4 modos, Mac: dev **164/164**, `--optimize`
  **164/164**, `--optimize --gc-stress` **164/164**, `--wasm` **164/164** (39 iguais ao dart2wasm
  onde a web difere);
- subconjuntos da suíte com o binário final no Mac (os tocados pela correção do tamanho e pela
  variância: `tearoff`, `tear_off`, `closure`, `deferred/`, `instantiat`, `variance/`, 182
  programas): nativo e wasm **178 iguais, 1 recusado pelos dois, 2 passam onde a VM falha, 1 falha**
  (`least_upper_bound_greatest_closure_2`); contra 31/10, 20 corrigidos e nenhuma regressão;
- `size_budget` (hello em wasm e nativo) passa no Mac; `cargo test` de `emit_native`, `runtime`,
  `emit_js`, `types` e `frontend` passa no Mac (o `emit_js --test basic` precisa do `node` no PATH);
  `clippy -D warnings` desses e de `cli` passa no Mac;
- no Windows (em andamento): a suíte wasm inteira, `clippy -D warnings` e `cargo test` dos crates tocados.

### Ficou, com o motivo

- **pilhas simbólicas e *hashes***, como combinado;
- **`least_upper_bound_greatest_closure_2`**: o CFE 3.6.2 estoura a pilha (252) ao compilar; não
  há o que igualar;
- **`variance`** (22) e **`macros`**: experimentos não implementados;
- **no JS**: o `toString` de *closure* no formato da VM, `external` sem implementação, a ordem de
  interpolação da VM, a correção do `async` da rodada 4 e os avulsos da tabela acima;
- o `super.x` de um campo da aplicação interna de um *mixin* aplicado duas vezes, lido de dentro da
  externa (cai na posição externa);
- a suíte inteira no modo `jit`.

## 02/11/2026 (madrugada): backend nativo — quinta rodada; medição da suíte com experimentos, pacotes e "recusados pelos dois"

### Medição da suíte `tests/language` (scripts/run-language-suite.py)

Correções na medição, pedidas pelo proprietário, antes dos itens da rodada:

- **"Recusado pelos dois" também no JS** (`forge`): um programa que a VM recusa (254) e o DartForge
  também (-1000) conta à parte, como já faziam `native` e `wasm`. O resumo de toda ferramenta
  imprime separados: iguais à VM, só iguais ao dart2wasm (`wasm`), recusados pelos dois e falhas.
  `compare --forge|--native|--wasm|--dartaot` lista regressões, correções (quantas viraram iguais e
  quantas viraram recusadas pelos dois) e os movimentos entre igual e recusado, nos dois sentidos.
- **Experimentos**: as linhas `// SharedOptions=` e `// VMOptions=` de cada teste pedem
  `--enable-experiment=a,b`; a VM, o DDC, o dart2wasm, o `dart compile exe` e o DartForge recebem os
  experimentos. O DartForge recebe também `--language-version=3.6`: um experimento só vale numa
  biblioteca cuja versão padrão é a atual da ferramenta, e o pacote da suíte está em 3.6, a versão
  atual da VM 3.6.2 (a nossa é 3.13). Um experimento que o DartForge não conhece é recusado na linha
  de comando com o nome dele (`--enable-experiment: unknown experiment 'variance'`) e aparece como
  falha de compilação — uma lacuna visível, não um "recusado pelos dois". Um experimento já lançado
  até a 3.6 (`digit-separators`, `records`, `patterns`…, `Feature::RELEASED_AT_FLOOR`) é aceito e
  não muda nada, como nas ferramentas 3.6.2. Conferido contra a medição do Dart oficial no Windows
  (`vm-win-flags.json`, mesmo `list.txt`): códigos de saída idênticos nos 2863 programas, exceto os 4
  dos pacotes abaixo (o Windows ainda não os tinha) e `closure/cycles_test` (tempo esgotado lá); a
  saída difere só em 2 (`hashCode` e caminhos). O modo novo `dartaot` (`dart compile exe` com os
  experimentos, depois o executável) dá os mesmos códigos que `aot-win-flags.json` nos 62 programas
  com experimento ou pacote, de novo exceto os 4 dos pacotes.
- **Pacotes `meta` e `collection`**: o `package_config.json` da suíte não os tinha, e quatro
  programas eram recusados por todas as ferramentas, a VM inclusive (`regress/regress14348`, por
  `collection`; os três `type_promotion/closure_runtime*`, por `meta`). O `setup` agora extrai
  `pkg/meta` da tag (`git archive`) e copia `collection` de `third_party/pkg/collection` (o lugar da
  dependência do DEPS na 3.6.2 depois do `gclient sync`) ou de `third_party/pkg/core/pkgs/collection`;
  `--meta DIR`/`--collection DIR` copiam de um diretório e `--packages-only` só acrescenta os dois
  pacotes, sem extrair a suíte de novo. A versão de linguagem de cada pacote é o limite inferior do
  `sdk:` do pubspec, como o pub escreve. Nesta máquina vieram do checkout em
  `/Volumes/ssd_dock/dart-sdk-src/sdk` (`pkg/meta` 1.18.3 e `core/pkgs/collection` 1.20.0-wip, mais
  novos que os da 3.6.2: os quatro testes só usam `meta.virtual`, que nem existe mais, num `show`, e
  `collection/equality.dart`). Os quatro passam a rodar na VM e são iguais à VM no JS, no nativo e no
  wasm.
- **Referências novas, sem sobrescrever as da v19**: `vm-r5.json` (a `vm.json` da v19 com os 62
  programas de experimento e de pacote rodados de novo) e `dart2wasm-r5.json`; `--ref`/`--alt`
  escolhem a referência (padrão `vm.json`/`dart2wasm.json`, que continuam as da v19). Resultados
  novos: `jsdev-r5c`, `f17` (nativo), `f17-wasm`.

Regras de contagem, para toda ferramenta que não é a VM (estão também na docstring do script):
**igual** = mesmo código de saída e mesma saída padrão que a referência da VM; **só igual ao
dart2wasm** (só no `wasm`) = igual ao dart2wasm onde a plataforma web o faz diferir da VM;
**recusado pelos dois** = a VM recusa compilar (254) e a ferramenta também (-1000), um programa com
erro estático que o filtro não tirou — não é falha; **falha** = todo o resto, inclusive um programa
que a VM recusa e nós aceitamos, e um experimento que não conhecemos.

**Totais novos** (referência `vm-r5`; nativo e wasm em projeção: a medição completa de 31/10 com
as rodadas por cima, a última em 860 programas — as falhas, todas as áreas tocadas e
`exception/`, `stack_trace/`, `async*/`, `sync_star/`, coleções):

| | iguais à VM | só dart2wasm | recusados pelos dois | falhas reais |
|---|---:|---:|---:|---:|
| JS dev (`forge`), v19 com as regras antigas de contagem | 2 756 | — | (dentro das falhas) | 107 |
| JS dev, v19 com as regras novas e a referência antiga | 2 756 | — | 47 | 60 |
| **JS dev agora** (`jsdev-r5c`) | **2 780** | — | **13** | **70** |
| nativo, fim da quarta rodada (referência antiga) | 2 778 | — | 54 | 31 |
| **nativo agora** (`f17`) | **2 810** | — | **13** | **40** |
| wasm, fim da quarta rodada (referência antiga) | 2 778 | 4 | 54 | 27 |
| **wasm agora** (`f17-wasm`) | **2 810** | **5** | **13** | **35** |

O delta, por causa:

- **os 58 programas com experimento** (`variance` 22, `wildcard-variables` 20, `macros` 10,
  `inference-using-bounds` 3, `null-aware-elements` 2, `digit-separators` 1): com a referência
  antiga a VM recusava 46 deles (254) por falta do experimento, e nós também — eram "recusados
  pelos dois" que não testavam nada; 6 eram falhas (a VM recusava e nós aceitávamos, os
  *wildcards*). Agora, no nativo: **26 iguais** (os 20 de *wildcard*, os 3 de
  *inference-using-bounds*, os 2 de *null-aware-elements*, `digit-separators`), **9 recusados
  pelos dois** (os 8 de `macros` e `test_runner/imported_augmentation_library_failure`: a própria
  VM recusa) e **23 falhas**: os 22 de `variance` (experimento que não implementamos, recusado com o
  nome dele) e `augmentation_libraries/class_augmentation` (o CFE 3.6.2 quebra com 252). No JS, os
  mesmos 26/9/23;
- **os 4 programas dos pacotes** `meta`/`collection`: de recusados pelos dois para **iguais**, no JS,
  no nativo e no wasm;
- **correções desta rodada** no nativo: 8 programas de falha para igual
  (`closure/identity_equality_tearoff`, `extension_methods/static_extension_inference`,
  `extension_type/regress_53607`, `inference_update_2` ×2, `mixin/super_2`,
  `mixin_legacy/super_2`, `void/void_check`), além dos de experimento que dependiam delas
  (*wildcards*, *null-aware*, *rethrow*); nenhuma regressão nos 860. No wasm, os mesmos 8, também sem regressão;
  dos 58 de experimento, 27 iguais (`augmentation_libraries/class_augmentation` conta como igual ao dart2wasm, que
  também o recusa), 9 recusados
  pelos dois e 22 falhas.
- **o que sobra de falha real fora dos experimentos**: nativo **17** (eram 31): 9 de pilha
  simbólica (o AOT sai com `--stack-trace none`: os 6 `stack_trace/*`, `regress21795`,
  `private/mixin_exception_throw` e `private/private3`, que imprimem a pilha), 2 de *hash*
  (`savannah`, `non_interface_object_pattern`), 5 em que a VM recusa ou falha e nós rodamos
  (`deferred/super_dependency` e `value_class` ×2 com 254, `least_upper_bound_greatest_closure_2`
  com 252, `string/interpolation_and_buffer` com 255) e 1 real, `typedef/aliased_constructor_tear_off`.
  Wasm **13**: os mesmos, menos 4 de `stack_trace/*` que no wasm já saem como a VM. JS **47**, as mesmas de antes (divergências da web e os dois `super_2` do *mixin* aplicado duas
  vezes).

### Windows: caminhos `\\?\` no `pub` e no `test` (os 4 testes que falhavam em `b5145d31`)

`cargo test --release --workspace` no Windows 11 x64 dava 3126 passados e 4 falhas. Causa comum de
três delas: `std::fs::canonicalize` devolve no Windows caminhos *verbatim* (`\\?\E:\…`), e com eles o
`pubspec.yaml` de um membro de *workspace* não era achado, o caminho relativo mostrado ao usuário saía
`../../../../../../\\?\E:/\/Temp/…` e o `git clone --mirror` recusava o destino. Correção na raiz:
uma função só, `dartforge_elements::config::canonicalize`, que tira o prefixo quando a forma comum
nomeia o mesmo arquivo (a regra do `dunce`: `\\?\C:\x` vira `C:\x`, `\\?\UNC\srv\share` vira
`\\srv\share`; fica *verbatim* acima de `MAX_PATH`, com nome reservado como `NUL`/`COM1`, componente
terminado em ponto ou espaço, ou com `/`). Todos os `canonicalize` do `crates/cli` (`pub`, `test`,
`pack`, `jit`) e do `crates/elements` (`config`, `load`, `frozen`, `distribution`,
`sdk_discovery`) passam por ela; o `without_verbatim` antigo, que tirava o prefixo de qualquer
caminho, agora segue a mesma regra. O `relative_path` da saída do `pub`, com raízes diferentes (dois
discos), devolve o caminho inteiro, como o `p.relative`.

- `workspaces_resolve_together_and_respect_pub_rules`: além do prefixo, o teste esperava `pkgs/*`;
  o `dart pub get` 3.6.2 no Windows (medido na máquina) diz `in "E:\Temp\dfglob\pkgs\*"`, com o
  separador da plataforma, e o teste agora espera o caminho normalizado da plataforma;
- `relative_local_git_repositories_resolve_like_pub`: o destino do `git clone` sai sem `\\?\`;
- `archives_keep_inner_links_and_skip_escaping_ones`: o Pub 3.6.2 extrai um *symlink* (e um
  *hard link*, que vira *symlink*) com `Link(path).createSync(relativo)`, que no Windows é
  `CreateSymbolicLinkW` com o alvo relativo (de diretório quando o alvo já é um diretório). O
  `unpack` faz o mesmo (`symlink_dir`/`symlink_file`, que pedem o modo sem privilégio) em vez de
  recusar; sem o privilégio (sem o Modo de Desenvolvedor e sem elevação) a extração falha, como no
  Pub, e o teste verifica a falha nesse caso em vez de ser desligado;
- `frozen::tests::a_load_onto_the_frozen_prefix_is_a_full_load` (`crates/elements`; falhou na
  máquina Windows antes desta correção, provavelmente a quarta da lista): o teste canonicalizava a raiz com o prefixo e comparava com caminhos sem ele; agora usa a mesma
  função.

Verificado na máquina Windows (`ssh dfwin-isaque`, cópia em `E:\Temp`, removida depois; nada no
checkout): `cargo test --release -j 4 -p dartforge-cli -p dartforge-elements` sem falhas, os 12
testes de `pub_get` incluídos (o Modo de Desenvolvedor está ligado lá, então o caminho com *symlink*
foi o exercitado). Os `canonicalize` de outros crates (`runtime`, `lsp`, `build`, `parity`,
`emit_native`, `sass`) não foram mexidos.

**Windows inteiro em `fe855176` (02/11, notebook i3-1215U, Windows 11 x64, `E:\Temp\winval`):**
`cargo test --release --workspace` com **3133 passados, 0 falhas** (195 ignorados, 33 min, `-j 4`);
diferencial nativo de `corpus/native` rodado no próprio Windows (não cruzado), contra o `dart run`
3.6.2: **164/164** em desenvolvimento (276 s) e **164/164** com `--optimize` (5326 s, `--jobs 3`).
O tempo do `--optimize` (19× o de desenvolvimento) é a mesma lacuna do tempo de compilação do AOT. Corpus JS
(`corpus/js`, Node do Windows): **277/277** em desenvolvimento e **277/277** em produção, DDC×VM
261/277, os mesmos números do macOS.

### Correções do backend nesta rodada

Na ordem pedida:

1. ***Mixin* aplicado duas vezes** (`mixin/super_2`, `mixin_legacy/super_2`,
   `closure/identity_equality_tearoff`): uma cópia por aplicação dos membros que usam `super`, com
   entradas, tabelas e desvirtualização por cópia; desenho em `docs/NATIVO.md` §6. O *layout* não
   mudou (os campos de um *mixin* aplicado duas vezes continuam com uma posição só).
2. **`void` que devolve valor** (`void/void_check`): um `void` de corpo em bloco com `return e`, ou
   cuja sobrescrita devolve valor, devolve `Ref`; desenho e a prova de que vale no wasm e no JS em
   `docs/NATIVO.md` §7 (famílias de sobrescrita mistas, nas duas direções, conferidas contra a VM no
   nativo, no wasm e no JS).
3. **Inferência de literal de mapa sem tipo**: a causa era o inicializador de um campo/variável de
   topo sem tipo ser inferido uma vez só, sem contexto. O CFE infere de novo o inicializador com o
   tipo inferido como contexto; agora nós também (`infer_untyped_variable_type`), e
   `final t = {0: {60: [1]}, 1: {}}` dá `t[0]` como `Map<dynamic, dynamic>` como na VM, no nativo
   e no JS (dev e produção).
4. Avulsos, os mais relevantes para produção primeiro:
   - **`rethrow` perdia a pilha original** (nativo e wasm): um `rethrow` lança de novo com a pilha da
     exceção capturada (§17.11) também sem a variável `s` do `catch` (ou com um `_`); antes saía com
     uma pilha nova. Medido contra a VM com `Error.throwWithStackTrace` e `catch (e)`, `on T {}`,
     `catch (a, b)`;
   - **entrada de mapa *null-aware*** (`{?k: v}`, `{k: ?v}`, 3.8) no nativo e no wasm: era recusada
     como construção não suportada;
   - **`_` com *wildcard variables*** (3.7, padrão do código novo): o parâmetro de tipo `_` não
     liga nome no corpo, na assinatura nem no supertipo (`class B<_> extends A<_>` usa o `typedef _`
     da biblioteca); um parâmetro `_` comum não é local, e `this._`/`super._` são lidos pela posição
     (`C.withPositional(this._, _)` e `M(this._, super._, super._)`);
   - **recusa de `_` repetido abaixo da 3.7**: numa biblioteca sem *wildcard variables* (versão < 3.7
     e sem o experimento), dois parâmetros, variáveis de `catch`, locais ou parâmetros de tipo `_`
     no mesmo escopo, campo de registro `_` e membro `_` em conflito com parâmetro de tipo `_` são
     erros, como no CFE (`dartforge_analysis::wildcards`, chamado pelo `emit_native` e pelo
     `emit_js`): antes o JS e partes do nativo aceitavam;
   - ***inference-using-bounds*** com dependência da direita para a esquerda
     (`class A<Y, X extends Iterable<Y>>`, `A(<num>[])` é `A<dynamic, List<num>>`): os parâmetros
     são escolhidos em ordem, e a restrição que o limite de `X` põe sobre o `Y` já escolhido é
     descartada, como no `_chooseTypes` do analisador;
   - **inferência de argumentos de tipo de extensão** por menor limite superior, registrada por
     acesso (`static_extension_inference`), o *factory* redirecionado de tipo de extensão para o
     construtor primário (`regress_53607`), *stubs* que lançam para privados inacessíveis no lugar
     de *forwarders* de `noSuchMethod` (`inference_update_2` ×2).

### Validação

- suíte: JS dev completa (2 863, `jsdev-r5c`); nativo e wasm em 860 programas (`f17`, `f17-wasm`:
  as falhas, os 58 de experimento, os 4 dos pacotes e todas as áreas tocadas), com a projeção acima;
  nenhuma regressão;
- harness nativo no binário final: dev, `--optimize`, `--optimize --gc-stress` e `--wasm`
  **164/164** cada (`--jobs 3`; no wasm, 40 iguais ao dart2wasm);
- `cargo test --release` de `analysis`, `types`, `elements`, `frontend`, `emit-js`, `emit-native`,
  `runtime`, `world` e `cli` sem falhas no macOS; `cli` e `elements` também no Windows; o teste do
  inventário com `embedded-llvm` passa; clippy `-D warnings` e `rustfmt` limpos;
- diferencial do JS (desenvolvimento e produção) no `corpus/js` inteiro: **277/277** e **277/277**
  (a inferência mudou em `crates/types`);
- `native_sdk` não mudou nesta rodada: o manifesto do SDK selecionado não precisou ser regenerado.

### Ficou, com o motivo

- pilhas simbólicas e *hashes* (o AOT sai com `--stack-trace none`): os 9 de pilha e os 2 de *hash*
  da lista acima;
- o experimento `variance` (22 programas, recusados com o nome do experimento), os 8 de `macros` e
  `augmentation_libraries/class_augmentation` (o próprio CFE 3.6.2 quebra com 252);
- os 5 que a VM recusa (ou em que falha) e nós rodamos;
- os campos de um *mixin* aplicado duas vezes (uma posição por aplicação no objeto): o desenho do
  §6 não muda o *layout*;
- `typedef/aliased_constructor_tear_off` (não tentado);
- no JS, o *mixin* aplicado duas vezes (`mixin/super_2`, `mixin_legacy/super_2`) continua entre as
  47 falhas: a cópia por aplicação foi feita só no backend nativo.

## 01/11/2026 (noite): backend nativo — quarta rodada: inventário do SDK selecionado e mais 4 da suíte

**O teste ignorado do inventário** (`actual_selected_sdk_inventory_loads_without_authorizing_opaque_startup`,
`--features embedded-llvm`) certificava só `dart:nativewrappers` em vez dos 8 corpos de partida
esperados. Causa: `863d4159` (terceira rodada) passou a manter, no `join_arc_facts`, os nomes dos
*tear-offs* das funções do SDK baixadas em módulos temporários — a correção do `toString` sem nome —,
e com isso o registro de cada biblioteca com *tear-offs* ganhou `dartforge_register_tearoff`,
`dartforge_register_tearoff_key` e `dartforge_register_instantiation` (endereços de código
entregues ao runtime), uma obrigação que o inventário não certifica; era exatamente o que já tirava
o `dart:mirrors` desde `c29dfd3c`. Os commits do ARC (`86a58081`, `6e26cb65`, `eccb3651`) não têm
relação. Correção na raiz: uma biblioteca do SDK entrega esses registros numa função própria,
`df.register.<lib>.tearoffs` (`sdk_module::tearoff_registration_symbol`), chamada na partida depois
dos registros; o registro volta a ser só metadado nominal e textual. Resultado: os 8 certificados de
novo e **também o `dart:mirrors`** (o teste agora espera os 9, com o motivo no comentário).

**Medida da suíte.** `run-language-suite.py native --jobs 3` em 708 programas: as 35 falhas e todos
os que passavam em `async/`, `const/`, `regress/`, `list/`, `map/`, `set/`, `deferred/`,
`closure/`, `null/`, `enum/`, `constructor/`, `patterns/` e afins (as áreas tocadas: a ordem de
erro do `async`, os mapas constantes, a cascata). No wasm, 157: as 35 e `async/`, `const/`, `list/`.

| | antes | depois |
|---|---:|---:|
| nativo: as 35 falhas | 35 | **31** (4 passam) |
| nativo: os 673 que passavam nas áreas tocadas | 673 | 673 (nenhuma regressão) |
| wasm: as mesmas 35 (6 já saíam como a VM ou o dart2wasm) | 6 | **8**; os 122 de fora, nenhuma regressão |
| nativo, projeção | 2 774 iguais, 2 828 (98,8%) | **2 778 iguais, 2 832 (98,9%)** |
| wasm, projeção | 2 834 (99,0%) | **2 836 (99,1%)** |

Harness nativo no binário final: dev, `--optimize`, `--optimize --gc-stress` e `--wasm` **164/164**
cada (`--jobs 3`; no wasm, 40 iguais ao dart2wasm). `cargo test --release` de `emit-native`, `runtime` e `world`, o teste do inventário
com `embedded-llvm` e clippy `-D warnings` limpos. Manifesto revisado do SDK selecionado
regenerado (mudaram só a unidade de `dart:_compact_hash` e a de *patch* do `dart:async`).

Corrigido:
- **`Future` do usuário que entrega valor de tipo errado ao `await`**: a partir do primeiro `await`,
  uma exceção não capturada do corpo `async` completa o futuro na hora, como o `_handleException`
  da VM faz quando a ativação já tem `_SuspendState`; antes do primeiro `await` continua
  assíncrona. O erro que sai do `then` de um futuro do usuário chega à zona de forma síncrona quando
  ninguém escuta ainda (`async_patch.dart`, `isSync` no `_await`). Três casos medidos contra a VM
  (erro do próprio `then`, `TypeError` do valor, erro depois de uma suspensão): saída idêntica;
- **mapa `const` com `hashCode` sobrescrito**: a fonte de um `const {…}` (mapa ou conjunto) com
  elementos simples recebe os elementos nos dados sem *hash* (`_HashVMBase._dfConstAdd`), e o
  `_ConstMap`/`_ConstSet` indexa com o *hash* canônico, como a constante do front end na VM;
- **campo `external` sem implementação** (fora de FFI e interop): ler e escrever lançam
  `NoSuchMethodError`, e o despacho o trata como membro ausente;
- **`regress41983`**: numa cascata sobre um literal de lista novo, cada `..add(x)` com `x`
  estaticamente do tipo do elemento é o *append* do literal (`dartforge_list_append`), sem os
  testes de classe e de forma, o caminho de crescimento e o despacho de reserva do `add` inlinado.
  As 20 000 `..add` compilam em **134 s** (objeto 1,8 s; antes não terminava em 2 400 s).

Ficou, com o motivo:
- `void/void_check` (sobrescrita de `void` que devolve valor): na ABI, um método `void` de corpo em
  bloco não devolve nada, e a chamada estática do `bar()` dentro dele já descarta o valor da
  sobrescrita; igualar a VM exige que todo `void` cujo corpo faz `return e` (e as chamadas de
  métodos `void` sobrescritos) devolvam `Ref`, uma mudança de ABI que vale um desenho próprio;
- ***mixin* aplicado duas vezes** (3): continua o desenho da rodada anterior (uma cópia dos métodos
  com `super` por aplicação, com entradas, tabelas e desvirtualização por cópia) — não feito;
- pilhas simbólicas e *hashes* (11), os 10 que a VM recusa e nós aceitamos, construções recusadas e
  inferência, como antes.

## 01/11/2026 (tarde, 15h): LSP — prefixo congelado: só o pacote é recarregado numa mudança de API

Primeiro passo da prioridade do topo. O projeto e esta rodada estão em `docs/LSP.md`
("Projeto: reanalisar só o que mudou" e "Quinta rodada"). Commits `9bb31e06` (projeto),
`a91e9b4c` e `264b658c`.

**O que mudou:**
- `crates/elements` põe o SDK e as dependências num prefixo congelado do programa
  (`load_lenient_frozen`, `Program::frozen`). Numa mudança de API, `load_lenient_onto`
  recarrega só o pacote sobre o prefixo (`build_outline_from`).
- `crates/types` resolve só o *outline* depois do prefixo (`resolve_outline_onto`).
- `crates/parity` usa os dois (`Taken::Onto`) e mantém a tabela de tipos, os `CoreTypes` e o
  *outline* do prefixo. A cada 16 cargas sobre o prefixo há uma recarga total.
- Treze *lints* e duas verificações de `phase_deprecated` deixaram de procurar
  classes e construtores andando pelo programa inteiro.
- O caminho dos compiladores não muda.

| ms (mediana) | frontend: r4 → agora × Dart | backend | `limitless_ui` |
|---|---|---|---|
| API privada → diagnóstico do arquivo | 478 → **236** × 25 | 127 → 105 × 30 | 272 → **161** × 85 |
| API exportada → diagnóstico do arquivo | 583 → **226** × 50 | 119 → 97 × 61 | 179 → **86** × 53 |
| API privada: CPU | 691 → **448** × 50 | 1.353 → 1.293 × 161 | 919 → 750 × 137 |
| API exportada: CPU | 2.180 → 1.999 × 110 | 1.388 → 1.337 × 183 | 866 → 657 × 101 |
| memória em repouso (MiB) | 744 → 776 × 847 | 286 → 299 × 469 | 1.045 → 1.105 × 773 |

**Validação:**
- testes em *release* de `elements`, `types`, `analysis`, `world`, `emit_js`,
  `emit_js_production`, `parity`, `lsp` e `dev`: 842, sem falhas;
- corpus JS 277/277 em desenvolvimento e em `-O2`;
- harness nativo: 164/164 (desenvolvimento);
- oráculo de paridade com a tabela de recursos idêntica.

**Ainda não estamos abaixo do Dart:**
- o diagnóstico do arquivo leva 4–10× o do Dart. Falta o passo 4: as bibliotecas do pacote
  que não dependem da editada e `simple_bounds`;
- os dependentes custam 7–18× em CPU;
- `references` dos tipos do SDK continua de 4 a 37× (passo 5);
- a memória em repouso no `limitless_ui` subiu 6%.

## 01/11/2026: backend nativo — terceira rodada nas falhas da suíte `tests/language`

**Medida.** Mesmo procedimento das rodadas anteriores (`scripts/run-language-suite.py native` e
`wasm`, `--jobs 2`, máquina em carga 50–130). No nativo, 567 programas: as 48 falhas que restavam
e todos os que passavam em `deferred/`, `closure/`, `null/`, `const/`, `enum/`, `patterns/`,
`async/`, `constructor/`, `invalid_returns/`, `list/`, `mixin/`, `extension_methods/` e
`nonfunction_type_aliases/` (as áreas tocadas). No wasm, 146: as 48 e `deferred/`, `closure/`,
`null/` e `enum/`.

| | antes | depois |
|---|---:|---:|
| nativo: as 48 falhas | 48 | **35** (13 passam) |
| nativo: os 519 programas que passavam nas áreas tocadas | 519 | 519 (nenhuma regressão) |
| wasm: as mesmas 48 (6 já saíam como na VM ou no dart2wasm) | 6 | **19**; os 98 de fora, nenhuma regressão |
| nativo, projeção para a suíte inteira | 2 761 iguais, 2 815 (98,3%) | **2 774 iguais, 2 828 (98,8%)** |
| wasm, projeção | 2 821 (98,5%) | **2 834 (99,0%)** |

Harness nativo no binário final, duas vezes: rebaseado sobre o `main` da primeira rodada do ARC
real e de novo sobre `47f3ce7a` (segunda rodada do ARC e o quadro de raízes enxuto do AOT, que
mexeu no `dead_slots` junto com esta rodada): dev, `--optimize`, `--optimize --gc-stress` e
`--wasm` **164/164** cada nas duas (`--jobs 2`, `--exec-limit` 60–120; no wasm, 40 iguais ao
dart2wasm); `cargo test --release` de `emit-native`, `runtime`, `world` e `types` e clippy
`-D warnings` limpos. JS de produção nos programas do corpus de padrões,
`if-case`, *sealed*, records, extensões e enums (35, por causa da mudança no `crates/world`):
desenvolvimento e produção 35/35.

Corrigido (`crates/emit_native`, `crates/runtime`, `crates/types`, `crates/world` e
`native_sdk/async/async_patch.dart`, com o manifesto revisado do SDK selecionado regenerado):
- **`deferred`: o *callback* de `p.loadLibrary().then(...)` não rodava** (achado da rodada
  anterior; importa para produção). Um programa que não importava `dart:async` não compilava o
  laço de eventos, e o `Future` de `loadLibrary` completa num *timer* (`_dartforgeLoadLibrary`):
  o `then` nunca era chamado, sem erro nenhum. Uma importação `deferred` agora conta como uso de
  `dart:async` (`source.rs`, `uses_dart_async`). `p.loadLibrary` como valor também
  (`lower/deferred.rs`: uma *closure* sem parâmetros que carrega o prefixo);
- ***tear-off* dinâmico de membro do SDK** (achado da rodada anterior): os nomes dos *tear-offs*
  e as entradas de instanciação das funções baixadas em módulos temporários (as do SDK) se
  perdiam no `join_arc_facts`, e o `toString` saía sem o nome. E `null.toString == (null as
  dynamic).toString` dava `false`: a entrada do tipo estático (`Object.toString$tearm`) e a que o
  despacho escolhe (`Null.toString$tearm`, outra cópia na imagem do SDK) tinham códigos
  diferentes. Agora cada entrada registra uma chave (`dartforge_register_tearoff_key`) — o
  seletor para o *tear-off* de método de instância (com a biblioteca num nome privado), o símbolo
  para os demais — e o `==`/`hashCode` do runtime comparam a chave e o receptor por `identical`;
- **`closure/type_arguments`**: a *closure* genérica local chamada dinamicamente sem argumentos
  de tipo instancia aos limites, e um limite que cita o parâmetro de tipo da *closure* de fora é
  lido na tupla de fora (`closures.rs`);
- **análise de raízes (`llvm/roots.rs`)**: o ponto fixo calcula a entrada de cada bloco no
  lugar, sem as cópias do conjunto vivo por instrução que o visitante precisa, e o `dead_slots`
  pré-computa os passos de cada bloco (pontos de coleta e escritas de *slot*) em vez de
  consultar dois mapas por instrução a cada volta. Num `regress41983` reduzido (2 000 `..add`,
  a mesma forma), a fase "LLVM IR" caiu de 38,1 s (CPU 15,6 s) para **5,0 s (CPU 2,0 s)**;
- **padrão de objeto que lê membro de extensão** (`P(:var b)`, `R3<…>(:var $3)`): o `crates/types`
  passa a registrar o membro também fora do fluxo do analisador (`field_elements`), o mundo
  fechado o mantém vivo (`crates/world/src/impact.rs`) e o backend o chama direto;
- **regra de supertipo com `FutureOr<Null>`** (`class G extends F<G<FutureOr<Null>?>>`): a
  tabela de RTI registra as formas do runtime antes das regras, para que a leitura de uma regra
  normalize `FutureOr<Null>` em `Future<Null>?` sabendo qual classe é `Future`;
- chamada genérica com prefixo (`prefixed.mk()` com o tipo inferido pelo contexto) recebe os
  argumentos de tipo inferidos; `return` de valor `dynamic` num corpo `async` checa
  `FutureOr<T>` (o futuro é esperado depois, como na VM); `List.filled(-1, …)` e
  `List.generate(-1, …)` lançam `RangeError (length)` (`List_allocate`, limite
  `Array::kMaxElements`);
- **enums**: `v6.new()` é o construtor sem nome; os argumentos de tipo inferidos viram o tipo do
  valor (`v2(y: 2)` é um `EnumAll<num, int>`); `super.toString()` e `super.index` chegam aos
  membros do `_Enum` da VM, também a partir de um *mixin* aplicado ao enum;
- `external` do programa sem implementação lança `NoSuchMethodError` na chamada, como na VM;
- um `Future<T>` definido pelo usuário que entrega à continuação do `await` um valor que não é
  `T` lança `TypeError` (`_checkFutureValue`, `async_patch.dart`). No nativo o teste
  `await_user_defined_future_soundness` ainda falha: a VM entrega esse erro à zona de forma
  síncrona quando ele sai do `then` antes da primeira suspensão, e aqui ele volta ao corpo e
  completa o futuro com erro (o wasm passa, igual ao dart2wasm).

Ficou, com o motivo:
- **pilhas simbólicas e *hashes* (11)**: os 9 de pilha (`stack_trace/*` ×6, `regress21795`,
  `private/mixin_exception_throw`, `private3`) comparam o texto do rastro da VM, e o padrão do AOT
  é `--stack-trace none` (o texto fixo `#0 main (dart:native)`); com `--stack-trace symbolic`,
  no macOS aarch64, com e sem `--optimize`, o rastro ainda sai sem quadros. Ligar o rastro por
  padrão é decisão de produto (o tamanho da tabela não foi medido,
  NATIVO-MAPAS-DE-PILHA-E-EXCECOES §13.14). Os 2 de *hash* (`unsorted/savannah`,
  `patterns/non_interface_object_pattern`) imprimem valores de `identityHashCode` da VM:
  inviável igualar;
- **`regress/regress41983`**: a análise de raízes e o *lowering* deixaram de ser o gargalo (no
  programa inteiro, o HIR caiu de 94,5 + 92,7 s para 3,7 + 1,9 s, e a CPU nos primeiros 2 400 s,
  de 945 para 380 s), mas a cascata de 20 000 `..add` vira uma função de ~240 000 blocos (cada
  `add` inlinado com o caminho rápido, o de crescimento e o despacho de reserva), e o LLVM não
  termina o objeto e o LTO dentro do limite. Passa a exigir não inlinar o `add` além de um
  orçamento por função, ou provar a classe exata do alvo da cascata;
- ***mixin* aplicado duas vezes** (3): o desenho continua o da rodada anterior (uma cópia dos
  métodos por aplicação, indexada a partir da base da cadeia) — não implementado;
- `void/void_check`: as funções `void` não devolvem valor na ABI, e a VM devolve o de uma
  sobrescrita que retorna; `const/map_hashcode_override`: o mapa `const` é montado a partir do
  `_Map` do literal, que chama o `hashCode` sobrescrito (a VM usa o *hash* canônico);
  `external_fields`: campo `external` ainda é um campo comum;
- `string/interpolation_and_buffer`: a VM 3.6.2 avalia o argumento do `write` antes do
  `toString` de `StringBuffer(t)` (o teste falha na VM); nós seguimos a especificação;
- construções recusadas (fábrica redirecionante de extension type, *tear-off* de construtor
  por typedef genérico), inferência (`static_extension_inference`, `inference_update_2` ×2) e os
  10 que a VM recusa e nós aceitamos (frente do frontend).

## 01/11/2026 (noite): LSP — o pacote inteiro só onde paga, respostas grandes como texto

Quarta rodada contra o `dart language-server` 3.6.2, revezando o servidor novo, o da terceira
rodada (`ddf83232`) e o Dart, 3 sessões de cada (carga 33–87; outros agentes usavam a máquina,
então valem a CPU e as razões medidas na mesma rodada). Tabelas e detalhes em `docs/LSP.md`,
"Quarta rodada".

* **O pacote inteiro no programa do residente só onde paga** (`Resident::context_pays`): entra
  quando o programa do pacote é no máximo 1,5× o do maior arquivo editado (medido nas
  importações transitivas do lote). Frontend e backend continuam com ele; o `limitless_ui`
  (os testes puxam o `package:test`: 2,2×) volta a carregar só o programa do lote, e o
  diagnóstico do próprio arquivo depois de uma mudança de API voltou a ficar perto do Dart.
* **`references` com milhares de localizações sai como texto JSON**, sem a árvore de valores
  (~145 MiB para as 53.624 de `String` no frontend): pico do processo do servidor 743 → 357
  MiB.

| ms (mediana; CPU) | `new_sali/frontend`: r3 → agora × Dart | `new_sali/backend` | `limitless_ui` |
|---|---|---|---|
| tecla num corpo → assentado | 236 → 219 (83) × 83 (47) | 72 → 72 (66) × 84 (42) | 456 → 344 (126) × 370 (155) |
| API exportada → diagnóstico do arquivo | 1.689 → 1.334 × 84 | 346 → 469 × 122 | 746 → **327** × 247 |
| API privada → diagnóstico do arquivo | 1.935 → 1.413 × 89 | 454 → 336 × 75 | 998 → **646** × 263 |
| API exportada → assentado (CPU) | 8.731 → 7.409 (2.299) × 338 (111) | 6.052 → 6.940 (1.466) × 598 (178) | 2.905 → 3.177 (852) × 425 (143) |
| API privada → assentado (CPU) | 3.161 → 2.375 (735) × 139 (44) | 6.686 → 4.932 (1.396) × 606 (158) | 3.062 → 3.638 (951) × 453 (158) |
| `references` sem edição (CPU) | 9.626 → 9.553 (2.157) × 344 (174) | 4,5 → 2,1 × 1,3 | 3,6 → 4,1 × 81 |
| memória pico / repouso (MiB) | 1.988/1.024 → **1.623/736** × 867/781 | 284/284 → 281/281 × 485/483 | 1.194/1.065 → 1.027/995 × 771/754 |

O assentado dos dependentes no `limitless_ui` ficou dentro do ruído (nesta rodada 3,2–3,6 ×
2,9–3,1 s; na medição à parte com o mesmo binário, automático 2,2–2,5 s × sempre 2,7–3,2 s),
com ~5% a mais de CPU. Não foram feitos: o *outline* incremental (no `limitless_ui` sobram
~0,3 s de carga e 0,15 s de *outline* na primeira fase; pede identidade estável de elementos
em `crates/elements` e `OutlineTypes` por biblioteca em `crates/types`) e o índice de
referências resolvidas (pede registrar o alvo de cada resolução do verificador de
`crates/parity` com uma identidade comum à de `crates/lsp/src/project.rs`; parcial, quebraria
o oráculo). Testes de `crates/lsp`, `crates/parity` e `crates/dev` em release; oráculo de
paridade nos quatro projetos com a tabela de recursos idêntica.

## 01/11/2026 (tarde): LSP — o pacote inteiro no programa do residente, `completion` em assinaturas

Terceira rodada contra o `dart language-server` 3.6.2 (3 rodadas revezadas, carga 29–81).
Tabelas e detalhes em `docs/LSP.md`, "Terceira rodada".

| ms (mediana; CPU) | `new_sali/frontend`: manhã → agora × Dart | `new_sali/backend` | `limitless_ui` |
|---|---|---|---|
| tecla num corpo → assentado | 102 → 83 (82) × 40 (44) | 62 → 73 (70) × 38 (46) | 142 → 197 (122) × 218 (152) |
| API exportada → dependentes | 5.947 → **4.644** (2.179) × 119 | 5.504 → **2.918** × 370 | 1.402 → 1.989 × 279 |
| API privada → dependentes | 2.629 → **1.692** × 54 | 5.300 → **2.526** × 375 | 1.798 → 2.371 × 228 |
| `completion` 1º depois de edição | 1.073 → **24** × 77 | 21 → **12** × 47 | 34 → 31 × 37 |
| `completion` sem edição | 163 → **26** × 5,7 | 36 → 16 × 12 | 107 → 116 × 14 |
| `references` sem edição | 7.255 → 4.785 × 110 | 4,0 → 4,2 × 1,3 | 3,7 → 3,4 × 23 |
| memória pico / repouso (MiB) | 2.096/1.170 → 1.857/**1.066** × 937/866 | 274/274 → 269/269 × 363/320 | 1.194/1.088 → 1.104/1.030 × 738/734 |

- **(1) Mudança de API.** O residente monta, para qualquer lote de um pacote de workspace,
  o programa com todos os arquivos do pacote (`Resident::set_context`): a análise dos
  dependentes reaproveita o programa que a do arquivo editado acabou de montar, e só infere
  os corpos deles. Os dependentes caíram à metade no backend e ~35% no frontend. A primeira
  fase (o próprio arquivo) ainda recarrega: o *outline* incremental (refazer só as
  bibliotecas afetadas) não foi feito. No `limitless_ui`, cujo pacote inteiro é bem maior que
  o programa de um arquivo, ficou mais lento.
- **(2) Índice de nomes referenciados: não feito.** Ele escolheria quase os mesmos arquivos
  que o filtro textual atual; o que custa é inferir os corpos de quem cita o nome. Medido: o
  DartForge devolve exatamente o que o Dart devolve nas 5 posições do frontend (`String`
  53.624 localizações, `bool` 22.978…). Ganhar exige um índice de referências resolvidas
  mantido pelo residente.
- **(3) `completion`** numa assinatura (tipo ou nome de campo, nome de método) responde com a
  análise retida quando a troca com a sentinela é recusada por mudar a assinatura.
- **(4) Memória.** Resumos das bibliotecas conhecidas e o teste de `part of` sem corpos; a
  conversão de posições de arquivos fechados guarda só o último arquivo. O pico do frontend
  vem das respostas de `references` aos tipos do SDK, montadas como JSON; o
  `malloc_zone_pressure_relief` não muda o *footprint* (testado, não ficou).

**Validação**: `cargo test --release -p dartforge-lsp -p dartforge-parity` 358/358 e clippy `-D warnings` limpo (`elements`, `frontend` e `dev` não mudaram nesta rodada). Testes novos: `resident_context_shares_the_program_between_batches`
(parity) e posições em assinaturas em `tests/incremental.rs` (comparadas a cada passo com o
servidor sem o caminho incremental). Oráculo de paridade: tabela de recursos idêntica à das
rodadas anteriores.

## 01/11/2026: ARC real — candidatos morrem no caminho rápido, corpos crus sem foto, escopos nativos baratos

Rodada sobre o ARC real (`DARTFORGE_ARC_PREPARED=1 --memory arc`), guiada por contadores
temporários no runtime (removidos antes dos commits) e por `sample`. Detalhes em
`docs/ARC-IMPLEMENTACAO.md` §9.3. Commits `062faf70`, `ca8dda78`, `fbd9ef19` e `cb4fef8e`.

**O que os contadores mostraram no servidor `dart:io`** (por pedido): 92 liberações chegam
ao runtime, e só 35% morriam no caminho rápido — 62% eram recusadas por serem candidatas a
ciclo e 3% por uma foto de escrita crua pendente. Os pontos de assentamento eram 385 (96
completos), e 285 escopos nativos abriam e fechavam. Cada pedido deixa uns 66 objetos de
lixo cíclico (11 `_Closure` e 11 contextos, 6 `_Future`, 6 `_HashMapEntry`, 5 `_List`, 4
`_AsyncCompleter`, 3 `_SyncStreamController`, 2 `_HttpHeaders`, e o `_HttpRequest`, o
`_HttpResponse` e o `_HttpIncoming` do pedido): a rodada de ciclos é necessária.

**O que mudou:**
- **Candidato a ciclo morre no caminho rápido.** O `prompt_release_last` aceita a palavra com
  `CANDIDATE` numa página de classe; a célula é reusada na hora e a entrada que sobra no
  buffer não nomeia ninguém. A rodada de ciclos só usa como semente a entrada cuja palavra
  ainda tem `CANDIDATE` (antes, a célula reusada virava semente e a rodada percorria o fecho
  de um objeto vivo). Candidato de região grande continua no caminho geral. Recusas: 62% → 0%.
- **Corpo `RAW` sem foto.** Bytes e unidades de código escritos crus (`words_mut`,
  `bytes_mut`, `fresh_body`) não mudam contagem nenhuma, e a foto deixava toda liberação e
  todo assentamento no caminho geral até o próximo assentamento. O `set_flags` continua
  fotografando, porque a troca de forma transforma palavras cruas em referências. Fotos por
  pedido: ~66 → ~14; assentamentos completos: 96 → 13.
- **Escopo nativo vazio barato.** O fechamento sem dono (o caso comum) não toca o heap; os ids
  saem de blocos por thread, sem atômico por escopo. O *thunk* Owned de um nativo que nunca
  roda Dart (`effects.tsv`) não abre escopo nenhum, e sem o teste de exceção pendente quando
  ele não lança.
- **Alvo de closure tipada** checado no registro uma vez por thread e época, sem o *mutex* do
  registro a cada chamada (45 por pedido).
- **Registro em linha do objeto novo:** o retain em linha registra a palavra zerada (objeto
  novo do runtime) com a primeira ocorrência, `CYCLE_FREE` quando o corpo é `RAW`.
- **Assentamento simples:** só zeros a drenar e nada pendente drena direto e sai.
- Quadros observacionais contados em vez de percorridos a cada última liberação.

**Medido e descartado:** `CYCLE_FREE` na alocação em linha de classes sem campo `Ref`
(nenhuma diferença: quase não há tais objetos no servidor) e a eliminação de pares
retain/release na mesma linha reta sem decremento no meio (nenhuma diferença: a preparação já
não deixa esses pares).

**Medidas** (Mac mini M4, contadores da thread do isolate, `load_keepalive` 4×8 por 4 s,
carga da máquina 48–60 com outros agentes; mediana de 3 rodadas alternadas tracing, ARC real
antes e depois; o binário tracing não muda nesta rodada):

| servidor | tracing | ARC real antes (`5dd38a71`) | ARC real agora (`cb4fef8e`) |
|---|---:|---:|---:|
| `dart:io`, instruções/pedido | 194 mil | 813 mil | **697 mil (−14%)** |
| `dart:io`, ciclos/pedido | 118 mil | 343 mil | **305 mil (−11%)** |
| `dart:io`, RSS | 17,9 MB | 25,7 MB | 25,9 MB |
| cru, instruções/pedido | 123 mil | 361 mil | **277 mil (−23%)** |
| cru, ciclos/pedido | 56,5 mil | 121,6 mil | **107,5 mil (−12%)** |
| cru, RSS | 16,2 MB | 21,9 MB | 22,0 MB |

Os ciclos absolutos estão inflados pela carga (a medida de 30/10, com a máquina quieta, dava
66 mil no tracing e 205 mil no ARC real); a razão vale. Para comparar: o modo rápido sem
contagem (`DARTFORGE_ARC_TOKENS=virtual`) gasta 530 mil instruções por pedido e o ARC simples
316 mil, no mesmo servidor e na mesma rodada.

**Onde está o resto** (o ARC real ainda gasta 3,6× as instruções do tracing no `dart:io`):
- **O código preparado em si:** o modo rápido, sem contagem nenhuma, já custa 2,7× o tracing:
  quadros donos, chamadas da ABI de listas e campos e os 250 escopos nativos por pedido que
  sobram (nativos que podem rodar Dart, quase todos só no caminho de erro: leituras de
  *typed data*, `list_get`, `codeUnitAt`). Passo seguinte: marcar no `effects.tsv` os que só
  rodam Dart para construir o erro e abrir o escopo só nesse caminho.
- **A drenagem e a rodada de ciclos** dominam o tempo de amostragem (~20% do isolate, faltas de
  cache) mais do que as instruções (~25 mil por pedido medidas com `DARTFORGE_ARC_CYCLES=never`).
  Passo seguinte: a rodada sobre a palavra no objeto (*trial deletion* no lugar, sem o mapa
  handle→índice).
- **Escritas de globais** (`dartforge_gc_global_root`: mapa e dois swaps por escrita, ~2,5%).

**Validação** (worktree do agente, carga 40–100):
- corpus `corpus/native` (`arcmodes.sh`), cada programa normal, com estresse
  (`DARTFORGE_GC_STRESS=1`) e com auditoria (`DARTFORGE_ARC_CHECK=1`): ARC real 154/154 em
  `fbd9ef19` e em `cb4fef8e`; ARC simples e modo rápido 154/154 em `cb4fef8e`. O
  `168_uso_de_valor_void_erro` é um teste de erro de compilação, igual ao `dart run`;
- harness `dartforge-differential --native` (tracing, o padrão): dev, `--optimize` e
  `--optimize --gc-stress`, **163/163** cada (`fbd9ef19`; o `cb4fef8e` só muda o assentamento
  do ARC real);
- os passos ARC do job macOS do CI: as 8 provas `prove-arc-aot.py` (`arc_native_port_owned`,
  `arc_source_return`, `arc_zeroed_instance`, `arc_local_fields`, `arc_field_allocation`,
  `arc_field_keepalive`, `arc_keepalive_pending`, `arc_loop_cleanup`), os campos tipados,
  o *resume* Itanium e o desenrolar estrangeiro com heap real: 11/11;
- `cargo test --release -p dartforge-runtime`: 339 + integração + 93 doc-tests, sem falha;
  `-p dartforge-emit-native --features embedded-llvm`: 1048 + integração, sem falha;
- clippy `-D warnings` em `dartforge-runtime` e `dartforge-emit-native` (`--all-targets`) e
  `cargo fmt` limpos (o único diff do `cargo fmt --check` é de outro agente, em
  `crates/emit_js_production/tests/shared_rule_bodies.rs`).

### Segunda rodada (01/11, manhã): escopos só no erro, índice da rodada na palavra, globais

Commits `0dcad6b0`, `13b69923`, `2a4e577e`, `f81d55b8` e `b35c41ed`, sobre o `main` que já traz
as rodadas do backend nativo, do AOT e do wasm. Detalhes em `docs/ARC-IMPLEMENTACAO.md` §9.4.
- **Escopo nativo só no caminho de erro.** Os nativos que rodam Dart só para construir o erro
  que lançam passam pelo *helper* de lançamento, que abre o próprio escopo (`throw_index`,
  `throw_range`, `throw_range_err`, `throw_type_error` e a falha do `as`): leituras e escritas
  de *typed data*, `DartForge_list_get`, os `_setIndexed`, `codeUnitAt`, `String.[]`, os
  `unbox_int/double/bool` e o `rti_as_with_context`. A preparação os chama sem *thunk* de
  escopo (`arc::NATIVES_SCOPING_THEIR_DART`; a certificação das portas também os aceita). No
  SDK compilado sobravam ~17 mil sítios de escopo em linha só de `unbox_int` e
  `rti_as_with_context`; os que restam (`nsm_call`, `safepoint`, `unbox` de outro tipo) são de
  erro ou caminho lento.
- **A rodada de ciclos sem o mapa handle→índice:** o objeto da palavra no objeto empresta os
  bits da contagem ao seu índice na região enquanto as arestas são resolvidas (`IN_REGION`,
  bit 46), com a contagem guardada no vetor do *trial* e devolvida antes dele. Um oráculo de
  grafos aleatórios sobre a palavra confere sobreviventes, contagens e que não sobra marca.
- **Escrita de global:** uma consulta só ao registro e um único *swap*; a cópia gerada para um
  global forte não retém nem solta um token em volta do `dartforge_gc_global_root`.

**Medidas** (mesmo método; carga 35–40; mediana de 3 rodadas alternadas tracing, ARC real do
`main` (`9abd761e`) e ARC real agora (`b35c41ed`)):

| servidor | tracing | ARC real antes | ARC real agora |
|---|---:|---:|---:|
| `dart:io`, instruções/pedido | 200 mil | 705 mil | **666 mil (−5,6%)** |
| `dart:io`, ciclos/pedido | 123 mil | 311 mil | **306 mil (−2%, no ruído)** |
| `dart:io`, RSS | 18,0 MB | 26,0 MB | 25,1 MB |
| cru, instruções/pedido | 123 mil | 277 mil | **248 mil (−10%)** |
| cru, ciclos/pedido | 56 mil | 107 mil | **98 mil (−8%)** |
| cru, RSS | 16,2 MB | 22,6 MB | 22,1 MB |

Somando as duas rodadas de 01/11: `dart:io` 813 → 666 mil instruções (−18%) e 343 → 306 mil
ciclos (−11%); cru 361 → 248 mil instruções (−31%) e 122 → 98 mil ciclos (−20%). O ARC real ainda
gasta 2,5× os ciclos do tracing no `dart:io` e 1,7× no cru.

**Validação** (`b35c41ed`, testes em `a19ee9c9`): corpus `corpus/native` nos três modos ARC
(real, simples e rápido), cada programa normal, com estresse e com auditoria, 155/155 cada (o
`168_uso_de_valor_void_erro` é um teste de erro de compilação); harness `--native` do tracing
164/164 em dev, `--optimize` e `--optimize --gc-stress`; `dartforge-runtime` 342 + integração +
93 doc-tests e `dartforge-emit-native --features embedded-llvm` 1055 + integração, sem falha;
os passos ARC do job macOS do CI 11/11 (as 8 provas `prove-arc-aot.py`, campos tipados, *resume* Itanium, desenrolar estrangeiro); clippy `-D warnings` nos dois crates e
`cargo fmt` limpos.

**O `sample` parou de anexar** a processos nesta máquina depois de uma sessão do `lldb` (fica
esperando autorização de acesso ao processo); a rodada usou só os contadores de hardware e a
contagem estática dos escopos no bitcode do SDK.

## 01/11/2026: backend nativo — segunda rodada nas falhas da suíte `tests/language`

**Medida.** Mesmo procedimento da rodada anterior (`scripts/run-language-suite.py native`,
`--jobs 2`, a máquina em carga 50–130): as 57 falhas que restavam e os programas que passavam
em `closure/`, `mixin/`, `nnbd/syntax/`, `extension_type/`, `generic_methods/`, `function/` e
`generic/` (424 programas em duas rodadas).

| | antes | depois |
|---|---:|---:|
| nativo: as 57 falhas | 57 | **48** (9 passam) |
| nativo: programas que passavam nas áreas tocadas | — | nenhuma regressão |
| wasm: as mesmas 57 (6 já saíam como no dart2wasm) | 6 | **15** (os mesmos 9), nenhuma regressão |
| wasm, projeção | 2 812 (98,2%) | **2 821 (98,5%)** |
| nativo, projeção para a suíte inteira | 2 752 iguais, 2 806 (98,0%) | **2 761 iguais, 2 815 (98,3%)** |

Antes de mexer, o harness nativo no binário do `main` integrado (`13a6ccc8`, com as duas
correções que só a suíte tinha medido): dev, `--optimize`, `--optimize --gc-stress` e `--wasm`
163/163. Depois das correções, os quatro modos de novo: 163/163 em cada (`--jobs 2`, `--exec-limit` 60–120 por causa da carga). JS de produção nos programas
de extension type e interop do corpus (13): iguais (a mudança no `crates/world` também vale
para ele).

Corrigido (`crates/emit_native`, e o `crates/world` para o membro combinado):
- **aplicação de mixin nomeada sobre classe abstrata sem construtor** (`class A = B with C`, `B`
  abstrata): `A()` cria o objeto e roda os construtores padrão implícitos — campos dos mixins
  aplicados e de cada superclasse (`mixin/super_with_initializer` ×2,
  `mixin_declaration_syntax`);
- **`late` com inicializador capturado por *closure***: o estado fica na célula e o
  inicializador roda na primeira leitura, onde ela estiver; a *closure* captura também o que o
  inicializador lê (análise de capturas e variáveis livres da *closure*). Como na VM, toda
  leitura antes de haver valor roda o inicializador, também a reentrante, e um `late final` que
  encontra valor quando o inicializador volta lança "has been assigned during initialization"
  (`late_modifier_edge_cases`, `late_modifier_runtime_error`);
- **membro combinado de extension type** (`implements A, B`): o mundo fechado nativo mantinha
  vivo só o cone do extension type e podava o *getter* da classe da representação; agora o cone
  é a classe da representação (`extension_type/combined_member_signature`);
- **instanciação aos limites da chamada dinâmica sem argumentos de tipo**: um limite que usa os
  parâmetros da classe é lido no tipo do receptor (`A<int>().foo()` dá `Q1 extends P` = `int`) e
  os limites seguintes recebem os anteriores; o *tear-off* de método checa os argumentos de tipo
  passados contra os limites e os parâmetros que os citam (`function/apply_generic2`); método sem
  parâmetros de tipo chamado com argumentos de tipo responde com o `noSuchMethod` do receptor
  (`generic/tearoff`);
- o construtor de funções acha o bloco corrente pelo id, sem varrer os blocos a cada instrução
  (uma cascata de 20 000 elementos era quadrática). `regress/regress41983` ainda estoura os
  300 s: o gargalo agora é a análise de raízes do coletor (`llvm/roots.rs`, conjuntos densos de
  bits por instrução e `dead_slots`) numa função gigante;
- `regress/regress31436` também passou a sair igual nesta rodada.

Ficou para depois, com o motivo:
- ***mixin* aplicado duas vezes na mesma cadeia** (3: `mixin/super_2` ×2,
  `identity_equality_tearoff`): o código do mixin é um só e o `super` resolve pela classe
  dinâmica; resolver exige uma cópia dos métodos por aplicação (índice contado a partir da base
  da cadeia, que é estável entre subclasses), com entradas uniformes, tabelas de métodos e
  desvirtualização por cópia. É uma mudança grande no despacho para três testes;
- `closure/type_arguments` avançou até a última parte (genérico local chamado dinamicamente
  com limite que cita o parâmetro de tipo da *closure* de fora);
- `deferred` com `main() => p.loadLibrary().then(...)` cujo *callback* lê um membro da
  biblioteca: o *callback* não roda (achado, não investigado a fundo);
- `toString` do *tear-off* dinâmico de um membro do SDK (`(null as dynamic).toString`,
  `(3 as dynamic).toString`): sai sem o nome e não é igual ao estático.

Continuam abertos (48): os de cima; construções recusadas (`external` ×2, `loadLibrary` como
valor, fábrica redirecionante de extension type, *tear-off* de construtor por typedef genérico,
construtor nomeado de `enum`) e o de 300 s (7); pilhas simbólicas e *hashes* (11); os 10 que a VM
recusa e nós aceitamos (frente do frontend); avulsos.

## 01/11/2026: LSP — o residente reaproveita o programa a cada tecla

Segunda rodada contra o `dart language-server` 3.6.2 (mesmo driver, projetos e arquivos de
31/10; 3 rodadas revezadas; carga 32–91). Tabelas completas em `docs/LSP.md`, "Segunda
rodada".

| ms (mediana; CPU) | `new_sali/frontend`: 31/10 → agora × Dart | `new_sali/backend` | `limitless_ui` |
|---|---|---|---|
| tecla num corpo → assentado | 1.294 → **102** (75) × 58 (50) | 148 → **62** (61) × 38 (42) | 531 → **142** (112) × 225 (150) |
| edição de corpo → diagnóstico | 1.449 → **124** × 29 | 200 → 52 × 17 | 538 → **171** × 217 |
| API exportada → diagnóstico do arquivo | 983 → 692 × 59 | 80 → 106 × 64 | 221 → 169 × 204 |
| API exportada → dependentes | 9.807 → **5.947** × 246 | 5.022 → 5.504 × 382 | 2.772 → 1.402 × 392 |
| `completion` sem edição | 277 → 163 (84) × 4,7 | 13 → 36 (13) × 8,6 | 292 → 107 (74) × 20 |
| `references` sem edição | 7.226 → 7.255 × 173 | 4,1 → 4,0 × 1,3 | 4,7 → 3,7 × 54 |
| memória pico / repouso (MiB) | 1.981/1.522 → 2.096/**1.170** × 939/863 | 308/215 → 274/274 × 461/458 | 1.129/1.058 → 1.194/1.088 × 764/764 |

- **Residente incremental** (`crates/parity/src/analysis.rs`, `World`): o processo residente
  guarda o programa, as tabelas de tipos, o *outline*, os limites simples e o resultado das
  passadas de inicializadores. Quando a análise seguinte é do mesmo lote e só corpos dos
  arquivos do lote mudaram, as unidades trocam de texto no lugar (`swap_unit`) e os
  diagnósticos desses passos só mudam de posição. No frontend, por tecla: carga 0,25–0,55 s
  → 0,01–0,03 s, *outline* 0,4–0,8 s → < 0,03 s, inicializadores 0,4–0,8 s → 0.
- **Buscas de referências que não cabem** num processo filho (`--reference-search`).
- **`completion` fora de um corpo** com a análise retida do texto real.
- **Lotes grandes no residente** (`DARTFORGE_LSP_RESIDENT_BATCH`, padrão 1.024; era 32): os
  dependentes de uma exportação do *barrel* do frontend assentam em 4,9 s em vez de 9,3 s,
  com pico total menor (472 × 791 MiB).

**Validação**: testes de `lsp`, `elements`, `frontend`, `parity` e `dev` em `289f373c` (main)
passam (654/654); no código desta rodada, `cargo test --release -p dartforge-lsp -p dartforge-parity` 357/357 e clippy `-D warnings` limpo (`elements`, `frontend` e `dev` não mudaram). Testes novos: `resident_reuses_the_program_when_only_bodies_change`
(parity) e `reference_search_in_a_child_process_finds_the_same` (lsp). Oráculo de paridade
(4 projetos, 410 posições): tabela de recursos idêntica em `22955178`, `289f373c` e agora —
references, implementation e definition 100%, completion top-5 100%, top-1 99%.

**O que falta**: *outline* incremental para mudanças de API (os dependentes ainda custam
3,6–34×); índice de nomes referenciados no residente (`references` no frontend, 4,5–42×);
`completion` dentro de um corpo quando a troca com a sentinela é recusada (~1 s de CPU no
frontend); a memória das recargas da biblioteca no servidor (frontend: pico 2,2×, repouso
1,36×).

## 31/10/2026: AOT nativo de programa grande — 2,45× menos CPU para compilar, executável 15% menor (rodadas até 01/11)

### 01/11/2026: correção — o descritor da área global uma vez entre as partes

A validação v18 do coordenador (`corpus/native` com `DARTFORGE_PART_KB=64`, `--optimize` com e sem
`--gc-stress`) pegou o `175_nested_literals_by_table` imprimindo `counter` como 0 na última linha.
Nenhuma das chaves das rodadas 4 e 5 mudava nada (textos, zeragem, saídas, `PRELINK_O=2`: todas
falhavam igual). A causa estava no particionador: o runtime acha a área global enxuta de produção
pelo **endereço** do descritor `@df.area`, e o particionador copiava cada constante local pequena
(< 4 KB) para cada parte que a cita. Com o programa dividido, cada parte tinha o seu descritor, e
a parte que caía no caminho lento do `@df.get_area` ganhava uma área própria (o `next()`
incrementava um `counter`, o `main` lia outro). No `new_sali/backend` o descritor tem 76 KB e já
era definido uma vez (constante grande compartilhada), por isso o programa grande de verdade não
falhava. Correção (`f04cd31e`): o `@df.area` sai sem `unnamed_addr` (o endereço é significativo),
e o particionador define uma vez toda constante local sem `unnamed_addr`, de qualquer tamanho.
Teste novo `partition::tests::small_constant_with_significant_address_is_defined_once`.

* Validação: `corpus/native` 164/164 com `DARTFORGE_PART_KB=64` (`--optimize` com
  `DARTFORGE_CHECK_SHADOW=1`, e `--gc-stress`), 164/164 pelo pipeline do programa grande
  (`DARTFORGE_PART_KB=64 DARTFORGE_LARGE_PROGRAM_FUNCTIONS=0`, nos dois modos), 164/164 nos
  quatro modos normais; clippy limpo.
* A interação com a mudança do agente do backend em `roots.rs` (ponto fixo sem copiar o
  conjunto vivo, passos de `dead_slots` pré-calculados) está certa: a zeragem só em laço continua
  valendo por bloco nos pontos de coleta e no fim do bloco.
* `cargo test -p dartforge-emit-native`: tudo passa menos
  `actual_selected_sdk_inventory_loads_without_authorizing_opaque_startup` (só o registro de
  `dart:nativewrappers` certificado, em vez de 8). **Já falha no `main` atual sem esta correção**
  (testei com a mudança do emissor revertida); vem de algum commit depois de `a9151f62` que muda o
  corpo dos registros do SDK (suspeitos: `86a58081`, a gravação de raiz global, ou as rodadas de
  ARC e do backend). Não mexi.

### 01/11/2026: quinta rodada — moldura de raízes mais enxuta e saídas compartilhadas

A forma do código do programa grande, cada forma com a sua chave (só no programa grande em
produção com rastreamento e pilha-sombra; detalhes em `docs/NATIVO-PRODUCAO-GRANDE.md` §9):

1. **textos da imagem estática sem raiz** (17% das gravações na moldura; `DARTFORGE_ROOT_STATIC_TEXTS=1` volta);
2. **zeragem das vagas mortas só dentro de laços** (45% das gravações eram zeragens; fora de laço o
   valor morto fica na vaga até ela ser regravada ou a moldura fechar; `DARTFORGE_CLEAR_DEAD_SLOTS=all` volta);
3. **uma saída por constante em cada função** (`optimize/shared_exits.rs`: os blocos vazios
   `return padrão` depois de cada chamada com exceção pendente viram um; 164 mil blocos a menos;
   `DARTFORGE_SHARED_EXITS=0` volta).

`new_sali/backend`, `aot --optimize`, objetos frios, mesmo binário, três pares alternados, carga 45–116:

| | IR | gravações na moldura | ligação (CPU, mediana) | **CPU total (mediana)** | executável |
|---|---:|---:|---:|---:|---:|
| as três desligadas | 515,7 MB | 848 892 | 337,8 s | 507,8 s | 37,61 MB |
| **padrão novo** | **493,8 MB** | **447 860** | **309,1 s** | **451,0 s** | **36,37 MB** |
| `dart compile exe` | | | | 39,0 s | 26,46 MB |

* Ligação −29 s (−8,5%), menos CPU nos três pares; executável **−1,24 MB**. Contra o Dart:
  **~11,6× a CPU** (era 12,1×, medido com carga menor), **1,37× o tamanho** (era 1,41×).
* Cada forma sozinha: −0,43 MB (textos), −0,73 MB (zeragem), −0,15 MB (saídas); a CPU de uma forma
  sozinha fica dentro do ruído da carga.
* Não entraram: o mapa de referências na gravação de campo (sem prova de que todo objeto nasce
  com o mapa estático, não é seguro), `receiver_provenance` reaproveitado (o módulo muda entre as
  três análises; sem prova), adaptadores `$c`/`$g`/`$tc` e `$async` (próxima rodada).
* `bench/performance` e o servidor HTTP: IR de produção idêntico byte a byte com as chaves
  desligadas e no padrão (12 programas).
* Validação: `corpus/native` pelo pipeline do programa grande (`DARTFORGE_PART_KB=64`) 164/164
  em `--optimize` (com `DARTFORGE_CHECK_SHADOW=1`) e `gc-stress`, o `80` incluído; no padrão
  164/164 nos quatro modos; `cargo test --release -p dartforge-emit-native --features
  embedded-llvm -- --include-ignored` todo verde; clippy limpo.

### 01/11/2026: quarta rodada — pré-ligação `-O1`, teste `is` fora de linha, ICF revertido

`new_sali/backend`, `aot --optimize`, SDK em cache, objetos frios, mesmo binário (base: `main`
de 01/11):

| | CPU | relógio (carga) | executável |
|---|---:|---:|---:|
| sem os itens desta rodada, ICF ligado | 495 s | 261 s (71) | 37,26 MB |
| **padrão novo** | **469 s** | 260 s (59) | **37,42 MB** |
| `dart compile exe` | 38,8 s | 78 s | 26,46 MB |

* **Pré-ligação `-O1` das partes** (era `-O2`; o ThinLTO otimiza de novo): os objetos caem de 93
  para 66 s de CPU, sem mudar a ligação nem o tamanho. `-O0` poupa mais (24 s), mas aumenta o
  executável 0,1 MB e fica atrás de `DARTFORGE_PRELINK_O=0`.
* **O teste `is` com cache do local de uso fora de linha no programa grande** (`@df.rti_test`):
  ~16 mil cópias de ~40 instruções de IR viram chamada; IR 534 → 512 MB, −0,8 MB.
* **`"nooutline"` nos inicializadores longos de topo** do programa grande (o *outliner* gastava
  ~17 s nas tabelas do `saslprep` para 4 KB).
* **ICF desligado de novo.** O `corpus/native` passado pelo pipeline do programa grande
  (`DARTFORGE_PART_KB=64`, chave nova que divide até um programa pequeno em partes) mostrou o
  `80_tearoffs_de_corpo_identico` divergindo com o `--icf=safe_thunks` da rodada de 31/10: os
  objetos que o ThinLTO do `ld64.lld` 22.1.8 gera não têm `__llvm_addrsig`, e o `safe` dobrava
  *tear-offs*. Devolve +1,0 MB; `DARTFORGE_ICF` ainda liga.
* Contra a rodada de 31/10: **575 → 469 s de CPU (12,1× o Dart, era 13,9×)**; executável 36,98 →
  37,42 MB (1,41×), por causa do ICF que estava errado.
* Desempenho: o servidor HTTP e os programas de `bench/performance` não mudam (programas
  pequenos; os itens só valem no programa em partes ou grande). O A/B substituto — os benchmarks
  pelo pipeline das partes, `-O2` contra `-O0` — deu executáveis do mesmo tamanho e uma oscilação
  de 0,84× a 1,87× com código praticamente igual: com a máquina carregada, inconclusivo.
* `receiver_provenance` três vezes: ~12% da CPU da HIR (~6 s); não mexi (o módulo muda entre as
  três). Detalhes em `docs/NATIVO-PRODUCAO-GRANDE.md` §8.
* Validação: `corpus/native` 164/164 nos quatro modos (desenvolvimento, `--optimize` com
  `DARTFORGE_CHECK_SHADOW=1`, `gc-stress`, wasm) e 164/164 pelo pipeline do programa grande
  (`--optimize` e `gc-stress`); `cargo test --release -p dartforge-emit-native --features
  embedded-llvm -- --include-ignored`: tudo menos
  `actual_selected_sdk_inventory_loads_without_authorizing_opaque_startup` (o inventário passou
  a certificar o registro de `dart:async`; esta rodada não muda o IR do SDK, e o teste passava
  antes do `main` de 01/11); clippy limpo.

### 31/10/2026: terceira rodada

Medido no macOS arm64 (Apple M4, 16 GB) com outros cinco agentes na máquina (carga 30–157 em
10 núcleos): o número principal é a **CPU** (usuário + sistema, com os filhos Clang e
`ld64.lld`), as medidas do DartForge e do `dart compile exe` foram alternadas, e o relógio vai
junto com a carga. Detalhes, perfis e o que resta em `docs/NATIVO-PRODUCAO-GRANDE.md` §7.

**`new_sali/backend`, `aot --optimize`, SDK em cache, objetos frios:**

| | CPU | relógio (carga) | executável |
|---|---:|---:|---:|
| antes | ~1 410 s | 2 620 s (92) | 43,52 MB |
| **agora** | **575 s** | 589 s (73) | **36,98 MB** |
| `dart compile exe` (mediana de 7) | 41,3 s | 78 s | 26,46 MB |

Ainda 13,9× a CPU e ~7,5× o relógio do Dart, e 1,40× o tamanho. Religação sem mudança: 91 s de
CPU. Desenvolvimento frio do backend: 235 → 177 s de CPU.

**`dart_ui/examples/file_explorer`** (38 mil funções HIR, abaixo do limiar de programa grande):
334 → 303 s de CPU e 20,69 → 19,88 MB (`dart compile exe`: 16,7 s e 12,26 MB). Como programa
grande (`DARTFORGE_LARGE_PROGRAM_FUNCTIONS=0`) daria 15,66 MB e 214 s, com o custo da §5.1 do
doc nas chamadas virtuais: o limiar fica para o dono decidir.

**O que pesava** (perfil `--time-trace` da ligação e amostras do `sample` na emissão):
* os dois `unormdata` (o mapa literal não constante do `unorm_dart` e a cópia dele no
  `dargres`): 22 MB de IR cada, numa função; `Machine code sinking` levou 690 + 628 s de tarefa
  do ThinLTO neles e eram o caminho crítico;
* o *machine outliner* no `df.register.program` (28 mil registros): 141,6 s de CPU contra 2,4 s
  sem ele, para o mesmo código;
* 33 cópias do descritor da área (`@df.area`, 76 KB) no executável, uma por parte;
* buscas lineares que eram quadráticas: chave de mapa/conjunto constante (front-end, avaliação e
  verificação) e o bloco atual do `FnBuilder` (a cada instrução emitida).

**O que entrou:**
* literal grande com literais aninhados de escalares constantes como dados
  (`lower/literals.rs`, `_dartforgeFillNested`), com inteiros curtos na tabela; teste
  `corpus/native/175_nested_literals_by_table.dart`; o hash revisado do `compact_hash.dart` no
  manifesto do SDK selecionado (`double_identity/selected_sdk_manifest.json`) foi atualizado —
  sem isso as provas de `double`/`clamp`/multiplicação recusavam o SDK e desligavam;
* constante local de 4 KB ou mais definida uma vez para todas as partes (`partition.rs`);
* `"nooutline"` no `df.register.program`;
* ICF `--icf=safe_thunks` com `-mllvm -addrsig` na ligação de produção do programa em partes
  no macOS (`DARTFORGE_ICF` escolhe): −1,0 MB no backend; o `corpus/native` passou com ele
  ligado em todo programa, mas no pequeno só tira 0,5% e fica desligado (**revertido em 01/11**:
  no programa em partes ele junta *tear-offs*, ver acima);
* índice por valor primitivo nas chaves de mapa/conjunto constante (`const_eval`) e bloco atual
  por índice;
* tarefas de geração por núcleo físico e memória (3 → 8 nesta máquina; `DARTFORGE_PARALLEL_PARTS`
  continua mandando);
* `--timings` com a CPU de cada fase; `DARTFORGE_LTO_PROFILE`, `DARTFORGE_PRINT_LINK` e
  `DARTFORGE_LARGE_PROGRAM_FUNCTIONS` como chaves de medida no macOS.

**Validação**: `corpus/native` **164/164** (os 163 e o 175) em desenvolvimento, `--optimize` com
`DARTFORGE_CHECK_SHADOW=1`, `--optimize --gc-stress` e `--wasm` (`--exec-limit 60`: com a
máquina cheia, cinco programas passavam dos 5 s padrão no `gc-stress`), duas vezes: com o ICF
ligado em todo programa e com o código final (ICF só no programa em partes, manifesto revisto); `cargo test --release -p
dartforge-emit-native --features embedded-llvm -- --include-ignored`: 1080 da biblioteca, 29 de
integração e 128 *doctests*, todos passam (a primeira rodada achou as 8 provas do SDK
selecionado recusando o `compact_hash.dart` novo: é o manifesto acima); `dartforge-types`,
`dartforge-runtime` e `dartforge-instrument` também; clippy `-D
warnings` e `rustfmt` nos crates tocados. O backend sobe como o do Dart (mesmas mensagens de
partida; sem banco nem Redis, os dois param na mesma conexão recusada). Desempenho: os programas
de `bench/performance` e o servidor HTTP não passam pelos caminhos novos (nenhum tem literal
aninhado por tabela — conferido no IR — e o ICF só liga no programa em partes); com o ICF
ligado neles a medida ficou no ruído da máquina carregada (0,64–1,38 por núcleo, sem direção).

**Achado para o agente do backend nativo/front-end:** num mapa literal sem tipo cujo valor é
`{60: [...], ...}` ao lado de um `{}`, o DartForge dá `Map<int, List<Object?>>` ao valor e a VM
dá `Map<dynamic, dynamic>` (o LUB desce para os literais). Anterior a esta rodada.

## 01/11/2026, madrugada: JS de produção — `async_await` igual ao `dart2js`, gzip dos programas pequenos abaixo dele

Detalhes em `docs/JS-PRODUCAO-VELOCIDADE.md` §14. Sobre o `31f63ea8`.
Validado localmente no macOS: corpus JS 277/277 em desenvolvimento e em
`-O1` e `-O2`, 260/260 em `-O3`, 256/256 em `-O4` (`--jobs 2`);
`production_forms` e o novo `json_closed_world` contra a VM em `-O1` a
`-O4` com as chaves ligadas e desligadas; testes de `emit_js`,
`emit_js_production` e `world` com os ignorados (191, 0 falhas); `clippy -D
warnings`; `rustfmt`. Medidas com execuções alternadas na mesma sessão e o
`uptime` anotado (carga 35–170).

**`async_await`: 1,25× → 0,99×** (580,5 → **463,5** ms; `dart2js -O4`
466,0). O perfil mostrava ~11% em avaliações de rti do `_thenAwait` de cada
`await` (`_Future<E>` e `_FutureListener<T, E>` por `bind`/`_eval`, buscas
em `Map`) e um `dart.fn` com rti em volta do corpo de cada invocação
`async`:
- `DARTFORGE_JSPROD_AWAIT_LISTENERS`: o `await` cria um `_Future<dynamic>`
  e, na zona raiz, um `_FutureListener<dynamic, dynamic>` — tipos fechados,
  avaliados uma vez; fora da raiz o ouvinte mantém os tipos (o `runUnary`
  de uma zona própria os vê);
- `DARTFORGE_JSPROD_ROOT_ZONE_BODIES`: na zona raiz o corpo é o próprio
  passo protegido (o que `registerBinaryCallback` da raiz devolveria).

**Gzip dos programas pequenos: 1,0009× → 0,963×** do `dart2js -O4` (16
programas: 697.384 → 668.824 bytes, gz 239.191 → 230.107; os 15 de antes,
0,968×). `DARTFORGE_JSPROD_UNANSWERED_CALLS`: o `object.toJson()` dinâmico
do `toEncodable` padrão do `jsonEncode`, num programa em que nenhuma classe
declara `toJson` (nem `noSuchMethod` além do `Object`), é a chamada
JavaScript, como o `dart2js` escreve — o `TypeError` é o
`NoSuchMethodError` do Dart, e `dart.dsend` com todo o caminho do
`noSuchMethod` (`InvocationImpl`, símbolos, `_checkAndCall`) sai do
programa (`json_encode` gz 20.299 → 17.087, `bench` 20.364 → 16.921). Só
para `toJson`: a primeira versão, para qualquer nome sem membro, quebrou
`f.call(3)` no corpus (91): nomes dos protótipos do JavaScript rodariam.

**Projetos:** `limitless_ui` 3.350.051 → 3.350.108 (gz 947.491 → 947.526),
`new_sali` 7.147.069 → 7.147.126 (gz 1.939.532 → 1.939.506): **+57 bytes**
em cada (0,002%), o método novo do `await` e a guarda da zona (sem a
guarda os projetos encolhiam, mas o tipo visto por um `runUnary` próprio
mudaria).

**Medidos e descartados:** a fábrica do *pretty printer* do JSON só no
`withIndent` (−464 bytes gz por programa sem `withIndent`, +230 no
`limitless_ui`, que o usa); o `jsonEncode` escrevendo numa *string* própria
em vez do `StringBuffer` (no ruído, +245 bytes); a rti do *completer*
pronta no ponto de chamada (o `dart2js` faz as mesmas duas avaliações, e
cada função `async` ficaria maior).

**O que continua:** `jsonEncode` 1,0–1,2× (sem um ponto quente no perfil);
o `json` de `bench/performance`, gz 1,034× (usa `withIndent` e o
`JsonUtf8Encoder`).

## 31/10/2026: backend nativo — falhas da suíte `tests/language` (AOT e wasm)

**Medida.** `scripts/run-language-suite.py native` (`dartforge aot --optimize`) e `wasm`
(`compile-wasm` + `dartforge-wasm-run`), contra o `vm.json` da VM 3.6.2. Com a máquina em
carga ~60–130 (seis agentes), a suíte inteira não coube: rodei, com `--jobs 2`, as 88 falhas
do `native-31-10` e mais os 148 programas que passavam em `closure/`, `function_subtype/`,
`covariant/`, `covariant_override/` e `generic_methods/` (as áreas que as mudanças tocam);
no wasm, as mesmas 88.

| | antes | depois |
|---|---:|---:|
| nativo: as 88 falhas do `native-31-10` | 0 passam | **31 passam**, 57 falham |
| nativo: 148 programas que passavam (áreas tocadas) | 148 | 148 (nenhuma regressão) |
| wasm: as mesmas 88 (contra `wasm-final`) | — | **29 passam a sair certos**, nenhuma regressão |
| nativo, projeção para a suíte inteira | 2 721 iguais, 2 775 (96,9%) com paridade | **2 752 iguais, 2 806 (98,0%)** |
| wasm, projeção | 2 783 (97,2%) | **2 812 (98,2%)** |

As projeções somam as correções ao resultado anterior e supõem que os ~2 600 programas fora
do recorte não regrediram; o harness nativo cobre o resto (abaixo). A suíte inteira deve ser
remedida com a máquina livre.

Corrigido em `crates/emit_native` e `crates/runtime` (`fb98e615`):
- ***tear-off* com o tipo do que o receptor roda**: o *forwarding stub* que torna um
  parâmetro covariante (`class D extends C implements I`, `C.f(B)` e `I.f(covariant A)`) e a
  sobrescrita com outros tipos dão o tipo reificado pela classe dinâmica, escolhido pelo id
  da classe do receptor (`typed_entry::tearoff_variants`, `rti.rs`). `super.m` continua
  exato;
- **RTI de tipos de função genéricos**: limite `dynamic`/`void` escrito é outro tipo que o
  `Object?` implícito (`void Function<X extends dynamic>()` ≠ `void Function<X>()`), e o
  runtime normaliza a lista de limites toda `Object?`; uma variável é subtipo do seu limite
  (`Y1 extends Y0`, ambiente de limites no `sub`); substituir um parâmetro de classe sob
  *binders* desloca os níveis (`B<i>`); typedef sem argumentos instancia nos limites, com os
  níveis deslocados; variável com limite `Never` vira `Never` (até o ponto fixo); nomes
  aninhados como a VM (`<Y0 extends <F1Y0>(F1Y0) => F1Y0>`);
- **checagens que faltavam**: parâmetro de tipo função, record ou `FutureOr` na entrada
  dinâmica (os que citam parâmetros de tipo da função ficam com a checagem que recebe a
  tupla); limites na instanciação de um valor de função genérico
  (`dartforge_rti_instantiation_bounds`); o *tear-off* de método genérico cujo limite cita o
  parâmetro da classe (`C<Object>.f2` de um `C<A>`), também antes de uma instanciação
  implícita; argumentos de tipo passados a uma instanciação (`NoSuchMethodError`); limites e
  parâmetros no encaminhador de `noSuchMethod` e no *tear-off* instanciado; casts implícitos
  no construtor de uma aplicação de mixin nomeada (`C<T> = B<T> with M`); parâmetro
  covariante de `super ==`;
- **igualdade e identidade**: `super == null` não chama o operador; `C<int>.new` e
  `E.staticMethod<int>` (estático de extensão) canônicos;
- `Invocation` com listas e mapas imutáveis, como a VM (`native_sdk/core/identical_patch.dart`;
  manifesto revisado do SDK selecionado regenerado em `9ca484f9`, só a unidade de *patch*
  do `dart:core` e a impressão digital do núcleo);
- `switch` com corpo de vários casos: as variáveis do corpo são outras que as de cada caso
  (a *closure* da guarda guarda a do caso); campo de record em padrão de objeto
  (`R3<…>(:var $1)`); `d < e` com `d` dinâmico devolve o resultado do operador; `o<T>` em objeto com `call`
  genérico (`EnumAll.v1<String>`); parâmetro de tipo do membro sombreia o da extensão.

Harness nativo (`dartforge-differential --corpus corpus/native`, `--jobs 2–3`, com o binário
anterior às duas últimas correções — parâmetro covariante de `super ==` e o ponto fixo do
NORM de `Never`, que só a suíte mediu): dev 163/163,
`--optimize` 163/163, `--optimize --gc-stress` 163/163 e `--wasm` 163/163 (15 iguais ao
dart2wasm). Com a carga, o limite de execução de 5 s estourou em 6 programas do
`--gc-stress` e em 126 do `--wasm`; repetidos com `--exec-limit 120`, todos passaram.

Continuam abertos no nativo (57; no wasm, 51 das mesmas 88):
*mixin* aplicado duas vezes na mesma cadeia — o código do mixin é um só e o `super` resolve pela
classe dinâmica (`mixin/super_2` ×2, `identity_equality_tearoff`) (3); aplicação de mixin nomeada
sobre classe abstrata sem construtor (3); construções recusadas ou incompletas (`external` sem
implementação ×2, `loadLibrary` como valor, fábrica redirecionante de extension type, *tear-off*
de construtor por typedef genérico, construtor nomeado de `enum`) e um que estoura os 300 s (7);
pilhas simbólicas e *hashes* (11); programas que a VM recusa e o DartForge aceita
(experimentos `wildcard-variables` e `value-class`, limites F inferidos, `super` sem *setter*;
um deles o CFE quebra com 252) (10); `late` com inicializador em *closure* (2); avulsos (21):
`toString` do *tear-off* de `Object.toString` e de estático de classe em constante, inferência
da extensão pelo UP (`Pair<int, double>` em `on Pair<T, T>`), membro combinado de extension
type que implementa duas classes, mapa `const` com `hashCode` sobrescrito, valor `void`
sobrescrito que retorna, *getter* de extensão em padrão de record, `noSuchMethod` com
`inference-update-2`, entre outros.

## 31/10/2026: LSP contra o servidor oficial do Dart nos mesmos projetos

Primeira medição do `dartforge-lsp` e do `dart language-server` 3.6.2 com o mesmo cliente
nos mesmos projetos (`new_sali/frontend`, `new_sali/backend`, `limitless_ui`), revezando os
dois a cada sessão, 3 rodadas, com tempo de CPU por pedido e por edição. Método, tabelas
completas e o que falta em `docs/LSP.md`, "DartForge × servidor oficial nos mesmos
projetos".

**Carga da máquina: 40–190 (10 núcleos, seis agentes).** O tempo de parede inclui a espera
por núcleo; as razões perto de 1× não são conclusivas, e a coluna de CPU é a régua mais
estável.

Medianas, ms (CPU entre parênteses quando medida), DartForge antes → depois × Dart 3.6.2:

| | `new_sali/frontend` | `new_sali/backend` | `limitless_ui` |
|---|---|---|---|
| partida até a 1ª análise | 14.408 → 10.771 (3.702) × 8.804 (4.839) | 8.468 → 6.577 (2.270) × 3.551 (3.353) | 18.461 → 16.040 (4.522) × 6.849 (4.514) |
| tecla num corpo → assentado | 1.832 → 1.294 (403) × 82 (48) | 372 → 148 (112) × 46 (44) | 752 → 531 (191) × 232 (149) |
| edição de API exportada → diagnóstico do arquivo | 10.313 → 983 × 104 | 6.605 → 80 × 63 | 2.803 → 221 × 184 |
| idem → dependentes assentados | 10.313 → 9.807 × 302 | 6.606 → 5.022 × 491 | 2.803 → 2.772 × 278 |
| `hover` sem edição | 3.605 → 2,3 × 1,5 | 51 → 2,3 × 0,6 | 11 → 0,7 × 1,3 |
| `completion` sem edição | 5.357 → 277 (83) × 9,2 (5,0) | 82 → 13 × 8,9 | 312 → 292 (80) × 18 (18) |
| `definition` sem edição | 3.366 → 3,2 × 1,4 | 54 → 2,9 × 0,9 | 14 → 4,2 × 1,6 |
| `references` sem edição | 20.961 → 7.226 (2.283) × 235 (107) | 82 → 4,1 × 1,3 | 4.131 → 4,7 × 36 |
| `prepareRename` sem edição | 5.347 → 2,0 × 0,9 | 48 → 2,9 × 0,5 | 2.871 → 5,1 × 0,9 |
| `codeAction` sem edição | 3.537 → 19 × 2,0 | 76 → 8,9 × 1,3 | 192 → 70 × 4,5 |
| memória, todos os processos: pico / repouso (MiB) | 1.969/654 → 1.981/1.522 × 888/864 | 331/224 → 308/215 × 359/321 | 1.322/1.039 → 1.129/1.058 × 762/762 |
| diagnósticos finais iguais aos do Dart | 0 × 0 | **99,6%** (6.680 de 6.704) | 0 de 1 (24 nossos em pastas com `pubspec` próprio) |

**Ganhamos ou empatamos** em `hover`, `references` no backend e no `limitless_ui`, no
diagnóstico do próprio arquivo numa edição de API e na memória do backend. **Perdemos** no
diagnóstico por tecla (2–16× em parede, 1,3–8× em CPU: o residente refaz o programa
inteiro a cada análise), nos dependentes de uma mudança de API (10–32×), em `references` e
`completion` no frontend (8–31×), na partida em parede (1,2–2,3×, com CPU igual ou menor) e
na memória do frontend e do `limitless_ui` (pico 1,5–2,2×; o repouso do frontend piorou por
causa das cargas de `references` dentro do servidor).

**Mudanças** (`crates/frontend`, `crates/elements`, `crates/lsp`):
- carga de biblioteca das consultas com o SDK e os pacotes **sem corpos**
  (`parse_lexed_outline`, `load_lenient_outline`) e orçamento da sessão semântica 8 → 32 MiB:
  o programa de um arquivo do frontend fica retido e a edição de corpo troca a unidade no
  lugar;
- chave da sessão sem o SDK e o *pub cache*, e com a data das pastas do projeto em vez de
  percorrer a árvore (a consulta reaproveitada custava 13 ms de CPU; agora 1–3 ms);
- `prepareRename` e a identificação do nome de `references`/`implementation` pela biblioteca
  (antes: o projeto inteiro, recarregado a cada pedido);
- busca de referências estreita (só os arquivos que citam o nome) quando a carga larga não
  cabe no orçamento dela; não fica retida;
- diagnósticos em duas fases numa mudança de API: o arquivo editado primeiro, os dependentes
  depois;
- o processo residente serve as unidades do SDK que já guardava, em vez de decodificá-las do
  cache do SDK a cada análise (`AnalyzedUnitCache::take_sdk_library`; vale também para a
  sessão residente do `dartforge dev`).

**Testes**: `cargo test --release -p dartforge-lsp -p dartforge-elements -p dartforge-frontend
-p dartforge-parity -p dartforge-dev` passa (o `dev`, 15/15, com o `runtime/ddc/dart_sdk.js`
copiado da cópia principal: sem ele, 7 testes falham por pré-requisito); clippy
`-D warnings` limpo. Novos: `outline_read_skips_declaration_bodies` (frontend) e os exemplos de `parse_lexed_outline` e `load_lenient_outline`. Ajustados em
`tests/session.rs`: abrir, fechar ou mudar no disco um arquivo que a carga da biblioteca não
leu não a recarrega mais (as referências vêm da carga larga, que recarrega). Oráculo de
paridade (references/implementation/definition, completion top-5) não foi rodado de novo;
os testes de `references`, `rename`, `navigation`, `completion` e `typed_incremental` passam.

**O que falta** (por ordem de ganho): o motor do processo residente refazer só a biblioteca
editada quando nenhuma assinatura de API muda (hoje refaz carga, *outline* e inicializadores
do programa inteiro a cada tecla: é a perda de 2–16× por tecla); as buscas de `references`
num processo filho (o pico e o repouso de memória do frontend vêm delas) e um índice de nomes
referenciados mantido pelo residente; `completion` fora de um corpo sem recarregar a
biblioteca; o tempo de parede da primeira análise.

## 31/10/2026, noite (2): JS de produção — `List.filled` do JSON, `join`, resto por divisor não literal e o `[]=` dos tipos de extensão

Detalhes em `docs/JS-PRODUCAO-VELOCIDADE.md` §13. Sobre o `ecbaceb7` (as
formas da §12, do outro agente de JS, integradas antes desta rodada).
Validado localmente no macOS: corpus JS 277/277 em desenvolvimento e em
`-O2`, 260/260 em `-O3` (no binário de antes de o resto deixar os divisores
literais de fora, o que só restringe a forma), 256/256 em `-O4` (os
excluídos de `production_levels.tsv`); `production_forms` contra a VM em
`-O1` a `-O4` com as chaves ligadas e desligadas; testes de `emit_js`,
`emit_js_production` e `world` com os ignorados (190, 0 falhas); `clippy -D
warnings`; `rustfmt`. Máquina **muito** carregada (carga 37–236, seis
agentes): os tempos são medianas de execuções alternadas na mesma sessão,
com o `uptime` anotado no documento.

**Causas achadas** (perfis `node --cpu-prof` do `-O4` e edições à mão no
arquivo gerado, uma por vez):
- `jsonEncode`: o `List.filled` de cada objeto JSON custava 12,5% porque o
  `JSArray.of` grava a rti (propriedade de símbolo) **antes** do
  `Array.prototype.fill`, e o `fill` do V8 sai do caminho rápido num
  *array* cujo mapa mudou (à mão: 40,9 → 32,4 µs/op só trocando a ordem);
- `list_methods`: `JSArray.join` passava por um `List<String>.filled` com a
  checagem de imutabilidade a cada escrita;
- `map_str_int`: `keys[(i * 7) % keys.length]`, o resto por divisor não
  literal ia pelo símbolo do `JSNumber`.

**Formas novas** (só produção, cada uma com chave `=0`):
`DARTFORGE_JSPROD_SHORT_FILL` (a da §12) passa a gravar a rti depois do
`fill` nas listas longas; `DARTFORGE_JSPROD_ARRAY_JOIN`;
`DARTFORGE_JSPROD_NONNEGATIVE_REMAINDER` (`x % y` com `x` nunca negativo
pelo passe da §12 e `y` um `int` não literal é o `%` do JavaScript).

**Defeito do mundo corrigido:** o `operator []=` de um tipo de extensão
usado só por escrita composta (`k[2] += 10`) não era mantido (o nó resolve
para a leitura); vivia por acaso enquanto algum outro `[]=` do programa
mantinha o seletor. O `join` novo tirou o último do SDK no corpus 231.

**Resultado** (Node, `-O4`, ms, `main` × nosso × `dart2js -O4`):
`map_str_int` 15,4 × **13,4** × 15,6; `list_methods` (programa de um
*kernel* só) 36,7–39,0 × 36,2 do `dart2js`; `collatz` 148,8 × 149,9 ×
149,9; `set_int` 15,0 × 16,2 × 15,4 (as mesmas células); `jsonEncode`
1,03–1,14× do `dart2js` (ruído contra o `main`). Tamanho: 16 programas de
`bench/` 701.603 → 697.384 (gz 240.699 → 239.191), **0,954× bruto e 1,0009×
gzip** do `dart2js -O4` (antes 1,007×); `limitless_ui` 3.350.093 →
3.350.051 (gz 947.503 → 947.491), `new_sali` 7.147.071 → 7.147.069 (gz
1.939.642 → 1.939.532): não crescem.

**Medidos e descartados:** o `IdentitySet` sobre um `IdentityMap`
(−1.084 bytes no `limitless_ui`, mas `set_int` 1,2× e `set_str` 2,3× mais
lentos que as células); `dart.str` de número sem o `toString` de extensão e
`JSArray.addAll` por índice (no ruído, +149 bytes no `limitless_ui`); a
receita do `is Map` em variável e `Array.isArray` antes do `is List` no
`writeJsonValue`; uma análise de locais nunca negativos mais geral
(*closures*, `switch`, `try`, resolução conferida), escrita em paralelo à
da §12 — ficou a da §12.

**O que continua:** `async_await` 1,2–1,4× (rti por chamada `async`); o
`_JsonStringifier` (o `dart2js` desvirtualiza `writeString` para
`_contents += s`); o gzip dos programas pequenos, 0,09% acima: o caminho
do `noSuchMethod` (`InvocationImpl`, `NoSuchMethodError`, `_checkAndCall`,
símbolos) que o `toJson` dinâmico do `_defaultToEncodable` mantém em todo
programa com `jsonEncode` (o `dart2js` escreve `object.toJson$0()` e deixa
o `TypeError` do JavaScript virar o erro) e o *pretty printer* do JSON
(`_JsonPrettyPrintMixin`), vivo mesmo sem `JsonEncoder.withIndent` — os
dois só saem com prova no mundo.

## 31/10/2026: `dartforge pub` com o solucionador PubGrub do Pub e os subcomandos do dia a dia; relator `expanded` do `dart test`

**Solucionador.** A busca com retrocesso (limite de 100000 decisões, que estourava no
`aggr`) foi trocada por um porte do `VersionSolver` do Pub 3.6.2 (PubGrub com aprendizado
de conflitos), incluindo as heurísticas que decidem *qual* solução sai: pacote com menos
versões primeiro, versão travada preferida, estável mais nova primeiro (mais antiga no
`downgrade`), fatos na ordem das dependências do pubspec e da listagem JSON do servidor
(árvore com ordem de inserção para YAML e JSON), faixas de dependência agrupadas como no
`PackageLister`. Versões e restrições seguem o `pub_semver` (build na ordem, `^` só
quando escrito, operações de conjunto exatas). Fonte de referência: Pub
`e6704cc5` (o `pub_rev` da tag 3.6.2) e `pub_semver` `8cce9d00`, baixados como tarball
para o scratchpad (fora do repositório).

**Saídas e mensagens.** `pubspec.lock` e `package_config.json` no formato exato do Pub,
agora com cabeçalho e `generator: pub`; mensagens do Pub (`Resolving dependencies...`,
`+ > < ~ * !`, `(x available)`, resumo, desatualizados/descontinuados, `example/` resumido
com `./example`) e a explicação de falha (`Because ... version solving failed.`,
numerada). Erros e códigos de saída do Pub (65 em dados, 69 para pacote inexistente,
66 sem pubspec).

**Subcomandos novos:** `upgrade` (pacotes, `--unlock-transitive`, `--major-versions`,
`--tighten`), `downgrade`, `add` (`dev:`/`override:`, descritores YAML, restrições,
`--path`, `--git-*`, `--hosted-url`, `--sdk`), `remove`, `deps` (tree/compact/list,
`--json`, `--executables`) e `outdated` (tabela e `--json`, sem avisos de segurança).
O pubspec é editado como o `yaml_edit` faz (ordem, aspas, seção vazia removida).
Matriz completa de `pub` e `test` em `docs/PUB-E-TEST.md`.

**Casos antes recusados:** arquivo hospedado com links (`new_sali/backend`: o `eloquent
3.4.3` usa nomes longos GNU com magic `ustar`, que o crate `tar` não aplica; links
internos mantidos e externos ignorados como no `extractTarGz`), repositório git local
com caminho relativo, `resolution: local/external` (não são raiz, como no Pub) e
membros de `workspace` com glob (na 3.6.2 o glob é só um caminho literal). Uma versão
travada cujo pubspec mudou de versão (path ou pacote do SDK) volta a ser trocada, como no
`SystemCache.describe` do Pub.

**`dartforge test -r expanded`** passou a imprimir o formato do `ExpandedReporter`
(linhas `MM:SS +n ~n -n: ...`, `loading`, `[E]` com erro e pilha indentados, `Skip:`,
resumo, aviso de `--chain-stack-traces`, cores e `printPath` da configuração). Num
pacote de teste com aprovação, falha, `skip` e `print` em duas suítes a saída coincide
com a do `dart test -r expanded -j 1`, exceto o tempo decorrido (inclui a compilação) e
os quadros da pilha nativa.

**Evidência diferencial** (`bench/results/2026-10-31-pub-test/README.md`), contra
`dart pub`/`dart test` 3.6.2 com caches separados e vazios:
* `pub get` nos 51 projetos de `/Volumes/ssd_dock/MyProjectsDart` (os 18 de antes e 33
  cópias novas): **51/51 idênticos** sem lockfile (resolução do zero: `pubspec.lock` byte a
  byte, `package_config.json` fora o carimbo `generated`, `workspace_ref.json`, stdout e
  código de saída), incluindo `aggr` e `new_sali/backend`; com os lockfiles existentes,
  49/51 na primeira rodada; os 2 restantes (`aggr`, `pg8000`) eram versões travadas
  cujo pubspec mudou de versão e ficaram idênticos depois da correção (re-execução).
* 32 cenários de `add`/`remove`/`upgrade --major-versions`/`--tighten`/`downgrade`/`deps`
  e falhas: todos com mesmo pubspec, lockfile, stdout e código de saída; a única
  diferença é a lista de sugestões que o Pub imprime depois de uma falha de resolução
  (não implementada).
* 10 cenários de `outdated` (tabela, `--json`, `--no-dev-dependencies`, `--show-all
  --no-transitive`, `--prereleases`): idênticos.

**Continua aberto:** sugestões após falha, avisos de segurança, `cache`, `global`,
`publish`/`token`/`login`, `--precompile`; no `test`, o protocolo JSON do package:test,
os relatores `compact`/`github`, plataformas de navegador/node, coverage, watch e
seleção por linha.

## 31/10/2026, noite: `serve` sem `build_runner`, edição de template e formas de produção

Detalhes em `docs/DEV-VS-DDC.md` (rodada da tarde) e
`docs/JS-PRODUCAO-VELOCIDADE.md` §12. Commits `4713b145` (desenvolvimento)
e `4e4f74bb` (produção). Validado localmente no macOS: corpus JS 277/277 em
desenvolvimento e em `-O2`, 260/260 em `-O3`, 256/256 em `-O4` (os
excluídos de `production_levels.tsv`); testes de `emit_js` e
`emit_js_production` com os ignorados (182, 0 falhas); testes de `build` e
`dev` com os ignorados (o `corpus_builders_through_the_native_executor`
precisa de um compilador com `--features native`, pré-requisito do
ambiente); `clippy -D warnings`; `rustfmt`; `tests/language` 2756/2863,
sem regressões contra a v12.

**Desenvolvimento.**
- O `dartforge serve`/`dev` **sobe num checkout limpo, sem
  `build_runner`** nem a VM: o motor nativo já gerava tudo (no
  `limitless_ui`, 636 ações nativas em 2,5 s), mas a entrada relativa
  (`serve web/main.dart`) dava raiz de pacote vazia e o carregador não
  achava os `.template.dart` recém-gerados. `package_root` e a sessão
  trabalham com caminhos absolutos.
- Depois de qualquer edição de API, toda edição do `limitless_ui` refazia
  o estágio A do ngdart de um pacote inteiro: o `example` mora dentro do
  diretório do `limitless_ui` e as bibliotecas dele contavam como do
  pacote de fora. "Do pacote" agora é `lib/`, `web/` ou `test/`; o
  estágio B por `.dart` aceita uma biblioteca editada de outro pacote; o
  índice do ngdart dos pacotes editáveis nasce com a primeira rodada. Na
  sessão (relatório `total`, máquina carregada): corpo 0,85–2,1 →
  0,23–0,56 s; API 3,3–5,5 → 1,4–3,5 s; template 0,57–1,0 → 0,33–0,62 s.
- Servidor com *keep-alive* e `ETag` (`304` para o módulo que não mudou)
  em vez de `no-store` + `Connection: close`.

Contra o webdev (`scripts/compare-dev-ddc.mjs`; máquina **muito**
carregada por outros agentes, carga 70–210: parede 2–4× a da manhã nas
duas ferramentas, compare as razões e a CPU):

| medida | DartForge | webdev |
| --- | ---: | ---: |
| `limitless_ui`, checkout limpo → primeira página | **29,6 s** (CPU 5,8 s, 0,52 GB) | 212,5 s (CPU 147,7 s, 4,94 GB) |
| `new_sali`, checkout limpo → primeira página | **56,1 s** (CPU 9,4 s, 0,98 GB) | 383,3 s (CPU 210,9 s, 6,78 GB) |
| `limitless_ui`, edição de corpo → código novo | **2,39 s** (CPU 0,71) | 8,13 s (CPU 3,99) |
| `limitless_ui`, edição de API → código novo | **8,75 s** (CPU 2,48) | 51,75 s (CPU 47,90) |
| `limitless_ui`, edição de template → código novo | **3,31 s** (CPU 0,88) | 7,63 s (CPU 2,82) |

De manhã, sem essa carga, o template era 3,06 s nosso contra 2,16 s do
webdev; o início do zero, 166,6 s (`build_runner` + `serve`) contra
114,4 s. O início morno empatou em CPU (5,9 × 8,2 s; 9,4 × 9,6 s): o
nosso refaz a compilação a cada processo.

**Produção** (trabalho passado no meio da rodada ao agente de produção;
medição parcial na §12): `IdentitySet` em células
(`DARTFORGE_JSPROD_IDENTITY_SET_CELLS`; `Set<int>` 23,4 → 9,7 ms contra
15,3 do `dart2js -O4`), locais nunca negativos para `x >> k` e `x % k`
(`DARTFORGE_JSPROD_NONNEGATIVE_SHIFTS`; `collatz` 208 → 139 ms contra 136),
`==` de primitivo sem despacho (`DARTFORGE_JSPROD_EQUALS_PRIMITIVES`;
`list_methods` 47,0 → 37,1 contra 38,4), as *closures* do `await` uma vez
por invocação (`DARTFORGE_JSPROD_AWAIT_CALLBACKS`), `List.filled` curto
por laço (`DARTFORGE_JSPROD_SHORT_FILL`) e toda receita fechada de função
no prelúdio (`DARTFORGE_JSPROD_SINGLE_RECIPES`); estas três medidas só à
mão. O `async_await` entrou no `scripts/compare-js-production.py`: o
prelúdio dá ao `dart2js` um `self` com `scheduleImmediate` por *promise*
(o primitivo do nosso runtime); antes o agendador dele lançava e nada era
impresso. Fica 1,5× do `dart2js`; o JSON a codificar, 1,1–1,4×; o gzip dos
programas pequenos, 1,003× (16 programas) / 1,011× (os 15 de antes).

**Próximos passos (desenvolvimento):** o primeiro carregamento antes do
estágio e a recarga das unidades geradas (0,1–0,3 s numa edição de
template); a edição de corpo do `new_sali`, que regenera o fecho de quem
importa o *barrel* (216 arquivos, ~1 s), poderia cortar pelo hash de API
mais o texto inteiro das anotações.

## 31/10/2026, noite: wasm — módulos 28–42% menores e exceções Wasm nativas por padrão

Continuação da seção "wasm — suíte de linguagem, modos do runtime e desempenho" (abaixo):
a meta é o tamanho e a velocidade do dart2wasm sem WasmGC. "Antes" é o compilador de
`91aa751d` (compilado de novo para medir); os módulos são os de `compile-wasm` (produção,
sem a seção de nomes).

**Para onde iam os bytes.** O hello world tinha 2,19 MB: 2 783 funções, 2,22 MB de código e
228 KB de dados. Com `DARTFORGE_KEEP_SYMBOLS=1` (agora também no wasm: fica a seção de
nomes, sem o DWARF) e o `DARTFORGE_WHY`, quase tudo vinha de três raízes que nenhum programa
pedia:
- o `df.register.<biblioteca>` registrava **todo** `_dartforge*` do SDK como ajudante do
  runtime, inclusive os que só o código gerado chama direto (`_dartforgeLoadLibrary` puxava
  `Future`, `Timer` e `Duration`; `_dartforgeRebuildCopiedIndex`, os conjuntos);
- o `df.prepare_isolate` registrava as tabelas de todas as classes de valores do runtime
  (as 14 listas tipadas e os 3 SIMD), com os `setRange`/`_slowSetRange`/`skip`… de cada uma;
- o `Error.safeToString` de uma string é `json.encode` (VM e dart2wasm): o
  `NoSuchMethodError` e os erros de argumento — vivos em todo programa pelo `noSuchMethod`
  do runtime — mantinham o codificador JSON inteiro (e, por ele, o `Uint8List` e o
  `double.toString`).

O que mudou, medido passo a passo no hello (bytes do módulo):

| passo | hello |
|---|---:|
| antes (`91aa751d`) | 2 188 280 |
| só os ajudantes que o runtime procura pelo nome (`RUNTIME_HELPERS`; um teste compara com os nomes nas fontes do runtime) | 1 801 302 |
| tabelas das listas tipadas e SIMD só com um produtor vivo (`; df.ref-when` na poda da ligação) | 1 553 927 |
| contexto por global, área fixa, sem verificação de pilha nas folhas (velocidade) | 1 550 474 |
| runtime wasm32 como bitcode no LTO do módulo (`-Clinker-plugin-lto`) | 1 491 136 |
| exceções Wasm nativas (`--exceptions tables`) | 1 443 479 |
| `Error._stringToSafeString` escrito à mão no overlay wasm (mesmo texto do `json.encode`) | 1 258 284 |
| porto do `Function.apply` (correção, abaixo) | **1 258 409** |

Medido e descartado: `--lto-O3` (o mesmo binário que `-O2`), `-enable-merge-functions`
(−0,6%), `-O2` do `wasm-ld` (nada); `opt-level="s"` no runtime tira mais 60 KB (−3,9%), mas
otimiza para tamanho também o coletor e os nativos dos kernels; não medi a velocidade com a
máquina carregada e não entrou (o nativo já o tinha recusado, docs/PLANO-TAMANHO-DESEMPENHO.md). O `wasm-opt -Oz` do binaryen (o do SDK do
Dart) tiraria ~19%, mas inlina e muda os quadros que o orçamento da pilha do motor
(`wasm_frames.rs`) mede depois da ligação; não entrou no pipeline.

Tamanho de cada programa (bytes, e com `gzip -9`):

| programa | antes | depois, `check` | depois, padrão (`tables`) | dart2wasm |
|---|---:|---:|---:|---:|
| hello | 2 188 280 (554 229) | 1 292 425 (385 546) | **1 258 409** (378 291) | 47 001 (15 338) |
| árvores | 2 211 639 (560 277) | 1 344 610 (398 314) | **1 307 255** (391 125) | 48 482 (15 878) |
| JSON | 2 703 407 (689 232) | 2 033 074 (571 349) | **1 942 692** (555 802) | 113 505 (38 499) |
| strings | 2 279 570 (572 674) | 1 394 538 (408 198) | **1 353 205** (401 357) | 61 453 (19 861) |
| objetos temporários | 2 216 369 (561 258) | 1 349 315 (399 561) | **1 311 415** (392 405) | 48 597 (15 917) |
| coleções | 2 543 236 (619 271) | 1 663 858 (457 698) | **1 603 143** (445 967) | 67 496 (21 318) |

De 28% (JSON) a 42% (hello) menores; o dart2wasm continua 17–27× menor. O que sobra no
hello (1,26 MB): ~560 KB do SDK (o `noSuchMethod` raiz mantém `NoSuchMethodError`,
`_InvocationMirror` e o `LinkedHashMap`: `dart:core` 246 KB e `_compact_hash` 150 KB), ~400 KB
do runtime Rust (coletor, RTI, nativos, `hashbrown`, formatação de `double`, a tabela RTI
comprimida e o `miniz_oxide` que a lê) e 194 KB de dados.

**Velocidade.** Três mudanças no código gerado do wasm:
- o prólogo lia o contexto chamando `dartforge_context` em toda função; agora lê o endereço
  que o runtime publica em `dartforge_wasm_context` (`sdk_module::context_in_wasm_global`,
  uma carga invariante);
- a área de globais usa a tabela fixa da produção nativa (`GET_AREA_FIXED`), que o wasm não
  usava;
- só as funções que podem chamar Dart (`deepens_stack`) verificam a pilha e o orçamento da
  pilha do motor; as folhas cabem na reserva de 160 KB (`ENGINE_RESERVE`).

Mais o runtime no LTO e as exceções nativas (abaixo). Medida: Node 24, três rodadas
alternadas dos quatro módulos (o "antes" compilado de novo de `91aa751d`, o `check`, o padrão
e o dart2wasm), **tempo de CPU da thread principal** (`performance.now` trocado por
`process.threadCpuUsage`, `--import` de um módulo de 10 linhas), em ms: a **mediana** das
iterações quentes e, entre parênteses, o mínimo. `uptime` ao fim: carga 117 / 114 / 83 (seis
agentes na máquina de 10 núcleos); mesmo em CPU, diferenças abaixo de ~10–15% são ruído.

| kernel | antes | `check` | padrão (`tables`) | dart2wasm |
|---|---:|---:|---:|---:|
| árvores binárias | 42,5 (30,0) | 40,1 (26,5) | 37,0 (27,9) | 31,7 (15,7) |
| JSON decode pequeno | 48,3 (38,9) | 45,7 (31,4) | 43,0 (34,4) | 24,5 (16,3) |
| JSON encode pequeno | 51,6 (43,3) | 44,6 (34,3) | 46,9 (39,0) | 30,0 (21,0) |
| JSON decode grande | 165,9 (157,7) | 174,5 (144,1) | 167,4 (146,4) | 90,9 (73,8) |
| JSON encode grande | 176,6 (154,5) | 155,0 (151,8) | 162,7 (148,5) | 118,2 (104,1) |
| JSON `utf8` | 113,2 (102,8) | 107,0 (85,8) | 106,1 (92,4) | 52,8 (38,5) |
| strings build | 64,3 (51,5) | 60,0 (49,4) | 63,6 (45,8) | 37,5 (27,4) |
| strings hashes | 26,0 (16,5) | 23,6 (16,6) | 21,9 (16,8) | 19,7 (15,4) |
| objetos temporários (`points`) | 13,1 (9,8) | 15,4 (10,7) | 14,8 (10,0) | 4,8 (4,2) |
| crivo | 31,3 (22,3) | 20,8 (16,2) | 24,6 (17,7) | 14,0 (8,1) |
| `list.add` | 3,6 (2,3) | 4,2 (2,4) | 2,5 (2,3) | 21,0 (7,5) |
| `list[i]` | 3,7 (2,6) | 3,1 (2,6) | 2,7 (2,5) | 13,0 (7,8) |
| `sort` | 110,1 (91,2) | 108,8 (91,8) | 111,7 (95,7) | 234,1 (213,0) |
| mapa `int` | 74,7 (32,1) | 60,9 (38,7) | 61,5 (38,1) | 60,5 (34,7) |
| conjunto de strings | 25,4 (15,6) | 20,8 (14,7) | 21,2 (16,6) | 19,7 (11,4) |

Pela mediana, o padrão ganha 10–20% em árvores, JSON pequeno, crivo, mapa e conjunto de
strings; o resto fica dentro do ruído (os objetos temporários, 13% pior pela mediana, empatam
pelo mínimo). O dart2wasm continua 1,2–3× mais rápido em alocação, árvores, JSON e strings
(e o DartForge 2–8× em `list.add`, `list[i]` e `sort`). O perfil (`node --cpu-prof` com a seção de
nomes) mostra onde está o resto: nas árvores, 74% no código gerado de `tree`/`count` e 17% no
coletor; no JSON, ~25% no coletor (`mark_pending` sozinho 14%); no crivo, o
`List<bool>.filled` não é desvirtualizado (o dart2wasm sabe pelo TFA que é um `_List` e
lê/grava direto). Ajustar o coletor pelas opções (`DARTFORGE_GC_GROWTH`,
`DARTFORGE_GC_YOUNG_KB`) não mudou nada acima do ruído.

**`--exceptions tables` no wasm: exceções Wasm nativas (`exnref`).** O modo por tabelas do
nativo (`invoke`/`landingpad`, `optimize/tables.rs`) agora existe no wasm e é o padrão do
`compile-wasm` (`target::WASM_TABLES_BY_DEFAULT`; `--exceptions check` volta ao outro):
- cada pouso é um funclet catch-all (`catchswitch`/`catchpad [ptr null]`/`catchret`,
  `llvm/mod.rs`, `wasm_catch_all`), que o `WasmEHPrepare` do LLVM baixa sem chamar a
  personalidade; o código do handler fica depois do `catchret`, fora do funclet. O pouso
  restaura o topo da pilha-sombra (como no nativo) e também o orçamento da pilha do motor,
  que os quadros atravessados não devolveram;
- `@df.throw` é `llvm.wasm.throw(0, null)` com a tag C++: a exceção continua sendo a
  pendente do runtime. A tag `__cpp_exception` vem de um objeto de 99 bytes que o
  `wasm-ld` recebe (`linker_wasm::EXCEPTION_TAG_OBJECT`; um teste confere que é o que o
  Clang monta): definida num `asm` de bitcode, o LTO a tomava por função;
- objetos com `-fwasm-exceptions -mllvm -wasm-use-legacy-eh=false` e o LTO do `wasm-ld` com
  `+exception-handling`, `-wasm-enable-eh`, `-exception-model=wasm` e
  `-wasm-use-legacy-eh=false`: só `try_table`/`throw`/`throw_ref`, que o
  `dartforge-wasm-gate` aceita (ele recusa o EH legado);
- os portos (`df.port.*`) e a entrada de callback FFI pousam do mesmo jeito; o `resume` de
  uma limpeza vira um novo `throw`. O `Function.apply` do wasm chamava a entrada uniforme
  da closure direto (a chamada indireta do wasm confere o tipo exato, e o porto de aridade 3
  não serve); com exceções nativas, uma closure que lança desenrolava os quadros do runtime
  (`corpus/wasm_interop/05`: o `with_roots` do `dartforge_js_call` achava o topo errado).
  Agora há um porto próprio, `@df.port.apply` (índice 16 de `DART_PORTS`).
- Defeito achado e corrigido no caminho: no modo por tabelas o prólogo não debitava o quadro
  do orçamento do motor (o `return` antecipado de `emit_stack_check`), e uma recursão estourava
  a pilha do V8/wasmtime em vez de lançar `StackOverflowError`.
- Também no nativo: o passo de tabelas troca por 0 a verificação de pendência onde nada pode
  estar pendente (depois de uma chamada Dart que só desenrola, quando o `?.` juntou o
  resultado antes da verificação) — no `Node.count` das árvores sobravam duas por chamada.

O Node 24 (V8 13.6) roda `exnref` sem flag; o `dartforge-wasm-run` (wasmtime 50) também.
`tables` contra `check`: o módulo fica 2,6–4,5% menor; a velocidade empata dentro do ruído
desta medição (pelo mínimo, `check` à frente em árvores e JSON pequeno, `tables` em strings
e JSON grande; pela mediana, o contrário em árvores). Fica o `tables`, pelo tamanho e
porque o caminho normal de toda chamada Dart deixa de ler o estado pendente; a comparação
de velocidade precisa ser refeita com a máquina livre.

**Correção.** Harness `--wasm` (corpus/native, hoje 163 programas): **163/163** no padrão
(`tables`) e **163/163** com `DARTFORGE_EXCEPTIONS=check` (com `--exec-limit 60`: com o limite
de 5 s, 27 programas estouravam o tempo pela carga da máquina, não por erro). Os 13 de
`corpus/wasm_interop` saem iguais nos dois modos no Node. Como os ajudantes e o passo de
tabelas também mudaram no nativo: harness `--native --optimize` **163/163** e, com
`DARTFORGE_EXCEPTIONS=tables`, **163/163**. Testes do `emit_native` tocados (poda, ajudantes,
tag, verificação limpa, ligação wasm) e `clippy -D warnings` de `emit_native`, `runtime` e `cli`
passam. Suíte
`tests/language` no wasm: com a carga da máquina (50–130) e 2 jobs, a suíte inteira levaria
~11 h (4 programas por minuto), então rodei os **555 programas de maior risco** para estas
mudanças — `exception/`, `async*/`, `await/`, `no_such_method/`, `function/`, `call/`,
`closure/`, `deferred/`, `string/`, `unsorted/` e todo nome com `throw`, `rethrow`, `finally`,
`try`, `catch`, `stack`, `apply` ou `error` (resultado `wasm-ae98-risk`). Contra o
`wasm-final` de 31/10 nos mesmos programas: **537/555 com paridade antes e depois, nenhuma
regressão** (e nenhuma correção nova). A suíte inteira fica para uma janela com a máquina
livre.

## 31/10/2026: verificador de tipos contra o `dart analyze`, `compareTo` do SDK e CI do macOS

**Falsos positivos do verificador.** Medida: `dartforge analyze --all` contra o
`dart analyze` 3.6.2, nos 2.863 programas válidos da suíte `tests/language` (as chaves de
`results/vm.json`), em lotes de 100 arquivos. Conta-se cada erro nosso que o oficial não
relata na mesma linha. Ficam de fora os testes de `macros/`, que travam o `dart analyze`.
- **Antes:** 231 erros a mais, em 70 arquivos. **Depois da primeira rodada:** 107, em 29 arquivos
  (152 depois da versão de linguagem e da inferência horizontal; o resto veio das correções de
  fluxo abaixo). **Agora:** 20, em 7 arquivos (segunda rodada, no fim da seção).
- **Versão de linguagem.** O verificador aplicava os `inference-update-1/2/3` (2.18, 3.2 e 3.4)
  em qualquer biblioteca. Agora são `Feature`s com `enabledIn` (`crates/frontend/src/features.rs`)
  e respeitam o `// @dart=x.y`: em 3.3, `??`, `??=`, `?:` e a expressão `switch` têm o tipo UP
  dos ramos, não o do contexto. Era o maior grupo (`*_disabled_test`).
- **Inferência horizontal como no analisador** (`_FunctionLiteralDependencies`):
  - todo literal de função vai depois dos outros argumentos, mesmo em chamada não genérica.
    A captura de escrita dele só vale depois da chamada;
  - um literal só depende das variáveis que aparecem nos tipos formais dos seus parâmetros
    **sem tipo**: `fold(null, (int? x, y) => …)` dá `int?`;
  - o estágio de um literal é 1 + o maior estágio de quem fornece aquelas variáveis, e os
    literais de um ciclo dividem um estágio;
  - o primeiro estágio adiado usa os tipos preliminares de antes dos argumentos, e cada estágio
    seguinte os escolhe de novo uma vez só.
- **`x ??= e` com `x` local:** o contexto de `e` é o tipo promovido. A coerção de `call` segue o
  tipo de escrita.
- **Análise de fluxo:**
  - as condições de uma variável de condição ficam por versão da gravação. Antes, `b = true` num
    ramo que lança apagava a de `b = x != null`, que a junção mantém;
  - ler uma variável de condição é uma condição (`bool b2 = b1;`);
  - `c ? a : b` com um ramo de condição junta as condições dos ramos
    (`bool b = x is int ? true : false;`);
  - uma expressão de tipo *bottom* (`T extends Never`, `T & Never`) encerra o fluxo, como
    `Never`;
  - todos os casos de um `switch` promovem o escrutínio resolvido pelo primeiro: depois de
    `case _ when f(c = c2)`, `case _ as int` não promove o `c._o` do `c` novo.
- **Outros:**
  - o operando de `throw` recebe o contexto `Object`;
  - `dart:core` importado com prefixo exporta `dynamic` e `Never` (`mycore.dynamic d`).
- **Testes** (`crates/types/tests/bodies.rs`, cada um falha sem a sua correção):
  `inference_updates_follow_the_language_version`, `horizontal_inference_stages`,
  `if_null_assignment_context_of_a_promoted_local`,
  `condition_variable_survives_a_write_on_a_branch_that_throws`,
  `condition_variable_aliases_and_bottom_type_parameters`,
  `conditional_expression_is_a_condition` e
  `switch_cases_promote_the_first_scrutinee_reference`. Depois de todas: corpus JS 277/277 (dev e
  prod), nativo 163/163 (dev e `-O`) e suíte de linguagem 2756/2863, sem programa ganho nem
  perdido.
- **Antes, no mesmo dia:**
  - a promoção do campo privado de representação de *extension type* (`7a9cafdb`);
  - o portão de `use_of_void_result` nos quatro compiladores (`30a7e77d`,
    `codes::REJECTING_CODES`).
- **O que sobrava (107):** análise de fluxo (`try/finally`, junções, padrões, `== null`), UP
  com interseções, `noSuchMethod`, *wildcards*, `variance` e `records/type_inference*`.

**Segunda rodada: 107 → 20 erros a mais** (29 → 7 arquivos), pelo `_fe_analyzer_shared` 76.0.0
(o do 3.6.2, no pub-cache). Commits `f0671f47` (107 → 82), `e3ce8928` (82 → 77) e `f15d9242`
(77 → 20). Validação no fim: testes de `types`, `analysis` e `frontend`, clippy limpo, corpus JS
277/277 (dev e prod) e nativo 163/163 com o CLI `--features native,jit`.
- **Promoção de campos pelos nós SSA:**
  - a junção de duas versões de gravação de uma variável leva as propriedades lidas nos dois
    caminhos para a versão nova, com os modelos juntados (`SsaNode._join`). A gravação não
    apaga mais as propriedades da versão antiga (`forget_fields_of` saiu);
  - `try/finally` segue o `attachFinally`: as promoções do `finally` se apoiam nas do `try`,
    também nas propriedades de uma variável gravada no `try` (`_applyPropertyPromotions`);
  - o subpadrão de propriedade de um padrão de objeto divide o nó SSA de `x._f`: promove a
    propriedade e vê a promoção dela (`pushPropertySubpattern`);
  - a cascata sobre um alvo que não é referência guarda o temporário como referência da
    cascata inteira (`(c.d.._e!)._e`).
- **Testes de nulo** (`tryMarkNonNullable`, `_equalityCheck`): a referência de uma propriedade
  ou de `this` tem o tipo **não promovido**. `c._f != null` depois de `c._f is int` lê `Object`,
  e `c._f == null` depois de `c._f is Null` deixa os dois ramos alcançáveis. Dois operandos
  `Null` são certamente iguais. `x is T` com qualquer tipo *bottom* nunca é verdadeiro.
  Em contexto de valor, `is` e `==` não mexem no estado: `f is Function` como argumento não
  vira tipo de interesse.
- **Padrões e `switch`:**
  - o `case` com rótulo parte da junção conservadora do que os **casos** gravam, sem o
    escrutínio (`switch (x = 0)`);
  - `promoteForPattern` junta ao "não casou" tanto o tipo errado quanto o "pode falhar mesmo
    assim" (`case []` contra `List<Object?>?`);
  - `var (b) = x != null;` guarda a condição em `b`;
  - o fluxo guarda quando morreu e o ponto de divisão do padrão de topo de um `case`
    (`Flow::dead_at`/`split`). A junção favorece o caminho localmente alcançável e junta dois
    caminhos localmente mortos (`FlowModel.join`).
- **Inferência:**
  - os tipos inferidos são demovidos depois das verificações (`_demoteTypes`, `ops::demote`):
    `X & B` vira `X` nos argumentos de tipo de chamadas, literais, *tear-offs* e extensões
    (`S & int` liga `T` a `S`);
  - `var y = x0;` com `x0: T & num` declara `T` e promove a `T & num`; um inicializador `late`
    promove como uma gravação (`late num? x = 3;` é `num`);
  - o literal de *record* infere os campos na ordem do fonte e usa os esquemas do contexto
    como estão (`(List<_>,)` deixa `[1, 2]` como `List<int>`);
  - um literal genérico não recebe nada de um contexto com outro número de parâmetros de tipo;
  - um parâmetro com variância explícita (`in`/`out`, mesmo sem o experimento) não fixa o
    tipo preliminar do contexto;
  - um método de `Object` sobre `dynamic` só é estático quando os argumentos servem
    (`n.noSuchMethod()` é chamada dinâmica).
- **Testes novos** (`crates/types/tests/bodies.rs`, no modo do analisador,
  `analyzer_diagnostics_of`; cada um falha sem a correção):
  `field_promotion_through_joins_finally_patterns_and_cascades`,
  `null_checks_of_property_references_and_bottom_type_tests`,
  `pattern_flow_split_points_and_switch_labels`,
  `demotion_records_generic_literals_and_dynamic_object_methods` e
  `explicit_variance_does_not_fix_the_context_type`.
- **Sem falso negativo novo:** os erros dos 1.460 programas de erro da suíte (os que não estão
  em `results/vm.json`) são os mesmos antes e depois de cada etapa. Placar do analisador
  23.001/23.012 (FP 1, o mesmo de antes: `pattern_assignment_error_test.dart:36`, o contexto de
  `(x) = e` com `x` promovido).
- **O que sobra (20):**
  - `augmentation_libraries` e `test_runner` (12): sem o experimento `macros`, o parser do
    3.6.2 reconhece as declarações `augment` (mas não o `library augment`) e não reclama do
    `import augment`; o nosso lê `augment String get foo` como uma variável `String`;
  - `least_upper_bound_greatest_closure_2_test` (2): com `X1 extends FutureOr<X1>`, o
    `dart analyze` 3.6.2 não dá diagnóstico nenhum na função (a subtipagem recursiva estoura);
  - *wildcards* fora do modo *wildcard* (6): com `_` declarado duas vezes no mesmo bloco, o
    analisador liga o nome à primeira declaração; nós, à última.

**`compareTo` do SDK na produção JS.** `JSNumber.compareTo(num b)` e `JSString.compareTo`
sobrescrevem `Comparable<T>.compareTo(T)`. A compilação própria do SDK só verificava parâmetros
que mencionam os parâmetros de tipo da classe, então `[1, 'a'].sort()` ordenava em vez de lançar
`TypeError`. A verificação passa a cobrir as sobrescritas, como já fazia nas classes do programa.
`-O3` e `-O4` a tiram, como o `dart2js --omit-implicit-checks`. Programa novo:
`corpus/js/277_covariancia_de_sobrescritas_do_sdk.dart` (excluído de `-O3` em
`production_levels.tsv`). O corpus JS fica em 277/277 em dev, `-O1` e `-O2`; em `-O3`, 260/260; em
`-O4`, 256/256.

**CI do macOS com a `libLLVM-22.dylib` pré-ligada** (`deedea66`):
- a pré-ligação leva 6 min e o *build* 10 min. O job chegou aos testes do JIT em 42 min; antes
  passava de 3 h;
- falhou com SIGBUS nos testes da biblioteca do `dartforge-jit`. Aqui reproduziu 1 vez em 29
  (SIGSEGV), sempre com testes em paralelo;
- o PC da falha é código do JIT já desmapeado, dentro do primeiro `run_reloadable_main` de
  `callback_lifetime_tests`, enquanto outras sessões eram criadas e destruídas. É uma corrida de
  tempo de vida entre sessões. Corrigido no mesmo dia: eram as tabelas de FFI do runtime,
  globais do processo (seção "SIGBUS/SIGSEGV intermitente nos testes do JIT" abaixo).

**Validação local** (`v8`, HEAD `17f8a7e6`):
- nativo 163/163 em dev, `-O`, *gc-stress* e wasm;
- JS dev e prod 276/276; suíte de linguagem 2756/2863;
- `cargo test --release --workspace`, *doctests* do `types` e clippy limpos.

Com as mudanças de tipos: suíte de linguagem 2756/2863 (só mudaram dois *hashes*), testes de
`types`, `frontend` e `analysis` limpos, corpus nativo 163/163 e JS 277/277.

**Defeito do *differential*.** As falhas em massa ("returned 0 but did not write") vinham de um
`CARGO_TARGET_DIR` relativo: ele vira o diretório de trabalho, e os compiladores, chamados com
outro diretório atual, gravavam em outro lugar. Agora o caminho fica absoluto. Ainda aberto: num
diretório de trabalho novo (cache frio), o oráculo DDC dos programas com divergência declarada
sai vazio, embora o mesmo `main.mjs` rode certo à mão. Com o `target/differential` de sempre,
277/277.

## 31/10/2026: JS de produção contra o `dart2js -O4` e o worker de desenvolvimento

Detalhes em `docs/JS-PRODUCAO-VELOCIDADE.md` §11 e `docs/DEV-VS-DDC.md`
(rodada de 31/10). Commits `506398e7`, `e99e4dd1` e `6fb7115d`, validados
localmente: corpus JS 275/275 em desenvolvimento e em `-O2`, 255/255 em
`-O4`; testes de `emit_js` e `emit_js_production` com os ignorados; clippy;
`tests/language` 2756/2863, sem regressões.

**Velocidade do JS de produção** (Node, `-O4`, ms; script novo
`scripts/compare-js-production.py`). As maiores diferenças eram constantes
canonizadas pelo runtime a cada avaliação (`utf8.encode`, `jsonEncode`, todo
`const` em função), `for`-in pelo `JsIterator` do runtime, `_current as E`
avaliando a rti, `is C` pela rti e o `sort` em Dart:

| *kernel* | antes | agora | `dart2js -O4` |
| --- | ---: | ---: | ---: |
| `list_sort` | 393,0 | **176,4** | 208,9 |
| `map_iterate` | 61,4 | **28,2** | 27,5 |
| `list_methods` | 83,5 | **47,6** | 38,5 |
| `map_object_key` | 68,3 | **45,7** | 44,2 |
| `utf8 short_round_trip` | 29,0 | **12,5** | 12,0 |
| `collections/map` | 46,9 | **36,1** | 45,9 |
| `json/encode_medium` | 45,5 | **39,9** | 33,3 |
| `Set<int>` / `Map<String,int>` | 21,2 / 15,6 | 18,9 / 14,3 | 14,0 / 11,9 |
| `json_encode/*_json` (µs/op) | 32,5–58,6 | ≈ | 23,2–45,1 |

**Tamanho** (`-O4`, bruto / gzip): `limitless_ui` 3,331 / 0,942 → 3,343 /
0,946 MB (0,903× / 0,963× do `dart2js -O4`); `new_sali` 7,087 / 1,924 →
7,131 / 1,936 MB (0,937× / 0,959×); corpus +0,45% (gz +0,32%). Os 15
programas de `bench/` ficam em 0,963× bruto, mas 1,009× no gzip (o piso do
runtime e o `convert`).

**Worker de desenvolvimento** (checkout limpo, sem `.dart_tool` nem
gerados):
- o `dartforge serve` **não sobe** num checkout limpo de nenhum dos dois
  projetos (`could not read …/app_component.template.dart`), nem com
  `DARTFORGE_BUILD_DART`; precisa de um `build_runner build` antes;
- início do zero: `limitless_ui` 166,6 s (`build_runner` 156,4 s +
  10,2 s) contra 114,4 s do webdev; `new_sali` 143,3 s contra 120,7 s. Com
  os gerados prontos: 10,2 / 14,1 s contra 10,7 / 10,6 s do webdev morno;
- edição → código novo na página, `limitless_ui`: corpo 1,74 × 3,70 s, API
  5,5 × 18,0 s (3,0 × 41 s de CPU), template 3,06 × **2,16 s**; `new_sali`:
  corpo 3,0 s, API 5,1 s, template 2,8 s; o webdev nunca mostra o código
  novo (recarga em `/login` dá 404);
- memória: 0,5–1,2 GB num processo contra 3,9–7,9 GB em 19 processos.

**Próximos passos:**
1. Dev: a sessão usar as saídas do `dartforge build --dart` (16,6 s no
   `limitless_ui` contra 2 min 9 s do `build_runner`) para subir do
   checkout limpo; a edição de template do `limitless_ui`.
2. Produção: `IdentitySet` em células (`Set<int>` 1,35×), o
   `_JsonStringifier` (1,2–1,4×), `collatz` (faixa), e o gzip dos
   programas pequenos.
3. Defeito achado: `num.compareTo` de uma `String` não lança no nosso JS.

## Ordem de trabalho a partir de 30/10/2026

O proprietário pediu para seguir sem parar, nesta ordem, com commit, push e o CI conferido
pelo `gh` a cada lote:
1. **CI verde.** Estava vermelho desde pelo menos 27/10 (detalhes abaixo). Conferir o
   resultado do lote de 30/10 e corrigir o que sobrar.
2. **ARC de verdade:** o RSS do servidor HTTP no ARC real e a elisão das cópias de
   campos `final`. As duas regressões do WIP foram corrigidas em 30/10 (abaixo).
3. **Modo de mapas de pilha:** agora liga e passa no macOS, mas perde para o A1 (seção de
   30/10 abaixo). Melhorar só com medida: a forma `"deopt"` no LTO do `lld`.
4. **Fusão de funções duplicadas no HIR** (seção de 30/10 abaixo), para o nativo, o JS e o
   wasm.

## 31/10/2026: `dartforge test` com `dart_test.yaml` e anotações; `pub get` com workspaces e git

**`dartforge test` (`f1a1b376`)** deixou de recusar `dart_test.yaml` e anotações de
biblioteca. Um planejador compilado nativamente com o próprio `test_core` do pacote
(cache em `.dart_tool/dartforge/test-planner/`) lê a linha de comando e os
`dart_test.yaml` global e do pacote com a `Configuration` do package:test, descobre
as suítes como o `Runner`/`Loader` (`paths`, `filename`, `presets`, `include`,
`on_os`, `on_platform`, `tags` por tag, `timeout`, `retry`, `skip`, `test_on`,
`concurrency`) e resolve a configuração de cada suíte. As anotações `@TestOn`,
`@Timeout`, `@Skip`, `@OnPlatform`, `@Tags` e `@Retry` são lidas da AST do DartForge
com as regras do `parseMetadata` (inclusive prefixos de import e a leitura não
resolvida de `new a.b()`); seletores, tags e o `Metadata` ficam com as classes do
package:test. As suítes rodam em paralelo até a `concurrency`. Só a plataforma `vm`
(reportada com o compilador `exe`); browser/node, coverage, debug, `--fail-fast`,
file reporters, seleção por linha/coluna e watch continuam recusados explicitamente.
Verificação: 22 execuções diferenciais contra `dart test` 3.6.2 (package:test 1.25.7
e 1.26.3) com resultados por teste e códigos de saída iguais; 55 casos do
`parseMetadata` oficial como oráculo de teste unitário; teste de integração
`test_runner_cli` (ignorado, 7 execuções diferenciais) passando. O formato do
relator `expanded` continua sendo o do DartForge, não o do `dart test`. A primeira
execução por configuração de pacotes compila o planejador (minutos).

**`dartforge pub get` (`972f3b26`)**: workspaces do Pub 3.6 (raiz achada como no
Pub, resolução conjunta com dev deps e overrides de todos os membros,
`workspace_ref.json`, remoção dos arquivos soltos dos membros), dependências git
(espelho `git/cache/<nome>-<sha1(url)>`, checkout por commit com
`.git/pub-packages`, commit travado preservado), `example/` resolvido por padrão,
`--dry-run`, `--enforce-lockfile`, `-C` e `--no-precompile`. A saída agora coincide
com o texto do Pub: caminhos relativos no lockfile e na configuração, aspas e ordem
de chaves do Pub, precedência `direct main` > `direct dev` > `direct overridden`
(o teste antigo esperava o contrário; conferido com o Pub real), lockfile
equivalente não é reescrito. Verificação: em 18 projetos reais do proprietário
(cópias só dos pubspecs, caches separados), 16 deram lockfile (fora o cabeçalho) e
`package_config.json` idênticos aos do `dart pub get`, incluindo os workspaces
`angular`, `playwright` e `playwright-connect` e o git de `pdfbox_dart`. Falham e
ficam abertos: `aggr` (a busca com retrocesso estoura o limite de 100000 decisões;
o Pub usa solucionador com aprendizado de conflitos) e `new_sali/backend` (arquivo
hospedado com link, recusado pelo extrator). Continuam recusados: repositório git
local com caminho relativo, globs em `workspace`, `resolution` diferente de
`workspace`, `--precompile`, SDK Flutter. Evidências:
`bench/results/2026-10-31-test-config-pub-workspace-git/README.md`.

## 31/10/2026: SIGBUS/SIGSEGV intermitente nos testes do JIT no macOS

**Defeito.** `cargo test --release -p dartforge-jit -- --include-ignored` caía às vezes no
macOS arm64: SIGBUS no CI, SIGSEGV local (1 em ~29), com o PC num endereço já desmapeado
chamado de código do JIT. O relatório local é do binário com o `libLLVM-22.dylib`. A
chamada que caiu é a **segunda** `run_reloadable_main` de
`captured_closure_survives_replacement_before_callback_publication` (desmontagem do
binário do relatório), não a primeira. Os registradores batem com a chamada uniforme de
uma closure `asFunction` (`x0` = closure, `x1` = argumentos na pilha).

**Causa.** As tabelas FFI do runtime (`crates/runtime/src/ffi.rs`: chave da assinatura →
entrada `df.ffi.<chave>$ent`, classe → letra C, structs/unions) eram globais do processo,
e a primeira gravação vencia (`or_insert`). Isso só vale com um programa por processo
(AOT). No JIT, a dylib do SDK da fonte é carregada uma vez e serve todas as sessões. Todo
programa com o SDK registra 18 chaves (`v_p`, `l_`, …), e os testes de
`migration::tests_mod::emitted_*` também. Quando um deles preparava o isolate antes do
teste de callback, a tabela guardava o endereço da sessão de migração. Essa sessão era
descartada (código desmapeado), e o `store(...)` (`v_p`) do teste de callback chamava
código morto. O defeito não é do LLVM em dylib nem das owners de callback: a ordem dos
testes decide. Com 3 núcleos no CI, a migração termina antes da segunda execução do
teste de callback com frequência. Na máquina local, o teste de callback quase sempre
registrava primeiro.

**Correção.** As três tabelas passaram a ser do isolate (`thread_local!`), e a
registração mais recente substitui a anterior. `df.prepare_isolate` as preenche na
entrada e em cada isolate criado, e elas só são lidas em threads Dart. Assim, cada
programa (sessão ou geração) usa as próprias entradas e os próprios ids. No AOT, com um
programa só, o comportamento não muda. Não houve serialização nem teste pulado.

**Evidência.**
- Reprodução determinística antes da correção: migração antes do teste de callback
  (`--test-threads=1 --exact <os dois> -Z unstable-options --shuffle-seed 1`, com
  `RUSTC_BOOTSTRAP=1`): SIGBUS em 5/5 com LLVM em dylib e 3/3 com LLVM estático.
- Teste novo `ffi::call_trampoline_lifetime_tests::later_session_does_not_call_a_dropped_sessions_ffi_entry`:
  três sessões com `Int64 Function(Int64)` (`llabs`), cada uma descartada antes da
  próxima. Antes: SIGSEGV em 3/3. Depois: passa.
- Depois da correção, com o binário de testes da lib (estático e dylib, cada um):
  - a ordem determinística: 5/5 com o teste do closure capturado e 3/3 com o outro teste
    de callback;
  - o teste novo isolado: 5/5;
  - a suíte completa com `--include-ignored`: 50/50;
  - a suíte embaralhada com 16 threads (`--shuffle-seed` 1..30): 30/30.
- `cargo test --release -p dartforge-jit -- --include-ignored` (lib e testes de
  integração, LLVM estático): 3/3, os 9 alvos verdes (lib com 46 testes).
- `cargo clippy --locked -p dartforge-jit -p dartforge-runtime --all-targets -- -D warnings`
  passa limpo. `cargo fmt --check` só acusa
  `crates/emit_js_production/tests/shared_rule_bodies.rs`, que já estava fora do formato
  antes desta mudança.

## 31/10/2026: wasm — suíte de linguagem, modos do runtime e desempenho

**Suíte `tests/language` (SDK 3.6.2) no alvo wasm.** O `scripts/run-language-suite.py`
ganhou os modos `wasm` (`compile-wasm` no perfil de produção, como `dart compile wasm`; o
módulo roda no `dartforge-wasm-run`) e `dart2wasm`. Como no `--wasm` do diferencial, um
programa também passa quando sai igual ao dart2wasm onde a web faz o dart2wasm divergir da
VM; um programa que a VM e o DartForge recusam compilar (254) conta à parte. 2 863
programas, contra o `vm.json`:

| | antes | depois |
|---|---:|---:|
| igual à VM | 2 680 | 2 722 |
| igual só ao dart2wasm | 7 | 7 |
| recusado pelos dois | 54 | 54 |
| **com paridade** | **2 741 (95,7%)** | **2 783 (97,2%)** |
| falhas | 122 | 80 |

42 programas passaram a sair certos, nenhum regrediu. Nos 183 programas que não saíam
iguais à VM antes, o dart2wasm sai igual à VM em 103; das 80 falhas que restam, 18 o
dart2wasm também erra (hashes, pilhas simbólicas, experimentos que a VM recusa). Não rodei
a suíte inteira no dart2wasm. Cada programa leva ~5 s para compilar (~10 s de CPU), e a
suíte inteira levou ~2 h 30 com 4 jobs na máquina compartilhada.

Corrigido no backend nativo, que serve ao nativo e ao wasm (harness nativo 162/162 nos
quatro modos, testes e clippy do `emit_native` e do `runtime`):
- `await` com o tipo de checagem do CFE (`Future<flatten(S)>`), inclusive extension type e
  variável com limite futuro; `async*` cancelado antes de começar; `void f() async`
  devolvendo o `Future`; `await for` em literal de coleção (`33a29f41`, `70f78a7c`);
- casts implícitos de `dynamic`: fonte do `for-in`, inicializador do `for`, `throw`,
  spread, argumento de chamada genérica e tipo de parâmetro no escopo de quem chama
  (`5663c5dc`); `Null`/`Never` na entrada dinâmica (`c5bbc4c7`);
- `??=` só escreve quando o valor lido é nulo (setter, `[]=`, `late final`) (`f1afa267`);
- covariância que a interface da classe acrescenta a um método herdado, o *forwarding
  stub* da VM (`71434a97`);
- tear-off: valor padrão que arranca a própria função (o compilador estourava a pilha) e
  `super.m == m` (`e895822e`);
- `null` testado pelo handle (`x is Never?`, `is Null`), `Never?` = `Null`, literal de tipo
  `Null` (`e7a00080`); listas da VM como `List<E>` no `runtimeType`, também em records
  (`659e4a80`);
- criação de aplicação de mixin nomeada sem `new`, `super(...)`, atribuição a estático de
  extensão, `C.new` como construtor sem nome (`0f398f5b`);
- só no wasm, como o dart2wasm: `new bool.hasEnvironment(...)` e afins fora de constante
  lançam `NoSuchMethodError` (`684c9d4e`).

O `async_patch.dart` mudou, então o manifesto revisado do SDK selecionado foi regenerado
(`736e2298`, só a unidade de patch do `dart:async`).

Continuam abertos (80, agrupados por causa provável): *forwarding stub* no tipo
reificado do tear-off e em mixins (10); checagem de parâmetro de tipo função na entrada
dinâmica e na instanciação (6); igualdade e identidade de tear-offs (instanciação,
construtor, `toString`) (6); RTI de tipos de função genéricos, substituição e argumentos de
tipo (11); construções ainda recusadas ou lentas para compilar (construtor `external`,
`loadLibrary` como valor, `TypeArguments`, fábrica redirecionante de extension type,
aplicação de mixin com superclasse abstrata; um estoura os 300 s) (8); programas que a VM
recusa ou que devem lançar e o DartForge aceita (11); pilhas simbólicas, hashes e identidade,
onde o dart2wasm também difere (7); `late` com inicializador recursivo em closure (2);
padrões (3); avulsos (16: `noSuchMethod`, `super` em mixin que estoura a pilha, `operator <`
dinâmico que não devolve `bool`, valor de chamada `void` sobrescrita, entre outros).

**Modos do runtime no wasm.** O `compile-wasm` recusa `--memory`, `--exceptions` e
`--roots` ("configures a native executable"); pelo ambiente (o diferencial compila no
processo e lê as variáveis), o `target.rs` recusa cada modo com a razão:

| modo | aceito no wasm? | corpus/native `--wasm` | por quê |
|---|---|---|---|
| tracing + `check` + pilha-sombra (padrão) | sim | 162/162 | — |
| ARC simples (`--memory arc`) | não | — | desligado no W3: o estado ARC do runtime wasm32 é a constante "desligado" (`heap.rs`, `ArcSlot`), para o otimizador tirar o ARC do módulo |
| ARC real (`DARTFORGE_ARC_PREPARED=1`) | não | — | exige o ARC |
| tokens virtuais (`DARTFORGE_ARC_TOKENS=virtual`) | não | — | exige o ARC preparado |
| `--exceptions tables` | não | — | "not available for WebAssembly yet": viraria EH Wasm (`exnref`), que o LLVM 22 gera e o perfil admite, mas a personalidade não foi escrita |
| `--roots maps` | não, por definição | — | o wasm não expõe a pilha do motor: não há como caminhar quadros nem ler mapas de pilha |
| `--stack-trace symbolic` | não, por definição | — | mesma razão |

Experimento (não comitado): tirei a recusa do `--memory arc`, a constante do `ArcSlot` no
wasm32 e a opção da lista do `compile-wasm`. O ARC simples compila e passa o corpus
(`--wasm` 162/162, com GC forçado 162/162). O ARC real e os tokens virtuais param na
preparação do SDK: `dartforge_native_DartForge_js_attach` (o interop JS do `dart:_js_helper`,
só do wasm) não tem contrato de posse ARC. E o custo de tirar a constante: o módulo
tracing cresce ~167 KB (+6,8%), sem mudar a velocidade.

**Desempenho no wasm** (Node 24/V8, três rodadas alternadas com carga ~3; mediana das
iterações quentes de cada kernel, em ms; memória: RSS máximo do Node / memória linear no
fim, em MB):

| kernel | tracing | ARC simples (experimento) | dart2wasm |
|---|---:|---:|---:|
| trees (árvores binárias) | 26,2 | 47,4 | 15,3 |
| JSON decode grande | 94,8 | 242,6 | 51,0 |
| JSON encode grande | 88,9 | 164,9 | 62,5 |
| JSON decode pequeno | 27,6 | 35,0 | 14,5 |
| strings build | 38,5 | 91,5 | 23,6 |
| strings hashes | 15,6 | 17,0 | 12,2 |
| objetos temporários (`points`) | 10,3 | 7,2 | 4,2 |
| crivo | 19,9 | 16,8 | 6,8 |
| `list.add` | 2,2 | 2,1 | 17,6 |
| `list[i]` | 2,5 | 2,5 | 6,8 |
| `sort` | 70,7 | 74,5 | 113,3 |
| mapa `int` | 22,0 | 53,9 | 24,2 |

| programa | tracing RSS / linear | ARC RSS / linear | dart2wasm RSS |
|---|---:|---:|---:|
| trees | 97 / 29 | 105 / 35 | 228 |
| JSON | 219 / 148 | 262 / 195 | 502 |
| strings | 130 / 67 | 140 / 79 | 334 |
| objetos temporários | 83 / 16 | 82 / 16 | 54 |
| coleções | 145 / 62 | 173 / 96 | 494 |

Tamanho: os módulos tracing têm 2,2–2,7 MB; os do dart2wasm, 49–113 KB (ele usa as strings
e os números do JavaScript e o GC do motor). O dart2wasm é 1,3–2,9× mais rápido em
alocação, árvores, JSON, strings e no crivo; o DartForge ganha em listas (`add` 8×, leitura
2,7×, `sort` 1,6×), empata no mapa e usa 2,3–3,4× menos memória (só nos objetos temporários
usa mais).

**Recomendação.** Para o wasm, o tracing (o padrão): é o único modo suportado, é o mais
rápido nas cargas de alocação e o ARC simples, no experimento, foi mais lento na maior parte
(até 2,6× no JSON), mais rápido só nos objetos temporários e no crivo, e gastou mais
memória linear. Levar o ARC para o wasm custa tamanho em
todo módulo (+6,8% sem a constante) e, para o ARC real, contratos de posse para os externs
do interop JS; não compensa hoje. O que mais rende no wasm é o tamanho do módulo e a
velocidade de alocação do tracing.

## 30/10/2026: modo de mapas no macOS, defeito do LLVM, CI e funções duplicadas

**O B1 (`--exceptions tables --roots maps`) passou a ligar e a passar no macOS.** Os
detalhes estão em `docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md` §14.13. Eram três defeitos:
- **Do LLVM (22.1.8, 23.1.4 e `main`):** o `MergeFunctions` copia `returned` para a chamada
  do thunk, e o `rewrite-statepoints-for-gc` o copia para o `gc.statepoint`, que devolve
  `token`. Provado com 5 linhas de IR no `opt` oficial. Origem: o PR #68475 (LLVM 18).
  - Issue: https://github.com/llvm/llvm-project/issues/237112
  - PR com a correção e testes (aprovado pelo proprietário, aberto do fork
    `insinfo/llvm-project`): https://github.com/llvm/llvm-project/pull/237129
  - **Contorno no DartForge:** sem `MergeFunctions` com raízes por mapas
    (`driver::merge_functions_at_link`).
- **Nosso:** os portos `df.port.*` e a entrada C do callback FFI faziam o LTO misturar
  pousos `{ ptr, i32 }` e `token` na mesma função. Agora há `noinline` nessas chamadas
  com mapas.
- **Nosso, desde `1e1bb1f3`:** as sondas de RTI e o `df.string_units_equal` não eram
  folhas de GC. Agora são `"gc-leaf-function"`.

A sabotagem `slack` dos testes dirigidos passou a zerar a folga: 4 KiB bastavam no macOS
arm64. Com isso, os 4 testes dirigidos de mapas passam no macOS.

**Quem mais tem o defeito do LLVM** (verificado no código de cada projeto, em
`/Volumes/ssd_dock/references`):
- **Exposto:** o passo próprio do Cangjie (`CJRewriteStatepoint`) quebra com
  `noundef returned`; provado com o `opt` compilado do fork deles.
- **Imune pela ordem dos passos:** Perry e JLLVM.
- **Imune só pela cópia antiga do passo:** gollvm.
- **Não usam o RS4GC:** TinyGo, Julia, Kotlin/Native, Clasp, Swift, Nim, Koka, CSharpLLVM,
  il2bc. O llilc usa, mas é da época do LLVM 3.x.

**Medida do modo de mapas** (servidor, mediana de 4 rodadas alternadas):

| | A0 | A1 com contexto | B1 | B1 com o RS4GC antes do `lto<O2>` (como o Perry) |
|---|---:|---:|---:|---:|
| req/s | 67,6k | **68,6k** | 61,3k | 58,5k |
| trees (ms) | 12,48 | **12,43** | 17,88 | 17,85 |

O caminho do Perry evita o defeito, mas perde o inlining e fica ainda mais lento.

**O ARC real** só passa pelo RS4GC com `--roots maps` (opcional). Na pilha-sombra, que é o
padrão, não há funções `gc`. As correções acima valem para qualquer modo de memória.

**CI.** Vermelho há dias, por causas anteriores a hoje. Corrigidas e validadas localmente:
- **Linux e macOS:** o `reload` do JIT recusava as declarações dos externs do fragmento só
  do wasm (`dartforge_js_*`). O nativo deixou de declará-los.
- **Windows, N15 e Heavy `build`:** a action `llvm-windows` não trazia `clang-cl`,
  `llvm-lib` e os headers C++ que a ponte COFF do JIT (`crates/jit/build.rs`) usa.
- **Heavy, testes ignorados:**
  - os testes de imagens gravadas e do sysroot glibc agora pulam sem o insumo;
  - o programa 81 voltou a ter acessos dinâmicos de verdade (a desvirtualização
    escondia as entradas `$g`);
  - o manifesto do SDK selecionado ganhou o patch do `http` (`722e654b`);
  - o teste da interpolação de `fromEnvironment` espera a constante dobrada, que é a
    semântica certa.

**Segundo lote do CI (30/10, tarde, `eff6d02e`…`f0e2cc74`).** O CI de `ae6509a1` ainda
ficou vermelho. Corrigido e validado localmente (export limpo: harness 161/161 nos 4
modos, corpus JS 274/274 em desenvolvimento e produção, testes ignorados e clippy limpos;
só o `ng_generator` falha no export, por não ter o `dart pub get` do `corpus/ngdart`):
- campo, `static` e variável de topo `late final` sem inicializador ganham o setter
  implícito (uma escrita; a segunda lança `LateError`). Programa 166;
- o cabeçalho `ddc-wrong:` declara um defeito conhecido do DDC sem invalidar o oráculo
  (programa 274: closure no inicializador do `for`);
- o JS de desenvolvimento com filtro emitia a classe de mixin legado como classe comum;
- os testes de imagens ligadas acham `ld.lld`, `lld-link` e `ld64.lld` com `.exe` no
  Windows, e o job Linux põe o `bin` do LLVM no `PATH`;
- o teste dos externs do JIT ignora os externs só do wasm;
- o teste de importação de bitcode do `allocation_inventory` (falha só no CI) imprime o
  módulo e as obrigações: a próxima rodada mostra a causa.

**O `33_mensagens_de_controle` não é defeito.** O `dartforge aot` sem `--optimize`
mantém os asserts, como `dart run --enable-asserts`; com asserts a VM também imprime
`do outro lado: null` (o `assert(available > 0)` do `_NativeSocket.read` fecha o soquete).
A referência do script `arcmodes.sh` vinha de `dart run` sem asserts, o mesmo artefato do
`158_assert…`. O script passou a usar `--enable-asserts`.

**Funções duplicadas.** `DARTFORGE_DUPLICATES=1` imprime as funções HIR de corpo igual em
dois níveis: exato, e refinado (chamando funções equivalentes, o caso dos pacotes copiados
para dentro do projeto).

| alvo | instruções HIR | duplicadas, refinado |
|---|---:|---:|
| SDK (servidor, antes da poda) | 1,11 M | 5,2% (o `dart:typed_data`, 24%) |
| `new_sali` backend (programa) | 4,53 M | 1,1% |

O próximo passo é o passo de fusão no HIR. Função sem endereço tomado tem as chamadas
redirecionadas; com endereço tomado, vira thunk e mantém a identidade do tear-off (programa
80). A comparação inclui os planos do ARC, as tabelas de exceção e o rastro. Vale para o
nativo, o JS e o wasm, e não depende do `MergeFunctions` do LLVM.

## Prioridade para 30/10/2026: HTTP a 99% do Dart com o A1 e o contexto por argumento

A rodada 7 de 30/10 mediu as peças do desenho do Dart AOT no servidor e no `trees`, pelo
método da rodada 6 (contadores da thread do isolate, carga < 3,5, `load_keepalive.dart`).
Os detalhes estão em `RELATORIO-PRONTIDAO-PRODUCAO.md`, na seção "rodada 7".

| | Dart 3.6.2 | A0 (padrão) | A1 | A1 + contexto (`2d8f2dc5`) | B0 (mapas) |
|---|---:|---:|---:|---:|---:|
| req/s | 70,2k | 68,2k | 68,7k | **69,5k (99,0%)** | 61,4k* |
| instruções/req | 176,3k | 186,5k | 190,6k | 185,7k | 232,0k |
| ciclos/req | 52,4k | 54,2k | 53,6k | 53,1k | 60,3k |
| executável | 5,63 MB | 4,52 MB | 4,72 MB | 4,70 MB | 7,79 MB |
| trees (ms) | 11,87* | 12,48 | 12,61 | 12,43 | 18,12* |

\* Valores da primeira rodada de 30/10; os demais são da segunda, que alternou só Dart, A0,
A1 e A1 com contexto.

**O que a rodada mostrou:**
- **Mapas de pilha não servem no macOS:**
  - **B1 (`--exceptions tables --roots maps`) não ligava;** corrigido na seção acima.
  - **B0 perde em tudo:** +72% de executável e +45% no `trees`. As raízes ficam no slot da
    pilha, e não no registrador preservado da forma `"deopt"`.
- **O A1 com contexto por argumento ganhou do A0 nas 4 rodadas:** de +1,6% a +3,1% em
  req/s, com menos instruções e ciclos. Custa +181 KB de executável (as LSDAs). O corpus
  passa em A1 otimizado e com estresse.

**Próximos passos:**
1. Corrigir o desenrolar lento de `StackOverflowError` profundo no A1: ~1,5 µs por
   quadro no macOS, porque o dyld relê o Mach-O a cada quadro. Depois, propor ao
   proprietário o A1 com contexto como padrão do AOT nativo. Hoje o padrão é o A0, e o
   relatório de prontidão recomenda o A0.
2. Fechar o ~1% que sobra contra o Dart: as sondas do memo de RTI (~200 por pedido) e o
   contexto por TLS nas entradas de seletor (~393 por pedido).
3. Os mapas de pilha ligam e passam desde 30/10, mas perdem para o A1 no macOS. Só voltam
   a competir com o RS4GC fora do LTO do `lld` e com a forma `"deopt"`.

## Segunda prioridade para 30/10/2026: concluir a reescrita do ARC de verdade

A reescrita (`08039025`, `WIP`) **foi validada e corrigida** num export limpo e entrou no
`main` com três commits de correção, `0c4f29af`, `5213a57a` e `2562a400`. O
desenho e as medidas estão em `docs/ARC-IMPLEMENTACAO.md` §9.

**O que a validação achou e foi corrigido:**
- **31 programas não compilavam no ARC real:** uma classe alocada com vários números
  de campos (contextos de closure, quadros `async`) gerava `@df.drop.<cid>` duas
  vezes. Agora há uma função de descarte por classe, e as classes acima de
  `arc_word::DROP_CLASSES` não ganham nenhuma.
- **104 programas abortavam com `DARTFORGE_ARC_CHECK=1`:** a alocação Owned em linha
  (TLAB e reuso) e o *move* em linha para o campo de um objeto novo passavam por fora
  do inventário de tokens da auditoria. Na auditoria, o runtime não entrega TLAB nem
  listas de reuso ao código gerado, e o *move* lê a cabeça da página.
- **`16_ffi_structs_por_valor` devolvia zeros:** o corpo do callback soltava a struct
  antes de a entrada C copiar os bytes, e o reuso imediato zerava a célula. Agora o
  corpo copia os bytes para o buffer da entrada C pelo novo
  `dartforge_ffi_compound_write`.
- **Cache do SDK truncado (anterior ao WIP):** duas compilações em paralelo no mesmo
  diretório deixavam a dylib do SDK com 0 bytes no cache.
- Também entraram o clippy (`write_literal`), o `transmute` da função de descarte
  na lista do teste `dart_ports` e o `cargo fmt` da reescrita.

**Validação** (`HEAD` = `2562a400`, export limpo):
- testes de runtime e emit-native em release e debug: só as 9 falhas que já existiam
  em `fbd1c287` (fixtures de outras plataformas, `constant_interpolation…`,
  `pruning_removes_unused_members` e o manifesto do SDK selecionado);
- clippy limpo;
- harness 160/160 nos 4 modos;
- corpus nos modos ARC simples, real e virtual, cada um normal, com estresse e com
  auditoria.

**As duas regressões do WIP foram corrigidas em 30/10.** `87_classe_do_slot_sob_coleta`
(auditoria) e `06_escuta_compartilhada` (estresse) eram o mesmo defeito: um candidato a
ciclo morto pelo descarte genérico era liberado duas vezes. Detalhes em
`docs/ARC-IMPLEMENTACAO.md` §9.1. O corpus passa nos três modos ARC, cada um normal, com
estresse e com auditoria.

**O que divergia no corpus ARC era artefato do script:** `33_mensagens_de_controle` e
`158_assert…` dependem dos asserts, e a referência vinha de `dart run` sem eles (ver o
segundo lote do CI, acima). O `arcmodes.sh` agora gera a referência com
`--enable-asserts`.

**Scripts** em `/Volumes/ssd_dock/tmp/arc-redesign/rarc`: `full.sh TAG`, que roda build,
testes, clippy, harness e corpus ARC no export `v1`; `tests.sh`, `harness.sh` e
`arcmodes.sh`; `tc/httpth.sh`, que mede o HTTP com os contadores da thread do isolate.

**O RSS do servidor HTTP no ARC real foi resolvido em 30/10** (`docs/ARC-IMPLEMENTACAO.md`
§9.2): 70,1 → 23,8 MB (ARC simples 19,5, tracing 17,5). Eram os buffers da rodada de
ciclos e da drenagem, refeitos a cada rodada (fragmentavam a zona *medium* do `malloc`),
e o limiar da rodada de ciclos, que o ponto de assentamento dobrava: as rodadas só
rodavam na coleta completa. O relatório `DARTFORGE_GC_MEMORY=1` passou a medir o
processo no macOS.

**CI:** a falha do `allocation_inventory` só no Linux era minha, de `592d7619`: o censo
das imagens do SDK não conhecia o atributo `"gc-leaf-function"`. Reproduzida aqui com
`--features embedded-llvm` e corrigida (`8769a175`).

**Comparação com o Swift e com o Dart VM (30/10, noite).** Mac mini M4, uma thread,
4×8 conexões *keep-alive* por 5 s, mesma resposta HTTP (cabeçalhos do `dart:io`, corpo
*chunked*). Servidores Swift e Dart crus em `/Volumes/ssd_dock/tmp/arc-redesign/rarc/swift`
(`raw/main.swift` com `kqueue`, `nio/` com SwiftNIO, `dartraw/server.dart` com
`RawServerSocket`). Ciclos por pedido (menos é melhor):

| servidor | Swift `-O` | SwiftNIO | DartForge tracing | `dart compile exe` | DartForge ARC simples | DartForge ARC real |
|---|---:|---:|---:|---:|---:|---:|
| cru (mesmo código) | 25,9 mil | — | 37,9 mil | 40,2 mil | 44,1 mil | 102,6 → **90,0 mil** |
| `HttpServer` do `dart:io` | — | 32,7 mil | 66 mil | 70 mil | 88 mil | 281 → **249 mil** |

O tracing já gasta menos ciclos que o `dart compile exe` nos dois. Contra o Swift cru fica
a 1,46×. O ARC real melhorou com quatro commits da noite: leituras de lista em linha
(`d9eb0315`), morte da última liberação sem ponto de assentamento (`1a8987b6`, laço de
alocar e soltar: 148 → 89 ciclos e 1017 → 714 instruções por iteração) e a escrita `Ref`
verificada em linha (`d0a6afa2`).

**Onde vai o tempo do ARC real** (perfil do servidor `dart:io` por `sample`): drenagem e
rodada de ciclos (~18%: cerca de 58 objetos de lixo cíclico por pedido), pontos de
assentamento (~18%), retains no runtime (~15%), escritas de campo e escopos nativos. Os
retains que ainda vão ao runtime são de objetos novos do runtime com escrita crua
pendente; registrar em linha os outros foi medido e não pagou (descartado).

**Segunda rodada do ARC real (30/10, madrugada de 31/10, `872ef877`…`16632201`).** A
contagem das chamadas da ABI por pedido (instrumentação temporária) guiou cada passo:
*boxes* de `int` (242 por pedido, todos Smi), argumentos Owned (292), capturas de ambiente
(108), *closures* (58), slots de quadros proprietários (190), contexto de *closure* (62) e
escritas escalares de lista saem em linha; a rodada de ciclos percorre as arestas uma vez.
Resultado, ciclos por pedido (medidas de 31/10, duas rodadas iguais):

| servidor | Swift `-O` | DartForge tracing | `dart compile exe` | ARC real antes | ARC real agora |
|---|---:|---:|---:|---:|---:|
| cru | 26 mil | 38 mil | — | 106 mil | **82 mil** (−23%) |
| `dart:io` | — | 66 mil | 70 mil | 280 mil | **205 mil** (−27%) |

Também: a chave do SDK de produção passou a incluir o digest da biblioteca do runtime
(`da4998f9`; o inventário publicado se amarra a ele, e o teste do Windows recusava o SDK
do CLI em cache) e o job macOS do CI ganhou 210 minutos (`d0cf42b5`; 150 foram cortados
nos testes do JIT).

**Próximos passos:**
1. ARC real: o que ainda vai ao runtime por pedido no `dart:io` é a morte (~102, já no
   caminho rápido), retains de objetos com escrita crua pendente (~43), escritas `Ref` de
   lista (~22), argumentos de índice variável (~14) e quadros (~8 aberturas). O resto do
   custo está na drenagem e na rodada de ciclos (~58 objetos de lixo cíclico por pedido) e
   na morte em linha no código gerado, ainda por fazer.
2. Reaplicar `/Volumes/ssd_dock/tmp/arc-redesign/final_field_elision.patch` e medir de
   novo.

## 29/10/2026, madrugada: mapas e `async` no JS, HTTP a 98,5% do Dart, reescrita do ARC

Commits até `6646f294`, validados localmente e enviados. A suíte
`tests/language` foi rodada num export recompilado de cada HEAD que mexia no
JS e ficou em **2756/2863** em todas as rodadas, sem regressões.

**JS de produção: velocidade contra `dart2js -O4`** (rodada encerrada em
`2df83de7`; detalhes em `docs/JS-PRODUCAO-VELOCIDADE.md` §9–§10).

| teste, -O4 (ms) | antes | agora | dart2js -O4 |
|---|---:|---:|---:|
| Node `map_int_object` | 12,6 | 5,2 | 4,3 |
| Node `async_await` | 571 | 333 | 247 |
| Node `collections/map` | 57,7 | 48,2 | 44,7 |
| Chrome `async_await` | 490 | 316 | 221 |
| Chrome `collections/map` | 29,3 | 16,1 | 30,7 |

A máquina estava muito carregada durante as medições, então os tempos
absolutos estão inflados; vale comparar dentro de cada linha.

**Mapas com chave primitiva.** O `IdentityMap` passou a guardar as entradas
em células encadeadas, achadas por `_strings`, `_nums` e um `Map` para as
demais chaves, como no `dart2js`.
- **Velocidade:** com chave `int`, de 2,9× para cerca de 1,2× do `dart2js`.
  No Chrome, `collections/map` agora ganha do `dart2js`.
- **Divergências antigas da VM, corrigidas:**
  - uma chave `-0.0` mantém a primeira chave inserida;
  - uma chave `NaN` nunca é encontrada de novo;
  - `addAll` só conta como modificação quando insere chaves novas.

**`async`.** Caiu de 2,2× para cerca de 1,35–1,5× do `dart2js`.
- **O que entrou:**
  - `async` compilado como máquina de estados;
  - os caminhos do `await` no runtime ficaram mais curtos;
  - estáticas preguiçosas de biblioteca viram propriedades comuns;
  - receitas de closures avaliadas uma vez.
- **Regressão e correção:** converter também as estáticas de classe fazia o
  V8 tratar a classe como dicionário (`json_encode` mixed_utf16 de 68 para
  100 µs/op). Corrigido em `aae6630f`.
- **O que falta:** o runtime das futures, que avalia dois tipos para criar
  cada `Completer` e lê a zona atual por um getter preguiçoso. Os próximos
  candidatos são passar o tipo do `Completer` pelo compilador e mover a zona
  atual para o nível de biblioteca.
- `async*` e `sync*` continuam com gerador: nenhum teste de desempenho os usa.

**Bug corrigido:** `main(List<String> args)` recebia `undefined`. Agora
recebe uma lista vazia, como a VM e o `dart2js`.

**Tamanho.** O -O4 foi de 0,927× para 0,935× do `dart2js -O4`, ou 0,981× com
gzip, e continua menor. O -O2 está em 0,840×, ou 0,877× com gzip. Quase todo
o aumento vem do mapa novo, cerca de 1 KB em cada programa com mapa de chave
primitiva.

**HTTP nativo, sexta rodada (em andamento).** O primeiro lote foi enviado:
- **`1dd200b2`:** o prólogo de cada função ficou mais curto, com tabela fixa
  de áreas, caminhos de exceção fora do fluxo principal e raízes dos
  parâmetros registradas na abertura do quadro.
- **`08b190c2`:** memo por thread das chamadas dinâmicas que erram o caminho
  rápido do seletor.

- **`6646f294`:** a tabela vazia de áreas saiu das constantes do executável.
  Com isso o servidor fica 16 KB menor que antes da rodada.

Os 160 programas do corpus passam nos quatro modos de memória.

| carga < 3,5, mediana de 4 | antes | agora | Dart 3.6.2 |
|---|---:|---:|---:|
| req/s | 65,4k | **69,8k** | 70,9k |
| % do Dart | 92% | **98,5%** | |
| instruções por requisição | 188,1k | 185,5k | 176,2k |
| ciclos por requisição | 55,2k | 53,0k | 51,9k |
| CPU do isolate por requisição | 10,39 µs | 9,68 µs | 8,85 µs |
| executável do servidor | 4,518 MB | 4,501 MB | 5,625 MB |
| trees (ms por rodada) | 12,2 | 12,2 | 11,85 |

Medido e descartado:
- elisão da checagem na cauda, que quebrava a detecção de estouro de pilha;
- exceções por tabelas;
- geração jovem maior.

**ARC de verdade: reescrita do núcleo (em andamento).** Aprovada pelo
proprietário. Os quatro passos entram juntos e valem só para o modo
`DARTFORGE_ARC_PREPARED=1`:
- contador dentro do objeto;
- metadados só para os casos raros;
- liberação gerada por classe;
- reaproveitamento imediato da célula liberada.

A meta é cerca de 2–3× o ARC simples, contra 11,6× hoje no `trees`. Ainda não
há commit enviado.

## 28/10/2026, fim do dia: níveis -O1..-O4 do JS, ARC de verdade, HTTP a 94% do Dart

Commits até `fb01240b`, validados localmente e enviados. A suíte
`tests/language` foi rodada num export recompilado de cada HEAD que mexia no
JS e ficou em **2756/2863** em todas as rodadas.

**JS de produção: os quatro níveis do `dart2js`.** Decisão do proprietário.
- `-O1`: sem minificação e mantendo os nomes de `late`.
- `-O2`: o padrão, igual ao perfil anterior.
- `-O3` (`--omit-implicit-checks`): omite as checagens implícitas.
- `-O4` (`--trust-primitives`): confia nos primitivos.
- Os pragmas `dart2js:as`, `downcast`, `parameter`, `types` e `index-bounds`
  (`trust`/`check`) valem em todos os níveis.
- `-O1` e `-O2` batem com a VM no corpus. `-O3` e `-O4` são validados nos
  programas que não dependem das checagens removidas; a lista de exclusão é
  `crates/differential/production_levels.tsv`.

| tamanho contra `dart2js -O4` | nosso `-O4` | razão | gzip -9 |
|---|---:|---:|---:|
| corpus (273) | 10,2 MB | 0,929× | 0,970× |
| `limitless_ui` | 3,34 MB | 0,90× | |
| `new_sali` | 7,09 MB | 0,93× | |

**Velocidade do JS contra `dart2js -O4`** (Node, ms; detalhes em
`docs/JS-PRODUCAO-VELOCIDADE.md`):
- **Causa principal da lentidão anterior:** os protótipos de `Number` e
  `String` viravam dicionários lentos no V8. Isso foi corrigido.
- **Também entraram:**
  - cache próprio de receitas RTI;
  - `async` sem `await` executado como função comum;
  - `length`/`isEmpty` nativos;
  - `for-in` em laço indexado.
- **Bug corrigido:** `!i.isEven` era sempre falso.
- **Empatados ou à frente:** cerca de metade dos kernels.
- **Mais lentos:**
  - `async_await` (4,6×): usamos geradores, o `dart2js` usa máquina de
    estados;
  - `list_sort` (2,3×);
  - `map_int_object` (2,4×);
  - `collatz` (1,5×).
- **Bug encontrado:** `main(List<String> args)` recebia `undefined`.
- Uma rodada nova trata desse bug e do `async`.

| kernel | antes | agora | dart2js |
|---|---:|---:|---:|
| point_sum | 33,6 | 3,4 | 3,5 |
| strings/hashes | 33,1 | 11,0 | 10,6 |
| sieve | 21,9 | 7,9 | 8,5 |
| shapes | 2,5 | 1,4 | 3,9 |
| list_sort | 578 | 324 | 139 |
| async_await | 2132 | 1154 | 250 |

**HTTP nativo (modo padrão)**, rodadas 4 e 5:
- `==` de String, StringBuffer e `jsonEncode` sem chamadas ao runtime.
- `memcpy`/`memmove` direto no código gerado.
- Memo de RTI de duas vias.
- `_List(length)` e literais mistos alocados no código gerado.
- Chamadas ao runtime por requisição: ~303 → ~100.

| | início (27/10) | agora | Dart 3.6.2 |
|---|---:|---:|---:|
| req/s | 55,6k | 65,8k | 69,8k |
| instruções por requisição | 271k | 207k | 211k |
| ciclos por requisição | 79k | 71,7k | 73,1k |
| CPU do isolate por requisição | — | 9,9 µs | 8,6 µs |
| executável | — | 4,52 MB | 5,63 MB |

A diferença que resta está no código compilado: o quadro raiz e a checagem
de exceção pendente em volta das chamadas, e os seletores com argumentos em
array.

**ARC de verdade** (`DARTFORGE_ARC_PREPARED=1`). O objeto morre no
*release* que zera a contagem, sem coleta no meio. Os testes provam isso para
local, campo, lista, valor de mapa, closure, cascata e finalizador.
- **O que entrou:**
  - retain/release e escritas em campos contadas direto no código gerado;
  - alocação pelo TLAB;
  - ciclos liberados nas drenagens: RSS do HTTP de 77 para 39–57 MB.
- **Modos alternativos:**
  - `DARTFORGE_ARC_TOKENS=virtual`: modo rápido, sem morte imediata;
  - `DARTFORGE_ARC_PROMPT=0`: o preparado antigo.
- **Bugs corrigidos:**
  - o ARC puro não contava instâncias canônicas novas;
  - a flag dos prefixos `deferred` quebrava a preparação ARC (programa 147);
  - o teste `lean_descriptor_gives_n_zeroed_slots_and_same_area` usava o
    formato antigo de descritor e lia fora dos limites.

| ciclos, ARC de verdade ÷ ARC simples | trees | strings | JSON | HTTP |
|---|---:|---:|---:|---:|
| razão | 11,6× | 3,6× | 2,5× | 3,8× |

O ARC simples continua o padrão. O tempo do ARC de verdade está em faltas de
cache nos metadados por objeto: 45% na morte em cascata, 13% na drenagem e 9%
no registro. A contagem em si fica abaixo de 10%. Chegar a cerca de 2–3× exige
reescrever o núcleo, com estes quatro passos juntos:
- contagem no objeto;
- metadados preguiçosos;
- *drop* por classe;
- reuso imediato da célula.

O estudo está em `docs/ARC-IMPLEMENTACAO.md` §8.6. Falta a decisão do
proprietário.

## 27–28/10/2026: ARC com nursery, HTTP nativo e tamanho do JS de produção

Commits até `4a8143dc`, validados localmente e enviados. A suíte
`tests/language` foi rodada em um export de cada HEAD enviado e ficou em
**2756/2863** em todas as rodadas.

**ARC.** O modo nursery foi corrigido: um slot dono registrava um objeto
jovem cedo demais e causava `RcZero`. Agora ele é o padrão para ARC simples
e preparado; o ARC puro só existe compilando com `DARTFORGE_ARC_NURSERY=0`.
Detalhes em `RELATORIO-PRONTIDAO-PRODUCAO.md` e `docs/ARC-IMPLEMENTACAO.md`.

| ARC (ms; HTTP em req/s) | padrão | simples antes | simples agora | preparado antes | preparado agora |
|---|---:|---:|---:|---:|---:|
| trees | 13 | 282 | 24 | 360 | 171 |
| strings | 42 | 132 | 53 | 370 | 267 |
| JSON | 74 | 300 | 165 | 743 | 582 |
| HTTP | 55,1k | 14,1k | 40,5k | 5,6k | 8,2k |

**HTTP nativo (modo padrão).** Três rodadas foram fechadas e a quarta está
em andamento.

| | início | agora | Dart 3.6.2 |
|---|---:|---:|---:|
| req/s | 55,6k | 62,8k | 71,0k |
| instruções / ciclos por requisição | 271k / 79k | 231k / 72k | 212k / 71k |
| CPU da thread do isolate por requisição | — | 11,2 µs | 8,7 µs |
| trees (ms) | 13,8 | 12,15 | 11,8 |

As principais mudanças foram estas:
- o contexto da thread passa como argumento nas chamadas diretas;
- memo de tipos por thread para os caches de RTI;
- `String.[]` Latin-1 e `StringBuffer` em linha;
- listas literais e listas tipadas pequenas alocadas pelo TLAB;
- varredura jovem por palavra de bitmap;
- limite jovem adaptativo.

O modo `DARTFORGE_CHECK_SHADOW=1` voltou a funcionar: três externos estavam
marcados como "coleta" sem alocar no heap. I/O e syscalls são equivalentes
aos do Dart. A diferença que resta está no runtime: cerca de 3,8 µs por
requisição, contra 1,6 µs na VM.

**JS de produção contra `dart2js -O4`.** Os detalhes estão em
`docs/JS-PRODUCAO-TAMANHO.md`, §15–§19.

| alvo (bytes) | início | agora | dart2js -O4 | razão |
|---|---:|---:|---:|---:|
| `print('hi')` | 44.383 | 2.128 | 5.074 | 0,42× |
| corpus (273), total | 17.242.249 | 10.967.007 | 11.024.980 | 0,995× (gz 1,040×) |
| corpus, mediana | 1,465× | 0,976× | | gz 1,032× |
| `limitless_ui` | 3.455.727 | 3.369.232 | 3.704.125 | 0,910× |
| `new_sali` | 7.404.523 | 7.178.421 | 7.614.136 | 0,943× |

Em gzip o corpus ainda está 4% acima, e a rodada seguinte trata disso.
Honrar `@pragma('dart2js:as:trust')` e confiar nos primitivos como o
`dart2js -O4` mudaria a semântica, por isso ficou de fora até decisão do
proprietário.

## Fechamento de 27/10/2026: nenhuma falha exclusiva do JS na suíte `tests/language`

Commits `0506fa00` a `3db3a201`, validados localmente (suíte completa sem
regressões, corpus JS **274/274** em dev e produção, testes e clippy de
`emit_js` e `types`) e enviados.

| Ferramenta | Iguais à VM 3.6.2 (2863 programas) |
|---|---:|
| DartForge JS dev, início da sessão (`f994cb83`) | 2526 |
| DartForge JS dev, agora (`3db3a201`) | **2756** |
| DDC 3.6.2 oficial | 2744 |

Falhas que só o DartForge tem: **53 → 0**. As 107 que restam o DDC também
erra (divergências da plataforma web: inteiros, `identical` de números,
mensagens).

Entraram neste fechamento: *tear-offs* genéricos de construtor (`C.new`,
`GGen.new`) e por alias; instanciação para limites com parâmetros externos
em escopo; captura das variáveis do inicializador do `for` pela primeira
iteração (declaradas no cabeçalho e inicializadas dentro da primeira
iteração); tipo de retorno de funções locais vindo da inferência comum
(inclusive `return` em ramo morto) e tipo de retorno escrito mantido em
funções `async`; `dynamic`/`void` contra `T?` resolvem `T` para `Object`
(argumentos de tipo de extensão); `await` de variável de tipo promovida a
tipo futuro; closure atribuída a local promovido inferida contra o tipo
declarado, como no CFE (o `types` agora registra quais atribuições usaram o
contexto promovido, sem mudar o analisador); instanciação implícita que
resolve para `Never` em constantes.

## Continuação de 27/10/2026: JS acima do DDC na suíte `tests/language`

Mais 21 commits, de `28a109bc` a `08c5d571`, todos validados localmente
(suíte completa sem regressões a cada lote, corpus JS em dev e produção,
testes e clippy do `emit_js`) e enviados.

| Ferramenta | Iguais à VM 3.6.2 (2863 programas) |
|---|---:|
| DartForge JS dev, início da sessão (`f994cb83`) | 2526 |
| DartForge JS dev, fechamento anterior (`7c965006`) | 2703 |
| DartForge JS dev, agora (`08c5d571`) | **2745** |
| DDC 3.6.2 oficial | 2744 |

Falhas que só o DartForge tem: **53 → 11**. As 107 que o DDC também erra são
divergências da plataforma web; em 12 programas acertamos e o DDC não.
Corpus JS: **271/271** em dev e produção.

Entraram nesta continuação: privados resolvidos por biblioteca (acesso e
chave de membro), *throwers* para privados de outra biblioteca mesmo sem
`noSuchMethod`, campos `abstract` sem armazenamento, campo privado virtual
quando um subtipo o sobrescreve; `!` dentro da cadeia de *null-shorting*;
`void f() => e` devolve o valor; `??` em valor padrão sem temporário;
estáticos e construtores via alias (inclusive `T<int>.named()` e alias de
variável de tipo); função local genérica com limite de tipo função com
nomeados (correção no *parser*); aliases `dartx` para membros herdados de
superclasse comum; instanciação de funções genéricas guardadas em locais;
cast de padrão `as` sempre avaliado; verificação de limites de métodos
genéricos com limite que menciona o parâmetro da classe (`checkTypeBound`)
e em sobrescritas; stubs reificados com todos os covariantes como `Object?`;
`late final` com `LateError.localADI`; encaminhadores de `noSuchMethod`
assinados (setter e genéricos com padrões); ordem de avaliação de nomeados
antes de posicionais; nomes de `Object.prototype` como parâmetros nomeados;
`static const` com setter estático homônimo; setters de mixin sob setter
covariante; enums genéricos instanciados e chamados via `call`; padrões
`dynamic` em métodos genéricos; cadeias de *factories* redirecionadas com
argumentos compostos; constantes canonizadas independentemente do
construtor; campos de record `constructor`/`prototype`; cast de `dynamic`
para `Never`/`Null`; parâmetros genéricos no formato antigo (`bar<V>(…)`).

As 11 que faltam: *tear-offs* de construtor genérico sem contexto e por
alias (`GGen.new`, `Bounded.new`) e literais de tipo instanciados (exigem
funções genéricas com rti montado em tempo de execução); variável de tipo
promovida a interseção em `await`; argumentos de tipo de extensão inferidos
(`[o].whereNotNull()`); avaliação constante com `T` → `Never` em
`const C(f)`; captura do inicializador do `for` por closures; contexto de
atribuição a local promovido (o `types` segue o analyzer, o CFE usa o tipo
declarado); `closure/type_arguments`, `regress50905`, `async/congruence_local`.

**LSP** (agente, `bff32730`, `351888b0`, `3faf2276`): formatação de 80 000
termos de 13 s para 0,14 s (o gargalo era a conversão de posições no LSP,
mais um caso quadrático no divisor de linhas), formatação numa thread
própria com cancelamento (`$/cancelRequest`) e `ContentModified`, e a busca
de SDK passa ao próximo candidato quando um não carrega. O hover de
`[1, 'a': 3]` já era igual ao do Dart 3.6.2 (a nota anterior estava errada).

**AOT nativo** (agente em andamento, commits locais ainda não enviados):
JSON já à frente do Dart (3,66 G contra 4,49 G ciclos); `async` de 6,39 G
para 5,17 G ciclos (Dart 5,08 G) com `await` de futuro concluído no estilo da
VM, quadros alocados na TLAB e cache de `is` para `null`/Smi; árvores ainda
+16% de ciclos; HTTP `/json` 47,2 mil contra 71,9 mil pedidos/s (25,5 contra
18,8 µs de CPU por pedido). Em andamento: segundo nível no cache de RTI dos
sítios `is`. Máquina carregada (load 5–20): comparações por ciclos e
instruções.

## Atualização de 26–27/10/2026: JS, analisador, LSP e extensão VS Code

Foco: compilador JavaScript pronto para produção, analisador e LSP com a
extensão do VS Code. Onze commits, de `784bd199` a `7c965006`, validados
localmente no macOS arm64 e enviados.

### Suíte `tests/language` (JS dev)

O runner da rodada anterior se perdeu junto com o scratchpad; agora está
versionado em `scripts/run-language-suite.py` (`setup --checkout <sdk> --tag
3.6.2`, depois `vm`, `ddc`, `forge`, `compare`). O conjunto que ele monta tem
**2863 programas** (os `*_test.dart` sem expectativas de erro estático, fora
de `vm/`, sem `dart:io`/`isolate`/`mirrors`/`ffi`/`html`/`js`/`dart:_`). Não é
o mesmo recorte dos 2266 das medições anteriores, então os números não se
comparam diretamente com os de 25/10.

| Ferramenta | Iguais à VM 3.6.2 |
|---|---:|
| DartForge JS dev, início da rodada (`f994cb83`) | 2526 |
| DartForge JS dev, agora (`7c965006`) | **2703** |
| DDC 3.6.2 oficial (`dartdevc` + Node) | 2744 |

Das 160 falhas restantes, **53 são só nossas** (o DDC acerta), 107 o DDC
também erra (divergências de plataforma web: inteiros, `identical` de
números, mensagens) e há 12 programas em que nós acertamos e o DDC não.
Nenhuma regressão entre as medições intermediárias. Corpus JS: **259/259**
em dev e em produção (eram 252).

O que entrou no emissor JS (e no mundo fechado de produção):

- Downcasts implícitos de `dynamic` para tipos com parâmetros de tipo
  (`List<T>`, `T`, `T Function(T)`), que antes passavam sem verificação; o
  maior grupo de falhas (100 programas de `function_type/`).
- Contexto por campo em literais de record (tear-off implícito de `call`).
- Mixins: inicializadores de campo de classe usada como mixin antes da 3.0;
  construtores encaminhados por aplicações nomeadas (`class D = B with M`),
  chamados e com argumentos verificados.
- `main` exportado por outra biblioteca (dev e raízes do mundo de produção).
- Ordem de avaliação da VM ao chamar o valor de um getter (argumentos antes
  da leitura, com receptor explícito ou `this` implícito).
- Tear-off implícito de `call` aplicado ao todo em cascata, condicional e
  `??`, e nos setters estáticos, de topo e de extensão e em `[]=`.
- `p.loadLibrary` como tear-off; `p.lista[i] = v` verifica o prefixo diferido.
- Acesso dinâmico a nomes privados com o símbolo da biblioteca; encaminhadores
  de `noSuchMethod` para privados da própria biblioteca e *throwers* para os
  de outra; campo privado que sobrescreve membro herdado vira par de acessores.
- Campos privados promovidos (`inference_update_2`, 39/39): como alvo de
  chamada e como receptor (o crate `types` passou a registrar o tipo
  promovido de `c._f(x)`).
- `await for` dentro de literais de coleção.
- `dynamic` em spread e `for`-in: cast para `Iterable`/`Map` e de cada
  elemento; `async =>` e `return` verificados contra `FutureOr<T>`; `await`
  com `awaitWithTypeCheck` quando o tipo estático não é futuro.
- `bool/String.fromEnvironment('dart.library.x')` coerente com os imports
  condicionais; declaração rotulada; casos vazios agrupados em `switch` com
  `continue` para rótulo; `toString`/`hashCode`/`runtimeType` de tipo anulável.
- Tear-off de método genérico cujo limite menciona o parâmetro da classe é
  verificado contra o tipo estático.
- **Dois problemas de robustez do compilador:** estouro de pilha na receita
  de tipo com parâmetro F-limitado fora de escopo (o processo abortava) e
  inferência de override quadrática: uma classe com 20 000 campos sem tipo
  levava **137 s** de inferência de corpos, agora **24 ms**.

Falhas só nossas que restam, por tema: setter de campo de mixin sobre setter
covariante (`forwarding_stub_set`), variável de tipo promovida a interseção
em `await`, setter estático com o nome de um `static const`, enums com mixins
genéricos, `generic/local_functions`, tear-offs de construtores por alias,
instanciação explícita de literais de tipo, `records/regress54375`,
`is/object_test`, `loop/for_variable_capture` e casos isolados de
`nosuchmethod_forwarding`, `covariant`, `patterns/exhaustiveness/*cast*`.

### Analisador

Commit `fdb0b494` (agente): 28 falsos positivos a menos e 3 diagnósticos
que faltavam, comparando com `dart analyze` 3.6.2 em 137 diretórios de
`tests/language` e 4032 variantes de multitestes, sem nenhuma divergência
nova. Corrigidos: `type_argument_not_matching_bounds` com tipos de função
genéricos (tear-off de construtor por alias, limites de parâmetros de
função antigos, tipos padrão de tipos crus com `Never` nas posições
contravariantes), `return_of_invalid_type`/`undefined_getter` em `x.call`
com `T extends Function`, tear-off implícito em atribuição a local promovido
e membro abstrato não implementado quando há conflito getter/método. Os
itens "implementação herdada que não conforma" e "`_` duplicado antes da
3.7" já estavam corretos; o segundo só diverge num arquivo com `pubspec.yaml`
sem `package_config.json`, onde usamos a versão corrente (3.13) por decisão
de projeto. Commit `47393676`: entrada de mapa dentro de literal de lista ou
conjunto (`{1, 'a': 3}`) derrubava a inferência (pânico ao digitar).

### LSP e extensão do VS Code

- `784bd199`: a extensão em `editors/vscode` funciona sem o Dart-Code:
  registra a linguagem `dart`, gramática TextMate própria, configuração de
  linguagem, descoberta do servidor (`dartforge.serverPath`, binário
  embutido em `server/`, `PATH`), comandos *Restart Server* e *Show Output*,
  convivência com o Dart-Code (`dartforge.enable`), configurações que o
  servidor lê de fato e empacotamento `.vsix` (95,7 KB sem o binário,
  5,85 MB com ele). Testada num VS Code 1.143.1 real (ativação,
  diagnósticos, *semantic tokens*, hover, formatação, reinício).
- `d566cc2e`: descoberta da SDK do Dart em oito etapas, comum ao compilador,
  ao `analyze` e ao LSP (`initializationOptions.sdkPath`, `DARTFORGE_SDK_LIB`
  — que a extensão preenche com `dartforge.sdkPath` —, SDK ao lado do
  executável/`pack`, `DART_SDK`, `FLUTTER_ROOT`, `dart`/`flutter` no `PATH`
  seguindo symlinks, locais conhecidos). Sem SDK, um aviso único; a SDK
  escolhida vai para o log. `$/setTrace` tratado. Robustez: despacho numa
  thread com pilha de 1 GiB (expressões muito aninhadas abortavam o
  servidor), mudança de `package_config.json` chega aos documentos abertos,
  pânico em notificação mostra um único aviso. Sessões reais:
  `postgres_fork` (25 arquivos, ~93 MB) e `new_sali/backend` (120 arquivos,
  ~245 MB, memória estável). Testes de sessão de editor em
  `crates/lsp/tests/editor_session.rs`.
- Pendências do LSP de 26/10, resolvidas em 27/10: a formatação de cadeias
  binárias longas era quadrática na conversão das edições em posições (cada
  edição varria a linha desde o início) e, para `1 * 2 + …`, no divisor de
  linhas (cópia do conjunto de regras a cada tentativa); 80 000 termos caíram
  de 13 s para ~0,15 s e de 1,5 s para 0,16 s, com o oráculo do formatador
  em 45 407/45 407. A formatação roda numa thread própria: passou de 200 ms,
  é respondida depois, sem travar o despacho, e aceita `$/cancelRequest`.
  O hover de `var x = [1, 'a': 3]` com `List<Object>` está certo: o
  `dart language-server` 3.6.2 dá `List<Object>` no topo, em campo e em
  local (a recuperação do parser separa `'a': 3` em dois elementos) e
  `dynamic` para `{1, 'a': 3}`; ficou um teste com esses casos. Uma SDK que
  existe mas falha ao carregar agora é recusada no log e a descoberta segue
  para o próximo candidato. Continua caro, como no `dart format`, formatar
  listas com milhares de argumentos (5 000 `a + b`: ~2,4 s nos dois).
- **Sessões grandes e incrementais (31/10).** Medido primeiro com um
  driver novo que fala com o binário real pelo protocolo
  (`crates/lsp/examples/editing_latency.rs`, `987955f6`): abre a pasta,
  digita um comando caractere a caractere esperando cada versão, rajada sem
  esperar, hover/completar, edição de API e de corpo em outro arquivo, e
  mede o *footprint* do servidor e dos filhos. Causa achada com `sample`:
  cada tecla reanalisava **todos** os arquivos do pacote do workspace num
  filho novo — verificadores de tipos dos 326 arquivos do
  `new_sali/frontend` (2,8 s) e carga de todo o programa (1,2 s de ler,
  lexar e analisar ~30 MiB de dependências). Correções:
  - `e18b1413`: plano por assinatura de API. O trabalhador guarda por
    arquivo os diagnósticos publicados, a assinatura de API
    (`elements::incremental::api_signature`) e os alvos das diretivas;
    edição que mantém a assinatura reanalisa só a biblioteca (arquivo,
    partes e dono), edição que muda reanalisa os dependentes
    transitivos, e o resto sai com o que tinha. Teste novo
    (`tests/typed_incremental.rs`) compara a cada passo de uma sequência de
    edições tudo o que o editor mostra com um servidor que analisa do zero.
  - `acba3c82`: processo de análise residente
    (`--typed-analysis --resident`) que guarda as unidades analisadas entre
    as análises (`parity::analysis::Resident`); só o que mudou é
    reanalisado. Cancelamento cooperativo por id (500 ms de tolerância,
    depois o filho é encerrado), orçamento `DARTFORGE_LSP_RESIDENT_MIB`
    (64), fim após `DARTFORGE_LSP_RESIDENT_IDLE_SECS` (300) parado; a
    primeira análise e lotes acima de 32 arquivos seguem num filho próprio.

  | | frontend antes | depois | limitless_ui antes | depois |
  |---|---:|---:|---:|---:|
  | `didChange` → tipados (mediana de 35 teclas) | 2.811 ms | 383 ms | 2.455 ms | 175 ms |
  | rajada de 15 teclas → assentado | 2.635 ms | 440 ms | 2.209 ms | 202 ms |
  | edição de API usada por outros arquivos | 3.809 ms | 2.183 ms | 2.187 ms | 887 ms |
  | edição de corpo nesse arquivo | 3.642 ms | 474 ms | 2.066 ms | 79 ms |
  | filho de análise: pico | 571 MiB | 751 MiB (residente) | 284 MiB | 333 MiB (residente) |

  Carga da máquina ~8–15; sob carga ~40–70 o antes era 6,0 s por tecla no
  frontend e o depois 0,28 s. O residente fica estável em ~353 MiB vivos
  (*footprint* ~720 MiB com as páginas que o alocador guarda) e devolve tudo
  ao terminar. Pendências: hover e completar no frontend ainda recarregam o
  programa dentro do servidor a cada consulta (~0,9 s, pico de ~480 MiB no
  servidor) porque passam do orçamento da sessão semântica (8 MiB de
  fonte); edição de API num arquivo do *barrel* (203 importadores) ainda
  custa ~2,2 s, num filho frio; outline e passadas de inicializadores
  continuam sobre o programa inteiro (~0,15 s por tecla). Detalhes em
  `docs/LSP.md`, "Edição incremental".

## Balanço das últimas 48 horas — corte em 20/10/2026 às 14:33:15

Janela fixa: **18/10 às 14:33:15 → 20/10 às 14:33:15, UTC−3**.
Foram **102 commits no DartForge**, até `bf412998`, e **20 no dart_ui**,
até `15aff70`. Os inventários completos, com assunto, data e arquivos de cada
commit do compilador, estão em
[history.json](bench/results/2026-10-20-status-48h/history.json) e
[ui-history.json](bench/results/2026-10-20-status-48h/ui-history.json).
As medições novas abaixo foram feitas após esse corte, no mesmo código.
As seções seguintes deste arquivo conservam o histórico: seus números e
pendências devem ser lidos com a data de cada rodada.

### Compilação JavaScript: medição nova em 20/10

**Neste recorte pequeno, JS dev compila mais rápido que o DDC. Produção também
compila mais rápido que dart2js O2, mas gera arquivos maiores.** Não há uma
nova medição completa de webdev/ngdart ou de edição incremental nesta rodada.

Mac M4 ARM64, Dart 3.6.2, Node 24.21.0; compiladores Rust construídos agora em
`--release`, perfil padrão otimizado/ThinLTO. Um aquecimento por ferramenta e
entrada; três processos novos medidos, alternando a ordem. Tempos incluem
início do processo, compilação e escrita; filesystem aquecido, sem apagar caches
do sistema. Todas as **12 combinações programa/ferramenta** executaram em Node
com stdout idêntico ao da VM. Esses três programas não representam aplicações
grandes nem demonstram paridade de toda a linguagem. Carga de um minuto durante
os comandos JS: 4,94–5,47; as VMs estavam suspensas.

| Entrada | JS dev, mediana | DDC direto, mediana | DDC / nosso | JS produção checked | dart2js O2 |
|---|---:|---:|---:|---:|---:|
| hello | 18,63 ms | 142,99 ms | 7,67× | 137,13 ms | 670,17 ms |
| classes | 19,08 ms | 145,06 ms | 7,60× | 136,83 ms | 695,72 ms |
| JSON | 20,51 ms | 158,61 ms | 7,73× | 297,78 ms | 815,70 ms |

O DDC é chamado diretamente pelo snapshot, sem custo de webdev/build_runner.
Não se compara nosso processo novo com um worker DDC persistente. Produção usa
o padrão checked do `dartforge-jsprod`, sem `-O4`/`--omit-checks`; o comparador
é `dart compile js -O2`, também sem omitir implicit checks.

**Tamanho implantável em bytes: bruto / gzip9**, sem source maps. Para dev/DDC
inclui módulos, launcher e **uma cópia do mesmo `dart_sdk.js`**; gzip é somado
por arquivo. Para produção, inclui o arquivo autossuficiente emitido.

| Entrada | JS dev | DDC | JS produção | dart2js O2 |
|---|---:|---:|---:|---:|
| hello | 7.089.075 / 838.342 | 7.088.414 / 838.070 | 47.262 / 16.362 | 33.591 / 11.102 |
| classes | 7.100.116 / 840.033 | 7.100.449 / 840.057 | 54.287 / 18.551 | 37.055 / 12.257 |
| JSON | 7.102.199 / 840.381 | 7.106.123 / 840.752 | 156.163 / 48.542 | 60.854 / 19.699 |

Nosso dev e DDC ficam praticamente do mesmo tamanho quando o SDK compartilhado
entra na conta. Nosso JS produção é **1,41× / 1,47× / 2,57×** maior em bruto
e **1,47× / 1,51× / 2,46×** em gzip nestas entradas, respectivamente. O custo
fixo do runtime é especialmente visível nos programas pequenos.

Aplicações grandes têm resultados **históricos**, sem recompilação nesta rodada:
em 14/10, `limitless_ui` produção tinha 3.535.556 B / 1.020.032 B gzip contra
3.870.230 / 1.020.812 do O2; `new_sali`, 7.625.133 / 2.078.667 contra
8.022.590 / 2.110.700. Portanto, o resultado de tamanho depende da aplicação.
A comparação dev/webdev de 13/10 encontrou 4,9 contra 103 s e 19,4 contra 340 s,
mas teve carga alta, poucas amostras e geração prévia no checkout: **não tratar
“20×” como resultado atual nem como garantia de checkout limpo**.
Método histórico: [DEV-VS-DDC](docs/DEV-VS-DDC.md).
Resultados novos, comandos completos, hashes e reprodução:
[evidências de 20/10](bench/results/2026-10-20-status-48h/README.md).

### AOT: medição nova e ganhos de compilação registrados

Executáveis macOS ARM64 reconstruídos com `--optimize`, política padrão de
exceções, contra `dart compile exe` 3.6.2. Cinco execuções externas alternadas,
quatro amostras internas por núcleo: **20 amostras por ferramenta/núcleo**.
Todos os checksums coincidem. O driver DartForge preservado é debug; os
executáveis Dart gerados são otimizados. Seus tempos de compilação não servem
para comparar a velocidade de um compilador release com o SDK oficial.

| Núcleo | DartForge, mediana [mínimo] | Dart AOT, mediana [mínimo] |
|---|---:|---:|
| fib | 5 [5] ms | 9 [9] ms |
| árvores | 21 [20] ms | 12 [11] ms |
| numérico | 103 [98] ms | 98,5 [95] ms |
| strings | 45 [43] ms | 46 [45] ms |
| JSON | 74,5 [71] ms | 75 [70] ms |

Fib está à frente; árvores continuam **1,75× mais lentas pela mediana**.
Os demais ficam próximos neste recorte. Isso não substitui medições de HTTP,
UI, memória ou aplicações prolongadas.

| Executável, bytes em disco | DartForge | Dart AOT 3.6.2 | Nosso / Dart |
|---|---:|---:|---:|
| hello, compilados agora | 2.996.032 | 5.329.888 | 0,56× |
| benchmark, compilados agora | 3.144.640 | 5.379.136 | 0,58× |
| explorador macOS, mesmo dart_ui `15aff70` | 25.224.176 | 12.257.440 | 2,06× |

O AOT do explorador DartForge vem da última matriz validada; o oficial foi
recompilado agora, mas **não executado nesta comparação de tamanho**. Recursos,
fontes e host AppKit externos não entram nesses bytes. Linux ARM64 e Windows
x64 DartForge da matriz medem 37.619.528 e 37.224.960 B; não há par oficial
novo nesses dois destinos. Não comparar tamanhos de formatos/sistemas distintos
como se fossem efeito exclusivo do compilador.

Ganhos locais de preparação/emissão demonstrados durante as 48 horas:

| Alteração | Antes → depois | Alcance da medida |
|---|---|---|
| Reutilizar resumos HIR nas rodadas FFI | emissão 108,45 → 97,15 s; poda 25,74 → 17,92 s | médias de duas amostras/versão, debug, cache aquecido; IR idêntico |
| Iterar bits ligados nas raízes GC | emissão 99,62749 → 95,57189 s (−4,1%); etapa LLVM 30,43604 → 26,54618 s | medianas de quatro amostras/versão, debug, ABBA; IR idêntico |
| Validar inventário em streaming + chaves emprestadas | 0,33913 → 0,16486 s; RSS 759.930.880 → 72.794.112 B | etapa isolada com dependências release, não consumo total do compilador |
| Serializar/hash sem strings temporárias completas | pico −136,3 MiB | harness isolado, não memória total do compilador |
| SCC sem copiar adjacências suspensas | 22,723 → 0,810 ms, estrela com 10 mil nós | grafo sintético, não um build inteiro |
| RELR opcional Linux | explorador 37.624.936 → 36.011.472 B (−4,29%) | mesmos insumos de ligação, revisão retida; desligado por padrão |

As rodadas usam bases diferentes: **não somar percentuais**. O iterador de bits
piora o microbenchmark totalmente denso em cerca de 3,09×; isso está preservado.
Referências: [HIR](bench/results/2026-10-20-hir-summary-reuse/README.md),
[raízes](bench/results/2026-10-20-root-bit-iteration/README.md),
[streaming/chaves](bench/results/2026-10-20-borrowed-keys/README.md),
[I/O](bench/results/2026-10-20-inventory-io/README.md),
[SCC](bench/results/2026-10-19-build-quality/README.md),
[RELR](bench/results/2026-10-19-linux-relr-packing/README.md).

### O que mudou por ferramenta nestas 48 horas

| Frente | Feito/corrigido | Evidência e limite |
|---|---|---|
| JS dev | Sessão residente recusa leitura obrigatoriamente inválida de final, preserva última emissão bem-sucedida e refaz estado após falha de escrita. Helper de medição restaura fontes. WebSocket testa o writer real e limites de tamanho UTF-8. Corpus deixa de publicar JS falso e retornar sucesso em erro. | 16 testes dev; 207 entradas aceitas com módulos idênticos antes/depois e uma rejeição intencional. Ganho de velocidade dessas correções não foi medido isoladamente. |
| JS produção | Refatoração interna/Clippy, preservando políticas de RTI, checks, efeitos e otimizações. | 88 testes com ignorados habilitados, incluindo VM/Node/O2. Não houve nova otimização geral de tamanho nesta janela. |
| Analyzer | Cache de dependências do pubspec evita copiar a chave em hits e separa modos normal/dev; testes passam a exigir SDK real. Comparador de paridade valida tipos, coordenadas, duplicatas, versão/saída do oráculo e preserva referências em falhas. | 89 fontes analisadas novamente pelo Dart: 417 diagnósticos iguais em código/local/comprimento. 210 testes analysis/parity e 16 cenários CLI na rodada posterior. Não é paridade completa de mensagens/linguagem. |
| LSP | Completion resolve URIs `file:` percent-encoded uma única vez, incluindo espaços; preserva o prefixo digitado. Contextos nomeados e correção de 16 diagnósticos Clippy. | 258 testes + 17 doctests; seis respostas do servidor Dart 3.6.2 para os cenários de URI. Sem nova medição de latência/memória de servidor grande. |
| Format | Varredura exige SDK/corpus não vazios, propaga erros de leitura, confere cobertura de tokens e recusa fallback escondido para outro SDK. Loader e formatter limpos no Clippy. | 426/426 arquivos aceitos do SDK iguais byte a byte ao `dart format`; 73 testes elements/formatter. Não é nova cobertura de todo Dart 3.13. |
| `dartforge test` | Mantido o runner nativo baseado no Engine de `package:test`; suíte CLI native/JIT ampliada e executada de verdade. Sem nova plataforma de execução de testes nesta janela. | 83 testes CLI em 14 executáveis; totais abrangem outros comandos, não são 83 testes do runner. Browser, coverage, watch, `dart_test.yaml` e anotações de biblioteca continuam sem suporte. |
| `dartforge pub get` | Metadados hosted só substituem o cache depois de validar identidade, versões, pubspec, digest e transporte. Resposta inválida deixa snapshot offline anterior utilizável; erro online continua erro. | 17 invocações reais contra registro loopback: resolução válida, oito rejeições e oito resoluções offline sem HTTP. Fontes/opções fora do subconjunto continuam recusadas. |
| Builders/macros/Sass | Detector usa configuração parseada, não busca textual; IR/digest nativo usa o mesmo gerador do executável; correções de SCC; templates Angular mais compactos; regras `@` modernas de Sass corrigidas. | Corpus nativo de builders e quatro testes Angular passaram na coleta ampla; 372 artefatos Angular conferidos em rodada dirigida. Fallbacks e configurações não suportadas continuam explícitos. |

O cache do analyzer, em 10 mil hits sintéticos de produção com pubspec de
16.464 B, reduziu tráfego de alocação de 165.270.000 para 630.000 B; isso não é
medição de RSS ou tempo do analyzer completo. Os placares anteriores de
23.001/23.012 diagnósticos e os tempos analyze/format de 14/10 continuam
históricos, sem nova varredura integral nesta atualização.

Fontes: [JS](bench/results/2026-10-19-js-quality/README.md),
[dev/produção](bench/results/2026-10-19-production-dev-quality/README.md),
[analysis](bench/results/2026-10-19-diagnostics-quality/README.md),
[paridade](bench/results/2026-10-19-parity-gates/README.md),
[LSP](bench/results/2026-10-19-lsp-uri-paths/README.md),
[LSP qualidade](bench/results/2026-10-19-lsp-quality/README.md),
[format](bench/results/2026-10-19-formatter-loader/checks.json),
[Pub](bench/results/2026-10-19-pub-cache-validation/README.md).
Os totais são de rodadas distintas e sobrepostas: não representam uma soma de
testes únicos ou uma execução única de todos os pacotes no HEAD.

### Nativo, JIT, integração e explorador

- Compilação cruzada real com bundles/sysroots verificados, cinco destinos
  implementados; executados macOS→Linux ARM64, macOS→Windows x64 e
  Linux→macOS ARM64. JIT é local ao sistema, sem execução JIT cruzada.
- Corrigidos alocador/símbolos COFF no JIT Windows, propriedade dos dados de
  contexto do SDK importado e seleção explícita de SDK em run/recarga/reinício.
  O último teste envenena o SDK ambiente e prova que `--sdk` correto prevalece.
- Backend: downcasts de literais preservando ordem de efeitos; proteção contra
  otimização de recursão que impedia StackOverflowError; reutilização de
  comprimento de strings com dominância; dominadores densos e ordem reversa
  de pós-visita; resumos HIR reutilizados; registros de tabelas em lote;
  metadados de frames async isolados da descoberta de event loops.
- Poda/auditoria: leitores limitados de ELF/RELR, Mach-O/relocações clássicas e
  chained fixups, PE, bitcode e archives; distinção entre ancestral nominal e
  classe alocável; tabelas de Type; invalidação de certificados; limites de
  entradas/callbacks e provas de footprints de alocação sem conceder pureza ou
  permissão de poda. Literal LLVM contendo `@nome` deixa de criar aresta falsa.
- Toolchain: descoberta LLVM portátil, ligação macOS compatível com bitcode 22,
  29 símbolos adicionais de frameworks Apple, pré-checagem de SDKs-oráculo,
  catálogo/cache alinhados às políticas de otimização e rooting. Fontes Rust
  globais formatadas em commit isolado, sem afrouxar regras.
- No dart_ui: bordas finas, transparência/cantos de popups, hover isolado do pai,
  menu retargetado com um clique secundário, seleção pelo ponto clicado e menu
  da área vazia; rename permanece ao selecionar texto; propriedades em janela
  independente sem mover o pai; tooltips de arquivos opt-in e apresentação
  configurável; opções de pasta/checkbox; abas integradas ao título Windows e
  Linux; cursor/caption Windows; sidebar redimensionável, colunas configuráveis
  e filtros múltiplos por faixa do nome. O inventário dos 20 commits acima
  registra essas implementações; não equivale a teste visual exaustivo.

Última matriz real, concluída antes deste balanço:

| Ambiente | Explorador AOT | Explorador JIT | Menu AOT/JIT |
|---|---|---|---|
| macOS ARM64 / AppKit / CPU | passou | passou | passou / passou |
| Ubuntu ARM64 / Wayland / OpenGL | passou, cruzado | passou | passou / passou |
| Windows 11 ARM64 / executáveis x64 / D3D11 | passou, cruzado | passou | passou / passou |

Fontes: 1.151 arquivos Dart da UI e 427 entradas verificadas do SDK coincidem
nos três sistemas. Smokes curtos de dois frames e eventos enviados à aplicação,
com popups nativos reais: **não certificam mouse físico, todos os botões,
aparência, sessões longas ou todas as GPUs**. Windows ARM64 nativo continua
sem destino implementado. VMs estão suspensas; este balanço não instalou agente
residente nem reiniciou hóspedes.
[Matriz e hashes](bench/results/2026-10-20-jit-sdk-selection/README.md).
Ao revisar os comandos, corrigiu-se também o validador PowerShell para colocar
`--sdk`/`--packages` antes da entrada em `run`, como já fazia o Python corrigido.
Essa linha recebeu validação de sintaxe/ordem; não se declara uma nova matriz GUI.

### Estado dos checks e o que falta

- Últimos fmt, Clippy estrito do workspace e CLI native/JIT, e documentação
  privada sem avisos: **passaram**. Backend nativo: última suíte dirigida
  completa com **1.046 aprovados**, incluindo ignorados. Runtime: 324 unitários,
  25 integrações e 88 doctests na rodada registrada.
- A coleta ampla do workspace terminou em **101**, não aprovada. Um alvo falhou
  por SDKs-oráculo não configurados e os dois testes passaram na reprise do mesmo
  executável com os SDKs corretos. O outro permanece falhando de verdade:
  `platform_constant_prunes_the_other_backends`, com seis definições estrangeiras
  ainda retidas. Fmt/Clippy verdes não anulam essa falha.
- Prioridade nativa: completar as provas de startup/registro e a poda de backends
  inativos; reduzir o explorador, ainda 2,06× o Dart no macOS; melhorar árvores e
  kernel de cor; medir HTTP e aplicações longas novamente; repetir a suíte ampla
  após corrigir o gate. Não suprimir a expectativa para conseguir verde.
- JS dev: nova comparação justa de worker residente/incremental e checkout limpo
  nos dois projetos grandes; dependências da geração/builders, tráfego HTTP e
  invalidação de API ainda merecem auditoria. A medição pequena acima não encerra
  essas pendências. JS produção: reduzir runtime/RTI/gzip e regressões de JSON,
  UTF-8 e coleções; não trocar checks por velocidade silenciosamente.
- Analyzer/LSP/format: ampliar paridade 3.13/augmentations, mensagens, referências
  e sessões grandes/incrementais. Sem novo placar global nesta janela.
- Pub/test: ampliar fontes/opções de resolução, configuração de suites e
  plataformas recusadas; executar esses fluxos nas VMs, além dos smokes desktop.
- Distribuição: compatibilidade com sistemas antigos, recursos/host da UI e
  validação visual/longa continuam necessários. O aviso de deployment target dos
  objetos LLVM não foi convertido em certificação de macOS antigo.

### Executar com Parallels Virtual Machines Control nesta máquina

Controlador instalado: `/usr/local/bin/prlctl` (26.3.3). Os caminhos abaixo são
os desta instalação. Executar uma VM por vez; seus 4 GB não comportaram build
pesado e JIT simultâneos. `exec` administrativo não garante sessão gráfica:
Windows usa `--current-user`, Linux usa o usuário `isaque` e a sessão Wayland.

```sh
prlctl list -a -o name,status
prlctl start 'Ubuntu24.04_arm64'
prlctl exec 'Ubuntu24.04_arm64' /bin/uname -m
# Ao terminar os processos/testes:
prlctl suspend 'Ubuntu24.04_arm64'

prlctl start 'Windows11'
prlctl exec 'Windows11' --current-user whoami.exe
```

Linux: compilar/executar o explorador e o smoke em AOT/JIT, dentro da sessão
gráfica já iniciada. A saída é nova e não substitui a evidência anterior.

```sh
prlctl exec 'Ubuntu24.04_arm64' /usr/sbin/runuser -u isaque -- \
  /usr/bin/env XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=wayland-0 \
  DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus \
  DARTFORGE_LLVM_DIR=/opt/llvm-22.1.8 \
  PATH=/opt/llvm-22.1.8/bin:/usr/local/bin:/usr/bin:/bin \
  python3 /media/psf/ssd_dock/MyRustProjects/dartforge/scripts/validate-native-desktop.py \
  --compiler /home/isaque/projects/dartforge/target/native-validation/release/dartforge-current-desktop-fixed \
  --ui-root /home/isaque/projects/dart_ui --sdk /opt/dart/3.6.2/dart-sdk/lib \
  --out /home/isaque/.cache/dartforge/review-20261020 \
  --backend wayland --presentation wayland-opengl --mode all
```

Windows: com o usuário conectado e LLVM x64 completo instalado:

```sh
prlctl exec 'Windows11' --current-user powershell.exe -NoProfile \
  -ExecutionPolicy Bypass \
  -File '\\Mac\ssd_dock\MyRustProjects\dartforge\scripts\validate-native-desktop.ps1' \
  -Compiler 'C:\tools\dartforge\target\native-x64\x86_64-pc-windows-msvc\release\dartforge-current-desktop.exe' \
  -UiRoot 'C:\tools\ui' -Sdk 'C:\tools\dart\3.6.2\dart-sdk\lib' \
  -LlvmDirectory 'C:\tools\llvm-22.1.8' \
  -Out 'C:\tools\validation\review-20261020' -Mode all
# Ao concluir:
prlctl suspend 'Windows11'
```

Para deixar o explorador aberto, execute diretamente na VM, removendo
`--frames=2`; opções do compilador ficam **antes** do arquivo em `run` e
argumentos do explorador ficam depois de `--`:

```sh
# Terminal Linux dentro da sessão gráfica:
export DARTFORGE_LLVM_DIR=/opt/llvm-22.1.8
export PATH=/opt/llvm-22.1.8/bin:$PATH
export DART_UI_FONTS=/home/isaque/projects/dart_ui/assets/fonts
cd /home/isaque/projects/dart_ui/examples/file_explorer
/home/isaque/projects/dartforge/target/native-validation/release/dartforge-current-desktop-fixed \
  run --sdk /opt/dart/3.6.2/dart-sdk/lib \
  --packages .dart_tool/package_config.json bin/file_explorer.dart -- \
  "$PWD" --lang=pt --backend=wayland --presentation=wayland-opengl
```

```powershell
# PowerShell dentro da sessão Windows:
$env:Path='C:\tools\llvm-22.1.8\bin;'+$env:Path
$env:DARTFORGE_LLVM_DIR='C:\tools\llvm-22.1.8'
$env:DART_UI_FONTS='C:\tools\ui\assets\fonts'
Set-Location C:\tools\ui\examples\file_explorer
& 'C:\tools\dartforge\target\native-x64\x86_64-pc-windows-msvc\release\dartforge-current-desktop.exe' `
  run --sdk C:\tools\dart\3.6.2\dart-sdk\lib `
  --packages .dart_tool\package_config.json bin\file_explorer.dart -- `
  C:\tools\ui\examples\file_explorer --lang=pt --backend=win32 --presentation=direct3d11
```

Compilar no macOS para o destino, sem delegar a compilação à VM:

```sh
export DARTFORGE_LLVM_DIR=/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64
export PATH="$DARTFORGE_LLVM_DIR/bin:$PATH"
forge="$PWD/target/test-tools/dartforge-explicit-sdk"
example=/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer
for target in linux-arm64 windows-x64; do
  suffix=''; [ "$target" = windows-x64 ] && suffix=.exe
  "$forge" compile-native "$example/bin/file_explorer.dart" --optimize \
    --sdk /Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib \
    --packages "$example/.dart_tool/package_config.json" \
    --target "$target" --target-bundle "$PWD/target/cross-bundles/current/$target" \
    -o "$PWD/target/explorer-review-$target$suffix"
done
```

Copiar o executável à VM e conferir SHA-256 antes de executar; fornecer as
fontes/recursos do destino com `DART_UI_FONTS`. Bundles precisam corresponder às
fontes atuais; após alteração do runtime, executar `dartforge export-target`
em diretório novo no destino e trazer o bundle ao host. Nunca editar o manifesto
para forçar compatibilidade. Para reconstruir o compilador, sysroots e detalhes
dos cinco destinos, consultar [CROSS-COMPILACAO](docs/CROSS-COMPILACAO.md).
Os comandos desta seção documentam reprodução; não foram todos reexecutados
durante este balanço. A matriz efetivamente executada está vinculada acima.

## Atualização de 25–26/10/2026: compilador JS

Foco do dia: item 1 do plano de produção (JS), na ordem combinada. Oito
commits, de `0592caa7` a `4d3ace5c`, todos validados localmente no macOS
arm64 e enviados.

### Medição

`tests/language` contra a VM 3.6.2, binário congelado após `afdac406`, no
conjunto de 2266 programas usado nas medições anteriores:

| Ferramenta | Iguais à VM |
|---|---:|
| DartForge JS dev, antes (`30ddcc54`) | 2017 |
| DartForge JS dev, agora | **2128** |
| DDC 3.6.2 oficial (`dartdevc` + Node) | 2227 |

Nenhuma regressão em relação à rodada anterior. O DDC também passou a ser
medido na suíte inteira (`run_lang.py ddc`), para separar divergências
nossas das que o próprio DDC tem: das falhas restantes, 113 são só nossas
(o DDC acerta) e 26 o DDC também erra. Os commits posteriores à medição
(`c25d7778`, `4d3ace5c`) corrigem mais cerca de 20 programas em subconjuntos
(patterns, switch, records, nomes que colidem com JS), ainda sem rodada
completa. Corpus JS: **252/252** em dev e em produção (eram 247).

### O que entrou

- Verificações de covariância como o DDC: parâmetros genérico-covariantes e
  `covariant` no prólogo, setter de campo covariante, leitura de membro
  contravariante, assinaturas reificadas com `Object?`.
- *Forwarding stubs* de covariância: implementação herdada (da superclasse
  ou de mixin) que não verifica um parâmetro covariante na interface da
  classe ganha um stub que verifica contra o tipo da implementação.
- Resolução de nomes dentro de extensões como o CFE (membros da própria
  extensão, do tipo `on`, de outras extensões; estáticos `get#`/`set#`),
  extensão explícita em compostos e índices, `?.` em compostos, `~`/`call`
  por extensão, limites de extensão, `show`/`hide` nos imports.
- `late` como o CFE: tipo anulável com flag `#isSet`, `fieldADI`, setter
  herdado para `late final` com inicializador, assinaturas para acesso
  dinâmico.
- Padrões: expressão de padrão constante canônica, variáveis por cabeça em
  casos compartilhados, escopos de `case`/`if-case`, contexto do padrão no
  inicializador.
- JS válido para nomes que colidem: construtores `length`/`caller`/
  `arguments` com `dart.defineValue` (lido também pelo verificador de
  produção), classe com nome reservado, locais × parâmetros `this.y`,
  temporários de inicializadores de campo, hexadecimal acima de 64 bits.
- Instanciação implícita/explícita de função estática com argumentos
  constantes é constante (`identical`).

### Pendências observadas

- JS: contexto de record literal em padrões de declaração
  (`patterns/call_tear_off_test`), limites de métodos genéricos em tear-off e
  chamada dinâmica (`covariant/type_parameter_test`,
  `subtyping_test.testGenericMethodBounds`), tipos de tear-off com
  parâmetro covariante (`regress31596_*`), promoção de campos
  (`inference_update_2`), `private/*`, `deferred/*`.
- Analisador (item 4): não rejeita implementação herdada que não conforma
  com a interface (`D extends C implements I` com parâmetro nomeado), nem
  parâmetros `_` duplicados antes da 3.7; falsos avisos de
  `type_argument_not_matching_bounds` com tipos de função genéricos e de
  `return_of_invalid_type` em `this.call` com `T extends Function`.
- Próximos itens do plano sem mudança hoje: AOT nativo (2160/2266, rodadas
  Linux/Windows), pub/test, analisador, Wasm, iOS/Android.

## Atualização de 20/10/2026

- Corrigida a seleção do SDK na biblioteca carregada por `run` e pela recarga:
  `--sdk` já valia na emissão, mas a carga redescobria o SDK do ambiente. O
  reinício agora encaminha a seleção ao filho que executa o IR. A regressão
  falhou antes com um SDK ambiente inválido e passou nos três modos após a
  correção, preservando argumentos do aplicativo. O validador desktop também
  corrigiu a ordem de `--sdk`/`--packages`: depois da entrada eram argumentos
  do aplicativo, e o SDK ambiente podia esconder isso. Passaram 83 testes CLI
  native/JIT em 14 executáveis, todos os ignorados incluídos, Clippy estrito,
  docs e fmt. Explorador real e menus passaram em AOT/JIT nos três sistemas;
  os AOT foram compilados no macOS com o backend `6ab2194e`, que não mudou.
  Os JIT finais usam a correção e SDK ambiente inválido, com a seleção explícita
  correta; run/recarga/reinício também passaram nas duas VMs. Fontes Dart,
  manifests/locks e os 427 arquivos verificados do SDK coincidem nos três.
  São verificações curtas de frames/popups, sem auditoria visual ou de todas
  as interações físicas. Falhas iniciais de empacotamento e de permissão do
  helper ficaram registradas. Sem mudança de runtime/bundles; VMs suspensas.
  A poda dos backends e a aprovação integral continuam pendentes. Evidências:
  `bench/results/2026-10-20-jit-sdk-selection/README.md`.

- O iterador privado dos conjuntos de raízes do GC percorre agora só os bits
  ligados, mantendo ordem crescente e o mesmo resultado de liveness/slots.
  Após profiling do explorador real, duas rodadas ABBA com cache aquecido,
  quatro amostras por versão e compiladores debug reduziram a mediana de
  emissão de IR completa de 99,62749 para 95,57189 s (4,1%); a etapa LLVM
  caiu de 30,43604 para 26,54618 s (12,8%). Os 1.082 vínculos de fontes e
  os 293.412.654 bytes de IR são idênticos nas dez execuções, incluindo
  aquecimentos. Não é medição de compilador release, Clang/link ou execução
  do aplicativo. O microbenchmark release de redução de checksum melhora
  conjuntos vazios/esparsos, mas piora os totalmente densos em cerca de 3,09×;
  essa limitação fica registrada, sem alegação de ganho universal. Passaram
  a equivalência do iterador, 1.046 testes nativos com ignorados, Clippy
  estrito, docs, runtime e fmt. Fixture AOT produção/desenvolvimento/sem poda
  e JIT macOS passou normal/sob estresse, com os três IRs montados idênticos.
  Sem mudança de runtime/bundles ou nova matriz dos hóspedes; VMs suspensas.
  O gate original de backends continua falhando, sem alterar a expectativa.
  Evidências: `bench/results/2026-10-20-root-bit-iteration/README.md`.

- Corrigidas referências falsas do leitor LLVM de poda: `c"@nome"` contém
  bytes, não um endereço. Referências reais no mesmo agregado, identificadores
  entre aspas e o tratamento conservador de assembly/literais incompletos ficam
  preservados. As duas regressões novas falharam antes e passaram depois;
  a visão de provas não precisa mais limpar essas arestas após a leitura.
  A observação do SDK remove 171 ocorrências falsas em 15 módulos; não são
  171 funções removidas. No par de caches com IR idêntico, todos os 32.713
  registros de funções permanecem iguais e a cobertura de alocação não muda.
  O grafo real da fixture perde quatro símbolos inexistentes; tabelas, pares
  e os três IRs montados permanecem iguais. Passaram 1.045 testes nativos com
  ignorados, Clippy estrito, docs, runtime e fmt. A primeira rodada teve uma
  falha de teste que exigia o comportamento antigo; log preservado e teste
  ajustado para exigir ausência da aresta já no resumo original, mantendo
  imutabilidade. Fixture aprovada em AOT/JIT macOS e AOT cruzado Windows/Linux,
  normal/sob estresse; cópias conferidas por hash e VMs novamente suspensas.
  Sem alteração do runtime/bundles ou ganho de tamanho/desempenho alegado.
  O gate original continua falhando e os dois modos retêm seis definições
  estrangeiras. Evidências:
  `bench/results/2026-10-20-byte-literal-pruning/README.md`.

- Corrigida a formatação global no commit isolado `1013f38d`: 700 arquivos Rust
  processados pelo formatter fixado no projeto, sem exclusões nem configuração
  nova. `cargo fmt --all -- --check` agora passa. Passaram Clippy estrito do
  workspace e com LLVM embutido, docs sem avisos, 324 unitários/25 integrações/
  88 doctests do runtime e 1.043 testes nativos com ignorados. Fixture aprovada
  em AOT produção/desenvolvimento/sem poda e JIT macOS, normal/sob estresse;
  os três IRs montados ficaram idênticos aos anteriores. Os fingerprints de
  bytes mudaram: bundles Linux/Windows reexportados nas VMs, com vínculo de
  fontes igual ao host. O compilador atual compilou a fixture para os dois
  destinos; passou nas VMs normal/sob estresse, com cópias conferidas por hash.
  Caminhos current promovidos atomicamente, bundles anteriores preservados e
  ambas as VMs suspensas. Evidências em
  `bench/results/2026-10-20-workspace-format/README.md`.
  Sem nova matriz GUI/JIT dos hóspedes ou ganho de tamanho/desempenho alegado.
  O gate original foi repetido após a formatação e ainda retorna 101 por manter
  corpos WindowsBackend no IR macOS, com log preservado e sem alterar a
  expectativa. A poda dos backends e a aprovação integral continuam pendentes.

- Concluída a validação CLI com `native,jit` e todos os ignorados: 82 testes
  passaram em 14 executáveis, zero falhas/ignorados/filtros, saída 0 em 361,75 s
  incluindo compilação. Cobriu sete regressões de I/O (TLS/HTTPS/sinais/sockets),
  quatro de execução JIT, três de memória, sete de recarga com estado e geração
  nativa/IR, além dos demais testes CLI. As dependências do caso i18n estavam
  presentes; os testes não retornaram por configuração ausente. Há sobreposição
  com a rodada básica, não são 82 testes distintos adicionais. Checker e
  CONTRIBUTING incluem agora essa segunda rodada explicitamente. Logs, ambiente
  e hashes em `bench/results/2026-10-20-cli-native-jit/README.md`. Sem alteração
  Rust/runtime/bundles ou nova matriz de VMs. Poda dos backends, fmt global e
  aprovação integral continuam pendentes.

- Terminou a passada integral do workspace com ignorados e sem filtros:
  2.978,67 s, saída 101, exatamente dois alvos falhos. O corpus diferencial
  estava sem os SDKs de referência configurados; o mesmo binário passou os dois
  testes com SDKs macOS 3.6.2/3.13.5 em diretório separado. O gate de ausência
  dos backends continua falhando, sem alterar a expectativa. Passaram os 1.043
  testes nativos, corpus de builders, quatro testes Angular e executor nativo
  de macros JSON. O comando básico não habilita os testes CLI native/JIT.
  O checker agora verifica os SDKs de referência antes do build: aceita o par
  válido e recusa SDK ausente, de outro sistema ou versão de piso incorreta.
  CONTRIBUTING documenta as variáveis separadas do SDK de fontes e PATH.
  Logs completos e falhas originais preservados em
  `bench/results/2026-10-20-workspace-complete/README.md`. Fmt global, poda dos
  backends e aprovação integral continuam pendentes.

  A rodada adicional CLI native/JIT terminou com sucesso; resultado final e
  evidências no item acima.

- Revalidado o JIT completo atual: 92 testes regulares e seis doctests passaram,
  incluindo todos os ignorados. O IR AOT podado antigo e seu SDK original também
  passam no executor atual normal/sob estresse, com a saída 35294 da VM Dart.
  A falha já estava corrigida por `a5e0962f`; o relatório histórico foi atualizado
  para apontar essa correção, preservando as falhas originais. O executor
  preservado disponível já tem hash diferente do binário que falhou inicialmente;
  não foi apresentado como controle negativo. O script local e CONTRIBUTING
  agora usam `--no-fail-fast`, como o CI, para executar os demais alvos após uma
  falha sem esconder o código de erro final. Sintaxe PowerShell validada. Iniciada
  uma passada integral do workspace com ignorados, sem exclusões/filtros e com
  dois jobs/threads; logs em `/tmp/forge-workspace-complete`, ainda em andamento.
  Isso não é aprovação integral: o gate de backends e fmt global seguem abertos.
  Sem alteração de fontes Rust/runtime/bundles ou nova matriz de hóspedes.
  Evidências: `bench/results/2026-10-20-jit-workspace-checks/README.md`.

- O auditor reconhece agora escritas em subobjetos tipados e delimitados de
  `alloca`: escalares, arrays, estruturas, GEPs encadeados e decaimento exato de
  array. Sem calcular offsets/padding; tipos e índices constantes devem caber.
  Destinos desconhecidos, reinterpretações, limites, SSA duplicado e estouro dos
  orçamentos continuam sem prova. `alloca` isolado agora também mantém exposição
  de ponteiro, corrigindo essa omissão; consumidor recusa endereços da pilha como
  entradas/imports/nós. Duas regressões falharam antes; passaram cinco testes
  novos e 1.043 nativos com ignorados, Clippy estrito e docs sem avisos. Clang
  aceitou os seis casos positivos. Na observação por instrução do SDK real,
  200.544 stores e 88.310 GEPs foram reconhecidos; nenhum registro final de função
  mudou, porque restam outras escritas desconhecidas no mesmo corpo. Permanecem
  649 closures conhecidas, sem ganho de poda/tamanho/desempenho alegado. AOT/JIT
  macOS passou normal/sob estresse e três IRs montados ficaram idênticos. VMs
  mantidas suspensas. Gate original repetido e ainda falha; ambos os modos retêm
  seis definições estrangeiras. Fmt global e aprovação integral seguem pendentes.
  Evidências: `bench/results/2026-10-20-stack-storage/README.md`.

- O inventário representa agora explicitamente a leitura LLVM do ponteiro de
  pilha (`sp`/`rsp`, i64 e um metadado inline exato), com footprint vazio da
  operação e exposição de registrador. Metadados gerais, outras formas e
  definições que sombreiam o intrínseco continuam sem prova. O consumidor valida
  a exceção de ABI e recusa entradas/imports/nós mesmo sem a marca de exposição.
  A regressão positiva falhou antes; passaram quatro testes novos, 1.038 nativos
  com ignorados, Clippy estrito e docs sem avisos. Clang aceitou os dois formatos.
  No SDK real, 27.067 funções ganharam registro explícito da chamada e 17.929
  perderam a marca genérica de ABI desconhecida. Mesmas identidades/digests de
  corpos, módulos e globais; comparação exaustiva das demais observações. As
  closures de alocação conhecidas continuam 649: ainda faltam as provas dos
  outros efeitos dos helpers. Fixture AOT/JIT macOS passou normal/sob estresse;
  três IRs montados idênticos. VMs mantidas suspensas, sem nova matriz de hóspedes.
  Gate original de ausência de backends repetido e ainda falha; ambos os modos
  mantêm seis definições estrangeiras. Fmt global e aprovação integral continuam
  pendentes; sem ganho de desempenho/tamanho alegado. Evidências:
  `bench/results/2026-10-20-stack-read/README.md`.

- O verificador canônico empresta agora chaves JSON sem escapes do buffer de
  entrada, evitando uma String por chave; escapes preservam cópia decodificada
  própria durante a leitura de valores aninhados. Sem alterar ordenação,
  duplicatas, hashes, schema ou consumidor de fatos. Passaram a regressão nova,
  1.034 testes nativos com ignorados, Clippy estrito e docs sem avisos. No ensaio
  isolado de 70 MB, seis amostras por variante em ABBA reduziram a mediana de
  1,08573 para 1,03803 s com dependências debug (4,4%); com dependências release,
  0,20487 para 0,16620 s (18,9%), mantendo aproximadamente 69 MiB de pico RSS.
  Um controle release separado da árvore completa anterior mediu 0,33913 s e
  aproximadamente 725 MiB, contra 0,16486 s/69 MiB no fluxo final: esse último
  ganho combina os dois incrementos. A troca de tempo registrada no incremento
  anterior vale para seu harness com dependências debug, não para release nem
  para o compilador inteiro. AOT/JIT macOS passou normal/sob estresse; três IRs
  montados e payload do SDK idênticos. VMs mantidas suspensas, sem nova matriz
  de hóspedes. Gate de ausência de backends repetido e ainda falha; seis definições
  estrangeiras, fmt global e aprovação integral continuam pendentes. Evidências:
  `bench/results/2026-10-20-borrowed-keys/README.md`.

- A verificação de integridade do cache evita agora a árvore JSON completa dos
  fatos: compara o formato canônico em fluxo e projeta só o envelope; o consumidor
  completo permanece inalterado. Mesmos hashes, schema e contrato de integridade,
  sem nova permissão de poda. Três regressões novas, incluindo 14 variantes de
  envelope comparadas ao verificador anterior; passaram 1.033 testes nativos com
  ignorados, Clippy estrito e docs sem avisos. No ensaio isolado do inventário de
  70 MB, seis amostras por variante em ordem ABBA reduziram a mediana do pico RSS
  de 759.996.416 para 72.941.568 bytes (menos 655,23 MiB/90,4%), mas aumentaram o
  tempo de 0,86049 para 1,12418 s (30,6%). É uma troca de memória por tempo nessa
  etapa, não ganho geral de velocidade. Fixture aprovada em AOT/JIT macOS e AOT
  cruzado Linux/Windows, normal/sob estresse; cinco IRs montados e os payloads dos
  três SDKs idênticos. VMs novamente suspensas. Gate original de ausência de
  backends repetido e ainda falha; seis definições estrangeiras, fmt global e
  aprovação integral continuam pendentes. Evidências:
  `bench/results/2026-10-20-stream-verify/README.md`.

- A integridade do cache usa agora buffers de 64 KiB para serializar/comparar
  JSON e calcular hashes de arquivos, evitando cópias integrais de saída.
  O formato canônico, LF único, vínculos de artefatos/runtime/compilador e recusa
  de duplicatas permanecem; o marcador só é publicado após sucesso do payload.
  Passaram quatro testes novos e 1.030 nativos com ignorados, Clippy estrito e
  docs sem avisos. No ensaio isolado com inventário real, seis amostras por
  variante/etapa após aquecimento e ordem ABBA mostraram reduções medianas de
  pico RSS de 136,31 MiB na comparação e 136,28 MiB na escrita/hash. Isso não é
  medição de memória do compilador inteiro; tempos próximos não demonstram ganho
  geral de velocidade. Payloads dos três SDKs e cinco IRs montados idênticos;
  fixture aprovada em AOT/JIT macOS e AOT Linux/Windows, normal/sob estresse.
  VMs novamente suspensas. Sem mudança de runtime/bundles/schema ou permissões
  de poda. O gate original ainda falha com seis definições estrangeiras; fmt
  global e aprovação integral continuam pendentes. Evidências:
  `bench/results/2026-10-20-inventory-io/README.md`.

- O auditor separa leituras/cálculos de endereço de escritas que podem implementar
  alocação. Formas escalares completas ganham footprint vazio, mantendo exposição
  de ponteiro; stores, chamadas opacas e formas não suportadas continuam abertas.
  Corrigida também uma falha que ignorava stores na mesma linha após aritmética
  ou leitura, inclusive sem espaço após números/SSA entre aspas. O Clang confirma
  a validade das regressões, e o consumidor continua recusando cobertura/calls de
  ponteiros e Ref. Os footprints conhecidos cresceram em 206 por SDK: 443→649
  macOS, 444→650 Linux e 428→634 Windows, sem perdas e com exposição transitiva
  preservada em todos os ganhos. Corpos idênticos, conferidos como multiconjunto
  por módulo/símbolo/digest; a primeira junção por chave ambígua foi corrigida no
  harness, preservando a falha. Passaram cinco testes novos e 1.026 nativos com
  ignorados, Clippy estrito e docs sem avisos. A fixture de dois isolados passou
  em AOT/JIT macOS e AOT Linux/Windows, normal/sob estresse; cinco IRs montados
  idênticos e VMs novamente suspensas. Sem mudança de runtime/bundles/schema ou
  ganho medido. O gate de ausência de backends foi repetido e falhou; seis
  definições estrangeiras, fmt global e aprovação integral continuam pendentes.
  Evidências: `bench/results/2026-10-20-read-only-allocation/README.md`.

- O inventário reconhece agora footprint vazio de alocação Dart nos leitores
  exatos de contexto TSD/TLS. Os padrões revisados são independentes do gerador;
  alterações futuras no helper não ampliam a prova automaticamente. Identidade,
  armazenamento, ABI, aliases e assembly opaco são conferidos; exposições de
  ponteiro/memória permanecem, e o consumidor continua recusando cobertura de
  nós, chamadas e entradas. No SDK real, os footprints conhecidos passaram de
  428 para 443 no macOS e de 429 para 444 no Linux: somente os 15 leitores,
  com corpos idênticos. Windows mantém 428 e não emite esse helper; a expectativa
  inicial de 15 foi corrigida no harness, preservando a falha. Passaram seis
  testes novos e 1.021 nativos com ignorados, Clippy estrito e docs sem avisos.
  A fixture de dois isolados coincidiu com a VM em AOT/JIT macOS e AOT cruzado
  Linux/Windows, normal/sob estresse. Os cinco IRs montados comparados ficaram
  idênticos; VMs novamente suspensas. Sem mudança de runtime/bundles ou permissão
  de poda, nova matriz GUI/JIT dos hóspedes ou ganho medido. O gate original de
  ausência de backends foi repetido e falhou; continuam seis definições
  estrangeiras, além de fmt global e aprovação integral pendentes. Evidências:
  `bench/results/2026-10-20-context-allocation/README.md`.

- A inserção de `nounwind`, `optsize` e `uwtable` agora reconhece tokens e o
  sufixo do cabeçalho LLVM: palavras dentro de nomes, parâmetros, atributos
  entre aspas, comentários e corpos inline não alteram o ponto de inserção.
  Cabeçalhos multilinha e cláusulas finais são preservados, e atributos
  explícitos não são duplicados na reaplicação. Quatro regressões falharam
  antes; cinco testes novos passaram, incluindo compilação LLVM real antes/depois.
  Passaram 1.015 testes nativos com ignorados, três exemplos de documentação,
  Clippy estrito e docs sem avisos. A fixture coincidiu com a VM em AOT macOS
  produção/desenvolvimento/sem poda e JIT macOS, normal/sob estresse, e em AOT
  cruzado Linux ARM64/Windows x64 nas duas condições. O IR montado dessa fixture
  permaneceu idêntico nos três perfis macOS. VMs novamente suspensas; sem mudança
  de runtime/bundles, nova matriz GUI/JIT dos hóspedes, permissão de poda ou ganho
  geral medido. Fmt global, aprovação integral e poda dos backends continuam
  pendentes. Evidências: `bench/results/2026-10-20-function-attributes/README.md`.

- Corrigida a substituição textual do contexto AOT: uma string Dart contendo
  `call ptr @dartforge_context()` era alterada e fazia Clang rejeitar o tamanho
  do inicializador. O scanner agora preserva strings, comentários, identificadores
  e assembly entre aspas. Um helper já definido, inclusive com nome escapado,
  preserva a primeira configuração; a reaplicação não reescreve seu caminho
  lento nem duplica a definição. Duas regressões falharam antes; passaram quatro
  testes novos e 1.010 testes nativos com ignorados, Clippy estrito e docs sem
  avisos. A fixture real coincidiu com a VM Dart em AOT macOS produção,
  desenvolvimento e sem poda, JIT macOS e AOT cruzado Linux ARM64/Windows x64,
  normal/sob estresse de GC. Cópias nas VMs conferidas por hash e ambas novamente
  suspensas. Sem mudança de runtime/bundles, nova matriz GUI/JIT dos hóspedes,
  permissão adicional de poda ou ganho de desempenho medido. Fmt global,
  aprovação integral e poda dos backends continuam pendentes. Evidências:
  `bench/results/2026-10-20-context-call-rewrite/README.md`.

- O registro das tabelas do SDK montadas no link agora usa o verificador dos
  getters constantes antes de formar um lote. São 662 registros para uma
  chamada na fixture real, preservando ordem, CIDs e tabelas vazias; o caminho
  escalar continua para corpos sem prova ou CIDs divergentes. O mínimo de oito
  entradas é compartilhado com o emissor do programa. Expandir o lote torna o
  IR completo idêntico ao anterior, com/sem poda de tabelas; cada IR montado caiu
  30.861 bytes. Passaram quatro testes novos e 1.006 testes nativos com ignorados,
  Clippy estrito e docs sem avisos. A fixture coincide com a VM em AOT macOS
  produção/desenvolvimento/sem poda e JIT macOS, normal/sob estresse; o AOT cruzado
  passou nas duas condições em Windows x64/Linux ARM64. Explorador e smoke de
  menus passaram em AOT nos três sistemas; VMs novamente suspensas. A medição
  local ABBA de processo inteiro teve medianas 6,3143/6,2915 ms, sem ganho de
  tempo demonstrado. A fixture AOT comum manteve tamanho; sem poda, caiu 16.512
  bytes. Uma recópia do compilador pelo harness causou recusa de assinatura no
  macOS; a publicação atômica dos mesmos bytes resolveu e as execuções foram
  repetidas, preservando as falhas. Sem mudança de runtime, bundles/schema ou
  permissão de poda; o teste de ausência de backends foi repetido e falhou.
  Fmt global e aprovação integral seguem pendentes. Evidências:
  `bench/results/2026-10-20-sdk-table-batches/README.md`.

- O inventário agora observa um footprint finito para `dartforge_object_new_t`
  e `_mt` quando o getter completo e sua tabela constante estão presentes e
  conferidos: corpo sem efeitos, símbolos únicos (inclusive aspas/escapes LLVM),
  armazenamento delimitado, CID literal coincidente, ABI e mapa de referências.
  As exposições de ponteiros/registro continuam explícitas; o consumidor recusa
  cobertura de nós, raízes e chamadas escalares/Ref/ptr sem provas próprias.
  A regressão positiva falhou antes e passou depois. Passaram seis testes novos,
  1.002 testes nativos com ignorados, Clippy estrito e docs sem avisos; a fixture
  de dois isolados coincidiu com a VM em AOT produção/desenvolvimento e JIT macOS,
  normal/sob estresse de GC. O SDK real tem 32.708 funções e nenhum corpo desses
  getters, que só será gerado em `assemble`; essa ausência não foi preenchida
  por uma declaração presumida. O IR de plataforma permaneceu idêntico, com
  seis definições estrangeiras, e o teste de ausência foi repetido e falhou.
  Sem nova autorização de poda, ganho medido, mudança de runtime/schema ou
  nova matriz Windows/Linux; fmt global e aprovação integral seguem pendentes.
  Falhas preliminares preservadas. Evidências:
  `bench/results/2026-10-20-table-allocation-footprints/README.md`.

- O startup de programas com pelo menos oito tabelas ativas agora usa um vetor
  ordenado de getters puros. No explorador real, 3.758 chamadas viraram uma;
  CIDs e ordem conferidos, e o IR é idêntico ao anterior após expandir o lote.
  O IR caiu 180.483 bytes; executáveis macOS/Windows diminuíram 33.024/25.600
  bytes e Linux cresceu 944 bytes, sem ganho universal ou de startup medido.
  Corrigido também o build script que podia ler o checkout antigo quando Cargo
  reutilizava seu binário: a regressão falha antes e passa depois. Bundles
  Windows/Linux reexportados das fontes finais, com hashes iguais e caminhos
  `current` atualizados; os anteriores foram preservados. Passaram 996 testes
  nativos com ignorados, 324 unitários/25 integrações/88 doctests do runtime,
  Clippy estrito e docs sem avisos. A fixture com dois isolados passou
  em AOT/JIT nos três sistemas, normal/sob estresse de GC; o explorador e o
  smoke de menus também passaram em AOT/JIT nos três, em execuções curtas.
  Duas montagens release Windows falharam ao abrir arquivo do `ring`; uma
  montagem em diretório isolado curto passou, sem causa definitiva atribuída.
  Falhas preliminares e correções de transporte/permissão/presentation foram
  preservadas. As VMs voltaram ao estado suspenso; nenhum agente periódico
  foi instalado. Sem nova permissão de poda: seis definições estrangeiras,
  fmt global e aprovação integral ainda pendentes. Evidências:
  `bench/results/2026-10-20-table-batches/README.md`.

- A visão de prova de classes limpa agora referências lexicais falsas dentro
  de bytes privados verificados mesmo sem startup de metadados certificado.
  A regressão com o texto `@opaque` falhou antes da correção. Ponteiros reais,
  constantes públicas e chamadas comuns de registro preservam suas arestas.
  Cada módulo começa com o resumo emprestado e só recebe cópia na primeira
  alteração efetiva; no caso de dois módulos, apenas um é copiado, mantendo o
  inventário original intacto. Passaram 993 testes nativos com ignorados,
  Clippy estrito e docs sem avisos. A reprodução nos dois modos conservou IR
  idêntico e seis definições estrangeiras. A fixture de runtime coincidiu com
  a VM em AOT de produção/desenvolvimento e JIT macOS, normal/sob estresse de
  GC. Sem ganho geral medido, nova matriz Windows/Linux ou alteração de ABI,
  protocolo e permissões de poda. A produção atual ainda para no primeiro
  startup antes dessa visão; poda estrangeira, fmt global e aprovação integral
  seguem pendentes. Evidências:
  `bench/results/2026-10-20-proof-summary-views/README.md`.

- A rejeição de startup importado mostra agora as obrigações verificadas e os
  destinos de helpers, com ordem determinística e até três itens por lista.
  Distingue destino ausente/ambíguo, definição/ABI não provada e alocação
  desconhecida; footprint vazio continua sem autorizar invocação pelo runtime.
  A reprodução real identifica os helpers de IndexError, LateError e
  StackOverflowError em `_internal`, preservando o registro amplo. Passaram
  989 testes nativos com ignorados, Clippy estrito e docs sem avisos. Os dois
  modos da reprodução conservaram IR idêntico e seis definições estrangeiras;
  a fixture de runtime coincidiu com a VM em AOT de produção/desenvolvimento
  e JIT macOS, normal/sob estresse de GC. O censo observacional do SDK anterior
  registra 41 helpers distintos, todos com alocação transitiva desconhecida;
  isso não é certificado de invocação. Sem mudança de permissão de poda,
  protocolo do inventário ou runtime, nova matriz Windows/Linux ou ganho
  medido. Fmt global e aprovação integral permanecem pendentes. Evidências:
  `bench/results/2026-10-20-startup-obligations/README.md`.

- O certificado de inicialização reconhece agora apenas o par reservado de CID
  e rótulo `_AsyncFrame`, sem tratá-lo como classe nominal ou layout fixo.
  Declarações nominais, tabelas, layouts e relações de subtipo conflitantes em
  qualquer resumo selecionado recusam essa exceção. Chamadas comuns e alocações
  continuam sem autorização. O inventário real certifica oito entradas de
  metadados, incluindo `dart:developer`; a primeira entrada de imagem/helpers
  segue sem prova. Passaram 984 testes nativos com ignorados, incluindo seis
  novos, Clippy estrito e documentação sem avisos. As consultas de subtipo em
  executáveis nativos mantiveram o resultado com registro amplo/restrito e GC
  normal/sob estresse. A fixture de runtime coincidiu com a VM antes/depois em
  AOT de produção/desenvolvimento e JIT macOS, normal/sob estresse. O IR completo
  do explorador permaneceu idêntico (293.593.137 bytes); não houve nova matriz
  Windows/Linux nem medição de ganho. Poda de backends, fmt global e aprovação integral seguem
  pendentes. Evidências: `bench/results/2026-10-20-frame-display-metadata/README.md`.

- O bundle completo do SDK registra sua imagem apenas pela primeira raiz de
  inicialização: as chamadas geradas caíram de 15 para uma, também confirmadas
  nas relocações dos objetos de desenvolvimento. A emissão pública por biblioteca
  conserva registro próprio, e o programa mantém seu registro para imagens
  separadas. As demais operações, objetos estáticos e marcadores são preservados.
  As regras anteriores agora certificam sete entradas apenas nominais/textuais;
  `dart:developer` continua recusada por publicar um `_AsyncFrame` ainda sem
  declaração nominal auditada. A expectativa inicial de oito foi corrigida
  com essa recusa explícita, preservando os logs das falhas preliminares.
  Passaram os 978 testes nativos com ignorados, Clippy estrito e docs sem avisos.
  Strings Unicode, JSON/UTF-8, erro de índice e três isolados coincidiram com a
  VM em AOT de produção/desenvolvimento e JIT macOS, normal e sob estresse de GC.
  A mesma fixture AOT cruzada passou no Linux ARM64 e no Windows x64 sob as duas
  condições. Os bundles foram reexportados nas VMs a partir de cópias temporárias
  das fontes atuais, com hash de runtime igual; os caminhos `current` foram
  atualizados e os anteriores preservados. Problemas de transporte/PowerShell
  da validação Windows foram corrigidos sem reiniciar VMs. A reprodução ainda
  emite seis definições estrangeiras nos dois modos; imagem/helpers, poda,
  fmt global e aprovação integral continuam pendentes. Sem ganho geral de
  desempenho/tamanho medido nem nova matriz de interface do explorador.
  Evidências: `bench/results/2026-10-20-sdk-image-owner/README.md`.

- A auditoria de imagens cobre agora as relocações de base PE32+ de Windows
  x64/ARM64: lê o diretório real, confere mapeamentos, blocos e spans completos
  HIGHLOW/DIR64, conta preenchimento ABSOLUTE e recusa formatos desconhecidos,
  tabelas parciais e escritas no próprio controle. Nove testes novos passaram;
  imagens das duas arquiteturas com 2.100 ponteiros tiveram cinco blocos e
  nenhuma escrita sobre strings. O explorador Windows preservado teve 51.190
  escritas declaradas, sem sobrepor os 871.040 bytes de strings estáticas.
  Arquivos foram inspecionados, sem execução. Passaram os 975 testes nativos
  com ignorados, Clippy estrito, docs sem avisos e o exemplo de documentação.
  A primeira rodada não encontrou `lld-link` no PATH; uma rejeição posterior
  do Clippy foi corrigida, com logs preservados e validação completa repetida.
  Imports/TLS/configuração do loader, inicializadores, marcadores e limites
  publicados ainda não estão provados. A alteração não autoriza registro/poda,
  não corrige a ausência dos backends nem o fmt global e não demonstra ganho
  de desempenho ou aprovação integral. Evidências:
  `bench/results/2026-10-20-pe-base-relocations/README.md`.

- O ponto fixo FFI de `prune_hir` reutiliza os fatos imutáveis dos corpos e
  tabelas, substituindo apenas a entrada e seus nomes exclusivos por rodada.
  O empréstimo do módulo impede mutações durante o reuso e termina antes das
  podas; invocações posteriores refazem a análise. Quatro testes novos comparam
  resumos completos, raízes crescentes/decrescentes e fatos após mudança do HIR.
  No caso transitivo de quatro rodadas, as construções dos corpos caíram de
  quatro para uma; o explorador real usa duas rodadas. Passaram os 966 testes
  nativos com ignorados, Clippy estrito e docs sem avisos. Os executáveis FFI
  com callback coincidiram com a VM, antes/depois, normal e sob estresse de GC.
  O IR completo do explorador ficou idêntico nas emissões fria e aquecidas.
  Na sequência aquecida ABBA, duas amostras por compilador tiveram média de
  108,45 s antes e 97,15 s depois (−10,4%); poda HIR de 25,74 s para 17,92 s.
  É emissão de IR com compiladores debug em um Mac, sem link e sem controle
  exclusivo da máquina; não demonstra ganho geral ou em outros sistemas.
  Sem nova matriz desktop/JIT, autorização adicional de poda ou aprovação
  integral; poda de backends e fmt global continuam pendentes. Evidências:
  `bench/results/2026-10-20-hir-summary-reuse/README.md`.

- A prova estrutural calcula dominância em ordem reversa de pós-visita do fluxo,
  reaproveitando a travessia que identifica os blocos alcançáveis. O percurso é
  iterativo, sem cópia do grafo ou recursão; entradas duplicadas de switch não
  ampliam a pilha pendente. No caso controlado de 512 blocos, o mesmo ponto fixo
  passou de 511 para duas varreduras, com conjuntos idênticos. Três testes novos
  complementam o oráculo independente por remoção de blocos. Passaram os 962 testes
  nativos com ignorados, Clippy estrito e docs sem avisos. O IR completo do explorador
  real (293.593.137 bytes) ficou idêntico antes/depois, em rodadas fria e aquecida.
  Executáveis com muitas ramificações coincidiram com a VM normal e sob estresse
  de GC. O amostrador opcional não produziu relatório e foi encerrado; os
  compiladores terminaram normalmente. Sem ganho geral de compilação demonstrado,
  nova matriz desktop/JIT ou aprovação integral; poda de backends e fmt global
  continuam pendentes. Evidências:
  `bench/results/2026-10-20-dominator-traversal/README.md`.

- A auditoria Mach-O agora cobre chained fixups de 64 bits e ARM64E, com limites
  de segmentos/páginas, múltiplos inícios, imports e nomes, bits reservados e spans
  completos. Corrigida também uma falha anterior: o leitor aceitava flags de
  mapeamento não revistas e escritas clássicas sobre cabeçalhos/LINKEDIT; o teste
  novo falhou antes da correção. Escritas que podem alterar o próprio controle
  das cadeias são recusadas. Passaram nove testes novos, os 959 testes nativos
  com ignorados, 35 dirigidos de imagens, um doctest, Clippy estrito e docs sem
  avisos. Imagens ARM64/x64 com 2.100 ponteiros adicionais tiveram exatamente
  2.101 rebases e um bind, atravessando duas/cinco páginas, sem atingir strings
  estáticas. Os arquivos foram inspecionados, sem execução. Isso não autoriza
  registro de imagens nem poda; provas de limites reais/callbacks/memória bruta,
  remoção dos backends, fmt global e aprovação integral continuam pendentes.
  Evidências: `bench/results/2026-10-20-macho-chained-fixups/README.md`.

- Acrescentada auditoria dos spans de escrita declarados pelas tabelas clássicas
  de rebase/bind Mach-O, uma etapa da prova ainda pendente de registro de imagens
  do SDK. Confere mapeamentos, comandos, operandos e limites; conta repetições sem
  expandi-las, inclusive um caso com um bilhão de escritas. No explorador macOS
  preservado, as 51.706 escritas potenciais não atingem os 850.904 bytes da seção
  de strings estáticas. Onze testes novos cobrem os quatro streams, bordas,
  mutações, arquivos ARM64/x64 recém-ligados e recusa explícita de chained fixups.
  Passaram os 950 testes nativos com ignorados, 26 dirigidos de imagens, um doctest,
  Clippy estrito do workspace/backend LLVM e docs sem avisos. O utilitário lê os
  três artefatos desktop anteriores, com hashes conferidos; não houve nova
  execução desktop. Registro completo de imagens, limites efetivamente publicados,
  callbacks, poda de backends, fmt global e aprovação integral continuam pendentes.
  Não há nova autorização de poda nem ganho de desempenho/tamanho demonstrado.
  Evidências: `bench/results/2026-10-20-macho-relocation-spans/README.md`.

- Corrigida uma falha na prova experimental de poda de classes: um campo com
  bits escalares podia ser lido como referência gerenciada e aceito como receptor
  de seletor. O teste novo falhou antes da correção. A prova agora acompanha
  identidade e limites das alocações locais, representações reais e todas as
  escritas, inclusive por aliases; confere os indicadores de referência na ABI
  apagada e separa leituras imutáveis de closures de protocolos de objetos comuns.
  Passaram nove regressões novas, incluindo oito execuções nativas com poda
  ligada/desligada e GC normal/sob estresse, e os 939 testes da biblioteca nativa
  com ignorados. Clippy estrito do workspace/backend LLVM e docs sem avisos
  passaram. Uma falha intermediária do Clippy foi corrigida e a validação repetida.
  A reprodução de produção ainda contém seis definições de backends estrangeiros
  nos dois modos: a inicialização interna do SDK continua sem prova completa.
  Nenhuma expectativa do teste de poda foi relaxada; fmt global, aprovação
  integral e remoção desses backends permanecem pendentes. Sem nova matriz
  desktop nem ganho de desempenho medido. Evidências:
  `bench/results/2026-10-20-class-storage-boundaries/README.md`.

## Atualização de 19/10/2026

- Corrigido o crash do JIT com IR AOT de desenvolvimento no macOS ARM64:
  `dartforge_context_slot` agora vem da mesma biblioteca do SDK que fornece as
  funções do runtime, como dado não chamável. A importação inspeciona globais
  externos do LLVM, também antes da primeira recarga, sem zerar a célula do SDK.
  O teste mínimo retornava 1 por contexto divergente e agora retorna 0; os dois
  IRs anteriormente problemáticos, com SDKs intactos, coincidem com a VM normal
  e sob estresse de GC. A API JIT regular também passa. Foram aprovados 92 testes
  JIT distintos e seis exemplos de documentação, contando a suíte final de
  execução 18/18 e quatro regressões novas; Clippy estrito e docs sem avisos
  passaram. Sem nova matriz desktop/Windows/Linux ou ganho de desempenho medido.
  A poda de backends, o fmt global e a aprovação integral continuam pendentes.
  Evidências: `bench/results/2026-10-19-jit-sdk-context-data/README.md`.

- O HIR reutiliza `df.string_len` dominante e move prefixos de leitura para
  entradas incondicionais de laços quando o receptor é invariável. Não muda o
  helper LLVM nem acrescenta metadados de invariância de memória; preserva
  receptores que mudam, entradas condicionais, debug e planos ARC já preparados.
  Passaram oito testes novos com interpretação independente, 930 testes da
  biblioteca nativa, Clippy estrito do workspace/backend LLVM e docs sem avisos.
  AOT ligado/desligado, ARC e desenvolvimento, e o JIT pela API regular, coincidiram
  com a VM em macOS, normal e com GC sob estresse. No IR real, a leitura da string
  estável sai do laço; sem ganho geral de tempo medido. A continuação ampla anterior
  terminou com falha na poda de backends em `production_profile`. Executar IR AOT
  podado no executor JIT também falha, reproduzido com o compilador anterior;
  a causa segue aberta. Fmt global e aprovação integral continuam pendentes.
  Evidências: `bench/results/2026-10-19-string-length-reuse/README.md`.

- A rodada ampla anterior terminou: corpus nativo de builders aprovado e os
  quatro testes de transparência Angular aprovados, incluindo edição por arquivo
  (557,38 s para a suíte Angular). Depois, os 15 testes básicos JavaScript falharam
  porque Node não estava no PATH. O mesmo executável de testes passou 15/15 ao
  incluir a instalação existente do Node; o log original foi preservado. O runner
  PowerShell agora verifica Cargo/Dart/Node antes de começar e inclui os testes
  ignorados. A descoberta do ambiente evita Join-Path em drives inexistentes;
  sintaxe e recusa antecipada de Node ausente foram conferidas no macOS. A
  continuação Cargo exclui o pacote de build já executado e ainda estava na
  linkedição nesta observação; não há aprovação global nem nova matriz desktop.
  Evidências: `bench/results/2026-10-19-workspace-node-preflight/README.md`.

- A rodada ampla original avançou: o corpus de builders pelo executor nativo
  passou em 2.428,12 s, com o compilador de referência preservado por SHA-256.
  A seleção estava sem filtro; o inventário observado contém dez casos planejados.
  A suíte entrou em `ng_transparency`: três testes passaram e a edição Dart por
  arquivo segue em execução. É resultado parcial com o compilador anterior,
  sem aprovação do workspace atualizado. Evidências:
  `bench/results/2026-10-19-workspace-builder-progress/README.md`.

- A chave do SDK compilado agora inclui a política efetiva de raízes pelo
  chamado. O teste reproduziu chaves iguais com LLVM diferente antes da correção;
  a decisão é compartilhada com o emissor, preservando a inatividade em ARC/mapas
  e a estabilidade do catálogo de classes. A auditoria excluiu três controles
  exclusivos do programa da chave do SDK. Passaram 922 testes nativos e oito
  dirigidos de cache; a matriz dirigida cobriu 72 subprocessos entre destinos,
  controles e modos de memória. O SDK real criou/reutilizou duas entradas ao
  alternar a regra, e os executáveis dos dois modos coincidiram com a VM normal
  e sob estresse de GC. Clippy estrito e docs sem avisos passaram; suíte ampla
  original continua nos builders, sem aprovação global ou correção do fmt.
  Evidências: `bench/results/2026-10-19-sdk-rooting-policy/README.md`.

- Corrigida a causa da divergência do cache: `DARTFORGE_OPTIMIZE_HIR=0` alterava
  o SDK emitido sem alterar sua chave compilada. O compilador anterior reproduziu
  byte a byte os 15 resumos problemáticos com esse modo. O teste novo falhou antes
  da correção; agora cobre os perfis desenvolvimento/produção, três destinos e
  composição com RELR. A opção usa a mesma política dos passes na chave do SDK,
  preservando a chave do catálogo de classes ao alternar o modo. Integração real
  em cache isolado criou duas entradas e reutilizou a otimizada ao voltar para
  `1`, com objetos/resumos intactos e IDs estáveis. Passaram 921 testes nativos,
  sete dirigidos de cache, Clippy estrito e documentação sem avisos. A suíte
  ampla original segue nos builders com o compilador anterior preservado;
  fmt global e aprovação integral continuam pendentes. Evidências:
  `bench/results/2026-10-19-hir-sdk-cache-policy/README.md`.

- A análise de proveniência física usa bitmaps na dominância, mantendo o ponto
  fixo e o orçamento de 512 blocos. Uma amostragem do builder real mostrou o
  custo anterior nas cópias/interseções de árvores. Passaram 920 testes de
  biblioteca nativa e a rodada dirigida, com quatro regressões novas, incluindo
  oráculo independente por remoção de blocos. Na medição isolada de 256 blocos,
  a representação anterior levou cerca de 7 s e a nova 23 ms; sem ganho geral
  de compilação demonstrado. Clippy estrito do workspace/backend LLVM e docs
  sem avisos passaram. O IR com SDK reconstruído ficou idêntico entre os
  compiladores; os executáveis coincidiram com a VM em execução normal e sob
  estresse de GC. O mesmo compilador anterior divergiu entre cache antigo e
  reconstrução limpa. Essa reprodutibilidade do cache segue em investigação.
  A suíte ampla de builders continua com o compilador anterior preservado;
  fmt global e aprovação integral permanecem pendentes. Evidências:
  `bench/results/2026-10-19-dense-dominators/README.md`.

- O exemplo Angular usa o alvo Cargo `ng_oracle`, eliminando a colisão de
  executável/dSYM com o `oracle` do formatador. Metadados conferidos: 48 exemplos
  sem nomes duplicados; ambos compilaram juntos e executaram independentemente.
  A fixture do formatador coincidiu com Dart 3.6.2 (1/1), e Clippy estrito do
  gerador passou. A suíte ampla parou no corpus de builders porque a CLI padrão
  substituiu o compilador externo por uma versão sem backend nativo. As
  instruções agora preservam uma cópia separada; a rodada foi retomada com um
  compilador nativo recém-montado e ainda não tem aprovação global.
  Evidências: `bench/results/2026-10-19-example-target-names/README.md`.

- Compactação RELR Linux opcional (`DARTFORGE_PACK_RELR=1`), com dependência de
  versão glibc e cache separado apenas para o SDK compartilhado afetado.
  Ligação controlada do explorador com os mesmos 47 insumos: 37.624.936 para
  36.011.472 bytes, menos 1.613.464 bytes (4,29%). Usa compilador retido com
  hash registrado, não rebuild completo desta revisão. Ambas as versões passaram
  em dois frames no Ubuntu ARM64/glibc 2.39; fixture de ponteiros passou nos dois
  modos, e o controle sem dependência de versão foi corretamente recusado.
  Cinco testes RELR e seis de cache passaram (dez distintos, três novos).
  Clippy estrito do workspace e do backend com LLVM embutido e documentação sem
  avisos passaram; fmt global e suíte integral continuam pendentes. Não houve
  nova matriz Windows/macOS/JIT, ganho de velocidade ou autorização de poda.
  Evidências: `bench/results/2026-10-19-linux-relr-packing/README.md`.


- `linked_images::elf_relocations::inspect` observa REL/RELA/RELR/JMPREL via
  PT_DYNAMIC e mapeamentos PT_LOAD, incluindo escritas que atravessam a borda
  da seção. Tags/tipos sem classificação permanecem explícitos; prefixos rápidos,
  bitmaps RELR, limites e overflow são conferidos. Exemplos são limitados sem
  interromper contagens. O explorador Linux ARM64 anterior, com SHA-256 conferido,
  tem 67.578 alvos declarados e nenhum span conhecido sobrepõe seus 854.760 bytes
  de strings. Há tags de inicialização; efeitos implícitos do carregador seguem
  sem prova. Fixtures PIE ARM64/x64 passaram com RELA/RELR e alvos alterados para
  a imagem foram detectados nos quatro casos. Passaram 15 testes de imagens
  (nove novos), exemplo executável, Clippy e docs estritos. Fmt global e aprovação
  integral continuam pendentes; sem nova execução desktop/JIT, rebuild do SDK,
  autorização de poda, certificado de imutabilidade ou ganho de binário medido.
  Evidências: `bench/results/2026-10-19-elf-relocation-records/README.md`.


- A nova API `linked_images::inspect` confere todos os bytes das seções de
  strings em arquivos ligados Mach-O/ELF/PE 64-bit LE. Compartilha a validação
  canônica com o inventário LLVM e registra palavras zeradas como padding,
  sem certificar âncoras. Os três exploradores da matriz anterior, com SHA-256
  conferido, passaram: 14.396 strings macOS, 14.548 Linux e 14.534 Windows;
  cada arquivo tem 36 strings de dois bytes. Fixtures de layout recém-ligadas
  para cinco destinos passaram e hashes alterados no arquivo final foram
  recusados. Passaram seis testes novos, 64 de inventário com SDK real, 11 de
  entradas nativas e o exemplo da API; Clippy/docs estritos passam. Inventário,
  registros dos módulos e tamanho do schema 5 permaneceram iguais ao anterior.
  Fmt global e aprovação integral seguem pendentes. A API ainda não é um gate
  automático do ligador: relocações/rebases/binds, limites registrados,
  marcadores, entradas/opções e permissões continuam sem prova. Sem autorização
  nova de classes/imagens, recompilação/execução nova do explorador ou ganho
  de binário comprovado. Evidências:
  `bench/results/2026-10-19-linked-string-sections/README.md`.


- A nova API `native_inputs::inspect` observa objetos Mach-O/ELF/COFF e membros
  de bibliotecas sem executar destinos ou seguir caminhos externos. Preserva
  nomes/duplicatas, faixas, digests, seções e estados de marcadores; distingue
  common de referência indefinida. Um comando Mach-O truncado agora é recusado
  explicitamente, sem parecer um objeto sem seções. Passaram 12 testes com LLVM,
  incluindo as três variantes reais do runtime e objetos para cinco destinos,
  mais o exemplo executável; Clippy/docs estritos passam. Cada runtime local tem
  466 membros e 229 seções candidatas a bitcode (1.331.200 bytes). Os 229 módulos
  embutidos de production foram lidos/verificados por LLVM, sem globais/funções
  na imagem; cinco assemblies de compiler_builtins seguem não revisados. O
  digest do arquivo production é igual ao anterior. A dependência `object`
  atualiza Cargo.lock e a identidade de fontes do runtime: bundles devem ser
  reexportados. Isso não autoriza imagens/classes; ligador, relocação, LTO,
  entradas estrangeiras e imagem final seguem pendentes. Sem nova matriz de
  execução desktop/JIT, aprovação global ou ganho de binário comprovado.
  Evidências: `bench/results/2026-10-19-native-input-census/README.md`.


- A inspeção estrutural LLVM agora lê IR/bitcode verificado e preserva nomes de
  seção completos, inclusive NULs e barras invertidas da impressão canônica.
  A integração com `embedded-llvm` conferiu os bytes/digests dos 15 artefatos
  reais do SDK: 5.255 definições de imagem na fonte, 5.233 retidas e 22 removidas
  pelo otimizador. Todas as retidas coincidem com os inicializadores observados;
  31.395 chamadas inline de assembly correspondem à leitura TSD exata revisada.
  Passaram 64 testes de inventário, oito da nova inspeção, um de objetos para
  cinco destinos e o exemplo executável da API. Clippy estrito do workspace e
  com `embedded-llvm` passa; documentação privada com avisos como erro também.
  O inventário/schema 5 permanece idêntico ao anterior;
  isso não muda a geração nem autoriza imagens/classes. Os artefatos examinados
  são bitcode anterior ao LTO: arquivos do runtime, entradas do ligador e imagem
  final ainda precisam de prova. Fmt global e aprovação integral seguem pendentes;
  sem nova matriz desktop/JIT ou redução comprovada do binário.
  Evidências: `bench/results/2026-10-19-bitcode-image-sections/README.md`.


- O schema 5 vincula um censo de seções/marcadores ao IR final de cada módulo.
  O scanner separa símbolos, strings e comentários, registra contribuições de
  funções/globais e conserva obrigações para assembly, atributos e formas não
  revisadas. Protocolos Mach-O/ELF/COFF compartilham o produtor do emissor. A
  rodada inicial isolou a leitura TSD ARM64 repetida nos 15 módulos; sua sequência
  exata foi revisada sem autorizar as leituras de memória seguintes. No SDK final,
  os 15 censos de fonte não têm contribuições opacas no subconjunto suportado:
  5.240 strings, 15 âncoras e 45 linhas de marcadores. Isso não prova os objetos,
  arquivos do runtime ou entradas/opções do ligador, nem autoriza register_image.
  Passaram 64 testes de inventário com SDK real, 51 de classes e 14 gerais de poda;
  Clippy/docs estritos passam. IR dos módulos e 15 startups permanece igual; JSON
  de cache cresceu 1,6%. Fmt, aprovação integral, imagem/startup completos e poda
  do explorador seguem pendentes, sem nova matriz desktop/JIT ou ganho de binário.
  Evidências: `bench/results/2026-10-19-source-image-census/README.md`.


- O schema 4 preserva o inicializador de todos os globais e revalida digest,
  símbolo e classificação. Um observador de strings estáticas confere header
  permanente/RAW, CID, palavras, comprimento, bytes/padding, hash VM e chave
  canônica, incluindo Latin-1, UTF-16 e surrogates. No SDK real foram observadas
  5.240 definições válidas (CID 6); 15 âncoras da seção continuam não classificadas.
  Isso não autoriza registro de imagem, handles ou retorno Ref/ptr: falta provar
  a faixa e todos os contribuidores do ligador. Passaram 58 testes de inventário,
  51 de classes e 14 gerais de poda, Clippy/docs estritos. IR dos módulos e corpos
  dos 15 startups permanecem iguais; cache JSON cresceu 1,6% para guardar dados.
  Fmt global, aprovação integral, imagem/startup completos e poda do explorador
  continuam pendentes; sem nova matriz desktop/JIT ou ganho de binário medido.
  Evidências: `bench/results/2026-10-19-static-image-layout/README.md`.


- O certificado de startup compõe agora nomes de classes/argumentos com arestas
  nominais. O schema 3 preserva inicializadores literais e verifica digest,
  propriedade privada, bytes/escapes, comprimento exato e UTF-8. Ponteiros
  deslocados, regiões externas, dados ambíguos e outros efeitos não são admitidos.
  O teste do runtime confirma texto preservado sem alocações Dart; startup misto
  compilado passa com registro amplo/certificado, normal/GC sob estresse. Passaram
  52 testes de inventário com SDK real, 51 de classes, 14 gerais de poda e o teste
  de runtime; Clippy/docs estritos passam. Cache JSON cresceu 1,3% para conservar
  7.753 inicializadores, com IR dos módulos e 15 startups inalterados. Isso não é
  ganho de tamanho do executável. Imagens, tabelas e helpers ainda impedem os
  startups completos do SDK; fmt, aprovação integral e poda completa seguem
  pendentes, sem nova matriz desktop/JIT. Evidências:
  `bench/results/2026-10-19-text-metadata-startup/README.md`.


- A prova de classes ganhou certificado separado para startups `void ()` que
  somente publicam arestas nominais literais por `register_subclass`. Exige
  corpo/ABI/identidade únicos, CIDs e arestas do snapshot reconciliado; pai 0
  compartilhado mantém seu certificado próprio. O grafo temporário descarrega
  apenas arestas pertencentes ao startup certificado, sem autorizar chamadas
  comuns ou a API globalmente. Passaram 45 testes de inventário com SDK real,
  51 de classes e 14 gerais de poda; executáveis amplo/certificado conservaram
  consultas transitivas/inversas, normais e com GC sob estresse. Clippy/docs
  estritos passam, fmt global segue pendente. O teste real confirma que os
  startups completos do SDK continuam recusados: nomes, argumentos, imagens,
  tabelas e helpers precisam de contratos próprios. Sem poda completa, nova
  matriz desktop/JIT ou ganho comprovado de tamanho/desempenho. Evidências:
  `bench/results/2026-10-19-nominal-subtype-startup/README.md`.


- Corrigida uma aprovação indevida do inventário: chaves dentro de parâmetros
  LLVM entre aspas ou atributos textuais podiam ocultar o corpo real e sua
  alocação. Dois testes falharam antes: observação de fechamento vazio e aceitação
  da função alocadora como importação vazia. O coletor respeita aspas/escapes,
  comentários e os limites entre funções; o CID real chega ao wrapper e o leitor
  recusa a importação vazia. Cinco fixtures foram compiladas pelo Clang 22.
  Passaram 41 testes de inventário, incluindo reconstrução/carregamento real do
  SDK, e 48 da prova de classes; Clippy/docs estritos passam. Schema permanece 2,
  com cache invalidado pelo fingerprint das fontes. Registro SDK e poda completa
  continuam pendentes, assim como fmt e aprovação integral. Sem nova matriz
  desktop ou ganho comprovado de tamanho/desempenho. Evidências:
  `bench/results/2026-10-19-quoted-body-boundaries/README.md`.


- Corrigida a invalidação do certificado de classes antes da construção do grafo
  amplo da poda HIR. Antes, uma análise anterior podia reduzir as raízes do
  fallback mesmo após a nova prova ser recusada. A regressão reproduziu a
  remoção indevida de métodos e agora compara o resultado com HIR completo sem
  certificado, com a opção experimental ligada/desligada. Passaram 48 testes da
  prova de classes, 14 gerais de poda, 3 de otimização anterior à poda e o teste
  dirigido com a opção desligada; Clippy/docs estritos passam. Fmt global,
  aprovação integral, certificação do registro SDK e poda completa continuam
  pendentes. Isso não recupera corpos removidos por poda anterior nem comprova
  ganho de tamanho/desempenho. Evidências:
  `bench/results/2026-10-19-class-certificate-invalidation/README.md`.


- O inventário versão 2 conserva operandos tipados e reconhece argumentos LLVM
  entre aspas, com escapes/delimitadores e comentários. Os 15 corpos de startup
  do SDK mantêm o mesmo digest; cada um agora registra a chamada
  `dartforge_register_image` antes perdida como ABI não suportada. Isso não
  autoriza registro: imagens estáticas e bindings de helpers exigem certificados
  próprios. Snapshots antigos e operandos incompatíveis com a ABI são recusados.
  Foi corrigida também uma aprovação indevida da prova: ABI LLVM `i64` não
  certifica resultado `Ref`, nem retorno `ptr` prova uma região de pacote.
  Resultados escalares/void continuam admitidos sob as regras anteriores.
  Passaram 38 testes de inventário, com publicação/carregamento real do SDK,
  47 da prova de classes, Clippy/docs estritos. Fmt global e aprovação integral
  seguem pendentes; sem nova matriz desktop/JIT ou redução comprovada do binário.
  O JSON do cache observado cresceu 16,8% (57.251.557 → 66.874.532 bytes) para
  guardar operandos; esse custo não é tamanho do executável. A autorização de
  startup e a poda completa continuam pendentes. Evidências:
  `bench/results/2026-10-19-registry-call-operands/README.md`.


- A prova reconhece o domínio vazio da entrada do loop em imagens fixas sem
  ingresso nativo aberto: retira somente a aresta automática em um resumo
  temporário e exige cobertura completa das demais raízes. Produtores de
  microtasks/timers/portas/finalizadores/eventos nativos continuam não admitidos;
  raízes nativas e trampolines FFI também impedem esse caso-base. Exposição do
  bridge por chamadas/endereço/tabelas conserva o comportamento amplo. Somente
  após sucesso são removidos `call_dart` e o bridge não referenciado. Passaram
  46 testes da prova, incluindo executáveis eager/closed normais e sob GC.
  O compilador final passou seis comandos reais de microtask/timer com a opção
  experimental ligada: referência VM, compilação AOT e quatro execuções AOT/JIT
  normais/estresse, todas iguais à VM; as filas reais conservaram seu loop.
  Clippy/docs estritos passam; fmt global continua pendente. A produção ainda
  falha, agora no registro SDK `df.register.dart$3a_internal`, sem publicar
  certificado. Domínios não vazios, certificados de registro SDK, poda completa,
  aprovação global e nova matriz desktop permanecem pendentes. Sem redução
  comprovada de tamanho ou medição de desempenho. Evidências:
  `bench/results/2026-10-19-empty-event-domain/README.md`.


- A investigação do callback encontrou uma lacuna separada na prova: raízes
  externas aceitavam `ptr` e podiam encaminhar pacotes sem domínio provado;
  casts de inteiros para ponteiros também eram aprovados. Dois testes HIR
  falhavam antes por aprovação indevida. Raízes locais agora exigem tipos
  escalares revisados; casts que fabricam ponteiros e unboxing para endereços
  são recusados. Casts de identidade de endereços locais continuam aceitos.
  Passaram os 41 testes da prova de classes, incluindo regressões nas seis
  categorias de raiz, controles escalares e programas compilados de closures
  com GC sob estresse. Clippy/docs estritos do workspace passam; fmt global
  continua pendente. Não foi repetida a suíte integral nem a matriz desktop.
  O Ref externo de `df.call_dart0` continua sem prova; não há nova autorização
  de callbacks, poda completa, medição de tamanho ou desempenho. Evidências:
  `bench/results/2026-10-19-entry-packet-boundaries/README.md`.


- O pai universal sintético 0 tem agora certificado próprio, emitido somente
  para uma declaração sem identidade física, tabelas, campos, layout, ancestral
  próprio ou alocação local. O resumo versão 9 conserva a marca e rejeita
  versões antigas e marcas inconsistentes. A reconciliação compartilha somente
  a declaração nominal 0 certificada nos dois lados; outros CIDs, tabelas e
  ambiguidades continuam sujeitos às colisões anteriores. O teste integrado
  aceita o caso provado e recusa premissas ausentes e alocação de CID 0.
  Passaram 839 testes nativos e 142 dirigidos de poda; Clippy/docs estritos
  passam, fmt global permanece pendente. O compilador reconstruído passou
  11 comandos reais de identidade de tipos: referência VM, duas compilações e
  oito execuções tracing/ARC, AOT/JIT, normal/estresse, todas iguais à VM Dart.
  A produção avançou além da colisão do CID 0 e agora recusa `df.call_dart0`
  por corpo/ABI de entrada não provados. O domínio do Ref recebido pelo loop
  exige prova independente; não houve relaxamento dessa fronteira, aprovação
  global, redução comprovada do explorador ou nova execução nas VMs. Persistem
  os avisos de deployment target macOS. Evidências:
  `bench/results/2026-10-19-universal-parent/README.md`.


- A reconciliação do inventário SDK informa agora o limite rejeitado: contagem
  ou repetição de módulo, snapshot divergente, colisão de símbolo, CID inválido
  ou colisão de classe. Nenhuma condição de admissão foi relaxada. O teste real
  de produção identifica `dart:_internal`, classe nominal 0, em colisão com a
  imagem local; o lowering insere o `Object` sintético 0 como pai universal.
  Unificar essa identidade exige prova própria e continua pendente. Passaram
  836 testes nativos e 10 de inventário antes do ajuste cosmético final da
  mensagem; o teste dirigido final passou no código definitivo. Clippy/docs
  estritos passam; fmt global e aprovação integral do workspace seguem pendentes.
  A poda de backend ainda falha, preservando registro amplo. Não houve nova
  execução nas VMs, medição de desempenho ou redução comprovada do explorador.
  Evidências: `bench/results/2026-10-19-inventory-reconciliation/README.md`.


- As funções internas geradas usam agora `df.prepare_rti` e `df.call_dart0`,
  separadas do namespace nativo reservado. O contrato fica em `dartforge-abi` e
  participa do fingerprint do cache do SDK. A recarga aceita o inicializador
  legado e republica ambos os formatos mesmo com corpo inalterado; o teste
  cobre também migração legado → atual. Passaram 835 testes nativos, 36 dirigidos,
  45 unitários JIT, 17 de hot reload e os testes/doctest ABI. Três programas reais
  passaram em tracing/ARC, AOT/JIT, normal/estresse, com saída igual à VM Dart:
  33 comandos incluindo referências e compilações. Clippy/docs estritos do
  workspace passam; fmt global e avisos macOS de deployment target persistem.
  A primeira suíte JIT falhou antes dos cenários por `ld64.lld` ausente no PATH;
  a repetição configurada passou sem mudar expectativas. A poda experimental
  continua falhando por reconciliação do inventário SDK com a imagem, mantendo
  registro amplo. Não houve relaxamento da prova, redução comprovada do
  explorador, aprovação global ou nova execução Windows/Linux. Os dois novos
  vínculos locais em Cargo.lock podem invalidar fingerprints de bundles de
  destino existentes. Evidências:
  `bench/results/2026-10-19-image-entry-symbols/README.md`.

- `_Type` tem agora a função de tabela registrada e ancestralidade marcada só
  quando o pai do SDK é resolvido e distinto. A prova protege seu CID criado
  pelo runtime mesmo sem alocação local ou vetor posicional; não autoriza o
  construtor nativo. Passaram 833 testes unitários nativos e 34 dirigidos; o
  teste ABI compilado confere CID/payload/identidade canônica sob GC. Um programa
  real de RTTI passou em AOT/JIT normal e sob estresse, igual à VM, em seis
  comandos. Clippy/docs estritos passam; fmt e avisos de deployment target
  continuam pendentes. A poda ainda falha na checagem de namespace das entradas
  geradas `dartforge_rti_init`/`dartforge_call_dart0`; a prova não foi relaxada.
  A rodada global de 9cf1d3ba terminou com exit 101 no teste C de ABI, por falta
  de sysroot para o Clang externo. O teste inalterado passou após configurar
  SDKROOT 26.5 e deployment target 14.0, incluindo objeto Wasm; o pré-requisito
  está em CONTRIBUTING. Não houve nova aprovação global, execução nas VMs ou
  redução comprovada do explorador. Evidências:
  `bench/results/2026-10-19-runtime-type-tables/README.md`.

- A declaração de `_AsyncFrame` acompanha alocações locais após poda de fontes,
  independentemente da flag do loop. O callback e o loop permanecem ativos quando
  necessários. Duas regressões falhavam antes: classe fictícia em módulo sem
  frames e metadados ausentes em geradores baixados sem a flag. Passaram os 830
  testes unitários nativos, os dois dirigidos, Clippy estrito e docs do workspace.
  O compilador native/JIT foi remontado: dois programas reais (microtask/timer e
  async/sync*/async*) passaram em AOT/JIT normal e sob estresse de GC, com saídas
  iguais à VM Dart, em 12 comandos. Fmt global continua pendente; a montagem
  mantém avisos de deployment target macOS. A poda de plataforma ainda falha,
  agora primeiro em `_Type` (CID 1073741569) com ancestralidade não provada.
  Frames reais não foram tratados como metadados nominais e continuam exigindo
  prova própria. Sem redução comprovada do explorador ou nova aprovação global
  e Windows/Linux. Evidências:
  `bench/results/2026-10-19-local-frame-metadata/README.md`.

- O lowering distingue ancestrais puramente nominais de classes alocáveis:
  marca abstratas/mixins e a raiz sintética inserida por `lower_program`,
  preservando a premissa ao juntar chunks. A prova exige ancestralidade,
  ausência de tabelas/funções de tabela e de identidade criada pelo runtime;
  tabelas ausentes em classes concretas continuam inválidas. Alocações ainda
  precisam de tabela/layout locais. Metadados e arestas de subtipo permanecem.
  A consulta de subtipo tem ABI conferida, sem aceitar Ref como CID. Passaram
  os 829 testes unitários nativos com `--include-ignored`, 31 dirigidos,
  Clippy estrito e docs do workspace. A execução compilada preservou seletores
  e relações de subtipo com registro amplo/fechado e estresse de GC. Fmt global
  segue pendente. O teste original de produção continua falhando, agora por
  `_AsyncFrame` (CID 1073741568) com ancestralidade não provada, em vez de
  `Object`. Frames assíncronos não foram classificados como meros metadados
  para contornar a prova. Sem redução comprovada do explorador, aprovação
  global ou execução nova Windows/Linux. Evidências:
  `bench/results/2026-10-19-nominal-class-metadata/README.md`.

- A prova experimental distingue declarações de tabelas importadas das definições
  locais. O inventário confere identidade símbolo/CID, corpo virtual e unicidade
  nos resumos verificados; o uso só ocorre após reconciliar o SDK selecionado e
  o universo local. Não autoriza chamadas nem alocações da classe importada.
  A regressão positiva falhava antes; passaram 28 testes finais da prova e nove
  do inventário, incluindo 662 declarações do SDK real. Os 826 testes unitários
  nativos passaram antes do ajuste final de ordem do guard e novas assertivas;
  os testes dirigidos finais cobrem esse refinamento. Clippy/docs estritos do
  workspace passam; fmt global segue pendente. O teste de produção continua
  falhando e agora aponta primeiro `Object` (CID 0), sem tabela própria, antes
  da checagem das declarações importadas. Metadados nominais/classes alocáveis
  e outras fronteiras ainda exigem prova; sem alegação de redução do explorador,
  aprovação global ou execução nova nas VMs. Evidências:
  `bench/results/2026-10-19-imported-table-declarations/README.md`.

- A prova experimental de tabelas fechadas tem diagnóstico opcional com
  `DARTFORGE_TRACE_CLASS_REACHABILITY=1`, junto à opção experimental já existente.
  Identifica a primeira fronteira sem relaxar a prova e descarta cobertura
  anterior/parcial ao rejeitar. No teste real, a primeira rejeição observada foi
  uma declaração de tabela importada de `dart:io` sem tabela local; o lowering
  declara tabelas do SDK, enquanto essa verificação exige definições locais.
  A poda de backends continua falhando; construtores estrangeiros já saem, mas
  getters/adaptadores/tabelas permanecem. Passaram 823 testes unitários nativos
  antes do refinamento das mensagens e 26 dirigidos na versão final, Clippy
  estrito e docs do workspace sem avisos. Fmt global segue pendente; sem nova
  aprovação global ou execução Windows/Linux. Evidências:
  `bench/results/2026-10-19-class-pruning-diagnostics/README.md`.

- A suíte global de `81e6762b` terminou com **exit 101**: builders nativos
  passaram (2410,20 s), assim como os quatro testes Angular (614,15 s), mas o
  gate do corpus exigia VM para oito casos JS-only que já declaravam referência
  DDC. A validação agora é compartilhada pelo CLI sem Forge e pelos dois gates:
  DDC precisa ser válido; a exceção da VM exige declaração e diagnóstico CFE de
  biblioteca indisponível. Timeout, ferramenta ausente, erro de sintaxe/runtime
  e rejeição no modo nativo seguem reprovando. Nenhum caso foi excluído e as
  14 divergências continuam exigidas e visíveis. Passaram 26 testes unitários,
  dois testes de corpus (241 programas: 233 referências VM válidas, incluindo
  negativos normalizados, e oito rejeições de plataforma) e um doctest.
  Probes reais confirmaram CLI anterior=1, corrigido=0, nativo=1 e sem Node=1.
  Clippy/docs do workspace e Clippy da CLI com JIT passam; fmt segue pendente.
  A combinação global de todas as features é inválida por contrato (`aot`/`dll`
  exclusivos), sem alteração desse guard. Não há aprovação global nova; a poda
  completa dos backends estrangeiros continua aberta. Evidências:
  `bench/results/2026-10-19-oracle-validity/README.md`.

- **Clippy estrito do workspace passa com todos os alvos**, sem novas supressões,
  na seleção padrão de features do macOS. Foram agrupadas as premissas de escrita
  de campos, opções ARC e evidências da auditoria; operandos/pontos de cópia têm
  seus dados associados. A enumeração de arestas preserva IDs e verificações de
  esgotamento. Fixtures e expressões do harness/JIT foram simplificadas, sem
  alterar expectativas nem inclusão de testes. Passaram 822 testes unitários
  nativos com `--include-ignored`, 122 doctests, 45 testes de biblioteca do JIT
  e 20 do diferencial. Docs do workspace passam sem avisos; fmt ainda reprova.
  Não certifica todas as features, novos testes nas VMs ou redução do binário.
  A suíte global de `81e6762b` passou pelo corpus completo de builders nativos
  sem filtro (2410,20 s) e continua nos testes Angular, sem resultado final.
  Evidências: `bench/results/2026-10-19-workspace-clippy/README.md`.

- Tipos de metadados nativos agora têm nomes: caches de despacho, tabelas de
  métodos, visitantes de raízes, locais/arestas async, Phis Ref e protocolos ARC.
  As representações e os algoritmos permanecem iguais. O helper de metadados
  de depuração usa `add_metadata`; a inicialização de tabelas e a ordenação do
  cache SDK foram simplificadas. Sem supressões, Clippy nativo caiu de 32/47
  para 10/25 diagnósticos (biblioteca/testes), mas ainda reprova. Passaram todos
  os 822 testes unitários com `--include-ignored` e os 122 doctests no macOS;
  docs do workspace continuam sem avisos. Fmt segue pendente. Não houve nova
  certificação Windows/Linux, redução do explorador ou solução da poda completa
  dos backends estrangeiros. A suíte global de `81e6762b` continua ativa nos
  builders nativos. Evidências: `bench/results/2026-10-19-native-metadata/README.md`.

- Documentação do workspace passa sem avisos com `RUSTDOCFLAGS='-D warnings'`,
  tanto pública quanto com `--document-private-items`. Foram corrigidos 28
  diagnósticos de links (nomes antigos, referências públicas a itens privados
  e `write` ambíguo). O helper `FieldDeclaration::identity`, usado apenas em
  testes, é compilado com `cfg(test)`, eliminando também o aviso de código morto.
  Os nove testes dirigidos de campos declarados passaram; os outros 813 foram
  filtrados, sem alegação de suíte nativa completa. Validação macOS e features
  padrão; não repetida nas VMs. Fmt global segue pendente; Clippy nativo mantém
  32 diagnósticos de biblioteca e 47 de testes. A suíte global da revisão-base
  `81e6762b` continua ativa nos builders nativos, sem resultado final.
  Evidências: `bench/results/2026-10-19-rustdoc-links/README.md`.

- LSP passa no Clippy estrito com todos os alvos, sem supressões: as 16
  pendências restantes foram resolvidas com tipos para requisições/metadados,
  opções de imports e consultas de ocorrências, aliases descritivos e duas
  enumerações de parâmetros. O despacho de renomeação usa `RenameKind`.
  Passaram os 258 testes regulares/protocolo e 17 doctests, sem alterar as
  expectativas. Validação macOS; sem nova certificação Windows/Linux ou de
  paridade completa com o SDK. Fmt global segue pendente; Clippy global falha
  no emissor nativo (33 biblioteca / 47 testes); docs compilam com avisos.
  A suíte global da revisão-base `81e6762b` continua ativa nos builders nativos,
  sem resultado final. Evidências: `bench/results/2026-10-19-lsp-quality/README.md`.

- LSP resolve os caminhos codificados de imports relativos e URIs `file://`
  antes de procurar arquivos, preservando o prefixo digitado nas sugestões.
  As duas regressões falhavam antes; passaram os 258 testes do LSP, incluindo
  três integrações e três testes de lookup. Seis cenários foram consultados no
  servidor oficial Dart 3.6.2, incluindo espaços, Unicode, `%25` e `%23`.
  A conclusão de `package:` mantém o comportamento anterior. Validação macOS;
  não houve nova execução Windows/Linux. Clippy do LSP segue falhando em 16
  diagnósticos (antes 17), sem supressões. Fmt/Clippy globais seguem pendentes;
  docs passaram com LLVM explicitamente selecionado, com avisos existentes.
  A suíte global sem filtros segue ativa na revisão-base `81e6762b`, no corpus
  de builders nativos; ainda não há resultado final nem certificação global
  desta alteração. Evidências: `bench/results/2026-10-19-lsp-uri-paths/README.md`.

- Cargo no macOS agora seleciona o linker compatível sem flags manuais: o
  wrapper do repositório usa `cc` para ligações comuns e examina arquivos LLVM
  antes de escolher Clang/LLD para bitcode. Bibliotecas LLVM não empacotadas
  (`-lLLVM*`) também são cobertas; escolhas explícitas de linker são preservadas.
  O SDK ativo é negociado quando necessário; `DARTFORGE_MACOS_SDK_ROOT` fixa um
  SDK, inclusive o ativo, sem substituição silenciosa. Passaram 19 testes Python,
  17 testes LLVM, 33 da CLI padrão e seis doctests JIT com Cargo normal.
  A CLI JIT foi montada sem `SDKROOT` e executou um programa com argumentos e
  closure com saída idêntica à VM 3.6.2. O builder nativo `cadeia_configuracao`
  passou com o compilador novo: 22 saídas iguais e três edições incrementais.
  A suíte global avançou além da ligação: 227 testes passaram antes da falta
  do compilador padrão `target/release/dartforge`; isso continua sendo falha da
  rodada global, não aprovação. Fmt/Clippy globais seguem pendentes, e docs
  geram avisos. O action macOS deixa de impor linker/rustdoc flags globais e
  executa os testes do seletor; não há nova certificação Windows/Linux.
  Evidências: `bench/results/2026-10-19-cargo-macos-linker/README.md`.

- A análise experimental de tabelas fechadas admite entradas tipadas de closures
  locais com ABI conferida, contexto `Ref` e até 12 argumentos representáveis.
  A leitura direta do campo de código precisa vir de um handle local protegido;
  o endereço só pode servir à chamada com assinatura exata. Mutação, escape,
  endereço forjado ou lido como objeto continuam recusados. As leituras de classe
  e ABI usadas pelo lowering também são cobertas. O teste positivo falhava antes;
  passaram os 25 casos dirigidos, incluindo 16 recusas e a comparação em 32
  binários/64 execuções com registro amplo/reduzido e GC normal/forçado, argumentos
  e retornos escalares e contexto capturado. A fixture não importa SDK.
  Passaram 822 testes unitários nativos com `--include-ignored` e 121 doctests;
  os 25 casos dirigidos foram repetidos após a última expansão da matriz.
  Clippy mantém 33/47 diagnósticos de lib/teste, sem novos nos arquivos alterados;
  checks globais de fmt/Clippy/link continuam pendentes, com avisos na documentação.
  A inicialização do SDK continua sem prova completa; o teste real de remoção
  dos backends estrangeiros foi repetido e ainda falha, sem relaxar a exigência.
  A opção permanece `DARTFORGE_CLOSED_CLASS_REACHABILITY=1`; não há nova alegação
  de redução do explorador. Evidências e checks:
  `bench/results/2026-10-19-closed-typed-closures/README.md`.

- A configuração Cargo deixa de exportar um caminho absoluto de LLVM para
  Windows em todos os sistemas. O seletor usa o prefixo explícito ou procura
  `llvm-config` 22.1 no `PATH`, sem presumir `/usr/lib/llvm-22`; consulta a versão
  da instalação selecionada e preserva erros de configuração explícita.
  `llvm-sys` também exige a linha 22.1 para manter seus cabeçalhos compatíveis.
  Com o LLVM real disponível só no `PATH`, o script anterior falhava e o novo
  passa. Cinco testes de seleção e seis cenários do script real cobrem
  precedência, descoberta e recusas. Os 17 testes do pacote LLVM passaram pelo
  caminho sem variáveis de prefixo, incluindo emissão dos cinco destinos; isso
  não é execução de binários Windows/Linux. Clippy estrito do pacote passa com
  todos os alvos (`--no-deps`). Checks globais mantêm falhas de fmt/Clippy/link
  Apple/LLVM 22; documentação gera avisos. Evidências:
  `bench/results/2026-10-19-llvm-installation/README.md`.

- Os testes de execução de builders deixam de aprovar seleção vazia, nomes de
  casos desconhecidos ou pré-requisitos ausentes. A execução pela VM também
  exige zero saídas pendentes: com um Dart inexistente, antes aprovava com 45
  pendências; agora reprova. Seis testes regulares cobrem os critérios. A rodada
  real com Dart 3.6.2 passou nos quatro testes VM, com 60 saídas idênticas em dez
  projetos, zero pendências e equivalência incremental/do zero nas 12 edições.
  O executor nativo próprio passou em `cadeia_configuracao`: 22 saídas iguais,
  zero pendências e três edições incrementais; não certifica os demais builders
  nativos. Passaram 66 testes regulares do motor, 16 doctests e Clippy estrito
  com todos os alvos (`--no-deps`); 18 testes continuam ignorados na seleção
  regular. Checks globais mantêm falhas de fmt/Clippy/link Apple/LLVM 22;
  documentação gera avisos. Evidências:
  `bench/results/2026-10-19-builder-executor-gates/README.md`.

- A detecção de builders agora usa o leitor YAML da configuração, reconhece
  chaves entre aspas/mapas em uma linha e não aciona o motor para definições
  vazias ou documentos posteriores ignorados pelo leitor. Arquivos ilegíveis,
  sintaxe inválida e seções de tipo incorreto seguem para validação em vez de
  permitir fontes geradas antigas. Quatro testes de detecção e dois novos
  testes de CLI falhavam antes; passaram os 60 testes regulares do motor (18
  ignorados nessa rodada), 16 doctests, quatro integrações da CLI e o gate de
  custo zero com seções vazias. O corpus adicional cobriu 11 projetos (10 planos
  iguais e um controle sem motor): 15 saídas iguais, 45 pendentes por falta de
  executor/fallback e nenhuma diferente; não é execução de todos os builders do ecossistema.
  Clippy estrito do motor e da CLI passou com `--no-deps`; checks globais
  mantêm pendências de fmt/Clippy/link, com avisos na documentação. Evidências:
  `bench/results/2026-10-19-builder-detection/README.md`.
- `compile-native --emit-ir` e `--digest` agora usam o motor de geração de
  fontes, como o AOT e o JIT. Duas regressões reproduzidas no compilador anterior
  cobrem fonte ausente e configuração inválida ignorada diante de fonte antiga.
  Passaram os dois testes novos e os dois casos existentes de i18n real em
  AOT/JIT, além de 33 testes unitários da CLI. Em um programa sem builders, o
  IR permaneceu idêntico byte a byte e os manifestos só diferem pelo caminho
  de saída. Seis cenários reais de `run`, `reload` e `reload --restart` preservam
  argumentos e os códigos de saída 0/37. O Clippy estrito da CLI passa com todas
  as features usando `--no-deps`; os checks globais ainda falham em
  fmt/Clippy/link Apple/LLVM 22, e docs geram avisos. Rodada macOS, sem nova
  certificação Windows/Linux. Evidências:
  `bench/results/2026-10-19-generated-native-ir/README.md`.
- A prova experimental de alcance de classes aceita closures uniformes locais e
  tear-offs com ABI conferida, sem escape ou escrita do handle. O receptor
  implícito também é protegido, sem presumir sua identidade só pela assinatura.
  As alocações transitivas do corpo mantêm suas tabelas; tabelas alheias saem.
  Três testes novos incluem 16 recusas e seis binários comparando registro
  amplo/reduzido sob GC normal/forçado (12 execuções). O teste positivo falhava
  antes; passaram 818 testes unitários nativos com `--include-ignored` (incluindo
  os 21 de alcance) e 121 doctests. A opção continua
  experimental: closures tipadas/encaminhadas/importadas e a inicialização
  completa do SDK permanecem fora da prova. O teste real de remoção dos
  backends estrangeiros ainda falha, sem relaxamento. Checks globais seguem
  com pendências de fmt/Clippy/link; docs geram avisos. Evidências:
  `bench/results/2026-10-19-closed-uniform-closures/README.md`.
- `pub get` só publica metadados hospedados depois de validar os candidatos;
  respostas rejeitadas deixam utilizável o cache offline anterior. Duas
  regressões falhavam antes; três testes novos cobrem rejeição, primeira
  resposta inválida e substituição válida. Passaram 39 testes da CLI com
  features padrão, Clippy estrito nessa configuração e 17 execuções do binário
  real contra registro local, preservando cache, lockfile e configuração após
  oito respostas inválidas. Rodada macOS; não repetida nas VMs. Clippy com
  todas as features ainda aponta pendências nativas/JIT; checks globais seguem
  falhando em fmt/Clippy/link Apple/LLVM 22, e docs geram avisos. Evidências:
  `bench/results/2026-10-19-pub-cache-validation/README.md`.
- Compiladores Windows/Linux remontados de `2b923a60` e bundles renovados com
  fontes de runtime coincidentes. O JIT macOS otimizado revelou 29 símbolos
  públicos ausentes de CoreFoundation/Security no catálogo gerado; foram
  conferidos no SDK e os cinco testes de ligação passaram em release.
  Explorador real e menus passaram em AOT/JIT nos três sistemas. Windows usa
  x64 em VM ARM64; seu menu AOT foi compilado no macOS. Os executáveis atuais
  macOS → Windows/Linux foram executados nos destinos com hashes conferidos.
  A rodada Linux após o OOM passou com um compilador por vez, depois do reinício
  forçado autorizado. Checks globais ainda têm pendências de fmt/Clippy/link;
  docs geradas com avisos. Evidências e limites:
  `bench/results/2026-10-19-desktop-refresh/README.md` e
  `RELATORIO-CROSS-COMPILACAO-DESKTOP.md`.
- O leitor do inventário do SDK verifica publicação, runtime, objetos e resumos,
  recalcula os fechamentos de alocação por componentes fortemente conexos e só
  admite importações com fechamento vazio, sem exposição e ABI compatível.
  Declarações globais repetidas ficam opacas; resumos obsoletos e colisões de
  símbolos/classes impedem a prova. No SDK real, reconheceu 428 fechamentos
  verificáveis entre 32.676 funções únicas; a inicialização continua desconhecida.
  A opção segue experimental e não completa a poda do SDK: o teste de remoção
  de backends estrangeiros permanece falhando, sem mudança na exigência.
  Passaram 815 testes unitários nativos com `--include-ignored`. Os sete testes
  regulares do leitor e dois de integração com o grafo usam artefatos sintéticos
  não executáveis; o teste ignorado do SDK foi executado com objetos reais.
  Clippy nativo segue em 33/47 diagnósticos, sem novos nos módulos alterados;
  formatação e link padrão do workspace continuam pendentes; docs geradas com
  avisos. Evidências: `bench/results/2026-10-19-sdk-inventory-consumption/README.md`.
- A prova experimental de alcance de classes agora recusa destinos alcançáveis
  sem corpo, chamadas locais/runtime com ABI incompatível, entradas uniformes
  inválidas, endereços de funções e memória nativa sem cobertura. Reutiliza a
  testemunha estrutural de SSA/CFG; slots locais e globais precisam de declaração
  e representação compatíveis. Uma reanálise falha descarta conjuntos temporários
  e antigos. Tabelas de classes canônicas criadas implicitamente por boxing,
  strings e exceções são protegidas mesmo sem vetor de registro do módulo.
  Quando a prova fecha, o emissor omite também as definições de tabela inativas.
  Três regressões falharam antes; foram adicionados 12 testes. Passaram 805 testes
  unitários nativos com `--include-ignored` e 121 doctests. A fixture de despacho
  gera e executa binários com registro amplo/fechado, com e sem `GC_STRESS`;
  não importa SDK e adapta apenas o wrapper `main` para ligar ao runtime sem SDK.
  O teste de produção repetido ainda falha na remoção de tabelas estrangeiras.
  Naquela rodada, o modo opt-in recusava SDK importado; não há alegação de cobertura
  completa dessas fronteiras. Sem novas supressões/diagnósticos Clippy nos arquivos
  alterados. Checks globais seguem pendentes (formatação, Clippy e link padrão
  Apple/LLVM 22); docs geradas com avisos. Detalhes e limites em
  `docs/NATIVO-PODA-DE-TABELAS.md` §7 e
  `bench/results/2026-10-19-class-coverage/README.md`.
- Os leitores do conversor nativo agora rejeitam offsets/varints fora do limite,
  estouros de frame/retorno/slots e índices inconsistentes nos mapas compactos
  DFGM/DFPC. Cada função é lida dentro do próprio stream; contagens de tabelas e
  registros LLVM são verificadas antes da alocação. A busca de função por endereço
  evita somar tamanho a um endereço próximo de `u64::MAX`. Cinco regressões
  falham no código anterior; os 13 testes isolados dos dois módulos passam em
  debug e com otimização. São sete testes novos no crate. Passaram os 793 testes
  unitários nativos com `--include-ignored`; um teste manual existente retorna
  sem executar aplicação quando `DARTFORGE_PROGRAM` não está definido, como nesta
  rodada. O restante da tentativa do crate teve 27 integrações passando e uma
  falhando: `platform_constant_prunes_the_other_backends` ainda encontra getters
  e tabelas dos outros sistemas, embora seus construtores já tenham saído.
  Um produtor anterior também apresenta essas definições; o teste foi mantido.
  A poda completa de tabelas com SDK importado continua pendente. Os 121 doctests
  passaram em execução separada, assim como o teste de seleção de SDK. O Clippy de emit-native caiu de 154/192 erros (lib/test) para 33/47,
  sem novas supressões; CLI e LSP também têm pendências. Formatação global e
  link padrão Apple/LLVM 22 continuam falhando; docs geradas com avisos. Não se
  trata de certificação de todo objeto malformado nem dos leitores independentes
  do runtime. Evidências e reprodução:
  `bench/results/2026-10-19-native-reader-quality/README.md`.
- Parity agora compara tipo do diagnóstico, severidade, mensagem/correção e
  coordenadas, reservando correspondências exatas antes de parear diferenças
  com a mesma chave. Quatro regressões falharam antes e passam após a correção,
  inclusive um pânico ao cortar caminho UTF-8. `probe_files` não descarta
  duplicatas e retorna falha para diferenças, fontes/referências inválidas ou
  análise incompleta; preserva metadados existentes do pacote. A gravação de
  oráculos valida a versão real do SDK e preserva fontes e referências quando
  a ferramenta falha, em vez de remover arquivos automaticamente. Passaram 210
  testes de analysis/parity e 16 cenários reais de CLI; a falha pós-validação
  do gravador foi exercitada com um substituto controlado, identificado como tal.
  Parity passa no Clippy estrito com todos os alvos, sem novas supressões.
  SDK 3.13.4 positivo não foi executado; a comparação cobre a projeção `Record`,
  não todos os campos JSON. Checks globais: formatação pendente, Clippy para em
  emit-native, testes param no link Apple/LLVM 22; docs geradas com avisos.
  Evidências, reprodução e limites: `bench/results/2026-10-19-parity-gates/README.md`.
- A sessão residente agora aplica o filtro de erros obrigatórios de linguagem
  e atribuição definida usado por `compile-js`: a leitura de um `final` não
  atribuído rejeita a revisão antes da emissão, preservando o JavaScript válido
  anterior. Avisos comuns de tipos continuam não fatais. Qualquer erro invalida
  hashes, dependentes, fragmentos e texto de módulos em memória; a nova tentativa
  recompila mesmo sem outra edição. Dois testes falharam antes das correções:
  aceitação da leitura inválida e retry de erro de gravação que deixava `main.js`
  ausente. A recuperação agora coincide com compilação limpa. O medidor de edições
  restaura bytes da fonte ao desenrolar um pânico, inclusive num teste com falha
  real de `Session` e SDK; a restauração normal não sobrescreve uma mudança
  posterior. A guarda depende da restauração de I/O e não cobre abort/kill.
  O teste WebSocket usa handshake e frames reais por TCP local, com limites
  de 125/126 e 65535/65536 bytes, UTF-8 e EOF, substituindo uma cópia da lógica.
  Passaram 104 testes dirigidos (13 dev, três do medidor e 88 de produção), sem
  ignorados nas execuções. O medidor está registrado no `cargo test` normal de
  dev. Dev e emit-js-production passam no Clippy com todos
  os alvos, sem novas supressões. Não foi medido ganho de desempenho. Checks
  globais ainda falham; esta execução do Clippy reporta parity/emit-native,
  a formatação geral e o link Apple/LLVM 22 continuam pendentes. Documentação
  gerada com avisos. Evidências e limites:
  `bench/results/2026-10-19-production-dev-quality/checks.json`.
- Analysis passa no Clippy com todos os alvos, sem novas supressões. O cache de
  dependências consulta `&str`, separado por modo normal/dev, em vez de copiar
  o `pubspec.yaml` em cada acerto; mantém o limite combinado e resultados próprios.
  Na sonda isolada de 10 mil acertos sobre 16.464 bytes, as alocações caíram de
  40 mil para 30 mil e os bytes alocados de 165.270.000 para 630.000. Isso não
  mede memória residente, parsing nem tempo total do analisador. Dois testes
  cobrem modos, resultados ausentes, mutação e esvaziamento do cache. Passaram
  193 testes de analysis/parity, sem ignorados. Os quatro testes de paridade
  agora falham sem SDK e respeitam caminho explícito inválido; o teste principal
  recusa pânicos e diagnósticos extras. A ausência de SDK foi exercitada como
  falha esperada (101). Uma execução nova do Dart 3.6.2 conferiu 417 diagnósticos
  dos 89 casos em arquivo, código, linha, coluna e comprimento (não mensagens
  ou severidade). Clippy global ainda falha, assim como formatação geral e link
  padrão Apple/LLVM 22; documentação gerada com avisos existentes. Evidências:
  `bench/results/2026-10-19-diagnostics-quality/checks.json`.
- World e emit-js passam no Clippy com todos os alvos, sem novas supressões.
  Bounds genéricos reaproveitam o `Box`, a busca de extensões empresta o tipo
  receptor, e estruturas complexas de parâmetros, atribuições e memos têm nomes.
  Passaram 115 testes: 25 de emissão/world, dois do utilitário de corpus e 88
  de produção, com os ignorados habilitados. Dos 208 programas de topo do corpus
  JS, os 207 aceitos produziram texto idêntico ao da revisão anterior. A fixture
  negativa restante foi recusada também pelo Dart oficial. O utilitário antigo
  retornava sucesso mesmo nessa recusa; agora falha, remove saída antiga do
  arquivo recusado e rejeita entradas ausentes, lista vazia e colisões de nomes.
  Cinco invocações inválidas reais foram conferidas. Determinismo exige ambos
  os arquivos do repositório e módulos não vazios. Isso não mede ganho de tempo
  nem comprova execução diferencial dos 207 programas. Clippy global ainda falha
  em analysis, dev, emit-js-production e emit-native; formatação geral e link
  padrão Apple/LLVM 22 continuam pendentes. Documentação gerada com avisos.
  Evidências: `bench/results/2026-10-19-js-quality/checks.json`.
- Build passa no Clippy com todos os alvos, sem novas supressões. A busca de
  componentes do grafo move o cursor suspenso em vez de copiar a lista inteira
  de dependências ao visitar cada filho; contextos privados agrupam eventos de
  invalidação e dados do planejamento. A ordem coincide com `graphs` 2.3.2 em
  192 grafos, e a cadeia de 50 mil nós passa sem recursão. No benchmark
  sintético de estrela com 10 mil nós, as medianas foram 22,723 ms antes e
  0,810 ms depois; isso não mede o tempo de um build inteiro nem memória total.
  Passaram 88 casos dirigidos: 54 unitários, 15 doctests, um teste de oráculo,
  seis de corpus, três de perfis, quatro de executor VM, quatro Angular e um
  de executor nativo (somente `cadeia_configuracao`, com produtor congelado da
  rodada anterior). A varredura Angular comparou 352 edições com motores novos.
  O teste de dependência iniciou antes do `pub get` terminar e falhou por
  ausência de `package_config.json`; a repetição após a preparação passou.
  Clippy global ainda falha em world (21), emit-js (31) e analysis (45);
  formatação geral e link padrão Apple/LLVM 22 continuam pendentes. A geração
  de documentação passou com avisos existentes. Evidências e limites em
  `bench/results/2026-10-19-build-quality/checks.json`.
- Types, macros-host e ng-generator passam no Clippy com todos os alvos,
  sem novas supressões. A refatoração nomeia estruturas de diagnóstico e
  agrupa argumentos relacionados; `compile_asset` recebe `AssetOptions`.
  `html::Node::Element` usa `Box`: o tamanho inline de `Node` no macOS ARM64
  caiu de 344 para 40 bytes (não é uma medição do consumo total nem de tempo).
  Passaram 119 testes de tipos, 74 do gerador e 21 de macros, incluindo os
  ignorados e os quatro casos de executor nativo com compilador montado nesta
  rodada. Os 372 artefatos oficiais do corpus Angular foram conferidos, sem
  pendências. A configuração local antiga apontava para caches Windows;
  `pub get --offline --enforce-lockfile` restaurou os caminhos. O teste agora
  verifica os diretórios das dependências e exige todos os artefatos, em vez
  de aceitar só dois. Suíte nativa: 777 passaram, zero falhas, nove ignorados;
  integração Sass/build passou. Restam os diagnósticos de Clippy em build
  (outros crates ainda podem ter mais), formatação geral e link Apple/LLVM 22.
  Evidências: `bench/results/2026-10-19-analysis-quality/checks.json`.
- Elements e formatter passam no Clippy com todos os alvos, sem supressões.
  A inserção mutável de nós foi renomeada de `Tree::new` para `Tree::push`, com
  exemplo executável e índice verificado antes de modificar a árvore. A
  varredura do formatador agora exige SDK e corpus não vazios, aceita
  `DARTFORGE_SDK_LIB` e propaga erros de leitura; o leitor de casos também não
  descarta entradas com erro. Suíte dos dois pacotes: 73 testes passaram,
  incluindo ignorados e doctests. SDK 3.6.2: 426 arquivos aceitos, zero falhas
  de cobertura de tokens e 426/426 saídas idênticas ao `dart format`, usando
  referências novas e uma reprise após a última alteração. SDK inexistente
  falhou como esperado, sem fallback. Restam diagnósticos globais em types,
  macros-host e ng-generator, a formatação geral e o link padrão Apple/LLVM 22.
  Evidências: `bench/results/2026-10-19-formatter-loader/checks.json`.
- Sass moderno usa a gramática de valores de declaração nas regras `@`
  desconhecidas: comentários `//` deixam de vazar para o CSS, delimitadores
  balanceados são respeitados e valores da sintaxe indentada podem continuar
  dentro de parênteses. Casos SCSS/Sass coincidiram com os oráculos oficiais
  1.101.0, 1.101.2 e 1.102.0 nos dois estilos; a rejeição de um corpo antes de
  fechar parênteses/colchetes e de comentários silenciosos em CSS puro também
  foi conferida. O modo 1.66 manteve suas saídas e sua regra de fim de linha,
  verificada contra a versão oficial. A divergência de `f24` saiu da
  lista; `f26_pai_na_raiz` continua pendente. O teste de cores que só imprimia
  valores agora verifica 128 canais/alfas contra Dart Sass 1.102.0. Suítes:
  15 testes Sass, 12 LLVM e um de integração com sass_builder passaram; Clippy
  com todos os alvos passou em LLVM/Sass. O verificador RS4GC agora nomeia a
  estrutura das entradas de phi. Restam avisos do Clippy em elements/formatter,
  a formatação geral e o link padrão Apple/LLVM 22. Evidências e comandos:
  `bench/results/2026-10-19-sass-at-rules/checks.json`.
- Frontend e runtime passam no Clippy com `--all-targets -- -D warnings`, após
  correção de 18 diagnósticos sem supressões. A coleta do corpus agora propaga
  erros de leitura e exige arquivos Dart: o teste do lexer deixava um SDK
  ausente passar e ignorava fontes ilegíveis. Três regressões cobrem entradas
  ausentes/vazias, UTF-8 inválido e contagem de arquivos rejeitados em subpastas;
  a execução externa com SDK ausente também falhou como esperado. Suíte dos
  dois pacotes, incluindo ignorados e doctests: 672 passaram. Aceitação do SDK
  3.6.2: 426/426 arquivos; corpus pdf/http/collection/intl: 367/367. Emissor
  nativo: 777 passaram, zero falhas, nove ignorados. O workspace ainda falha no
  Clippy em outros crates e no link padrão Apple/LLVM 22; formatação geral
  permanece pendente. Documentação compilou com avisos. Comandos, versões e
  limites: `bench/results/2026-10-19-core-quality/checks.json`.
- `scripts/cargo-macos-llvm.py` monta/testa o compilador Rust no macOS com o
  `ld64.lld` do LLVM selecionado e verifica o SDK antes de iniciar o Cargo.
  Contorna a incompatibilidade entre o linker Apple e o bitcode LLVM 22 sem
  alterar configurações globais. Quatro testes do comando passaram. O teste de
  bitcode agora também valida o formato encapsulado do Darwin por leitura de
  volta no LLVM; a suíte de `dartforge-llvm` passou com o comando. Uso e limites
  em `docs/CROSS-COMPILACAO.md`.
- Compilação cruzada com `--target`/`--target-bundle` e `export-target`; o
  explorador real executou em AOT e JIT no macOS ARM64, Ubuntu ARM64 e Windows
  x64 na VM ARM64. Executáveis cruzados também rodaram nos destinos. Escopo,
  correções do JIT COFF e limites: `RELATORIO-CROSS-COMPILACAO-DESKTOP.md`.
- O certificado experimental de `clamp(0, 1)` foi alinhado aos nativos numéricos
  embutidos em HIR. A exigência antiga de implementação no runtime impedia toda
  certificação válida. Seis testes dirigidos passaram; 373 resultados de clamp
  e 2.736 de cor coincidiram com VM/AOT oficiais, OFF/ON e sob GC stress. O LLVM
  confirma caminhos rápidos ativados. A opção continua experimental e desligada
  por padrão; não há nova alegação de desempenho.
  Evidências: `bench/results/2026-10-19-clamp-embedded/`.


- Corrigida a disponibilidade de enums canônicos em arestas de `phi`, incluindo
  a leitura quente do cache sob sua própria guarda de inicialização. Sete testes
  dirigidos passaram; suíte nativa: 769 passaram, duas falhas anteriores restam,
  nove ignorados. Binários OFF/ON do teste de identidade de enums e de 2.736
  saídas de cor coincidiram com VM/AOT oficiais e sob GC stress. A correção não
  remove getters/checks nem ativa a especialização experimental por padrão.
  Detalhes: `RELATORIO-ENUM-PHI.md`.

- Corrigido falso negativo do teste de notifier ARC: ele dependia de uma forma
  antiga de inlining e procurava o guarda escalar no corpo errado. A regressão
  agora acompanha as leituras reais, confere representação/vetor de layouts e
  recusa a ausência de layout admissível. Os 14 testes dirigidos passaram;
  suíte ampla: 770 passaram, uma falha de especialização de cor resta, nove
  ignorados. Tracing/ARC coincidiram com a VM oficial, também sob stress e
  ARC_CHECK; código de produção ARC e fixture Dart permaneceram intactos.
  Detalhes: `RELATORIO-ARC-NOTIFIER.md`.

- A especialização de contextos de cor agora protege os índices observados dos
  enums e encaminha falhas da guarda ao corpo genérico, mantendo ABI e exceções.
  Reexecução só é admitida em prefixo único sem efeitos; limites de análise e
  crescimento permanecem explícitos. Suíte nativa: 777 passaram, zero falhas,
  nove ignorados; 15 testes dirigidos passaram. Na rodada pareada de um milhão
  de conversões, OFF/ON registraram 32.373/30.338,5 µs (cerca de 6,3% menos tempo),
  contra 5.743 µs no AOT oficial. É um resultado localizado, com opção ainda
  desligada por padrão. Detalhes: `RELATORIO-COR-GUARDADA.md`.

## Consolidação de 13–15/10/2026: últimas 48 horas

Esta seção atualiza o retrato geral. As seções anteriores permanecem como
histórico de seus produtores, datas e limites; seus números não são medições
novas do `main`. Os registros abaixo identificam os checkpoints realmente
compilados e testados. Não há declaração de paridade completa com o SDK Dart.

### AOT, HTTP/JSON e tamanho

- O File Explorer Dart comum passou a compilar e executar em AOT, com
  renderização, navegação, retorno entre pastas e fechamento normal verificados
  em macOS. A validação usa o caminho de UI com host; não comprova AppKit direto
  na thread principal em todos os modos. O programa original foi preservado;
  sondas e navegação usam cópias/fixtures controlados.
- Entraram propagação explícita do SDK escolhido, especialização de flags de
  plataforma, poda de CFG morto, preservação de constantes/records/interpolações
  UTF-16, devirtualização com prova do receptor físico e igualdade de padrões
  enum somente com identidade/implementação certificadas.
- Leitura direta de armazenamento de listas, fechamento de arrays JSON sem
  buffer intermediário e cópia Latin-1 conferida reduziram trabalho em caminhos
  específicos. Isso não estabeleceu melhoria do servidor HTTP: a medição de
  reserva de listas registrou **54 ms de CPU/1.000 requisições JSON** antes e
  depois, contra **34 ms** no AOT oficial; texto foi 52→50 contra 34 ms.
  Uma conexão keep-alive e cliente local: não é capacidade máxima nem p95.
- Certificados do SDK e guardas de observadores permitem experimentos de
  eliminação de boxing, multiplicação/clamp primitivos e especialização de
  contextos constantes. A prova continua recusando fontes alteradas, aliases
  nativos ou observadores incompatíveis. Experimentos não são garantias gerais
  para `dynamic`, mapas heterogêneos ou FFI.
- No kernel consumido de cor, uma rodada pareada registrou **75.322,5→72.195 µs**,
  contra **5.472,5 µs** no upstream. Houve melhora localizada, mas o DartForge
  permaneceu muito mais lento. Não extrapolar para todo o dart_ui/HTTP/JSON.
- REA **6.5.0**, Node, JDK e Ghidra foram instalados no SSD e usados localmente
  nos executáveis reais, incluindo o snapshot ELF anexado ao launcher Mach-O
  upstream. A atribuição por intervalos disjuntos evita somar aliases ou comparar
  apenas o launcher oficial com todo o nosso código.
- Na base inicial comparável, o File Explorer ocupava **28.475.840 bytes** contra
  **11.682.880** no upstream. Em outro checkpoint congelado, especialização
  constante OFF/ON produziu **28.293.424/28.293.776 bytes**: +352 bytes, sem ganho
  de desempenho comprovado no aplicativo. Não é uma série longitudinal isolada.
  O problema de tamanho/desempenho do AOT permanece aberto.
- O inventário de alocações do SDK agora é separado dos registros ambíguos de
  classes, ligado a artefatos/ABI/fontes exatos e conservador em memória bruta e
  chamadas desconhecidas. A validação independente encontrou **429 corpos
  comprovadamente vazios e 32.295 desconhecidos**. Ainda não existe consumidor
  que use esse inventário para podar classes: nenhum ganho de tamanho é atribuído
  a esse trabalho.

Evidências e plano: `docs/PLANO-NATIVO-TAMANHO-PERFORMANCE.md`,
`bench/results/2026-10-14-rea-native-investigation/`,
`2026-10-14-rea-size-followup/`, `2026-10-14-native-list-reserve-http/`,
`2026-10-14-native-latin1-char-codes/`,
`2026-10-14-primitive-sdk-production/performance-v3/`,
`2026-10-14-constant-specialization/file-explorer/` e
`2026-10-15-complete-allocation-summary/` (diretórios sob `bench/results/`).

### JIT, hot reload e File Explorer

- Corrigidas identidades de campos privados por biblioteca/classe declaradora,
  comparação semântica dos tipos antes da publicação e preservação da ABI dos
  frames async por corpo/geração, inclusive após inlining.
- Argumentos do programa e configuração de pacotes chegam ao runtime. Recursos
  do pacote, fontes e ícones voltaram a resolver corretamente. Na ausência de
  `package_config.json`, APIs de URI seguem o comportamento nullable da VM,
  preservando URIs comuns sem configuração artificial.
- O File Explorer JIT teve ícones/fontes, navegação raiz↔filha, reload de corpo,
  recusa de edição inválida mantendo callbacks anteriores, nova edição válida e
  fechamento por Cmd+W verificados. `pthread_main_np()` foi **0** nesse JIT:
  trata-se da integração com host, não de execução AppKit na thread 0.
- Adaptadores/metadados de callback não equivalem a publicação de endereço.
  Gerações sem código escapado podem ser recuperadas; callbacks realmente
  publicados mantêm seus donos de código, inclusive tombstones após `close()` e
  descarte da sessão. Capturas anteriores à publicação e callbacks entre sessões
  têm regressões específicas. A retenção conservadora ainda pode crescer com
  callbacks/reloads; não há certificação de uso prolongado de memória.
- Stubs eager do JIT são experimento **desligado por padrão**. Rollback e reuso
  de slots órfãos foram testados; a medição grande de partida foi cancelada no
  fechamento. Não há ganho de startup publicado para esse experimento.
- Após corrigir descoberta do SDK, **45 testes paralelos do JIT e duas
  integrações passaram localmente** sem a variável de override de testes. A
  preparação do driver C usa SDK macOS explícito; caminhos Windows nos fixtures
  async são literais raw. Estes resultados não certificam todo programa Dart.

Evidências: `bench/results/2026-10-14-jit-private-field-identity/`,
`2026-10-14-jit-field-type-identity/`, `2026-10-14-jit-file-explorer/`,
`2026-10-15-jit-package-config/` e
`2026-10-15-jit-callback-sdk-discovery/`.
O índice antigo `2026-10-14-jit-arc/index.json` conserva o diagnóstico histórico
anterior à distinção entre metadados e publicação; não deve substituir os testes
posteriores de recuperação de gerações.

### ARC e integração com dart_ui

- ARC ganhou provas/refinamentos de origens físicas de campos, tratamento de
  Phi/ownership em caminhos excepcionais, eliminação de RC para null/inteiros
  tagged comprovados e correções das fronteiras dinâmicas. Frames, loops,
  keepalive, campos, unwind estrangeiro e retornos mantêm verificações positivas
  e contraprovas que devem falhar.
- Candidatos de ciclos mortos usam backlinks conferidos por geração; consulta
  redundante de metadados foi removida. A medição pareada posterior não mostrou
  ganho material de CPU: árvores/JSON variaram cerca de 1 ms e ARC preparado
  continuou mais caro. Não declarar ARC competitivo com tracing/upstream.
- As provas AOT foram corrigidas para chamar o helper ARC realmente emitido,
  preservando chamadas cruas do harness e contagens exatas. A rodada local
  consolidou **336 execuções AOT** e **17 testes dos exemplos**; traps esperados
  foram contados como contraprovas, nunca como programas válidos.
- `package:dartforge` unifica SIMD, anotações, bomba nativa, callbacks, GC,
  recursos, bibliotecas e memória mapeada. `packages/dartforge_simd` foi integrado
  e removido. Imports e código continuam Dart comum, com capacidades/fallbacks.
- `PreferRegion`, `PreferAcyclic`, `NoEscape` e `Realtime` expressam preferências
  ou obrigações verificáveis, não fatos assumidos. Preferências sem prova mantêm
  a política geral; contratos sem prova são recusados no modo de verificação.
- Há sondas de callback de thread estrangeira e retenção de código, mas não
  validação de hardware de áudio nem certificação realtime. Reentrância modal,
  progresso seguro de callbacks síncronos e as demais obrigações de P1–P17
  continuam com os limites registrados no relatório e na especificação ARC.

Evidências: `bench/results/2026-10-14-arc-candidate-cpu/`,
`2026-10-15-arc-proof-call-targets/` e
`2026-10-15-release-fixes/arc-source-return/`;
`docs/ARC-IMPLEMENTACAO.md`, `docs/ARC-CICLOS-ESPECIFICACAO.md` e
`RELATORIO-DARTFORGE-PARA-DART-UI.md`.

### JavaScript de desenvolvimento e produção

- Corrigidos ordem de avaliação de atribuições indexadas/compostas, receivers,
  argumentos eager em inlining e resultados de atribuição por extensão.
  Laços tipados exigem guardas que não invoquem getters públicos substituíveis.
- Em Node, a rodada de desenvolvimento mediu razão DartForge/DDC de **0,95–0,95**
  nos dois kernels tipados, **0,93** em shapes, **1,13** em closures e **1,08**
  em fib. São kernels, não paridade geral de aplicação nem tempo de compilação.
- Produção checked ainda ficou **1,22–1,75×** mais lenta em kernels JSON e
  **3,59×** em UTF-8; coleções de objetos ficaram **1,75–3,10×** mais lentas que
  dart2js O2. Há kernels próximos ou melhores, sem compensar automaticamente
  essas regressões.
- Tamanho: `limitless_ui` **3.535.556 bytes** (gzip9 **1.020.032**) contra O2
  **3.870.230** (**1.020.812**); `new_sali` **7.625.133** (**2.078.667**) contra
  **8.022.590** (**2.110.700**). O hello continua maior: **46.212/16.046** contra
  **33.595/11.106**. O4 tem contrato menos verificado e é listado separadamente
  nos registros; não é substituto silencioso de O2.
- O primeiro protótipo de append com consulta por escrita regrediu **3,64×** e
  foi rejeitado. A versão de janela fechada, opt-in, mediu **69.534→29.850 µs**,
  contra **40.332 µs O2/32.403 µs O4**, com +794 bytes/+387 gzip no kernel.
  Continua **desligada por padrão**; não é ganho de JSON/HTTP.
- O experimento adicional de store UTF-8/JSON não foi concluído e foi retirado
  integralmente do compilador. Patch/evidências ficam arquivados; a gate JS
  após a retirada passou em **92 testes**.
- O fixture de getters captura o contador antes do console I/O. Um controle
  explícito simula console tocando `byteLength`, reproduz a contaminação antiga
  e conserva comparações estritas entre VM, dart2js e modos DartForge.

Evidências: `bench/results/2026-10-14-current-js-status/summary.json`,
`2026-10-15-js-fresh-list/benchmark-v2/`,
`2026-10-15-js-json-utf8-store/` e os testes de `typed_public_accessors`.

### Analyze, LSP e format

- No placar de 14/10, **23.001/23.012** diagnósticos acertaram código/posição;
  **22.996** também acertaram mensagem; **11 ausências, cinco mensagens
  divergentes**, sem extras/posições erradas. São oráculos registrados com fontes
  conferidas, não 9.440 novas execuções do analyzer oficial naquele dia.
- A rodada expôs `invalid_assignment` suprimido por padrão; a publicação desse
  erro no analyze/LSP foi corrigida depois. Também entraram correções de patterns,
  alvos de rename/navigation, contextos condicionais e escritas em locais final.
- Quatorze casos novos de projeto tiveram 13 resultados exatos no modo padrão
  anterior à correção, 14 com fallback completo. File Explorer válido de 43
  arquivos teve saída diagnóstica exata. Não transformar isso em paridade total.
- LSP comparou **17 respostas normalizadas exatas** em quatro arquivos. Em três
  sessões pareadas, referências custaram **48,78 ms** contra **10,84 ms** no
  servidor oficial; definition/rename/formatting foram mais rápidas nesse recorte.
  Não há certificação de grandes projetos/sessões longas.
- Format comparou **10.474 arquivos aceitos por ambos**, sem diferença UTF-8;
  duas entradas foram aceitas apenas pelo DartForge. Rejeições são contadas
  separadamente. Modificadores/seletores de construtor passaram a respeitar a
  versão de linguagem.
- Nas medições de processos novos com cache de disco não controlado, analyze
  do File Explorer válido teve mediana **0,465 s vs 0,682 s**; format dos 426
  arquivos do SDK, **0,417 s vs 1,552 s**. Não são startup de disco frio nem
  throughput de servidor persistente.

Evidências: `bench/results/2026-10-14-current-tools-status/` e regressões de
análise/LSP/format registradas nos commits de 14/10.

### Pub, runner de testes, distribuição e ambiente

- Agora existe **`dartforge pub get` nativo**, sem subprocesso Dart/Pub, para o
  subconjunto implementado: fontes path/hosted, busca com backtracking,
  preferências de lock, cache/offline, overrides da raiz e sidecar de overrides.
  Pacotes `sdk: dart` vêm do manifesto do SDK selecionado. Fontes/opções fora
  do subconjunto são recusadas; não é equivalência integral ao Pub.
- Agora existe **`dartforge test`**, compilando suites comuns de `package:test`
  para executáveis nativos e usando seu Engine. Descoberta, filtros, JSON,
  falhas reais/async e política de saída foram validados em 11 casos integrados
  e 20 execuções VM/nativo. Execução é serial; browser, coverage, watch,
  `dart_test.yaml` e anotações de biblioteca ainda são recusados. Não é runner
  próprio completo nem substituição universal do `dart test`.
- `pack` inclui `sdk_packages.yaml` e árvores vendorizadas declaradas, conserva
  metadados e recusa caminhos/symlinks que escapem do SDK. Pub offline foi
  executado contra o SDK realmente relocado, cache isolado e `_macros` real.
- Cache do SDK exclui a entrada sintética por identidade da biblioteca, evitando
  caminhos temporários aleatórios após canonicalização; chave passou para v6.
  URIs de fixtures Windows usam `url::Url`, sem montar `file://D:\...` à mão.
  Manifesto revisado do SDK foi atualizado para a correção real de `dart:isolate`,
  mantendo rejeição de qualquer fonte não revisada.
- O fechamento corrigiu drivers C sem sysroot macOS, caminhos raw Windows,
  descoberta do SDK nos testes JIT, catálogo obsoleto de transmutes e provas ARC
  com nomes anteriores do ABI. Os checks comportamentais foram preservados.
- Toolchains Dart/Rust, caches, REA e builds permanecem no SSD; `~/tools` é o link
  de compatibilidade. A migração anterior liberou **6,26 GB** internos. Não é
  uma nova medição de espaço livre. Artefatos grandes/observações privadas de UI
  ficam fora do Git; evidências publicadas usam somente fixtures controlados.

### Correção adicional: raízes por mapas (B0)

A execução pesada agendada encontrou três recusas RS4GC nos testes dirigidos.
A reprodução local confirmou que declarações nativas fora da tabela fixa
perdiam o atributo LLVM `gc-leaf-function`, apesar dos efeitos certificados
usados pela análise de raízes. A emissão agora aplica a mesma regra às duas
fontes de declarações; funções desconhecidas/coletáveis continuam conservadoras.
A especialização RTI substituía uma operação por um helper com efeitos mais
conservadores: maps mantém a operação original, e os predecessores Phi seguem
a mesma decisão de emissão. O checker não foi alterado. **Sete testes focados
e os quatro `directed_maps` completos passaram localmente**, incluindo stress,
poison, sabotagens e comparação com shadow stack. A confirmação Windows remota
também passou na suíte pesada nativa. Evidência: `bench/results/2026-10-15-maps-native-effects/`.
Não é ganho de desempenho nem certificação de todos os perfis do coletor.

### Fechamento verificável

O código final **`c42f5995`** passou nas três validações:

- [CI completo — Linux, Windows e macOS](https://github.com/insinfo/dartforge/actions/runs/38701702025): **verde**, incluindo workspace Windows, emissor/JIT,
  reload, provas ARC e distribuição em ambiente limpo.
- [Sem toolchain do sistema](https://github.com/insinfo/dartforge/actions/runs/38701701967): **verde** em Windows sem MSVC/SDK e macOS sem Xcode/CLT.
- [Suíte pesada nativa](https://github.com/insinfo/dartforge/actions/runs/38701717466): **verde**, incluindo B0 dirigido/sabotagens, B1, ARC, GC stress,
  SDK compilado das fontes e AOT/JIT Unix. É a suíte `native`; os gates de JS,
  análise e versões modernas pertencem a outras seleções, sem declarar nova
  execução desses gates nesta rodada.

As falhas e cancelamentos intermediários permanecem nos registros. O orçamento
Windows passou a 180 minutos; builds Cargo macOS são serializados para evitar
links LLVM estáticos simultâneos. Todos os alvos, testes e contraprovas foram
preservados. A correção B0 não modifica o checker RS4GC.

A release desta rodada usa
[`v0.1.0-alpha.2`](https://github.com/insinfo/dartforge/releases/tag/v0.1.0-alpha.2),
com distribuições Windows x64, Linux x64 e macOS arm64. O
[workflow Release](https://github.com/insinfo/dartforge/actions/workflows/release.yml)
cria draft, compila cada pacote, verifica AOT de desenvolvimento/produção e
resolução Pub offline no SDK relocado, e publica após sucesso das três plataformas.
O estado público e os arquivos ficam no link da release. A atualização final
deste relatório e dos registros de CI não muda as fontes do compilador validadas
em `c42f5995`.

O trabalho geral de desempenho/tamanho e de compatibilidade Dart continua
aberto; esta versão é experimental e não transforma as limitações acima em
paridade com o SDK oficial.


## Continuação: callbacks fechados, SSA e janela AppKit (2026-10-13)

P5 implementado para **código AOT residente no processo**: fechar o
NativeCallable publica um tombstone antes de soltar o root da closure.
Trampolim e contexto nativo publicados nunca são reutilizados; chamadas tardias
retornam exceptionalReturn escalar, descartam void/listener ou zeram retorno
composto, antes de decodificar argumentos ou entrar no isolado. Frame já ativo
protege sua captura até retornar. Teardown do worker aposenta callbacks antes
de notificar onExit; no principal, após finalizers e formatação da exceção.

**18 casos nativos passaram**, em tracing/ARC/ARC preparado, normal e GC stress:
retorno inteiro de thread estrangeira, tipos escalares, fechamento na própria
chamada, 256 endereços distintos, callback do worker após onExit e chamada C de
atexit. Erro toString ainda chama callback vivo (15); após finalização, atexit
recebe exceptionalReturn (69), com saídas 0/255 conferidas. Builds usam símbolos
removidos por padrão. Outras **seis execuções nativas passaram** para structs
de 8 bytes (direto) e 24 bytes (retorno indireto), zerados após close.
VM/AOT oficial executam somente a alternativa segura que não chama após close.
Registros: `late-callback-validation.json` e
`late-composite-callback-validation.json` no diretório de evidências do pacote.

**Limites:** a memória nativa desses contextos/slots cresce com a criação de
callbacks e fica até o fim do processo. Unloading/reload JIT não tem garantia
de retenção do código de entrada. Callback local **vivo** vindo de outra thread
continua fatal; P4 e a entrada preparada de áudio continuam pendentes.

NoEscape HIR v2 agora exige validação tipada CFG/SSA em todos os corpos
admitidos: dominância, ordem de definição/uso, predecessor/arestas Phi, tipos e
ABI das chamadas estáticas. IR malformado e operações fora do subconjunto são
recusados. Permanecem quatro obrigações: mapping do parâmetro fonte para ABI,
proveniência de aliases escalares nativos, helpers/cleanup gerados e nova prova
após transformações. **Não certifica NoEscape AOT nem remove RC/GC.**
Normalização de literais null/boolean exatos permite eliminar dispatch morto
em predicados nullable preparados, preservando os produtores com efeito dos
operandos booleanos eager. Seis variantes nativas produzem os mesmos
87 bytes que VM/AOT oficial; os três IRs preservam comparação real do parâmetro
com null e eliminam dispatch de igualdade nesses dois predicados. Registro
`nullable-equality-validation.json`. Não é medida de ganho de desempenho.
O verificador inicial P5 usava ARC_PREPARE, variável ignorada pelo compilador;
foi corrigido para ARC_PREPARED e os casos foram repetidos. Registros iniciais
ficam explicitamente superseded, sem comprovar o perfil preparado.

Uma sonda adicional cria, mostra, redimensiona e fecha NSWindow real pelo
backend embedded do dart_ui, conferindo registry, geração e eventos de resize/
close, com progresso de timer/mensagem. Seis variantes passaram no registro
histórico `embedded-window-validation.json` (CLI ab3c245e…); isso amplia a sonda
anterior sem janela, mas não comprova rendering, teclado/mouse ou menus/modal.
A sonda NSWindow foi repetida na CLI nova (6f963ffc…): seis variantes
passaram com trace de 285 bytes e stderr vazio, em
`embedded-window-current-validation.json`. Registros históricos não foram
reatribuídos à CLI nova. Gate atual passou: **544 testes de biblioteca do emissor
(incluindo ignorados), 119 testes dos exemplos da documentação Rust do emissor e 362 do runtime**
(118 casos positivos e um caso de rejeição esperada; runtime: 253 lib,
24 integração, 85 docs), com SDK 3.6.2/LLVM 22 explícitos. Rustfmt dos novos
arquivos SSA/normalização e diff check passaram. Logs/hashes estão em
`p5-ssa-final-gates.json`; resta o aviso preexistente unused_mut em roots.rs.
Não é aprovação global do workspace. O objetivo geral P1–P17 permanece
incompleto.

## Continuação: recursos, bibliotecas e memória mapeada (2026-10-13)

O pacote único exporta `using`, `usingAsync`, `ScopedResource`,
`ResourceHandle`, `NativeLibrary`, `MappedMemory` e `MappedBytes`.
`using` usa `try/finally` com corpo síncrono; `usingAsync` aguarda antes da
liberação. Erro de liberação prevalece sobre erro do corpo, conforme Dart.
O corpo não pode reter o recurso; não há prova de escape nem semântica ARC.
`ResourceHandle` tenta liberar uma vez, inclusive quando o callback falha.

P16: `NativeLibrary.openFirst` preserva ordem, caminho escolhido e os erros
de todas as tentativas quando nenhuma funciona. `hasSymbol`, lookup e acesso
ao handle emprestado usam o FFI existente. Close é idempotente; consultas
posteriores são recusadas. Ponteiros/funções já obtidos dependem da biblioteca
aberta. Não implementa ligação estática automática de `@Native`.

P17: mapeamento compartilhado de arquivo existente, intervalos conferidos,
offsets alinhados, intervalos vazios, escrita e flush explícito. Implementação
FFI para macOS/Linux/Windows de 64 bits; **execução validada somente no macOS**.
`MappedBytes` confere acesso e owner a cada operação, inclusive após close.
`withBytes` empresta uma Uint8List externa sem cópia; readonly também impede
escrita pela view do buffer. O empréstimo deve ser síncrono e não escapar;
close é recusado durante o callback. Views brutas escapadas não são revogáveis.
É necessário impedir truncamento externo e sincronizar escritores concorrentes.
Os donos implementam `ScopedResource`; não há finalização automática.

**18 execuções nativas passaram**, iguais à VM/AOT oficial, nos seis modos
de cada sonda: bibliotecas, escopos e mapeamento. Análise do pacote sem problemas.
`scripts/verify-package-resources.py` confere referências, artefatos/manifestos
distintos e estabilidade das fontes. Registro em
`bench/results/2026-10-13-dartforge-package/resources-and-mapping-validation.json`.
A primeira rodada foi recusada por alteração de fonte durante os testes;
a rodada final foi repetida após congelar a correção de captura de erro do SO.
Depois, seis execuções adicionais de mapeamento conferiram explicitamente
ENOENT/ERROR_FILE_NOT_FOUND (código 2), iguais à VM/AOT oficial, em
`mapped-memory-os-errors-validation.json`.
Outras seis execuções conferiram acesso a partir de isolado estrangeiro:
transferência recusada ou acesso recusado, preservando o mapping do dono,
iguais à política verificada na VM/AOT oficial. Registro separado em
`mapped-memory-owner-validation.json`.
Não são medidas de desempenho nem validação do Windows/Linux.

Há uma análise interna inicial de não publicação sobre HIR, separada da
certificação de `NoEscape`. Resultados exigem auditoria de ABI, helpers gerados
e cleanup; a gate das anotações continua recusando obrigações não provadas.
Revisão independente identificou reconstrução de handles por leituras
escalares de campos/globais: esses acessos são recusados sem witness de layout.
Memória local sem prova de inicialização definida também é recusada: leitura
escalar não inicializada pode expor bits antigos de referências. **16 testes
do recorte HIR passaram**, incluindo essas regressões e hashes sem mapas de
debug não ordenados, preservando bits de NaN. Obrigações de ABI/SSA/helpers
ficam explícitas nos resultados; não permitem reduzir RC/GC ou certificar AOT.
O gate ampliado do emissor passou em **518 testes de biblioteca**, incluindo
os ignorados, com LLVM 22 e SDK 3.6.2 explícitos. A primeira execução escolheu
o Clang do sistema, sem `ld64.lld`/`lld-link`: suas 15 falhas foram todas de
lançamento dos linkers, sem falha de asserção de comportamento. A repetição
com o toolchain correto passou. As pendências gerais do workspace continuam
registradas nas seções anteriores; este recorte não é aprovação global.
Também passaram 118 doctests do emissor; a documentação Rust do crate foi
gerada (resta o aviso `unused_mut` preexistente em `llvm/roots.rs`). Logs e
hashes do checker estão indexados em `resources-final-gates.json`.
Afinidade, prova transitiva completa, ponte de áudio e demais P1–P17 seguem
no escopo da continuação; o objetivo geral permanece incompleto.

## Continuação: dispatcher AppKit e normalização dos contratos (2026-10-13)

Implementado `EventLoop.supportsNativeEventDispatcher`, registro e retirada
de `void Function()` para trabalho nativo limitado. O callback roda no turno
externo Dart, após o CFRunLoop retornar, microtasks e controle do isolado;
nunca por dreno Dart dentro de um frame estrangeiro. Root persistente e frame
ativo protegem substituição/remoção/desligamento durante o próprio callback.
Disable/teardown removem o registro; reentrância/modal P3 continua ausente.

No dart_ui, duas alterações na `main`: hook opcional de início no
`MacosEmbeddedWindowingBackend` (padrão do SDK modificado preservado) e cleanup
de inicialização parcial; descrição do status ativo aceita um adapter
compatível. O probe real usa `finishLaunching` e dispatcher de NSEvents,
sem símbolos `Dart_*` falsos. **Seis variantes nativas passaram**: falha
injetada preserva erro e limpa delegate/sink; backend novo recebe lançamento,
entrega evento sintético, avança timer/mensagem e encerra. Não cria janela;
rendering, input, menus/resize/modal não estão cobertos por essa evidência.

Implementada normalização A0 das quatro anotações: identidade definidora,
esquema, const aliases globais/estáticos/locais, prefixos/reexports, spans,
alvos e corpo disponível. Homônimos/shadowing/ambiguidade não ganham autoridade.
Preferências e obrigações têm tipos distintos, sem gerar fatos de segurança.
`--dartforge-contracts=verify|ignore` é opção explícita e imutável de AOT/IR.
Padrão `verify`: pedidos NoEscape/Realtime conhecidos são recusados com posição,
pois **seus checkers completos de emissão ainda não existem**. Preferências são
`not_applied`; `ignore` registra pedidos/diagnósticos sem certificá-los.
Erros de linguagem Dart continuam verificados. Manifesto `.contracts.json`
acompanha cada artefato com hashes/origem/política e `certified: false`;
mudanças entram no cache, preservando emissão relocatável. O nome inclui
o nome completo do artefato para não colidir entre `.arc`/`.tracing`.

**Resposta sobre Isolate.onEvent/handleEvent:** não implementados no DartForge,
cuja biblioteca/overlay usa SDK 3.6.2. A bomba nativa administra espera e filas;
não oferece contagem de notificações por evento nem processamento externo de
um evento. Essas APIs ajudariam P2, mas exigem ownership de agendamento,
notificação de timers prontos, protocolo de callback fora do isolado e turno
único; não resolvem automaticamente P3 ou áudio em thread estrangeira.
Na [patch 3.13.3](https://github.com/dart-lang/sdk/blob/3.13.3/sdk/lib/_internal/vm/lib/isolate_patch.dart#L738)
lançam UnsupportedError. A superfície experimental foi retirada do main,
conforme [SDK #64285](https://github.com/dart-lang/sdk/issues/64285).
`@Since` marca a declaração, não a implementação.

Validação funcional e registros em
`bench/results/2026-10-13-dartforge-package/`; scripts duráveis do pacote
verificam política de contratos e `scripts/verify-native-pump.py` verifica
wake, ciclo de vida, dispatcher e saída de erro. A ponte preparada de áudio,
checkers NoEscape/Realtime, suporte Windows/Linux e loops modais continuam
pendentes. Não inferir ganho de desempenho desses testes.

Gates deste recorte: **502 testes de biblioteca do emissor, 24 de integração
e 117 doctests; 358 do runtime; 118 de tipos; 5 de CLI**. Todos passaram,
incluindo os ignorados nos escopos executados. Os testes de integração do
emissor requerem `DARTFORGE_TEST_SDK_LIB` apontando para o SDK 3.6.2; sem essa
configuração a primeira execução falhou por ausência do SDK, e foi repetida
com a configuração correta. Análise de `packages/dartforge` sem problemas.
Não é aprovação do workspace inteiro: pendências gerais de fmt/Clippy/JIT
registradas abaixo continuam. Registros finais separados preservam os hashes
históricos: `contracts-final-validation.json`,
`pump-dispatcher-final-validation.json` e `stress-current-validation.json`.
O runner `scripts/verify-embedded-integration.py` repetiu os seis modos
AppKit com o mesmo compilador final; registro separado em
`embedded-integration-final-validation.json`. `final-gates.json` indexa
os logs compactados dos gates deste recorte.

## Continuação: pacote único, bomba macOS e entrada de áudio pendente (2026-10-13)

Três subagentes trabalharam na `main`, sem branches, worktrees ou commits.
`packages/dartforge` reúne SIMD, metadata e capacidades do runtime.
**`packages/dartforge_simd` foi removido por pedido do proprietário**;
consumidores locais migraram para `package:dartforge/simd.dart` ou
`package:dartforge/dartforge.dart`. A implementação SIMD foi movida sem
alteração; o LLVM continua emitindo operações em `<4 x i32>`.

Disponíveis `@PreferRegion()`, `@PreferAcyclic()`, `@NoEscape()` e `@Realtime()`,
também por `package:dartforge/annotations.dart`, com esquema 1 e mesma identidade.
`meta` dá feedback dos alvos expressáveis; não tem alvo de variável local,
portanto `PreferAcyclic` mantém seus alvos documentados. **São metadata portátil:
o compilador ainda não aplica essas preferências nem certifica esses contratos.**
A especificação usa a declaração em `package:dartforge/src/annotations.dart`;
o modo de verificação continua proposta.

Implementado adaptador macOS opt-in: `EventLoop.supportsNativePump`,
`enableNativePump()`, `disableNativePump()` e pedido de encerramento por microtask.
Integra CFRunLoop/fila principal à espera do runtime na thread principal;
mensagens, timers e microtasks acordam uma fonte persistente. A espera libera
locks/empréstimos antes de entrar no SO; instalação e notificadores são
sincronizados, com última liberação pelo dono após o retorno do callback CF.
Habilitar mantém o processo vivo; operações são idempotentes, com encerramento
explícito. VM/outros sistemas não oferecem a capacidade. A aplicação continua
responsável pelos NSEvents; adaptação a `appkitEmbedded`, reentrância P3,
callback síncrono entre threads P4 e fechamento tardio seguro P5 estão pendentes.

**Áudio:** falta a ponte AOT preparada do §36.4 de
`docs/ARC-CICLOS-ESPECIFICACAO.md`. Listener entrega eventos assíncronos;
isolateLocal exige a thread proprietária. Esperar o isolado responder
introduz bloqueio proibido por `dsp-v1`. Próximo passo: kernel C ABI sobre
buffers/contexto preparados, verificação transitiva pós-lowering, domínio
explícito e protocolo de admissão/quiescência/descarte fora do callback.
`Realtime` não habilita entrada estrangeira nem certifica o wrapper.

Corpus ampliado com `ffi_probe.dart`: alocador/arena reais do dart_ui,
alinhamento, views/overlap, UTF-8/UTF-16, mutex, structs e callbacks com erro
e encerramento após entregas. FFI/vídeo/strips deram **18 execuções** nos
seis modos tracing/ARC/ARC preparado, normal/stress, idênticas à VM/AOT oficial
(550/9.067/33.201 bytes). A bomba passou em **12 execuções** de fila principal,
wake de timer/microtask, mensagem de worker, rejeição fora da thread principal
e desligamento em callback sem timer/port Dart mantendo o processo vivo.
Metadata teve seis execuções iguais; SIMD preservou resultado e IR vetorial.
Não são medidas de ganho de desempenho. Vídeo agora tem benchmark separado,
com conversão e checksum discriminados e destino reutilizado.

O probe de metadata expôs contrato de ownership ausente para `Timer.tick`
no SDK preparado. Adicionados contratos escalares explícitos para ele e para
as três entradas da bomba, com efeitos conservadores e regressão que rejeita
retorno `Ref` incorreto; verificador preservado. Gates finais:
**496 testes do emissor e 353 do runtime** (244 unitários, 24 integração,
85 doctests), incluindo ignorados por padrão, passaram. Análise Dart limpa;
três alvos inválidos receberam diagnóstico. `cargo fmt --check` continua
falhando em arquivos do workspace; não declarar gates globais aprovados.

Evidências em `bench/results/2026-10-13-dartforge-package/`; logs/binários em
`target/`, todos no SSD. Pacote local, sem publicação; nenhuma alteração no
repositório dart_ui nem comandos mobile nesta entrega.

## Continuação: corpus real do dart_ui e timer periódico (2026-10-13)

Incorporadas as correções do proprietário em
`RELATORIO-DARTFORGE-PARA-DART-UI.md` §§7–8: corpus diferencial primeiro,
reaproveitamento de `appkitEmbedded`, capacidades específicas e contratos
explícitos; mobile continua proposta e exige separar host/alvo. A entrega
anterior foi renumerada para §9, preservando essas seções.

Três subagentes avançaram em frentes independentes. Criados
`bench/dart_ui/video_probe.dart` e `strip_probe.dart`, importando o código
atual da `main` do framework, sem vendoring nem mudanças no dart_ui.
`python3 scripts/verify-dart-ui.py --dart-ui-root <checkout> --output <SSD>`
compara VM JIT, AOT oficial e tracing/ARC/ARC preparado, normal e GC stress;
gera package config próprio e registra hashes das fontes, compilador e flags.
Não executa `pub get` nem modifica a configuração do framework.

**Vídeo:** NV12/I420/YUY2 em seis pares espaço/faixa, strides/offsets,
matriz com tolerância de um nível, BGRA, opacidade, crop, destino reutilizado
e RGBA/BGRA empacotados. Saída completa de **9.067 bytes** idêntica.
**Strips:** 11 cenas com curvas, clipping fracionário, transformação afim,
sobreposição, winding/holes e geometria maior que o viewport; pixels e todos
os registros/alphas ativos. Saída de **33.201 bytes** idêntica. Ao todo,
**12 execuções nativas** iguais às duas referências. O modo separado
`--benchmark` consome os registros e bytes em checksum; o smoke teve
382 quads, checksum 948835220 e zero crescimento nos quatro AOTs. Não foi
uma medição de ganho de desempenho.

Corrigido o catch-up observado no probe `qsort`: o runtime aplica a regra do
SDK 3.6.2 **antes do callback**, pula períodos somente quando atraso > período,
e mantém a fase ao reagendar. `Timer.tick` inclui os períodos pulados; período
zero continua incrementando uma vez por callback. Getter nativo registrado
no catálogo e na tabela de efeitos, com classificação conservadora.
As regressões `142_periodic_timer_native_callback.dart` e
`143_periodic_timer_delayed_callback.dart` conferem bloqueio nativo, callback
lento, microtasks, cancelamento e período zero, sem igualdade de tempo de
parede: **12 execuções AOT** iguais à VM e ao AOT oficial nos mesmos seis modos.

O probe portátil real passou novamente: zero ticks dentro do callback por
~1.500 ms e **quatro após retornar**, contra 34 antes da correção; nessa rodada,
o comportamento de entrega pós-bloqueio ficou alinhado à referência. Isto
não implementa progresso dentro de callback nem loops modais/reentrância.
HTTP também respondeu **1.100 JSONs corretos** (100 + 1.000 de aquecimento)
com GC stress, como teste funcional, sem usar seu tempo como benchmark.

Gates das partes alteradas, incluindo os testes ignorados por padrão:
**494 testes do emissor** e **349 do runtime** (240 unitários, 24 integração,
85 doctests), sem falhas ou ignorados. As limitações gerais de
fmt/Clippy/JIT do workspace permanecem; não declarar gate global aprovado.
Dados em `bench/results/2026-10-13-dart-ui-corpus/validation.json`;
logs/binários locais em `target/dart-ui-evidence/`, no SSD. Temporários
próprios removidos após arquivar os logs.

Próximo marco conforme a revisão: ampliar recortes FFI/assincronismo e medir
kernels com cargas controladas; depois adaptar o runtime ao `appkitEmbedded`
com wake sem perda e ciclo de vida definido. Sem nova API `runOneTurn`,
callbacks síncronos entre threads ou comandos mobile nesta entrega.

## Continuação com subagentes: base nativa para dart_ui (2026-10-13)

Três subagentes trabalharam em arquivos separados, na `main`, sem branches,
worktrees ou commits. Implementada a base da fase 1 de
`RELATORIO-DARTFORGE-PARA-DART-UI.md`:

- **Detecção AOT:** `const bool.fromEnvironment('dartforge.compiler')` é
  `true`; `String.fromEnvironment` dá `"true"` e `bool.hasEnvironment` dá
  `true`. A consulta `int` usa o default, pois `"true"` não é inteiro.
  Constantes locais/importadas e identidade em coleções são preservadas.
  Não foi adicionada uma opção geral `-D`; nomes const cujo cálculo ainda
  não suportamos dão diagnóstico, em vez de assumir silenciosamente ausência.
  O analisador continua mantendo consultas desconhecidas; outros backends
  não foram validados nesta rodada.
- **Inline:** `vm:prefer-inline` usa preferência limitada na HIR e
  `inlinehint` no LLVM; `vm:never-inline` impede cópia HIR e emite `noinline`.
  Cobre funções, métodos/getters, construtores, funções locais e corpos
  async; preserva metadata no ARC preparado. Recursão, restrições estruturais
  e de depuração continuam prevalecendo sobre a preferência.
- **Windows GUI:** `aot`/`compile-native` aceitam `--subsystem=gui|console`.
  A ligação escolhe o subsistema PE e o CRT correspondente, com
  `_set_app_type(2)` para GUI e entrada `mainCRTStartup`. Padrão console;
  GUI rejeitado em outros hosts. Não há patch do executável após a ligação.

Validação integrada: **487 testes do emissor passaram**, sete ignorados;
**três testes de CLI passaram**. O teste opcional de ligação cruzada também
passou: headers PE GUI=2 / console=3 e entradas válidas, além da ligação DLL.
Sem Windows/Wine nesta máquina, não houve execução Windows. As regressões
novas `140_compiler_environment.dart` e `141_inline_pragmas.dart`, JSON `138`,
Socket `139` e a composição de pixels real do dart_ui deram **30 execuções
AOT iguais ao Dart**, em tracing/ARC/ARC preparado, com e sem GC stress,
efeitos e RC conferidos. A `140` também passa na VM com a definição explícita
`-Ddartforge.compiler=true`; sem ela, valida o fallback oficial.

Sondas reais do dart_ui, sem modificar seu repositório: transferência de frames
compilou/executou os quatro caminhos e confirmou visibilidade em **200 rodadas**;
o probe portátil de callback `qsort` compilou/executou e reproduziu o bloqueio
nos dois mundos: **zero ticks durante ~1.505 ms**. Após retornar, o DartForge
entregou 34 ticks contra quatro da VM; registrar/investigar catch-up do timer
periódico antes de comparar responsividade. O teste de frames rodou enquanto
havia builds, portanto os seus tempos não são uma comparação de desempenho.

Evidências em `bench/results/2026-10-13-dart-ui-phase1/validation.json` e logs
comprimidos em `target/dart-ui-evidence/phase1/`, no SSD. Não há ganho de tempo
atribuído ao inline nesta entrega. Os números históricos do comparativo não
foram substituídos por estes testes funcionais. Fmt/Clippy/JIT seguem com as
limitações gerais já registradas.

**Após o corpus diferencial da revisão §7: P1/P2**, bomba macOS opt-in com fonte persistente de CFRunLoop
e wake integrado ao `IsolateQueue`, sem perder notificações entre destravar
e esperar, com bombeamento também entre lotes de trabalho Dart. Tratar pausa,
encerramento e temporização. Isso ainda não distribui `NSEvent` do AppKit.
P3 fica separado: `dispatch_next` hoje usa um único `CURRENT`/`MESSAGE_ROOT`;
é necessário empilhar estado e raízes antes de oferecer um turno reentrante.
Detecção arbitrária de dependências Future/deadlocks e orçamento capaz de
interromper callbacks longos continuam propostas, sem implementação.

## Continuação: validação de constantes antes do AOT (2026-10-13)

Corrigida a aceitação de `const JsonUtf8Encoder()` pelo `compile-native`.
O emissor agora executa o verificador de constantes compartilhado com o
analisador antes do lowering, em todas as bibliotecas de usuário, inclusive
código não alcançável. A inferência registra vínculos de declarações locais
para preservar referências entre constantes. As declarações públicas dos
construtores do SDK são consultadas sem verificar os corpos internos do SDK.

Validação: **480 testes do emissor passaram**, sete ignorados; os dois testes
novos também passaram após ampliar os casos para imports, typedefs, records
e construtores encaminhados por mixin. Dez exemplos inválidos foram rejeitados
pelo DartForge e pelo Dart oficial sem emitir IR. Os **128 arquivos da raiz de
`corpus/native/` emitiram IR**; isso não equivale à execução de todo o corpus.
As sondas `53`, `120` e `131` deram **18 execuções idênticas ao Dart**, em
tracing, ARC e ARC preparado, com e sem GC stress e efeitos/RC conferidos.

Os servidores original e UTF-8 recompilaram e validaram 110 respostas JSON
cada, contando o aquecimento. Compilações aquecidas: **0,369 s / 0,448 s**;
primeiras compilações: 16,689 s / 2,638 s, com cache em estados diferentes.
Não houve nova medição de desempenho de execução. Evidências em
`bench/results/2026-10-13-http-json-aot/const-validation.json` e detalhes em
`RELATORIO-HTTP-JSON-AOT.md` §10. Logs comprimidos no SSD; temporários removidos.
As pendências gerais de fmt/Clippy/JIT permanecem.

Próximo foco de desempenho: medir alocações temporárias de objetos/mapas no
encode de JSON e o custo de Stream/Future antes de escolher a próxima mudança.

## Continuação: leitura direta e medições HTTP/JSON (2026-10-13)

`Socket_Read` agora lê no buffer Dart, seguindo o SDK 3.6.2. Leituras completas
dispensam o buffer Rust de payload e a cópia posterior. Leituras curtas copiam
para uma lista de tamanho exato, sem prender a capacidade originalmente
solicitada. O receiver e o buffer parcial ficam enraizados durante alocações.
Não muda a ABI; as demais rotas de I/O continuam como estavam.

Validação: runtime **345 testes passaram**; TCP/HTTP `04` e a nova regressão
`139_socket_read_buffers.dart` idênticos ao Dart em tracing, ARC e ARC
preparado, com e sem GC stress e efeitos/RC conferidos. A `139` também passou
com coleta agendada (`42,3`) nos três modos. O teste unitário cobre leitura
completa, curta, vazia, EOF, erro e tamanho físico exato sob GC stress.

Medidas com uma conexão keep-alive e cliente Python local, cinco rodadas
alternadas (CPU do processo por mil requisições; não capacidade máxima):

| carga | antes | depois | resultado |
| --- | ---: | ---: | --- |
| GET texto, 50 mil por rodada | 35,0 ms | 34,4 ms | diferença pequena, faixas sobrepostas |
| GET JSON pequeno, 50 mil | 37,2 ms | 37,2 ms | sem ganho de CPU demonstrado |
| POST 64 KiB, resposta texto, 20 mil | 54,5 ms | 52,0 ms | **4,6% menos CPU** |
| POST 64 KiB, resposta JSON pequena, 20 mil | 59,0 ms | 57,0 ms | **3,4% menos CPU** |

A dica de JSON direto em UTF-8 também foi medida: exemplo separado
`bench/comparison/server_utf8.dart`, encoder reutilizado, saída byte a byte
igual. Contra `server.dart`, na mesma versão do runtime: **38,0 → 36,0 ms**
por mil GETs JSON (**5,3% menos CPU**). O exemplo compila no Dart oficial e
passou com GC stress no DartForge. Não são percentuais somáveis nem medidas
do desafio SQLite/Linux. Detalhes e comandos em `RELATORIO-HTTP-JSON-AOT.md` §9.

O medidor novo `scripts/measure-http-pairs.py` teve um erro inicial ao
reutilizar a resposta como corpo do próximo POST. As rodadas afetadas foram
descartadas e repetidas após a correção. Também foi corrigida a recomendação
de `const JsonUtf8Encoder()`: o construtor não é const. O DartForge aceitou
essa chamada inválida, embora o Dart oficial a rejeite; o diagnóstico no
`compile-native` foi corrigido na continuação acima. O exemplo válido e as
medidas finais não dependem dessa sintaxe inválida.

A aceitação do construtor const inválido foi corrigida na continuação acima.
Objetos/mapas temporários do encode e custo de Stream/Future seguem como foco.

## Continuação: JSON com tabelas em produção no macOS arm64 (2026-10-13)

Corrigido o aborto de `jsonDecode` identificado na rodada HTTP/JSON. No
Mach-O produzido pela LTO do `ld64.lld` 22.1.8, `JsonCodec.decode` tinha
encoding zero no unwind e perdia seu FDE; a personalidade não era chamada.
O mesmo objeto nativo da LTO tinha o FDE e, religado, capturava a exceção.
O caminho macOS arm64 de LTO com tabelas/mapas agora retém os objetos
otimizados e faz a ligação final nativa, preservando o unwind. A otimização
entre módulos continua; o custo é uma ligação adicional. Temporários ao
lado da saída, removidos em sucesso e falha.

Regressão nova `corpus/native/138_json_error_unwind.dart`: JSON inválido em
String/UTF-8, encode não suportado, catch/finally e decode válido depois.
Validação: **478 testes do emissor passaram**; regressão tracing/ARC e sondas
`101`/`04` idênticas ao Dart em produção/tabelas, com e sem GC stress e efeitos
conferidos, em duas compilações. Debug passou, recompilação aquecida produziu
binário idêntico e erro injetado na ligação final não deixou temporários.
O servidor AOT corrigido respondeu 1.000 JSONs corretos em keep-alive.
ARC preparado também passou com e sem stress e com RC/efeitos conferidos.
As recompilações aquecidas das sondas levaram 0,3–0,4 s; ganho de throughput
ainda não medido. Detalhes em `RELATORIO-HTTP-JSON-AOT.md` §8. Mapas de raízes
não executados nesta rodada; continuam as pendências gerais de fmt/Clippy/JIT.

## HTTP/JSON AOT e toolchains no SSD (2026-10-13)

A transferência de Dart e Rust para `/Volumes/ssd_dock/tools` foi concluída e
testada pelo proprietário: Dart executou, Rust compilou e executou, e **6,26 GB**
saíram do disco interno de 256 GB. `~/tools` aponta para o SSD, preservando
referências antigas. `tools/env.zsh` define PATH, `CARGO_HOME`, `RUSTUP_HOME` e
`PUB_CACHE`; carregar esse arquivo explicitamente nos comandos não interativos
que não herdem as variáveis. Builds/caches ficam em `target/` no SSD e
temporários desta rodada em `target/tmp-http-json` via `TMPDIR`.

Analisados os quatro textos anexados, a transcrição do vídeo do servidor de
US$ 12 e as regras do repositório. Resultado em
`RELATORIO-HTTP-JSON-AOT.md`: priorizar menos cópias e objetos intermediários,
JSON direto em UTF-8, perfil de criação/iteração de mapas e do SDK HTTP;
statements preparados, JSON no SQLite e group commit são decisões da aplicação.
Os 64 mil usuários do vídeo não são req/s nem uma medida do DartForge.

**Implementado:** `Socket_WriteList` toma emprestada a fatia de bytes Dart
durante o envio, eliminando o `Vec` temporário e a cópia da lista inteira.
O empréstimo termina antes de construir um erro. Windows mantém a cópia
necessária à operação overlapped; Unix consome a fatia durante a chamada.
O teste com sockets reais cobre listas internas/externas, view somente leitura,
offsets, envios vazios e erro do SO. O ganho estrutural é uma alocação e uma
cópia Rust a menos por envio; throughput ainda não quantificado.

Validação: runtime **344/344**, sonda TCP/HTTP idêntica à VM em tracing/ARC
com e sem GC stress e com efeitos conferidos; servidor AOT respondeu 1.000
JSONs corretos em uma conexão keep-alive. Na primeira rodada, a sonda
JSON/mapas `101` passava com `check`, mas abortava com `tables`
(`_Unwind_RaiseException returned 5`), também no compilador anterior; a
continuação acima corrigiu esse caso. Documentação do workspace gerada com avisos;
fmt/Clippy estritos têm pendências fora desta alteração e o teste completo
do workspace foi bloqueado na ligação do JIT (bitcode LLVM 22 com ligador
Apple/LLVM 21). Evidências e limites no relatório desta rodada.


## Fechamento de 2026-10-13 (macOS arm64)

Dia de migração do código para o inglês e de rodadas paralelas em todas as
frentes, cada uma validada localmente (o CI não foi acompanhado, por decisão do
dono). As seções seguintes têm os detalhes de cada rodada.

### O que entrou hoje

| frente | onde estava | onde ficou |
| --- | --- | --- |
| Migração para o inglês | código em português | código Rust, CLI, variáveis, features, scripts, workflows, protocolos, ABI, pacotes Dart, `native_sdk`, `tools`, `bench` e as pastas do corpus em inglês (fases 1–4; `docs/GLOSSARIO-PT-EN.md`) |
| Analisador: placar | 22.965/23.012, FN 47, mensagem errada 10 | **23.001/23.012**, FN 11, mensagem errada 5, FP 0 |
| Analisador: pub-cache (209 pacotes) | FP 35, FN 40 | **FP 0, FN 6**; `verified.txt` 477 → 485 códigos |
| Analisador: CPU no `new_sali/backend` | ~14 s | **~5 s** (`dart analyze`: 21,7 s frio, 2,6 s com cache) |
| LSP: oráculo | references/implementation 96% | **100%**; definition 100%; completar top-5 100% |
| LSP: memória do servidor (`new_sali`) | 240–640 MiB | **27–31 MiB** (análise tipada num processo filho) |
| Nativo A0: `bench/performance` nosso/Dart (ciclos) | 0,963 | **~0,88** (duas rodadas; carga alta, ±5%) |
| JS produção: `limitless_ui` bruto / gzip | 4,10 / 1,16 MB | **3,54 / 1,02 MB** (`dart2js -O4`: 3,71 / 0,98) |
| JS produção: `new_sali` bruto / gzip | 8,42 / 2,28 MB | **7,60 / 2,07 MB** (`dart2js -O4`: 7,62 / 2,02) |
| JS produção: corpus bruto | 33,1 MB | **16,0 MB** (`dart2js -O4`: 9,8 MB) |
| JS: `limitless_ui` no Chrome | 2/47 rotas, e2e 1/15 | **47/47 rotas, e2e 15/15** nos dois perfis |
| Corpus diferencial JS | 238 | **241/241** nos dois perfis; `corpus/modern` 26/26 |
| JS de desenvolvimento contra o DDC | não medido no macOS | início a frio 20× mais rápido, ~10× menos memória, app pronto 3–4× antes (`docs/DEV-VS-DDC.md`) |

### O que falta, por frente (ordem de prioridade)

* **JS de produção** (`docs/JS-PRODUCAO-TAMANHO.md` §14): ganhar do `dart2js
  -O4` no gzip (1,037× no `limitless_ui`, 1,024× no `new_sali`) — tipo do
  tearoff na chamada do `bind` no lugar das assinaturas (~14 KB gzip), JSON
  das regras rti (8,5 contra 5,3 KB), tabela `t$N` (7,7 KB), ids implícitos nos
  `addRtiResources` (7,3 KB), nomes do topo sem valor (6,5 KB); o piso de 46 KB
  do `print('hi')` (`_Universe`/`_Parser` vivos pelo `str()` dinâmico).
* **JS de desenvolvimento** (`docs/DEV-VS-DDC.md`): a edição de corpo dispara o
  motor de build inteiro (2,8 s de 4,2 s no `new_sali`; os templates deviam
  depender só do hash de API); o outline do programa é refeito a cada edição;
  servidor sem cache HTTP (`no-store`, cada recarga baixa 51–93 MB); projeto
  recém-clonado ainda precisa de um `build_runner build` (Sass e saídas do
  ngdart); closures 1,65× mais lentas que no DDC.
* **Nativo** (`docs/PLANO-TAMANHO-DESEMPENHO.md` §6): árvores (~2× o Dart) e
  lista ligada (4×) limitadas pelo mutador e pelo cabeçalho de 16 bytes;
  medir `DARTFORGE_ROOTS_IN_CALLEE=1` com a máquina parada; `List.[]=`
  covariante por faixa de ids; mapa de `String` e interpolação; tamanho do
  `__text` (precisa do `new_sali/backend` nesta máquina). ARC: cabeçalho maior
  só no ARC (decisão do dono, não iniciado), poda do SDK no preparado, unwind
  das funções do SDK sob GC stress no modo de mapas.
* **Analisador** (`docs/ANALISADOR-PARIDADE-PLANO.md` §10–§11): recursos
  3.13/augmentations (~8 FN), exibição de `Null?`/`Never Function(Never)`/
  `Comparable<Object?>`, membros de extensão em referências de documentação
  (6 FN do pub-cache); carga sem os corpos das dependências e inferência dos
  corpos (~45% do perfil).
* **LSP**: incremental I5–I6 — o patch `wip/analysis-outline-incremental.patch`
  (aplica limpo sobre o HEAD de hoje; não validado) traz a leitura das
  dependências sem os corpos e a análise só dos arquivos afetados por uma
  edição, com o modo `analyze_project --files N` para conferir; pico do
  processo filho (639 MiB no `new_sali/frontend`: nós de expressão e escopos).
* **Migração, resto da fase 4**: nomes de programas e subpastas dentro do
  corpus (`corpus/js/*_interpolacao`, grupos `linguagem`/`pacotes`/`sonda`,
  `oraculos/`, `esperado/`, `perfil_*`; exigem regravar oráculos), classes
  internas do runtime (`_Contexto`, `_Celula`, `_AcumuladorDeTexto`,
  `_ProgramaDeRegExp` em `runtime/src/layout.rs`), variáveis e mensagens
  internas dos scripts, fontes Go/TS/Swift do comparativo, fixtures em
  português nos testes Rust. Os comandos citados nas seções antigas deste
  arquivo usam os nomes de antes da migração (mapa no glossário).

## Comparação com o DDC oficial no macOS (2026-10-13)

`dartforge serve` contra `webdev serve` (Dart 3.6.2, webdev 3.7.1, Chrome 155
headless; máquina muito carregada, carga 25–220, então a CPU é a medida mais
confiável). Script `scripts/compare-dev-ddc.mjs`; relatório completo em
`docs/DEV-VS-DDC.md`.

| medida | DartForge | webdev (DDC) |
| --- | ---: | ---: |
| início a frio, `limitless_ui` / `new_sali` | **4,9 s / 19,4 s** | 103 s / 340 s |
| edição de corpo, `limitless_ui` (relógio / CPU) | 5,3 s / **1,44 s** | 5,3 s / 4,91 s |
| edição de API pública, `limitless_ui` | **10,6 s / 3,5 s** | 40,5 s / 40,9 s |
| edição de template, `limitless_ui` | **4,0 s / 1,34 s** | 7,0 s / 4,32 s |
| memória, `limitless_ui` (soma dos processos) | **476–587 MB**, estável, 1 processo | 2,9 → 7,1 GB, crescendo, 22 processos |
| memória, `new_sali` | **0,75–1,06 GB** | 4,9–7,0 GB |
| JS com gzip, `limitless_ui` / `new_sali` | **4,17 / 7,84 MB** | 5,24 / 9,94 MB |
| app pronto no Chrome, `limitless_ui` / `new_sali` | **666 / 1.063 ms** | 2.863 / 4.171 ms |
| `bench/performance` no Node (mediana) | ≈1,0× | 1,0× |

O JS bruto é ~10% maior no nosso, mas comprime 20% melhor. A recarga do webdev
não funciona no `new_sali` (404 em `/login`); a nossa funciona. Saída idêntica à
do DDC nos núcleos de benchmark; closures 1,65× e listas encadeadas 1,3× mais
lentas no nosso, `fib` 0,38× e `shapes` 0,50×.

## Analisador: velocidade do motor (2026-10-13, noite, macOS)

O `new_sali/backend` caiu de ~14 s para **~5 s** de CPU e o `openssl_bindings`
do pub-cache de 63 s para 5 s, com placar (23.001/23.012, FP 0), saída do
`analyze`, projetos reais e pub-cache idênticos (commit `0b7f6faa`). Saiu o
trabalho quadrático por biblioteca: tabelas dos corpos alocadas na primeira
escrita, índice de elementos por unidade, biblioteca e declaração
(`Program::index`), interfaces, variáveis inferidas e promoção de campos
divididas entre as passadas (`SharedInference`), `is_shadowed` por índice
montado uma vez por árvore, e leituras de `package_config`/`pubspec` uma vez.
*Footprint* 685 → 603 MiB. `DARTFORGE_ENGINE_TIMES=1` mostra o tempo por
fase; exemplo `analyze_project`. `dartforge-parity output` sem argumento volta
a achar `corpus/analyze-output`. Detalhes em
`docs/ANALISADOR-PARIDADE-PLANO.md` §11.

## Nativo A0: coletor (2026-10-13, tarde, macOS arm64)

Commits `0d74c459`, `21d27ab2`, `9f55b545`; `docs/PLANO-TAMANHO-DESEMPENHO.md`
§6 e `NATIVO-ESPACO-UNIFICADO.md` §2.8.1.
* **Envelhecimento no coletor geracional:** o objeto só é promovido na segunda
  coleta menor que sobrevive (estados `SURVIVOR`/`SURVIVOR_B` alternados; a
  barreira os conta como jovens); faixas livres reordenadas quando fragmentam;
  pausa adaptativa quando a estrutura fica viva; `DARTFORGE_GC_AGING=0`
  desliga. A coleta completa promove tudo e o ARC não envelhece.
* **Slots mortos da pilha-sombra zerados** antes de cada ponto de coleta
  (`roots::dead_slots`). Nas árvores, os objetos marcados caem de 4,54 para
  1,76 milhão e as coletas completas de 29 para 3.

Ciclos do processo (carga 9–190), `bench/performance` nosso/Dart contra a base
desta rodada: **0,928 → 0,886** (temporários 0,92 → 0,65; coleções 0,57 →
0,52; objetos que escapam 2,06 → 2,03). Tamanhos iguais.

## JS: classe e construtor numa chamada, rodada 5 (2026-10-13, fim da tarde)

`C = t$C(class …, function …)`: a definição da classe e o construtor padrão
numa chamada, com o nome da classe uma vez em vez de três
(`DARTFORGE_JSPROD_CLASS_CONSTRUCTOR=0` desliga). `limitless_ui` 3,537 MB (gz
1,021) contra 3,708 / 0,984 do `dart2js -O4` (0,954× bruto, 1,037× gzip);
`new_sali` 7,602 / 2,068 contra 7,616 / 2,019 (0,998× bruto — passou a ser
menor — e 1,024× gzip). Ablação do gzip que falta por construção no
`docs/JS-PRODUCAO-TAMANHO.md` §14.

## Nativo A0: velocidade (2026-10-13, macOS arm64)

Commits `d52fc252`, `61b2f129`, `98ac0751`, `143bface`, `1fef8307`,
`e0a61865`, `587e3de0` e `docs/PLANO-TAMANHO-DESEMPENHO.md` §5.
* Objeto recém-alocado segue sem barreira nem raiz nos inicializadores de
  campo com índice constante.
* Contexto da thread no macOS arm64 lido de uma chave `pthread`
  (`TPIDRRO_EL0` + `dartforge_context_slot`, pela GOT, como o Go), sem
  `tlv_get_addr`; as leituras dos chamados em linha se fundem.
* `is` com cache no ponto de uso (`dartforge_rti_e_cache`; desligado sob
  `DARTFORGE_EFFECTS=check` e com `DARTFORGE_NO_RTI_CACHE=1`); cache das
  receitas RTI lido em linha.
* A marcação do GC não procura anexos quando não há nenhum.

Medido por ciclos do processo (máquina com carga 17–50; tempo de parede não
confiável), `bench/performance` nosso/Dart: **0,963 → 0,877** (chamadas 0,77 →
0,68, coleções 0,50 → 0,46, JSON 1,18 → 1,01, objetos em coleções 1,42 →
1,04, objetos que escapam 2,40 → 2,10, textos 1,10 → 1,08). `bench/comparison`:
fib 11–15 → 7–10 ms (Dart 10–13), árvores 35–48 → 26–35 (Dart 14–22). Tamanhos
iguais ("olá" 3,22 MB; Dart 5,33). Validação: corpus nativo 125 + `33`
conhecido em desenvolvimento e produção, ambos com GC stress, efeitos
conferidos, ARC e preparado; `emit_native --lib` 478; runtime 226; JIT.
Resta: promoção precoce do coletor (árvores 2,1× o Dart; 10% das coletas são
completas), cabeçalho de 16 bytes (lista ligada 4,8×), `[]=` covariante por
faixa de ids, mapa de `String` e interpolação.

## Migração para o inglês: concluída no código Rust (2026-10-13)

Fases 1–3c (crates, pastas, identificadores, mensagens, ABI do runtime,
catálogos, opções da CLI, variáveis `DARTFORGE_*`, features do Cargo,
scripts, workflows, protocolos `dfexec`/recarga e formatos em disco), sem
aliases para os nomes antigos. O mapa antigo → novo está em
`docs/GLOSSARIO-PT-EN.md` (seções por fase; "Interface com usuário e CI
(fase 3c)" tem as 73 opções, as 127 variáveis e os 57 scripts). Exemplos:
features `native`/`native-generators`/`embedded-llvm`; `--memory`,
`--exceptions check|tables`, `--roots shadow|maps`, `--stack-trace
symbolic|none`, `--debug`, `dartforge pack`; `dartforge-parity scoreboard
--details --workers 8`; `scripts/native-corpus.sh`, `scripts/prove-arc-aot.py`;
workflows `heavy.yml`, `no-toolchain.yml`, `debug.yml`, `pub-scoreboard.yml`;
`crates/analysis/verified.txt`. Os comandos citados nas seções antigas deste
arquivo usam os nomes de antes. Falta a fase 4: APIs dos pacotes Dart
(`packages/`, `native_sdk/`), nomes do corpus e de `tools/`, README.

Validação da 3c num snapshot limpo: `cargo test --workspace` 2.109 (1 falha
conhecida, `produces_object_and_bitcode`); corpus nativo 125 + `33` em tracing,
prepared, arc e efeitos `check`; JS 241/241 nos dois perfis; differential
nativo 241/241 em A0, A1, B0, B1 e ARC; `dart:io` 130/130; JIT × AOT 241/241;
determinismo idêntico com 1, 4 e 8 workers; placar 22.997, FP 0; formatador
43.000/43.000 (corpus maior com o pub-cache baixado). Achou duas regressões do
JS de produção (`corpus/modern/330_atalhos_ponto` e o teste ignorado
`identity::no_filter_and_all_live_emit_the_same_text`), em correção.

## Analisador (rodada de 2026-10-13, tarde, macOS)

Placar 22.975 → **23.001/23.012** (FP 0, posição errada 0, mensagem errada
9 → 5, FN 37 → 11); pub-cache (209 pacotes) FN 29 → **6** com FP 0 e mensagem
errada 0; projetos reais FP 0. Parser: `void` e tipos sem nome nas cláusulas
(`expected_type_name`, `expected_named_type`), `this.` sem nome, recuperação de
`required`, `var_and_type` e `function_typed_parameter_var`. Inferência:
anotações de membro no escopo da classe, código morto após `${throw}` e após
`for(;;)` em elemento de coleção, captura em closure por declaração, variância
declarada, tipo de criação sem construtor (com alias), `[] =` ambíguo entre
extensões. `deprecated_member_use` cobre nomes de anotação, referências `[…]`
de documentação e inicializadores de campo. `methods_without_combined_signature`
deixou de ser quadrático no número de bibliotecas. Restam recursos
3.13/augmentations, valor padrão em tipo de função, exibição de
`Null?`/`Never Function(Never)`/`Comparable<Object?>` e membros de extensão
em referências de documentação (`docs/ANALISADOR-PARIDADE-PLANO.md` §10).
Desempenho no `new_sali/backend`: ~12 s de CPU contra 21,7 s do `dart analyze`
frio e 2,6 s com cache; próximo ponto quente `UnitBodyTypes::new` (quadrático).

## Analisador (rodada de 2026-10-13, macOS)

| medida | antes | depois |
| --- | ---: | ---: |
| Placar (posição exata) | 22.965/23.012 | **22.975/23.012** |
| FP / posição errada / mensagem errada / FN | 0 / 0 / 10 / 47 | **0 / 0 / 9 / 37** |
| Pub-cache (209 pacotes): FP / mensagem errada / FN | 35 / 2 / 40 | **0 / 0 / 29** |
| Projetos reais (10): FP | 0 | **0** |
| Mutações (567): FP | 7 | **4** (só lints não publicados) |
| Códigos em `verified.txt` | 477 | **485** |

Códigos novos publicados: `argument_type_not_assignable`,
`const_eval_throws_exception`, `deprecated_new_in_comment_reference`,
`override_on_non_overriding_member`, `return_of_invalid_type_from_closure`,
`unchecked_use_of_nullable_value`, `unused_import`, `variable_type_mismatch`.
Consertos: promoção de `T?` para `T & S`; `?.` cobre a invocação; mensagem do
`assert` no ramo falso; `if-case` dentro de `case` não troca o escrutínio;
`identical(x, null)` promove; fábrica `const` redirecionada instancia o alvo;
setter implícito de `late final`; `@internal`/`@literal` pelo `package:meta`;
UP de tipo de extensão que implementa `Object`; `C?` sem tear-off de `call`;
recuperações do parser (`var enum;`, `"$"`). Caixa do caminho no macOS
resolvida (o motor canoniza raiz, arquivos e `package_config`). `is_shadowed`
deixou de ser quadrático: `element.dart` do analyzer ~60 s → 1,5 s; pub-cache
inteiro ~50 min → 6 min. Formatador 39.762/39.762. Detalhes em
`docs/ANALISADOR-PARIDADE-PLANO.md` §9. Restam: variância (experimento),
exibição de `Never?`/alias, recursos 3.13 (construtores primários,
augmentations), casos isolados.

## LSP (rodada de 2026-10-13, macOS)

Oráculo contra o `dart language-server` 3.6.2: references 96 → **100%**,
implementation 96 → **100%**, definition 98 → **100%**, prepareCallHierarchy
99 → **100%**, hover/typeDefinition 100%, completar top-1/top-5 98/97 →
**99/100%**; nenhuma categoria caiu. As buscas seguem as relações do índice do
Dart (mixin por `typedef` não faz subtipo, class alias faz); referências de
documentação só quando resolvem para o elemento; `this.x` só para o campo;
membros de `Object` num receptor `dynamic`. No completar: alvo classe sem
membros de `Type`, `A.n(…)` só construtores, não importados também no nome de
uma declaração, par getter/setter explícito deixou de colidir. Os `Create
function/method` usam os nomes declarados dos parâmetros de tipo função (`p1`
só para os sem nome). A varredura das pastas do workspace saiu do laço
principal (pedido de 1,5 s: 16 s → 2 s), o que resolveu o teste flaky
`cancellation`. Medianas (ms, Dart/DF): references 2,6/2,1, implementation
1,0/1,6, completion 4,5/7,2.

**Memória com a pasta do workspace aberta (rodada seguinte, mesmo dia).** A
regressão do I7 (pico ~430–920 MiB e RSS em repouso de 99–640 MiB no
`new_sali`) era fragmentação do alocador do sistema, não retenção: em repouso
só 17–27 MiB estavam em uso, contra 97% de páginas fragmentadas no `vmmap`.
Cada análise tipada passou a rodar num processo filho do próprio binário
(`--typed-analysis`, JSON por stdin e stdout; `DARTFORGE_LSP_TYPED_IN_PROCESS=1`
volta à thread). O índice de faixas do motor (`crates/parity`) ficou compacto
e é montado só quando um diagnóstico precisa dele. Physical footprint no
`new_sali` (macOS arm64):

| | antes | depois | Dart 3.6.2 |
| --- | ---: | ---: | ---: |
| servidor: pico / platô | 602–640 / 240–477 MiB | **27–31 / 27–31 MiB** | — |
| filho da análise (só enquanto roda) | — | 639 MiB (`frontend`), 310 MiB (`core`) | — |
| pico total | 602–640 MiB | **664 MiB** | 737–1.402 MiB |
| em repouso | 240–477 MiB | **27 MiB** | 736–1.286 MiB |

O RSS não serve de régua no macOS (cai a poucos MiB sob pressão com o
footprint intacto). O teste `plateau_workspace` cobre a regressão. Oráculo
idêntico em todas as categorias; razões de latência DartForge/Dart iguais ou
menores. Saíram 22 avisos de código morto (o `SdkIndex` era montado e nunca
lido), a completion marca itens deprecated como o Dart, e o clippy do crate
caiu de 147 para 17 avisos. Para baixar o pico do filho: escopos materializados
(~45 MiB, 672 mil entradas) e tamanho dos nós de expressão (141 MiB), em
`crates/elements` e `crates/frontend`. Pendentes: I5–I6 (motor).

## JS: nomes compartilhados e o piso, rodada 4 (2026-10-13, tarde, macOS)

O renomeio de propriedades dá o mesmo nome curto a membros de classes que
nunca coabitam numa cadeia de supertipos, como os nomes por posição de campo do
`dart2js` (−44 KB gzip no `limitless_ui`, −70 KB no `new_sali`). Operadores
vivos por seletor (acabou o "operador sempre vivo"), remendos do SDK que tiram
`allMatches` e o rti de *records* do piso, *polyfill* do DOM só com
`dart:html`, classe sem parâmetros de tipo próprios sem `_ti`. A produção
omite os nomes dos campos `late` como o `dart2js -O2` (decisão do dono); o
diferencial compara a produção contra a referência com essa troca declarada e
nomeia o programa (o desenvolvimento segue idêntico à VM). Correções: constante
de enum por atalho de ponto (`modern/330`) e o teste `identity.rs`. Cada passo
tem um `DARTFORGE_JSPROD_*=0`.

| alvo (bruto / gzip) | antes | depois | `dart2js -O4` |
| --- | ---: | ---: | ---: |
| `print('hi')` | 52,0 KB | **46,3 KB** | 5,1 KB |
| corpus (240) | 17,06 / 5,66 MB | **15,99 / 5,33 MB** | 9,76 / 3,17 MB |
| mediana nosso/dart2js | 1,64× / 1,72× | **1,51× / 1,58×** | — |
| `limitless_ui` | 3,727 / 1,099 MB | **3,547 / 1,026 MB** | 3,708 / 0,984 MB |
| `new_sali` | 7,854 / 2,189 MB | **7,627 / 2,082 MB** | 7,616 / 2,019 MB |

Corpus diferencial 241/241 e `corpus/modern` 26/26 nos dois perfis;
`limitless_ui` e2e 15/15 e 47 rotas limpas nos dois perfis; `new_sali` carrega
sem erro. Medido e descartado: tirar `dart.fn` e assinaturas (teto de 5,2 KB
gzip no `limitless_ui`) — `docs/JS-PRODUCAO-TAMANHO.md` §13.

## JS: o piso do runtime, rodada 3 (2026-10-13, macOS)

`print('hi')` saía com 84,7 KB contra 5,1 KB do `dart2js -O4`: três chamadas
dinâmicas do runtime do DDC (`diagnoseIndexError`, `stringContainsUnchecked`)
e as `RegExp` do `JSNoSuchMethodError` mantinham vivos o `noSuchMethod`
inteiro, os mapas e as expressões regulares. Remendos do SDK só no perfil de
produção (`emit_js/src/sdk_patches.rs`), nomes de *template* `JS()` que não
alcançam classes chaveadas por `dartx`, promoção dos dois lados de `&&`, `x++`
descartado como `++x`, *inline* que descarta argumento de parâmetro sem uso (o
`checkBinding` do ngdart), constantes de enum uma a uma, e regras rti e
`addRtiResources` do programa numa chamada só. O verificador pula comentários.

| alvo (bruto / gzip) | antes | depois | `dart2js -O4` |
| --- | ---: | ---: | ---: |
| `print('hi')` | 84,7 KB | **52,0 KB** | 5,1 KB |
| `01_print` | 85,5 / 28,0 KB | **60,1 / 20,2 KB** | 34,9 / 11,5 KB |
| corpus (240) | 23,37 / 7,59 MB | **17,06 / 5,66 MB** | 9,76 / 3,17 MB |
| mediana nosso/dart2js | 2,35× | **1,64×** | — |
| `limitless_ui` | 3,857 / 1,136 MB | **3,727 / 1,099 MB** | 3,708 / 0,984 MB |
| `new_sali` | 8,024 / 2,243 MB | **7,854 / 2,189 MB** | 7,616 / 2,019 MB |

Corpus diferencial 241/241 nos dois perfis; `limitless_ui` e2e 15/15 e 47
rotas limpas nos dois perfis; `new_sali` carrega sem erro. Pendente, por
ganho: omitir os nomes dos campos `late` (~64 KB / ~19 KB gzip no
`limitless_ui`; o `dart2js -O2` já faz, mas muda a mensagem do
`LateInitializationError` — decisão do dono); o `dart:_rti` no piso e os
operadores das classes nativas vivos pela regra "operador sempre vivo";
assinaturas e `dart.fn` com análise de fluxo de valores de função; o
*polyfill* do DOM em todo programa (`docs/JS-PRODUCAO-TAMANHO.md` §12).

## JS: `limitless_ui` corrigido e nova rodada de tamanho (2026-10-13, tarde, macOS)

Os dois erros da rodada anterior também aconteciam no desenvolvimento.
Membros `external` de `extension` sobre tipos de interop
(`SafeElement.innerHtml`) agora viram acesso direto ao receptor, e o `isA` dos
tipos de `dart:js_interop` é rebaixado como no DDC. Os campos estáticos de
tipos de extensão de interop (`Node.ELEMENT_NODE`) passaram a ser emitidos;
sem eles o `sanitize_dom` travava no desenvolvimento. Corpus **241/241** em
desenvolvimento e produção (novos 239–241). `limitless_ui`: 47 rotas sem erro
no Chrome e e2e **15/15** nos dois perfis (a suíte atual tem 15 testes; antes,
a produção passava 1/15). `new_sali` carrega sem erro.

| alvo (bruto / gzip) | antes | agora | `dart2js -O4` |
| --- | ---: | ---: | ---: |
| corpus (237) | 23,86 / 7,58 MB | **23,05 / 7,49 MB** | 9,61 / 3,12 MB |
| `01_print` | 88,4 KB | **85,5 KB** | 34,9 KB |
| `limitless_ui` | 4,03 MB | **3,86 MB** | 3,71 MB |
| `new_sali` | 8,34 MB | **8,02 MB** (24–28 s) | 7,62 MB (68 s) |

O que reduziu: regras de tipo repetidas apontam para a primeira, assinaturas
sem `_ti =>`, estáticos literais atribuídos na carga, tabela de nomes com
prefixo compartilhado, formas compactas `t$G`/`t$Q`, `null` encadeado,
símbolos `dartx` num laço e `Object.create` puro
(`docs/JS-PRODUCAO-TAMANHO.md` §11). `DARTFORGE_JSPROD_RULE_FORWARDING=0`
desliga o encaminhamento de regras.

## JS de produção: o piso do runtime (2026-10-13, macOS)

Substituições de variável de tipo só as citadas, nativos de *typed data* só
com código estrangeiro vivo, *inline* de funções triviais (`inline.rs`),
nomes privados do SDK sem uso fora, constantes-apelido dobradas, classe só
comparada (`instanceof`) não instanciada, e o enum de
`JS_GET_NAME`/`JS_BUILTIN` fora do mundo (`docs/JS-PRODUCAO-TAMANHO.md` §10).

| alvo | antes | depois | `dart2js -O4` |
| --- | ---: | ---: | ---: |
| corpus (237 programas, brutos / gzip) | 33,1 / 9,30 MB | **23,9 / 7,58 MB** | 9,6 / 3,1 MB |
| `01_print` | 128,6 KB | **88,4 KB** | 34,9 KB |
| `limitless_ui/example` | 4,10 MB | **4,03 MB** | 3,71 MB |
| `new_sali/frontend` | 8,42 MB | **8,34 MB** (30 s; dart2js 68 s) | 7,62 MB |

Corpus `--production` e desenvolvimento 238/238. A varredura das 47 rotas do
`limitless_ui` no Chrome mostrou dois erros anteriores à rodada
(`SafeElement|get#innerHtml` em 45 rotas e um `isA` não rebaixado na rota
`typeahead`): o e2e 26/26 precisa ser refeito. Variáveis novas (`=0`
desliga): `DARTFORGE_JSPROD_TYPE_VARIABLES`, `_NATIVE_GATE`, `_INLINE`,
`_PRIVATE_NAMES`.

## Nativo no modo padrão (A0): tamanho e desempenho (2026-10-12, noite)

Meta do dono: executáveis menores que os do `dart compile exe` e desempenho
perto ou acima do Dart, com a saída igual. Commits `78a576ee`, `87843aae`,
`27df80c1`, `d0cb9da5`, `4e1114aa` e o deste registro. Medido só no macOS
arm64: o ligador (`-no_exported_symbols`, outliner) é do Mach-O; `nounwind` e
o contexto em `thread_local` valem no Unix (o Linux não foi medido nesta
rodada); o Windows fica como era nesses dois.

**Tamanho.** `new_sali/backend` (`aot --optimize`, macOS): **61,9 → 37,8 MB**
(`dart compile exe`: 26,5 MB). Programas do `bench/performance`: 3,46 → 3,20 MB
(Dart: 5,33 MB; 40% menores). O que entrou, na ordem do ganho no backend:

| mudança | backend |
| --- | ---: |
| partida (`3dfe34e7`) | 61,9 MB |
| `-no_exported_symbols` no executável (árvore de exportação, tabela de símbolos e ligação fraca de ~50 mil símbolos `weak`/externos; `__LINKEDIT` 7,4 → 0,6 MB) | 54,6 MB |
| `nounwind` nas funções Dart (sem CFI em DWARF; `__eh_frame` 1,43 → 0,23 MB) | 53,1 MB |
| *machine outliner* do LLVM na LTO do programa grande (`driver::outliner_na_ligacao`; `__text` 44,6 → 30,0 MB) | 38,5 MB |
| tabela da RTI comprimida (1,37 MB de texto → ~0,26 MB) | 37,3 MB |
| literal de mapa montado de uma vez, pontos de coleta a menos | ~37,0 MB |
| contexto em `thread_local` (desempenho; +0,2 MB no grande) | 37,5–37,8 MB |

Medido e descartado: `minsize` no programa grande (38,58 contra 38,51 MB);
`-machine-outliner-reruns=1` (−0,46 MB, ligação mais lenta; não ligado);
exceções por tabelas (A1) sem o outliner tiram só 0,7 MB do `__text` e põem
2 MB de tabelas — a checagem da pendência não é o que pesa. A tabela de
registro das classes (`df.register.program`, 37 mil chamadas, 440 KB) como
dados ficaria do mesmo tamanho (~400 KB de tabelas com relocações).

O que resta para ficar abaixo do Dart está no código por função (`__text`
30 MB contra ~10,7 MB de código do Dart, §1.3 de
`docs/NATIVO-PRODUCAO-GRANDE.md`): moldura da pilha-sombra, conferência da
pilha, adaptadores `$c`/`$g`/`$tc`/tear-offs (~12 mil funções), `$async`
(3,6 MB) e os inicializadores de literais não constantes de folhas
constantes (`unormdata` ×2, 1,3 MB mesmo com o outliner).

**Desempenho** (`bench/performance`, A0/`dart compile exe`, média geométrica):

| medida | antes | depois |
| --- | ---: | ---: |
| rodadas separadas (`measure-performance-modes.py`, 3 repetições) | 1,231 | 1,094 |
| base, novo e Dart alternados no mesmo laço (máquina com carga ~15–19) | 1,161 | 0,999 |

Por programa (alternado): chamadas 0,91 → 0,77; coleções 0,64 → 0,64; JSON
1,96 → 1,27; numérico 0,61 → 0,58; objetos em coleções ~1,7–2,2 (ruído da
carga; numa repetição só deles 2,23 → 1,87); objetos que escapam 2,41 → 2,24;
temporários 0,55 → 0,51; textos 1,26 → 1,13; tipados 0,76 → 0,76.
`bench/comparison` (A0, ms; Dart 3.6.2 entre parênteses): fib 14–15 → 11–12
(9–10), árvores 33 → 25–29 (11), textos 79–82 → 73–78 (44–45), JSON 108–110
→ 103–110 (72–73), numérico igual (~96–107); "olá" 3,46 → 3,22 MB (5,33).

O que mudou no código gerado (todos no modo padrão, fora do ARC):
* O contexto da thread numa variável `thread_local` do módulo
  (`sdk_modulo::com_contexto_em_tls`): o prólogo não chama mais a função Rust
  com quadro; o `fib` passava 1/3 do tempo nela.
* O marco da conferência da pilha nasce no bloco de entrada (virava alocação
  dinâmica depois do `df.get_area` em linha).
* Objeto recém-alocado sem ponto de coleta desde a alocação: corpo direto e
  gravação sem barreira; `this` com corpo direto no AOT (sem corpo de fora).
* Literal de mapa `{k: v, …}` como na VM (`_dartforgeFillMap`), em vez
  de um `[]=` por seletor por entrada; o `hashCode` das chaves passa a rodar
  depois de todos os valores, como na VM.
* Texto constante estático, vetores constantes e leitura do vetor de
  argumentos deixam de ser pontos de coleta.
* As raízes pelo chamado (`DARTFORGE_ROOTS_IN_CALLEE=1`) ficam só sob
  medida: com menos raízes, slots compartilhados retêm valores mortos até o
  quadro fechar (núcleo `json`: 1,05 milhão de objetos vivos contra 650 mil,
  pico 121 contra 95 MB). Falta zerar o slot do valor que morre.

Validação: corpus nativo 125/126 (o `33` difere em todos, soquete Unix) em
tracing desenvolvimento e produção, produção com `DARTFORGE_GC_STRESS=1`,
desenvolvimento com GC stress, produção com o outliner forçado
(`DARTFORGE_OUTLINER=1`), ARC e preparado; emissor 476 testes (`--lib`) e
doctests novos; runtime 226 + integrações + 85 doctests;
`new_sali/backend` compila e sobe (mesma saída de partida do Dart).
`compile-native --emit-ir --optimize` agora emite o IR da produção.

Próximos passos, pelo que o perfil mostra: `tlv_get_addr` ainda aparece
(2–5% em árvores e textos; o Go lê `TPIDRRO_EL0` direto); mapa de `String`
(`hash_mapa_buscar` 20% do núcleo `textos`, `[]=`/`[]` passam pelos
adaptadores `$c`); alocação (`arvores` 2,2–2,5× o Dart: varredura dos jovens,
`memset`); `ordenar_objetos` e `lista_ligada` (2–4×).

## Retomada de 2026-10-12 (macOS arm64)

ARC continua só com `--memory=arc`; tracing é o padrão. §§27–34, a
publicação da preparação ARC no CLI AOT/JIT e o gate ARC >= A0 **continuam
incompletos**.

### Ambiente desta máquina

* LLVM 22.1.8 oficial (`LLVM-22.1.8-macOS-ARM64`, recorte igual ao da ação
  `llvm-unix`) em `/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64`; Dart
  3.6.2 em `~/tools/dart/3.6.2/dart-sdk` (o de `/Volumes/ssd_dock/DartSDKs`
  é o de Windows); `gh` no PATH, já autenticado. Exportar `DARTFORGE_CLANG`,
  `DARTFORGE_LLVM_DIR`, `LLVM_SYS_221_PREFIX`, `DARTFORGE_SDK_LIB` e
  `SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk`: o clang
  do pacote não acha o SDK sozinho. O diagnóstico ARC exige também
  `DARTFORGE_MEMORY=arc`; `compile-native --emit-ir` procura `ld64.lld` no
  PATH (pôr o `bin` do LLVM nele).
* O volume passou de “SSD Dock” a `ssd_dock`. Os build scripts guardam o
  caminho antigo por `env!`: depois de mudar o caminho, `touch
  crates/*/build.rs` (senão “manifest path … does not exist”). Sem o espaço,
  o `llvm-sys` do JIT local deixa de ter o impedimento anterior (ainda não
  experimentado). Com JIT, o linker do pacote exige o SDK 26.5: os `.tbd`
  do SDK 27 trazem `arm64e.x1`, que o LLVM 22 não reconhece.
* `python3` do sistema é 3.9: `scripts/prove-arc-aot.py` deixou de usar
  `write_text(newline=)`.

### CI e regressões corrigidas

O `main` estava vermelho desde `aa05983e`, e ninguém tinha percebido:
* Windows: `runtime/tests/ownership.rs` não listava `dartforge_value_class`
  e `dartforge_list_append`, auditadas no catálogo (75 ≠ 77).
* Linux, macOS e Windows: `memory_cli` (AOT e JIT com `--memory=arc`)
  entrava em pânico. Desde `a1edd273`, closures, chamadas tipadas e, depois,
  seletores exigiam portas certificadas sob ARC. Mas a CLI não prepara nem
  certifica os corpos. Agora `Module::arc_preparado` marca a publicação da
  preparação: corpos preparados exigem certificado para os invocadores
  uniformes (guarda preservada). Sem preparação, a imagem ARC segue o regime
  legado da CLI, com chamada direta. As fábricas com owner do código
  (literal de lista, interpolação) só são emitidas em corpos preparados, que
  inserem a liberação desse owner. `memory_cli` (AOT) passa localmente.

Runtime no macOS: os testes de `rastro.rs` alocavam seção e registros em
blocos separados do heap e gravavam deslocamentos `i32` entre eles. No
macOS, esses blocos podem ficar a centenas de GiB de distância, o que
produzia SIGBUS/SIGSEGV em paralelo e uma falha em série. Agora os testes
usam uma área única; as asserções não mudaram. Também foi removido um
`unsafe` aninhado que só existe fora do Windows (`heap.rs`).

### Preparação conjunta com o SDK da fonte

`arc_audit_pipeline --memory=arc --sdk-in-group --externs-inventory`
em `bench/performance/numeric.dart` passou, em ordem, por:
* `Stopwatch.start` (`CallSeletor`): a mudança dos seletores preservada no
  fechamento foi validada, inclusive pela prova AOT ampliada.
* `Uri.splitQueryString`: a tupla RTI vinda de getter `df.rti.*` é aceita
  só com prova estrita (`caches_rti::getters_rti`: retorno de cache auditado,
  de `dartforge_rti_recipe` ou de outro getter comprovado, e todos os
  escritores do cache vindos da receita). Uma constante não basta. Os gates
  com módulo exigem essa prova; a produção de contratos por corpo aceita só
  a forma.
* `medir` (`JuntarTextos`): nova fábrica
  `dartforge_arc_string_join_owned_v1`, que enraíza as partes, copia as
  unidades e entrega owner. Partes só Ref/`int` em SSA.
* `df.$type.toString`: leitura escalar auditada do ID; o texto ganha owner
  na preparação (`dartforge_arc_rti_string_owned_v1`); convenção de retorno
  Ref declarada.
* `dartforge_rti_init`: `dartforge_rti_init_table` passou de `1 1 0`
  para `0 0 0`. Isso foi confirmado pela análise estática (`folha_pura`) e
  por execução AOT tracing com `DARTFORGE_EFFECTS=check`, normal e sob
  GC stress (nome presente na tabela conferida, saída igual à do Dart). O
  parâmetro Rust passou a `*const i64`, como no vetor constante do módulo.
* `df.ffi.v_lp$ent`: contratos de `dartforge_ffi_closure_address`
  (`pending`) e `dartforge_ffi_pointer_address` (`pending-dart`), e
  `ChamadaNativa` só com escalares. O resultado é Trivial, sem pendência
  (`dartforge_ffi_callback_exit` limpa a exceção do callback) e com
  reentrada Dart.

* `df.ffi.v_lp$cb` (corpo do callback): o lowering registra, sob ARC, o
  contexto e os argumentos não-Ref como fatos escalares. A closure do
  contexto e o `Pointer` do argumento ganham owner por
  `dartforge_arc_ffi_callback_closure_owned_v1` e
  `dartforge_arc_ffi_callback_pointer_owned_v1` (o ajudante Dart roda num
  escopo nativo; o token da porta Owned é copiado antes de fechar).
* `df.ffi.p_$ent`: `dartforge_arc_ffi_return_pointer_owned_v1`, com a
  closure emprestada e o mesmo protocolo de escopo.
* `df.ffi.l_HHl$ent` (`Handle`): o escopo de handles agora abre depois das
  conversões que lançam e fecha antes das do retorno. Corrige um defeito
  também do tracing: uma exceção entre abrir e fechar (ou qualquer retorno
  de struct, que nunca fechava) deixava células e raízes para sempre. O
  `Handle` devolvido é lido e o escopo fechado na mesma chamada
  (`dartforge_ffi_handles_close_with_object`; sob ARC, a variante Owned
  retém antes de soltar as células). `handles_abrir`, `handle_novo` e
  `handles_fechar` passaram de `1 1 0` para `0 0 0`, conferido por
  execução com `DARTFORGE_EFFECTS=check` (29 pontos instrumentados no
  IR), normal e sob GC stress, tracing e ARC legado da CLI: saída igual à
  do `dart run` 3.6.2 (`target/prova-handles/`). Handle inválido: a VM não
  confere (comportamento indefinido); o DartForge lança `ArgumentError` e
  fecha o escopo — exercitado 3000 vezes sob GC stress.

O módulo 0 (programa e FFI) passa inteiro. No SDK:
* Ajudantes em IR (`df.string_*`, `df.core_len*`, `df.typed_*`,
  `df.identical`): leituras escalares puras do objeto emprestado ganham
  contrato do lado do emissor (`optimize::arc::CONTRATOS_DOS_AJUDANTES`,
  consultado antes do catálogo do runtime; o resultado `i1:scalar` entrou
  no gerador). Continuam em linha. `df.typed_base` empresta a base da
  visão. O teste `ajudantes_concordam_com_os_efeitos` amarra os contratos
  aos efeitos.
* Sem endereços derivados sob ARC: `df.string_allocate` vira
  `dartforge_arc_string_allocate_owned_v1`; listas do núcleo leem/gravam
  elementos compactos por `dartforge_arc_list_read_bits_v1` (nova) e
  `…_gravar_bits_v1` sobre o armazenamento `Ref` emprestado; as voltas N13
  (que guardariam o armazenamento) ficam desligadas; listas tipadas não têm
  caminho rápido e o acesso por índice/bytes vai ao intrínseco do runtime
  (`RangeError`), agora com contrato (`ref:borrow`, `pending-dart`), assim
  como `DartForge_string_codeUnitAt`.
* Defeito corrigido: `sdk_fonte.rs` baixa cada função do SDK num módulo à
  parte e, nas quatro junções, perdia `retornos_ref_dart` e `sitios_arc` —
  todo retorno Ref do SDK ficava sem a convenção Owned. As junções passam
  por `lower::juntar_fatos_arc`, também usada no lowering paralelo.

Depois disso, a preparação conjunta passou o módulo 1 do SDK inteiro e
avançou pelo `dart:core` (módulo 2). O que entrou:
* Exceções: um ponto único de lançamento (`emitir_lancamento`); sob ARC o
  valor vai como `Ref` emprestado a `dartforge_arc_throw_ref_v1`/
  `…_com_rastro_ref_v1` com a guarda canônica e caminho morto
  `Unreachable`. `catch`/`finally` leem a exceção e o rastro pela
  preparação como owners (`dartforge_arc_exception_owned_v1`,
  `…_rastro_owned_v1`). Os 14 construtores de erro do runtime ganham
  variantes `dartforge_arc_<nome>_owned_v1` (escopo nativo + retain,
  `owner_em_escopo_nativo`). As trocas por variantes Owned ficam todas na
  preparação (`optimize::arc::nativos`): o regime ARC legado da CLI, sem
  preparação, não ganha owners que ninguém soltaria.
* Contratos gerados para 167 natives escalares e 79 sem resultado, das
  assinaturas Rust reais (o `build.rs` confere) e de `efeitos.tsv`: `Ref`
  emprestado, saída pelos efeitos, invalidação quando recebem `Ref`,
  alocam ou rodam Dart. Novo modo de parâmetro `u8/i8:scalar`.
* Thunk Owned genérico `df.arc.owned.<base>` (definido pelo emissor LLVM:
  escopo nativo, guarda, retain) para natives que devolvem `Ref` e uma
  lista explícita de construtores do runtime (`typed_novo`, `view_nova`,
  `string_concat`…); contrato derivado na hora dos tipos da chamada.
* Owners para records literais (`dartforge_arc_record_from_refs_owned_v1`,
  arestas contadas), tear-offs canônicos, objeto `Type` canônico, `is`/`as`
  (`rti_e`, `rti_como*`), consultas de record, ABI uniforme
  (`args_casam`, `arg_indice`, slot da tupla de tipos).
* Phis: cópia do empréstimo na aresta quando o Phi recebe owner, é
  devolvido com entradas triviais ou recebe um owner ainda vivo depois da
  junção (`arc/phis.rs`); Phis escalares I8; aliases de parâmetros não são
  invalidados por barreiras locais.
* Corpos gerados com a convenção Dart: closures, getters de constante e de
  global, corpos e entradas uniformes de recusa (antes perdiam
  `retornos_ref_dart`/vetores); globais Dart escalares (`I64` pelo tipo)
  com fato próprio (`globais_escalares_dart`); saídas compartilhadas por
  guardas separadas por caminho; `Unreachable` não exige tokens; retorno
  implícito explícito; `And/Or/Xor` seguem a semântica do emissor (i64 com
  operandos estendidos); bit/valor de Smi por `@df.bit_smi`/`@df.smi_value`.
* Marcas rebaixadas (`1 1 0` → `0 0 0` ou `1 0 0`): `rti_e`,
  `exception_capturavel`, `args_casam`, `arg_indice`, natives de record,
  `string_igual_a`, `string_concat`, `to_string_bool/f64`, `rastro_texto`,
  `membro_recusado` (encerra o processo). Execução com
  `DARTFORGE_EFFECTS=check` e GC stress, tracing e ARC legado
  (`target/prova-efeitos/`), instrumentou no programa `arg_indice`,
  `args_casam`, `capturavel`, `record_fieldAt/numFields`, `rti_e`,
  `string_concat` e `to_string_bool`, com saída igual à do Dart. As
  demais só são chamadas pelo SDK compilado à parte e ficam conferidas
  pela leitura da implementação.

Depois, até o fim do SDK (16 módulos):
* Phis (`arc/phis.rs`): a entrada owned lida depois da junção, inclusive
  por outro Phi adiante (entrada de Phi conta como uso no predecessor) e
  quando ela própria é um Phi; o mesmo token em dois Phis da mesma aresta
  (troca `r0, r1 = r1, r0`); Phi devolvido de triviais por outro Phi; o
  critério “precisa de owner” espelha o produtor (classes `Borrowed` cruas
  idênticas nas entradas diretas; o empréstimo do chamador fica preso ao
  parâmetro que o traz) e exige que o owner do empréstimo domine o Phi
  (o cabeçalho de laço que carrega um alias de valor do corpo).
* Saídas por guarda (`arc/saidas.rs`): o trampolim vazio até um tratador é
  copiado por guarda, e um tratador com instruções ganha um trampolim por
  guarda; a aresta normal recebe o cleanup do caminho.
* Captura por endereço das funções locais diretas: `PlanoTokens` ganha
  `enderecos_recebidos` (parâmetro `Ptr` → tipo do local) e
  `enderecos_cedidos` (chamada → posições); os tipos saem de um ponto fixo
  no módulo (`locais::produzir_enderecos`), conferindo tipo, inicialização
  antes da chamada e ausência de alias. Locais SIMD entram no produtor.
* Leituras e escritas físicas por índice: `CellGet` escalar e
  `dartforge_object_get/set` (quadro assíncrono, record, enum, campo por
  índice) vão às entradas auditadas `ler/gravar_campo_*_v1`; captura
  escalar por `dartforge_arc_scalar_env_v1` (nova, confere classe e mapa).
* `Unbox` SIMD e as operações `Simd` (exceto `Carrega`/`Grava`, que leem
  por endereço) são puras, com origens Trivial; `TabelaDeFuncoes` é dado
  constante. O thunk Owned genérico aceita `Ptr` (tabelas constantes) e
  cobre `lista_de_tabela(_g)`.
* Convenção Owned também para corpos `async`/`sync*`, funções de valor
  padrão, getters `late`, valores e `toString` de enum. `=> expr` numa
  função void devolve vazio (o LLVM já emitia `ret void`).
* `%` com divisor variável é emitido em linha depois do guarda do zero
  (`emit_modulo`, como `~/`), sem chamar o native que lança — vale também
  para o tracing.
* Contratos de `Isolate_spawnFunction`, `Isolate_spawnUri` e
  `Process_Start` (o gerador do lote aceita atributos entre `no_mangle` e a
  assinatura).

`arc_audit_pipeline` termina com `Preparação conjunta com SDK da fonte:
Ok((21945, 240417))` (cópias, drops). É a preparação conjunta em memória,
ainda **não publicada** na CLI: nenhum binário AOT roda com ela, e as
entradas novas (`env_escalar`, trampolins, captura por endereço) só têm
testes unitários e de preparação.

### Publicação na CLI (`DARTFORGE_ARC_PREPARED=1`)

Opção explícita, só com `--memory=arc`; o padrão (tracing) e o ARC legado
não mudam. Com ela:
* SDK: `emitir_bibliotecas_do_sdk` baixa as 15 bibliotecas, prepara-as
  juntas (`preparar_arc_modulos_dart`) e certifica as portas do conjunto
  (`certificar_portas_arc_modulos`: idempotência do conjunto inteiro uma
  vez, layouts da união, ABIs tipadas pelo registro do runtime). A opção
  entra na chave do cache (`arc=preparado_v1`).
* Programa: preparado sozinho com `Module::arc_callees_externos` — cada
  callee do SDK ganha o resumo da própria chamada (`resumos_externos`:
  tipos dos operandos, falha possível, Ref Owned); certificado como imagem.
* Escopo nativo: funções do runtime que rodam Dart, com contrato, vão por
  thunks `df.arc.scope.<base>` (abrir, chamar, fechar); as ABIs
  `dartforge_arc_*` escopam por dentro (`em_escopo_nativo`, também nos
  handles FFI). A entrada abre um escopo raiz para a preparação do isolado
  e outro para `main` e o laço de eventos (os tokens de portas chamadas
  pelo runtime fora de escopos internos só morrem no fim: correto, mas
  acumula; isolados de `Isolate.spawn` ainda sem escopo).
* No modelo de checagem, chamadas falíveis sem guarda (a otimização tirou
  a guarda de um callee que a análise conjunta vê como falível, p.ex.
  getters de constante com `lista_de_tabela_g`) ganham a guarda de
  pendência, em vez de `invoke`.
* A certificação aceita comparação de identidade de handles, as fábricas
  contratadas (`AllocList`, `AllocRecord`, `JuntarTextos`, caixa SIMD),
  `ArcLoadStrong` e as chamadas C de ABI escalar.
* `dartforge_call_dart0` (o laço de eventos) é callback Owned; `toString`
  de formas de record com convenção Owned; leitura Ref de campo zerado e
  ainda não gravado dá null (`ler_campo_ref_v1`); o thunk Owned das
  fábricas tipadas usa a variante `_t`, que registra a tabela da classe
  (sem ela, a visão tipada caía num laço de `noSuchMethod`); cargas e
  gravações de campo FFI conferem a pendência (valem para todos os modos).
* Contratos: `rti_registro_nomeado` (`1 0 0`), `equal` (`0 0 0`, pura),
  `ffi_endereco_do_composto`, compostos FFI pelo thunk Owned, chamada C
  com composto por valor.

Os quatro programas de prova (`prova-lote5`, `-ajudantes`, `-efeitos`,
`-handles`) rodam com programa e SDK preparados, com e sem
`DARTFORGE_EFFECTS=check`, com `DARTFORGE_ARC_CHECK=1`, normal e GC
stress: 16 execuções iguais ao `dart run` 3.6.2 (`p6-*`).

Corpus nativo (`corpus/native`, 130 programas) com a opção, contra o
`dart run` (`target/corpus-preparado*/resumo.tsv`). A primeira rodada achou
e corrigiu:
* escopo nativo por evento do laço e por isolado de `Isolate.spawn`
  (`em_escopo_nativo_se_arc`: só com portas ARC registradas);
* a máquina de estados assíncrona gravava no quadro entre a chamada e a
  guarda de pendência: a gravação vai para a aresta de sucesso e as
  releituras para antes da chamada (vale para todos os modos);
* `layouts_externos_arc`: os layouts declarados do SDK, para conferir campos
  de objetos dele no programa preparado sozinho;
* poda da HIR solta também `retornos_ref_dart`, `funcoes_locais_diretas_arc`
  e `sitios_arc` das funções removidas;
* `this` escalar de membros de extensão e de tipos de extensão com fato de
  parâmetro escalar; Phis de vetor SIMD como escalares; função de literal em
  pedaços com retorno Owned; `num` → `double` com guarda;
* leituras auditadas aceitam objetos estáticos da imagem
  (`Heap::objeto_legivel`) e o record posicional do runtime (contagem
  escalar na palavra 0, campos Ref);
* contratos: `ffi_ponteiro_novo` (thunk Owned), `ffi_simbolo_nativo` (modo
  novo `bytes:native`), `rti_tupla_juntar` (`1 0 0`), `lista_de_tabela`
  (`1 0 0`), `double_modulo` (`0 0 0`), `df.num_to_double`.

Depois, os locais Ref capturados por funções locais diretas: o inlining
deixava o local só com `Load`/`Store` sem promovê-lo (o mem2reg agora roda
após cada inlining que muda a função), e um ramo morto com `null == 0` não
era dobrado (`GetField` sobre `null`). O corpus fica com 125 de 126 iguais
ao Dart, no preparado e no tracing; o `33` imprime `null` nos dois modos
(anterior ao ARC, soquete Unix no macOS).

Exceções por tabelas (`--exceptions=tables`) com a preparação publicada:

* a preparação do programa importa os invokes e pousos materializados
  (`preparar_arc_modulo_tabelado`); antes partia de planos vazios e
  recusava o `invoke` de uma `ChamadaTipada` sem pendência;
* closures e seletores ficam com a conferência, sem invoke: a invocação
  uniforme do runtime devolve a exceção pendente (`uniformes` em
  `tabelas.rs`); `ChamadaTipada` continua sítio;
* no fluxo de tokens, um sítio com invoke conta como chamada que pode falhar
  (o drop do resultado não vai para o pouso), e um invoke de callee que não
  falha só deixa um pouso morto;
* o emissor pré-nomeia os rótulos `lpad<b>.dart` dos pousos que não retomam,
  para os phis das saídas.

O corpus fica com 125 de 126 iguais também nesse modo
(`scripts/native-corpus.sh tables`).

Rastro simbólico (`--stack-trace=symbolic`) com a preparação:
`dartforge_stack_trace_enter` (borrow, sem retenção: o desenrolamento só lê o
endereço enquanto o quadro que o empurrou está na pilha) e
`dartforge_stack_trace_exit` ganham contrato. Um programa com lançamento
síncrono e cadeia async (`target/prova-rastro/`) sai igual em tracing e no
preparado, normal/GC stress. Os dois perdem os quadros de quem espera
(`meio`, `topo`) que a VM mostra: a cadeia async do §13.14 ainda não
casa no macOS, independente do ARC.

### Desempenho no macOS arm64 (`aot --optimize`, 3 repetições alternadas)

`scripts/measure-performance-modes.py` ganhou o modo `ARCP` (a preparação
publicada) e roda fora do Windows. Primeira medida neste Mac
(`target/bench-arcp/`): ARC/A0 15,5 e ARCP/A0 18,2 na média geométrica,
contra 2,0 da última medida no Windows (55beeb34). A causa principal era
483bfe01, que tirou do lowering ARC os endereços derivados das listas:
`produto_f64` levava 2201 ms no ARCP contra 3 ms no A0, `lista_leitura` 224
contra 1,3.

Os caminhos rápidos voltaram (62eda051). A preparação
(`optimize/arc/derivados.rs`) põe `@df.keep_alive(lista)` depois de cada
acesso pelo endereço derivado (dados de tipadas, armazenamento do núcleo):
a lista fica emprestada e viva até ali. O verificador recusa endereço que
escapa (para chamada, terminador ou outro uso) e acesso ao armazenamento do
núcleo com barreira em algum caminho desde a leitura (o crescimento troca o
armazenamento). As voltas N13 continuam desligadas sob ARC: o ponto seguro
pode rodar Dart. Com isso (`target/bench-arcp2/`): ARC/A0 8,7, ARCP/A0 9,6;
tipados, crivo, numérico e `fib` ficam iguais ao A0.

No runtime, a conferência do retorno Owned das portas uniformes procurava o
bloco nas listas de livres a cada retorno (`observar_owner_codigo`); agora
só consulta o token. A conferência física de layout busca o par `(cid,
campos)` por busca binária (o produtor já ordena; fora de ordem cai na
sequencial). No `json` ARCP: `encode_grande` 9060 → 600 ms, `reviver` 7452
→ 310, `utf8_bytes` 3607 → 370 (ainda 8–10× o A0).

O que domina agora (perfil do `json` ARCP): os tokens do código num
`HashMap<Ref, usize>` (`owners_codigo`, inserção e remoção a cada
retain/release), a tabela ARC, a drenagem dos zeros e a conferência física
das leituras e gravações de campo. O ARC legado estourava a memória no
`json` (1,78 GB de pico, saída 255): desde 4758df70 a leitura de elemento
`Ref` de lista geral sob ARC criava um token do código que, sem a
preparação, ninguém soltava. Sem preparação, a leitura agora é emprestada
(`dartforge_arc_list_ref_v1`): pico de 331 MB e `json` 2,6–3× o A0 no ARC
legado (antes 55–123×).

Rodada válida com tudo isso (`target/bench-arcp3/`, 88b6e57e, 3 repetições,
com agentes compilando ao lado): **ARC/A0 2,748, ARCP/A0 5,225**. Maiores
custos do ARCP: `chamadas/formas` 23×, `closures` 17×, `lista_leitura` 16×,
`arvores` 14×, `mapa` 14×, `lista_add` 13×, `ordenar_objetos` 12×,
`mapa_str_objeto` 12×; `json` 7–10×. No legado: `arvores` 16×,
`lista_leitura` 16×, `lista_add` 10×.

As voltas N13 voltaram sob ARC. O ponto seguro e a limpeza da pendência
não contam como barreira do armazenamento (as mensagens de controle não
rodam Dart do programa; limpar só solta a exceção). Na preparação, o
caminho lento do `[]` dentro das voltas rápidas é uma chamada; então, com
a preparação, cada acesso relê o armazenamento (a forma conferida antes
das voltas vale). `lista_leitura`: legado 20,2 → 1,28 ms (o A0), ARCP
20,2 → 6,2. `lista_leitura` (15×) espera N13 sob ARC; `chamadas/closures`
(17×) e `formas` (23×) passam pela invocação uniforme.

**Próximo passo:** tokens do código fora do hash (contador na tabela ARC),
leituras de campo `final` emprestadas sem retain; depois §§27–34 e o gate ARC >= A0.

### Evidências (macOS arm64, logs em `target/`)

| verificação | resultado |
| --- | --- |
| emissor, unitários incluindo ignorados | 480 + 110 doctests (473 + 7 ignorados com tabelas) |
| quatro programas de prova com programa e SDK preparados (`DARTFORGE_ARC_PREPARED=1`), com/sem `conferir`, `DARTFORGE_ARC_CHECK=1`, normal/GC stress | 16 execuções, saída igual ao Dart (`p6-*`) |
| os mesmos em tracing e ARC legado, com `conferir`, normal/GC stress | 16 execuções, saída igual ao Dart (`r6-*`) |
| provas AOT do CI (`prove-arc-aot.py`, 8 exemplos) | todas `sucesso` (`prova6-*.log`) |
| corpus nativo com programa e SDK preparados, `DARTFORGE_ARC_CHECK=1` | 125 de 126 iguais ao Dart (`corpus3-prep/resumo.tsv`) |
| corpus nativo preparado com `--exceptions=tables` | 125 de 126 iguais (`corpus-tabelas/resumo.tsv`) |
| quatro programas de prova, preparado com `--exceptions=tables`, `DARTFORGE_ARC_CHECK=1`, normal/GC stress | 8 execuções iguais ao Dart (`p7/`) |
| corpus nativo em tracing, mesmo binário | 125 de 126 iguais (`corpus3-tracing/resumo.tsv`; o `33` difere nos dois) |
| marcas rebaixadas (`41`, `122`, `gc_d02`, `95` do corpus) com `conferir`, tracing/ARC legado/preparado, normal/GC stress | 24 execuções iguais (`corpus-conferir/`) |
| quatro programas de prova, tracing/ARC legado/preparado, `conferir`, normal/GC stress | 24 execuções iguais (`r7-*`) |
| runtime, todos os alvos incluindo ignorados | 227 + integrações + 83 doctests (`lote5-runtime.log`) |
| `%` variável, setter `=>`, `late`, enums, `sync*`/`async`, captura escalar e por endereço, records, SIMD, `try`/`finally`, com e sem `DARTFORGE_EFFECTS=check`, tracing/ARC, normal/GC stress | 8 execuções, saída igual ao Dart (`prova-lote5/`) |
| programas anteriores (`ajudantes`, `efeitos`, `handles`) refeitos com conferir, tracing/ARC, normal/GC stress | 12 execuções, saída igual ao Dart (`r5-*`) |
| `Handle` AOT, tracing/ARC, normal/GC stress, efeitos conferidos | 8 execuções, saída igual ao Dart (`prova-handles/`) |
| listas compactas/gerais, tipadas, visões, `ByteData`, strings pela CLI, tracing e ARC, normal/GC stress | 4 execuções, saída igual ao Dart (`prova-ajudantes/`) |
| efeitos rebaixados, exceções, records, nomeados, `is`, com `DARTFORGE_EFFECTS=check`, tracing/ARC, normal/GC stress | 12 execuções (três programas), saída igual ao Dart |
| `memory_cli` (nativo), incluindo ignorados | 2 passaram (`arc-lote3-memoria-cli.log`) |
| `prove-arc-aot.py arc_native_port_owned --debug` | 24 execuções, `sucesso` (`proof-native-port-owned/evidence.json`) |
| `memory_cli` (nativo, sem JIT), incluindo ignorados | 2 passaram (`memoria-cli.log`) |
| interpolação/lista/`runtimeType` pela CLI, tracing e ARC, GC stress | saída igual ao `dart run` 3.6.2 |

As novas fábricas Owned têm testes unitários e de preparação, mas ainda
**não têm prova AOT** própria. A CLI ainda não publica a preparação ARC, e
o resultado do CI destes commits deve ser conferido com `gh run list`.

## Fechamento de 2026-10-11

Trabalho encerrado por pedido do proprietário. Não retomar automaticamente
hoje. ARC somente com `--memory=arc`; tracing permanece padrão. As §§27–34,
a integração completa ao CLI AOT/JIT e o gate ARC >= A0 **não estão concluídos**.
Este fechamento preserva também a mudança dos seletores ainda em validação.

### Avanços desde a última atualização

Os commits até `ba300764` acrescentaram:

* Composição de inclusões/escritas por campos em chamadas locais; propagação
  de publicações globais, objetos lançados, finalizadores e capturas.
  Fronteiras opacas conservam tratamento conservador. Receivers emprestados,
  derivados, anuláveis, de mixins e de junções têm origem, layout físico e
  frescor conferidos no ponto de uso.
* Owners explícitos para células, ambientes e closures; proteção das capturas
  antes de substituir o armazenamento; preservação das checagens Dart,
  reentrada e saídas de exceção pendente. Avaliação e caches RTI passam por
  contratos de proveniência e conferência das escritas.
* Convenção Owned das entradas uniformes; proveniência de vetores e
  descritores; callbacks e alvos tipados registrados a partir dos corpos e
  dependências verificados. Adaptadores de repasse de closures/seletores
  exigem destinos certificados. Retornos int/double/bool têm ABIs próprias.
  Os certificados são de imagens AOT fixas, não de recarga JIT.
* Preparação conjunta de módulos Dart e SDK, com áreas globais separadas e
  conferência das variantes compartilhadas. Slots Ref capturados por funções
  diretas são transportados por quadro e índice, com contratos, tokens,
  cleanup e verificação de vida/repasse. A integração ao CLI normal falta.
* Mapas de quadros suspensos por instância e referências restauradas
  explicitadas na HIR; leituras proprietárias de quadros assíncronos.
  Ainda não é prova de todos os caminhos de suspensão/cancelamento.
* Leitura proprietária de globais Ref; leitura `late` confere inicialização;
  escritas preservam marcas e guarda `late final`. Provas AOT conferem vida
  após limpar/substituir o armazenamento original.
* Poda de funções locais sem entrada e ramos de classe comprovadamente
  impossíveis. Phis Ref cíclicos ancorados e junções mistas com `null` deixam
  de receber classificação Trivial prematura.
* Boxing SIMD produz owner preservando os bits. Recepção UDP usa retorno
  Owned e porta registrada, preservando a raiz da exceção em pending; foi
  validada no runtime, sem prova AOT completa de um programa Dart com UDP.
* Listas: leitura proprietária de elementos gerais; comprimento, forma,
  capacidade e armazenamento auditados; escritas e troca de dados contam
  arestas Ref e preservam owners durante crescimento e descompactação.
  Armazenamento ARC é Ref, sem inferir referência de I64. Crescimento
  empresta o receiver, sem produzir um owner adicional para ele.
* Literais (`ba300764`): `dartforge_arc_list_new_owned_v1` enraíza elementos
  antes de alocar, conta referências copiadas e entrega um owner da lista.
  HIR confere tipos reais e recusa ponteiros/vetores sem caixa/representações
  incompatíveis; pistas de tags não certificam representação. Tracing
  conserva a fábrica anterior e preenchimento homogêneo sem vetor extra.
* Efeitos e `ownership.tsv` entram nas chaves de cache do runtime e SDK.
  O diagnóstico inventaria externs, assinaturas e primeiro uso antes da
  preparação. Os 581 símbolos ausentes do catálogo bruto não são 581
  bloqueios comprovados: existem normalizações e poda posteriores.

### Evidências e trabalho preservado em andamento

| verificação até a etapa dos literais | resultado | evidência local |
| --- | --- | --- |
| Runtime, unitários incluindo ignorados | 225 passaram | `target/arc-literal-runtime-test.log` |
| Emissor, unitários incluindo ignorados | 451 passaram | `target/arc-literal-emitter-test.log` |
| Exemplo executável da fábrica Owned | 1 passou | `target/arc-literal-api-test.log` |
| Literais AOT, ARC/tracing, O0/O2 | 12 execuções aprovadas, com controles negativos | `target/proof-list-literal/evidence.json` |
| Armazenamento/crescimento AOT, ARC/tracing, O0/O2 | 24 execuções aprovadas, com controles negativos | `target/proof-list-storage/evidence.json` |

As provas de literais conferem referências repetidas, caixas int/double,
bits de NaN, vida após liberar referências do chamador e liberação final da
lista, dos elementos e das caixas. Controles negativos precisam dar trap;
falha de compilação não é aprovação. São provas de HIR preparada com harness
explícito, sem demonstrar funcionamento do SDK inteiro ou do corpus do CLI.

O diagnóstico conjunto de `bench/performance/numeric.dart` e SDK da fonte
passou da lista literal v11 e chegou a `df.comum$2edart..medir` v26:
`CallSeletor` de `tc:start` (`Stopwatch.start`) sem classificação de ownership.
Evidência: `target/arc-literal-sdk.log`.

A mudança dos seletores incluída neste fechamento encaminha `CallSeletor`
em ARC ao adaptador uniforme certificado, confere receiver/argumentos Ref,
descritor, tupla RTI e guarda pending; tracing conserva a chamada legada.
Há teste de preparação/recusa transacional e extensão da prova
`arc_native_port_owned` para um seletor com vetor local.
A primeira bateria terminou com 451 aprovados e uma falha no teste novo:
emissão tentada sem certificar a imagem AOT. O teste foi corrigido para
certificar os corpos preparados, preservando a guarda de emissão. O teste
corrigido passou na repetição, mas a bateria completa ficou sem término
confirmado e foi reiniciada. Essa execução foi interrompida a pedido do
proprietário para encerrar imediatamente. Log parcial:
`target/arc-seletor-local-emitter.log`. A prova AOT ampliada e o diagnóstico
do SDK após essa mudança **ainda não foram executados**. Não afirmar 452
testes aprovados, certificação final ou SDK completo funcionando.

### Desempenho, limites e retomada

Última medição completa:
`bench/results/2026-10-10-modos-e14922fb-windows/decisao.json`.
32 kernels, nove programas, sete repetições e 189 execuções com resultados
iguais ao Dart AOT recém-compilado. Gate **reprovado**: ARC/A0 geométrico
2,005985; árvores 12,815576; lista ligada 4,985826. São razões de tempo:
ARC levou aproximadamente 2 vezes o tempo do padrão no agregado. Não existe
medição posterior comprovando melhora; rodadas distintas não isolam mudanças.

Na retomada, concluir a bateria do emissor incluindo ignorados e executar
`python scripts/prove-arc-aot.py arc_native_port_owned --debug`. Repetir
`arc_audit_pipeline --memory=arc --sdk-in-group --externs-inventory` em
`bench/performance/numeric.dart` e resolver o próximo erro observado, sem
afrouxar ownership, certificação ou guardas. Fechar preparação do SDK e
publicação conjunta no CLI AOT/JIT; conferir portas, lifetimes, exceções,
cancelamento, suspensão e recarga JIT. Faltam provas nativas específicas dos
slots compartilhados. Completar políticas/regiões/grupos/borrows das §§27–34,
otimizar e medir até ARC >= A0; depois continuar analisador/LSP.

A lentidão veio da resolução sucessiva de caminhos do SDK sem contrato,
reconstruções de ABI/cache e baterias amplas repetidas. O avanço fragmentado
ainda não entregou a integração completa utilizável; o inventário ajuda a
organizar a retomada. Não há novo resultado de CI confirmado neste fechamento.
Os testes locais não comprovam workspace inteiro, Clippy ou todos os sistemas
do CI. A cópia pessoal `docs/ANALYZER-ESPECIFICACAO copy.md` permanece intacta,
fora das alterações deste trabalho.

## Retomada de 2026-10-10

Referências da documentação Rust corrigidas no workspace: nomes antigos
como Heap::allocate passam a Heap::alocar; referências públicas para itens
privados usam texto de código, e links públicos válidos são qualificados.
Descrições de opções do emissor foram corrigidas. Removido o wrapper privado
atribuir_slots sem chamadores; o emissor já usa raizes::analisar.
Passou cargo doc --locked --workspace --no-deps --document-private-items
com RUSTDOCFLAGS=-D warnings (53,58 s), log
target/workspace-doc-links-correcao.log. Isso resolve a pendência de
documentação descrita abaixo; não altera algoritmos de memória ou políticas.
Clippy continua com as pendências registradas. ARC apenas --memory=arc;
tracing padrão. §§27–34 e gate ARC/A0 continuam incompletos/reprovados.

Acessos a campos pelo módulo agora são resolvidos no mesmo ponto fixo de
aliases/arestas, com LerPorEsquema/GravarPorEsquema. Cada receiver usa seu
campo nominal; uniões de layouts não confundem índices iguais. Elimina
a inferência preliminar de layout único que perdia leituras encadeadas.
Escritas permanecem fracas. Topo, esquema ausente e índice inválido tornam
a fronteira opaca e invalidam campos conhecidos; receiver vazio só é fechado
após convergir a fila, sem confundir bottom intermediário com prova negativa.
Verificador independente cobre seleção, inclusões e opacidade. Regressões
conferem a.filho.proximo após escrita pelo alias, layouts distintos,
aliases tardios, ordem invertida, budget, esquema/índice inválidos e
adulteração de arestas/opacidade. Oráculo de caminhadas de comprimento exato
confere sete leituras em 343 grafos de três nós com saída em todos os nós,
nas duas ordens de restrições (686 soluções); fronteiras vazias têm testes
separados. Passaram 322 testes unitários do emissor incluindo ignorados,
24 integrações, 97 exemplos e um compile-fail. Rustfmt dos três módulos
e git diff --check passaram. Logs target/arc-campos-esquemas-{focado,suite}.log.
Esquemas/inicializadores são premissas locais guardadas pelo módulo; seleção
não prova não nulabilidade, vida, observadores/SDK ou política. Contexto
implícito do emissor continua opaco. ARC apenas --memory=arc; tracing padrão.
Sem nova medição/gate: ARC/A0=2,000274 anterior continua reprovado; §§27–34
incompletos. Pendências de Clippy/documentação da etapa anterior permanecem.
CI 38064260616: Linux/mensagens passaram; Windows/macOS ainda executam.

Resumos de campos/publicações da §28.3 passam a compor cobertura por
chamadas diretas locais. Worklist separada dos aliases normais parte de
folhas cobertas e só promove o chamador quando todos os callees preservam
campos/publicam somente pelo resultado e a ABI é exata. Guarda de contexto
impede reaprovar prólogos/raízes/tabelas opacos; recursão sem base de
cobertura, externos, throws, stores e conversões continuam desconhecidos.
Regressões conferem cadeia de 1.000 funções nas duas ordens, recursão,
dependência externa/falível, ABI, tabela ausente/conferência de pilha e
consumo no módulo. Alterar um callee transitivo para materializar texto
invalida a análise antiga e torna a chamada opaca na reanálise.
Passaram 318 testes unitários do emissor incluindo ignorados, 24 integrações,
97 exemplos e um compile-fail; rustfmt dos dois módulos e git diff --check
passaram. Logs target/arc-cobertura-composta-{focado,suite}.log.
Clippy com -D warnings encontrou erros em frontend e runtime/build.rs,
arquivos não alterados nesta etapa; não afirmar lint completo aprovado.
Documentação com RUSTDOCFLAGS=-D warnings falhou em 15 links públicos para
itens privados, também fora dos dois módulos alterados. Logs
target/arc-cobertura-composta-{clippy,doc}.log; ambos terminaram em 101.
Não resume escritas por campo/SDK, observadores ou vidas nem seleciona
políticas/remove contagens. Análise pelo módulo só em ARC; tracing padrão,
ARC apenas --memory=arc. §§27–34 e gate completo continuam incompletos.
CI 38064260616: Linux/mensagens passaram; Windows/macOS ainda executam.

Comparação do trial deletion denso concluída: antes b706e0b0, depois
687a7fd4; diferença de produção somente em arc.rs, benchmark inalterado.
CLI release recompilada com jit; novo AOT otimizado com --memory=arc.
Sete pares alternados, afinidade 0x4, ARC puro, 256 MiB, sem rastro,
auditoria ou stress. Passaram 14 processos com resultados iguais e hashes
inalterados. Auditoria reconstrói medianas do stdout sem aquecimento.
Árvores 633,281 → 643,327 ms (1,59% mais lento); lista ligada
156,701 → 149,509 ms (4,59% mais rápido). Depois menor em 2/7 pares de
árvores e 4/7 de lista; resultado misto, sem ganho geral demonstrado ou
significância estabelecida. Não comparar tempos de rodadas diferentes como
pares. A0/outros 30 kernels não reexecutados: ARC/A0=2,000274 anterior
continua reprovado. Dados/scripts/logs/hashes em
bench/results/2026-10-10-arc-trial-denso-windows. CLI SHA-256
4daefc7bc235f40bd72a0b0aeabff2f49881f313b8615554c37372e632d7f418.
ARC somente --memory=arc; tracing padrão. §§27–34 continuam incompletos.
Commits até 687a7fd4 publicados em main após CI 38057171425 verde.
Nova CI 38064260616 em execução; mensagens passaram. Evidência ainda local
para preservar a rodada de CI ativa.

Trial deletion em `EstadoDoArc::coletar_ciclos` passa a usar um mapa
handle→índice e vetores de contadores/marcas, em vez de três tabelas de hash
para a mesma região. Mantém multiplicidade, entradas externas, proteção de
raízes/construção, gerações e validação de trial negativo antes do descarte.
Regressões cobrem handles esparsos com arestas repetidas e inventário
adulterado, sem alterar RC, romper arestas ou publicar mortos no erro.
Passaram 190 testes unitários release do runtime (incluindo ignorados),
16 integrações e 42 exemplos públicos. Rodada ampliada com
DF_ARC_SEEDS=1000 passou nos três oráculos de grafos aleatórios.
Logs target/arc-trial-denso-{suite,oraculo}.log. Sem medição de desempenho
dessa mudança; o agregado ARC/A0=2,000274 anterior continua reprovado.
ARC somente com --memory=arc; tracing permanece padrão. Integrar passes
ao fluxo normal significa executá-los quando ARC for explicitamente
selecionado. C++ serve às fixtures de exceções estrangeiras; compilador e
runtime são Rust. §§27–34 continuam incompletos. CI 38057171425 terminou
com sucesso em Windows, Linux, macOS arm64 e mensagens de commit.

Comparação local da transferência de mortos concluída: antes 55beeb34,
depois b706e0b0, diferença de produção restrita a arc.rs/heap.rs. CLI release
recompilada; novo AOT --optimize com --memory=arc. Sete pares alternados,
afinidade 0x4, sem rastro/auditoria/stress, ARC puro e heap 256 MiB.
14 processos com resultados iguais; hashes dos dois executáveis e CLI
inalterados. Auditoria posterior confere cobertura/ordem e reconstrói medianas
do stdout, descartando aquecimento. Árvores 676,749 → 654,343 ms (3,31% menor),
lista ligada 129,341 → 124,507 ms (3,74% menor), nas medianas desta amostra.
Depois menor em 6/7 pares de árvores e 5/7 de lista; há pares mais lentos.
Não afirmar ganho uniforme/significância ou extrapolar para o gate completo:
A0 e os outros 30 kernels não foram reexecutados. ARC/A0=2,000274 permanece
o agregado anterior reprovado. Dados/manifesto/comparador em
bench/results/2026-10-10-arc-transferencia-mortos-windows.
CLI nova SHA-256 3a3c71197e2cc6cf1e2434f43d9a76ed1e5d1051f6a4d5a774f4042d6b2dabc3.
ARC só --memory=arc; tracing padrão. §§27–34 continuam incompletos.
CI 38057171425: Windows/Linux/mensagens passaram; macOS continua nos testes
JIT com ignorados. Mudança local para preservar a rodada publicada.

Perfil diagnóstico de objetos_escapam-ARC da revisão 55beeb34, com rastro,
afinidade 0x4 e resultados iguais, inclui árvores/lista e aquecimentos:
382 drenagens, 3.182.096 µs acumulados. Zeros iniciais 35,67%, ciclos mais
zeros posteriores 22,29%, mortos/tabelas laterais 7,33%. Não é nova medição
do gate nem tempo exclusivo de árvores. Registro em
bench/results/2026-10-10-perfil-arvores-lista-55beeb34-windows.
Inspeção remove cópia da lista inteira de mortos no heap: transferir_mortos
remove os mesmos metadados e troca o buffer com o destino vazio; rodadas
seguintes acrescentam handles. Jovens continuam na varredura e ainda exigem
limpeza lateral na primeira rodada. API tomar_mortos conserva seu resultado.
Regressão confere cascata, identidade do primeiro buffer (sem cópia), entrega
incremental, rodada vazia e remoção dos metadados. Passaram teste focado,
188 testes unitários release do runtime (incluindo os ignorados), 16 integrações
e 42 exemplos públicos; logs target/arc-transferencia-mortos-{focado,suite}.log.
Inclui fonte única AOT/JIT. git diff --check passou antes dos artefatos brutos.
Sem comparação de tempo antes/depois da transferência; ARC/A0=2,000274 pertence
ao produtor anterior 55beeb34. Gate reprovado; §§27–34 incompletos. ARC apenas
--memory=arc, tracing padrão. CI 38057171425: Windows/Linux/mensagens passaram;
macOS chegou aos testes JIT com ignorados. Mudança local sem interromper rodada.

Medição ARC/A0 atualizada para 55beeb34: CLI release recompilada com LLVM
embutido, 18 executáveis novos, nove programas/32 kernels, sete pares alternados
em afinidade Windows 0x4. Passaram 126 execuções com resultados iguais entre
modos/repetições e aos resultados Dart AOT anteriores. Fontes do benchmark
conferidas contra 5bbfc80f, sem alterações; Dart não foi reexecutado.
Auditoria independente confere cobertura/alternância, reconstrói medianas do
stdout sem aquecimento, compara resultados e recalcula média geométrica:
ARC/A0 = 2,000274042309175. Gate ARC≥A0 continua reprovado; árvores 12,82×,
lista ligada 5,28× e mapa str→objeto 4,88× são os maiores custos relativos.
Não atribuir agregado/diferenças à instrumentação condicional isoladamente.
Rastro/auditoria/GC off desligados, ARC puro, heap 256 MiB, stress e opção de
ciclos ausentes. CLI SHA-256
2A63E6782B507291A3277FBCB00B0307DEA0CDB10AB7184BE33125E8FB7239B5.
Evidência em bench/results/2026-10-10-modos-55beeb34-windows. Captura inicial
de hashes ocorreu após duas execuções, no começo da fase de medição; os 18
hashes permaneceram iguais no fechamento. Ponto de captura explícito no
manifesto; não alegar captura anterior à primeira execução. ARC somente com
parâmetro explícito --memory=arc; tracing padrão. §§27–34 ainda incompletos.
CI 38057171425: Windows/Linux/mensagens passaram; macOS continua nos testes
emit_native com ignorados. Mudança local sem interromper essa rodada.

Saídas do Tarjan compartilhado passam por conferência independente em
optimize/scc.rs::conferir: cobertura única dos nós, alcance interno direto/
inverso e ordem estrita das arestas externas. Não executa Tarjan novamente;
recusa componente vazio, nó omitido/duplicado/inexistente, aresta fora do
grafo, SCC dividida, fusão sem conexão forte ou dependência invertida.
Guarda ocorre antes de devolver qualquer partição, incluindo grafos de heap,
chamadas e efeitos. Tempo/espaço O(V + E), sem recursão; marcas por direção
evitam limpar V posições para cada componente. Precondição de destino válido
do Tarjan permanece conferida antes de construir a partição.
Oráculo Floyd agora confere 27 atribuições × 512 grafos (13.824 candidatas),
além dos casos adulterados e cadeia/ciclo de 100 mil nós. Passaram cinco
testes focados antes da ampliação do oráculo, depois 307 testes do emissor
(sete ignorados), 97 exemplos públicos e um exemplo de rejeição na compilação.
Logs target/arc-verificador-scc-{focado,suite,doc}.log. Rustfmt e git diff --check
passaram; aviso preexistente de atribuir_slots não usado. Não verifica produtor
de arestas, ciclo concreto, vida ou certificado de política; não conclui
§§27–34 nem aprova gate ARC/A0. ARC só --memory=arc; tracing padrão.
CI 38057171425: Windows, Linux e mensagens passaram; macOS está nos testes
emit_native com ignorados. Alteração local para preservar essa rodada.

Preparação compartilhada de tipos/raízes validada também em AOT real:
python scripts/prove-arc-aot.py arc_source_return --debug passou as
16 execuções ARC/tracing × O0/O2 da revisão 5cb4e56d, sem mudanças nos fontes.
Quatro positivas e 12 controles por trap conferem sobrevivência/morte do Mint
na cadeia identidade(identidade(valor)); retirar retain, drop do retorno ou
drop intermediário reprova a prova. Auditoria, ARC puro e GC stress ligados;
compilação malsucedida ou saída inesperada não contam como negativo aprovado.
Evidência congelada em
bench/results/2026-10-10-raizes-retorno-cadeia-windows: 96 arquivos brutos,
manifesto com comandos/versões/hashes e três logs (AOT, 306 testes, 98 exemplos).
Artefatos anteriores de 12 casos permanecem preservados. Gerador Rust debug,
Clang 22.1.8, SDK 3.6.2, executáveis O0/O2; não é medição de desempenho nem
certificado global. Gate ARC/A0 reprovado; §§27–34 continuam incompletos.
ARC só --memory=arc; tracing padrão. CI 38057171425 ainda ativa no
Windows/macOS, Linux/mensagens passaram. Nenhum push para interromper a rodada.

Contexto implícito por raízes entra na análise ARC e na guarda de cobertura
dos resumos locais. Preparação de tipos/apontados e conversões de Phi foi
extraída para llvm/contexto_arc.rs e compartilhada com a emissão, conservando
a ordem de preparação. Consulta executa a mesma análise de raízes/classificação
de coleta antes da redução por mapas: slots da pilha-sombra conservam a
possibilidade de prólogo tanto no fallback por orçamento quanto na conferência.
Regressão demonstra chamada coberta à folha identidade com Ref viva: nenhuma
instrução opaca/contexto explícito, mas quadro e pilha.estouro reais no LLVM;
escape agora conserva EmissaoImplicita topo. Alloca Ref também exige quadro
mesmo sem SSA vivo; alloca escalar e identidade sem chamada são controles.
Passaram teste focado, 306 testes do emissor (sete ignorados), 97 exemplos
públicos e um exemplo de rejeição na compilação; logs
target/arc-contexto-raizes-{focado,suite,doc}.log. Rustfmt dos arquivos novos/
de análise e git diff --check passaram; llvm/mod.rs recebeu apenas extração
local, sem reformatação geral. Aviso preexistente de atribuir_slots não usado.
Não há nova prova AOT ou desempenho. Instrumentação, SDK/observadores, vidas
e certificados continuam pendentes; não conclui §§27–34. Gate ARC/A0 reprovado.
ARC só --memory=arc; tracing padrão. CI 38057171425: Linux/mensagens passaram;
Windows/macOS continuam ativos. Mudança local para preservar essa rodada.

Análise nova registra fronteira de emissão implícita no próprio corpo:
contexto explícito insere conferência de pilha antes da HIR, com possível
StackOverflowError; tabela ausente no modo tabelas ou caminhos de
invocações/pousos/saídas/cleanup sem resumo também conservam opacidade.
Escape publica topo sob CausaEscape::EmissaoImplicita, sem fabricar ValueId;
grafo mantém arestas/SCCs conhecidas, mas registra opacidade do prólogo.
Regressão com corpo vazio compara análise e LLVM gerado (pilha.estouro),
inclui controle sem conferência, tabela ausente e tracing sem análise.
Os 512 grafos HIR continuam conferindo arestas/SCCs contra Floyd; agora
não confundem precisão desses campos com cobertura do prólogo com contexto.
Passaram o teste focado, 304 testes do emissor (sete ignorados), 97 exemplos
públicos e um exemplo de rejeição na compilação; logs
target/arc-emissao-implicita-{focado,suite,doc}.log. Rustfmt dos quatro arquivos
e git diff --check passaram; aviso preexistente de atribuir_slots não usado.
Quadros por vivacidade, instrumentação, SDK/observadores e vidas continuam
pendentes; não certifica política ou conclui §§27–34. Gate ARC/A0 não aprovado.
ARC apenas --memory=arc; tracing padrão. CI 38057171425: Linux/mensagens
passaram; Windows está nos testes ignorados, macOS no cleanup de laços AOT.
Mudança local para preservar a rodada publicada.

Guarda das tabelas de emissão inclui agora o próprio chamador, mesmo sem
chamadas locais: mudar confere_pilha, saídas ou cleanup invalida o consumo
da análise anterior. Tabelas são fotografadas e comparadas por conteúdo,
incluindo mapas/conjuntos, em vez de hash da representação Debug. Dependência
acompanha o símbolo após reordenar funções/tabelas; alteração só numa função
fora das dependências não invalida o chamador. Regressão cobre mudança/restauro
da tabela própria, reordenação e troca do modo de exceções.
Passaram 303 testes do emissor (sete ignorados), 97 exemplos públicos e um
exemplo de rejeição na compilação. Logs target/arc-tabela-chamador-{suite,doc}.log;
há aviso preexistente de atribuir_slots não usado em llvm/raizes.rs.
Não certifica cobertura de todos os efeitos implícitos nem vida/observadores;
não remove contagens ou aprova o gate ARC/A0. ARC só com --memory=arc;
tracing permanece padrão. CI 38057171425: Linux/mensagens passaram,
Windows/macOS ainda ativos. Mudança local para preservar essa rodada.

Verificador independente da solução de inclusão em analise/points_to.rs
confere cada restrição sem executar novamente a worklist: domínio/budgets,
sementes, desconhecimento, cópias, materialização/inclusão de stores e leituras.
Receiver topo em store exige opacidade global do campo, inclusive para nós
materializados depois. Solução conservadora maior que o menor ponto fixo é
aceita; conjuntos truncados, resultados de leitura apagados, campo omitido e
marcador opaco removido são rejeitados. Toda saída de resolver passa por essa
conferência, incluindo os domínios HIR e resumos paramétricos que o consomem.
Verificação não prova que o produtor cobriu toda a HIR/SDK, inicializações,
observadores ou vida completa: campo ausente é bottom só nas restrições,
nunca certificado de ausência no heap. Certificado de política permanece pendente.
Passaram oito testes focados (incluindo os 512 grafos de inclusão e cadeia
profunda), 302 testes do emissor (sete ignorados), 97 exemplos públicos e um
exemplo de rejeição na compilação. Logs
target/arc-verificador-inclusao-{focado,suite,doc}.log. Rustfmt dos dois arquivos
e git diff --check passaram. Não conclui §§27–34, não remove contagens nem
aprova o gate ARC/A0. ARC só com --memory=arc; tracing padrão.
CI 38057171425: Linux/mensagens passaram; Windows/macOS ainda executam.
Mudança local para preservar a rodada da revisão publicada b0ee6645.

Extração HIR deixa de tratar materialização/conversão como transferência pura.
Constantes String/StringWtf8 registram operação opaca e invalidam campos,
pois o caminho JIT pode materializar texto. Phi não exato também perde precisão
do resultado e dos campos; Phi Ref com texto literal já é rejeitado pelo SSA,
enquanto Phi I64 com esse operando exige fronteira opaca. Aritmética só dispensa
efeitos com entradas/saída exatas por operação, sem concluir pureza pela largura
do resultado. Divisão/resto, DoubleToInt e CheckNotNull conservam efeitos
opacos até cobertura própria de erro; CheckNotNull ainda transfere aliases
normais separadamente dos efeitos. Não é mudança na emissão LLVM.
Regressões conferem texto em constante, operando Add e Phi, Add com entrada
Ref, divisão por zero e assert de null; em todos a leitura posterior de campo
fica desconhecida e a publicação opaca permanece topo. Controles positivos
conservam soma escalar exata e Phi de referências já avaliadas.
Passaram 36 testes focados, 299 testes do emissor (sete ignorados), 96 exemplos
públicos e um exemplo de rejeição na compilação. Logs
target/arc-conversoes-opacas-{focado,suite,doc}.log. Rustfmt de hir.rs e
git diff --check passaram. Cobertura global/implícita, verificador independente,
observadores, vidas e políticas continuam pendentes; não conclui §§27–34 nem
aprova o gate ARC/A0. ARC só com --memory=arc; tracing permanece padrão.
CI 38057171425: Linux e mensagens passaram; Windows/macOS continuam ativos.
Alteração local para preservar a rodada da revisão publicada b0ee6645.

GrafoHeapArc local em optimize/arc/analise/grafo.rs extrai arestas de campos
do points-to HIR e executa Tarjan sobre sítios/contextos de objetos, sem
reutilizar SCC de chamadas como SCC de heap. Fotografia imutável expõe arestas,
componentes e fronteiras opacas; campo/esquema ausente, valor sem origem
fechada, operação não coberta e budget não viram grafo fechado/vazio conhecido.
Componente unitário é cíclico só com autoaresta. Oráculo Floyd-Warshall confere
arestas, partição e ciclo nos 512 grafos HIR de três objetos. Regressões cobrem
parâmetro opaco sem arestas materializadas, falta de layout, orçamento 0/1,
tracing sem análise, recursão sem fabricar autoaresta de objeto e duas instâncias
resumidas no mesmo sítio produzindo autoaresta abstrata conservadora.
Passaram 297 testes do emissor (sete ignorados), 96 exemplos públicos e um
exemplo de rejeição na compilação. Três testes focados iniciais passaram antes
das duas regressões finais; logs target/arc-grafo-sitios-{focado,suite,doc}.log.
Rustfmt do novo arquivo e git diff --check passaram.
Fotografia sem ciclo/opacidade não certifica toda a vida/continuações; não
emite NaoParticipa/AciclicoProvado, não forma grupo concreto e não seleciona
política. Supergrafo de tipos/layouts completos, contexto/fluxo/forma,
observadores, fronteiras globais e certificados continuam pendentes.
Não conclui §29.1/S2 ou §§27–34; gate ARC/A0 segue não aprovado. ARC permanece
exclusivo de --memory=arc, tracing padrão. CI 38057171425 ainda executa nos
três alvos; mensagens passaram. Mudança local, sem cancelar a rodada publicada.

PublicacoesHeapArc separa publicação somente pelo resultado de cobertura
desconhecida. Extrator exige inspeção positiva de todo o corpo de folha de
valores já avaliados, sem stores/chamadas/alocações/throw; não deduz a
propriedade do alias normal ou Borrowed. Consumo exige ABI exata e cobertura
dos caminhos implícitos de emissão; contexto/tabela com confere_pilha invalida
tanto campos quanto publicação. Conversões de argumento e texto seguem opacos.
Chamada coberta não acrescenta OperacaoOpaca: resultado devolvido conserva
seu alcance, e resultado descartado não publica o argumento por essa chamada.
Regressão confere retorno do filho, descarte da identidade, StoreGlobal que
mantém alias mas publica, ArcCopy fora da cobertura e guarda implícita opaca.
Não propaga essa prova a chamadas encadeadas/recursivas enquanto faltar
contrato completo de SDK/caminhos implícitos. Passaram 29 testes focados,
292 testes do emissor (sete ignorados), 91 exemplos públicos e um exemplo
de rejeição na compilação. Logs target/arc-publicacao-folha-{focado,suite,doc}.log.
Rustfmt dos quatro arquivos de análise e git diff --check passaram.
Retenções interprocedurais, observadores, vidas, certificados e seleção de
políticas continuam pendentes; nenhum retain/release foi eliminado. Não
conclui §28.3/S1 ou §§27–34; gate ARC/A0 segue não aprovado. ARC permanece
exclusivo de --memory=arc; tracing padrão. CI 38057171425 ainda em execução
nos três alvos; mensagens passaram. Alteração local, sem interromper a rodada.

AnaliseHir agora expõe points-to e operações opacas apenas por consultas
imutáveis, sem permitir substituir a solução ou apagar evidência de efeitos.
Doctest compile_fail confere que desconhecidos().clear() não é permitido.
Solução produzida pelo módulo guarda também modo de memória, sítios nominais
do chamador e layouts locais; consumo de escape exige o módulo mesmo quando
não há chamadas. Mudança Ref→I64 no layout, troca da origem nominal ou mudança
para tracing invalida consumo sem alterar o corpo HIR. Premissas são ordenadas
nominalmente: reconstruir HashMaps em ordem diferente não causa invalidação.
Guarda de layouts inclui conservadoramente todos os layouts do módulo; não
é cache SDK, versão de pins ou certificado independente. Fatos fornecidos
manualmente à entrada analisar continuam premissas explícitas, não certificados.
Passaram 292 testes do emissor (sete ignorados), 89 exemplos públicos e um
exemplo de rejeição na compilação. Logs target/arc-premissas-modulo-{suite,doc}.log;
29 testes focados iniciais também passaram antes de ampliar a regressão de
premissas. Rustfmt dos três arquivos de análise e git diff --check passaram.
Verificador independente, exportação SDK, vidas/observadores, seleção de
política e §§27–34 completos permanecem pendentes. Não altera ativação ARC:
somente --memory=arc; tracing permanece padrão. Gate ARC/A0 não aprovado.
CI 38057171425 da revisão publicada b0ee6645 continua em execução nos três
alvos; mensagens passaram. Esta alteração permanece local para preservar a
rodada em execução. Não considerar a CI ou o objetivo integral concluídos.

ResumoHeapArc agora separa aliases normais de CamposHeapArc, com cobertura
positiva inicial para preservação de campos: constantes primitivas/null,
Phis e saídas de representação exata, sem chamada/store/alocação/throw.
Análise HIR conserva arestas dos campos através de folha coberta com ABI
exata, inclusive retorno Void, sem deduzir noescape ou apagar retenções.
Conversão de argumento (I64→Ref) e materialização de texto continuam opacas,
pois podem coletar/reentrar antes do corpo. Texto constante dentro da folha,
ArcCopy, stores e saídas excepcionais também ficam fora da cobertura inicial.
Resumo do módulo confere o contexto exigido pelo backend e a tabela de
emissão: confere_pilha pode construir erro SDK sem instrução HIR explícita.
Modo/tabelas dos callees transitivos passam a integrar a guarda local do
escape; alteração de tabela invalida análise mesmo sem mudança de corpo.
Índices dos corpos são consultados por mapa, sem busca linear por dependência.
Regressões conferem filho preservado através de identidade, retorno Void,
campo opaco após escrita, conversões, texto e mudança de confere_pilha.
Fonte/SDK reais confirmam cobertura do método identidade. Passaram 29 testes
focados, 292 testes do emissor (sete ignorados) e 87 exemplos públicos.
Logs target/arc-campos-preservados-{focado,suite,doc}.log. Rustfmt dos cinco
arquivos de análise e git diff --check passaram. Check Clippy integral ainda
não aprovado pela limitação preexistente registrada abaixo.
Escritas por caminho, cobertura de chamadas/callbacks/SDK, escape/retenção,
observadores e certificados permanecem pendentes. Não conclui §28.3/S1 ou
§§27–34. ARC só com --memory=arc, tracing padrão; gate ARC/A0 segue reprovado.

CI 38051323600 terminou cancelada: Linux e mensagens passaram; Windows
excedeu 60 min e macOS excedeu 90 min (check 114210866774). macOS aprovou todas
as matrizes AOT e testes emit_native com ignorados (266 unitários, 22 de
integração, dois adicionais e 55 exemplos), mas foi interrompido ainda na
compilação/ligação dos testes JIT, sem iniciar essa bateria. Não inferir
aprovação do JIT nem da rodada integral. Log target/ci-macos-37497527.log.
Sem toolchain 38051323611 terminou aprovada.
Prazo da matriz macOS ampliado de 90 para 150 min para concluir a compilação
fria JIT e as verificações restantes; Linux permanece com 90 min, Windows
com a ampliação anterior para 90 min. YAML conferido; não é aprovação de CI.

Aliases de retorno normal agora convergem nas SCCs de chamadas diretas locais
em optimize/arc/analise/chamadas.rs. Alcance dos corpos é descoberto sem
recursão Rust; Tarjan ordena dependências e worklist monotônica reaplica
chamadores da SCC quando aliases/null crescem. Substituição usa argumentos
reais, inclusive permutações na recursão. Bottom vazio sem null é interno;
SCC sem evidência de retorno normal é elevada a desconhecido antes do consumo.
União de retornos de blocos diferentes agora também respeita orçamento:
oráculo encontrou excesso antes da correção; não truncar conjunto de aliases.
Análise do módulo guarda todos os corpos transitivos consultados, sem cache
persistente: mudança no fim de caller→ponte→id invalida o escape do caller.
Operações continuam opacas para efeitos/escape; retornar argumento não prova
que seus campos ou publicações ficaram intactos. Ausência de corpo externo
mantém resultado desconhecido. Oráculo Floyd-Warshall confere todos os 512
grafos de três funções em três budgets; testes cobrem recursão com permutação,
null, SCC sem base, externo e cadeia de 2.000 funções em ambas as ordens.
Passaram 27 testes focados, 290 testes do emissor (sete ignorados) e 85 exemplos
públicos. Logs target/arc-resumos-scc-{focado,suite,doc}.log. Rustfmt dos três
arquivos de análise alterados e git diff --check passaram. Clippy do workspace
não aprovado: limitação preexistente registrada abaixo permanece.
Ainda pendentes: efeitos/observadores, campos/globais/frescor, alvos dinâmicos
RTA, vidas, resumos SDK e versões de layouts/pins/gerações JIT. Não conclui
§28.3/S1 ou §§27–34; entrada continua explícita, sem seleção de política ou
eliminação de contagens. ARC só com --memory=arc; tracing permanece padrão.
Gate ARC/A0 segue não aprovado. CI 38051323600: Linux passou, Windows cancelado
por prazo antigo; macOS continua nos testes JIT com ignorados. Mudança local.

Análise HIR consome aliases de retorno normal de CallStatic com corpo local
disponível no módulo. Substitui parâmetros pelos argumentos reais, sem
fabricar objeto fresco; retorno só null conserva conjunto vazio. Aridade e
representação Ref são conferidas. Chamadas continuam opacas para escrita,
retenção e efeitos excepcionais: campos conhecidos perdem precisão e escape
da operação permanece topo, sem deduzir noescape do alias normal.
Dependências dos corpos usados ficam registradas; calcular_no_modulo confere
presença e assinatura local de cada callee antes de consumir escape.
Consumo sem módulo é recusado quando há dependências. Regressão altera o
callee para null, remove o corpo e confere invalidação/reanálise conservadora.
Fixture com frontend/SDK reais também extrai alias de um método identidade.
Regressão focada adicional cobre callee que grava autoaresta em campo antes
de devolver o argumento: alias preservado, leitura do campo permanece topo.
Passaram 286 testes do emissor (sete ignorados), um teste focado da fonte e
85 exemplos públicos. Logs target/arc-chamadas-resumos-{suite,fonte,doc}.log.
Teste final de mutação passou em target/arc-chamadas-resumos-mutacao.log.
Clippy --locked -p dartforge-emit-native --all-targets -- -D warnings parou
antes da análise do emissor por collapsible_if preexistente em
crates/runtime/build.rs:231; log target/arc-chamadas-resumos-clippy.log.
Não considerar esse check aprovado nem equivalente ao gate do workspace.
Chamadas encadeadas/SCC recursivas, efeitos de campos/escape/observadores,
resumos SDK e dependências de layouts/pins/gerações JIT continuam pendentes.
Guarda de corpo é local, não formato de exportação versionada. Entrada de
análise permanece explícita e só no módulo ARC; não seleciona política nem
remove contagens. ARC só com --memory=arc; tracing permanece padrão.
Não conclui S0/S1, §§27–34 ou o gate de desempenho ARC/A0.

ResumoHeapArc inicial em optimize/arc/analise/resumos.rs separa retorno
normal paramétrico dos resumos de alcance/efeitos.tsv. Extrator sobre SSA
propaga placeholders de parâmetros Ref por Copy/Move/Phi/bitcast Ref→Ref;
outros produtores, representações opacas e budget viram desconhecidos.
Null é propagado por worklist de nulabilidade sem criar objeto abstrato.
Marca nulo indica literal adicional; aliases de parâmetros ainda podem ser
null mesmo com essa marca falsa. Não inferir não nulabilidade dos argumentos.
Instanciação substitui placeholders pelos conjuntos reais do chamador,
preservando aliases existentes em vez de criar objetos frescos. Confere
assinatura local do corpo/aridade; orçamento menor e argumento opaco
conservam topo. Resumo não prova noescape/noheapmutation, não elimina
retenções nem efeitos normais/excepcionais e não muda ownership da ABI.
Passaram 284 testes do emissor (sete ignorados), quatro focados finais
incluindo união Phi/parametros/null e instanciação, e 84 exemplos públicos.
Logs target/arc-resumo-alias-{suite,focado,doc}.log. Consumo automático nas
chamadas HIR, SCC recursivas, resumos de campos/escape/observadores,
dependências SDK/JIT e exportação versionada ainda pendentes. ARC continua
exclusivo de --memory=arc; esta etapa não conclui S0/S1 ou o gate ARC/A0.

CI 38051323600: Windows cancelado ao exceder limite do job de 1h,
conforme anotação do check 114210866777. Não houve falha de teste nesse
ponto: matrizes nativas concluídas, interrupção durante ng_transparencia
no passo dos ignorados. Log target/ci-windows-37497527.log. Linux passou;
macOS ainda ativo. Não considerar rodada integralmente aprovada.
Limite Windows ampliado para 90 minutos, como nos jobs Unix, para permitir
conclusão dos testes ignorados após compilação/matrizes. Alteração local,
sem nova rodada enquanto macOS da revisão publicada continua ativo.

Análise local de escape em optimize/arc/analise/escape.rs agrupa publicações
por retorno, exceção, global e operação opaca. Percorre transitivamente os
campos do esquema de cada nó sem recursão, preservando ciclos/aliases.
Esquema/campo ausente, campo opaco e budget excedido propagam topo;
operação sem resumo conserva alcance desconhecido mesmo sem argumentos,
pois pode acessar globais/callbacks. Borrowed na ABI não fornece noescape.
Solução HIR conserva esquema e assinatura local do corpo; consumo de
escape após alteração do corpo é recusado. Hash é guarda local do objeto
HIR, não formato canônico de resumo SDK nem dependência de geração JIT.
Oráculo independente confere retorno/descendentes nos 512 grafos de três
nós; regressões cobrem payload de exceção, corpo obsoleto, opacidade,
budget, fatos ausentes e publicação global sem retorno do objeto.
StoreGlobal ainda opaco ao domínio de efeitos, sem inventar precisão
transitiva antes de um contrato específico. Passaram 280 testes do
emissor (sete ignorados), 18 testes focados finais incluindo a regressão
global acrescentada, e 78 exemplos públicos. Logs
target/arc-escape-transitivo-{suite,focado,doc}.log.
Não certifica confinamento/região ou cobertura global: observadores,
resumos interprocedurais, callbacks/async, vidas, esquemas/versionamento e
certificados continuam pendentes. Nenhuma política ou ativação ARC foi
alterada; somente --memory=arc seleciona ARC. CI 38051323600 continua
ativa: Linux passou, macOS executa testes do emissor com ignorados e
Windows chegou aos ignorados após as matrizes nativas.

Entrada analisar_no_modulo liga sítios/layouts do lowering à análise local
sem fatos de objeto escritos manualmente. Reconhece AllocObject e fábrica
reservada dartforge_object_new com ABI, classe e contagem constantes; esquema
deve corresponder ao layout registrado. Fábrica zerada inicializa os campos
sem arestas; construtor/callback não é tratado como parte dessa primitiva.
Passo preliminar resolve origens/cópias/Phis e só registra esquema de acesso
quando todos os nós possíveis do receiver pertencem à mesma classe/layout
com índice válido. Receiver opaco/multilayout continua desconhecido.
Sítio obsoleto, origem com classes conflitantes ou quantidade incompatível
são erros. Falta de origem/layout e ABI não reconhecida conservam topo;
não há inferência de sítio a partir do ValueId. Tracing devolve None antes
de executar a análise. Chaves de layout valem só nesta versão do módulo.
Teste HIR preserva autoaresta criada por store, perde precisão sem layout
e recusa quantidade incompatível. Teste com frontend/SDK reais confere nó
da alocação em main e igualdade com o sítio registrado pelo lowering,
sem plano manual de objeto. Passaram 277 testes do emissor (sete ignorados),
o teste focado da fonte e 75 exemplos públicos. Logs
target/arc-points-to-modulo-{suite,fonte,doc}.log.
Entrada ainda explícita, sem certificados/seleção de política: chamadas,
efeitos transitivos, escape/observadores, resumos SDK, contextos e dependências
de versão permanecem pendentes; não conclui S0/S1 nem o gate ARC/A0.
ARC continua exclusivo de --memory=arc. CI 38051323600 permanece ativa:
Linux passou; macOS chegou a unwind estrangeiro e Windows a keepalive pending.

Lowering ARC registra origens nominais de alocações reconhecidas em
Module::sitios_arc, por símbolo e índice SSA atual. Origem é ordinal da
alocação original, não ValueId; registro adicional conserva identidades
anteriores e atribui novos ordinais a corpos sintéticos/alocações novos.
Inclui variantes Alloc*, Box/JuntarTextos e fábricas object_new conhecidas;
operações opacas sem contrato não recebem origem fabricada.
Junção paralela e poda de fonte transportam/removem fatos. Inliner remapeia
índices SSA de cópias conservando SitioArc, com atualização por função nos
workers. Contextos ainda unidos conservadoramente; cópia não prova singleton.
Poda antes de cada rodada evita que ID reutilizado herde uma origem morta;
poda final remove alocações eliminadas por limpeza/substituição escalar.
Registro usa a seleção do Context, inclusive nos módulos parciais, e tracing
não recebe esses fatos. Corpos sintéticos finais também são registrados.
Teste da fonte confere origens das alocações antes dos passes e ausência
no tracing. Regressão do inliner confere duas cópias da mesma origem com
IDs distintos, poda, otimização/idempotência e ativação explícita.
Passaram 276 testes do emissor (sete ignorados), dois testes focados,
74 exemplos públicos e a matriz AOT da cadeia da fonte com 16 casos no
Windows. Logs target/arc-sitios-origem-{suite,focado,doc,aot}.log.
Identidades ainda não são dependências versionadas de recarga; contexto,
instância genérica, esquema semântico/capturas e exportação SDK permanecem
pendentes. Extração points-to ainda recebe fatos nominais explicitamente;
esta etapa não conclui S0/S1 ou o gate e não torna ARC padrão.

Extração HIR inicial em optimize/arc/analise/hir.rs valida SSA e consome
fatos explícitos de origem/esquema de AllocObject e GetField/SetField.
Produz restrições para campos iniciais, stores inclusivos, leituras,
ArcCopy/ArcMove, CheckNotNull e Phis. IDs locais SSA só indexam variáveis;
identidade dos nós vem da origem nominal fornecida, nunca de ValueId.
Fatos obsoletos, número de campos ou índice incompatível são recusados.
Parâmetros Ref/I64/Ptr são topo sem resumo; bitcast I64→Ref, operações
não cobertas e valores inteiros em campos sem esquema de representação
não provam trivialidade. Calls/operações sem contrato preciso tornam
opacos os campos nominais fornecidos, inclusive após outras escritas.
Regressões HIR preservam ciclo/identidade de inicializadores e stores,
conservam desconhecimento de chamadas/origens ausentes e recusam inferir
ausência de handle pela largura. Passaram 274 testes do emissor (sete
ignorados) antes do endurecimento de constantes/representações, os 12
testes focados finais do domínio/solver/extração e 74 exemplos públicos.
Logs target/arc-points-to-hir-{suite,focado,doc}.log.
Produtor automático de origens/layouts, contratos SDK, escape/observadores,
dependências de versão e integração no pipeline continuam pendentes.
Esta análise explícita não certifica políticas nem ativa ARC: somente
--memory=arc escolhe ARC na CLI. CI 38051323600 continua executando;
Linux passou, macOS passou retorno da fonte/instâncias zeradas/campos
tipados e chegou a campos locais; Windows executa recarga JIT.

Solver inicial de inclusão em optimize/arc/analise/points_to.rs:
worklist agenda dependentes de variáveis/campos alterados, com semeadura,
cópias/Phi, leituras e stores monotônicos por campo/layout nominal.
Stores nunca apagam arestas antigas; receiver desconhecido torna o campo
opaco para qualquer nó, inclusive descoberto depois, sem contaminar outros
layouts. Topo por budget se propaga aos dependentes. Validação de índices
ocorre antes da solução. Consulta de campo ausente/opaco não prova vazio.
Oráculo de fecho transitivo confere inclusão nos 512 grafos de três
variáveis com três budgets. Regressões cobrem aliases tardios, stores
após leitores, ordem invertida, opacidade, limites e cadeia de 20 mil
variáveis com dependência cíclica. Passaram 270 testes do emissor (sete
ignorados), cinco testes focados finais do solver, incluindo o oráculo
acrescentado, e 69 exemplos públicos. Logs
target/arc-points-to-worklist-{suite,oraculo,doc}.log.
Restrição ausente não significa efeito ausente do programa: produtores
HIR, valores iniciais, resumos SDK/callbacks, instanciação de placeholders,
efeitos transitivos, escape/observadores e certificados ainda pendentes.
Este solver consome fatos explícitos e não escolhe políticas nem altera
a ativação exclusiva por --memory=arc. Não concluir S0/S1 ou o gate ARC/A0.
CI 38051323600: Linux terminou com sucesso; macOS passou pela seleção de
memória e retorno da fonte e executa instâncias zeradas; Windows passou
o teste geral e executa recarga JIT. Rodada ainda não concluída.

Base de domínio da §28.1 em optimize/arc/analise/modelo.rs:
SitioArc usa identidade nominal de origem/especialização, independente de
ValueId; NoAbstrato distingue alocação/contexto, parâmetro, resultado
externo, global e unidade sintética. Placeholders não são objetos frescos:
instanciação futura deve substituí-los pelos aliases reais. Chaves ainda
não são produzidas/preservadas automaticamente pelo lowering/inliner/async.
ConjuntoPontos separa conjunto conhecido vazio de topo desconhecido.
Inclusão/união são monotônicas; budget imutável excedido eleva ao topo,
sem conjunto truncado. Topo é absorvente mesmo entre budgets distintos.
API não fornece atualização forte, singleton ou certificado de política;
limites de contextos, motivos de desconhecimento e resumos SDK ainda
precisam de produtores/solver e versionamento no plano.
Testes exaustivos de três conjuntos sobre três nós, com quatro budgets,
conferem união exata/ou topo, comutatividade, associatividade e idempotência.
Regressões distinguem placeholders/contextos e recusam perda de aliases
ao reduzir budget. Passaram 266 testes do emissor (sete ignorados) e
63 exemplos públicos; logs target/arc-dominio-pontos-{suite,doc}.log.
Este é o domínio usado pela futura worklist, não a conclusão de S0/S1,
integração ao pipeline ou mudança da seleção explícita --memory=arc.
CI 38051323600 continua ativa; macOS chegou à seleção de memória na CLI,
Linux chegou aos testes JIT e Windows continua no passo test.

Infraestrutura comum prevista na §33.2 agora em optimize/scc.rs:
Tarjan iterativo recebe grafo indexado, devolve a partição completa em
ordem de dependências e recusa destinos fora do grafo. Resumo de exceções
usa esse utilitário para recursão direta/mútua; SCCs de chamadas continuam
distintas dos futuros grafos de tipos, sítios e unidades de memória.
Oráculo independente por fecho transitivo confere partição e ordem nos
512 grafos de três nós. Cadeia e ciclo com 100 mil nós passam sem recursão
Rust; casos vazio, isolado, autoaresta e duplicação de arestas passam.
Regressão do consumidor confere recursão, propagação a chamadores,
chamada externa desconhecida e folha que continua sem lançar.
Passaram 263 testes do emissor (sete ignorados), 55 exemplos públicos e
os 16 casos AOT da cadeia da fonte no Windows. Logs
target/arc-scc-compartilhado-{suite,doc,aot}.log. Extração não certifica
points-to, aciclicidade de instâncias, regiões/grupos ou o gate ARC/A0;
seleção de ARC continua exclusiva de --memory=arc. Mudança ainda local.

Prova arc_source_return ampliada para repassar(valor) chamar
identidade(identidade(valor)), usando os dois corpos reais do lowering.
Planos continuam padrão, preenchidos pelos produtores: retorno Ref Owned,
retenção no callee e liberação automática do resultado intermediário.
Modo de depuração do Context preserva as fronteiras no inliner; os demais
passes HIR e o passe de exceções continuam ativos. Preparação é idempotente
e HIR final passa pelo verificador. Harness confere owner antes de soltar
o argumento, sobrevivência do retorno e morte após a liberação final.
Novo controle retira a liberação intermediária identificada no HIR,
exigindo alvo único no LLVM. Matriz Windows aprovou quatro positivos e
12 traps em ARC/tracing × O0/O2, com auditoria, ARC puro e GC stress.
Log target/arc-retorno-cadeia-matriz.log; artefatos e manifesto em
target/proof-source-return. Tracing compara o protocolo explícito do
harness sem ativar ARC. Isso amplia a prova entre corpos da fonte, sem
certificar o programa/SDK completo, dispatch ou o gate ARC/A0.
CI 38051323600 da revisão publicada 37497527 está executando; esta
ampliação é posterior e ainda não tem confirmação remota.

CI 38048561639 da fonte 137e428c: Linux e mensagens passaram; macOS
falhou na seleção explícita de memória via FFI no JIT (esperado 1, obtido
0). AOT do mesmo teste passou. O JIT usa o runtime do SDK RTLD_LOCAL,
mas @Native procurava a ABI no processo e podia alcançar o runtime do
hospedeiro com outro heap/TLS. Resolvedor agora obtém símbolos do catálogo
reservado na própria instância do runtime; símbolos estrangeiros continuam
no lookup do processo. Seleção local é gerada para módulo Cargo e fonte
avulsa do AOT a partir do mesmo catálogo, incluindo as condições Unix.
Não altera ativação ARC: somente --memory=arc escolhe essa política na CLI.

Regressões passaram: 184 testes do runtime (três ignorados), quatro de
fonte única (incluindo resolver/cache para todo o catálogo), 42 exemplos
públicos e três testes da CLI com oito execuções AOT/JIT/recarga no Windows.
Teste com lookup estrangeiro simulado verifica que homônimo do hospedeiro
não é consultado para a ABI, e consulta local acompanha o heap tracing/ARC.
Logs target/arc-ffi-runtime-local-{focado,suite,doc,cli}.log e
target/arc-ffi-runtime-local-fonte-final.log. Confirmação macOS depende da
CI seguinte. Rodada 38048561639 terminou com falha: Windows tinha uma
expectativa desatualizada de assinatura no teste do catálogo ownership.
Expectativa agora confere a assinatura unsafe exata; regressão adicional
confere a fábrica Owned e seu callback de tabela estática.
Reprodução local de todo o workspace terminou com duas falhas: essa
expectativa e a auditoria de portas, que detectou um transmute desnecessário
no novo teste do resolvedor. Conversão removida em favor do ponteiro de
função tipado, sem ampliar a lista de exceções da auditoria.
Após as correções, passaram 184 testes do runtime (três ignorados), quatro
de fonte única, dois de ownership e dois de portas. Logs
target/arc-ci-windows-workspace-local.log e
target/arc-ci-windows-correcoes-focado.log. Demais alvos da reprodução
completa passaram antes dessas correções restritas aos testes.
Sem toolchain 38048561628 passou. Esta rodada não foi integralmente aprovada.

Lowering agora registra a convenção Ref Owned pelo símbolo do corpo tipado
Dart da fonte. Fatos acompanham a junção paralela e a poda; não são inferidos
para thunks, callbacks FFI ou funções extras por terem a mesma largura.
Produtor produzir_retornos_ref_do_lowering confere símbolos/representações,
preenche os planos padrão e recusa Borrowed conflitante atomicamente.
Preparação do módulo usa esse produtor antes de verificar os corpos.
Teste com método realmente baixado da fonte passa pelo CFG excepcional,
preparação ARC e LLVM sem um plano manual de retorno; produz retain do
argumento emprestado. Tracing não recebe os fatos nem a preparação.
Passaram 258 testes do emissor (sete ignorados) e 55 exemplos públicos.

Prova AOT arc_source_return baixa uma identidade Object? da fonte Dart
com o SDK 3.6.2, extrai seu corpo no grupo fechado e usa a nova entrada de
preparação. Harness nativo recebe Mint real, solta o argumento, confere
sobrevivência/identidade do retorno e morte após soltá-lo. ARC/tracing ×
O0/O2 aprovou quatro positivos e oito controles por trap, retirando retain
ou release do retorno. Tracing compara os tokens explícitos do harness,
sem ativar ARC no heap. Isso não certifica preparação completa de um
programa/SDK, construção, dispatch, Finalizable ou exceções gerais.
Matriz e upload de fonte/IR/saídas/logs adicionados à CI. Logs locais:
target/arc-retornos-lowering-{suite,doc}-final.log e
target/arc-retornos-lowering-aot.log. CI da fonte 137e428c está executando
nas três plataformas; estas mudanças ainda são locais.
Fontes registradas em e1233a12; a matriz final foi repetida nessa revisão
sem alterações locais e passou nos 12 casos. Evidência congelada em
bench/results/2026-10-10-retorno-da-fonte-windows: 72 arquivos brutos,
incluindo fontes Dart, e três logs, com 75 hashes conferidos. Manifesto
registra SDK 3.6.2, backend/flags e a precedência dos logs de suite/doctests.

CI 38043719059 da fonte 124624db terminou com sucesso em Windows,
Linux, macOS arm64 e mensagens; Sem toolchain 38043719100 também passou.
Essa rodada inclui os campos tipados da fonte e as matrizes anteriores,
mas não as correções/preparações locais posteriores.

Nova entrada preparar_arc_modulo_tabelado importa o inventário excepcional
do módulo, prepara ownership e publica corpos/planos/tabelas numa transação.
O produtor produzir_tabelas_arc_do_modulo exige o mesmo conjunto de símbolos,
SSA válida e tabelas vazias ou iguais às fornecidas; recusa conflitos e
inventário incompleto sem alterar planos. Tracing devolve sem alterações.
Teste usa o passe real de exceções sobre a forma de chamada/conferência/catch
da fonte e verifica invoke, preservação do clear e owner no pouso. Falha de
ownership depois da importação conserva módulo e planos; repetição é
idempotente. Contratos de ownership/callees/escopos ainda são fornecidos;
essa entrada não constitui cobertura automática completa da fonte/SDK.
Passaram 257 testes do emissor (sete ignorados) e 54 exemplos públicos.
Fixture arc_guarded_returns exercita a entrada integrada no caminho ARC
automático: AOT normal Windows com auditoria, ARC puro e GC stress terminou
em zero com saída 9223372036854775807. Nessa fixture o inventário inicial é
explícito; o teste unitário usa o passe real de exceções. Logs finais em
target/arc-planos-excecoes-{suite,doc,aot}-final.log e saída
target/arc-planos-excecoes-normal.stdout. Unwind executado desta etapa em
Unix permanece para a CI; não há certificação de unwind Windows.

Preparação explícita do módulo ARC agora publica também as tabelas de
exceções verificadas quando esse modo já foi selecionado. Antes, corpos e
planos podiam ganhar novos invokes/pousos enquanto Module::tabelas ainda
descrevia o CFG antigo. A publicação acompanha a ordem final das funções e
só ocorre após verificar todo o conjunto; erros conservam corpos, planos e
tabelas anteriores. Tracing permanece sem alterações, e a API não seleciona
o modo de exceções nem ativa ARC implicitamente.
Regressão cobre seis situações: publicação, plano ausente, tracing, modo
de tabelas desligado, falha posterior em um corpo e ordem invertida das
funções; confere invoke LLVM e idempotência. Passaram 254 testes do emissor
(sete ignorados) e 52 exemplos públicos. A fixture arc_guarded_returns
passa a usar a publicação pelo módulo no caminho ARC automático; prova
AOT normal no Windows, com auditoria/puro/GC stress, terminou em zero e
imprimiu 9223372036854775807. Logs target/arc-publicacao-tabelas-*. Isso
valida a API explícita, sem certificar unwind Windows ou a integração
automática completa da fonte/SDK.

Contrato de ativação: ARC é opt-in por `--memory=arc`; sem a opção,
tracing permanece padrão. Integração dos passes ao fluxo normal de
compilação deve respeitar essa seleção. C++ aparece nas fixtures de
exceções estrangeiras; compilador e runtime são implementados em Rust.

Correção da seleção na CLI registrada em 4fe197ef: AOT/compile-native/run/
reload partem de tracing, mesmo com DARTFORGE_MEMORY=arc herdado; somente
a opção explícita escolhe ARC. run/reload agora reconhecem --memory=arc.
Em run a opção deve preceder o arquivo; depois dele é argumento de main.
O reinício da recarga transporta a seleção para o filho e mantém a variante
do SDK. Três testes passaram, incluindo oito execuções que consultam o modo
real do heap pela ABI via FFI (AOT, JIT e recarga com/sem reinício), além da
recusa de política inválida antes de carregar fonte. Log local:
target/arc-memoria-cli-test.log. Commit ainda local enquanto a CI anterior
termina; a regra opt-in está documentada em ARC-IMPLEMENTACAO.md e JIT.md.

Preparação explícita ARC agora reconhece a fábrica zerada object_new usada
pelo lowering de instâncias. Confere classe/quantidade constantes, tipos e
layout, preserva ID, RTI/inicializadores/construtor subsequentes e entrega
token Owned. Campos ainda em construção conservam seu protocolo; isso não
certifica receivers/campos nem inicialização completa. Casos inválidos são
atômicos, a preparação é idempotente e tracing conserva o corpo.
LLVM conserva o registro preguiçoso de métodos pela nova fábrica
dartforge_arc_object_owned_t_v1. Seu callback segue o protocolo estático de
tabelas, sem Dart/GC/exceções; função Rust unsafe documenta a validade da
memória devolvida. Catálogo confere a assinatura Rust completa e distingue
TabelaEstatica de um inteiro/endereço genérico. Produtor runtime genérico
recusa essa entrada sem proveniência do emissor, sem publicar mapas.

Regressões aprovadas: 253 testes do emissor, 182 do runtime, 52 exemplos
públicos do emissor e 42 do runtime; sete/três testes ignorados das respectivas
bibliotecas permanecem fora desta rodada. Exemplo arc_zeroed_instance usa
o thunk real emitido para uma biblioteca e registra somente na fábrica.
Matriz nativa Windows aprovou quatro positivos e 12 controles por trap,
ARC/tracing × O0/O2: owner vivo, campo zero, lookup/chamada do método e morte
após release; retirada do owner/tabela/release é detectada. Repetição final
após explicitar unsafe na assinatura Rust também aprovada. Logs em
target/arc-instancia-*; matriz e regressão da CLI adicionadas à CI local.
Fontes registradas em 7ab36e24. Evidência congelada em
bench/results/2026-10-10-instancia-zerada-windows: 80 arquivos brutos
e seis logs, com hashes e vínculos aos blobs das fontes.
Integração automática completa no fluxo ARC, construção parcialmente falha,
versões/pins, contratos do SDK e gate ARC/A0 continuam pendentes.

Lowering de campos da fonte agora conserva I64/F64/I1/Ref em GetField e
SetField somente no modo ARC, com índice constante e campo não-late.
FFI, late e índices dinâmicos conservam seus protocolos atuais. Emissão
LLVM converte bits de leitura para a representação lógica, tanto inline
quanto na extensão; tipo_do_resultado também respeita essa representação.
A primeira prova AOT encontrou uma comparação de bool do SDK dart:ffi
ainda classificada como i64; corrigida e coberta por regressão LLVM.
Substituição escalar aceita F64/I1/I8 consistentes, recusa mistura com
bits legados e reinicializa os novos locais no ponto da alocação, inclusive
em laços. Regressões de fonte, LLVM e substituição escalar passaram;
250 testes da biblioteca e 52 exemplos públicos aprovados, sete ignorados
da biblioteca permanecem fora desta rodada local.

Prova nativa reproduzível: scripts/prove-typed-fields-aot.py recebe uma
CLI recompilada e executa a fixture campos_tipados_arc.dart em tracing
sem --memory e ARC explícito, com/sem otimização. Gera também uma variante
com padding até CAMPOS_EM_LINHA=4096: double/bool/Ref ficam na extensão.
Oito casos aprovados no Windows, com GC_STRESS=1; ARC_CONFERIR=1 e
ARC_BERCARIO=0 na execução. Verifica int máximo, -0.0, bool, identidade,
null, herança, mixin, enum e erros de late. Manifesto registra hashes da
CLI/fontes/executáveis/saídas; logs locais em target/arc-fonte-campos-*.
Matriz adicionada à CI Windows/Linux/macOS, execução remota ainda pendente.
Preparação automática completa de owners/cleanup a partir da fonte,
contratos do SDK e gate ARC/A0 continuam pendentes.

Na CI 38038842350 da fonte b198aadd, as matrizes de campos, pending e
laços do script compartilhado terminaram com sucesso nas três plataformas
(144 casos no total). A rodada completa terminou com sucesso em
Windows/Linux/macOS e mensagens. Sem toolchain 38038842352 terminou com
sucesso. Isso não inclui as preparações novas de AllocObject/campos locais
nem os campos tipados da fonte, publicados na rodada seguinte.

Campos locais registrados no commit 5d0272e9. Evidência congelada em
bench/results/2026-10-10-campos-locais-windows: 16 execuções,
80 arquivos AOT e quatro logs, com hashes e blobs das fontes. A base
abff112f e as modificações durante a execução são explícitas no manifesto.

Preparação ARC do módulo agora escolhe getters/setters auditados para
GetField/SetField de objetos locais com inicialização completa. Prova mantém
conjuntos de sítios por cópia/move/Phi, inclusive origem desconhecida nas
junções e ciclos ancorados; não faz forte atualização nem presume singleton.
Todos os layouts candidatos devem concordar na representação/índice.
Publicação/contenção, setter nativo de bits ou chamada sem contrato fechado
de heap invalidam a origem inteira, conservadoramente no corpo todo.
Ausência de chamada Dart no catálogo de ownership não basta para esse fato.
Ref vira empréstimo do receiver; keepalive existente retém antes da
invalidação. Escalares usam leitura de bits e conversões explícitas de
double/bool/byte. IDs dos resultados/usos são preservados e limites antes
da operação seguem seu primeiro auxiliar. Erros não publicam corpos/planos;
tracing não recebe essa preparação. Testes cobrem retenção antes da
sobrescrita, escalares/escopos, aliases publicados, entrada opaca em Phi,
layouts divergentes, Phi de dois sítios/classes e Phi ancorado em laço.
Passaram 248 testes do emissor (7 ignorados) e 52 exemplos de documentação.

Prova AOT arc_local_fields passou no Windows com embedded-llvm:
ARC/tracing × O0/O2, quatro positivas e doze controles por trap, retirando
keepalive, cleanup do receiver ou release final do retorno. HIR aloca 40
campos/Mint, muda e relê int 85, double -0.0, bool true e byte 255, lê o
campo Ref 39 e o substitui por null. Confere valor/owner do retorno, morte
do receiver após sair da função/coletar e morte do retorno após release.
Endereço diagnóstico não é raiz Ref. Logs target/arc-campos-{focado,suite,
doc}.log e target/arc-campos-locais-{example,matriz}.log. CI preparada para
a matriz nas três plataformas, ainda não publicada. Não prova receivers
recebidos por parâmetro, chamadas de SDK com resumos de heap, guardas late,
fonte Dart completa, versões/pins, suspensão, certificados contra
adulteração ou desempenho ARC≥A0. Integração ao pipeline padrão continua
pendente e será exclusiva de --memory=arc; tracing permanece o padrão.

Alocação Owned/inicialização de campos registrada no commit bee4dceb.
Evidência Windows congelada em
bench/results/2026-10-10-alocacao-campos-windows: 12 execuções,
60 arquivos AOT e três logs de verificação, com SHA-256/blobs de fontes.
Base/alterações durante a execução são explícitas no manifesto; nenhum
resultado nativo é atribuído apenas à revisão base b198aadd.

Preparação explícita do módulo ARC agora materializa AllocObject com layout
completo em fábrica Owned e setters auditados. Mantém IDs dos objetos,
reserva auxiliares acima dos metadados e inicializa as arestas Ref antes
do cleanup dos filhos. Escalares preservam bits; double usa bitcast, bool/byte
extensão sem sinal. Recusa classe/layout/tipo/quantidade incompatíveis,
literais implícitos e IDs esgotados, sem publicar corpos/planos se outra
função ou verificação posterior falhar. Usa a cópia privada já existente
do grupo, sem duplicar a cópia do módulo. Tracing permanece intacto.
Teste adicional confere keepalive de campo borrowed antes do safepoint da
alocação e release depois da inicialização. Passaram 242 testes do emissor
(7 ignorados) e 52 exemplos de documentação.
Prova AOT arc_field_allocation passou no Windows com embedded-llvm:
ARC/tracing × O0/O2, quatro positivas e oito controles por trap sem owner
do objeto ou sem release do filho. HIR cria objeto de 40 campos/Mint;
confere bits 85, -0.0, true e byte 255, aresta Ref no índice 39, owner do
retorno e morte final de objeto/filho após liberação/coleta. Logs
target/arc-objetos-{suite,doc}.log e target/arc-alocacao-campos-matriz.log.
CI nova preparada para essa matriz, ainda não publicada. Não certifica
lowering Dart completo, origem de receivers para getters, guardas late/tipo,
registro de métodos, construtores, versões/pins, suspensão ou desempenho.
Não integra o passe ao pipeline padrão; ARC permanece exclusivo de
--memory=arc e tracing continua o padrão.

Commits até b198aadd publicados em main após a aprovação de ed5897e9.
CI 38038842350 e sem toolchain 38038842352 da fonte b198aadd estão em
execução; elas não incluem a preparação de AllocObject acima. Na CI Linux,
as três matrizes do script compartilhado (campos, pending e laços) já
terminaram com sucesso; o job completo continua em execução. Windows e
macOS ainda executando, sem atribuir suas matrizes pendentes como aprovadas.

CI 38034929874 da fonte ed5897e9 terminou com sucesso nos quatro jobs:
mensagens, Windows, Linux e macOS. Sem toolchain 38034929845 também passou.
Os registros de artefatos abaixo conservam o estado observado ao congelar
as provas; a conclusão posterior não certifica as mudanças locais novas.

CI pesada 38031768190 da fonte b5887c94 terminou com sucesso: 26 jobs
aprovados, incluindo corpus nativo ARC/ARC stress, tracing, SDK da fonte,
JIT×AOT, determinismo, variantes A1/B0/B1, paridade de análise e custo zero.
Dois jobs foram skipped pela configuração (limitless_ui e sonda AOT do
executor de macros). É validação daquela revisão, não das mudanças de
ownership posteriores nem do critério de desempenho ARC≥A0.

Keepalive de campos conferido também nos artefatos da CI 38034929874,
fonte ed5897e9: 36 execuções Windows/Linux/macOS, doze positivas com stdout
exato 1 LF e código 0, 24 controles vazios por trap (Windows -1073741795,
Linux 132/SIGILL, macOS 133/SIGTRAP). Os 144 arquivos brutos foram
congelados em bench/results/2026-10-10-keepalive-campos-ci, com hashes,
IDs/digests dos artefatos e etapas. Windows/Linux estavam completos com
sucesso; macOS ainda em execução ao registrar. Não certifica o workflow
inteiro, o script compartilhado novo ou mudanças posteriores de pending,
Phi/cleanup/laços. Limites de HIR explícita e tracing de comparação mantidos.

Prova AOT arc_loop_cleanup passou no Windows: ARC/tracing × O0/O2 ×
normal/erro, oito execuções positivas e oito controles negativos por trap.
A HIR aloca Mint Owned em cada volta; preparação produz release na aresta
normal antes da coleta e na saída de erro. Hooks LLVM com assinatura Borrow
conferem owner antes/depois do release em dez iterações normais; depois da
saída da função e coleta, conferem morte física do último objeto. No erro,
uma pendência real pré-carregada encerra a primeira iteração; identidade da
exceção, ausência de owner e morte final são verificadas. Retirar o release
normal ou de erro produz trap, não uma falha de compilação aceita como prova.
Endereços de diagnóstico não são raízes Ref. Tracing compara o mesmo corpo
com tokens explícitos; isso não ativa preparação ARC no seu fluxo normal.
Não certifica throw originado no hook, unwind, morte de todas as alocações
anteriores, junções gerais, lowering Dart, Finalizable ou suspensão.
Evidência congelada em bench/results/2026-10-10-cleanup-laco-windows:
16 execuções e 80 arquivos brutos. Manifesto registra base 66fd85db,
modificações locais, hashes SHA-256 e blobs das fontes novas; não atribui
as fontes modificadas durante a prova à revisão base.

scripts/prove-arc-aot.py reúne campos, pending e laços, com saída positiva
exata 1 LF e controles exigindo trap de instrução, rejeitando erro de carga,
abort e falha de geração. Registra comandos, saídas, versões e hashes;
temporários ficam em target e são removidos ao encerrar. As três matrizes
locais passaram: 20 positivas e 28 negativas, 240 hashes conferidos. Laços
também passaram com embedded-llvm. Teste do exemplo passou (1 teste), YAML
confere 48 steps válidos. CI preparada para usar o script e guardar logs e
manifestos nas três plataformas; essa revisão ainda não foi publicada.
Na CI 38034929874 da fonte ed5897e9, Windows e Linux terminaram com sucesso;
macOS ainda em execução. A etapa de campos passou nas três plataformas,
mas essa execução não cobre o script novo nem pending/laços novos.
ARC continua exclusivo de --memory=arc; tracing permanece o padrão.

Preparação ARC agora insere cleanup de tokens mortos em arestas normais,
incluindo arestas críticas, Switch e voltas de laços. Planejamento usa o
mesmo fluxo linear de tokens, com vivacidade e dependências de borrows;
consumos/Phi continuam conferidos antes do descarte. Não libera entradas
transferidas nem resultados Phi antes de sua definição. Divide a aresta,
transporta Phi/limites lexicais e reserva IDs acima de corpos/metadados.
O verificador isolado mantém inventários estritos, sem descarte implícito.
Release introduz barreira conservadora: keepalive e cleanup repetem até
estabilizar, sobre cópias privadas do grupo. Retenções de retorno são
preparadas antes do planejamento e compartilham o produtor existente.
Testes cobrem junção crítica, owner sustentando Phi Borrowed até a cópia
do retorno, alocação em laço sem acumular token, tracing, idempotência e
IDs SSA/bloco esgotados sem publicação parcial. Inventário do segundo
invoke deixa de carregar o owner morto após o primeiro, conservando seu
tratador. Passaram 130 testes ARC, 238 testes do emissor (7 ignorados) e
52 exemplos de documentação. No Windows passaram 8 execuções AOT de
retorno/erro simples/misto em ARC/tracing e 32 regressões nativas de campos
e pending com controles negativos. Prova resume automática passa a emitir
13/17 drops estáticos em vez de 18/21, com convenção e fluxo verificados;
não é medição de desempenho. Logs target/arc-arestas-{focado,suite,doc,
resume,nativa}.log e target/prova-{arc-resume-arestas,arestas-nativa}.
Isso não prova morte nativa em laços/junções gerais. Arestas excepcionais e
pousos ainda exigem cleanup preparado; Finalizable, captura e suspensão
exigem extensões de vida materializadas como usos na HIR. Produção dessas
extensões, slots/regiões/pins, lowering completo e desempenho ARC≥A0
continuam pendentes. ARC permanece exclusivo de --memory=arc.

Keepalive de Phi agora prepara todas as origens emprestadas num mesmo lote,
antes de reconstruir a classificação da junção. Evita rejeitar o estado
intermediário com uma entrada promovida a Owned e outra ainda Borrowed.
Percurso das origens deduplica SSA e termina em ciclos ancorados; cópias
falíveis continuam somente na aresta normal, com pendências/Phis transportados.
Disponibilidade excepcional é conferida antes da primeira promoção do lote.
Testes cobrem dois getters distintos em diamante normal/falível, Phi com
volta já transferindo o próprio token, repetição idempotente e tracing intacto.
Origem sem ABI suportada no segundo ramo recusa o grupo sem publicar a
primeira promoção privada. Passaram 127 testes ARC, 235 testes do emissor
(7 ignorados) e 52 exemplos de documentação; logs
target/arc-keepalive-phi-{focado,suite,doc}.log. Não é prova nativa desses Phis,
cleanup geral de laços/arestas nem integração do lowering. ARC continua
exclusivo de --memory=arc.

Prova AOT arc_keepalive_pending passou no Windows: ARC/tracing × O0/O2
× sucesso/erro, oito execuções positivas e doze controles negativos. O
getter auditado de record retorna BorrowArg(0); preparação insere cópia
somente na continuação normal e cleanup do owner temporário no erro.
Harness LLVM prepara record/Mint, confere identidade do retorno, morte do
record, sobrevivência do resultado Owned e morte após a liberação final.
No erro, uma exceção real é pré-carregada: o getter atual não lança por si
mesmo. A prova cobre o desvio pending, não uma falha originada dentro dele.
Confere retorno null, ausência de owner do resultado, identidade da exceção,
clear e morte de record/filho após cleanup. Controles sem retenção, sem
liberação final e retenção antes da guarda no erro falham por trap
(-1073741795); nenhuma falha de geração conta como controle aprovado.
Teste do exemplo confirma posição da cópia/cleanup e idempotência. Logs
target/arc-keepalive-pending-{example,build}.log e target/proof-keepalive-pending.
Matriz nova preparada na CI Windows/Linux/macOS, ainda não publicada nesta
retomada para não cancelar a validação em curso de ed5897e9. Não certifica
alocador de record no lowering (harness usa ABI legada com retain imediato),
fonte Dart, suspensão/recarga ou unwind. Tracing mantém tokens explícitos
somente no harness de comparação; ARC no produto continua exclusivo de
--memory=arc.

Prova AOT arc_field_keepalive passou no Windows: ARC/tracing × O0/O2,
quatro execuções positivas e oito controles negativos sem retenção ou sem
liberação final, todos recusados por trap (-1073741795). HIR de leitura Ref
seguida de substituição da aresta recebe keepalive automaticamente e retorna
Owned; o harness LLVM cria instância de 40 campos e Mint mortal, observa
owner 1→3→0 e morte do receiver após sair da função, sem alocar entre morte
e observação. Compilação usa o driver normal e as ABIs auditadas de objeto,
campo Ref e caixa Owned. Teste do exemplo confere retenção antes do setter
e preparação idempotente. Logs target/arc-keepalive-campos-{build,example}.log
e target/proof-field-keepalive. CI foi ampliada para executar a matriz e
preservar IR/stdout/stderr/exit em Windows, Linux e macOS; execução desses
novos jobs ainda pendente. No tracing, o harness mantém o mesmo corpo de
tokens explícitos para testar compatibilidade runtime; não ativa preparação
ARC no fluxo padrão. Não prova lowering de fonte nem keepalive pending.
Evidência Windows congelada em
bench/results/2026-10-10-keepalive-campos-windows: fonte 663fe2ab,
12 execuções e 50 arquivos brutos com SHA-256 e bytes originais.

Keepalive ARC agora cobre resultado Ref borrowed de runtime falível com
guarda canônica de pendência. Divide somente a aresta de sucesso e retém
num bloco novo; a chamada/guarda permanecem juntas e o caminho de erro não
recebe token nem release desse resultado. Confere disponibilidade no corpo
original antes da promoção, transporta os predecessores de Phi e os limites
da aresta normal, move a pendência para o ID auxiliar e reconstrói ownership.
Phi invalidado no próprio bloco busca suas origens emprestadas para proteção.
Testes conferem retorno Owned e cleanup normal, ausência de cópia/drop no
erro, limites lexicais, repetição idempotente, tracing intacto e recusa sem
publicação parcial de uso no erro, guarda adulterada e IDs de bloco esgotados.
Passaram 126 testes ARC, 234 testes do emissor (7 ignorados) e 52 exemplos
de documentação. Logs target/arc-keepalive-pending-{focado,suite,doc}.log.
Não é prova AOT nem integração completa do lowering; regiões/slots,
cleanup geral de laços/arestas, suspensão, Finalizable e versões/pins
continuam pendentes. Preparação permanece exclusiva de --memory=arc.

Preparação ARC do módulo protege automaticamente resultados runtime Ref
borrowed sem falha quando o checker detecta invalidação/reentrada. Retém na
definição, antes da barreira, conservando o ID original como Owned e criando
um ID auxiliar para a leitura emprestada; usos/arestas mantêm seus IDs.
Move efeitos e limites anteriores para a leitura auxiliar, reconstrói Phis
após promoção e usa o cleanup/retorno Owned já verificado. Só copia quando
há necessidade detectada; caminho já protegido e repetição são idempotentes.
Ancestral invalidado é escolhido na cadeia de aliases, evitando tentar
proteger repetidamente um alias cuja sustentação já se perdeu. Verificador
isolado continua somente de leitura e recusa o corpo sem proteção.
Testes cobrem campo substituído, argumento de chamada Dart, diamante com
transferência Owned no Phi, ancestral/alias, tracing intacto, IDs esgotados
e falha de função posterior sem publicação parcial. Origem não auditada e
resultado falível não recebem cópia antes da guarda; proteção somente na
aresta normal, cleanup de laços/arestas gerais, regiões/slots, suspensão,
Finalizable e versões/pins continuam pendentes. Isso não escolhe ABIs de
campos no lowering nem integra ownership ao fluxo padrão; ARC permanece
exclusivo de --memory=arc. Não é nova prova AOT nem medição de desempenho.
Passaram 233 testes do emissor (7 ignorados), incluindo 125 testes ARC,
e 52 exemplos de documentação. Logs target/arc-keepalive-{focado,suite,doc}.log.

ABI dartforge_arc_read_field_ref_v1 confere instância viva, índice e marca Ref
no mapa físico e devolve BorrowArg(0), sem alocação, coleta, retenção ou Dart.
Null/Smi são Ref válidos; bits iguais a handle num campo escalar são recusados.
O receiver vivo não basta após substituir sua aresta: prova runtime em
ARC/tracing com stress demonstra morte do filho sem cópia e sobrevivência
quando o empréstimo é copiado para Owned antes da substituição. Contrato
HIR liga leitura borrowed e retenção do retorno Ref Owned, rejeita anotação
Owned falseada/getter legado sem contrato e preserva tracing atomicamente.

Verificador de tokens agora reconstrói barreiras conservadoras de aliases
Borrowed(Valor): catálogo runtime fornece invalidação/reentrada, chamadas
Dart/opacas exigem owner independente do argumento derivado, outras
operações não provadas são barreiras. União dos caminhos inválidos chega a
ponto fixo nos laços; Phi confere cada aresta e nova leitura renova seu SSA.
Recusa empréstimo após substituir campo mesmo com receiver ainda disponível;
cópia anterior para Owned passa. Mantém prioridade de diagnósticos de owner
consumido. Fixture de escopo usa getters EnvGet sem reentrada em vez de
presumir que chamadas desconhecidas sejam aliases puros. Testes cobrem
junção, volta de laço, releitura, argumento de callback opaco e não publicação
parcial. Não insere keepalive, não refina regiões/aliases de slots nem prova
proveniência geral, layouts, versões/pins ou suspensão. Seleção das ABIs no
lowering e integração no fluxo normal continuam pendentes, somente por
--memory=arc. Sem nova prova AOT. Passaram 181 testes runtime (3 ignorados),
2 testes do catálogo, 41 exemplos runtime, 229 testes do emissor (7 ignorados)
e 52 exemplos do emissor. Logs target/arc-leitura-ref-runtime-{suite,doc}.log
e target/arc-leitura-ref-emissor-{suite,doc}.log.

ABIs dartforge_arc_write_field_{escalar,ref}_v1 distinguem valor escalar
de referência emprestada sem uma flag dinâmica de classificação. Atualizam
bits, mapa real e barreira via Heap::definir_campo; a operação Ref conserva
uma aresta forte sem consumir tokens do chamador, com retenção antes da
liberação anterior e autoatribuição sem mudar multiplicidade. Escalar remove
a aresta antiga e nunca segue os bits novos, mesmo iguais a um handle.
Ambas invalidam borrows pela substituição de aresta; não alocam/coletam nem
executam Dart. Catálogos ownership/efeitos e externs LLVM auditados juntos.
Testes ARC/tracing com stress cobrem mapa estendido, arestas repetidas,
autoatribuição, null, autociclo desfeito, -0/NaN/MAX e morte do filho quando
a última aresta é retirada, preservando os mesmos bits em posições escalares.
Receiver/índice inválidos não gravam. HIR liga duas fábricas Owned, gravações,
leitura escalar e dois drops; assinaturas falseadas e setter legado sem
contrato são recusados sem publicação parcial. Preparação é idempotente e
não altera tracing. Seleção no lowering, receiver/layout, guardas Dart de
tipo/late, versões/pins e proteção de borrows continuam pendentes; não é
integração no fluxo normal nem nova prova AOT. ARC só por --memory=arc.
Passaram 180 testes runtime (3 ignorados), 2 testes do catálogo e 40 exemplos
runtime; 225 testes do emissor (7 ignorados) e 52 exemplos de documentação.
Logs target/arc-gravacao-runtime-{focado,suite,doc}.log e
target/arc-gravacao-emissor-{suite,doc}.log.

ABI dartforge_arc_object_owned_v1 aloca INSTANCIA zerada e publica um único
token Owned no mesmo empréstimo do heap, antes do retorno/reabastecimento
TLAB. Confere classe i32 e número de campos u16 sem truncar o cabeçalho.
Ownership (classe nativa, quantidade escalar, resultado Owned), efeitos de
alocação/invalidação e extern LLVM foram auditados juntos. Testes em
ARC/tracing com GC stress cobrem 0/1/32/33/40 campos, mapa estendido zerado,
filho forte, sobrevivência a outra alocação/coleta e morte após um release.
Contrato HIR liga fábrica, leitura escalar, bitcast e cleanup automático;
assinatura incompatível ou alocador legado sem contrato são recusados sem
publicação parcial. Preparação é idempotente e não altera tracing.
Não escolhe a fábrica no lowering, não executa construtores/registra métodos
nem certifica layouts/recarga; inicialização de campos e integração no fluxo
normal continuam pendentes, sempre exclusivas de --memory=arc.
Sem nova prova AOT desta ABI. Passaram 178 testes runtime (3 ignorados),
2 testes do catálogo e 38 exemplos runtime; 224 testes do emissor
(7 ignorados) e 52 exemplos de documentação. Logs
target/arc-instancia-runtime-{focado,suite,doc}.log e
target/arc-instancia-emissor-{focado,suite,doc}.log.

Conjunto publicado b5887c94 validado na CI 38029919066: Windows, Linux,
macOS e mensagens concluídos com sucesso. CI Sem toolchain do sistema
38029919046 concluída com sucesso. ABIs de instâncias/campos, barreiras de
empréstimos e prova de keepalive acima são posteriores a essa fonte e ainda
exigem CI própria.

Auditoria dos layouts declarados percorre as 15 bibliotecas do SDK da fonte
usando a mesma etapa de registro de classes da produção, sem baixar seus
corpos. Preparação ARC valida os fatos de cada módulo sem produzir operações
de ownership quando não há funções. Isso não certifica forma física dos
objetos do SDK, receiver, versões/pins ou integração de todos os corpos.
Passaram 223 testes do emissor (7 ignorados) e 52 exemplos de documentação;
logs target/arc-layouts-sdk-{suite,doc}.log. ARC só é ativado na CLI por
--memory=arc; tracing continua o padrão.

Provas AOT da fonte f81c7d2b preservadas em
bench/results/2026-10-10-unwind-boxing-unix-ci: 72 execuções Unix, 114
arquivos com SHA-256 e bytes originais. Os 12 IR ligam Box HIR de i64::MAX
à fábrica Owned auditada antes do cleanup. C++ é apenas a fixture que lança
exceções estrangeiras; compilador/runtime continuam Rust. Workflow, jobs e
etapas concluídos com sucesso. Não cobre a ABI escalar de campos nem os
contratos locais posteriores a essa fonte, e não prova unwind Dart real.

Lowering ARC registra layouts_campos_arc por classe/posição com as mesmas
regras usadas para ler/gravar campos. Inicializador e representação do campo
têm implementação compartilhada; late escalar com inicializador permanece
Ref. Fatos incluem herança, mixins e prefixo int/Ref de enum, excluem estáticos
e ficam ausentes em tracing. Teste parte de fonte Dart e confere os fatos
após otimização. Preparação atômica do módulo valida classe única, quantidade
de campos e representações válidas, recusando dados obsoletos/ambíguos sem
publicar alterações. Esses fatos declarados não provam receiver, forma física,
versão/pins de recarga nem inicialização; a ABI escalar ainda não é escolhida
pelo lowering. Integração padrão/borrows permanecem pendentes e exclusivos
de --memory=arc. Passaram 223 testes do emissor (7 ignorados), 52 exemplos
de documentação e cargo check --locked -p dartforge-jit --all-targets;
logs target/arc-layouts-{suite,doc,jit-check}.log.

ABI dartforge_arc_read_field_scalar_v1 copia bits somente após conferir
objeto vivo, índice e ausência da marca de referência no mapa real do heap.
Borrow do receiver, resultado I64 escalar, sem retenção/invalidação/GC/Dart;
catálogo ownership, efeitos e extern LLVM auditados juntos. Violação de
precondição é falha interna com aborto C, não TypeError Dart. Lowering ainda
deve provar receiver/layout e guardas de tipo/late antes de escolher a ABI;
não troca dartforge_object_get no fluxo padrão nem classifica seu resultado.
Prova de heap com stress em ARC/tracing inclui mapa estendido (40 campos),
bits de handle em campo escalar, referência com os mesmos bits, -0/NaN e
ausência de alocação/coleta. HIR liga resultado escalar ao bitcast e recusa
getter genérico e retorno Ref falseado, sem publicação parcial. Sem nova
prova AOT desta ABI; integração/layouts/borrows completos continuam pendentes.
Passaram 176 testes runtime (3 ignorados), 2 testes do catálogo e 37 exemplos
runtime; emissor: 221 testes (7 ignorados) e 52 exemplos de documentação.
Logs target/arc-campo-runtime-{suite,doc}.log, target/arc-campo-ownership.log
e target/arc-campo-emissor-{suite,doc}.log. ARC continua exclusivo de --memory=arc.

Verificador ARC reconfere constantes, boxing bool, aritmética e conversões
pelas mesmas regras do produtor, no módulo privado puros. Regressão reproduziu
aceitação de Bitcast Ref/F64 com resultado falseado Trivial
(target/arc-puras-verificador-antes.log); agora rejeita esse caso, larguras
inválidas, operações sobre referência, constante com tipo falso e destino Ref
de Bitcast. HIR permite apenas I64/F64 para essa reinterpretação. Operandos
escalares exigem origem Trivial, inclusive LNot/boxing bool, e operações
cobertas exigem efeito vazio. Preserva diagnóstico de contrato ausente.
Passaram 220 testes do emissor (7 ignorados) e 52 exemplos de documentação;
logs target/arc-puras-verificador-{suite,doc}.log. Proveniência geral,
domínio/estouro, demais operações e integração padrão continuam pendentes;
preparação ARC permanece exclusiva de --memory=arc.

Verificador de tokens reconstrói os contratos dos locais escalares no corpo
atual: não aceita só mapas produzidos antes de uma alteração de CFG/instruções.
Regressão reproduziu aceitação indevida de leitura após retirar a gravação
de um ramo (target/arc-locais-verificador-antes.log); a correção rejeita esse
caso e escape do endereço, leitura/gravação incompatível e efeito falseado,
sem alterar mapas. Produção e verificação compartilham análise somente de
leitura; verificação exige o inventário completo e valores gravados Trivial.
Demais contratos/proveniência/borrows continuam obrigações separadas.
Passaram 219 testes do emissor (7 ignorados) e 52 exemplos de documentação;
logs target/arc-locais-verificador-{suite,doc}.log. Integração padrão pendente,
sempre condicionada a --memory=arc.

Produtor ARC de locais escalares privados cobre Alloca/Store/Load de
I1/I8/I64/F64. Exige alloca Ptr na entrada, endereço usado só por leituras/
gravações diretas, tipos compatíveis e valores SSA Trivial. Inicialização é
análise must por interseção de predecessores, com ponto fixo nos laços;
gravação na volta não autoriza a primeira leitura. Bytes literais de estado
late exigem faixa 0..255; não certifica a semântica completa de late.
Regressões cobrem diamante, laço, quatro tipos, escape, erro de tipo e origem
gerenciada/ausente, sem publicação parcial. Preparação do módulo liga leitura
escalar a boxing Owned e cleanup, é idempotente e preserva tracing. Emissão
LLVM é conferida; não é nova prova AOT. Campos/layouts, locais Ref, endereços
capturados/importados e integração padrão continuam pendentes.
Passaram 218 testes do emissor (7 ignorados) e 52 exemplos de documentação;
logs target/arc-locais-{suite,doc}.log e target/arc-locais-modulo.log.

Contratos ARC cobrem ZExt/Trunc entre I1/I8/I64: exigem tipo exato de
origem, contrato Trivial e aumento/redução de largura respectivamente.
Não classificam handles/ponteiros por largura. Regressão cobre as seis
conversões válidas, direção inversa inválida, origem ausente/gerenciada e
sequência bool/byte/palavra até boxing permanente, sem Owned ou pending.
Emissão LLVM confere zext/trunc; não constitui prova AOT do código assíncrono.
Passaram 212 testes do emissor (7 ignorados) e 52 exemplos de documentação;
logs target/arc-larguras-{suite,doc}.log. Passe padrão ainda pendente e
restrito a --memory=arc; contratos de loads/campos e borrows exigem cobertura.

CI 38025125587 na fonte f81c7d2b concluída com sucesso: Windows, Linux,
macOS e mensagens. As provas AOT Unix de unwind passaram nos dois sistemas.
Avanços locais de bitcasts, larguras inteiras, locais, verificação e layouts
são posteriores a essa fonte e ainda exigem CI própria.
CI Sem toolchain do sistema 38025125631 concluída com sucesso na fonte f81c7d2b.

Contratos ARC cobrem os bitcasts F64/I64 usados pelo lowering de campos.
Exigem representações opostas, resultado compatível e origem Trivial após
resolver produtores/Phis, sem depender da ordem física dos blocos. Operações
numéricas também exigem operandos Trivial: aritmética não pode apagar um
contrato Owned/Borrowed ou fabricar origem para o bitcast seguinte. Falhas
preservam classes e planos. Regressões cobrem referência/ponteiro, origem
ausente/gerenciada, destino divergente e cadeia aritmética; emissão LLVM
confere bitcast nas duas direções, sem nova prova AOT de payloads double.
Passaram 211 testes do emissor (7 ignorados) e 52 exemplos de documentação;
logs target/arc-bitcast-{suite,doc}.log. Integração padrão permanece pendente
e será exclusiva de --memory=arc; tracing continua sem essa preparação.

CI 38021212710 concluída com sucesso na fonte 34198a54: Windows, Linux,
macOS arm64 e mensagens verdes. Inclui as etapas completas com ignorados,
SDK/JIT/recarga e as provas AOT Unix já preservadas. Avanços posteriores
de boxing/unbox/fatos/plano de módulo ainda precisam da nova rodada CI.

Preparação ARC de módulo reúne fatos escalares/RTI e preparação do conjunto
em uma transação: preparar_arc_modulo_dart. Tracing é no-op; ARC só publica
corpos e planos depois da validação completa. Testes cobrem idempotência,
fato RTI inválido e runtime sem contrato após parâmetros válidos, sem
publicação parcial. Não materializa tabelas nem ativa o passe no pipeline;
contratos/CFG/borrows ainda exigem cobertura. Log target/arc-modulo-atomico.log.
Passaram 209 testes do emissor (7 ignorados) e 52 exemplos de documentação;
logs target/arc-modulo-atomico-{suite,doc}.log.

Fatos RTI do lowering agora alimentam planos pela API
produzir_parametros_rti_do_lowering. Exige ID de parâmetro I64 com origem
RTI explícita e rejeita a mesma definição marcada como escalar Dart.
Não prova vida do universo/pins/recarga. Publicação atômica, idempotente,
e tracing permanece sem alteração. Teste da declaração Dart genérica já
prepara o parâmetro oculto pelo produtor RTI, sem inserir classe manual.
Negativos cobrem origens conflitantes, representações, ownership, símbolos,
IDs e parâmetros duplicados nos dois produtores. Integração padrão segue pendente.
Passaram 208 testes (7 ignorados) e 51 exemplos de documentação;
logs target/arc-rti-planos-{suite,doc}.log.

Parâmetros ocultos de RTI ($tipos/$tipos_de_fora) têm produtor explícito
add_param_rti e mapa separado dos escalares. Auditoria: tipos.rs representa
tuplas como IDs do universo canônico (Tipo::Tupla), não handles do heap Dart.
Fatos são produzidos somente no modo ARC e transportados por finalização,
absorção, combinação dos módulos e poda. Teste distingue RTI de escalar e
confere ausência dos fatos em tracing; ainda falta consumir o mapa RTI nos
planos e provar vida do universo/pins/recarga. Não infere Trivial por largura
nem ativa ownership no pipeline padrão.
Passaram 208 testes (7 ignorados) e 50 exemplos de documentação;
logs target/arc-parametros-rti-{suite,doc}.log.

Poda transporta os fatos escalares com os corpos: produção remove entradas
de funções eliminadas; poda da fonte conserva somente símbolos vivos.
Regressão cobre conservação de fatos vivos e retirada de fatos mortos nos
dois caminhos, evitando que planos ARC recebam símbolo obsoleto após poda.
Log target/arc-parametros-poda.log. Clones especializados e o restante dos
contratos/borrows ainda exigem cobertura antes da inserção padrão ARC.
Passaram os 15 testes de poda e 50 exemplos de documentação;
logs target/arc-parametros-poda-{suite,doc}.log.

Fatos de parâmetros do lowering agora alimentam PlanoFuncaoDart pela API
produzir_parametros_escalares_do_lowering: verifica símbolos/IDs/tipos e
conflitos antes de publicar qualquer Trivial. Tracing não aplica os fatos;
repetição válida é idempotente. Lower_program já conserva o modo escolhido.
Teste da declaração Dart genérica liga lowering, otimização, planos e
preparação ARC; parâmetro oculto exige contrato próprio e continua fora
do fato escalar. Casos negativos cobrem ID obsoleto, Ref indevido, classe
conflitante, plano ausente e função removida, sem publicação parcial.
Ainda não chama essa preparação no pipeline padrão: contratos de operações,
parâmetros nativos, retornos e exceções/borrows precisam de cobertura completa.
Passaram 208 testes do emissor (7 ignorados) e 50 exemplos de documentação;
logs target/arc-parametros-planos-{suite,doc}.log.

Lowering preserva fatos de parâmetros escalares Dart por símbolo/ID SSA,
somente quando Context.memoria_arc está ativo. Programa e SDK propagam
o modo escolhido antes do lowering. Declarar parâmetros do outline marca
int/double/bool sem caixa; add_param nativo/oculto não inventa essa marca.
Os mapas são transportados ao finalizar/absorver funções e combinar módulos
do lowering paralelo, FFI e SDK. Teste com declaração Dart genérica distingue
três escalares, T/Ref e o parâmetro oculto i64; tracing deixa o mapa vazio.
Log target/arc-parametros-lowering.log. O fato ainda precisa alimentar os
planos ARC; parâmetros manuais, clones especializados e transporte completo
após todos os passes ainda exigem cobertura. Não liga inserção ao pipeline.
Passaram 207 testes (7 ignorados) e 49 exemplos de documentação após esses
metadados; logs target/arc-parametros-lowering-{suite,doc}.log.

Unbox bool agora tem contrato ARC produzido e reconferido pelo verificador:
conserva o helper LLVM u8/I1 e o ID original, exige a mesma saída TypeError,
preserva Borrow e não consome o owner. A conferência de pending admite
essa operação explícita. Teste int/double/bool cobre parâmetros Borrow,
caixa Owned com drops em sucesso/erro, falta de saída excepcional e
adulteração que tentava retirar pode_falhar/pending. Emissão textual LLVM
confere chamada I1 e conversão u8; não é prova AOT nova.
Log target/arc-unboxing-bool.log. Reentrada/borrows e integração padrão
condicionada a --memory=arc continuam pendentes.
Após unbox bool: 207 testes passaram (7 ignorados) e 49 exemplos de
documentação passaram; logs target/arc-unboxing-bool-{suite,doc}.log.

Unbox I64/F64 na preparação do conjunto fechado vira chamada auditada
Borrow, preservando ID SSA e resultado escalar. Exige CFG de TypeError
preparado; sem saída de erro, a transação rejeita sem alterar corpos/planos.
Teste cobre referência emprestada do chamador e caixa Mint Owned local:
o owner permanece durante unbox e recebe um drop em cada saída normal/erro.
O resultado só fica disponível no sucesso. Log target/arc-unboxing-hir.log.
Bool ainda exige adaptar u8/I1 e transportar a conferência de pendência;
prova de empréstimos através da reentrada e integração padrão seguem pendentes.
Validação completa: 207 testes do emissor passaram (7 ignorados), mais
49 exemplos de documentação; logs target/arc-unboxing-{suite,doc}.log.

Catálogo validado de unbox int/double/bool com argumento Borrow,
resultado escalar e falha pending. O TypeError pode chamar helper Dart
do SDK; a marca pending-dart preserva essa reentrada e exige invalidação
de borrows. O gerador continua rejeitando reentrada não declarada.
Novo resultado ScalarF64 distingue double de referências/i64. Isso ainda
não traduz Unbox HIR nem fornece a prova de empréstimos através da reentrada.
Passaram o teste unbox do emissor, os dois testes do catálogo/gerador,
a suíte completa com 206 testes (7 ignorados) e 85 exemplos de documentação.
Logs target/arc-unbox-{contratos,runtime-catalogo,suite,doc}.log.

Condição reafirmada pelo proprietário: integração de ownership no pipeline
somente com --memory=arc; tracing permanece o modo sem essa opção.
A CLI já traduz a opção para DARTFORGE_MEMORY, consultada antes de preparar
programa/SDK. O pipeline documenta essa condição; regressão cobre boxing
double no tracing com exceções checagem/tabelas, sem fábricas Owned ou RC ARC.

Preparação do conjunto fechado classifica parâmetros F64/I1/I8 como Trivial:
são representações escalares sem handle ou slot forte. Classificação prévia
incompatível rejeita atomicamente a preparação. I64/Ptr continuam exigindo
proveniência e contrato explícitos; não há inferência por largura.
Teste parametrizado cobre boxing de double/bool por SSA, conflito de token,
I64 sem contrato rejeitado e I64 com contrato Trivial aceito.
Log local: target/arc-parametros-escalar.log. Integração padrão segue pendente.
Suíte do emissor após a regressão tracing: 205 testes passaram, 7 ignorados;
49 exemplos de documentação passaram (target/arc-parametros-suite.log e
target/arc-parametros-doc.log). A rodada anterior interrompida não foi contada.

Prova Unix de unwind estrangeiro/forçado congelada em
bench/results/2026-10-10-unwind-forcado-unix-ci: fonte 34198a54,
CI 38021212710, 72 execuções Linux/macOS. As três modalidades de unwind
(C++, forçado estrangeiro e forçado com tag Dart) passaram nos perfis
simples/quadros/Retoma, O0/O2, ARC/tracing, com GC_STRESS ativo.
Foram conferidos stdout exatos e preservados 114 arquivos com SHA-256,
incluindo as ligações macOS. O tag Dart forçado não é payload Dart real;
não prova finally/cancelamento, SEH/statepoints ou ownership no pipeline padrão.

Boxing escalar preparado pelo conjunto fechado ARC: Box I64/F64 vira chamada
às fábricas Owned; bool permanece caixa estática Trivial. A nova ABI
dartforge_arc_box_double_owned_v1 retém antes de qualquer coleta e conserva
bits de -0.0/NaN. O catálogo distingue f64:scalar de i64:scalar. Formas
incompatíveis são rejeitadas sem publicar corpos ou planos parciais.
Teste de runtime com Heap::new(true) passou nos dois modos; a suíte local
passou com 175 testes de runtime e 203 do emissor (3/7 ignorados).
Dois testes do conjunto cobrem liberações, transferência de retorno sem
retenção extra e atomicidade. A fixture estrangeira agora começa com Box
I64 e seus três testes HIR passaram. Logs: target/boxing-runtime-suite.log,
target/boxing-compiler-suite.log e target/boxing-fixture.log.
Ainda não liga a inserção ARC ao pipeline padrão nem cobre boxing SIMD.

CI 38021212710, fonte 34198a54: job Linux completo passou; macOS passou
as etapas de resume ARC e unwind estrangeiro, incluindo os perfis Retoma,
quadros locais e unwind forçado. A separação do runtime em dylib resolveu
a ligação macOS. A rodada completa Windows/macOS ainda estava em andamento
na consulta; boxing desta retomada ainda não pertence àquela fonte.

Fixture macOS da CI 38019611633 terminou com falha de ligação: o compact unwind
recusou quatro personalidades (Dart legada, cleanup Itanium, Rust e C++). Não
executou a fixture; não é falha observada de ownership ou destrutores.
A fixture local agora liga o runtime de produção numa dylib macOS, usando
force_load e install_name absoluto, e liga C++/LLVM a essa imagem separada.
Conserva as informações de unwind; não desliga compact unwind. O limite por
imagem consta no código da Apple:
https://github.com/apple-oss-distributions/ld64/blob/main/src/ld/passes/compact_unwind.cpp
Comandos de ligação são publicados como *.ligacao.txt. Em Linux, a fixture
continua usando a staticlib. Três testes HIR passaram novamente
(target/unwind-dylib-fixture.log); driver Clang 22 aceitou a composição Darwin
dynamiclib/force_load na consulta -###. Ligação e execução macOS do novo
conjunto, incluindo Retoma e forçado, passaram na CI 38021212710.


Provas de retomada sob estresse ativo nos dois sistemas: fonte f460ce72,
CI 38019611633, Linux e macOS passaram as etapas AOT. Dezesseis stdout
MAX LF e dezesseis IR conferidos, com rastro explícito e capturas Owned.
Evidência congelada em bench/results/2026-10-10-rastro-owned-estresse-unix-ci,
com fonte, etapas, jobs, digests Actions e SHA-256. Variáveis DARTFORGE_* agora
ativam de fato GC_STRESS/ARC_CONFERIR; as rodadas antigas com nomes sem prefixo
não comprovavam essa ativação. A fixture estrangeira macOS ainda executa.
Não prova morte final das capturas, identidade do objeto nativo de unwind Dart,
forced unwind, finally, inlining observado ou integração padrão de ownership.


Em validação local: Retoma aceita fechamento explícito de quadros locais,
além de ArcDrop. O fechamento permitido é somente a ABI auditada
quadro_fechar_v1 com um ID SSA Native/i64, retorno Void, sem coleta ou Dart.
Tokens, materialização das tabelas e emissor compartilham essa regra; a pilha
LIFO completa é conferida antes de admitir o cleanup. Externs arbitrárias,
coleta, alteração de pendência, aliases/importados e quadros esquecidos continuam
rejeitados. O par do landingpad permanece o mesmo até resume.

Fixture com perfil retoma prepara dois quadros fortes e um owner independente;
o pouso fecha os quadros, libera o owner e retoma sem executar o catch Dart.
Três testes HIR da fixture passaram. Teste LLVM local gera objetos Linux O0/O2
e verifica LIFO no catch estrangeiro e na Retoma pura; também verifica resume
original e rejeição de ordem invertida, quadro esquecido e coleta. Logs target/retoma-quadros-fixture.log
e target/retoma-quadros-llvm.log. Suíte completa do emissor: 202 aprovados,
7 ignorados; 49 doctests aprovados. Logs target/retoma-quadros-emit-lib.log e
target/retoma-quadros-doc.log. YAML e sintaxe POSIX do workflow passaram.
Workflow local inclui simples/quadros/retoma × O0/O2 × ARC/tracing ×
C++/forçado estrangeiro/forçado com classe Dart, com nomes de ambiente
corrigidos (36 execuções por runner).
Execução nativa do novo perfil Retoma e do caminho forçado ainda não foi provada.

A fixture C++ agora também chama _Unwind_ForcedUnwind com objeto nativo
prealocado. Há duas classes: estrangeira e DARTFRGE, para conferir que
FORCE_UNWIND impede o catch mesmo com a classe Dart. Esta segunda é injeção
de falha sem payload/pendência Dart; não é lançamento Dart do runtime.
O callback confere identidade, fase cleanup/force e, somente ao fim
da pilha, destrutor local, ausência de catch Dart, topo de raízes, pendência zero,
owner consumido e morte do Mint após coleta. Imprime forced-cleanup-ok, faz
fflush e encerra o processo no callback terminal; retorno ao lançador ou descarte
prematuro do objeto falham. Não é simulação de pthread_cancel nem comprova
shutdown/finally/async. Sintaxe C++ GCC C++17 Wall/Wextra/Werror e script POSIX
passaram no WSL; três testes HIR passaram novamente. Log
target/unwind-forcado-fixture.log. Prova com runtime completo pendente.

CI 38019611633, fonte f460ce72: as duas provas Linux terminaram com sucesso.
Artefato estrangeiro conferido byte a byte: oito stdout foreign-cleanup-ok LF,
quatro IR e quatro C++, simples/quadros × O0/O2 × ARC/tracing. Rodada usa
DARTFORGE_ARC_CHECK=1, DARTFORGE_ARC_NURSERY=0 e DARTFORGE_GC_STRESS=1,
nomes efetivamente consumidos pelo runtime. Morte final do Mint, liberação dos
dois quadros, identidade C++, destrutores e restauração de raízes passaram.
Evidência congelada em bench/results/2026-10-10-unwind-estrangeiro-linux-ci,
com SHA-256, fonte, etapa, job e digest Actions. o job Linux completo terminou com sucesso depois da captura; macOS e workflow
completo continuam ativos; macOS já executa a prova de retomada; não extrapolar a prova Linux para macOS ou para o perfil
Retoma com quadros, que foi desenvolvido depois desta fonte. O escopo completo ARC,
§§27–34, SEH/statepoints, forced unwind, finally, async/cancelamento e gate de
desempenho continuam pendentes conforme os registros anteriores.

## Retomada de 2026-10-09

Atualização da investigação de unwind estrangeiro (fonte publicada 414cc174):
o job Linux da CI 38017693830 passou as oito provas de retomada Dart, mas a
fixture C++ O0/ARC terminou com código 18 na observação após coleta. Identidade
C++, destrutores locais, desvio do catch Dart, restauração de raízes, pendência
zero e consumo do owner passaram antes dessa observação. O diagnóstico lia
cabeçalhos antigos em faixas livres quando o heap não zerava mortos. Correção
local consulta também faixas livres e região disponível; regressão reproduziu
o falso positivo e agora passa em tracing e ARC sem zerar cabeçalhos.
A execução C++ completa precisa ser repetida antes de afirmar morte final.

Revisão dos ambientes de prova: o runtime lê DARTFORGE_ARC_CHECK,
DARTFORGE_ARC_NURSERY e DARTFORGE_GC_STRESS. Atribuições anteriores a
ARC_CONFERIR, BER e GC_STRESS eram ignoradas. Os resultados anteriores
comprovam as saídas e propriedades observadas, mas não ativação dessas opções.
Arquivos históricos congelados permanecem intactos; suas alegações de execução
sob essas opções devem ser lidas com esta correção. Testes unitários que usam
Heap::new(true) ativam estresse diretamente e não dependem dessas variáveis.
O workflow local agora usa os nomes efetivamente consumidos pelo runtime.

O job macOS da mesma CI falhou antes de executar a fixture: faltaram frameworks
Security/CoreFoundation na ligação da staticlib do runtime. A fixture local
agora usa alvo::bibliotecas_do_sistema(), a lista já mantida pelo projeto.
Os dois testes HIR da fixture passaram no Windows, inclusive com dois quadros
locais; log target/unwind-quadros-fixture.log. O novo teste LLVM passou: verifica
inventário de dois quadros, fechamento LIFO no braço estrangeiro e objetos
Linux O0/O2; log target/unwind-quadros-llvm.log. A ligação/execução macOS
precisa ser repetida.

Em preparação local: certificado de cleanup passa a guardar a pilha de quadros
locais no pouso e o emissor fecha esses quadros em ordem LIFO no braço estrangeiro.
Fixture adicional guarda o mesmo Mint em dois quadros, transfere um token por
Move e carrega um owner independente; caminhos Dart/normal fecham os quadros,
o unwind estrangeiro deve liberar o owner e ambos os quadros antes da coleta.
Não amplia a cobertura para quadros importados, aliases, Retoma com quadros
ativos, forced unwind, finally, suspensão ou cancelamento. Prova nativa pendente.
Suítes locais completas de biblioteca: emissor 202 aprovados/7 ignorados,
runtime 174 aprovados/3 ignorados; sem falhas. Logs target/unwind-quadros-emit-lib.log
e target/unwind-quadros-runtime-lib.log. Teste de regressão do diagnóstico e
grafos ARC aleatórios incluídos. Ignorados não foram convertidos em passes.
Fixture recompilada: 2 testes aprovados. Doctests: emissor 49 e runtime 35
aprovados; log target/unwind-quadros-doc.log. YAML e script POSIX do workflow
validados. Aviso preexistente atribuir_slots não utilizado continua presente.


Prova de unwind C++ com heap real preparada para CI Unix:
`arc_foreign_unwind` gera HIR com contrato auditado de print_handle Borrow,
catch preparado e Mint Owned; depois injeta o hook C++ por troca explícita de
símbolo no IR. Hook conserva assinatura/ownership, lança objeto C++ e não cria
pendência Dart. Fixture confere identidade nativa, destrutores, catch Dart não
executado, topo de raízes restaurado, owner consumido e Mint morto após coleta.
É fault injection, não chamada C++ proveniente do lowering FFI completo.
CI gera objetos LLVM O0/O2 e executa ARC/tracing sob ARC_CONFERIR/GC_STRESS,
com conferência de efeitos na geração; artefatos IR/C++/stdout são publicados.
Execução nativa desta fixture ainda pendente; teste HIR Windows e sintaxe C++
no WSL passaram. Não valida forced unwind, finally ou inlining observado.
Diagnóstico `dartforge_arc_observe_heap_v1` consulta páginas registradas e
inventário de owners sem dereferenciar endereço externo nem criar referência:
bit 0 bloco vivo, bit 1 owner do código. Catálogo parcial passa a 29 contratos,
argumento i64 Native (pode já estar liberado), resultado u8 escalar, efeitos0/0/0.
Teste cobre tracing/ARC × estresse direto ligado/desligado, dois tokens,
último release/coleta e endereços fora do heap; 33 doctests iniciais passaram.
Conferência de efeitos também precisa de unwind: snapshot de profundidade na
entrada e restauração no pouso abandonam externs atravessadas e suas proibições
de coleta. Não conferem saída normal nem alteram pendência. Teste nested do
runtime e corpo LLVM em DARTFORGE_EFFECTS=check passaram, objetos Linux O0/O2;
35 doctests do runtime e dois testes do catálogo passaram. Logs
`target/observacao-heap-*`, `target/efeitos-unwind-*`, `target/unwind-estrangeiro-*`.
Fonte anterior 24efc32a passou também na prova de rastro explícito macOS;
16 stdout Linux/macOS conferidos byte a byte em `target/ci-38015289408`.
Job Linux completo da CI 38015289408 terminou com sucesso; não contém esta fixture.

Cleanup estrangeiro certificado no emissor: preparação ARC liga o inventário
validado ao corpo e às tabelas (invokes/pousos/saídas/conferência de pilha).
Campos privados impedem inventar owners; LLVM/materialização rejeitam certificado
obsoleto após mutações. Catch certificado Unix usa personalidade cleanup,
landingpad cleanup+catch, guarda de seletor e libera owners locais antes de
resume. Restaura quadro de raízes anterior e fecha entrada de rastro; Retoma
pura também fecha quadro/rastro antes de resume, evitando topo apontando para
um quadro abandonado quando o próximo tratador é estrangeiro.
Teste do corpo misto confirma release único no braço estrangeiro, par original,
Phi do catch Dart e rejeição de corpo/invoke obsoleto; LLVM gerou objetos Linux
O0/O2. Suíte local passou: 201 unitários (sete ignorados), nove contratos
(13 ignorados), dois de depuração e 49 doctests. Após ajuste de fechamento,
42 testes LLVM passaram novamente. Logs `target/cleanup-estrangeiro-*`.
Ainda falta execução de estrangeira/forced unwind com heap real e prova de
morte final. Catch com abertura de quadro proprietário é rejeitado até
integrar seu fechamento; SEH/statepoints/finally/cancelamento continuam pendentes.
Em Windows o certificado é conferido, mas a emissão permanece no perfil legado.
CI 38015289408 da fonte anterior 24efc32a passou na prova do rastro explícito
Linux; macOS continua compilando. Essa fonte não contém o cleanup certificado.

Inventário de owners na entrada dos pousos: análise linear agora entrega
snapshots validados e ordenados por ValueId, após consumo da aresta de erro,
sem resultados exclusivos do sucesso. API `tokens_na_entrada_dos_pousos`
rejeita inventário/CFG inválido e pouso sem entrada alcançável. Preparação
atômica recalcula `PlanoFuncaoDart.owners_no_pouso` após conferir o corpo;
metadados anteriores são substituídos somente no sucesso da transação.
Teste confere argumento consumido ausente, owner independente presente e
resultado ausente no erro, além de substituição de snapshot obsoleto.
95 testes ARC e 48 doctests passaram; logs `target/owners-pouso-*`.
Este inventário prepara geração de cleanup estrangeiro: o emissor ainda
retoma sem liberar owners locais no braço estrangeiro do catch misto.
Não fecha essa pendência nem cobre quadros/escopos/finally/cancelamento.

Evidência Unix da captura Owned congelada em
`bench/results/2026-10-09-excecao-owned-unix-ci`: fonte 7418650d,
CI 38013924479, 16 execuções Linux/macOS e 32 arquivos IR/stdout byte exatos.
JSON registra hashes, jobs, fonte e digests. Auditoria dos oito erros confere
captura antes de release original, coleta e impressão da cópia. Fonte anterior
à captura Owned do rastro; não estende a prova a morte final/inlining/unwind
estrangeiro/SEH/statepoints ou integração completa da especificação ARC.

Prova AOT com rastro explícito: caller cria StackTrace Owned conhecido e
empresta-o ao propagador/callee. Lançamento publica esse rastro; catch captura
valor e rastro Owned e compara as duas identidades. Depois libera os owners
originais, limpa a pendência, coleta e retém a captura do rastro novamente:
ARC_CONFERIR confere sua validade após coleta. Oito variantes Windows
simples/misto × ARC/tracing × normal/erro passaram com código zero e MAX+LF
byte exato; dois caminhos ARC estritos também passaram. Geração com
DARTFORGE_EFFECTS=check, execução ARC_CONFERIR=1, BER=0, GC_STRESS=1.
Logs/IR/stdout `target/rastro-aot-*`. CI Unix usa o mesmo exemplo e requer
nova execução desta fonte. Não observa morte final em AOT nem identidade do
objeto nativo de unwind; não cobre rastro implicitamente capturado da pilha,
SEH/statepoints ou limpeza de owners do catch em unwind estrangeiro.
Prova anterior de captura Owned da exceção, fonte 7418650d, passou nas oito
variantes de cada plataforma Linux/macOS na CI 38013924479. Downloads em
`target/ci-38013924479/{linux,macos}`: 16 stdout MAX+LF conferidos byte a byte,
IR com getter Owned e resume. Job Linux completo terminou com sucesso.

Captura Owned do StackTrace: `dartforge_arc_stack_trace_owned_v1` retém o rastro
antes de devolver o handle, sem safepoint entre leitura e retenção. Quando
não há rastro corrente, cria um StackTrace sem instalá-lo para um throw futuro.
`dartforge_arc_throw_with_stack_trace_ref_v1` publica valor/rastro Ref emprestados
em raízes runtime independentes, sem consumir os owners do chamador.
Catálogo parcial passa a 28 contratos. Teste do runtime passou em tracing/ARC:
identidade do valor e rastro, sobrevivência ao clear/liberação dos originais,
texto não vazio após coleta e morte dos dois objetos após o último release.
32 doctests do runtime, 95 testes ARC e dois testes do catálogo passaram.
Logs `target/rastro-owned-*`.
Ainda falta integrar a observação do rastro na prova AOT de unwind; este teste
é do protocolo runtime, não prova SEH/Itanium, finally ou identidade nativa.
CI 38013924479 da fonte 7418650d já passou pelas oito variantes AOT Linux
com captura Owned da exceção; job Linux completo terminou com sucesso; workflow e macOS continuam em execução.

Leitura Owned da exceção: nova ABI `dartforge_arc_exception_owned_v1` conserva
identidade de Ref, encaixota escalares e entrega owner independente antes de
sair do empréstimo do heap. Não consome pendência/rastro; null sem pendência.
Contrato auditado 26: resultado Ref Owned, sem parâmetros/falha nova,
retenção persistente zero, invalidação conservadora e efeitos 1/0/0.
Runtime testou Ref, Mint/Smi, double, bool e null em tracing/ARC sob coleta:
a cópia sobrevive ao clear/liberação original; Mint/double morrem após o
último release. Dois testes passaram e 30 doctests do runtime passaram.
Passaram também 94 testes ARC, 42 LLVM e 47 doctests do emissor.
Prova AOT agora compara identidade do valor Ref antes do clear, libera o
owner original, coleta e imprime o Mint capturado. Erro passa a imprimir MAX;
troca de identidade imprime null e reprova stdout. Não observa identidade do
objeto nativo de unwind, rastro ou morte final em AOT. Oito variantes Windows
simples/misto × ARC/tracing × normal/erro passaram, código zero e MAX byte exato,
sob ARC_CONFERIR=1, BER=0, GC_STRESS=1. Variante erro ARC misto também passou
com DARTFORGE_EFFECTS=check na geração; CI ativa essa conferência para os
próximos oito casos Unix. Logs `target/ownership-excecao-owned-*` e
`target/excecao-owned-*`. Execução Unix da leitura Owned ainda pendente.

Resume simples/misto executado também em macOS ARM64: CI 38011679848, fonte
5445efd54f24100d4cf3b1cd821de340bd115d85, passos AOT Linux/macOS terminaram
com sucesso nas 16 execuções. Artefatos baixados e conferidos byte a byte:
normal MAX+LF, erro vazio nessa fonte anterior à leitura Owned. Perfil misto
conferiu seletor catch no runtime real, com heap e cleanup/resume interno.
Congelado em `bench/results/2026-10-09-arc-resume-misto-unix-ci`, 32 IR/stdout,
fonte/jobs/digests/hashes. Sem prova direta de inlining, estrangeira/forced
unwind com owners do catch, rastro/identidade nativa/morte final ou SEH/statepoints.
Não valida lowering completo, §§27–34 completos nem gate ARC/A0.


Fechamento parcial da CI 38010046028: Linux terminou com sucesso, incluindo
as quatro provas ARC resume. macOS falhou no novo doctest de emit_all, antes
da prova AOT: o exemplo exigia target triple explícito, mas o cabeçalho macOS
é genérico e o gerador completa o alvo. Corrigida a asserção para presença de
declarações. Prova de geração do corpo escolhe Linux x86-64 ou aarch64 conforme
a arquitetura nativa registrada no LLVM, sem exigir backend x86 no runner ARM.
CI Unix passa a testar emit_native com embedded-llvm explicitamente, cobrindo
a geração O0/O2 dos dois corpos de cleanup/catch. Log macOS terminal obtido
pela API do job 114087818227 em `target/ci-38010046028/macos-falha.log`.
Não valida execução macOS; requer nova rodada com a correção e a variante mista.


Primeira execução AOT Linux do resume ARC integrado: o passo da CI 38010046028
(job 114087818147, fonte f626bb6887cdefb079eeba0910625111905e63bc) passou nas
quatro variantes automático ARC/tracing × normal/erro. Runtime real com Mint,
ARC_CONFERIR=1, BER=0, GC_STRESS=1; códigos zero e stdout byte exato.
Download auditado: propagador usa personalidade de cleanup, landingpad cleanup,
release antes de resume do par recebido, sem df.throw nesse pouso.
Congelado em `bench/results/2026-10-09-arc-resume-linux-ci`, oito IR/stdout,
hashes de fontes/arquivos e digest do artefato Actions. IR é entrada, não
otimizado; não há executáveis nem versão exata da libLLVM no artefato.
O workflow inteiro e a prova macOS ainda estavam ativos/pendentes nesta
conferência. Fonte anterior à variante mista e à guarda de seletor do catch;
não testa estrangeira/forced unwind/catch no perfil novo, não observa
identidade/rastro/morte final em AOT nem valida o lowering completo/gate ARC/A0.


Catch no perfil de cleanup Itanium: quando a mesma função contém Retoma e
catch preparado, o pouso catch agora compara o seletor com
llvm.eh.typeid.for(null). O caminho não selecionado retoma o par original,
antes de entrar no tratador Dart. Isso é necessário quando o inliner combina
cleanup e catch. Atualiza o rótulo predecessor dos Phis do sucessor do pouso.
Passaram 42 testes LLVM, incluindo objetos Linux O0/O2 do corpo real com
catch+cleanup e Phi; não é execução de estrangeira com heap ARC.
A prova AOT `arc_guarded_returns` aceita `automatico misto`: o caller
mantém seu catch e ganha um segundo invoke automático sem tratador, ficando
com ambos os modos em Unix. Quatro variantes mistas Windows passaram sob
ARC_CONFERIR=1, BER=0, GC_STRESS=1, stdout byte exato (normal MAX, erro vazio).
Logs `target/retoma-misto-win-*` e `target/retoma-catch-*`.
CI passa a rodar simples/misto × ARC/tracing × normal/erro em Linux/macOS,
antes dos testes lentos do SDK. A execução Unix dessas variantes está pendente.
A prova não certifica estrangeira/forced unwind com owners locais do catch:
limpeza desses owners fora do caminho Dart ainda precisa de CFG/metadata ARC.
Também não certifica morte final, rastro/identidade em AOT, SEH/statepoints,
pipeline semântico completo ou o gate de desempenho.


Integração de resume ao protótipo ARC: HIR ganha saída Retoma, cujo Return
é placeholder sem transferência. A preparação automática usa esse modo
em Unix e mantém Lanca em Windows, até a integração SEH. O emissor escolhe
`dartforge_personality_cleanup_itanium`, emite landingpad cleanup e retoma
o par original. Drops ARC precedem resume; não chama df.throw nesse pouso.
Tokens/emissor/materialização conferem pouso não inicial, um invoke estático,
entrada exclusivamente excepcional, retorno canônico e corpo só de drops.
Rejeitam limpeza da pendência e demais operações no cleanup; emissor rejeita
statepoint token e Retoma no módulo público Windows. Esses limites continuam
pendências de integração, não substituem SEH/statepoints/finally/cancelamento.
A materialização preserva Retoma em vez de recalcular Lanca/Guarda.
Passaram 93 testes ARC, 41 LLVM, seis tabelas, 47 doctests e quatro testes dirigidos,
incluindo rollback/idempotência e materialização. O corpo real do emissor
gerou objetos Linux O0/O2 no LLVM embutido. Isso é geração, não execução
com heap ARC Unix; a CI agora executa a prova AOT automática em Linux/macOS,
ARC/tracing × normal/erro sob ARC_CONFERIR=1, BER=0, GC_STRESS=1 e compara
stdout. Essa execução ainda está pendente. AOT Windows ARC/tracing × normal/erro
passou nas quatro variantes, código zero e stdout conferido byte a byte;
Windows mantém Lanca. Logs locais `target/retoma-*`.
A primeira execução com LLVM embutido não iniciou por falta da DLL no PATH;
repetida com o binário da toolchain instalada, passou. Não certifica identidade
/rastro/morte final em AOT nem o pipeline semântico completo ou gate ARC/A0.

Pesado 38003888914 terminou com sucesso no commit
351c99d7b5a26a5d6ecb3f98012485cce2350499. Relatórios baixados em
`target/ci-38003888914`: A1/B0/B1/ARC Linux e macOS 238/238; JIT×AOT
238/238 sem divergências (sete sem IR, iguais por construção); dart:io
Linux/Windows 130/130 e SDK da fonte 238/238. Seis casos interop têm DART!
por falta de oráculo Dart nativo. O placar Windows 476/476 soma AOT+ARC,
não 476 programas distintos. Essa rodada precede Guarda explícita,
invokes automáticos e Itanium/Retoma; não valida as integrações novas nem
DDC portátil e não altera o gate de desempenho.


Itanium agora percorre as cadeias de ações SLEB128: separa índice de ação,
seletor catch e presença de cleanup, incluindo registros filtro zero em
índice positivo e elos negativos relativos ao campo de deslocamento.
Pouso misto instala cleanup para estrangeira/forced unwind sem tratá-la
como Dart; no quadro handler Dart, entrega o seletor da ação correspondente.
O protocolo legado mantém suas decisões. Passaram 12 testes LSDA/fases e
29 doctests; trechos reais compilados como metadados Linux x86-64/macOS ARM64.
Quatro provas Linux O2 passaram: puro, inlining Dart, estrangeira em pousos
mistos e estrangeira com inlining. Comprovam objeto original, cleanups 1→2
uma vez cada e seletores corretos; a última retoma até o handler C++ real,
sem capturar estrangeira como Dart. IR otimizado confirma que as funções
de cleanup foram incorporadas e o teste de seletor antes do resume foi
preservado. Artefatos em `bench/results/2026-10-09-itanium-acoes-mistas-linux`;
logs `target/ownership-itanium-acoes-*`. A estrangeira é objeto de fixture,
não throw C++ de um objeto de linguagem. Sem heap/pendência/rastro/drops ARC
ou fonte Dart. O emissor ainda usa o protocolo legado; filtros tipados,
statepoints, SEH/funclets e integração no pipeline continuam pendentes.
Forced unwind tem somente prova unitária; macOS somente compilação.
Não altera o gate ARC/A0 nem conclui o objetivo completo.

Personalidade Itanium de cleanup: nova entrada Unix
`dartforge_personality_cleanup_itanium` separa cleanup puro de catch-all
Dart. Cleanup não para a busca; instala na fase de unwind, inclusive para
exceção estrangeira/forçada. Catch-all só instala o quadro selecionado para
exceção Dart. Entrega o objeto original ao landingpad e seletores 0/1,
sem reinicializar o objeto entre os pousos. A entrada legada conserva as
decisões e o par zero dos pousos/statepoints atuais. Símbolo registrado
somente em Unix; efeito auditado 0/0/0 no novo callback.
Passaram nove testes LSDA/fases/portas e 29 doctests do runtime. Os trechos
reais compilaram como metadados Linux x86-64 e macOS ARM64. Prova executada
em Linux x86-64 via WSL: dois cleanups aninhados usam resume real, cada um
uma vez, na ordem interno→externo; ambos e o handler recebem o mesmo objeto,
com seletores corretos. LLVM O0 e O2 terminaram com código 0; funções
noinline, sem prova de inlining. Artefatos
em `bench/results/2026-10-09-itanium-cleanup-resume-linux`; logs
`target/ownership-itanium-*`. A fixture usa objeto nativo com classe Dart,
sem heap/pendência/rastro Dart. O emissor ARC ainda não usa essa entrada;
integração de resume aos cleanups ARC, ações tipadas/pousos mistos/statepoints,
SEH/funclets e execução macOS continuam pendentes. Exceção estrangeira e
forced unwind foram conferidos só na decisão unitária. Não certifica ARC
completo nem altera o gate de desempenho.

Preparação de resume nativo: o leitor de LSDA agora devolve pouso e índice
de ação, em vez de descartar a ação. Preserva zero (cleanup sem ação tipada)
e índices positivos da tabela; ainda não interpreta cadeias de ações.
Endereços udata4 continuam lendo a ação em uleb128. A comparação de faixa
usa diferença após conferir o início, evitando overflow em início+comprimento.
Os testes de LSDA/portas agora também estão habilitados em Unix; seis testes
passaram localmente no Windows, incluindo índice 129 e limite u64::MAX.
Logs `target/ownership-lsda-*`. AOT ARC normal/erro da variante automática
reconstruído com o runtime alterado passou sob stress e auditoria, código 0,
IR e stdout idênticos aos artefatos anteriores. As personalidades ainda tratam
os pousos como handlers. Auditoria confirmou que Itanium entrega zero no
par do landingpad e reinicializa o objeto de unwind em df.throw; SEH
ignora a fase de unwind e usa RtlUnwindEx na busca. Preservar esse objeto,
distinguir cleanup/handler nas fases e emitir resume/funclets continua
pendente; nenhum resume nativo foi certificado. Referência consultada:
[Exception Handling in LLVM](https://llvm.org/docs/ExceptionHandling.html).

Preparação automática de chamadas Dart: nova API
`preparar_arc_funcoes_dart` cria invoke, continuação normal e pouso local
para chamadas diretas falíveis sem sítio prévio. O pouso libera exatamente
os tokens disponíveis, fecha escopos léxicos ativos e propaga via Lanca.
Sítios existentes preservam seus tratadores; Phis e eventos em arestas
seguem o último segmento do bloco original. A transação cobre todos os
corpos/planos, inclusive falhas depois de divisões anteriores. Conferência
de pendência logo após chamada exige sítio já preparado para não omitir
catch/finally. A API estrita anterior mantém seu contrato.
Passaram 92 testes ARC, 39 LLVM e 46 doctests. Nova variante `automatico`
da prova Guarda adiciona um propagador sem classes/efeitos/invokes/pousos
manuais, com escopo 7 e token local independente. Quatro AOT ARC/tracing
normal/erro sob auditoria e stress passaram: normal imprime i64::MAX;
erro executa o pouso gerado, libera seu token e alcança o tratador externo.
O conjunto insere (1 retenção, 5 drops nos caminhos compilados). Artefatos
em `bench/results/2026-10-09-arc-invokes-automaticos-aot`; logs
`target/ownership-invokes-*`. O tratador externo continua explícito.
Esta prova não certifica resume nativo/identidade do objeto de unwind/rastro
em todas as plataformas (§20.4), morte final, ausência geral de vazamentos
ou ganho de runtime. Quadros proprietários abertos exigem cleanup explícito;
contexto por vivacidade, finally/dispatch indireto/cancelamento/suspensão e
integração no pipeline padrão continuam pendentes. O objetivo ARC completo
e o gate ARC/A0 permanecem abertos.

Resumos ARC agora compartilham com LLVM a decisão de contexto explícito:
leitura/limpeza de pendência, operações em linha que usam contexto,
pousos e marcas nas tabelas implicam conferência de pilha no prólogo.
Antes, uma folha que só lia ou limpava pendência podia ser considerada
infalível, omitindo a exigência de saída excepcional nos callers.
A preparação conjunta propaga essa falha transitivamente; o resumo
isolado também a registra. Regressão cobre pending/clear na cadeia
caller→intermediária→folha, rejeição atômica e emissão de df.throw
no prólogo. Passaram 87 testes ARC, 39 LLVM e 45 doctests; quatro AOT
ARC/tracing normal/erro sob stress e auditoria terminaram com código 0,
LLVM e stdout idênticos ao arquivo da prova Guarda anterior. Logs
`target/ownership-contexto-*`. Não induz estouro de pilha nativo nem
gera os invokes dos callers; contexto adicional por quadros calculados
por vivacidade e integração no pipeline padrão continuam pendentes.

Separação de múltiplos retornos Guarda: o passe guarda os índices originais
dos blocos e acrescenta as folhas, eliminando a busca linear por retorno.
A localização das saídas passa de O(G*B) para O(B+G), sem medir ganho no
runtime. Regressão com IDs fora da ordem física comprova transferência
owned no sucesso e dois drops nos erros, além de idempotência. Falha no
escopo da segunda saída ou esgotamento SSA depois de separar a primeira
preserva todos os corpos e planos originais. Passaram 86 testes ARC;
log `target/ownership-guardas-multiplas-test.log`. Integração no pipeline
padrão e demais requisitos ARC continuam pendentes.

Retornos Guarda: a preparação conjunta separa a conferência de pendência
em dois blocos, mantendo o operando do resultado só no sucesso e marcando
o erro como Lanca. Fecha o borrow depois da retenção normal, transportando
os eventos de saída para ambas as folhas. IDs reservam definições e
metadados, sem wrap. Publica CFG, tabelas e escopos somente após verificar
o conjunto. Passaram 85 testes ARC, 39 LLVM e 45 doctests.
Nova prova `arc_guarded_returns`: quatro execuções AOT ARC/tracing sob
stress e auditoria, código 0. Normal imprime i64::MAX após liberar a cópia
de retorno e coletar; erro desenrola do callee por df.throw, chega ao
landingpad do invoke no caller, limpa a exceção e libera o token da fábrica.
Artefatos em `bench/results/2026-10-09-arc-guardas-unwind-aot`; logs
`target/ownership-guardas-*`. Morte final, geração dos invokes do caller,
arestas gerais, finally/cancelamento/suspensão e pipeline padrão pendentes.

Nova Heavy `38003888914`, suíte nativo com dois fragmentos, iniciada sobre
`351c99d7b5a26a5d6ecb3f98012485cce2350499` depois de conferir ausência de
rodadas Heavy ativas/em fila/em espera. Inclui catálogo 25 e preparação
conjunta até Lanca; não inclui esta separação de Guarda. A rodada anterior
foi encerrada e conferida conforme o registro abaixo.

Heavy `37998421467` concluído com sucesso sobre
`116f6bed11bcc79d5d25f3e560db5bf0c07bd8a6`. Relatórios e logs conferidos em
`target/ci-37998421467`. Linux: AOT/A1/B0/B1/ARC 238/238 em
69,8/39,1/84,3/87,7/38,4 s; io 130/130 em 52,2 s. macOS:
58,7/37,5/60,9/63,3/35,6 s; io 130/130 em 51,6 s. JIT×AOT Linux/macOS:
238/238, zero divergências, sete sem IR por construção e zero timeout
nos dois perfis, em 41,6/53,4 s. SDK da fonte: AOT 238/238 em 39,5 s e
JIT×AOT com o mesmo placar em 87,8 s. Windows: ARC 238/238 em 58,0 s,
ARC stress 57,2 s, io 130/130 em 75,1 s; A1 53,8/52,5 s, B0
102,9/90,8 s e B1 117,3/102,9 s (normal/stress); stress base 73,7 s.
Dois fragmentos base 119/119 em 38,5 s cada. Mapas/sabotagens B0: quatro
testes passaram em 768,16 s. SDK: recarga 2/2 (30,34 s), Script/spawnUri
1/1 (0,87 s), produção autocontida 1/1 (33,03 s). macOS recarga: 7/7.
Os seis DART! de interop não têm oráculo nativo válido; 238/238 não prova
paridade de todos os oráculos. Unix usou `gerar-dart-sdk-js: false`, sem
prova da geração portátil de DDC nesta rodada. Ela antecede as mudanças
de Mint mortal, catálogo 25 e preparação conjunta ARC.

Saídas ARC por tabelas: `Lanca` não transfere resultado ao caller. O passe
agora normaliza seu operando para o placeholder antes da classificação e
libera todos os tokens locais, incluindo o antigo resultado. O verificador
rejeita saída Lanca não canônica, metadado obsoleto ou saída sem Return;
`Guarda` exige separar sucesso/erro explicitamente no CFG antes do ARC.
A normalização vale para função isolada e preparação conjunta; saídas
marcadas também propagam falha aos chamadores nos resumos.
Regressão confirma nenhuma retenção e um drop do antigo resultado, ordem
release→df.throw no LLVM, idempotência e preservação atômica na rejeição
de Guarda. Passaram 82 testes ARC, 39 LLVM e 45 doctests, logs
`target/ownership-saidas-lanca-*`. Não houve execução nativa desse unwind
por tabelas; a prova AOT anterior executa o protocolo de pendência runtime.
Separação automática de Guarda e integração no pipeline continuam pendentes.

Resumos ARC agora respeitam `TabelasDaFuncao.confere_pilha`: o prólogo
LLVM marcado pode lançar mesmo que o corpo seja uma folha sem operações
falíveis. A preparação conjunta propaga essa falha aos chamadores por
lista de trabalho, em O(V+E); o resumo individual também inclui a marca.
Regressão: folha marcada torna caller→intermediária→folha falível, rejeita
o primeiro caller sem saída e preserva todos os corpos/mapas. Com invoke
e pouso preparados, a chamada verifica e transfere o resultado só no
sucesso. 81 testes ARC e 45 doctests passaram, logs
`target/ownership-pilha-resumos-*`. Esta é uma prova dos contratos sobre
HIR, sem induzir estouro de pilha nativo; produção completa dos metadados,
demais origens de contexto e integração no pipeline continuam pendentes.

Preparação conjunta: o índice de símbolos dos resumos agora é construído
uma vez e reutilizado por todos os corpos. Antes havia uma reconstrução
com N entradas por função, impondo trabalho quadrático em N, inadequado
para o SDK. A validação de resumos repetidos permanece no construtor do
índice e nenhuma assinatura ganha contrato implícito. 80 testes ARC e
45 doctests passaram; AOT ARC normal e tracing no segundo erro passaram
sob stress, com LLVM e stdout idênticos à rodada anterior. Logs
`target/ownership-indice-funcoes-*`. Isto não é uma medição de ganho no
runtime nem altera o gate ARC/A0; a API ainda não está no pipeline padrão.

A preparação conjunta agora classifica resultados das chamadas internas,
depois constantes/aritmética/Phis/runtime cobertos, e só então confere os
argumentos das chamadas. Isso remove mapas manuais de argumentos ordinários
cobertos: regressão combina resultado de chamada, Phi I64, soma e nova
chamada em blocos fora de ordem, sem classes/efeitos no caller. O parâmetro
I64 do callee continua exigindo contrato semântico explícito. Trocar entrada
do Phi por Ref é rejeitado sem publicar o conjunto.
80 testes ARC, 45 doctests e oito execuções AOT sob stress passaram.
Stdout e LLVM são idênticos à preparação conjunta anterior, nos caminhos
normais e nos três erros em ARC/tracing. Logs
`target/ownership-funcoes-argumentos-*`. Ainda faltam produtores para
operações/ABIs não cobertas, CFG/escopos e integração no pipeline padrão;
não há nova medição de desempenho nem conclusão do objetivo completo.

Preparação conjunta Dart: `inserir_arc_funcoes_dart` recebe conjunto fechado
de funções e planos, cria contratos provisórios de chamadas internas e
publica somente após inserir/verificar todos os corpos. Usa o resumo de
exceções existente, inclusive ciclos e conferência de pilha. A cadeia
caller→intermediária→folha funciona com callees posteriores; erro no último
corpo preserva todos os corpos e mapas; recursão sem saída excepcional é
rejeitada. 79 testes ARC e 45 doctests do emissor passaram.
O exemplo AOT agora prepara caller e callee juntos nas variantes automáticas:
insere uma retenção no callee e três drops no caller. Oito execuções ARC/
tracing sob stress passaram, cobrindo caminho normal e os três erros. Os
seis LLVM excepcionais são byte a byte iguais aos artefatos de
`2026-10-09-arc-excecao-ref-aot`; logs `target/ownership-funcoes-dart-*`.
O controle sem cleanup continua rejeitado antes do IR. Esta API ainda exige
CFG e escopos preparados, contratos dos argumentos escalares ordinários e
descritores/proveniência; não foi ligada ao pipeline padrão, nem conclui
cleanup em arestas, finally, cancelamento ou suspensão.

Exceção Ref e cleanup executado no AOT: a nova ABI
`dartforge_arc_throw_ref_v1` recebe borrow Ref, não consome token do código
e publica a raiz da exceção pendente; pode alocar o rastro. Catálogo agora
com 25 contratos, incluindo `dartforge_exception_clear`. O teste do runtime
passou em ARC e tracing com auditoria/stress: Mint vivo após release do
código e coleta enquanto pendente; morto após clear e nova coleta.
As variantes `retorno-mortal-erro-1/2/3` executaram os três retornos de erro
da prova AOT em ARC e tracing: seis códigos 0, saídas com zero/uma/duas
linhas, LLVM com clear antes dos drops inseridos. Artefatos e hashes em
`bench/results/2026-10-09-arc-excecao-ref-aot`.
Passaram 77 testes ARC, 39 LLVM, dois do catálogo, 29 doctests runtime e
43 do emissor. Logs `target/ownership-excecao-ref-*`.
O terceiro lançamento usa null pela ABI interna, não prova `throw null`
de Dart. Morte final no AOT, invoke de callee Dart nativo e integração
no pipeline padrão continuam pendentes, assim como o restante do objetivo.

Chamadas Dart falíveis com CFG excepcional preparado: regressão combina
resumo do callee, produtor de CallStatic, tabela invoke e inserção de cleanup.
O temporário passado por borrow recebe drop nas duas saídas; o resultado
Owned é transferido apenas no sucesso e nunca liberado no erro. Tentar
devolvê-lo no erro é rejeitado com ARC003 por não atravessar a aresta de
sucesso, preservando função e mapas. 77 testes ARC passaram
(`target/ownership-chamada-falivel-cleanup-test-final.log`). Esta prova é
do passe sobre HIR, não execução nativa de exceção; o contrato da externa
usada no corpo do callee permanece premissa explícita do teste.

O produtor de chamadas Dart agora classifica todos os resultados antes
de conferir argumentos, preservando atomicidade e removendo dependência
da ordem física dos blocos. Regressão: b0→b2→b1, com b1 armazenado antes
de b2, usa o resultado escalar de `criar` em `identidade_escalar` sem
classes manuais no caller; o wrapper completo verifica a cadeia.
Trocar esse argumento por parâmetro I64 sem contrato continua rejeitado,
sem publicar as classes/efeitos já preparados. 76 testes ARC passaram
(`target/ownership-chamadas-ordem-test.log`).

Chamadas Dart diretas: `verificar_contrato_funcao_dart` cria resumo com
campos privados após verificar ownership, convenção e representação dos
retornos do callee. `produzir_chamadas_dart` confere símbolo, aridade,
tipos reais e contratos conflitantes antes de publicar mapas atomicamente.
Ref é emprestado na entrada e Owned no resultado; argumentos não Ref
exigem Trivial semanticamente classificado. Chamadas diretas no corpo
conservadoramente tornam o resumo falível; o caller precisa de saída
excepcional preparada. Resumos devem ser refeitos após mudança do corpo;
contratos externos fornecidos e proveniência continuam premissas.
A prova `arc_strong_slots` usa agora esse resumo, sem contrato manual de
CallStatic. AOT com Mint mortal e cleanup automático passou em ARC auditor
sem berçário e tracing sob stress: i64::MAX, 42, null, código 0.
75 testes ARC e 43 doctests passaram. Logs locais
`target/ownership-chamadas-dart-*`. Ainda falta integrar no pipeline comum,
produzir os demais contratos e cobrir dispatch indireto/closures/stubs.

Regressão de cleanup com saídas Ref distintas: a saída que devolve um
Owned transfere esse token sem drop; a saída que devolve o parâmetro
borrowed retém o retorno antes de liberar os dois temporários Owned.
O teste confere os operandos exatos, ordem determinística e idempotência.
Suíte dirigida `cargo test --locked -p dartforge-emit-native --lib
optimize::arc`: 72 testes passaram. Ainda não integra o passe no pipeline
comum: contratos de chamadas e proveniência produzidos pelo lowering
continuam necessários.

Prova AOT do cleanup automático de saídas: a variante
`retorno-mortal-auto-cleanup` de `arc_strong_slots` deixa os três retornos
de erro sem drops manuais. `inserir_arc_saidas_dart` insere exatamente
três liberações e nenhuma cópia; LLVM b1/b3/b5 libera v8/v14/v18 antes
do retorno. ARC com auditoria, berçário desligado e GC stress, e tracing
com GC stress, terminaram com código 0 e saída idêntica (Mint i64::MAX,
42, null). O controle sem cleanup continua rejeitado antes do IR pelo
token v8 pendente. Artefatos em
`bench/results/2026-10-09-arc-cleanup-saidas-aot`.
Os caminhos de erro foram verificados no IR, sem induzir erro de impressão;
esta prova não observa a morte final no AOT nem conclui o lowering padrão.

O resumo de 2026-10-08 abaixo precede os commits `c0cded84` (mapas por
`"deopt"`), `f8dfedbf` (metadados ARC por página) e `72a3212c` (decisão dos
jovens e cascata). As medidas atualizadas do ARC estão em
`docs/ARC-IMPLEMENTACAO.md`: ARC/A0 passou de 2,81 para 2,13 na média
geométrica da rodada ali descrita; ainda há trabalho para aproximar de A0.
O benchmark completo dos mapas está registrado na especificação §13.16:
32 núcleos, sete execuções alternadas presas ao núcleo P, B0/A0 = 0,972 e
B1/A0 = 0,961. Esses tempos são da máquina Windows.

A revisão do analisador avançou até `86aa8d2c`: criação por alias genérico
sem argumentos escritos infere pelos parâmetros e limites do alias. A
retomada acrescenta regressões de contexto aninhado (`D<X> = C<List<X>>`),
incluindo `new`, construtor nomeado e argumentos explícitos incompatíveis.
O Dart 3.6.2 aceita os cinco casos positivos sem diagnóstico.
O caso negativo revelou que `D<int>()` ainda usava a expansão sem argumentos
(`C<List<dynamic>>`); a correção aplica os argumentos escritos ao alias.
O oráculo exige `C<List<int>>` nos dois relatos de retorno incompatível.
Verificação: `cargo test --locked --release -p dartforge-types --test bodies`
passou 58 testes; os três testes de SDK/projetos externos permanecem ignorados.
O último placar preservado do histórico (`E:\dftemp\placar_det326.txt`,
2026-10-09 00:25) registra 22.917/23.012 na posição exata, 22.900 mensagens
iguais, FP 0, FN 79 e 16 posições erradas. Ele antecede as duas últimas
correções de criação por alias; ainda não é uma medição deste HEAD.
O relatório da rodada `37896396380`, sobre `719e94cc`, atualiza o placar:
22.920/23.012 na posição exata, 22.903 mensagens iguais, FP 0, FN 76 e
16 posições erradas. Determinismo idêntico com 1, 4 e 8 trabalhadores;
9.440 arquivos, 541/607 códigos com posição e mensagem 100%. Relatório
conferido: três FN a menos que no placar anterior.

ARC: o registro dos jovens protegidos percorre diretamente as raízes, em vez
de consultar os metadados de todos os jovens. Grafos aleatórios: 3.000 sementes
em cada um dos três modos, sem falhas; suíte completa do runtime: 108 testes
passaram, três microbenchmarks ignorados. Medida dirigida em produção, cinco
execuções alternadas no núcleo P: árvores 888,131 → 865,112 ms; lista ligada
179,882 → 172,090 ms. Mesmos resultados em todas as execuções. Detalhes e
limites em `docs/ARC-IMPLEMENTACAO.md`. A rodada completa com estas mudanças
está registrada abaixo.

O índice dos metadados agora usa inverso modular por página, sem divisão na
consulta, recalculado ao reformatar a página. Suíte: 112 testes passaram,
três microbenchmarks ignorados; grafos aleatórios: 3.000 sementes por modo.
O benchmark isolado do módulo ARC real (`bench/arc/metadata.rs`) reduziu
o tempo de retain/release/consulta em cerca de 13%, confirmado numa segunda
rodada. O ganho dirigido em árvores/lista ficou perto de 2%, com dispersão;
a rodada completa ficou perto da média anterior, sem provar ganho relevante.

Benchmark completo auditável, 32 núcleos e sete execuções alternadas no núcleo
P: A1/A0 0,983, B0/A0 0,959, B1/A0 0,950, ARC/A0 2,107, A0/Dart 1,260.
378 execuções, resultados iguais em todos os modos e repetições, sem falha
nem núcleo ausente. Amostras brutas, protocolo e hashes dos 54 executáveis:
`bench/results/2026-10-09-modos-windows/`. ARC continua distante de A0
(árvores 14,65×, lista ligada 7,63×); o ganho isolado do índice não se
traduz em ganho demonstrado na média completa.

CI [37885116876](https://github.com/insinfo/dartforge/actions/runs/37885116876),
sobre `1f1a394c` (antes do índice sem divisão): Windows ARC com auditoria e
`--gc-stress`, A1/B0/B1 com e sem estresse, todos 238/238. Linux x86-64:
AOT, A1/B0/B1 e ARC 238/238; `dart:io` 130/130; JIT × AOT 238/238 no
placar, sem divergência (sete casos sem IR, iguais por construção). Os
relatórios foram conferidos. Windows com SDK da fonte: AOT 238/238 e
JIT × AOT 238/238 no placar, sem divergência (sete casos sem IR, iguais por
construção); o job também passou os contratos, a recarga e a produção
autocontida. macOS arm64: AOT, A1/B0/B1 e ARC 238/238, `dart:io` 130/130,
JIT × AOT 238/238 no placar, sem divergência (sete sem IR, iguais por
construção). Todos os jobs terminaram com sucesso; relatórios conferidos.

CI [37887482790](https://github.com/insinfo/dartforge/actions/runs/37887482790),
sobre `c3dd048b` (já com o índice ARC sem divisão): distribuição sem MSVC e
Windows SDK, e sem Xcode/Command Line Tools no macOS, passou. Em ambos os
sistemas, os relatórios com e sem a toolchain mostram corpus AOT 238/238 e
`dart:io` 130/130, também 130/130 com `--gc-stress`. Isso valida a
distribuição; a nova etapa dirigida de mapas segue pendente no Pesado.

O Pesado passou a ativar também os quatro testes de `directed_maps` na
célula B0 sem estresse: o corpus sozinho não exercita as sabotagens. O YAML
e a compilação do código de teste foram conferidos; os quatro testes passaram
na rodada `37889904811`, sem ignorados ou filtrados (log conferido, 777 s).
A guarda dos temporários limpa `target/tmp-mapas-*`
inclusive quando uma asserção falha.
Rodada atualizada [37889904811](https://github.com/insinfo/dartforge/actions/runs/37889904811)
disparada sobre `c3dd048b`, incluindo o índice ARC sem divisão e esta etapa,
terminou com sucesso. Os oito relatórios Windows de A1/B0/B1 e ARC, com e sem
`--gc-stress`, já foram conferidos: todos 238/238. As outras sete células
terminaram; B0 também terminou, com corpus 238/238 e quatro testes dirigidos
passando. Todos os jobs terminaram com sucesso. macOS: AOT, A1/B0/B1 e ARC
238/238, `dart:io` 130/130, JIT × AOT 238/238 no placar, sem divergência
(sete sem IR, iguais por construção); relatórios conferidos.
Esta revisão antecede a junção das faixas livres.

O medidor dos modos agora preserva cada execução em `amostras.jsonl`,
incluindo stdout com as rodadas brutas, stderr e código de saída. Confere
o resultado em todas as repetições e a presença dos mesmos núcleos em cada
modo; falha de compilação, execução, timeout ou divergência invalida a
rodada (código 1). Os testes do medidor exercitam inclusive uma falha na
primeira repetição que desaparece na última; entram no CI rápido.

O alocador junta faixas contíguas na ponta da lista
livre da mesma classe e página; não percorre os demais livres nem muda a
quarentena. Os testes dirigidos de reutilização, bloqueio por objeto vivo e
quarentena passaram. Suíte completa do runtime: 115 testes passaram, três
microbenchmarks ignorados; grafos aleatórios: 3.000 sementes em cada um dos
três modos, sem falha. Duas medidas independentes, cinco execuções alternadas
no núcleo P: ARC árvores caiu 4–5%, lista ligada 7,5–8,8%. A0 variou cerca
de ±3% em árvores e −2,3% a 0% em lista ligada. Resultados iguais nas 40
execuções. Dados e limites: `bench/results/2026-10-09-arc-faixas/`.
Isso confirma o ganho dirigido. A rodada completa depois da junção deu
A1/A0 0,991, B0/A0 0,969, B1/A0 0,952, ARC/A0 2,108 e A0/Dart 1,270,
com todas as 378 execuções válidas e resultados iguais. Não houve ganho
demonstrado na média ARC (antes 2,107); dados em
`bench/results/2026-10-09-modos-faixas-windows/`. A validação do corpus
nativo desta mudança passou no Windows na rodada `37896396380`: ARC com
auditoria 238/238, ARC sob `--gc-stress` 238/238 e A0 sob estresse 238/238,
com os três relatórios conferidos. Linux x86-64 também passou AOT, A1/B0/B1
e ARC 238/238, `dart:io` 130/130 e JIT × AOT 238/238 no placar, sem
divergência (sete sem IR, iguais por construção); relatórios conferidos.
macOS segue em validação.

CI rápido [37887482951](https://github.com/insinfo/dartforge/actions/runs/37887482951)
terminou com sucesso sobre `c3dd048b`, antes da junção das faixas.
CI rápido `37891600372` e distribuição sem toolchain `37891600401`
terminaram com sucesso sobre `7006187e`. O Pesado
[37896396380](https://github.com/insinfo/dartforge/actions/runs/37896396380),
suíte `todos`, roda sobre `719e94cc` (mesmo código de produção de `7006187e`).
Ele repõe a rodada diária `37895158314`, cancelada pela concorrência quando
foi feito um disparo nativo redundante; esse disparo também foi cancelado.

Preparação dos donos da HIR: programa e SDK agora compartilham
`optimize::preparar_para_emissao`, com o modo de memória definido antes dos
passes e tabelas de exceção materializadas por último. Isso centraliza o
ponto de integração exigido pela especificação §18.2.
O passe de tabelas separa agora preparação do CFG e materialização:
o inventário de sítios fica privado até a conferência final e as saídas são
recalculadas sobre a HIR final. Ainda faltam cleanups em funções sem tratador,
atualização do inventário pelos futuros passes ARC, inserção e verificação
de ownership (§20).
Verificação local após a separação: suíte unitária completa de
`dartforge-emit-native` em release, 90 testes passaram, sete testes manuais
ignorados. Os três testes novos cobrem o CFG antes da publicação, a pendência
recalculada após editar a continuação e um sítio invalidado entre etapas.
O teste de emissão determinística também passou com
`DARTFORGE_EXCEPTIONS=tables`. O corpus completo remoto ainda usa `719e94cc`,
anterior à separação do passe.
Validação local do passe separado, código de `a80d1b84`: A1, A1 sob
`--gc-stress`, B1 e B1 sob `--gc-stress`, todos 238/238 e código de saída 0.
O harness foi recompilado com `embedded-llvm`; SHA-256
`3F5B4C1286C66C8BFB25880798B3C90F5C5DDCF753FC4C675204A99CE3798738`.
Relatórios em `target/validacao-a80d1b84/`, incluindo a tentativa inicial
inválida (`DARTFORGE_MEMORY=rastreamento`, rejeitada antes de compilar),
seguida das quatro rodadas válidas com `tracing`. São verificações de
correção; os tempos totais do harness não medem desempenho dos modos.

A base `optimize::arc::vivacidade` calcula vivacidade reversa por ponto fixo
em blocos alcançáveis, com usos de `Phi` por predecessor e conjuntos por
instrução, bloco e aresta. Recebe o inventário semântico; não classifica
valores pela largura `i64`. Testes dirigidos cobrem junção, backedge,
inalcançáveis e o resultado de chamada ausente na aresta excepcional.
Suíte unitária do emissor: 94 testes passaram, sete manuais ignorados.
Os dois exemplos da API passaram como doctests; formatação dos dois módulos
novos conferida com rustfmt, edição 2024.
A análise ainda não é chamada pelo pipeline; classificação, dependências
de borrows/keep-alive, tokens, inserção e verificação de ownership faltam.

A variante `vivacidade_com_emprestimos` agora inclui owners e aliases
intermediários nos usos de cada borrowed, inclusive por aresta de `Phi`.
Valida os IDs contra função/inventário e rejeita ciclos com `ARC003`, origem
e caminho determinísticos. Testes cobrem cadeia transitiva até o último uso,
owners distintos nos dois predecessores e dependências inválidas.
Suíte: 97 testes passaram, sete manuais ignorados; quatro doctests passaram.
Formatação dos módulos ARC conferida. Escopo, dominância, escape, keep-alive,
classificação e integração ao pipeline ainda faltam; não é o verificador
completo de ownership da §20.3 nem altera as contagens do runtime.

Representação tipada acrescentada: `Ownership::{Trivial, Owned, Borrowed}`,
com origem de owner local ou do chamador e identidade de escopo.
`vivacidade_classificada` exige cobertura explícita de parâmetros/definições,
rejeita IDs obsoletos e resultados locais que aleguem owner do chamador,
e alimenta a análise de dependências. Um teste preserva alias gerenciado
reempacotado em `I64`, sem deduzir referência pela largura.
Verificação: 100 testes unitários passaram, sete manuais ignorados;
sete doctests passaram e formatação dos módulos ARC conferida. O produtor
automático dos contratos/proveniência e o transporte pela HIR/otimizações
ainda faltam, assim como escopos, tokens e integração ao runtime/emissão.

O inventário tipado agora confere dominância SSA dos owners locais em blocos
alcançáveis: owner de outro ramo, definido depois do alias ou local usado
para sustentar parâmetro é rejeitado com `ARC003`, bloco/origem e caminho.
`Phi` de owner e alias são simultâneos na entrada, sem depender da ordem
textual entre eles. Suíte: 103 testes passaram, sete manuais ignorados;
sete doctests passaram e formatação conferida. Ainda faltam disponibilidade
dos tokens após consumo e existência de resultados apenas no sucesso de
`invoke`; dominância SSA não substitui essas provas.

`vivacidade_classificada_com_excecoes` recebe o inventário e o plano de
tabelas da mesma HIR preparada. Confere a forma do desvio de `invoke` e
exige dominância da aresta de sucesso para sustentar um alias pelo resultado:
retirar essa aresta não pode deixar o bloco do alias alcançável. Isso rejeita
também a junção alcançada pelo pouso, mesmo quando a dominância SSA aceita.
Teste dirigido aceita o sucesso isolado, rejeita a junção com erro e plano
incompatível; teste do grafo cobre a diferença entre dominância de bloco e
aresta. Suíte: 105 testes passaram, sete manuais ignorados; oito doctests
ARC passaram. A análise continua fora da emissão; não prova consumo de
tokens, nem disponibilidade de resultados de chamadas sem `invoke`.
O plano excepcional permite aos passes intermediários consultar os sítios
por índice e símbolo da função, antes de publicar `Module::tabelas`. A
materialização usa essa consulta e confere todos os sítios antes de calcular
as saídas. Os quatro testes do passe passaram, incluindo rejeição de função
trocada; a consulta ainda não insere owners ou cleanups ARC.
Na revisão `0d64636e`, a suíte completa do emissor passou: 106 testes,
zero falhas, sete manuais ignorados. O teste `emitir_ir_e_deterministico`
também passou com `DARTFORGE_EXCEPTIONS=tables`, em 33,39 s, emitindo o
mesmo programa duas vezes sequencialmente e quatro em paralelo. O SDK
`C:/tools/dartsdk-3.6.2/lib/libraries.json` estava presente: não foi o
retorno antecipado por SDK ausente. Esse teste verifica o IR de `print(1)`;
não substitui o corpus excepcional nem a integração ARC.
Os quatro testes de emissão em `lib.rs` deixaram de usar o caminho fixo
Windows e o retorno antecipado por SDK ausente. Agora respeitam
`DARTFORGE_TEST_SDK_LIB`, ou a descoberta usada pela emissão real, e exigem
`libraries.json`. Suíte local: 106 passaram, sete manuais ignorados;
subprocesso com SDK inexistente falhou explicitamente (código 101).
Isso remove uma aprovação sem execução nos runners Unix; a execução destes
quatro testes nas plataformas do CI ainda precisa ser conferida.
O localizador de SDK dos testes também passou a ser compartilhado pelo
inventário de natives e pelos testes/medições de `sdk_modulo`, removendo
seus retornos antecipados por SDK ausente. Os testes manuais de poda e
produção respeitam o caminho explícito dos testes. Suíte local final:
106 passaram, sete manuais ignorados; subprocessos com SDK inexistente
reprovaram inventário e sobreposição (código 101), sem aprovação vazia.

ARC: `registrar_imortal` agora confere o incremento de geração antes de
publicar o registro, como os demais registros. O teste de esgotamento
passou em debug e release: a última geração válida permanece registrada,
o próximo registro falha sem inserir metadados ou voltar a zero.
A hipótese de compactar slots removendo `Option<MetaArc>` foi descartada:
neste compilador Windows x86-64 ambos ocupam 24 bytes. A implementação
experimental passou nos grafos de 3.000 sementes por modo, mas foi removida
por não reduzir memória; nenhum ganho de desempenho é atribuído a ela.
Também foi descartada a inicialização parcial de páginas de metadados:
duas séries dirigidas mostraram ARC/árvores −1,1% e +17,7%, com grande
dispersão na segunda série; lista ligada −2,3% e −4,2%. Não foi comprovado
ganho consistente. As 40 execuções tiveram os mesmos resultados e código
zero; medidas, hashes e alteração de produção preservados em
`bench/results/2026-10-09-arc-paginas`. O runtime voltou à versão
anterior, mantendo a correção do overflow de geração.
O runtime restaurado foi recompilado e conferido em release: 102 testes
passaram, zero falhas, três microbenchmarks ignorados, com
`DF_ARC_SEEDS=3000` nos três modos de grafos. A CLI também foi reconstruída
com o código restaurado; SHA-256:
`33619EB5424E767C463DBBA4AD948F281CC369BA1BCAC31EE46D45CE43ECA3D7`.

O runtime passou a distinguir quadros proprietários e observacionais
(`d8039a51`). Cópias retêm, substituição retém antes de soltar e fechamento
solta cada ocorrência; ativação e promoção preservam multiplicidade. A
auditoria inclui só os owners. `mover_raiz` (`d2497ce7`) transfere entre
slots proprietários sem reter novamente, consumindo o conteúdo substituído.
O código gerado ainda usa quadros observacionais; HIR/ABI permanecem pendentes.
O auxiliar de construção de caixas e o ambiente temporário de closures FFI
foram migrados em `2f3256b3`. Suítes locais debug/release: 108 aprovados,
três microbenchmarks ignorados; teste de closure com coleta forçada nos modos
puro e berçário aprovado. Os demais owners temporários Rust seguem pendentes.
Nova rodada [37915929494](https://github.com/insinfo/dartforge/actions/runs/37915929494),
suíte `nativo`, sobre `2f3256b3`, disparada após confirmar a conclusão da
anterior e a ausência de Pesado ativa. Valida os quadros proprietários,
movimento e primeiros caminhos Rust migrados. Relatórios Windows ARC e ARC
com estresse conferidos: 238/238 cada, em 38,1 s e 53,4 s de harness.
A rodada segue ativa; estes placares não provam conclusão nas três plataformas.
Linux x86-64 também concluiu: relatórios finais conferidos, AOT, A1/B0/B1
e ARC 238/238, `dart:io` 130/130, JIT × AOT 238/238 no placar, zero
divergências (sete sem IR, iguais por construção). macOS, SDK da fonte e
testes dirigidos de mapas B0 ainda em execução na última consulta.
As migrações posteriores das listas (`0fa1d363`), visões e resultados I/O
(`838df90e`), records da ABI e concatenação (`7cc04df8`) ainda não estão
nesta rodada. Validação local debug/release até `7cc04df8`: 113 aprovados,
três microbenchmarks ignorados; os três testes ABI de owners passaram com
`DARTFORGE_ARC_CHECK=1`. HIR, contratos por extern, slots persistentes,
saídas excepcionais e desempenho global continuam pendentes.
Owners persistentes de globais/exceção (`9e7f6451`) e tabelas canônicas
(`2496944a`) foram acrescentados depois dessa rodada. Eventos transferem o
owner da fila para um slot ativo (`5a32f599`); timers periódicos mantêm uma
cópia ativa para cancelamento reentrante. Teste de retorno com exceção
pendente aprovado em debug/release, puro/berçário com auditoria; não exercita
desenrolamento LLVM por tabelas. A fila de mensagens ainda precisa contar
os handles `ValG::Mesmo` compartilhados no mesmo isolado enquanto esperam.
Essa perda de raiz foi reproduzida por um envio real: a string morria na
fila após sair o owner do emissor. A correção retém tokens próprios por
ocorrência nas mensagens normais, liberados após publicação no despacho ou
no descarte. Testes de fila, aliases e callback passaram com auditoria
puro/berçário; suíte release: 122 aprovados, três microbenchmarks ignorados.
Mensagens de controle, término concorrente e ABI completa seguem pendentes.
Na rodada `37915929494`, mapas B0 concluíram 4/4 (754,11 s, sem
ignorados/filtrados), log `113774099093` conferido. SDK da fonte também
concluiu: recarga 2/2 em 56,10 s, Script/Spawn 1/1 e produção autocontida
1/1 em 47,89 s; log `113774098910` conferido. Só macOS permanece ativo.
Na consulta seguinte, macOS concluiu os passos de testes do JIT e recarga;
o corpus AOT/JIT permanece em execução. Os logs finais ainda não foram lidos.
Depois da publicação `d57a9ef6`, a fila rejeita postagem após encerramento
antes de consultar handles do grafo. O teste usa um handle já coletado e
um clone antigo da fila; a rejeição não tenta reter esse handle. A marca e
a admissão usam o mesmo mutex; o fechamento drena owners pendentes.
Suíte release: 126 aprovados, três microbenchmarks ignorados. Os seis testes
de materialização passaram com auditoria ARC e berçário. Essas mudanças
ainda não estão na rodada remota ativa. Os caminhos de controle observados
copiam com `compartilhar=false`; validação de seus owners ativos continua
pendente, assim como HIR/ABI, desenrolamento LLVM e desempenho global.
O owner ativo da mensagem de controle foi migrado para quadro proprietário.
Teste sobre a fila real confirma que a cópia portátil independe do original,
sobrevive à coleta durante o tratamento e é liberada ao retornar; aprovado
com auditoria nos modos puro e berçário. Suíte release: 127 unitários,
13 de integração e 12 doctests aprovados, três microbenchmarks ignorados.
Todos os comandos OOB e seu desenrolamento LLVM ainda precisam de validação.
Relatórios finais do SDK da fonte na rodada `37915929494` conferidos:
AOT 238/238 (57,0 s), JIT × AOT 238/238 no placar, zero divergências,
sete sem IR iguais por construção (122,8 s). Esses relatórios validam
`2f3256b3`, sem as migrações posteriores.
A rodada `37915929494` terminou com sucesso. Relatórios finais macOS
conferidos: AOT e A1/B0/B1/ARC 238/238, `dart:io` 130/130, JIT × AOT
238/238 no placar, zero divergências, sete sem IR iguais por construção.
Log macOS `113771976182`: recarga 7/7 e testes JIT 16/16, 5/5, 6/6.
O relato de erro não tratado agora protege erro e rastro com owners antes
do clear, mantendo-os durante descrição, fallback e envio. A descrição
bem-sucedida ocupa outro slot; retorno com exceção não consulta o resultado.
Teste do protocolo passou com auditoria puro/berçário. Release: 128 unitários,
13 de integração e 12 doctests aprovados, três microbenchmarks ignorados.
O teste não cobre o handler Dart completo nem desenrolamento LLVM por tabelas.
Nova rodada [37922301282](https://github.com/insinfo/dartforge/actions/runs/37922301282),
suíte `nativo`, dois fragmentos, sobre `a49ab81b`, disparada após confirmar
a conclusão da anterior e ausência de Pesado ativa ou na fila. Valida as
migrações posteriores dos owners Rust, persistentes, eventos e mensagens;
ainda não há placares desta rodada.
A entrada do isolado passou a usar owners para entrada, argumento e mensagem
de pronto durante a chamada Dart. Se a produção de pronto lançar, não lê
o resultado nem chama a entrada. Dois testes do protocolo, normal e com
exceção pendente, passaram com auditoria puro/berçário. Release: 130 unitários,
13 de integração e 12 doctests aprovados, três microbenchmarks ignorados.
A primeira execução detectou texto AOT desatualizado por edição de indentação
durante o build; recompilação com fontes estáveis passou a conferência de
fonte única. A rodada `37922301282` continua ativa sobre `a49ab81b` e não
inclui esta mudança de entrada. Spawn nativo e desenrolamento exigem validação.
Callbacks de finalização passaram a manter owner ativo pelo quadro dos
eventos. Teste pelo laço real limpa a fila por encerramento reentrante e
coleta dentro do callback: a closure sobrevive e é liberada ao retornar.
Auditoria puro/berçário aprovada. Release: 131 unitários, 13 de integração
e 12 doctests aprovados, três microbenchmarks ignorados; fonte única conferida.
A fila continua raiz observacional: seus owners e os anexos condicionais,
nos três caminhos de coleta, permanecem pendentes. A rodada `37922301282`
está ativa e não contém esta alteração de callback.
A fila de finalizações passou a contar owners por ocorrência: publicação
retém, consumo e encerramento soltam, ativação/promoção incluem sua
multiplicidade. A publicação nos coletores ARC usa o estado local retirado
do heap; no tracing com berçário, retém ações registradas e a sincronização
reconstrói as promovidas. Teste de anexo real confere RC 2 com owner externo
e fila, RC 1 só na fila e morte após consumo, em puro/berçário e menor/completa.
Aliases e encerramento também testados. Release final: 133 unitários,
13 de integração e 12 doctests aprovados, três microbenchmarks ignorados;
fonte única AOT/JIT conferida. Os owners condicionais de anexos continuam
pendentes; a rodada `37922301282` não contém esta mudança da fila.
Regressão local da §22.4 reproduzida: um anexo Dart ainda mantém dono e
ação como raízes incondicionais. O teste
`anexo_dart_nao_enraiza_ciclo_entre_dono_e_acao`, sem owners externos,
falhou porque o dono sobreviveu à completa no modo berçário. Log local:
`target/runtime-anexo-ciclo-debug.log`. A falha precedeu a integração abaixo. Foi necessário substituir essas
raízes pela aresta lateral dono → ação nos descritores e percursos, com
contagem/remoção por ocorrência e transferência para a fila sem duplo release.
Também falta cancelar ações Dart se dono e alvo morrem juntos e tratar
identidades fracas com geração. Tracing precisa da mesma semântica antes de
servir como oráculo; a falha não será invertida nem ignorada no teste.
A aresta lateral dono → ação foi integrada aos percursos fortes ARC e ao
tracing, incluindo velhos lembrados na menor. Anexar conta e aciona a barreira;
desanexar/encerrar removem a ocorrência. Descarte do dono cancela anexos Dart
e debita a aresta uma vez; morte só do alvo transfere a ação à fila, retendo
o owner de fila antes de soltar a aresta. O ciclo antes vazando passou em
tracing, berçário e puro. Dono velho com ação jovem/captura de alvo passou:
o alvo fica vivo até desanexar, sem forçar finalização antecipada.
Release: 135 unitários, 13 de integração e 12 doctests aprovados, três
microbenchmarks ignorados; fonte única AOT/JIT aprovada. Ainda faltam
identidades fracas com geração, estados explícitos, índices da tabela lateral
e validação nativa completa. O verificador separado da barreira na menor
ainda precisa enumerar as arestas laterais. A rodada `37922301282`, ativa
sobre `a49ab81b`, não contém esta integração.
O verificador separado da menor passou a seguir arestas laterais de
finalizadores. Sabotagem sem barreira foi detectada pelo diagnóstico exigido;
o caso com publicação correta preserva a ação e a libera após retirar o dono.
Release: 137 unitários, 13 de integração e 12 doctests aprovados, três
microbenchmarks ignorados; fonte única AOT/JIT aprovada.
Na rodada `37922301282`, Linux concluiu com sucesso. Relatórios finais
conferidos: AOT e A1/B0/B1/ARC 238/238, `dart:io` 130/130, JIT × AOT
238/238 no placar, zero divergências, sete sem IR iguais por construção.
SDK da fonte, mapas B0 e macOS continuam ativos na última consulta.
A identidade fraca do dono nativo é zerada quando o wrapper morre, antes
de reclamar seu bloco, nos três coletores. A obrigação callback/token fica
registrada até morrer o alvo. Teste exige reutilização física do endereço
e confirma que o wrapper novo não desanexa a obrigação antiga; o callback
roda uma vez ao morrer o alvo, em tracing, berçário e puro.
Release: 138 unitários, 13 de integração e 12 doctests aprovados, três
microbenchmarks ignorados; fonte única AOT/JIT aprovada. A representação
geral com geração, estados explícitos e retenção do módulo nativo continuam
pendentes. Esta correção não conclui a §22.4.
Na rodada `37922301282`, log mapas B0 `113796236829` conferido: 4/4,
sem ignorados/filtrados, 770,09 s. Log SDK `113796236661` conferido:
recarga 2/2 (52,23 s), Script/Spawn 1/1 (1,41 s), produção autocontida
1/1 (47,31 s). Relatório Windows ARC 238/238, 42,6 s de harness.
Só macOS permanece ativo na última consulta; a rodada valida `a49ab81b`.
O filtro de candidatos por corpo BRUTO agora respeita a indicação de
arestas laterais no metadado. A indicação é conservadora por geração,
restaurada na ativação/promoção/drenagem para anexos e chaves de ephemerons.
Teste com filtro que responde sem referências exige coleta do ciclo lateral
e reset no registro seguinte. `MetaArc` continua com 24 bytes.
Release: 139 unitários, 13 de integração e 12 doctests aprovados, três
microbenchmarks ignorados; fonte única AOT/JIT aprovada. Desempenho desta
mudança ainda não foi medido. A rodada remota continua nos testes JIT macOS
e não contém as mudanças posteriores de finalizadores.
As arestas laterais dos anexos Dart passam a usar índice por dono, uma
posição por ocorrência. Marcações, lembrados, percursos fortes ARC e indicações
de candidatos consultam o índice. Remoções o reconstruem antes do próximo
percurso, inclusive entre rodadas internas; encerramento o limpa. A tabela
de registros fica encapsulada no heap. Teste de aliases em dois donos retira
a posição intermediária e confere os percursos e RC 3 → 2 → 1 → morte.
Release: 140 unitários, 13 de integração e 12 doctests aprovados, três
microbenchmarks ignorados; fonte única AOT/JIT aprovada. O índice elimina a
varredura global na visita de arestas, mas remoções ainda percorrem registros;
ganho de desempenho não foi medido. Geração e estados explícitos seguem pendentes.
Na rodada remota `37922301282`, macOS concluiu o passo de testes JIT e
está em recarga na última consulta; ainda não há seus relatórios finais.
O descarte ARC passou a consultar o índice por dono antes de cancelar
anexos Dart. Objetos sem entradas não varrem nem reconstroem a tabela;
os donos com entradas mantêm débito por ocorrência e reconstrução.
Release: 140 unitários, 13 de integração e 12 doctests aprovados, três
microbenchmarks ignorados; fonte única AOT/JIT aprovada. A consulta não
tem ganho de tempo demonstrado no benchmark completo. MacOS concluiu o
passo de recarga e está no corpus AOT/JIT da rodada `37922301282`.

Fechamento da rodada `37922301282` sobre `a49ab81b`: todos os jobs
selecionados concluíram com sucesso. Artefatos finais do macOS conferidos:
AOT, A1, B0, B1 e ARC 238/238, `dart:io` 130/130 e JIT × AOT
238/238 idênticos, zero divergências, sete sem IR iguais por construção.
Log do job `113792887228`: testes JIT 16/16, 5/5 e 6/6; recarga 7/7.
Esta rodada não cobre as mudanças posteriores de finalizadores.

A entrada de finalização passa da fila para um slot proprietário antes de
chamar Dart, por movimento sem retain adicional. O retorno consome esse
slot, em vez de retirar a frente atual da fila. Assim, um laço de eventos
reentrante não reencontra a ação ativa nem consome outra entrada ao retornar.
Teste abre o laço recursivamente, confere execução única de duas ações,
coleta com ambos os callbacks ativos e verifica liberação após retorno.
Release: 141 unitários, 13 de integração e 12 doctests aprovados, três
microbenchmarks ignorados; fonte única AOT/JIT aprovada. Sem nova medição
de desempenho; estados explícitos e identidades com geração seguem pendentes.

Rodada [37928965861](https://github.com/insinfo/dartforge/actions/runs/37928965861),
suíte nativo em dois fragmentos sobre `12409870`, iniciada após confirmar
a anterior terminal e nenhuma Pesado ativa. Na última consulta, compilação
Windows e os jobs Linux/macOS estão em andamento. Cobre os owners de entrada
de isolado e as correções de finalizadores até a transferência antes do callback;
não cobre a propagação posterior de `externalSize`.

`externalSize` deixa de ser descartado pelo patch de `NativeFinalizer`:
attach e `asTypedList(finalizer:)` passam o tamanho à ABI de seis argumentos.
O anexo nativo registra esses bytes sem aresta Dart, soma-os à pressão
de memória e os desconta em detach, execução por coleta ou encerramento.
Teste nos três coletores confirma pressão, persistência enquanto o alvo vive,
ausência de callback em detach, execução única e preservação de outros bytes
externos após repetir cada transição. Release: 142 unitários, 13 de integração
e 12 doctests aprovados, três microbenchmarks ignorados; fonte única AOT/JIT
aprovada. Harness release recompilado com runtime atual; corpus
`14_finalizadores` com coleta sob estresse: tracing 1/1 (14,3 s), ARC
com auditoria e ciclos em toda drenagem 1/1 (22,3 s). São tempos do harness,
não benchmark. Esses testes cobrem attach/detach e `asTypedList(finalizer:)`;
não substituem o corpus completo das três plataformas.
A soma não saturada de `bytes_jovens` foi reproduzida em release:
duas contribuições `isize::MAX` e uma de um byte atingem `usize::MAX`;
a comparação com uma nova alocação de 16 bytes perdia o gatilho por overflow.
Incrementos de alocação, anexos, crescimento e bytes externos, além da
comparação, agora usam soma saturada. Dívida no limite continua pedindo coleta.
O teste também adiciona outro byte e exige que a dívida permaneça no limite.
Release: 143 unitários, 13 de integração e 12 doctests aprovados, três
microbenchmarks ignorados; fonte única AOT/JIT aprovada. O corpus dirigido
do commit anterior não foi reexecutado com esta mudança dos contadores.
O contador de bytes externos vivos e a estimativa interna passaram a `u128`;
somente a projeção para estatísticas/gatilhos fica limitada a `usize`.
Alocação, anexos, crescimento, descarte e reconstruções após coleta atualizam
essa soma interna. Assim, descontar contribuições depois de exceder `usize`
preserva os bytes que a projeção havia ocultado. Teste nos três coletores
soma três contribuições `isize::MAX`, coleta com objeto vivo, retira as três
e exige o objeto mais os 17 bytes externos anteriores; no fim exige zero.
A dívida de alocação continua saturada, pois só precisa pedir atendimento.
Impacto desta soma mais larga no caminho de alocação ainda não foi medido.
Release: 144 unitários, 13 de integração e 12 doctests aprovados, três
microbenchmarks ignorados; fonte única AOT/JIT aprovada. O corpus nativo
completo desta versão ainda aguarda rodada posterior à atualmente ativa.
Harness release recompilado sobre `ef122930`; corpus dirigido
`14_finalizadores` em tracing sob estresse 1/1 (14,8 s de harness).
Rodada local ampliada de `corpus/native` em ARC com estresse, auditoria e
ciclos em toda drenagem terminou em 122/130 (389,4 s de harness), código 1.
As oito falhas foram estouros do limite de execução de 60 s: casos 60, 91,
92, 96, 97, 98, 99 e 136. Esta rodada não está aprovada; não transformar os
timeouts em sucesso. Repetição isolada do caso 60 iniciada com o mesmo modo,
auditoria e limite, para separar concorrência de custo do próprio caso.
Essa repetição isolada também terminou por timeout: 0/1, código 1, 60,7 s
de harness, antes da saída das 26.667 closures. Controle sem auditoria,
mantendo estresse/ciclos e limite de 60 s, iniciado para separar custos;
não substitui o resultado reprovado da configuração original.
O controle sem auditoria também esgotou 60 s: 0/1, código 1, 60,7 s de
harness, no mesmo trecho. Retirando somente o estresse desse controle,
mantendo ARC e ciclos em toda drenagem, o caso passou 1/1 em 0,7 s de
harness. O custo está associado à frequência de coleta desse cenário,
não apenas à concorrência ou à auditoria. Não é benchmark nem prova de
ausência de regressão: faltam comparação anterior e perfil das drenagens.
Perfil diagnóstico do caso 60 reduzido a mil iterações (não substitui o
original), sem auditoria, com estresse/ciclos e `DARTFORGE_GC_TRACE=1`:
4.018 drenagens, 991.674 µs acumulados; trial de ciclos 976.231 µs e
8.670.594 nós examinados ao fim. Cerca de 98% do tempo registrado foi
no trial. Executável anterior preservado, SHA-256
`D83FF48FC984AE65887625B8563C5183E0134211AF80C2A156600FBA80B697F0`.
O trial passa a cortar raízes protegidas da região: as arestas que saem
delas permanecem entradas externas nos RC dos filhos. A raiz continua
candidata para a rodada em que perder proteção, inclusive quando encontrada
como descendente de outra semente. Teste cobre ciclo com outro ciclo anexo,
ausência de nova expansão enquanto protegido e descarte de todos ao retirar
a raiz. Release: 145 unitários, 13 de integração e 12 doctests aprovados,
três microbenchmarks ignorados; fonte única AOT/JIT aprovada. Recompilação
do harness posterior concluída. Perfil isolado de sete pares alternados em
afinidade `0x4`: mediana do processo 1,056755 → 0,348021 s (inclui rastro),
drenagens acumuladas 846.023 → 153.583 µs, trial 836.393 → 144.988 µs,
nós examinados 8.757.350 → 1.351.029. Os 14 processos retornaram zero,
com stdout idêntico; drenagens 4.039 antes e 4.037 depois.
Registro auditável em `bench/results/2026-10-09-arc-corte-raizes`, com
entrada, patch, comparador, hashes e dados brutos comprimidos. A primeira
amostragem de sete pares ocorreu com outro caso local ativo e não é usada
neste registro. O caso original após o corte continuou excedendo 60 s:
com auditoria 0/1 (60,7 s de harness), sem auditoria 0/1 (60,6 s).
Os oito timeouts não estão resolvidos. A exigência de ARC ≥ A0 continua aberta.

Medição completa de `5bbfc80f`, nove programas e 32 kernels, somente A0/ARC,
sete execuções alternadas em afinidade `0x4`: 126 processos válidos, resultados
iguais e 18 hashes de executáveis inalterados. Média geométrica ARC/A0
**1,968695**; árvores 13,25×, construção de textos 8,31× e lista ligada 5,27×.
ARC ≥ A0 permanece não atingido. Resultados também conferidos contra os do
Dart AOT na rodada anterior; entradas do benchmark não mudaram. A rodada
atual não reexecutou Dart nem mediu A1/B0/B1. Registro e dados brutos em
`bench/results/2026-10-09-modos-5bbfc80f-windows`.
CLI recompilada com LLVM embutido, SHA-256
`3F9B8C99A1131044E9E12B569ECB32B058879F0384F5B103F9DFC69ADBA477CD`.
A tentativa inicial definiu `DARTFORGE_GC_STRESS=0`, mas a presença ativa
estresse no runtime. Foi encerrada e preservada como inválida. A medição
válida removeu a variável e reutilizou os 18 executáveis após conferir hashes.
As mudanças acumuladas e a dispersão impedem atribuir o agregado ao corte
de raízes isoladamente. A rodada remota `37936255800` está ativa sobre
`5bbfc80f`; aguardar fechamento antes de outro disparo no main.
A instrumentação de `Heap::drenar_arc` agora só consulta `Instant` e reserva
as nove métricas de etapas quando o rastro está ligado. Antes, esse trabalho
ocorria em toda drenagem, embora a impressão já dependesse do rastro.
A sequência de coleta e o formato das métricas foram preservados. Suíte
release: 145 unitários, três microbenchmarks ignorados, 13 integrações e
12 doctests aprovados (`target/runtime-rastro-condicional-release.log`).
Teste de ciclo com `DARTFORGE_GC_TRACE=1`: 1/1, com métricas emitidas
(`target/runtime-rastro-condicional-ligado.log`). Ainda não há comparação
antes/depois desta mudança; a razão 1,968695 acima pertence a `5bbfc80f`.

Artefatos concluídos da rodada `37936255800` (`5bbfc80f`) conferidos:
Windows ARC 238/238 (52 s de programas, 55 s de harness), ARC sob estresse
238/238 (54 s, 58 s de harness) e `dart:io` 130/130 (89 s, 91 s de harness).
Relatórios em `target/ci-37936255800/{arc,arc-stress,io}`. Não incluem a
instrumentação condicional posterior. A rodada inteira ainda está ativa;
macOS e SDK continuam pendentes na consulta mais recente; B0 concluiu.
Artefato Linux da mesma rodada conferido em `target/ci-37936255800/linux`:
AOT/A1/B0/B1/ARC 238/238, `dart:io` 130/130, JIT 238/238 e JIT × AOT
238/238 idênticos, zero divergências, sete sem IR iguais por construção e
nenhum timeout nos dois executores. Não inferir desses relatórios o
fechamento dos jobs macOS e SDK.

Medição dirigida da instrumentação condicional (`296c001e`), objetos/textos:
28 processos, quatro kernels, sete repetições A0/ARC, resultados iguais,
código zero e stderr vazio. ARC/A0: árvores 12,02×, lista ligada 6,23×,
construção de textos 2,30× e hashes 2,89×. Dados e hashes em
`bench/results/2026-10-09-arc-rastro-condicional`. Afinidade/configuração
iguais à rodada anterior; não comparar os quatro kernels como se fossem
os 32 completos nem atribuir a diferença causalmente só à instrumentação.
B0 remoto `113842810178` conferido: 4/4, zero ignorados/filtros, 769,54 s.

`Heap::arc_contar_iniciais` passou a reter durante a visita imutável ao
corpo/anexos, sem construir `Vec` por objeto. A retenção só altera os
metadados do estado ARC separado: não coleta, chama Dart ou modifica o
corpo visitado. Ocorrências repetidas continuam contadas individualmente;
o modo berçário continua sem contar nesta etapa. Suíte release aprovada:
145 unitários, três microbenchmarks ignorados, 13 integrações e 12 doctests
(`target/runtime-iniciais-diretas-release.log`). Medição de desempenho desta
mudança ainda pendente; não está nos quatro executáveis da rodada anterior.

Job SDK `113842809750` da rodada `37936255800` concluiu com sucesso:
artefato `target/ci-37936255800/sdk-jit` confirma JIT 238/238 e JIT × AOT
238/238 idênticos, zero divergências, sete sem IR iguais por construção.
Log `target/ci-37936255800/sdk-job.log` confirma recarga 2/2 (54,19 s;
cinco filtrados), Script/spawnUri 1/1 (1,45 s; sem filtro), autocontido 1/1
(46,15 s; 112 filtrados) e contratos dirigidos do SDK aprovados. Esses testes
filtrados não representam suítes completas. Só macOS permanece ativo na
consulta atual; não disparar outra rodada pesada antes de seu fechamento.

Medição dirigida de `ae4fd9c5` (contagem inicial sem vetor): 28 execuções
válidas e quatro kernels, sete repetições por A0/ARC, resultados iguais à
rodada anterior. ARC/A0: árvores 12,47×, lista ligada 5,29×, construção de
textos 2,48× e hashes 3,31×. Dados e hashes em
`bench/results/2026-10-09-arc-iniciais-diretas`. Não há ganho amplo
comprovado nem aprovação de ARC ≥ A0; esta rodada não substitui os 32
kernels e não inclui a validação posterior de callbacks NativeFinalizer.

A ABI de anexar NativeFinalizer agora recusa os trampolins síncronos
conhecidos (Pointer.fromFunction e NativeCallable.isolateLocal), com
ArgumentError antes de tocar handles/anexos ou contabilizar externalSize.
O callback pode executar fora da thread do isolado; esses modos exigem a
thread dona. Consulta do modo imutável sob o mutex do registro mantém o
contexto vivo até terminar a leitura. Ouvintes não recebem essa recusa;
ponteiros desconhecidos seguem sujeitos ao contrato nativo do chamador.
A validação fica no attach, sem alterar o sentinela de asTypedList.
Teste cobre os três modos, recusa antes de criar anexo e remoção do registro.
Suíte release: 146 unitários, três microbenchmarks ignorados, 13 integrações
e 12 doctests aprovados (`target/runtime-finalizador-callback-release.log`).
Validação compilada de `0f6629ad` concluída: callbacks inválidos recusados
em AOT tracing, AOT ARC e JIT; caso válido `14_finalizadores.dart`, incluindo
asTypedList(finalizer:), igual ao Dart 3.6.2 nos três caminhos. Onze etapas
terminaram com zero, sob estresse e auditoria ARC quando aplicável. JIT no
modo padrão da CLI; não afirmar ARC no JIT. Dados, hashes, diagnóstico e
roteiro em `bench/results/2026-10-09-finalizador-callback`. Não cobre
listener compilado, corpus completo ou encerramento por grupo.

A rodada pesada `37936255800` sobre `5bbfc80f` fechou com sucesso em todos
os jobs. Artefatos macOS arm64 conferidos em
`target/ci-37936255800/macos`: AOT/A1/B0/B1/ARC 238/238, dart:io 130/130,
JIT 238/238 e JIT × AOT 238/238 idênticos, zero divergências, sete sem IR
iguais por construção e nenhum timeout nos dois. Log macOS
`target/ci-37936255800/macos-job.log` confirma suites JIT 16/16, 5/5 e 6/6
e recarga 7/7; teste isolado de recarga 1/1 em 14,94 s, sete em 1,85 s.
Esta rodada não inclui a instrumentação condicional, contagem inicial direta
nem recusa de callbacks síncronos posteriores. Exige nova validação ampla
após publicar esses commits; não extrapolar o verde para a integração ARC
na HIR ou para desempenho >= A0.

Publicados commits até `0d6f8f2e`. Nova rodada pesada
[37944870979](https://github.com/insinfo/dartforge/actions/runs/37944870979),
suite nativo, dois fragmentos, confirmada em fila sobre
`0d6f8f2ea0cde1153f2398ed25625cfa6c80d594`. Disparo após o fechamento da
anterior; cobre instrumentação condicional, contagem direta e validação
NativeFinalizer. O CI rápido anterior ainda estava ativo e foi substituído
pelo novo push de fonte; não registrar sucesso daquela revisão incompleta.
Aguardar a rodada atual antes de outro disparo pesado no main.

Artefatos já concluídos de `37944870979` conferidos em
`target/ci-37944870979`: Windows ARC 238/238 (54,0 s de programas,
56 s de harness), ARC sob estresse 238/238 (56,5 s, 58 s de harness),
dart:io 130/130 (96,6 s). Linux AOT/A1/B0/B1/ARC 238/238 e dart:io
130/130; JIT 238/238 e JIT × AOT zero divergências, sete sem IR iguais
por construção, nenhum timeout nos dois. JIT com SDK da fonte também
238/238, zero divergências, sete sem IR e nenhum timeout nos dois.
Os relatórios preservam os seis DART! de interop indisponível na VM;
não são seis execuções Dart bem-sucedidas. macOS e job SDK ainda ativos
na consulta; recarga da CLI com SDK já concluiu com sucesso no job,
sem extrapolar para o fechamento da rodada. Fonte desta rodada continua
`0d6f8f2e`, sem os metadados Finalizable publicados depois.

Commits de lowering até `927e75a5` publicados em main. CI rápido
`37949272042` ativo sobre essa revisão; `37944870474` foi cancelado
pelo push de fonte. Nenhum novo disparo pesado enquanto a rodada anterior
continua ativa. Integração ArcKeepAlive exige limites semânticos no CFG,
incluindo saltos que atravessam finally e transferência para estado
suspenso; fechar_escopo apenas retira o mapa de nomes e não representa
essas saídas. A proteção efetiva e o gate de desempenho permanecem pendentes.

Verificador excepcional de ownership agora confere também usos diretos
do resultado de invoke, antes restrito à definição dos aliases borrowed.
Instruções e retornos no caminho sem sucesso são recusados com ARC003;
entradas de Phi usam disponibilidade na aresta do predecessor. Permite
Phi com resultado no sucesso/null no erro e Phi no sucessor imediato da
aresta que publica o resultado. Teste dirigido cobre retorno excepcional,
junção válida/inválida e publicação imediata. ARC 16/16
(`target/arc-resultado-invoke-release.log`); emissor release 109/109, sete
manuais ignorados (`target/emissor-resultado-invoke-release.log`). Esta
API continua fora do pipeline: não insere drops, transfere tokens nem
emite ArcKeepAlive. Não tomar essa validação por ownership integrado.

Novo `arc::verificar_escopos` confere um PlanoEscopos explícito por
instrução, terminador e aresta. Abertura/fechamento usa pilha léxica,
escopo zero reservado à invocação; junções e backedges exigem estados
iguais. Usos/definições borrowed conferem o limite e a cadeia transitiva
de sustentação. Cleanup de aresta precede usos de Phi. Recusa limites
obsoletos e saída com escopos abertos. Quatro testes dirigidos cobrem
fechamento antecipado, aliases transitivos, cleanup/junção/backedge e
Phi/metadata obsoleta. Emissor release 113/113, sete manuais ignorados
(`target/emissor-escopos-release.log`). O lowering ainda não produz o
plano nem o transporta pelas otimizações; API fora do pipeline, sem
inserção de proteção, prova de consumo/escape ou transferência de retorno.
Três exemplos da API passaram em doctests release
(`target/arc-escopos-doc-release.log`). Módulo novo formatado isoladamente.

Diagnósticos de fluxo de verificar_escopos agora incluem caminho de
blocos desde a entrada; junções mostram os dois caminhos incompatíveis.
Árvore de descoberta armazena um predecessor por bloco e reconstrói o
caminho só no erro, sem copiar todos os prefixos durante a análise.
Testes conferem caminhos da junção e erro transitivo. Fixture do retorno
transitivo corrigida para return_ty Ref. Emissor release 113/113, sete
manuais ignorados (`target/emissor-escopos-caminhos-release.log`).
Não altera emissão/runtime; integração do plano e tokens continua pendente.

Job SDK da rodada `37944870979` concluiu com sucesso. Artefato
`target/ci-37944870979/sdk` conferido: AOT com SDK 238/238 (62,5 s),
seis DART! de interop preservados. Log `sdk-job.log` confirma recarga
2/2 (51,03 s, cinco filtrados), spawn 1/1 (1,37 s) e executável
autocontido 1/1 (47,46 s, 112 filtrados); não são suítes completas dos
testes filtrados. Rodada ainda ativa no macOS, no passo de testes do
JIT, na consulta posterior. CI rápido `37949272042`: Linux e mensagens
concluídos com sucesso; Windows e macOS ainda ativos. Nenhum novo
disparo nem reinício. Fonte remota continua distinta dos verificadores
locais `d33a8831`/`88c9bd01`, ainda não publicados.

Rodada pesada `37944870979` sobre `0d6f8f2e` fechou com sucesso.
Artefatos macOS em `target/ci-37944870979/macos` conferidos:
AOT/A1/B0/B1/ARC 238/238, dart:io 130/130, JIT 238/238 e JIT × AOT
zero divergências, sete sem IR iguais por construção, nenhum timeout
nos dois. Log macOS confirma suites JIT 16/16, 5/5, 6/6, recarga
isolada 1/1 (20,24 s) e suíte 7/7 (1,72 s). B0 Windows dirigido
4/4 sem filtrados/ignorados, 743,94 s (`b0-job.log`). Essa revisão
cobre mudanças recentes de runtime anteriores aos metadados de
Finalizable/verificadores locais; não prova ownership integrado ou >= A0.

Base de ABI para slots proprietários: novo fragmento `arc_abi.rs`
exporta abertura, cópia, movimento e fechamento com nomes
`dartforge_arc_frame_{abrir,copiar,mover,fechar}_v1`. Declarada no LLVM
e em efeitos.tsv: nenhum ponto de coleta, exceção Dart ou chamada Dart.
Usa slots contados do Heap, com cópia antes da liberação do antigo,
movimento sem retain físico e fecho LIFO. Cópia/fecho validam quadro
proprietário; não aceitam observacional. Teste ABI acompanha duas cópias,
movimento inclusive para o próprio slot e liberação do último owner em tracing/ARC;
teste do guarda recusa quadro observacional. Runtime release 148 unitários
(três microbenchmarks ignorados), 13 integração e 16 doctests
(`target/runtime-arc-quadros-validado-release.log`). Ainda não há produtor
no lowering, slots protegidos através de await/exceções, ownership.tsv
nem as quatro externs mínimas retain/release/collect/verificar_abi.
Emissor release com o runtime atualizado: 113/113, sete manuais ignorados
(`target/emissor-arc-quadros-release.log`). Não prova inserção de slots
no programa Dart nem proteção efetiva Finalizable.

Publicados commits até `ba5f3b69`. Rodada pesada
[37954216065](https://github.com/insinfo/dartforge/actions/runs/37954216065),
suite nativo, dois fragmentos, confirmada em fila sobre
`ba5f3b69dda293be4160f5fb2f9ae9beb7baef63`, após fechamento bem-sucedido
da anterior. Cobre metadados Finalizable, verificadores de ownership e
nova ABI de quadros. CI rápido `37954199477` pendente na consulta; o
novo push de fonte substitui a rodada anterior ainda ativa. Não declarar
sucesso remoto desta revisão nem disparar outra pesada enquanto estiver ativa.

Preparação excepcional agora conserva saída local de unwind em ARC
mesmo sem catch/finally da fonte: não usa o atalho de saída pura para
chamada direta ou closure que também pode retornar pendente. Tracing
mantém a política anterior. O CFG oferece pouso/saída onde os owners
deverão ser limpos; ainda não fecha quadros nem insere drops. Teste
dirigido compara os dois modos e as duas formas de chamada, confere
SSA/CFG e materialização da saída excepcional. Emissor release 114/114,
sete manuais ignorados (`target/emissor-arc-unwind-local-release.log`).
Validação executada AOT desta revisão ainda pendente; rodada remota
`37954216065` é anterior a esta mudança de CFG.

Validação AOT dirigida de `0c03571c` concluída: CLI release com jit
reconstruída em 5 min 43 s. Comparação com Dart 3.6.2 de exceção profunda
e finally/relançamento/break: tracing/ARC × checagem/tabelas, oito imagens
otimizadas. Os 18 processos monitorados saíram com zero; oito saídas AOT
iguais ao Dart, stderr de execução vazio. Exceção profunda sem variável
GC_STRESS; finally com GC_STRESS=1; heap 256 MB, ARC auditado/ciclos sempre,
berçário desligado, SDK da fonte e sem DLL selecionada no ambiente.
Hashes da CLI, imagens, entradas, resultados e script reproduzível em
`bench/results/2026-10-09-arc-unwind-local`. Não é corpus completo,
prova de cleanup/Finalizable nem benchmark. A rodada remota acima segue
na revisão anterior, sem esta alteração de CFG.

O lowering agora preserva `Local::tipo_estatico`, TypeId Dart original
separado da representação HIR. Declarações usam
`Context::tipo_local_semantico` (sem chamar apagar), closures em ambiente ou
captura direta, funções locais diretas e capturas do stub assíncrono
propagam a identidade original. None representa informação não fornecida;
não pode ser tratado como prova de que Finalizable não exige proteção.
Parâmetros/this e outros caminhos sem TypeId explícito ainda precisam de
cobertura. Não emite ArcKeepAlive nem muda ownership dos locais ainda.
Suíte release do emissor: 106/106, sete testes manuais ignorados;
`target/emissor-tipos-locais-originais-release.log`. A primeira tentativa
usava tipo_local, que já apaga tipos, e foi corrigida antes do commit.

Classificação estática de Finalizable integrada aos metadados de Local:
identidade do marcador de dart:ffi resolvida uma vez por Context, subtipos
pela hierarquia semântica, nullable/FutureOr e limites/promovidos, tipos de
extensão antes de apagar, Never excluído. Resultado Option<bool>: None
para informação insuficiente (hierarquia ausente/limite recursivo), nunca
prova negativa. Metadado propagado nas capturas diretas e em ambiente;
async usa o mesmo local de origem. Ainda não emite proteção de vida.
Testes dirigidos 2/2; suíte release do emissor 108/108, sete manuais
ignorados (`target/emissor-finalizavel-completo-release.log`). Formatação
do módulo novo aplicada isoladamente. Parâmetros/this e produtores sem
tipo semântico, regiões lexicais e operações ArcKeepAlive em saídas,
finally e suspensão/retomada continuam pendentes. route_return pode
entrar em finally: a obrigação deve considerar os escopos atravessados e
a transferência do valor de retorno, não só o fechamento estrutural de bloco.

Parâmetros ligados por declarar_parametros agora recebem explicitamente
p.ty do outline, inclusive os que não têm offset no corpo. Reconstrução de
parâmetros no corpo async/sync* preserva o TypeId da origem, também sem
offset. atribuir_tipo_semantico exige local já ligado e atualiza tipo e
classificação juntos; None não se torna falso. Suíte release do emissor
108/108, sete manuais ignorados (`target/emissor-parametros-semanticos-release.log`).
Ainda faltam this e outras entradas especializadas; não afirmar cobertura
de todos os produtores nem emissão de proteção de vida.

FnBuilder agora carrega this_finalizavel junto ao receptor. Membros
classificam a identidade estática da classe ou o tipo on de extensão;
construtor de tipo de extensão usa a identidade original da declaração.
Capturas efetivas de this em closures, diretas e corpos async propagam a
classificação. Receptores auxiliares de getters late e entradas do SDK
recebem o mesmo dado. Trocas temporárias para RTI e inicialização de mixins
salvam/restauram receptor e obrigação juntos. Classe ausente/hierarquia
insuficiente continuam indeterminadas. Não emite ArcKeepAlive.
Suíte release do emissor: 108/108, sete manuais ignorados;
`target/emissor-this-finalizavel-release.log`. Ainda requer validar os
metadados contra casos semânticos dirigidos e integrar as operações de
proteção em escopos, retornos/finally, exceções e suspensão.

Teste semântico dirigido com o SDK real valida declarar_parametros e this:
Recurso implementando dart:ffi.Finalizable, Recurso? e o tipo de extensão
Envelope recebem obrigação estática; a classe homônima do programa,
dynamic e Never não recebem. Envelope conserva o TypeId original distinto
da representação apagada. O teste carrega, resolve e infere o programa
antes de construir FnBuilder, sem substituir a identidade do marcador.
Suíte release do emissor: 115 testes aprovados, nenhuma falha e sete
manuais ignorados (`target/emissor-finalizavel-semantico-release.log`).
Não valida ainda todos os produtores (capturas, corpos assíncronos,
late e entradas especializadas), nem emite ArcKeepAlive ou cleanup ARC.
Inspeção de simplificar::pura confirma que a ABI dartforge_arc_frame_*
não é classificada como chamada pura: efeitos de GC/exceção nulos não
autorizam sua remoção. Isso é inspeção, não prova de execução integrada.

ABI mínima ARC implementada em arc_abi.rs: dartforge_arc_retain/release,
dartforge_arc_collect e dartforge_arc_verify_abi. Tokens do código têm
inventário por handle/multiplicidade separado de owners de slots e mensagens;
entram no alcance e na auditoria, sem contar raízes observacionais novamente.
Null/Smi são no-op. Release exige token do código; owner de quadro não o
substitui. Retain/release não coletam nem executam Dart; collect é safepoint
explícito, com callbacks nativos possíveis e ações Dart apenas enfileiradas.
verificar_abi retorna 1 só para versão 1 e heap ARC; não certifica contratos
do módulo ou retornos owned. Declarações LLVM nounwind e efeitos registrados.
Teste dirigido cobre tracing, ARC puro e berçário, multiplicidade RC 2 -> 1,
raiz observacional sem dupla contagem, release sem token, ativação com tokens
existentes e ciclo recuperado após o último release. Runtime release final:
152 aprovados, três microbenchmarks ignorados, 13 testes de integração e
20 exemplos de documentação (`target/runtime-arc-tokens-validado-release.log`).
Emissor release com o runtime atualizado: 115 aprovados, sete manuais
ignorados (`target/emissor-arc-tokens-release.log`).
Ainda não inseridos pelo lowering; ownership.tsv, contratos de resultados,
consumo CFG, ArcKeepAlive e gate >= A0 continuam pendentes. O inventário por
handle é uma base auditável, não uma alegação de custo mínimo.

Entrada LLVM ARC passa a conferir dartforge_arc_verify_abi(1) depois de
ativar o modo e antes dos registros/código Dart, tanto autossuficiente quanto
modo SDK/preparar_isolado. Resultado diferente de 1 termina com llvm.trap e
unreachable, sem converter incompatibilidade em exceção Dart ignorável.
Tracing não emite a conferência. Isso só confere versão de tokens e modo do
heap; não certifica o contrato de cada módulo/extern ou retornos owned.
Suíte release do emissor: 116 aprovados, sete manuais ignorados
(`target/emissor-arc-conferencia-abi-release.log`). Validação compilada na
fonte eaa457d8: guarda extraída de df.prepare_isolate no IR real; stubs
retornando 1/0 compilam e respectivamente continuam/terminam por llvm.trap
(0xC000001D). Dois AOT ARC otimizados, checagem/tabelas, gc_d04_finally sob
estresse, auditados/ciclos sempre, produzem stdout igual ao Dart e stderr
vazio. Dez processos raiz: nove zeros e uma falha fatal esperada. CLI
release/JIT compilada com sucesso em 8m59s, hash F63880596B253DB5...2B22FFC0.
Evidência e script exato em bench/results/2026-10-09-arc-abi-entrada-local.
Não certifica DLL antiga completa, ownership SSA/KeepAlive/cleanup integrado,
corpus completo ou desempenho. A conferência só valida versão de tokens e modo.

Rodada Pesado 37954216065 (ba5f3b69), artefatos parciais baixados em
target/ci-37954216065: Windows ARC 238/238 (42 s, harness 45 s), estresse
238/238 (54 s, harness 56 s), io 130/130 (91 s, harness 96 s). Linux
AOT/A1/B0/B1/ARC 238/238, io 130/130; JIT x AOT 238/238 idênticos, zero
divergências, sete sem IR iguais por construção, zero timeouts nos dois.
SDK da fonte AOT 238/238 (76 s). Os seis DART! de interop são falhas da VM
por biblioteca web não suportada; não equivalem a seis oráculos Dart válidos.
Rodada terminou com sucesso, inclusive macOS. Artefatos macOS conferidos:
AOT/A1/B0/B1/ARC 238/238, io 130/130; JIT x AOT 238/238 idênticos, zero
divergências, sete sem IR iguais por construção e zero timeouts nos dois.
Não contém o novo CFG excepcional, teste semântico ou ABI mínima.

Nova rodada Pesado 37961824053, suíte nativo com dois fragmentos, disparada
no main eaa457d8 somente depois do sucesso terminal de 37954216065. Contém
o CFG excepcional ARC, teste semântico com SDK real, ABI mínima de tokens
e conferência da entrada. Terminou com sucesso, conforme consulta ao GitHub
Actions. Artefatos baixados em `target/ci-37961824053` e conferidos: Windows
ARC 238/238 (39,6 s), ARC stress 238/238 (63,1 s), io 130/130 (97,1 s).
Linux/macOS AOT/A1/B0/B1/ARC 238/238 e io 130/130. Os seis DART! de
interop continuam sem oráculo Dart VM válido por bibliotecas web indisponíveis.
Relatórios JIT de Linux, macOS e SDK da fonte também conferidos: 238/238,
zero divergências JIT × AOT, sete sem IR iguais por construção e zero timeouts
nos dois. A rodada não contém as operações ARC ou o verificador de tokens
publicados posteriormente.
CI rápido 37961221420 foi cancelado pela publicação de 26307f47; o novo
37965545755 estava ativo na última consulta. Sem toolchain 37965545388 passou.

HIR passa a representar ArcCopy/ArcDrop/ArcMove. LLVM chama retain/release
para copy/drop; move mantém os bits sem RC físico. Tipos de resultado fixos:
Ref para copy/move, Void para drop. Só recebem SSA Ref já avaliado ou null;
o operador não introduz boxing/alocação, nem classifica I64 por largura.
Verificador de representação rejeita escalares, literais não avaliados e
resultado incompatível. Visitantes comuns de leitura/substituição e os dois
visitantes assíncronos incluem os operandos; operações não são puras para DCE.
Testes dirigidos de IR verificam uma retenção/drop, movimento sem contagem
extra, inlining/remapeamento e rejeições. Suíte final release do emissor:
119 aprovados, sete manuais ignorados
(`target/emissor-arc-instrucoes-validado-release.log`). Três exemplos públicos
passaram, 19 exemplos restantes filtrados (`target/arc-instrucoes-doc-release.log`).
Check do workspace passou (`target/arc-instrucoes-workspace-check.log`), com
avisos existentes. Testes novos formatados isoladamente. Ainda não produzidos
automaticamente pelo lowering. ArcKeepAlive, slots fortes, contratos completos
de externs, saídas excepcionais/suspensão e gate >= A0 continuam pendentes.
A rodada remota 37961824053 testa eaa457d8, sem essas três instruções novas.

Verificador auxiliar `verificar_tokens` confere disponibilidade e consumo
único no CFG preparado. Exige inventário semântico e contrato explícito por
instrução ordinária; ausência não implica empréstimo. Copy cria um token,
move transfere e drop consome. Phi owned transfere simultaneamente na aresta,
inclusive trocas em backedges; junções exigem o mesmo inventário, sem união.
Invoke produz resultado apenas no sucesso e aplica consumos específicos de
sucesso/erro. Retorno owned transfere ao chamador; retorno borrowed exige
owner externo. Saídas rejeitam tokens restantes. Diagnósticos incluem caminho.
Sete testes novos cobrem transferências, consumo duplicado, vazamento, junções,
Phi nullable, loops/trocas, borrows, invokes e contratos ausentes/obsoletos.
Suíte release: 126 aprovados, zero falhas, sete manuais ignorados
(`target/emissor-arc-tokens-cfg-release.log`). Ainda fora do pipeline: não
certifica os contratos fornecidos, nem proveniência/ABI, escopos, invalidação
de campos/borrows, regiões ou estados suspensos. CFG/SSA e representações
válidos são precondições; não substitui os verificadores correspondentes.
Os quatro exemplos públicos passaram, 22 restantes filtrados
(`target/arc-tokens-cfg-doc-release.log`).

Os mapas de escopo do FnBuilder agora carregam identidade léxica monotônica
por função: zero é a invocação e blocos irmãos recebem IDs diferentes,
independentemente da profundidade. Retirar/restaurar um caso de switch
preserva seu ID junto dos locais. O teste semântico com SDK real verifica
sombreamento, restauração e não reutilização, além de Finalizable original.
Suíte release: 126 aprovados, sete manuais ignorados
(`target/emissor-escopos-identidade-release.log`). Isso ainda é metadado do
lowering. Check final do emissor com testes passou, sem aviso novo
(`target/emissor-escopos-identidade-check.log`). O metadado ainda não
exporta PlanoEscopos para a HIR, emite KeepAlive ou cobre
cleanup de return/throw/await. As identidades não autorizam antecipar drops.

`Local::escopo` agora registra a identidade da ligação no mapa léxico. A
inserção central atribui o ID corrente também para capturas de closures,
funções diretas e posições do ambiente; copiar o Local não transporta um
escopo de outra função para a nova ligação. Tipo estático/Finalizable original
continuam preservados. Teste com SDK real ampliado confere parâmetros em zero,
sombreamento, restauração e religação de captura/ambiente com tipo preservado.
Release do emissor: 126 aprovados, zero falhas, sete manuais ignorados
(`target/emissor-locais-escopo-release.log`), apenas aviso preexistente de
atribuir_slots. O transporte desses metadados para HIR/otimizações e a
produção de KeepAlive/cleanup continuam pendentes.

HIR agora representa ArcLoadStrong/ArcStoreStrong com
`SlotForte::Quadro { quadro, indice }` e modos Copy/Move. Quadro é SSA I64
escalar da ABI, não um ponteiro nativo; índice identifica slot Ref proprietário.
LLVM usa as novas externs quadro_carregar_v1/quadro_receber_v1 para carga
owned e transferência SSA→slot; Copy usa quadro_copiar_v1. Visitantes comuns
e assíncronos incluem ID do quadro e valor. Tipos fixos são Ref/Void; o
verificador rejeita Ptr como quadro e valor não avaliado. Tokens confere carga
Owned e consumo no Move, sem consumo no Copy; ID do quadro deve ser Trivial.
Runtime valida quadro/slot/token antes da mutação. Move não retém a origem;
publica o destino antes de soltar o antigo, inclusive quando ambos são aliases.
Load cria token independente que sobrevive ao fecho do quadro. Copy de slot
também libera antigo após publicação. Null/Smi não têm token físico.
Runtime release: 155 unitários aprovados, três microbenchmarks ignorados,
13 integrações e 22 exemplos públicos
(`target/runtime-arc-slots-ssa-release.log`). Testes conferem RC exato em
tracing/ARC puro/ARC jovem, aliases, último owner, destino/token inválidos
sem mutação e chamadas reais da ABI C. Emissor release: 128 aprovados,
sete manuais ignorados (`target/emissor-arc-slots-fortes-release.log`), com
testes de IR/otimização e consumo de tokens. Não prova execução LLVM dessas
instruções, inserção automática, vida/fecho dos quadros, slots globais/heap/
nativos, proveniência geral, ownership.tsv ou gate >= A0.
Check final do workspace passou, com avisos preexistentes
(`target/arc-slots-workspace-check.log`). Todos os 30 exemplos públicos do
emissor passaram (`target/arc-slots-emissor-doc-release.log`). Novos testes,
tipos e módulos formatados isoladamente, sem reformatar arquivos antigos.
Rodada pesada 37970899962 acompanha 92207014: operações ARC iniciais,
verificador de tokens e metadados léxicos, sem esta nova ponte de slots.
Foi disparada após término da rodada anterior; aguardar os jobs pendentes.
Artefatos Windows ARC já conferidos em `target/ci-37970899962`: 238/238
(35,1 s) e ARC sob estresse 238/238 (57,9 s). Os seis DART! de interop
continuam sem oráculo Dart VM válido por bibliotecas web indisponíveis.
macOS, SDK da fonte e B0 ainda ativos na consulta; não inferir conclusão
da rodada desses relatórios parciais nem disparar outra enquanto estiver ativa.
Artefatos Linux da mesma rodada conferidos: AOT/A1/B0/B1/ARC 238/238,
io 130/130; JIT 238/238 e JIT × AOT sem divergências, sete sem IR iguais
por construção, zero timeouts nos dois. Windows io também conferido:
130/130 (93,6 s). Arquivos em `target/ci-37970899962`. Três jobs restantes
continuam ativos na consulta; nenhum resultado desses jobs foi presumido.

Publicação antes da liberação do owner antigo agora vale também para raízes
internas do runtime, cópia global, movimento global→global, quadro→quadro e
global→quadro. Copiar retém antes da publicação; mover transfere a ocorrência
sem retain da origem. As operações continuam sem coleta/Dart entre etapas.
Runtime release final: 155 aprovados, três microbenchmarks ignorados,
13 integrações e 22 exemplos públicos
(`target/runtime-arc-publicacao-validado-release.log`). A primeira edição
incompleta foi detectada por sete testes de owners/auditoria, corrigida antes
de publicar e a suíte completa repetida com sucesso.
O mapa de owners globais não armazena null/Smi: não fornece o valor real de
uma carga global. O descritor forte global ainda precisa associar armazenamento
real, representação e registro de owner; não implementar carga lendo só esse
mapa. ArcLoadStrong/ArcStoreStrong globais e inserção automática seguem pendentes.

Ponte runtime SSA→owner global disponível como
`dartforge_arc_global_receive_v1(id, valor)`: não desreferencia o ID, consome
token do código sem retain da origem, publica o registro e solta owner antigo.
O chamador deverá publicar os bits reais antes, sem safepoint até a chamada;
o token SSA sustenta o valor nesse intervalo. Null/Smi removem owner antigo,
sem transformar o registro no armazenamento do valor. Token ausente é falha
interna antes de mutar o registro. Declaração LLVM nounwind e efeitos sem
alocação gerenciada/exceção/Dart registrados. Teste dirigido cobre RC exato,
alias no destino, rejeição sem mutação, Smi e owner SSA independente, em
tracing/ARC puro/ARC jovem. Runtime release: 156 aprovados, três microbenchmarks
ignorados, 13 integrações e 23 exemplos públicos
(`target/runtime-arc-global-receber-release.log`). Ainda não há descritor HIR
global nem emissão da sequência publicação/transferência; a integração precisa
validar símbolo e representação contra o catálogo do módulo, preservando lazy
initialization e a identidade do armazenamento por isolate/módulo.
Check do emissor passou, com apenas aviso preexistente de atribuir_slots
(`target/arc-global-extern-check.log`).

`SlotForte::Global { simbolo }` conecta as operações
fortes à área real de globais do isolate. Carga lê os bits e retém resultado;
Move publica antes de chamar global_receber_v1; Copy protege temporariamente
o valor, publica e atualiza o owner antes de retirar a proteção. O símbolo
precisa de declaração Ref única no catálogo do módulo; não inferir por I64.
Visitantes e detecção da área global incluem o descritor. Não altera inicialização
lazy nem insere operações no lowering. A primeira rodada passou 128 testes e
falhou no teste novo por selecionar uma gravação do parâmetro no prologue em
vez do slot global; a comparação foi restringida ao endereço global específico.
Validação release repetida terminou com 129 aprovados, zero falhas e sete
manuais ignorados (`target/emissor-arc-global-validado-release.log`). Todos
os 31 exemplos públicos passaram (`target/arc-global-emissor-doc-release.log`)
e check do workspace passou, com avisos preexistentes
(`target/arc-global-workspace-check.log`).
Não comprova execução LLVM das operações novas, transporte da proveniência,
ABI de globais importados, regiões ou inserção automática/KeepAlive.
Da rodada pesada 37970899962, SDK da fonte AOT 238/238 (56,6 s), JIT
238/238, zero divergências, sete sem IR iguais por construção e zero timeouts
nos dois; Windows B0 238/238 (116,3 s). Artefatos conferidos em
`target/ci-37970899962`. Só macOS permanecia ativo na consulta.

Prova AOT dirigida no exemplo `arc_strong_slots`: HIR explícita
usa quadro proprietário, global Ref, carga após fecho do quadro, coleta,
substituição por Smi e null. Emite IR e liga pelo driver real com otimização.
Resultado esperado: `slots ARC`, `42`, `null`, uma linha por valor. O literal
é permanente; essa prova não certifica coleta de objeto mortal, inserção
automática ou escopos/borrows. Duas imagens otimizadas ARC/tracing compiladas
pelo driver real e executadas com código zero e saída idêntica esperada.
ARC executou com ARC_CONFERIR=1, ARC_BERCARIO=0 e GC_STRESS=1; tracing
com GC_STRESS=1. O harness declara explicitamente print_handle, exportado
auxiliar de observação fora do catálogo normal do lowering. A primeira
tentativa sem essa declaração foi recusada pelo Clang, corrigida antes da
prova. Rodar via cargo run, que aplica a configuração do LLVM do repositório;
invocação direta sem esse ambiente não encontrou clang.exe. IR, imagens,
saídas, log de ligação tracing e hashes SHA256 em `target/arc-slots-compilados`;
build ARC final em `target/arc-slots-compilados-build-final.log`.

Evidência dessa prova sobre `7e5be7a8` preservada em
`bench/results/2026-10-09-arc-slots-fortes-aot`: IR, saídas, hashes,
metadados e comandos de reprodução. Os executáveis não entram no histórico.
Hashes das duas saídas são idênticos; `.gitattributes` preserva os bytes do
arquivo de evidências. Mantêm-se as limitações da prova dirigida acima.

Fechamento da rodada pesada 37970899962 sobre `92207014`: todos os jobs
selecionados concluíram com sucesso, incluindo macOS arm64. Artefatos macOS
conferidos em `target/ci-37970899962/macos`: AOT/A1/B0/B1/ARC 238/238,
ARC em 39,5 s, `dart:io` 130/130 em 57,2 s. Essa rodada não contém os
slots fortes de quadro/global nem a prova AOT adicionados depois da fonte
validada; o sucesso não certifica essas mudanças posteriores.

Teste dirigido `abi_global_recebe_objeto_mortal_e_libera_ultimo_owner`
exercita a ABI C real do runtime com objeto mortal e armazenamento global do
chamador, sem quadro observacional: sobrevive ao fecho do quadro proprietário,
reatribuição com alias e troca por Smi enquanto há token SSA; após o último
release e coleta, o objeto está morto. Passou em tracing sob estresse e ARC,
também em processo com ARC_CONFERIR=1 e ARC_BERCARIO=0. Suíte release do
runtime: 157 aprovados, três manuais ignorados, 13 de integração e 23 exemplos
públicos aprovados (`target/runtime-arc-global-mortal-completo.log`;
auditor em `target/runtime-arc-global-mortal-auditor.log`). Isso testa o
runtime, não a execução AOT da HIR nem a inserção automática de ownership.

Prova AOT mortal no exemplo `arc_mortal_slots`: a produtora aloca um alvo
não permanente, transfere seu token SSA ao global forte e retorna antes das
coletas. Um contêiner weak com token próprio observa o alvo sem torná-lo raiz
no chamador; auxiliares LLVM conferem vivo antes e morto depois da remoção do
global. ARC com ARC_CONFERIR=1/ARC_BERCARIO=0 e tracing, ambos com GC_STRESS=1,
passaram com código zero e saída `1`, `null`. Imagens otimizadas reais pelo
driver. Controles negativos ARC: `no-owner` abortou antes de imprimir;
`no-release` imprimiu `1` e abortou na segunda verificação; ambos com
0xC000001D, trap esperado, sem diagnóstico de panic. IR, saídas e logs em
`target/arc-mortal`. A compilação inicial levou 4m28s; compilação das variantes
após mudança no exemplo levou 1m03s, gerações posteriores usaram cache.
HIR explícita e observadores LLVM: não certificam lowering automático,
promoção weak com retenção, escopos Finalizable, ABI de SDK ou desempenho.

Catálogo inicial `crates/runtime/ownership.tsv`, com 13 externs ARC auditadas.
Build gera acesso tipado e coerções contra assinaturas Rust reais; distingue
Ref borrow/consume, escalar/native, resultado owned/escalar/void, retenção
persistente e invalidação. Extern ausente gera erro na consulta pública,
sem default borrow. Todas as externs `dartforge_arc_` exigem contrato; outras
famílias ainda estão sem cobertura. Teste do gerador rejeitou falta, duplicata,
nome desconhecido, tipo/marca inválidos e incompatibilidade excepcional.
Check do runtime e teste de integração passaram; 27 exemplos públicos
passaram (`target/ownership-runtime-check.log`, `target/ownership-runtime-test.log`,
`target/ownership-runtime-doc.log`). O catálogo não é ainda consumido pelo
produtor HIR/LLVM, e cobertura total/contratos excepcionais continuam pendentes.

Ponte HIR `contrato_chamada_runtime`: traduz as externs auditadas para
efeitos de consumo e classe do resultado. Valida aridade/tipos declarados,
exige Ref SSA/null e conserva I8 no resultado de verificar_abi; não aceita
I64 como Ref. Retain direto exige ArcCopy para tornar o token produzido
explícito. O verificador de tokens compara planos de chamadas conhecidas com
ownership.tsv e rejeita omissão de consumo ou classe de resultado incompatível.
Teste de release gerado do catálogo passou; o plano que omitiria consumo foi
recusado pelo catálogo. Suíte unitária release do emissor: 132 aprovados,
sete manuais ignorados (`target/arc-contratos-hir-lib.log`); 33 exemplos
públicos aprovados (`target/arc-contratos-hir-doc.log`). A revisão corrigiu
a tradução inicial I1 para I8 antes da validação final. Flags de retenção/
invalidação são devolvidos ao produtor, mas ainda exigem análise de slots e
borrows; não há geração automática do plano completo no lowering.

Catálogo ampliado para 15 externs: gc_global_root recebe ID nativo e Ref
borrowed, publica owner persistente sem consumir SSA e sinaliza invalidação;
marcar_constante recebe Ref borrowed/getter nativo e mantém a referência
permanente, também sem consumo SSA. Build aceita novas linhas auditadas fora
do prefixo ARC, exige cobertura dessas duas exportações e continua recusando
exceção/execução Dart sem esquema próprio. Dois testes de catálogo/gerador,
27 exemplos públicos do runtime e 32 testes ARC da HIR passaram
(`target/ownership-raizes-test-final.log`, `target/ownership-raizes-doc.log`,
`target/ownership-raizes-hir-test.log`). Na revisão, a remoção de formatação
alheia sofreu erro de codificação e truncou build.rs; restauração do Git e
reaplicação UTF-8 foram seguidas pela repetição aprovada do teste do catálogo.
Nada dessa versão truncada foi publicado. Cobertura completa e inserção
automática de ownership seguem pendentes.

Esquema ownership.tsv com seis colunas obrigatórias, incluindo saída normal
ou pending conferida com efeitos.tsv. Parâmetros Ref podem consumir nas duas
saídas, só no sucesso ou só na pendência; consumo específico exige pending.
Build continua recusando execução Dart sem contrato próprio. Tradução HIR
gera sempre/sucesso/erro e pode_falhar; o verificador exige invoke preparado,
produzindo resultado apenas no sucesso. gc_collect e marcar_permanente foram
auditados como borrowed/void com pending conservador da tabela de efeitos:
catálogo com 17 externs. Dois testes do catálogo/gerador, 33 testes ARC HIR
e 27 exemplos públicos runtime passaram (`target/ownership-pending-test.log`,
`target/ownership-pending-hir.log`, `target/ownership-pending-doc.log`).
Consumo por uma única aresta tem fixtures sintéticas do gerador; nenhuma
extern real do catálogo ainda usa esses dois modos. Não comprova caminhos
excepcionais das demais externs, retorno borrowed, callbacks Dart, inserção
automática ou análise de invalidação dos borrows.

Produtor `produzir_contratos_runtime` preenche classes dos resultados e
efeitos de todas as CallRuntime auditadas de uma função. Faz validação antes
de alterar os dois mapas: símbolo desconhecido, tipo/ID inválido ou conflito
com metadados existentes não deixam alteração parcial. Retorna contratos
com retenção/invalidação para análise posterior. Teste de CFG com plano gerado
para gc_collect passou com invoke preparado e recusou falta do pouso/negação
da pendência. Teste de atomicidade e resultado owned também passou.
35 testes ARC e 34 exemplos públicos do emissor aprovados
(`target/ownership-produtor-runtime-test.log`, `target/ownership-produtor-runtime-doc.log`).
Parâmetros, outras instruções, chamadas Dart e integração no pipeline padrão
ainda exigem seus produtores; não há inserção completa de RC ou cleanup.

Produtor `produzir_contratos_arc` compõe o runtime auditado com as classes
fixas das cinco primitivas: copy/move/load Owned, drop/store Trivial. Rejeita
tipo/classe incompatível ou plano que sobrescreva uma primitiva, sem alterar
parcialmente os mapas. Teste com as cinco operações e fecho de quadro usou
as classes produzidas no verificador de tokens e passou; erro posterior de
tipo e sobrescrita de contrato foram recusados sem mudança parcial.
A primeira compilação encontrou o trecho novo no laço de parâmetros, corrigido
para o laço de instruções antes da execução final. 36 testes ARC e 35 exemplos
públicos aprovados (`target/ownership-produtor-arc-test-final.log`,
`target/ownership-produtor-arc-doc.log`); erro inicial em
`target/ownership-produtor-arc-test.log`. Ainda não classifica Phi/parâmetros
nem referências das demais operações, certifica vida dos slots ou insere
ARC automaticamente no pipeline padrão.

Produtor ARC agora classifica Phi Ref owned a partir de entradas owned/null,
inclusive laços e troca entre Phi. Segue moves para exigir origem fora do
ciclo, sem assumir que um move crie owner. Entrada emprestada sem cópia,
ciclo sem origem e plano que sobrescreva Phi são recusados. A transação inclui
essa etapa: falha depois de classificar ARC/runtime não altera os mapas.
Classes borrowed/trivial explícitas de Phi são preservadas e ainda dependem
de suas provas. Testes nullable, laço com move e troca simultânea usam classes
geradas e passam pelo verificador de tokens, que confere disponibilidade e
consumo por aresta. 37 testes ARC e 35 exemplos públicos aprovados
(`target/ownership-phi-produtor-test-final.log`, `target/ownership-phi-produtor-doc.log`).
Não prova Phi borrowed, provenance/lifetime dos slots, parâmetros semânticos,
geração completa do ownership nem desempenho. Rodada 37978604806 sobre
c3effdd7 ainda ativa: macOS no corpus AOT/JIT na consulta; não contém este
produtor nem o catálogo posteriores.

Entrada `produzir_e_verificar_tokens` produz metadados ARC e confere tokens
e escopos antes de publicar classes/plano. As entradas permanecem intactas
quando a produção passa, mas a verificação encontra vazamento ou empréstimo
em escopo inexistente; testes dos dois casos passaram. 38 testes ARC e
36 exemplos públicos aprovados (`target/ownership-producao-verificada-test.log`,
`target/ownership-producao-verificada-doc.log`). Exige planos da mesma versão
do CFG e contratos semânticos dos parâmetros/demais instruções; ainda não
certifica vida de slots, invalidação, Finalizable nem integra automaticamente
a emissão padrão. Rodada pesada 37978604806 segue ativa na consulta.

Resultado `ref:borrow(N)` no catálogo exige argumento Ref borrowed existente;
índice inválido e owner escalar/consumido são recusados. Record fieldAt foi
auditado como empréstimo do record, sem token independente; catálogo com
18 externs. HIR liga o resultado SSA ao owner do argumento no escopo de
invocação, conservando as restrições dos ancestrais; owner null produz null
trivial. Teste CFG recusou leitura após liberar o record. Teste C ABI real
em tracing/ARC sob estresse confirmou vida do campo até a liberação do record
e morte posterior, também com ARC_CONFERIR=1/ARC_BERCARIO=0. cell_get_ref não
foi classificado borrowed: seu caminho escalar pode criar caixa nova.
Validação no perfil padrão Cargo: runtime 158 aprovados, três ignorados,
15 de integração e 27 exemplos; 39 testes ARC e 36 exemplos do emissor.
Logs `target/ownership-borrow-runtime-completo.log`,
`target/ownership-borrow-record-auditor.log`, `target/ownership-borrow-record-hir.log`
e `target/ownership-borrow-record-doc.log`. Invalidação e proveniência/forma do
objeto ainda precisam de prova; não equivale a cobertura completa da ABI.

Fechamento da rodada pesada 37978604806 sobre c3effdd7: sucesso em todos os
jobs selecionados. Artefatos em `target/ci-37978604806`: Linux/macOS
AOT/A1/B0/B1/ARC 238/238, io 130/130, JIT 238/238, zero divergências,
sete sem IR iguais por construção e zero timeouts nos dois. Windows ARC
238/238 (58,9 s), ARC estresse 238/238 (60,0 s), io 130/130 (93,9 s);
SDK fonte AOT 238/238 (57,7 s) e JIT 238/238 sem divergências (121,5 s).
Seis casos Windows ARC têm oráculo Dart indisponível para interop web;
placar do harness não os transforma em prova diferencial válida. Essa rodada
não contém o catálogo/produtores nem o resultado borrowed posteriores.

O produtor de contratos runtime agora confere a existência e o tipo real de
cada argumento SSA contra a anotação da chamada antes de publicar os mapas.
Isso impede passar um Ref como ID de quadro I64 apenas mudando a anotação.
Teste de regressão rejeita tipo divergente e SSA ausente sem publicação
parcial, e aceita o parâmetro I64 correto. Quarenta testes ARC e 36 exemplos
públicos do emissor aprovados (`target/ownership-ssa-real-test.log` e
`target/ownership-ssa-real-doc.log`). Dominância, tipos de constantes,
proveniência e vida dos quadros continuam exigindo suas verificações próprias.
Rodada pesada 37986946507 sobre 54b5c836 segue em compilação na consulta;
não contém esta conferência adicional de argumentos.

O adaptador runtime também confere constantes dos parâmetros I64: inteiros
são aceitos; endereço de função exige parâmetro native explícito. Bool,
double, null e literais de texto não viram inteiros por anotação. Teste de
regressão cobre todas essas variantes e o getter nativo de marcar_constante.
Quarenta e um testes ARC e 36 exemplos públicos aprovados nos logs
`target/ownership-constantes-test.log` e `target/ownership-constantes-doc.log`.
Isso fecha a validação das constantes no adaptador auditado, sem certificar
proveniência de endereços ou integração completa no lowering.

Publicação por produzir_e_verificar_tokens agora começa por checagem CFG/SSA:
entrada existente, IDs de blocos/valores únicos, destinos existentes, Phi antes
das instruções ordinárias, predecessores sem duplicação e cobertura exata,
existência de operandos e dominância dos usos alcançáveis. Entradas de Phi são
conferidas no fim do predecessor, preservando produção simultânea e backedges.
Blocos mortos exigem existência, sem inventar dominância. Regressões recusam
uso antes da definição, destino/valor ausente e Phi incompleto/duplicado;
aceitam Phi de laço e recusam usar valor do corpo na aresta de entrada.
CFG inválido preserva os mapas de publicação. Quarenta e três testes ARC
aprovados (`target/ownership-ssa-cfg-test.log`), assim como 36 exemplos públicos
(`target/ownership-ssa-cfg-doc.log`); tipagem completa, representação,
invalidação/proveniência e integração no pipeline padrão continuam pendentes.

A publicação dos planos também confere a representação dos operandos das
primitivas ARC: copy/move/drop/store exigem SSA Ref ou null, e o ID do slot
de quadro exige SSA I64. A classe Owned fornecida pelo chamador não transforma
um escalar em referência. Regressão recusa parâmetro I64/constante inteira
como referência e quadro Ref, aceitando null e quadro I64. Quarenta e quatro
testes ARC aprovados (`target/ownership-arc-representacao-test-final.log`),
assim como 36 exemplos públicos (`target/ownership-arc-representacao-doc.log`).
O primeiro log registra falha na prioridade do diagnóstico de SSA ausente,
corrigida antes da publicação. Ainda não certifica abertura/fechamento do quadro,
índices válidos, slots globais ou proveniência de outras operações.

O produtor ARC agora classifica resultados de ICmp/FCmp/LNot como bool Trivial
e gera seus efeitos vazios de consumo/saída excepcional. Recusa resultado que
não seja I1, classe incompatível e contrato que alegue consumir operandos.
Regressão passa comparação de referência emprestada/null, comparação double
e negação lógica por produzir_e_verificar_tokens; conflitos preservam mapas.
Não infere propriedade por I64/Ptr nem classifica parâmetros genericamente.
Quarenta e cinco testes ARC aprovados
(`target/ownership-bool-produtor-test-final.log`), assim como 36 exemplos públicos
(`target/ownership-bool-produtor-doc.log`). Tipagem dos operandos dessas
operações continua uma pré-condição do verificador HIR; não integra ainda o
produtor no lowering/emissão padrão.

Phi I1 agora recebe Trivial do produtor após conferir entradas bool constantes
ou SSA I1 e recusar classes conflitantes. A regra vale também para backedges,
sem gerar token nem entrada de consumo para Phi. Regressão executa o produtor
e verificadores em laço com Phi/LNot sem mapas iniciais; trocar a entrada
booleana por inteiro é recusado sem publicação parcial. Quarenta e seis testes
ARC aprovados (`target/ownership-phi-bool-test-final.log`), assim como 36 exemplos
públicos (`target/ownership-phi-bool-doc.log`). Phis I64/Ptr e
parâmetros continuam exigindo contratos semânticos; essa regra não amplia a
cobertura de referências borrowed nem integra a emissão padrão.

Conferência parcial dos artefatos de 37986946507, revisão 54b5c836, em
`target/ci-37986946507`: Windows ARC 238/238 (35,6 s), ARC estresse 238/238
(56,1 s), io 130/130 (92,5 s); Linux AOT/A1/B0/B1/ARC 238/238, io 130/130
(36,4 s), JIT 238/238 sem divergências, sete sem IR iguais por construção e
zero timeouts nos dois perfis (27,5 s). SDK fonte AOT 238/238 (38,5 s).
Rodada ainda ativa: não equivale ao fechamento de macOS/SDK fonte JIT.
Seis casos de interop têm oráculo Dart indisponível, não prova diferencial.
As alterações posteriores a 54b5c836 não estão nessa rodada.

O relatório Linux expôs falha de preparação do oráculo DDC: generate-dart-sdk.ps1
usava dart.exe em Unix. O script agora seleciona dart em Unix/dart.exe em
Windows e recusa código de saída não zero antes de inspecionar o artefato.
Geração real com SDK 3.6.2 no Windows produziu 7.087.858 bytes e passou node
--check; os dois arquivos temporários foram removidos. Execução Unix da
correção ainda exige a próxima rodada; não altera os placares já publicados.

Auditoria de dartforge_print_handle em saida.rs: recebe handle emprestado,
produz void e imprime descrição temporária sem consumir/guardar o handle.
Adicionado ao ownership.tsv, agora 19 externs, com pending conservador igual
à tabela de efeitos, sem retenção persistente/invalidação. Dois testes do
catálogo passaram (`target/ownership-print-audit.log`), incluindo coerção da
assinatura Rust C e modos de parâmetro/resultado. Quarenta e seis testes ARC
HIR também passaram (`target/ownership-print-hir.log`). Essa extern é necessária
para ligar o produtor à prova AOT arc_strong_slots; antes da publicação dos
planos, a prova ainda precisa representar invoke/cleanup de impressão no CFG.

Revisão da integração da prova AOT após 23fd911a encontrou lacuna no modelo
excepcional do plano ARC. EfeitoTokens.pode_falhar exige atualmente entrada
em TabelasDaFuncao.invocacoes, e a validação exige chamada no fim do bloco,
condição false e pouso. Esse é o formato de desenrolamento Dart; externs
runtime pending retornam normalmente com pendência e exigem leitura/conferência
explícita antes da bifurcação. optimize/tabelas.rs::e_sitio não transforma
CallRuntime em invoke. Os testes com gc_collect/record fieldAt e pousos
sintéticos provam a transferência lógica do token, mas não essa integração
com a emissão real de externs pending. Não publicar esse formato como tabelas
LLVM de impressão. Próxima dependência: representar e verificar saídas de
pendência runtime separadamente de invoke Dart, incluindo disponibilidade do
resultado só no sucesso e cleanup por aresta, então ligar arc_strong_slots.

PlanoTokens.pendencias agora descreve chamada runtime e bloco de erro sem
publicar invoke/pouso LLVM. Exige sufixo CallRuntime, exception_pending I8,
ICmp Ne zero e CondBranch erro/sucesso; saídas duplicadas como invoke são
recusadas. A análise de disponibilidade confere resultados/aliases só após
a aresta de sucesso, e o fluxo de tokens aplica consumos/produção por aresta.
Regressão passa impressão com cleanup nos dois caminhos pelo produtor e
verificadores, mantendo tabelas LLVM vazias; omitir drop no erro ou inverter
arestas é recusado. Catálogo com 20 externs inclui exception_pending normal,
retorno Rust u8 (HIR I8); o gerador aceita u8:scalar com coerção C exata.
Tentativas iniciais i64/i8 foram recusadas pela assinatura antes de publicar.
Quarenta e sete testes ARC, dois testes do catálogo e 36 exemplos aprovados:
`target/ownership-pendencias-test-u8.log`, `target/ownership-pendencias-runtime.log`,
`target/ownership-pendencias-doc.log`. Mapa ainda fornecido pelo chamador;
produção automática dele, cobertura de todas as formas de conferência,
remoção da representação sintética antiga dos testes runtime, integração da
prova AOT e inserção geral de cleanup continuam pendentes.

O produtor de contratos agora reconhece o sufixo explícito auditado de
pendência e gera PlanoTokens.pendencias a partir da aresta then de erro.
Reconhecimento usa a mesma checagem estrutural do verificador; não cria CFG,
invoke nem pouso LLVM. Mapa prévio com erro diferente é recusado antes de
publicar classes/efeitos. A regressão de impressão/cleanup passou sem cadastro
manual do mapa, e o conflito preservou as três entradas. Quarenta e sete
testes ARC aprovados (`target/ownership-pendencias-produtor-test-final.log`),
assim como 36 exemplos públicos (`target/ownership-pendencias-produtor-doc.log`).
Formas não reconhecidas não recebem mapa implícito e continuam recusadas na
verificação completa quando pode_falhar não tem saída. Demais formas de
conferência, testes runtime sintéticos e integração AOT continuam pendentes.

Prova AOT arc_strong_slots agora passa a HIR final por
produzir_e_verificar_tokens antes de emitir LLVM. Impressões têm leitura
exception_pending I8, comparação e bifurcação explícita; no erro, drop do
owner carregado e limpeza do global. Produtor gera classes das primitivas,
contratos auditados e mapa de pendências. Só literal permanente e caixa de
42 (Smi imediato) recebem contratos Trivial específicos fornecidos pelo harness.
ARC com ARC_CONFERIR=1/ARC_BERCARIO=0/GC_STRESS=1 e tracing com GC_STRESS=1
executaram com código zero e saída slots ARC/42/null. Variante no-cleanup
recusada com v8 não consumido no caminho [0, 1], código 1, antes de gravar IR.
Evidência congelada em `bench/results/2026-10-09-arc-slots-verificados-aot`
com IRs, saídas, log negativo, hashes dos executáveis/fonte e reprodução.
Não executa erro real de impressão, não prova coleta mortal/vida de slots,
não insere cleanup automaticamente e não integra a emissão padrão de Dart.

Regressões auditadas de gc_collect e record fieldAt foram convertidas de
invoke/pouso sintético para leitura real exception_pending, comparação e
bifurcação com mapa produzido automaticamente, sem tabelas LLVM preenchidas.
O teste do getter agora recusa também copiar seu resultado borrowed no caminho
de erro, mesmo mantendo vivo o record owner; no sucesso continua recusando uso
após liberar esse owner. gc_collect recusa mapa de erro omitido e contrato
infalível conflitante com ownership.tsv. Quarenta e sete testes ARC aprovados
(`target/ownership-runtime-pending-real-final.log`); essa mudança fortalece os
casos auditados, sem alegar cobertura de todas as externs/invalidações.

A validação de saídas excepcionais agora recusa explicitamente CallRuntime
em TabelasDaFuncao.invocacoes: extern runtime usa conferência de pendência,
não pouso de desenrolamento LLVM. Regressão tenta publicar gc_collect no
formato antigo e recebe diagnóstico específico; os casos válidos de invoke
Dart e pendência runtime continuam passando. Quarenta e oito testes ARC e
36 exemplos públicos aprovados (`target/ownership-runtime-sem-invoke-test.log`,
`target/ownership-runtime-sem-invoke-doc.log`). Prova AOT de slots recompilada
e executada em ARC com auditoria/berçário desligado/estresse, código zero e
saída slots ARC/42/null (`target/ownership-runtime-sem-invoke-aot.log`).
Não autoriza CallRuntime desconhecida nem prova cobertura geral da ABI.

Publicação dos planos agora verifica pilha intraprocedural dos quadros
proprietários locais abertos pela ABI ARC. Uso exige ID SSA aberto, fechar
exige topo LIFO, junções/backedges exigem pilhas idênticas, saídas alcançáveis
exigem pilha vazia. Operações fortes e externs de quadro conferem IDs ativos,
incluindo os dois quadros de mover_v1. Quadros importados/aliases não recebem
presunção de vida. Regressões recusam uso após fechamento, fechamento duplo,
ordem LIFO inválida, quadro aberto na saída e pilhas divergentes em junção;
falha preserva classes/planos. Quinquenta e um testes ARC e 36 exemplos
aprovados (`target/ownership-quadros-vida-test-final.log`,
`target/ownership-quadros-vida-doc.log`). Prova AOT de slots passou novamente
em ARC com auditoria/estresse e saída slots ARC/42/null
(`target/ownership-quadros-vida-aot.log`). Não certifica índices/capacidade,
aliases, efeitos interprocedurais de quadros, suspensão/cancelamento ou ABI
importada, nem insere abertura/cleanup automaticamente no lowering.

Quadros locais com capacidade inteira constante agora conferem índices
constantes dos slots fortes e das externs, inclusive origem/destino de mover_v1.
Capacidade constante negativa e índice negativo são recusados; índice igual
ou maior que capacidade conhecida também. Regressão aceita slot zero de
quadro com capacidade um, recusa slot um, carregar índice -1 e abrir capacidade
-1. Cinquenta e dois testes ARC e 36 exemplos públicos aprovados
(`target/ownership-quadros-limites-test.log`, `target/ownership-quadros-limites-doc.log`).
Prova AOT de slots passou novamente em ARC com auditoria/estresse e saída
slots ARC/42/null (`target/ownership-quadros-limites-aot.log`). Capacidade ou
índice dinâmico ainda não recebe prova de limite; isso não certifica descritores
gerais, ABI importada ou efeitos interprocedurais.

Produtor agora classifica Const de inteiro/double/bool/null e texto permanente
como Trivial, com tipo determinado pela variante, rejeitando inteira anotada
como Ref e conflitos antes de publicar. Literal de texto é estático no AOT
ou canônico/permanente no cache JIT (llvm/textos_ir.rs::emitir_const_string).
Regressão produz seis constantes sem sementes manuais; prova AOT de slots
retirou o contrato manual do literal, conservando só o Smi 42 específico.
Quinquenta e três testes ARC e 36 exemplos públicos aprovados
(`target/ownership-constantes-produtor-test.log`, `target/ownership-constantes-produtor-doc.log`).
AOT com auditoria/estresse passou (`target/ownership-constantes-produtor-aot.log`).
Não classifica caixas gerais, parâmetros ou endereços por largura.

Fechamento da rodada pesada 37986946507 sobre 54b5c836: sucesso. Relatórios
finais em `target/ci-37986946507`: macOS AOT/A1/B0/B1/ARC 238/238, ARC 43,9 s,
io 130/130 (52,9 s), JIT 238/238 sem divergências, sete sem IR iguais por
construção e zero timeouts nos dois (54,2 s). SDK fonte JIT também 238/238,
zero divergências, sete sem IR e zero timeouts nos dois (76,6 s). Completa
os resultados Windows/Linux conferidos acima, mantendo a limitação dos seis
oráculos indisponíveis de interop. Não contém as mudanças posteriores a 54b5c836.

Contrato puro de FCmp agora exige operandos escalares I64/F64 e LNot exige
bool I1/I8 (ou constantes correspondentes), recusando referências e endereços
diretos antes de publicar mapas. Isso evita tomar bits de handle como escalar:
llvm/mod.rs::coagir reinterpreta/trunca bits, sem descaixa nesse caminho.
Regressão recusa FCmp/LNot sobre parâmetro Ref emprestado e preserva entradas;
comparações/negações válidas anteriores permanecem aprovadas. Cinquenta e
quatro testes ARC e 36 exemplos públicos aprovados
(`target/ownership-puras-escalares-test.log`, `target/ownership-puras-escalares-doc.log`).
Não substitui tipagem original completa nem valida todas as operações HIR.

Entradas de Phi Ref agora exigem representação SSA Ref ou null antes de
publicar planos; o produtor de Phi owned também recusa entrada SSA com outro
tipo, mesmo que o chamador a tenha classificado Owned. Isso impede tratar
bits escalares como referência por largura/classe fornecida. Regressão recusa
parâmetro I64 em Phi Ref com classe Owned no produtor e classes Trivial
explícitas na publicação, preservando mapas. Cinquenta e cinco testes ARC e
36 exemplos públicos aprovados (`target/ownership-phi-representacao-test-final.log`,
`target/ownership-phi-representacao-doc.log`). Phis Ref válidos com null,
transferências simultâneas e laços continuam passando; proveniência semântica
geral e Phis managed-I64 ainda exigem produtores próprios.

Produtor ARC agora cobre aritmética inteira/flutuante, operadores de bits,
deslocamentos, negação e conversões IntToDouble/DoubleToInt: resultado Trivial
com tipo da operação e efeitos vazios, exigindo representações escalares
compatíveis nos operandos. Ref/null/endereço direto não vira operando numérico.
Regressão produz/verifica multiplicação, conversão e divisão sem sementes e
recusa multiplicação sobre null sem alterar mapas. Cinquenta e seis testes
ARC aprovados (`target/ownership-aritmetica-produtor-test.log`), assim como
36 exemplos públicos (`target/ownership-aritmetica-produtor-doc.log`). Isso não prova
guardas de domínio/estouro, semântica Dart dos operadores, tipagem original
completa ou inserção ARC no pipeline padrão.

Phi I64/F64 agora recebe Trivial somente com entradas de mesmo tipo e contrato
Trivial conhecido ou constantes numéricas correspondentes. O grafo propaga
origens entre Phis e recusa ciclos sem origem externa, mesmo com classe
previamente fornecida; largura I64 sozinha não classifica um parâmetro.
Regressão verifica laços inteiros e flutuantes, rejeita parâmetro sem contrato
ou Owned e confere publicação atômica. Cinquenta e sete testes ARC aprovados
(`target/ownership-phi-numerico-test.log`) e 36 exemplos públicos
(`target/ownership-phi-numerico-doc.log`). Parâmetros ainda exigem metadados
semânticos explícitos; isso não certifica managed-I64 nem integra inserção
automática de RC/cleanup no pipeline padrão.

A revisão de Phi bool encontrou uma lacuna no produtor: uma entrada SSA I1
sem classe conhecida era aceita. Phi I1 agora compartilha a análise de origem
dos Phis numéricos: exige constante bool, entrada Trivial auditada ou Phi
fundado em origem externa. O teste de origem/contrato cobre também I1,
incluindo parâmetro desconhecido/Owned e ciclo fechado com classe fornecida;
o laço com LNot continua aprovado. Suíte ARC 57/57 e exemplos públicos 36/36
(`target/ownership-phi-bool-origem-test.log`,
`target/ownership-phi-bool-origem-doc.log`). Não se deduz o contrato de
parâmetros pela representação física, inclusive para booleanos.

Catálogo ownership ampliado de 20 para 22 externs: consultas posicionais
`record_numFields` e `record_shape` emprestam o receptor e devolvem i64 escalar,
sem reter nem invalidar empréstimos. O corpo somente lê `Heap::record`;
`shape` delega a `numFields`. Mantida saída pending conforme efeitos.tsv.
Gerador exige as duas entradas e o teste confere modos e marcas (2/2 em
`target/ownership-record-consultas-catalogo.log`). Teste runtime verifica
resultados para record/null e liberação do record/campo após as consultas,
em tracing e ARC, sob GC_STRESS=1, ARC_CONFERIR=1 e BERCARIO=0 (1/1 em
`target/ownership-record-consultas-runtime.log`). Não cobre fieldNames,
alocadores ou demais consultas do SDK; ausência continua sem contrato.
Suíte ARC do emissor após regenerar o runtime embutido: 57/57
(`target/ownership-record-consultas-arc.log`).
Regressão no produtor/verificador exercita ambas as consultas com receptor
Borrowed do chamador e conferência real de exception_pending/ICmp/CondBranch:
resultado Trivial e mapa de pendência produzidos automaticamente. Retorno
do resultado no ramo excepcional é recusado sem alterar classes, efeitos
ou pendências. Suíte ARC 58/58
(`target/ownership-record-consultas-produtor.log`).

Retenção de módulo/biblioteca nativa e encerramento por grupo continuam pendentes.
Inserção inicial de cleanup em `inserir_arc_saidas_dart`: prepara retenção
do retorno e usa o mesmo fluxo do verificador para obter tokens restantes
por Return, excluindo o token transferido. Insere ArcDrop em ordem de IDs e
publica HIR/classes/plano somente após wrapper Dart completo. Análise auxiliar
admite apenas tokens restantes nos retornos; mantém disponibilidade, consumo
único, convenção e igualdade dos inventários em junções. Regressão insere
cleanup nas saídas normal/erro de chamada runtime com pendência real, verifica
idempotência e rejeita consumo duplicado sem publicar nem mapa de pendências.
O diagnóstico esperado no teste foi ajustado para a indisponibilidade detectada
antes do segundo consumo. Suíte ARC 71/71 e exemplos públicos 40/40
(`target/ownership-saidas-cleanup-test-final.log`,
`target/ownership-saidas-cleanup-doc.log`). Não fecha quadros, divide arestas
críticas nem prepara finally/cancelamento/suspensão; prova AOT desse cleanup
e integração no lowering padrão continuam pendentes.
Variante AOT `retorno-mortal` usa a fábrica Owned com i64::MAX, retém pela
função identidade preparada pelo passe, transfere o token devolvido ao slot
e libera o token original antes de fechar o quadro/coletar. ARC auditado sob
estresse e tracing sob estresse saíram zero com
`9223372036854775807`, `42`, `null`; saídas SHA256 idênticas
`1c4a8377d250d969e710606d42c4123b710b6cba2b3abf2de17bfbb62b3825c9`.
Variante sem cleanup saiu 1 antes de IR/executável negativo. A primeira
compilação revelou declaração LLVM ausente para a fábrica; incluída com
aloca=true, lanca=false e chama_dart=false, conforme runtime. Testes LLVM
39/39 (`target/ownership-retornos-mortal-llvm-test.log`). Evidência congelada
em `bench/results/2026-10-09-arc-retorno-mortal-aot`. Prova sobrevivência
durante uso, não a morte após último owner no AOT; a morte tem prova runtime
separada. Não certifica ausência de vazamentos nem cleanup automático.
Nova ABI `dartforge_arc_box_int_owned_v1` entrega int em representação Ref
com token Owned já registrado, para preparar a prova de retorno com objeto
mortal. Smi dispensa contador; i64::MAX cria Mint. Catálogo 23 externs:
argumento i64 escalar, resultado Ref Owned, saída normal, pode coletar,
invalidação conservadora de borrows, sem retenção oculta de argumento.
Teste em tracing/ARC com GC_STRESS=1, ARC_CONFERIR=1 e BERCARIO=0 cria
segunda retenção, libera token inicial, confere sobrevivência, libera o
último e confere morte após coleta; também cobre Smi. Runtime dirigido
1/1, catálogo 2/2 e exemplos públicos 28/28
(`target/ownership-fabrica-owned-runtime.log`,
`target/ownership-fabrica-owned-catalogo.log`,
`target/ownership-fabrica-owned-doc.log`). Ainda falta usar essa fábrica
na prova AOT de retenção inserida; a rodada pesada ativa sobre 116f6bed
não contém esta ABI.
Após reconstrução do runtime embutido, suíte ARC do emissor 70/70
(`target/ownership-fabrica-owned-arc.log`).
Fechamento de `37992860235` sobre `4530b91f`: todos os jobs selecionados
completed/success. macOS AOT/A1/B0/B1/ARC 238/238 em
58,5/49,3/96,7/96,5/46,4 s; io 130/130 em 70,3 s; JIT 238/238 em
65,5 s, zero divergências, sete sem IR iguais por construção, zero timeouts
nos dois. Relatórios em `target/ci-37992860235/macos`; Windows completos
em `windows`, Linux e SDK nos diretórios já registrados. Windows A1 e
estresse 238/238 em 54,8/53,7 s; B0 e estresse 238/238 em 114,8/65,3 s;
B1 sob estresse 238/238 em 102,9 s; estresse básico 238/238 em 57,6 s;
fragmentos básicos 119/119 em 39,2/24,5 s. Log B0 confirma mapas 4/4,
749,20 s. Log SDK confirma recarga 2/2 (49,68 s), ScriptSpawn 1/1
(1,39 s) e produção autocontida 1/1 (49,93 s). Etapa DDC do action Linux
foi skipped com gerar-dart-sdk-js=false: não prova geração portátil em Unix.
Esta rodada não inclui os produtores de Phis posteriores, catálogo 22,
wrapper Dart nem inserção de retenção dos retornos.
Windows B1 normal também conferido: 238/238 em 102,7 s.
Após confirmar Pesado sem runs in_progress/queued/waiting, iniciada rodada
`37998421467`, suite=nativo, fragmentos=2, sobre
`116f6bed11bcc79d5d25f3e560db5bf0c07bd8a6`. Estado inicial queued;
cobre as mudanças posteriores da produção ARC e exemplo de retorno.
Prova AOT da retenção inserida disponível na variante `retorno` do exemplo
de slots fortes: função identidade Ref começa retornando o parâmetro, recebe
uma ArcCopy pelo passe e é verificada antes de emitir. Acrescentada após a
otimização geral para conservar a fronteira de chamada real; LLVM mostra
chamada ao callee e um retain antes do retorno Owned. Contrato dessa chamada
é explícito no fixture. ARC com auditoria/berçário desligado/estresse e tracing
com estresse saíram zero e mesma saída; variante retorno-sem-cleanup saiu 1
por token v8 restante, sem IR/executável negativo. Evidência congelada em
`bench/results/2026-10-09-arc-retorno-inserido-aot`, incluindo LLVM,
saídas, diagnóstico, hashes e reprodução. Variante normal do exemplo também
compilou/executou sob auditoria/estresse. Continua sendo literal permanente:
não prova contagem física/morte de objeto mortal nem inserção automática de
drops/cleanup ou integração no lowering padrão.
Primeira inserção automática de retenção disponível em
`inserir_retencao_retornos_dart`: retorno SSA Ref Borrowed/Trivial recebe
ArcCopy e transfere token Owned; Owned existente transfere diretamente;
null direto não precisa cópia; literal permanente direto ganha Const antes
da cópia. Publica HIR/classes/plano somente após wrapper Dart completo.
IDs novos não coincidem com IDs existentes dos planos/tabelas. Regressão
confere idempotência, retorno de parâmetro sem sementes, null/literal e
rejeição atômica quando outro token permanece sem cleanup. Suíte ARC 70/70
e exemplos públicos 39/39 (`target/ownership-retornos-insercao-test.log`,
`target/ownership-retornos-insercao-doc.log`). ArcCopy já emite retain LLVM,
mas esta alteração ainda não tem prova AOT própria da transformação nem
está no lowering padrão; drops, cleanup excepcional e suspensão continuam
pendentes. Não confundir inserção dos retornos com ARC automático completo.
Wrapper `produzir_e_verificar_tokens_dart` integra o produtor dos parâmetros
Ref à transação de CFG/SSA, instruções, tokens, escopos e quadros. Exige plano
de retorno Owned para Ref e Trivial para os demais tipos, conforme §20.1.
Regressão mostra tentativa de mover parâmetro Borrowed falhando sem publicar
nem os parâmetros; copy seguido de retorno Owned passa sem sementes; retorno
Borrowed incompatível é recusado preservando mapas. Suíte ARC 69/69 e
exemplos públicos 38/38 (`target/ownership-wrapper-dart-test.log`,
`target/ownership-wrapper-dart-doc.log`). O lowering ainda precisa preparar
RC/transferências/cleanup e fornecer contratos dos tipos/operações não cobertos;
essa integração da API não certifica o pipeline padrão inteiro.
Disponível produtor `produzir_parametros_ref_dart` para a convenção §20.1:
parâmetros Type::Ref recebem Borrowed(Chamador,0); outros tipos permanecem
para seus produtores semânticos, sem deduzir I64 pela largura. Conflito de
classe ou parâmetro duplicado rejeita sem publicação parcial. Exemplo público
exercita produção do parâmetro e wrapper completo com copy/drop, sem sementes
manuais. Suíte ARC 68/68 e exemplos públicos 37/37
(`target/ownership-parametros-dart-test.log`,
`target/ownership-parametros-dart-doc.log`). A API deve ser usada somente
para funções dessa convenção; não infere contratos de callees, resultados
nem tipos originais perdidos, e ainda não está no lowering padrão.
Prova AOT de slots fortes repetida no HEAD `d22f564d`, após as mudanças de
Phis e catálogo 22: ARC com ARC_CONFERIR=1, BERCARIO=0 e GC_STRESS=1,
e tracing com GC_STRESS=1, saíram zero com `slots ARC`, `42`, `null`.
Saídas idênticas por SHA256
`ae77d0d327c27325eb89d55726350cdbd595b6c54ae17f696c287b04a2a90246`.
Variante no-cleanup saiu 1 por token v8 não consumido no caminho [0,1];
ausência de IR/executável negativo conferida. Evidências locais em
`target/ownership-slots-d22f564d-*`; arquivo congelado anterior permanece
sem alteração. Essa prova continua limitada ao exemplo HIR, sem demonstrar
inserção automática no lowering de programas Dart ou erro real de impressão.
Na rodada `37992860235` sobre `4530b91f`, SDK da fonte completed/success:
artefatos AOT 238/238 em 59,7 s e JIT 238/238 em 122,4 s, zero divergências,
sete sem IR iguais por construção e zero timeouts nos dois. Relatórios
em `target/ci-37992860235/sdk-aot` e `sdk-jit`. Só macOS segue ativo.
Produtor agora infere Trivial para Phis Ref com origem null/Trivial conhecida,
inclusive ciclos ancorados, sem classe manual do resultado. Demandas explícitas
de consumo ARC, efeitos de chamadas e retorno Owned são propagadas para trás
pelas entradas de Phis e preservam o token lógico, inclusive para null.
Regressão verifica o mesmo laço de null com retorno Trivial e Owned, além
de laço sobre literal permanente. Suíte ARC 67/67 e exemplos públicos 36/36
(`target/ownership-phi-ref-trivial-auto-test-final.log`,
`target/ownership-phi-ref-trivial-auto-doc.log`). Não mede ganho de desempenho
nem infere ownership de parâmetros/representações managed-I64.
Regressão de ciclo emprestado ampliada para dois Phis mutuamente dependentes,
com blocos em ordem inversa: âncora externa produz ambos e o wrapper completo
aceita o retorno Borrowed. Trocar a fonte do segundo Phi por cópia Owned
deixa a dependência incompatível após propagação e rejeita todos os mapas,
inclusive a classificação da cópia. Suíte ARC 66/66
(`target/ownership-phi-borrowed-ciclo-test.log`). Na rodada pesada
`37992860235`, job B0 agora completed/success; macOS e SDK da fonte ainda
ativos. Logs detalhados de B0 aguardam o fechamento da rodada.
Produtor de Phi Borrowed agora atravessa dependências de Phis Ref ainda sem
classe quando há entrada emprestada externa conhecida. Após o ponto fixo,
confere todas as entradas antes de publicar: dependência não resolvida ou
contrato divergente rejeita a produção inteira. Regressão produz/verifica
Phi de laço com parâmetro emprestado e backedge próprio; remover a origem
externa é recusado sem alterar mapas. Suíte ARC 65/65 e exemplos públicos
36/36 (`target/ownership-phi-borrowed-laco-test.log`,
`target/ownership-phi-borrowed-laco-doc.log`). Não resolve todos os contratos
equivalentes de owners/escopos nem certifica invalidação de slots.
Produtor agora propaga contratos Borrowed idênticos por Phis Ref ainda sem
classe, quando todas as entradas estão conhecidas (null/Trivial também são
compatíveis). Não converte entrada Owned em empréstimo implícito. Dependência
direta no Chamador é representada pelo parâmetro de origem, como exige o
inventário para valores locais. Ponto fixo resolve cadeias acíclicas mesmo
com ordem física inversa dos blocos. Regressão usa apenas contrato do
parâmetro e retorno Borrowed, sem classes manuais para os dois Phis, e
passa pelo wrapper completo. Suíte ARC 64/64 e exemplos públicos 36/36
(`target/ownership-phi-borrowed-produtor-test.log`,
`target/ownership-phi-borrowed-produtor-doc.log`). Ciclos emprestados e
contratos diferentes porém equivalentes ainda exigem produção mais geral;
invalidação de slots e inserção RC/cleanup permanecem pendentes.
Prova de escopo herdado ampliada ao wrapper completo: aliases locais de
chamadas Dart com contratos explícitos alimentam Phi Borrowed; PlanoEscopos
abre 7 antes da primeira chamada e fecha na saída. Uso do Phi enquanto
aberto passa; fechar antes do ArcCopy rejeita uso fora do escopo 7 e preserva
classes/efeitos. Suíte ARC 63/63
(`target/ownership-phi-borrowed-wrapper-test.log`). Não prova o corpo dos
callees Dart nem infere seus contratos: eles são premissas do fixture.
Conferência de Phi Borrowed ampliada para os escopos de toda a cadeia de
owners: o resultado deve conservar todos os limites não zero herdados da
entrada, além da mesma raiz. Substitui a igualdade somente do escopo direto,
que deixava escapar limite de um alias anterior e recusava representação
equivalente do limite na cadeia do resultado. Regressão usa escopo 7 herdado
por alias de escopo zero: perde-lo é recusado; conserva-lo na cadeia ou
diretamente é aprovado pelo verificador de tokens. Suíte ARC 62/62
(`target/ownership-phi-borrowed-escopos-test.log`). Ativação/fechamento dos
escopos continuam responsabilidade do PlanoEscopos; epochs/invalidação
de slots ainda exigem metadados próprios.
Verificador de tokens agora confere cada entrada de Phi Borrowed: owner raiz
deve coincidir com o resultado, e entradas Borrowed devem conservar o escopo
declarado diretamente. Owned pode emprestar ao Phi sem transferir o token,
desde que seu próprio ID seja a raiz; null/Trivial não cria obrigação.
Regressão aceita empréstimo de uma cópia Owned mantida viva e rejeita troca
para outro owner válido no inventário. O primeiro caso negativo escolhido
era barrado antes pela regra de owner Chamador apenas em parâmetros; foi
substituído por owner Valor(param) para exercitar a conferência da aresta.
Suíte ARC 61/61 (`target/ownership-phi-borrowed-owner-test-final.log`) e
exemplos públicos 36/36 (`target/ownership-phi-borrowed-owner-doc.log`).
Não certifica epochs de slots, invalidação geral nem transformação automática
de escopos de empréstimos nas cadeias de aliases.
Phi Ref com classe Trivial explícita agora exige entradas Ref Trivial/null
e origem conhecida fora do ciclo. Antes, a classe fornecida escapava da
análise do produtor para Phis owned. Regressão rejeita entradas Owned e
Borrowed sem alterar mapas e verifica entradas Trivial/null pelo wrapper
completo. Suíte ARC 60/60 e exemplos públicos 36/36
(`target/ownership-phi-ref-trivial-test.log`,
`target/ownership-phi-ref-trivial-doc.log`). Phis Borrowed ainda exigem
proveniência e validação próprias; isso não certifica invalidação geral.
Pré-condição SSA ARC agora exige representação Ref no operando SSA de um
retorno Ref; null e literais permanentes diretos permanecem permitidos.
Contrato Owned sobre parâmetro I64 não autoriza reinterpretar seus bits como
referência no retorno. Regressão usa o wrapper completo, confere rejeição
atômica e retorno Owned válido quando o parâmetro realmente é Ref.
Suíte ARC 59/59 (`target/ownership-retorno-ref-test.log`). Tipagem de retornos
escalares e proveniência geral continuam fora desta validação parcial.

Rodada `37992860235` sobre `4530b91f`: artefatos Linux conferidos em
`target/ci-37992860235/linux`. AOT/A1/B0/B1/ARC 238/238 em
68,7/39,0/82,4/86,2/37,9 s; io 130/130 em 51,9 s; JIT 238/238,
zero divergências JIT × AOT, sete sem IR iguais por construção e zero
timeouts nos dois, 41,2 s. Os seis DART! de interop continuam sendo
oráculos indisponíveis na plataforma, não provas diferenciais válidas.
Windows ARC 238/238 em 55,8 s, ARC sob estresse 238/238 em 37,3 s e io
130/130 em 90,5 s; relatórios baixados em `target/ci-37992860235/windows-*`.
São tempos de harness desta rodada, não benchmark isolado ARC/A0.
macOS, SDK da fonte e job B0 ainda ativos nesta consulta; não fechar a
rodada nem iniciar outra Pesado. GitHub recusou disponibilizar o log do
job Linux enquanto a rodada está ativa: a geração DDC portátil ainda
exige conferir esse log ao fechamento. Esta rodada antecede as mudanças
de FCmp/LNot, Phis escalares e catálogo de 22 externs.

Na rodada `37928965861`, artefatos Windows já conferidos: ARC 238/238
(33,0 s de harness), ARC sob estresse 238/238 (62,5 s), `dart:io` 130/130
(71,3 s). A rodada segue ativa e não contém `externalSize` nem a soma saturada.
Linux concluiu com sucesso: relatórios finais AOT/A1/B0/B1/ARC 238/238,
`dart:io` 130/130, JIT 238/238 e JIT × AOT sem divergências (sete sem IR
iguais por construção). Artefato JIT com SDK da fonte também conferido:
238/238, zero divergências e sete sem IR. Jobs macOS, SDK e B0 ainda ativos
na última consulta; não inferir o fechamento deles desses relatórios parciais.
Depois disso, B0 concluiu: log `113817498842` confirma 4/4 testes de mapas,
sem ignorados/filtros, 745,25 s. SDK da fonte concluiu: log `113817498673`
confirma recarga 2/2 (53,59 s), ScriptSpawn 1/1 (1,41 s) e produção
autocontida 1/1 (46,35 s), além dos contratos dirigidos. Só macOS segue ativo,
no passo dos testes JIT; aguardá-lo antes de iniciar outra Pesado no main.
Fechamento da rodada `37928965861` sobre `12409870`: todos os jobs
selecionados concluíram com sucesso. Relatórios macOS conferidos:
AOT/A1/B0/B1/ARC 238/238, `dart:io` 130/130, JIT 238/238 e JIT × AOT
zero divergências, sete sem IR iguais por construção. Log `113814679929`
confirma testes JIT 16/16, 5/5, 6/6 e recarga 7/7. As mudanças posteriores
de `externalSize`, contadores amplos e corte do trial aguardam nova rodada.

Rodada [37908897460](https://github.com/insinfo/dartforge/actions/runs/37908897460),
suíte `nativo`, sobre `2cf783e0`, iniciada depois de confirmar que a anterior
estava concluída e nenhuma Pesado ativa. Valida as análises auxiliares ARC,
a consulta do plano excepcional, recarga e descoberta de SDK corrigidas;
não contém a correção posterior da geração imortal nem as experiências locais.
Relatórios Windows já conferidos: ARC 238/238, ARC com estresse 238/238,
`dart:io` 130/130. A rodada terminou com sucesso em todos os jobs.
O relatório AOT com SDK da fonte desta rodada também foi conferido:
238/238, em 56,9 s de harness. O artefato AOT é publicado antes dos testes
de recarga e produção; seus resultados próprios foram conferidos abaixo.
Relatórios adicionais conferidos: Windows A1/B0/B1 com e sem estresse,
todos 238/238; Linux AOT, A1/B0/B1 e ARC 238/238, `dart:io` 130/130;
JIT × AOT Linux e SDK da fonte 238/238 no placar, zero divergências
(sete sem IR, iguais por construção). O log Linux confirma execução dos
quatro testes de emissão agora sem retorno antecipado, do inventário de
natives e da sobreposição, todos aprovados com o SDK real. macOS também
passou AOT, A1/B0/B1 e ARC 238/238, `dart:io` 130/130 e JIT × AOT
238/238 no placar, zero divergências (sete sem IR, iguais por construção).
Relatórios finais e log `113748988829` conferidos; recarga macOS executou
sete testes, todos aprovados. B0 concluiu os quatro
testes dirigidos de mapas: 4/4, nenhum ignorado ou filtrado, 470,75 s;
incluindo sabotagem, coleta agendada, folha que coleta e comparação com
pilha-sombra. Log do job `113752465885` conferido.
O job SDK da fonte concluiu com sucesso; log `113752465447` conferido:
recarga corrigida executou os dois testes existentes (2/2, zero falhas,
nenhum ignorado, cinco não ignorados filtrados), em 58,36 s;
`script_e_spawn_uri_no_jit` passou (1/1), e produção autocontida passou
(1/1, 47,01 s). A proteção de seleção vazia e o comando corrigido foram
executados no runner. Isso fecha a lacuna de recarga do job anterior,
que executava zero testes por um filtro obsoleto.

Na rodada remota `37896396380` sobre `719e94cc`, os quatro testes dirigidos
de mapas também passaram, sem ignorados/filtrados, em 752 s; log do job B0
conferido. Esta rodada ainda antecede a preparação excepcional separada.
Relatórios adicionais conferidos nesta rodada: Windows A1/B0/B1 com e sem
estresse, todos 238/238; `dart:io` Windows 130/130; JIT × AOT com SDK da
fonte 238/238 no placar, zero divergências (sete sem IR, iguais por
construção). A rodada terminou com sucesso em todos os jobs. macOS: AOT,
A1/B0/B1 e ARC 238/238, `dart:io` 130/130 e JIT × AOT 238/238 no placar,
zero divergências (sete sem IR, iguais por construção); relatórios conferidos.
Nova rodada [37902293030](https://github.com/insinfo/dartforge/actions/runs/37902293030),
suíte `nativo`, disparada sobre `8b4b79b8`, para validar a preparação
excepcional separada e as análises novas nas plataformas do CI. Nenhuma
rodada Pesado estava ativa no disparo. As análises ARC seguem fora da emissão.
Relatórios já conferidos desta rodada: Linux x86-64 AOT, A1/B0/B1 e ARC
238/238, `dart:io` 130/130 e JIT × AOT 238/238 no placar, zero divergências
(sete sem IR, iguais por construção). Windows A1/B1 com e sem estresse,
238/238; JIT × AOT com SDK da fonte 238/238 no placar, zero divergências
(sete sem IR, iguais por construção). Isso valida o passe excepcional
separado nestes casos/plataformas; não prova a integração de ownership,
que continua pendente. B0 concluiu os quatro testes dirigidos de mapas:
4/4, nenhum ignorado ou filtrado, 741,81 s. SDK da fonte concluiu AOT
238/238, os testes individuais de contratos, `script_e_spawn_uri_no_jit`
e produção autocontida. A etapa de recarga filtrava um nome antigo:
executou zero testes (sete filtrados), portanto não valida recarga nesta
rodada. O comando foi corrigido para executar os dois testes ignorados
existentes de preservação de estado com SDK da fonte; falta validar a
correção numa próxima rodada. A rodada `37902293030` concluiu com sucesso.
Relatórios finais do macOS conferidos: AOT, A1/B0/B1 e ARC 238/238;
`dart:io` 130/130; JIT × AOT 238/238 no placar, zero divergências (sete
sem IR, iguais por construção). Isso valida a preparação excepcional da
revisão `8b4b79b8`, sem provar as alterações posteriores ou ownership.
O job agora lista os testes ignorados de recarga antes de executá-los e
reprova seleção vazia. O filtro PowerShell foi conferido com listagens de
binários anteriores: dois testes com JIT, rejeição da lista vazia sem JIT;
isso valida a proteção do script, não os testes na revisão atual. A
validação local de recarga da revisão `7217e888` concluiu com sucesso:
`cargo test --locked --release -p dartforge-cli --features jit --test reload_state -- --ignored --nocapture`,
com `DARTFORGE_SDK_FROM_SOURCE=1`: dois testes passaram, zero falhas ou
ignorados, cinco filtrados (os não ignorados), em 14,43 s. Executados
`cli_preserva_estatico_apos_editar_o_mesmo_arquivo_dart` e
`cli_preserva_o_estado_do_espaco_unificado_em_tres_recargas`. A listagem
`--ignored --list` do binário novo também seleciona esses dois pelo filtro
do CI. SHA-256 do harness `reload_state-a53f60c187219ad4.exe`:
`44CD0183FE465FDCA8F7FA0B675335BABABF8D67A384379A037439519F17980C`.
Essa execução é local; a etapa corrigida foi posteriormente validada no
runner da rodada `37908897460` (resultados acima).

## Situação geral (2026-10-08, fim da tarde)

Foco pedido: o nativo AOT/JIT funcionando por inteiro, com `docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md` e
`docs/ARC-CICLOS-ESPECIFICACAO.md`, e os modos alternativos pelo menos tão rápidos quanto o padrão; depois o
analisador e o LSP. Medidas nesta máquina (Windows 11, i3-1215U — núcleos P e E misturados: tempos de
microbenchmark só valem presos a um núcleo P).

### Resumo por frente

| frente | o que já funciona | o que falta |
| --- | --- | --- |
| **Nativo AOT** (`crates/emit_native`, `runtime`) | corpus 238/238 em Windows, Linux x86-64 e macOS arm64; `--gc-stress` 238/238; `dart:io` 130/130 nos três; `new_sali/backend` compila e responde 39/42 rotas no e2e; produção com LTO e o SDK da fonte | `bench/performance`: A0/dart = 1,22 (média geométrica; JSON, objetos e textos 1,5–2,4×, numérico e listas abaixo do Dart); e2e do backend precisa do banco |
| **JIT** (`crates/jit`, `dartforge run`/`reload`) | 238/238 e JIT × AOT sem divergência nos três sistemas; recarga de estado e `io_regression` verdes | recarga com mudança de layout em todos os casos do R0 |
| **Exceções por tabelas (A1)** | 238/238 (+ `--gc-stress`); o pouso pega-tudo do `$ent` saiu (`a4b5ac42`): A1/A0 = 1,001 | tamanho: +3,2% no `new_sali/backend` (meta −3%); segue opt-in |
| **Raízes por mapas (B0/B1)** | forma nova por `"deopt"` (§14.12, abaixo): raiz no registrador preservado, sem derrame por chamada. Windows: corpus 238/238 em desenvolvimento com `--gc-stress` e as três conferências, e 238/238 em produção com o conferidor do RS4GC | testes dirigidos (`directed_maps`) caem num esgotamento de registradores do LLVM em `dart:convert` no modo de desenvolvimento (em curso); B1; Linux/macOS com a forma nova; o `bench/performance` inteiro; commit |
| **ARC** (`--memory=arc`) | ARC puro correto: corpus 238/238 com auditoria, `--gc-stress`, ciclos em toda drenagem e JIT × AOT; grafos aleatórios contra oráculo (3000 sementes); efêmeros pelo ponto fixo | desempenho: ARC/A0 = 2,000274 em 55beeb34 (árvores 12,82×, lista ligada 5,28×); gate reprovado. Metadados por página já disponíveis; análise, provas, políticas e integração dos §§27–34 continuam incompletas. Evidência: `bench/results/2026-10-10-modos-55beeb34-windows` |
| **JS desenvolvimento** (`crates/emit_js`, contrato do DDC) | corpus diferencial **238/238** byte a byte contra `dart run` (medido hoje, 60 s); `limitless_ui` 26/26 no e2e; `new_sali/frontend` com os 11 passos do fluxo iguais ao oficial | — |
| **JS produção** (`crates/emit_js_production`) | corpus `--production` 238/238; `new_sali` 12.082.759 bytes, `limitless_ui` 6.693.718 bytes (brutos) | piso de ~1 MB enquanto o runtime for o `dart_sdk.js` do DDC: compilar o SDK pela nossa trilha (PLANO passo 4); precisão do mundo fechado, minificação, *code splitting* (`docs/JS-PRODUCAO.md` §6) |
| **Analisador** (`crates/analysis`, `types`) | placar **22.965/23.012** (99,8%, posição exata), FP 0, posição errada 0, mensagem errada 10, FN 47 (2026-10-12, macOS); projetos reais sem diagnóstico a mais (10 projetos, 567 mutações; 209 pacotes do pub-cache sem FP de código publicado); CLI e LSP publicam **477** códigos de `verified.txt` | os 47 FN e as 10 mensagens (variância, limites F, `Null?`); os 13 códigos com FP nos pacotes do pub-cache (`unchecked_use_of_nullable_value`, `unused_import`, `argument_type_not_assignable`…) fora de `verified.txt`; caixa do caminho no macOS (o mesmo arquivo por dois caminhos de caixa diferente vira duas bibliotecas) |
| **LSP** (`crates/lsp`) | oráculo contra o `dart language-server` 3.6.2 (`r6`): símbolos, dobras, tokens, correções, hover, rename, highlight, selectionRange e workspace/symbol 100%; formatação (`formatting`, `rangeFormatting`, `onTypeFormatting`) pelo porte do `dart_style`, edições idênticas às do Dart; references 96%, implementation 96%, completar top-1/top-5 97%, assistências 98% (títulos; o resto são erros -32001 do próprio Dart) e 100% (edição, com `SnippetTextEdit`; reprise macOS 2026-10-12); latência mediana de references/implementation 1,5/1,1 ms (Dart 1,2/0,6; eram 269/221 ms) | nomes declarados dos parâmetros de tipo de função nos `Create …` (`p1`); incremental I5–I6 (dependem do motor em `crates/types`/`parity`) |
| **Geração de código e ngdart** (`crates/build`, `dartforge serve`) | builders pelo executor nativo; `limitless_ui` e `new_sali/frontend` pela nossa trilha | compilador de visões do ngdart (§2.0) |

### Analisador (rodada de 2026-10-12, macOS)

| medida | antes | agora |
| --- | ---: | ---: |
| Placar (posição exata) | 22.920/23.012 | **22.965/23.012** (99,8%) |
| FP / posição errada / mensagem errada / FN | 0 / 16 / 17 / 76 | **0 / 0 / 10 / 47** |
| Projetos do proprietário e de 2024 (10, `projetos`): FP | 429 (cópias com `pub get`) | **0** |
| Mutações desses projetos (567): FP | 122 | **7** (nenhum de código publicado) |
| Pacotes do pub-cache (209): FP / mensagem errada | 46 / 33 | **35 / 2** (nenhum de código publicado) |
| Códigos em `verified.txt` | 167 | **477** |

Como medir nesta máquina: `DARTFORGE_PARITY_SAMPLES=100000 dartforge-parity scoreboard --details
--workers 8` (o corpus e o oráculo gravado estão no repositório); para os projetos reais, copiar
`new_sali/{core,frontend,backend}`, `limitless_ui` e os de `2024/` para uma pasta própria (só `.dart`,
`pubspec.*`, `analysis_options.yaml`), rodar `dart pub get` lá (os `package_config.json` dos originais
apontam para o Windows) e `dartforge-parity projects --oracle --corpus <pasta com projects.json>`
(`--mutations 30 --rerecord` para as mutações); os pacotes do pub-cache pelo
`scripts/parity-pub-cache.py` (`PUB_CACHE=~/tools/dart/pub-cache`). `DARTFORGE_ORACLE_3_6` aponta
o SDK 3.6.2. Achados: FP nos projetos reais por anotação de FFI por alias de `typedef`, constante
privada usada só em `case`, anotação de diretiva, falso `top_level_cycle` de campos `late`; FP de
arquivo único (partes sem a biblioteca no lote, padrão de `super.x` sem corpos da superclasse); `/x.dart`
numa biblioteca de `lib/`; promoção por condição guardada antes da 2.15; captura de escrita em função
local. Os 310 códigos novos de `verified.txt` são os sem erro emitido no corpus (no placar de 22.954) e nesses projetos.

### Formatador (`dart format`)

`crates/formatter` é o porte do `dart_style` 2.3.8, o que o `dart format` do SDK 3.6.2 traz; o 3.6.2
formata no estilo "curto" (o "alto" só existe atrás do experimento `tall-style` e não foi portado).
Tokens do analyzer sobre o lexer do frontend, parser próprio que monta a árvore no formato do AST do
analyzer, e o `SourceVisitor` com `ChunkBuilder`, `LineSplitter` (busca por custo, 5000 tentativas,
fila com sobreposição) e a checagem de sanidade de brancos do original. `dartforge format` tem as
opções principais (`-o write|show|json|none`, `--show`, `--summary`, `--set-exit-if-changed`,
`-l/--line-length`, `-i/--indent`, stdin) com as mensagens e códigos de saída do `dart format`; o LSP
responde `textDocument/formatting`, `rangeFormatting` e `onTypeFormatting` com as edições mínimas do
`generateMinimalEdits` do analysis server.

Oráculo (`cargo run --release -p dartforge-formatter --example oracle`, referências do `dart format`
em cache em `target/oraculo-formatar/`), saída byte a byte:

| conjunto | idênticos |
| --- | --- |
| `.dart` do repositório (corpus/, examples/, native_sdk/, packages/…) | 9866/9866 |
| `lib/` do SDK 3.6.2 | 426/426 |
| pub cache (340 pacotes) | 27899/27899 |

Os arquivos que o `dart format` recusa (erro de sintaxe) ficam fora do placar. LSP: contra o
`dart language-server` 3.6.2, 80 arquivos não formatados do corpus com edições idênticas em
`formatting` (80/80) e em `rangeFormatting`/`onTypeFormatting` (400/400).

Todo arquivo que o `dart format` aceita sai idêntico (38191/38191), e o parser não recusa nenhum deles;
a árvore confere nó a nó com a do `parseString` do analyzer 6.11 nos três conjuntos.

Falta: as correções `--fix*` (não suportadas, o comando recusa); `--selection`; a
mensagem de erro de sintaxe tem o formato do `dart format`, mas o texto do diagnóstico é o do nosso
parser; o estilo "alto" (padrão do `dart format` a partir do SDK 3.7).

### Nativo: modos de raízes e exceções contra o padrão

`scripts/measure-performance-modes.py` (`bench/performance` em produção, 32 núcleos, 7 execuções alternadas,
média geométrica contra A0 = pilha-sombra + checagem):

| modo | antes da forma por `"deopt"` |
| --- | ---: |
| A1 (pilha-sombra + tabelas) | 1,001 |
| B0 (mapas + checagem) | 0,996 (`formas` 1,43×) |
| B1 (mapas + tabelas) | 1,005 |
| ARC | 2,972 |
| A0 contra `dart compile exe` | 1,220 |

A regressão do B0 em `formas` era da forma do §14.8: cada raiz tinha dois nomes (`%v` e o `%raiz` relocado
pelo statepoint e mantido pelo `llvm.fake.use`), e cada uma era gravada e recarregada em toda chamada que
coleta. Na forma nova (§14.12) a raiz vai no operando `"deopt"` e fica no registrador preservado
(`-use-registers-for-deopt-values`); o mapa (DFGM v2) leva a máscara dos registradores, e o percorredor os
lê no contexto desenrolado. `chamadas`, presos a um núcleo P (ms):

| núcleo | A0 | B0 (§14.8) | B0 (`"deopt"`) |
| --- | ---: | ---: | ---: |
| `formas` | 5,38 | 7,53 | 5,29 |
| `fib` | 10,88 | 11,40 | 10,37 |
| `closures` | 40,95 | 30,78 | 30,12 |

Achados no caminho, consertados: constantes estáticas e cargas dos canônicos `true`/`false` no `"deopt"`
esgotavam os registradores (o LLVM as rematerializa; agora saem do operando antes do RS4GC); chamadas do
prólogo e de dentro dos ajudantes `@df.*` (corpo sem indentação) ficavam sem as raízes; e uma corrida
antiga do ThinLTO distribuído — o `lld-link` grava o índice ao lado de cada entrada, inclusive dos
bitcodes do SDK, e compilações paralelas se sobrescreviam (215 de 238 falhavam com `--jobs 4`).

### Não commitado ainda

A forma por `"deopt"` (emissor, `crates/llvm`, conversor, runtime, verificadores, spec §14.12) e, no ARC,
o filtro acíclico dos candidatos e o tempo da drenagem no rastro; `docs/ARC-CICLOS-ESPECIFICACAO.md` entra
no commit do ARC, atualizada com a implementação.

### Ordem de trabalho

1. Mapas: o esgotamento em `dart:convert` (desenvolvimento), os testes dirigidos, B1, o `bench/performance`
   inteiro com os cinco modos; commit e CI (Linux/macOS com a forma nova).
2. ARC: metadados por bloco (§19.1), retain/release em linha, drenagens sem `HashSet` por objeto; medir
   até ficar perto de A0; atualizar e commitar a `ARC-CICLOS-ESPECIFICACAO.md`.
3. Analisador: os 17 FP e os FN; ampliar `verified.txt`.
4. LSP: incremental I5–I6 (snippets das ações de código e latência de references/implementation feitos em 2026-10-12).

## Rodada de 2026-10-08, manhã: nativo (ARC, Linux e macOS) e analisador

Foco pedido: o nativo AOT/JIT funcionando por inteiro, com `docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md` e
`docs/ARC-CICLOS-ESPECIFICACAO.md`; depois o analisador e o LSP.

### Nativo

| medida | Windows | Linux x86-64 | macOS arm64 |
| --- | ---: | ---: | ---: |
| corpus nativo (AOT) | 238/238 | 238/238 | 238/238 |
| JIT, e JIT × AOT | 238/238, 0 divergências | 238/238, 0 | 238/238, 0 |
| `--gc-stress` | 238/238 | — | — |
| A1 (pilha-sombra + exceções por tabelas) | 238/238 (Pesado) | 238/238 (era 201/238: o lançamento passava por uma função Rust) | 238/238 |
| B0 (mapas + checagem) | 238/238 (Pesado) | 238/238 | 238/238 |
| B1 (mapas + tabelas) | 238/238 (Pesado) | 238/238 | 238/238 |
| memória ARC (`--memory=arc`, auditoria a cada sincronização) | 238/238; ciclos em toda sincronização 238/238; `--gc-stress` 238/238; JIT 238/238 | 238/238 | 238/238 |
| `dart:io` (`corpus/native`) | 130/130 | 130/130 | 130/130 (era 129: a espera com prazo do laço de eventos passava do prazo; agora pelo `kevent`) |
| builders pelo executor nativo | todos | — | — |

* **ARC** (`docs/ARC-IMPLEMENTACAO.md`): o RC das ocorrências fortes entre objetos velhos, com as raízes
  adiadas e o berçário pela coleta menor; a gravação num velho conta na hora, o acesso cru tira uma foto;
  na completa, o ponto fixo global (§22.3: alcance pelas raízes com a regra condicional dos efêmeros, lote
  único dos inalcançáveis); *trial deletion* lateral com `DARTFORGE_ARC_CYCLES=always`; reclamação pelas
  marcas do RC. Grafos aleatórios com semente contra um oráculo independente (3000 sementes por modo); a
  primeira rodada achou dois vazamentos, corrigidos (o zero que volta a subir não virava candidato; o ciclo
  valor→portador de um efêmero ficava vivo pela chave). Opt-in por `--memory=arc` no AOT e no JIT; o
  rastreamento continua o padrão, com o IR de sempre.
* **Exceções por tabelas no Linux**: a primeira execução (A1/B1) parou em 37 programas; o lançamento passava
  por uma função Rust com a guarda de abortar na variante `panic=unwind` do runtime. O `@df.throw` chama o
  `_Unwind_RaiseException` direto.
* **§11 da especificação dos mapas**: itens 7 (`ld.lld`/`ld64.lld` com o RS4GC na LTO), 8 (o RS4GC cai com EH
  por funclets), 12 (`link.exe`), 13 (inliner e `gc`), 14 (`C-unwind` sob `panic=abort`) e 16 (bytes por
  instrução) resolvidos; o 5 no Linux e no macOS passou a rodar no Pesado.

### Analisador

| medida | 2026-10-07 | agora |
| --- | ---: | ---: |
| Placar (posição exata) | 22.867/23.012 | **22.895/23.012** (99,5%) |
| FP / FN | 57 / 121 | **31 / 95** |

* `DEPRECATED_OPTIONAL` do 3.13.4; modificadores antes do `this` e o `const` sem construtor primário
  (3.13.4); o `dart:core` implícito decidido só pela unidade definidora (o `String` do macro 411 voltou a
  resolver).
* A CLI e o LSP publicam só os códigos de `crates/analysis/verified.txt` (167); 391 códigos já têm zero
  FP, posição e mensagem certas no corpus e esperam a conferência nos projetos reais para entrar.

## Fechamento de 2026-10-06

Frentes desta rodada: as quatro especificações (`docs/ANALYZER-ESPECIFICACAO.md`,
`docs/ANALYZER-ESPECIFICACAO-INFRA.md`, `docs/LSP-ESPECIFICACAO.md`,
`docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md`), implementadas com fidelidade ao analyzer e ao
`analysis_server` 3.6.2, sem aproximações. Tudo está na `main` (83 commits desde `fa8cfa96`,
2026-10-02 → 2026-10-06; o último é `0ab14440`).

### Números

| medida | 2026-10-02 | agora |
| --- | ---: | ---: |
| Placar do analisador (corpus, posição exata) | 80,5% (19.015 na r8, não publicada) | **99,3%** — 22.841/23.012 (sem o arquivo em que o 3.13.4 cai) |
| FP do placar | 493 → 339 (r8) | **78** (era 1.339 no começo de 2026-10-05) |
| FN do placar | — | **146** (era 2.095 no começo de 2026-10-05) |
| Projetos reais (`new_sali` core e frontend, `limitless_ui`): diagnósticos sem par no `dart analyze` | 61 | **0** |
| Lints novos (`E:\dftemp\lints_novos`, 14 regras) | — | 211/211 iguais ao `dart analyze` |
| Casos s01–s12 do T5 (`corpus/specification/t2/t5`) | — | 12/12 iguais |

Oráculo do LSP (`crates/lsp/oraculo`, `dart language-server` 3.6.2 contra o `dartforge-lsp`,
projetos `app`, `args`, `path`, `string_scanner`; rodada `r6` de 2026-10-06, com as reprises por
método depois dela; a métrica do top-5 do completar passou a contar lista vazia contra lista vazia
como igual):

Reprise de 2026-10-12 (macOS arm64, Dart 3.6.2 em `~/tools/dart/3.6.2`, por `ORACULO_DART`/`ORACULO_SDK_LIB`;
cópias de `args-2.7.0`, `path-1.9.1` e `string_scanner-1.4.1` do pub-cache sem `dev_dependencies` e um `app`
novo com os mesmos erros comuns; o cliente anuncia `experimental.snippetTextEdit`; os caminhos das cópias na
caixa do disco; com outra caixa o DartForge não achava as unidades, corrigido em `e67fa017`), antes → depois
dos commits LSP de 2026-10-12 (`8f165ec1` … `aeff64f8`): assistências (títulos) 98 → 98% (as 7 posições que
restam são `-32001` do servidor do Dart, em imports com prefixo); edição das assistências de título comum
(métrica nova: `TextEdit`/`SnippetTextEdit` normalizados, com o `insertTextFormat`) 70 → 100% (391); edição das
correções de título comum 68 → 100% (25); references 96 → 96% e implementation 96 → 96% (respostas idênticas às
de antes nas 410 posições); mediana de references 269,0 → 1,5 ms (Dart 2,2 e 1,2 nas duas rodadas) e de
implementation 220,9 → 1,1 ms (Dart 0,8 e 0,6); com `2efbd7f6` (as posições de locais no disco convertidas uma
vez por arquivo) o p90 de references caiu de 1872,6 (antes) e 1328,5 ms para 42,8 ms (Dart 173,4; reprise só de
references/implementation/rename: medianas 2,1/1,4/1,4 ms contra 2,4/1,1/1,6 do Dart). Os demais recursos ficaram
como na tabela (o `resumo.py` voltou a contar lista vazia contra lista vazia no top-5 do completar: 97%).

| recurso | 2026-10-01 | agora |
| --- | ---: | ---: |
| documentSymbol / foldingRange / semanticTokens / documentLink | 97 / 100 / 100 / 100% | 100 / 100 / 100 / 100% |
| correções (quickfix) | 75% | 100% |
| ações de fonte (Sort Members, Organize Imports, Fix All) | 0% | 99% |
| hover (texto / assinatura) | 86 / 97% | 100 / 100% |
| definition / typeDefinition | 96 / 95% | 98 / 100% |
| implementation | 77% | 96% (o resto é a busca de subtipos no SDK) |
| references | 58% | 96% |
| documentHighlight | 95% | 100% |
| prepareRename / rename | 96 / 89% | 100 / 100% |
| assistências (refactor): títulos / edição | 26% / — | 98 / 100% |
| selectionRange | 48% | 100% |
| prepareCallHierarchy / prepareTypeHierarchy | 86 / 76% | 99 / 100% |
| completion: alvo / top-1 / top-5 | 98 / 63 / 52% | 100 / 97 / 97% |
| signatureHelp | 99% | 100% |
| workspace/symbol | 0% | 100% |
| inlayHint (arquivos) | 50% | 97% |
| formatting | 0% | 0% (fora do escopo: porte do `dart_style`) |

### O que foi feito, por especificação

**Analisador (ANALYZER-ESPECIFICACAO e INFRA).**
* Parser com a recuperação do fasta: agrupamento do scanner (`discardBeginGroupUntil`,
  `insertSyntheticClosers`, o quirk do `<`), `moveSynthetic`, `ensureIdentifier` por contexto,
  seletores pela `parsePrimary`, laços de progresso com `UNEXPECTED_TOKEN`, `ensureColon`,
  criação implícita `C<T>.n(…)` (`parseImplicitCreationExpression`), `assert(…)` como
  `FunctionExpressionInvocation`, `super` sem parênteses num inicializador, `}`/fim no lugar de um
  comando, declaração de topo com nome sintético (`TopLevelDeclarationIdentifierContext`).
* T5 fechado: as portas de sintaxe (`libs_com_erro_de_sintaxe`, `recuperacao_do_parser`) saíram; a regra
  é a dos trechos pulados, no CLI, na paridade e no LSP. O `UnusedLocalElementsVerifier` roda em
  toda biblioteca, com os ramos de operador, índice, `call` e redirecionamento do coletor de usos.
* Recuperação estática do `TypePropertyResolver` (`c.s` estático pela instância, na leitura, na
  chamada e na escrita), escrita anulável com `UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE`,
  `NEW_WITH_UNDEFINED_CONSTRUCTOR`/`_DEFAULT` (faltavam), `WRONG_NUMBER_OF_TYPE_ARGUMENTS` em literais de
  tipo, `E.values` constante, contexto numérico da atribuição composta, `isNullable` do analyzer no
  `body_might_complete_normally`, `tryPromoteForTypeCheck` sem promoção a `Never`,
  `built_in_identifier_as_type` só onde o fasta relata, nomes de tipo sintéticos sem relato.
* INFRA: mensagens de contexto (III.3), saída do CLI (III.4), pipeline por biblioteca e ordem das
  fases (III.5, itens 1–3), fases ausentes (III.6), harness da saída inteira (s1–s7).
* Lints: as regras novas pelo elemento e pelo tipo resolvido; o placar e o `projetos` filtram os TODOs
  como o `dart analyze`; as extensões usadas de cada passada se juntam (os `unused_import` falsos).

**LSP (LSP-ESPECIFICACAO).**
* Árvore no formato do analyzer com `NodeLocator`; correções, ações de fonte, refatorações (Extract
  Method/Local, Inline Local/Method, Convert Getter/Method, Move to file), rename, references com o
  SDK e os pacotes, documentHighlight, call/type hierarchy, workspace/symbol, completar com o
  `FuzzyMatcher`, o coletor e os não importados, dicas, cores, lentes de augmentation, `dart/` métodos,
  `willRenameFiles`, configuração por pasta, diagnósticos tipados no fluxo contínuo (I7).
* Incremental I1–I4: troca de unidade por assinatura de API, inferência de corpos isolados,
  completar especulativo e `didChange` de corpo sem recarga.
* M1: o SDK como o analyzer vê (`SdkLayout::load_como_analyzer`, sem patches, internas do dart2js);
  implementation pela carga ampla.
* Assistências portadas dos produtores: Convert to block body, Add/Remove type annotation, Convert to
  async function body, Convert to final field, Convert to getter, Convert to normal parameter, Convert
  class to a mixin, Invert conditional expression (pela regra do produtor), e as anteriores
  (`assistencias*.rs`).

**Nativo (NATIVO-MAPAS-DE-PILHA-E-EXCECOES).**
* Rastro no formato da VM (§13.14) com quadros embutidos e `<asynchronous suspension>`, a forma
  compacta DFPC no conversor COFF/ELF, a tabela do rastro no JIT, o conferidor de dominância das raízes
  no modo sombra, D7 por extern pela tabela de efeitos, D9 (callback da FFI que lança, `Isolate.exit`
  em `finally`), as combinações A1/B0/B1 no pesado do CI.
* Etapa 1 (exceções por tabelas) escrita conforme o §13.15 e compilando no workspace.

**Nativo, depois do fechamento (2026-10-06, `abcaf6fc`…`a14d2270`).**
* Testes do `dartforge-jit` e do `dartforge-llvm` verdes, inclusive os ignorados (era só o `bin` do LLVM
  fora do `PATH`); `reload_state` e `io_regression` 7/7.
* **Etapa 1 (A1) verificada e arquivada** (§13.16): corpus/js 238/238, corpus/native igual a A0 com e
  sem `--gc-stress`; no `new_sali/backend` o `.text` não cai e as tabelas somam 1,57 MB (+3,2% contra a
  meta de −3%). O padrão segue `checagem` com o E1.1.
* **Etapa 2 (B0) verificada e arquivada** (§14.11): corpus B0 com e sem `--gc-stress` igual a A0, os 4
  testes dirigidos de `directed_maps` verdes com cada sabotagem pegando, produção B0 correta (ThinLTO
  distribuído, o passe depois da otimização); no mesmo pipeline o `.text` do `bench/performance` em B0
  é 25% maior, mais 100 KB de mapa. As raízes por mapas ficam experimentais.
* Defeitos achados ao rodar e consertados: pouso com `fake.use` fora dos mapas, chamada C variádica em
  função `gc`, conferidores do RS4GC e da sombra (apelidos de raiz, ajudantes, parâmetros), quadro morto
  e exceção que atravessa Rust sem porta (agora acusados), `__tmp_use` e RS4GC antes da LTO.
* CI: produção sem o Clang (N15: o gerador embutido e a LTO completa do `lld`), `__Unwind_GetGR` na
  `libSystem.tbd` do macOS, o placar consolidado do Pesado sem os relatórios dos modos e o job dos modos
  com o harness do gerador embutido.

**JS de produção (2026-10-06, `518e5388`).**
* `this` repetido como local (§8.4 de `JS-PRODUCAO-TAMANHO.md`): `new_sali` 12.361.194 → 12.082.759
  bytes, `limitless_ui` 6.946.316 → 6.693.718; corpus `--production` 238/238, `limitless_ui` 26/26 no e2e,
  `new_sali` com o `flow.mjs` nos 11 passos sem erro.
* `@pragma('dart2js:as:trust')`/`tryInline`: avaliados e **sem efeito no nosso contrato**. Os
  `as:trust` do SDK estão só nas bibliotecas do runtime do `dart2js` (`js_runtime`, `js_shared`), que
  não compilamos; o `tryInline` é dica a um *inliner* que o emissor no contrato do DDC não tem.
* Construtor sem `.new`: 30.280 citações no `new_sali` (~60 KB com um nome curto). O runtime do DDC lê
  `new` por texto (`defineNamedConstructor`, tear-offs de construtor), então é mudança de contrato, do
  grupo da §7 (emissor no estilo do `dart2js`, spec nova), não um passo de compactação.

### O que falta

1. **Analisador** (placar `placar_det219`, 22.759/23.030; perda 271 em 111 códigos, quase todos com 1 a 5).
   * Maiores perdas: `dead_code` 14, `concrete_class_with_abstract_member` 12,
     `unchecked_use_of_nullable_value` 9, `missing_identifier` 8, `unused_element` 7, `use_of_void_result` 6,
     `expected_identifier_but_got_keyword` 6, `const_initialized_with_non_constant_value` 6.
   * Arquivos com `augment` (o 3.6.2 os lê como declarações homônimas): parte de `concrete_class_with_abstract_member`,
     `invalid_implementation_override` e `enum_with_abstract_member`.
   * Só do 3.13: `deprecated_optional` (porta do `DeprecatedFunctionalityVerifier`), restos de atalhos de ponto e de
     construtores primários.
   * `dot_shorthands/equality/equality_extension_override_error_test.dart` tem oráculo 3.6.2 num
     arquivo que o nosso parser marca 3.13 (o "só nosso 1" da referência 3.13.4): regravar com o 3.13.4.
   * Recuperação do parser com palavras embutidas em posições de tipo e de declaração (sondas em
     `E:\dftemp\stm\lib\bi.dart`: `List<abstract>`, `abstract y = 1;` local, `factory f;` em classe,
     `typedef abstract T(…)`).
   * T5 passo 8 (o `importacoes::nao_usados` pelo `Coletor`): refatoração sem mudança de regra.
   * INFRA III.9 e os itens de precisão restantes das famílias.
2. **LSP.**
   * Os 73 produtores de assistência registrados no `assist_internal.dart` do 3.6.2 estão portados
     (o `UnwrapIf` tem espécie mas não é registrado). Falta a moldura com `SnippetTextEdit` e os grupos
     de edição ligada (os produtores escrevem o texto padrão, como num cliente sem snippets).
   * Latência de references e implementation (≈290 e 227 ms de mediana contra 3 e 2 ms do Dart, que
     responde do índice); inlayHint com o nome de parâmetro vindo de tipo de função (os tipos de
     função não guardam os nomes dos posicionais).
   * Incremental I5 (cache por corpo com offsets relativos, diagnósticos tipados incrementais) e I6
     (edição de assinatura sem recarga total).
   * Importação condicional no LSP: o servidor do Dart não declara `dart.library.*`; o
     `load_como_analyzer` marca todas as bibliotecas como suportadas.
3. **Nativo.** As Etapas 1 e 2 foram verificadas, medidas e arquivadas (acima). Falta conferir no CI
   os consertos de 2026-10-06 (sem toolchain, macOS, Pesado) e o e2e do `new_sali/backend`, que precisa
   do banco.
4. **Documentação.** As notas "escrito, não compilado" das especificações estão desatualizadas: o
   código compila e os testes do workspace passam (fora jit/llvm por DLL). Atualizar cada nota com o
   estado medido.
5. Trabalho e pendências de 2026-10-02 que seguem abaixo (CI sem MSVC, JS, pub) não foram retomados
   nesta rodada.

### Continuação de 2026-10-06

* **Assistências:** `ConvertClassToEnum` (constantes deduplicadas pelo avaliador),
  `ConvertToSwitchExpression` (com o `isAlwaysExhaustive` e o `throw` seguinte), as 13 do Flutter
  (embrulhar, mover, remover, trocar, `children`, filho por último, `StatefulWidget` e
  `StatelessWidget`) e o `AddDiagnosticPropertyReference`, com os casos de teste do
  `analysis_server` portados e um pacote `flutter` reduzido nos testes.
* **Analisador:** o campo `late` em inferência fica provisório na interface da própria classe (o
  falso `top_level_cycle` de `late final a = m();` sumiu e o tipo deixou de ser `dynamic`); o receptor
  literal de tipo de `C.m()` com `m` indefinido fica resolvido; o SDK como o analyzer usa o
  `dart2jsPath` do `html_common`. Placar estável (21.176 acertos, FP 588).
* **Documentação:** porte do `DocCommentBuilder` (linhas, blocos indentados e cercados, diretivas
  `{@…}`, `@docImport`, `@nodoc`, texto de link e o scanner do Dart nas referências), usado pela
  árvore e pelo LSP.
* **LSP:** dicas de anotações, `super(…)`/`this(…)`, setter de método, padrão objeto, `for-in` de
  coleção e tipos crus aninhados; hover com o `computeDocumentation` (`Copied from`, parâmetro pelo
  executável, acessor sintético sem comentário) e o `Member` substituído; typeDefinition e
  documentHighlight como os do Dart; references e rename pelo índice (construtor de `[A.ctor]` sem o
  prefixo, operador em `[int.+]`, `new A.n()`, `extends Object` implícito, arquivos candidatos,
  `getImportElement`, `super.x` e argumentos de comprimento 0 dos `this.x`); implementation pelo
  `findMemberElement`; completar no nome de `A.n()` e `C.m()`.

### Continuação de 2026-10-07, analisador (placar 22.617 → 22.759)

Cada correção saiu da fonte do analyzer 3.6.2 (ou do checkout main, para os códigos do 3.13), foi
medida no placar e registrada na seção do código na especificação. Em resumo:

* **Scanner:** a string sem fecho, o caractere inesperado e o comentário de bloco sem fecho não interrompem
  mais a leitura. O lexer segue como o do fasta, e os erros saem dos tokens.
* **Tipos:** inferência de mixin no outline (`_MixinInference`, com o `topMerge` do `InterfacesMerger`); alvo
  genérico ou por alias da factory redirecionadora; `index` do `Enum` concreto; `noSuchMethod` só encaminha o
  que falta; `isStrictlyNonNullable` inteiro.
* **Verificações:** padrão em nomeado `required` em toda lista; erro de biblioteca adiada nas constantes pelo
  ancestral; campo e método de enum contra herdados; primário de tipo de extensão contra estáticos; tipo
  anulável no `implements` de tipo de extensão; campo inicializado na declaração e no construtor primário;
  contagem de argumentos de tipo no `call` implícito.
* **Resolução:** estático privado de outra biblioteca; método em literal de alias de função instanciado;
  extensões ambíguas pelo `this` implícito; setter de extensão sem getter possível; criação implícita com
  nome que não é classe pela regra do `AstRewriter`; atalhos de ponto com classe abstrata e argumentos de tipo;
  padrão de tipo inválido que não cobre o casado; `readType` inválido no `??=` sem getter.

### Continuação de 2026-10-07, analisador (placar 22.335 → 22.617)

Cada correção saiu da fonte do analyzer 3.6.2 (ou do checkout main, para os códigos do 3.13), foi
medida no placar e registrada na seção do código na especificação. Em resumo:

* **Mensagens e posições:** supertipo da variância pelo alias escrito; diretivas de doc pelo
  deslocamento do `_DirectiveParser`; atalhos de ponto potencialmente constantes; tipos desambiguados
  (`(where C is defined in …)`) na atribuição.
* **Constantes:** instanciação implícita de tear-off com parâmetro de tipo; `CONST_TYPE_PARAMETER` em
  padrão, `case T` e anotação de parâmetro de tipo; ciclos de constante de enum e do `values`; local
  declarado adiante; criação pelo primário de tipo de extensão; `as` com o ambiente de tipos do
  construtor.
* **Fluxo e inferência:** promoção por `is` no código inalcançável; promoção de campo pelo alvo de
  cascata; variável de padrão promovida pelo tipo casado e junção de casos compartilhados; `??=` com a
  escrita só no ramo do nulo; formais de campo e de `super` finais.
* **Verificações:** entradas fora de mapa e expressões em mapa (com o parser de listas do fasta);
  `final` no `for-in`; `call` que não é método; `==` em override de extensão; getter/setter de enum,
  extensão e tipo de extensão; parâmetro de tipo em membro estático; anotações locais, de alias e de
  tipos de função; `this` implícito em inicializador de campo; setter estático indefinido; `x.new`;
  retorno de construtor gerador com `=>`; índice de escrita composta contra o `[]=`; final não
  inicializado com campo repetido e na representação.

### Continuação de 2026-10-06, analisador (placar 21.974 → 22.335)

Cada correção saiu da fonte do analyzer 3.6.2 (ou do checkout main, para os códigos do 3.13), foi
conferida com o `dart analyze` 3.6.2 quando a fonte deixava dúvida, e entrou na especificação junto
com o código.

* **Fechados em 100%:** `prefix_shadowed_by_local_declaration`, `assignment_to_primary_constructor_parameter`,
  `private_optional_parameter`, `duplicate_field_formal_parameter`, `extraneous_modifier_in_primary_constructor`,
  `invalid_use_of_type_outside_library` (alias seguido até a classe), `const_with_undefined_constructor`
  (`const .id(…)` do atalho de ponto), `instance_access_to_static_member`, `assignment_to_final_no_setter`.
* **`could_not_infer`** (38 → 51 de 54, mensagens todas certas): decisão pela escolha do `_chooseTypes`
  como o `tryChooseFinalTypes`, `isSatisfiedBy` pelo fecho maior, `_` como topo e fundo no subtipo,
  restrições de parâmetro já fixado descartadas, esquema `dynamic` tratado como `_` em toda expressão
  e no retorno `dynamic` não imposto, tear-off, `call` implícito e literais de coleção com relator.
* **`unused_element`** (FP 29 → 0, FN 32 → 8), **`unchecked_use_of_nullable_value`** (FN 28 → 9),
  **`invalid_assignment`** (mensagens 17 → 5, valor padrão conferido, `?.` sobre literal de tipo,
  `call` implícito), **`experiment_not_enabled`** (FN 60 → 6, com os recursos anteriores ao piso:
  `nonfunction-type-aliases`, `generic-metadata`, `class-modifiers`, `sealed-class`).
* **Subtipagem:** tipo de extensão sem `implements` que leve a classe não é subtipo de `Object`;
  `this` fora de contexto de instância tem o tipo da declaração; local potencialmente não anulável
  por `!(Null <: T)`.
* **Segunda leva (22.257 → 22.335):** `mixin_class_declares_non_trivial_generative_constructor` (16/16,
  regra do 3.13), `prefix_identifier_not_followed_by_dot` (15/15), `return_of_invalid_type_from_closure`
  (32/32: toda closure conferida contra o retorno inferido e ajustado ao contexto),
  `extra_positional_arguments` e `not_enough_positional_arguments` (100%: construtor do atalho de ponto
  pela criação comum, primeiro construtor repetido, `this.x` obrigatório fora de construtor), e a
  constante de enum avaliada como criação (`const_eval_throws_exception` 36 → 43,
  `const_constructor_param_type_mismatch` e `const_constructor_field_type_mismatch` completos).
* **Pendente conhecido:** promoção de campo do alvo de cascata (`c?.._field()`), curingas com o
  experimento `primary-constructors` desligado, criação por alias com limite F. O tipo
  inferido da constante de enum genérica foi resolvido (a constante é tipada pela criação inferida).

### Ferramentas desta rodada

* `scripts/compare-scoreboards.py` (dois placares, código a código) e
  `scripts/remove-function.py`.
* Oráculos em `E:\dftemp`: `cmp.py` (corpus contra `dartforge analyze`), `oraculo_lsp.py`,
  `oraculo_multi.py`, `oraculo_diag.py` (publicações de diagnóstico), `lsp\dif_metodo.py`
  (diferenças por amostra de um método do oráculo do LSP).
* Placar: `DARTFORGE_PARITY_SAMPLES=100000 ./target/release/dartforge-parity scoreboard --details`;
  projetos reais: `dartforge-parity projects`.

---

## Fechamento do dia 2026-10-02

### O que entrou no `main` hoje

| commit | o quê |
| --- | --- |
| `f6d11d9e`, `5325e977`, `2d2571ab`, `c47214be` | JS de produção: SDK próprio como padrão, mundo fechado incremental, `oxc_minifier`, nomes de propriedade minificados, rti só para tipos testados, parâmetros nomeados com chave curta, `--omit-checks`/`-O4` opcional, `assert` desligado por padrão (`--enable-asserts`) |
| `40e33061`, `fbeded9b` | Nativo: HIR em paralelo e poda da HIR; SDK de produção em ThinLTO, LTO `-O1` no programa em partes, corpo de objeto `memory(none)`, trampolins da FFI só das assinaturas pedidas (3.591 → 44), literais constantes grandes por tabela |
| `36584c8e` | Parser: `part of` com experimento macros |
| `bc0f9c7d` | `docs/ANALYZER-ESPECIFICACAO.md` (436 códigos com perda, achados T1–T8) e `docs/LSP-ESPECIFICACAO.md` |
| este commit | `docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md` (chave `--roots=shadow\|mapas` e `--exceptions=check\|tabelas`, etapas com critério de abandono) |

CI verde em `fbeded9b`; placar do pub verde (232/244). Falhou o
workflow "Sem toolchain do sistema" (Windows sem MSVC, passo "Ligar e
executar sem MSVC") em `fbeded9b` — **investigar primeiro amanhã**. O
Pesado de `fbeded9b` ainda rodava no encerramento.

### Números (bytes brutos)

| JS | início do dia | agora | `dart2js -O4` |
| --- | ---: | ---: | ---: |
| `new_sali/frontend` | 24.895.868 | 12.356.255 | 7.616.039 (`-O2` 8.022.592) |
| `limitless_ui` | 14.043.321 | 6.942.798 | 4.511.811 |
| `01_print` | 257.738 | 156.012 | 34.929 |

Compilação do `new_sali`: 14 min 22 s → 29–45 s (`dart2js` 57 s);
`limitless_ui` 8 min 23 s → ~13 s.

Nativo, `new_sali/backend` em `aot --optimize`: 94,0 → 57,2 MB (meta do
`dart compile exe`: 27,3 MB). Frio 405 → 340 s medido **antes** dos
literais por tabela; a única rodada com tudo (560 s) foi com a máquina
cheia — remedir com a máquina vazia. Religação sem mudança 148 → 57–65 s.
Desenvolvimento frio 91–101 s (1,5–1,7× o Dart). e2e 39/42.

Analisador: paridade 80,5% na última medição publicada; a medição
intermediária r8 deu 82,6% (19.015/23.030; FP 493 → 339), mas **não foi
publicada**.

### Trabalho guardado (fora do `main`)

* **Analisador (r3, T1–T4, famílias):** a árvore não compilava
  (`ForInit::Pattern` novo sem os braços em
  `crates/types/src/constantes/verificador.rs` e
  `inferencia/instrucoes.rs`) e quatro códigos publicados davam erro no
  corpus (`conflicting_static_and_instance`, `enum_without_constants`,
  `unnecessary_null_comparison`, `invalid_override`). Tudo continua no
  working tree e também em `E:\dftemp\analise\wip-2026-10-02.patch`.
* **JS:** `this` como local (−278 KB no `new_sali`, não validado) em
  `E:\dftemp\jsprod\guardado`.
* **Pub:** normalização de durações do placar em
  `E:\dftemp\backend-real\pub-placar-duracoes.diff`.
* Experimentos dos mapas de pilha e patches do linzj em
  `E:\dftemp\spec-mapas`.

### O que falta, por frente (ordem de prioridade)

1. **CI:** consertar o "Sem toolchain do sistema" (Windows sem MSVC).
2. **Analisador:** fechar `ForInit::Pattern` em `types`; corrigir os 4
   códigos publicados com erro; terminar T1 (homônimos/`augment`: a
   primeira declaração vence) e T3; placar sem regressão; publicar os 12
   candidatos novos; depois as famílias A–F da especificação.
3. **LSP:** source actions (Sort Members, Fix All), assists, selectionRange
   48%, references 58%, completion top-1 63%; inferência incremental por
   corpo.
4. **Nativo:** etapa 1 da `NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md` (exceções
   por tabelas com a pilha-sombra atual; abandonar se `.text` + tabelas não
   cair ≥ 3%); depois o protótipo de mapas no Windows x86-64. Pilha
   simbólica no formato da VM junto (7 das 12 divergências do pub).
   Estabilizar a chave do cache do ThinLTO (uma edição pequena refaz 43
   módulos). Remedir o frio com a máquina vazia.
5. **Pub (12 divergências):** 7 texto de stack trace, 3 tempo (`args`,
   `html` noah_ark, `mysql`), 1 mirrors, 1 `resolvedExecutable` (já
   `aot-diferente`).
6. **JS:** reaplicar e validar o `this` como local; construtor sem `.new`
   (~80 KB); honrar `@pragma('dart2js:as:trust')`/`tryInline` (~30–50 KB);
   o resto do intervalo até o `dart2js` pede um emissor de produção no
   estilo `dart2js` (spec nova).
7. Pendente de resposta do dono: apagar `E:\dfpoda-antes`,
   `dfpoda-depois`, `dfpoda-medidas`, `dfantes`, `llvmwrap`.


## Analisador: paridade com o analyzer 3.6.2 (2026-10-01)

**Placar: 16.014 → 17.428/23.030 na posição exata (69,5% → 75,7%)**,
mensagem igual 15.581 → 17.000, FP 667 → 617, FN 6.749 → 5.351, posição
errada 267 → 251 (o "12.241" do pedido era anterior aos passos 2–4 do A01).
Especificação e resultado por código: docs/ANALISADOR-PARIDADE-PLANO.md. O
que entrou: diagnósticos de `types` com a unidade (não mais adivinhada pelo
intervalo), nomes de tipo pelo `NamedTypeResolver` inteiro (corpos e
outline, escopo de instância, número de argumentos de tipo), a regra de
elementos iguais por localização da 6.11 nas duplicatas, código morto em
expressões, atribuição definida de `final`/`late`, avisos de tipo
(`unnecessary_type_check`, `unnecessary_null_comparison`,
`dead_null_aware_expression`, `?..`/`...?`/`C?.x`), extensões ambíguas,
anotações, `super` (membro concreto, abstrato, indefinido), padrões que
nunca casam, receptor `Never`, classe abstrata instanciada, retornos
(`return;`, closures, `yield`), parâmetro de tipo em estático, variável não
anulável sem inicializador. Importação condicional pela URI principal, como
o analyzer. **Publicados: 50 → 159 códigos**, 0 erro emitido no corpus e
nos 209 pacotes do pub-cache (`scripts/parity-pub-cache.py`). O LSP já
rodava `types` (L01); os códigos novos chegam ao editor pela mesma regra.

## Analisador: A01–A04 (2026-09-27)

**Placar agora: 12.241/23.030 na posição exata (53,2%)**, 11.849 com a
mensagem igual; FP 1.086, FN 10.502, posição errada 287 (de 11.758, FP
1.212, medidos no mesmo oráculo em `0e4a7128`). Cláusulas de herança e a
porta do `ErrorVerifier` (`analise::clausulas`: dez códigos novos sem erro
emitido, `class_used_as_mixin` FP 91 → 0, `extends Enum` sem FP), privados
não usados (`analise::privados`: `unused_element` 88 → 165, `unused_field`
0 → 34, 0 FP) e os trechos de `dead_code` do `NullSafetyDeadCodeVerifier`
(16 → 74, posição errada 43 → 7). Dois FP publicados do limitless_ui
(V01): `// ignore` agora vale para erro (com `cannot-ignore`) e os gerados
do `build_runner` fora de `lib/` são vistos. Augmentations: escopo de
imports por unidade (`parts-with-imports`; antes o `compile-js` escolhia o
import errado em silêncio), tipos omitidos herdados da declaração
aumentada, `augment enum`. Detalhes, evidência e o que falta: docs/PENDENCIAS.md
(A01–A04) e docs/AUGMENTATIONS.md.

## Paridade do analyzer sobre a inferência comum (2026-09-26)

**Oráculo.** Regravado com o Dart 3.6.2 do Linux (`dartforge-parity
oracle`): os mesmos registros de antes, com os caminhos absolutos da raiz
trocados por `<raiz>` nas mensagens, para o oráculo não depender da
máquina. A regravação agora limpa o cache do `dart analyze` (o cache morno
omitia 7 registros) e mantém os 566 arquivos de `new-syntax.json` quando
o 3.13.4 não está disponível. Placar de partida, com o oráculo novo:
**9.293/23.030 (40,4%)**, FP 1.780, FN 13.269, posição errada 468.

**Placar agora: 11.758/23.030 na posição exata (51,1%)**, 11.366 com a
mensagem igual; FP 1.233, FN 10.949, posição errada 323. O que mais subiu
(acertos, antes → depois):

| código | antes | depois | oráculo | FP depois |
|---|---:|---:|---:|---:|
| `type_argument_not_matching_bounds` | 0 | 1.068 | 1.175 | 2 |
| `use_of_void_result` | 71 | 439 | 479 | 4 |
| `invalid_use_of_type_outside_library` | 0 | 330 | 350 | 2 |
| `subtype_of_base_or_final_is_not_base_final_or_sealed` | 0 | 329 | 329 | 0 |
| `return_of_invalid_type` | 30 | 154 | 167 | 0 |
| `unchecked_use_of_nullable_value` | 144 | 214 | 384 | 12 |
| `creation_with_non_type` | 0 | 48 | 66 | 16 |
| `invalid_null_aware_operator` | 0 | 42 | 135 | 0 |
| `non_bool_operand` | 0 | 35 | 35 | 0 |
| `undefined_operator` | 0 | 32 | 62 | 10 |

FP que caíram sem perder acertos relevantes: `non_bool_condition` 157 → 0,
`undefined_method` 146 → 9, `argument_type_not_assignable` 57 → 5,
`undefined_identifier` 185 → 112, `undefined_class` 104 → 49,
`not_assigned_potentially_non_nullable_local_variable` 40 → 8,
`unused_local_variable` 9 FP + 3 posições → 0.

**Publicação.** A regra passou de "100% no corpus" para **"nada emitido
errado"**: zero FP, zero posição errada e zero mensagem errada no corpus e
nos projetos reais; FN é permitido. O placar reprova um código listado que
emita algo errado e lista os candidatos. `crates/analysis/verified.txt`
foi de 3 para **50 códigos** (3.711 acertos publicados, antes 17). Os
projetos reais desta máquina são 60 pacotes do pub-cache (2.150 arquivos):
nenhum FP de código publicado, 21 FP no total (antes 328). Os três projetos
do proprietário não existem aqui — conferir `projetos` lá antes do corte.

**O que mudou na inferência (`crates/types`), cada um com teste:** a
interface mais específica vence o membro sobrescrito; `Null <: FutureOr<S>?`;
extensões visíveis por todo import não adiado; `return_of_invalid_type` pelo
`ReturnTypeVerifier` (funções, métodos, construtores, `async`, geradores);
verificações de `bool` do `BoolExpressionVerifier`; `e!` com contexto `K?`
e `unnecessary_non_null_assertion`; `invalid_null_aware_operator` no
operador; `use_of_void_result` onde o `checkForUseOfVoidResult` o relata;
os limites dos argumentos de tipo escritos (`limites.rs`, com
super-bounded e variância de alias); fluxo de `switch` com cases que
dividem o corpo, `??=` e padrão constante. Fora de `types`: modificadores de
classe entre bibliotecas (`analise/modificadores.rs`), padrões `int? x?` e
`a--` e `required (…)? t` no parser, locais antes de erro de sintaxe.

**Pendente:** exibir o alias de `typedef` nas mensagens (161 mensagens
diferentes de `type_argument_not_matching_bounds`); `invalid_override`; o
LSP ainda não roda `types`, então só publica os códigos verificados que não
dependem de tipos. (`extends Enum`, `dead_code`, privados não usados e
`class_used_as_mixin`: ver a seção de 2026-09-27 acima.)

## Fechamento do dia 2026-09-25
## Fechamento do dia 2026-09-25

**Estado:** um ramo só (`main`, mais `exploracao-inicial`), histórico sem
trailers de IA e com a regra no repositório. Placar do analyzer local em
`b263ab77`: **9.287/23.030 na posição exata (40,3%)**, FP 1.776, FN 13.275,
posição errada 468 (de manhã: 6.464/26.133, 24,7%, FP 4.337). Projetos
reais: 2, 119 e 183 diagnósticos nossos, 0 publicados. Suíte do workspace
verde com o ambiente completo. Pesado de `eb2c4304` verde em todos os jobs
(placar do runner igual ao local); o Pesado 36186964445 de `625ef30c`
(regra do receptor anulável e escopo das escritas) também passou em todos
os jobs (JS, produção, nativo, JIT × AOT, determinismo, análise).

**O que falta, em ordem:**

1. Portar o T1 (`wip/inferencia`, só no bundle de backup): códigos do
   analyzer emitidos por `types` em todo lugar (`aviso_com_codigo` já é o
   caminho) e o fim da `paridade/src/ponte.rs`. Os FN maiores são de tipo:
   `type_argument_not_matching_bounds` 1.175, `use_of_void_result` 383,
   `unchecked_use_of_nullable_value` 206.
2. FP restantes: `expected_token` 329, `undefined_identifier` 185,
   `undefined_method` 146 (mensagens com `'{1}'` sem o tipo, em casos que o
   analyzer relata com outro código: enum, `invocation_of_non_function`,
   `instantiate_abstract_class`), `undefined_class` 104.
3. `new_sali/frontend` sem os gerados do `build_runner` nesta máquina: rodar
   o `build_runner` lá para o placar dos projetos voltar a medir só o nosso.

## Integração no `main` e oráculo pela versão da ferramenta (2026-09-25)

**Um ramo só.** As 12 frentes que estavam em worktrees separadas foram
mescladas no `main` (analyzer, LSP, macros, JS e nativo), e as worktrees e
branches antigas foram apagadas; restam `main` e `exploracao-inicial`. O
histórico foi reescrito sem trailers de assistente de IA, e a regra agora
está no repositório (`CONTRIBUTING.md`, `AGENTS.md`, hook
`scripts/hooks/commit-msg` e o job `messages` do CI). Os conflitos de
integração foram quase todos o mesmo commit trazido por dois caminhos; o
único de desenho foi o do LSP (duas APIs de navegação entre documentos):
ficou a do servidor (`definicao_no_workspace`/`hover_no_workspace`), com a
navegação sintática e as referências (`referencias_em`) por baixo dela.
Fora da integração, por ser um porte e não um merge: o T1 de `types`
(`wip/inferencia`, códigos do analyzer em `types` e o fim da
`paridade/src/ponte.rs`), preservado só no backup em bundle.

**O placar do analyzer media o SDK errado em 566 arquivos.** Sondado hoje:
numa biblioteca 3.6, o analyzer **3.13.4** relata atalho de ponto e
construtor primário como `experiment_not_enabled` e continua analisando —
exatamente o que o nosso parser faz —, enquanto o **3.6.2**, que não
conhece esses recursos, responde com cascata (`missing_identifier`,
`expected_executable`, `missing_const_final_var_or_type`). Como a
ferramenta é 3.13 (VERSOES-LINGUAGEM.md, D2), a referência nesses arquivos
é o 3.13.4. O comando novo `dartforge-parity new-syntax` roda o 3.13.4
sobre os grupos 3.6 **sem mudar a versão do pacote**, escolhe pelo próprio
oráculo os arquivos em que ele acusa um recurso desconhecido do 3.6.2
(`dot-shorthands`, `primary-constructors`, `private-named-parameters`) e
troca só os registros deles: **498 em `analyzer` e 68 em `linguagem`**
(`corpus/diagnostics/new-syntax.json`). O `oraculo` respeita a lista ao
regravar, e a linha de cada grupo do placar diz quantos arquivos são do
3.13.4. Não foi `// @dart=3.13` por arquivo: os arquivos continuam
bibliotecas 3.6.

**Correções do parser** que o oráculo novo expôs, cada uma conferida contra
o 3.13.4 (sondas nos testes de `declarations.rs`):

* `augment` sem o experimento é **identificador comum** em qualquer posição,
  como no scanner do fasta (`abstract_scanner.dart`); antes, `augment class`
  e afins viravam modificador com `experiment_not_enabled`. `augment class
  C {}` agora dá `expected_token` e `missing_const_final_var_or_type` no
  `augment`, e `augment mixin M {}` dá `missing_identifier` no `mixin` (o
  `TopLevelDeclarationIdentifierContext` do fasta), nos dois SDKs.
* `experiment_not_enabled` no lugar do analyzer: no `(` (ou `.`) do
  cabeçalho primário, não no cabeçalho inteiro; no `.` do atalho de ponto
  (ou no `const` de `const .x(…)`), não em `.nome`.
* A versão na correção: `x.y.0` quando o erro vem do parser do fasta, `x.y`
  quando vem do `AstBuilder` do analyzer (corpo `;`, membros `this`/`new`,
  `final` declarante) — `Parser::exigir_no_ast`.
* Membros `new`/`this` sem o recurso: o 3.13.4 os lê como com o recurso
  (com os diagnósticos da parte `this`) e só acrescenta
  `experiment_not_enabled`. Os commits de 2026-09-24 que os faziam imitar o
  3.6.2 (`expected_class_member`) foram revertidos por essa regra; eram 176
  FP. `factory(` continua dependendo da versão: antes da 3.13 é um método
  chamado `factory`.

**Placar local** (8 trabalhadores, `work/placar/`):

| medição | oráculo | acertos exatos | FP | FN | posição errada |
|---|---|---|---|---|---|
| `main` antes (oráculo antigo) | 26.133 | 6.464 (24,7%) | 4.337 | 19.190 | 479 |
| só o oráculo 3.13.4 nos 566 arquivos | 23.030 | 6.735 (29,2%) | 3.066 | 14.816 | 1.479 |
| + correções do parser (`augment`, intervalos, versão) | 23.030 | 8.866 (38,5%) | 2.636 | 13.755 | 409 |
| + membros `new`/`this` sem o recurso como o 3.13.4 | 23.030 | 8.992 (39,0%) | 2.377 | 13.631 | 407 |
| + códigos oficiais dos erros de construtor primário; nomeado privado sem o recurso | 23.030 | 9.064 (39,4%) | 2.244 | 13.559 | 407 |
| + bibliotecas da VM (`dart:ffi`, `dart:mirrors`…) visíveis na análise | 23.030 | 9.074 (39,4%) | 1.917 | 13.547 | 409 |
| + receptor anulável e `void` como o analyzer; promoção em closure (T1) | 23.030 | 9.260 (40,2%) | 1.776 | 13.275 | 495 |
| + `use_of_void_result` da invocação no receptor | 23.030 | **9.287 (40,3%)** | **1.776** | **13.275** | **468** |

O denominador caiu porque o 3.13.4 não produz a cascata do 3.6.2 nesses
arquivos. Por código, no fim: `experiment_not_enabled` 1.586/1.736,
`missing_const_final_var_or_type` 513/541, `expected_token` 912/1.107,
`expected_class_member` 0 FP (eram 176).

**Códigos da 3.13.** A tabela do analyzer 6.11 não tinha os erros de
construtor primário, que saíam sem código (`dartforge_sem_codigo`, 142 FP).
O gerador (`gerar_codigos`) acrescenta agora um suplemento de 7 códigos do
3.13.4 no fim da tabela (os índices da 6.11 não mudam), transcritos do
`messages.yaml` da referência e conferidos contra o oráculo:
`non_redirecting_generative_constructor_with_primary`,
`primary_constructor_body_without_declaration`,
`multiple_primary_constructor_body_declarations`,
`primary_constructor_body_with_expression_body`,
`const_primary_constructor_with_body` (bloco e expressão) e
`primary_constructor_body_with_modifier`; o `covariant` declarante passou a
`extraneous_modifier_in_primary_constructor` e o nome repetido a
`duplicate_constructor`, que já existiam. Cada um no token do analyzer (o
`this`, o nome do construtor, o `{`/`=>`, o `async`). O nomeado privado sem
nome público, sem o recurso, deixou de ser erro: o 3.13.4 só relata o
recurso desligado (59 FP). `dartforge_sem_codigo` zerou.

**`dart:ffi` na análise.** O motor de paridade carregava o SDK pelo perfil
do DDC, que não tem `dart:ffi`, `dart:mirrors`, `dart:cli` nem
`dart:nativewrappers`; o `dart analyze` enxerga toda biblioteca pública da
plataforma, pela fonte e sem patches. Sem elas, cada `Pointer`, `Struct`,
`Array`… virava `undefined_class`. O motor agora acrescenta essas
bibliotecas das seções da VM, sem os patches: `undefined_class` foi de 335
a 104 FP. O Pesado 36180760974 (`eb2c4304`) mediu no runner exatamente o
placar local daquele commit (8.992/23.030, FP 2.377), verde em todos os
jobs.

**Receptor anulável e `void`** (`crates/types`, com o código do analyzer já
na emissão — o desenho do T1 — por `aviso_com_codigo`). Pelo
`TypePropertyResolver` do analyzer: com receptor potencialmente anulável,
se o membro não é de `Object` nem de extensão sobre o tipo anulável, o erro
é sempre `unchecked_use_of_nullable_value` (acesso, método ou operador,
conforme o nó) — nunca `undefined_*`, mesmo quando o membro também falta
no tipo não anulável. Receptor `void` dá `use_of_void_result`. Antes, a
busca tirava a anulabilidade em silêncio (FN) ou relatava `undefined_*`
(FP). Na invocação de método com receptor `void`, o analyzer relata no
receptor (`this` em `this.m()`); no acesso a propriedade, no nome.
Resultado: `unchecked_use_of_nullable_value` 144 acertos, 13 FP;
`use_of_void_result` 71 acertos, 0 FP.

**Promoção em closure.** Dois defeitos de fluxo apareceram como FP nos
projetos reais, cada um conferido no analyzer 3.6.2 e 3.13.4:

* escrita **fora** de closures só tira as promoções na entrada dela; a
  variável continua promovível dentro (porte do commit `31eddfe` do T1, a
  regra `conservativeJoin(anywhere.written, anywhere.captured)`); só a
  escrita **dentro** de alguma closure ou função local a captura. O teste
  `cadeia_clamp_e_funcao_local` fixava o contrário e foi corrigido;
* as escritas eram coletadas por **nome**: num `main` de teste com vários
  `test(…)`, `x = …` numa closure capturava um `x` homônimo de outra. A
  varredura agora tem escopos léxicos e resolve cada escrita para a
  declaração (`instrucoes::Escrita`, `local_da_escrita`).

Projetos reais com o ambiente completo (`dart pub get` no `new_sali`, que
não tinha `.dart_tool` nesta máquina): **2, 119 e 183** diagnósticos
nossos, os mesmos da base, **0 publicados** e 0 da regra nova. O
`new_sali/frontend` sobe por falta dos gerados do `build_runner` aqui
(`uri_has_not_been_generated` e a cascata). O comando `projetos` passou a
imprimir as amostras com `--details`.

**Próximos alvos, pelo placar:** (1) os FN de tipo continuam os maiores
(`type_argument_not_matching_bounds` 1.175, `use_of_void_result` 479,
`unchecked_use_of_nullable_value` 384), e dependem do porte do T1; (2)
`undefined_class` tem 335 FP e `undefined_identifier` 292.

**Outras correções do dia:** o teste nativo
`construto_nao_suportado_e_erro_com_todos_os_diagnosticos` esperava que
`#a` fosse recusado, e o nativo passou a baixá-lo; agora usa a entrada de
mapa null-aware, recusa explícita e estável. As referências passaram de
`D:/Projects/dartforge/references` (apagado) para `E:/references`, e o LLVM
de `E:/DartSDKs/llvm` para `E:/llvm` (`.cargo/config.toml`, `jit/build.rs`,
`emit_native/driver.rs`, `differential`, `scripts/env.ps1`).

**Suíte local inteira verde** (`cargo test --workspace`), com o ambiente
que o CI prepara: `scripts/env.ps1` (põe o `bin` do LLVM no `PATH` — sem
ele, os testes do JIT caem com `STATUS_DLL_NOT_FOUND` ao carregar
`LLVM-C.dll`), `scripts/generate-dart-sdk.ps1` (`runtime/ddc/dart_sdk.js`, que
os testes de `dev` e `emit_js` leem) e `dart pub get` em `corpus/ngdart`.

## Continuação no SSD (2026-09-24)

O checkout de trabalho está em `E:\MyRustProjects\dartforge`. A cópia do
repositório no D: foi feita sem as pastas ignoradas pelo `.gitignore` e sem os
worktrees antigos de `.claude`; o histórico e os arquivos rastreados foram
preservados. As toolchains ficam em `E:\DartSDKs` e `E:\Rust`, e as compilações
locais usam `TEMP`/`TMP` em E:.

Na integração `fbf7f86`, a [CI 36005425772](https://github.com/insinfo/dartforge/actions/runs/36005425772)
e o [Pesado 36005425989](https://github.com/insinfo/dartforge/actions/runs/36005425989)
passaram. O corpus com SDK da fonte ficou em **162/223** no nativo; JavaScript
de desenvolvimento e produção passou o corpus completo, e macros ficaram
**9/9**. O analyzer registrou **5.925/26.133** diagnósticos na posição exata,
5.636 mensagens iguais, 4.319 falsos positivos, 19.740 falsos negativos e
468 posições erradas, com resultado idêntico em 1/4/8 trabalhadores. Esta é a
medição combinada anterior, preservada como referência histórica.

Na integração `1a781ce`, a [CI 36011412677](https://github.com/insinfo/dartforge/actions/runs/36011412677)
e o [Pesado 36011412710](https://github.com/insinfo/dartforge/actions/runs/36011412710)
passaram. O nativo com SDK da fonte chegou a **166/223**; JS de
desenvolvimento e produção passaram, e macros ficaram **15/15**. O analyzer
atingiu **5.945/26.133** diagnósticos na posição exata, 5.656 mensagens
iguais, 4.313 falsos positivos, 19.720 falsos negativos e 468 posições
erradas, determinístico em 1/4/8 trabalhadores. Essa é a última medição
combinada concluída daquela etapa.

Na integração `3869bba`, a [CI 36013738458](https://github.com/insinfo/dartforge/actions/runs/36013738458)
e o [Pesado 36013738501](https://github.com/insinfo/dartforge/actions/runs/36013738501)
passaram. O nativo com SDK da fonte manteve **166/223**; JS de
desenvolvimento e produção passaram, e macros chegaram a **17/17**. O
analyzer atingiu **5.954/26.133** diagnósticos exatos, 5.665 mensagens
iguais, 4.310 falsos positivos, 19.711 falsos negativos e 468 posições
erradas, determinístico em 1/4/8 trabalhadores. Esse é o último placar
combinado concluído; o HEAD posterior está em
[CI 36016165004](https://github.com/insinfo/dartforge/actions/runs/36016165004) e
[Pesado 36016164949](https://github.com/insinfo/dartforge/actions/runs/36016164949).
O placar do analyzer compara o corpus legado congelado: o relatório assinala
oráculos 3.6.2 desatualizados em grupos cujas fontes mudaram. Ele serve para
detectar regressões entre commits, mas exige uma rodada controlada de
regravação antes de afirmar paridade com o SDK 3.13 atual.

Depois dessa medição, gates isolados confirmaram macros **18/18** em JS de
desenvolvimento e produção, com a augmentation do caso 421 idêntica ao CFE
([Pesado 36013877707](https://github.com/insinfo/dartforge/actions/runs/36013877707)).
O nativo com SDK da fonte chegou a **168/223** após os casos 67 e 95, sem
regressões no placar isolado; o caso 95 passou também no teste dirigido de
argumento dinâmico incorreto
([Pesado 36014381691](https://github.com/insinfo/dartforge/actions/runs/36014381691)).
No motor de build, o cliente `dfexec/1` agora responde pedidos de `BuildStep`
inválidos com erro pelo canal e mantém a ação viva para o builder tratar a
exceção. A medição combinada desses commits ainda está pendente.
O analyzer isolado alcançou **5.880/26.133** diagnósticos exatos, 5.591
mensagens iguais, 4.320 falsos positivos, 19.785 falsos negativos e 468
posições erradas, determinístico em 1/4/8 trabalhadores
([Pesado 36013697965](https://github.com/insinfo/dartforge/actions/runs/36013697965));
esse é o placar da base isolada do commit de construtores, não a soma com as
outras mudanças integradas.

No ramo de trabalho atual, o motor de build aceita um executor Dart injetado,
serve `BuildStep` com visibilidade por fase e por pacote, e possui um cliente
para o protocolo `build.*` sobre o canal `dfexec/1`. O processo que executa
builders existe pela VM Dart (`packages/build_executor`, `crates/build/src/vm.rs`,
opcional); sem ele, o padrão continua indisponível.
Gates isolados da integração recente: o nativo chegou a **166/223** no SDK da
fonte após RTI de fábrica redirecionadora e getter de interface implementado
por campo ([Pesado 36010112751](https://github.com/insinfo/dartforge/actions/runs/36010112751));
macros passaram **15/15** em JS de desenvolvimento e produção, incluindo
definição de método da fixture 418, com augmentation idêntica ao CFE
([Pesado 36010353726](https://github.com/insinfo/dartforge/actions/runs/36010353726));
o analyzer isolado alcançou **5.868/26.133** diagnósticos na posição exata,
5.579 mensagens iguais, 4.326 falsos positivos, 19.797 falsos negativos e
468 posições erradas, com determinismo em 1/4/8 trabalhadores
([Pesado 36008510883](https://github.com/insinfo/dartforge/actions/runs/36008510883)).
Esses números vieram de ramos isolados antes da integração `1a781ce`; a
medição conjunta acima prevalece para aquele commit.

Depois dela, foram integrados `Function.apply` e `NoSuchMethodError` de
aridade de closures, chamada de função devolvida por getter estático,
modelos de macros 413–415, hover/definition de import prefixado no LSP,
diagnósticos de conflito estático com mixins e o hospedeiro `BuildStep` com
limites de leitura/escrita por ação. Gates isolados: nativo com SDK da fonte
**164/223** após os casos 38 e 45, sem regressão de status; macros **12/12**
em desenvolvimento e produção, augmentations 413–415 idênticas às do CFE;
o mixin `on` levou o analyzer isolado a **5.870/26.133**, 11 acertos a mais
no grupo sem aumento de falsos positivos. A nova rodada combinada do HEAD
`60336d7` é [CI 36008167645](https://github.com/insinfo/dartforge/actions/runs/36008167645)
e [Pesado 36008167704](https://github.com/insinfo/dartforge/actions/runs/36008167704).

No ramo `ci/native-sdk-is-selector` (`9d29894`), a
[CI 36044895512](https://github.com/insinfo/dartforge/actions/runs/36044895512)
e o [Pesado 36044895476](https://github.com/insinfo/dartforge/actions/runs/36044895476)
passaram. O nativo com SDK da fonte foi de **179/223** para **183/223**,
igual no JIT (0 divergências JIT × AOT, `--gc-stress` sem regressão):
`206_enum_factory` (name/index implícitos de enum), `139_string_tostring`
e `134_object_identical` (toString padrão de genérica com argumentos) e
`53_enums_membros` (values implícito + aresta enum → `Enum` do SDK).
O grupo "chamada de membro sem implementação compilada" caiu de 6 para 3
(`57_nosuchmethod`, `216_poda_nosuchmethod`, `223_nosuchmethod_argumentos`,
os encaminhadores noSuchMethod); os maiores grupos agora são geradores
(6), `RegExp` da fonte (5) e instanciação de tipo genérico (3).

No ramo `ci/native-sdk-is-selector` (`47f843b`), a
[CI 36048526413](https://github.com/insinfo/dartforge/actions/runs/36048526413)
e o [Pesado 36048526434](https://github.com/insinfo/dartforge/actions/runs/36048526434)
passaram. O nativo com SDK da fonte manteve **183/223**, igual no JIT
(223/223 saídas idênticas JIT × AOT, 0 divergentes; `--gc-stress` sem
regressão): o encaminhador estático de método só com posicionais
(`Invocation.method` + `noSuchMethod` em linha) passa na fixture
`nsm_encaminhador_metodo_no_sdk`, mas os 3 do corpus seguem bloqueados —
`216_poda_nosuchmethod` ainda exige os encaminhadores de getter/setter, e
`57`/`223` exigem nomeados/genéricos mais o miss dinâmico do seletor.

Integração em `ci/integracao-ssd`: ajustes de caminhos, seleção do LLVM 22 no
runner, correção de símbolos estáveis e da RTI entre módulos, regressões de
hot reload, navegação e hover LSP, limpeza do executor de macros e avanço do
motor nativo de Sass. O teste local foi limitado a `cargo check` direcionado
e testes pequenos; o corpus e o link de produção rodam no GitHub Actions.

Na rodada de validação, [CI 35965411301](https://github.com/insinfo/dartforge/actions/runs/35965411301)
e [Pesado 35965411225](https://github.com/insinfo/dartforge/actions/runs/35965411225)
passaram. O executável autocontido em produção passou após a correção dos
símbolos RTI; após canonizar literais de string e corrigir a notação exponencial
sem precisão explícita e aplicar `%` de Dart a inteiros e doubles,
[Pesado 35970757894](https://github.com/insinfo/dartforge/actions/runs/35970757894)
mediu SDK da fonte: **112/223** tanto AOT quanto JIT. O corpus padrão
chegou a **92/223**, JS desenvolvimento/produção **223/223**, Dart moderno
**22/26**, macros **6/7**, com determinismo em 1/4/8 trabalhadores. P5c/P5d
ainda não está completo. Após preservar a RTI de `Set`/`Map` e corrigir
os seletores de `Type` e `_StackTrace`,
[Pesado 35971998885](https://github.com/insinfo/dartforge/actions/runs/35971998885)
mediu **119/223** tanto AOT quanto JIT; 104 casos ainda falham. A
correção de `%` fez `11_int_truncdiv_modulo_negativos` passar e permitiu que
`13b_double_tostring_divergencia_web` avançasse até os seletores de `Type`.
Após corrigir setters dinâmicos, `StackTrace` e acesso lexical a `super`,
[Pesado 35975868181](https://github.com/insinfo/dartforge/actions/runs/35975868181)
mediu **125/223** no job AOT com SDK da fonte (98 casos ainda falham); o
workflow geral foi cancelado pelo push seguinte após esse artefato sair. A rodada
ainda não continha a correção do getter que sobrescreve campo herdado nem
as correções seguintes de `Object` em enums e `_Type`.
Com o despacho de getters herdados, `Object`/`_Type`/enums e a RTI estrutural
de records, o job AOT com SDK da fonte do
[Pesado 35978288847](https://github.com/insinfo/dartforge/actions/runs/35978288847)
mediu **146/223** (77 falhas). `46_classes_getters_setters`,
`52_enums_basico`, `61_list_metodos`, `100_records_basico`, `112_typedef`,
`129_comparable_sort` e `198_antigo_records19` passaram nessa rodada.

O analisador integrado passou em [CI 35970218894](https://github.com/insinfo/dartforge/actions/runs/35970218894)
e [Pesado 35970218770](https://github.com/insinfo/dartforge/actions/runs/35970218770):
**5.381/26.133 (20,6%)** diagnósticos na posição exata, ante 1.445/26.133
antes do merge, com relatório idêntico em 1/4/8 trabalhadores. Após
preservar os aliases e getters do SDK e adaptar `NodeList` nativo aos
métodos de `List` do DDC, [JS de produção 35972736058](https://github.com/insinfo/dartforge/actions/runs/35972736058)
passou **223/223** e a galeria passou **26/26** no navegador
([E2E 35971652262](https://github.com/insinfo/dartforge/actions/runs/35971652262)).

Os diagnósticos de corpo e inicializador `external`, campos `abstract` e
aridade de operadores elevaram a paridade para **5.565/26.133 (21,3%)**
na [rodada Pesado 35973737505](https://github.com/insinfo/dartforge/actions/runs/35973737505),
após [CI 35973737483](https://github.com/insinfo/dartforge/actions/runs/35973737483)
verde: 5.276 mensagens iguais, 4.369 falsos positivos, 20.050 falsos
negativos e 518 posições erradas, com determinismo em 1/4/8 trabalhadores.
No corpus, `external_method_with_body` ficou 31/31,
`external_with_initializer` 27/27, `abstract_field_initializer` 6/6 e
`abstract_static_field` 8/8; todos sem falsos positivos. A aridade de
operadores ficou 137/141, também sem falsos positivos; quatro casos com
construtor primário ainda dependem de recuperação do parser.

A [rodada Pesado 35975036788](https://github.com/insinfo/dartforge/actions/runs/35975036788)
do diagnóstico de enum sem constantes, após
[CI 35975036834](https://github.com/insinfo/dartforge/actions/runs/35975036834)
verde, mediu **5.567/26.133 (21,3%)**: 5.278 mensagens iguais, 4.369
falsos positivos, 20.048 falsos negativos e 518 posições erradas, com
determinismo em 1/4/8 trabalhadores. `enum_without_constants` passou de
0/126 para 2/126, sem falsos positivos. O corpus acusa, em parte dos casos
restantes, enum vazio onde o texto atual contém constantes explícitas (por
exemplo `enum E(int x) { v(0); ... }`); o relatório marca os grupos 3.6.2
como oráculo desatualizado. Regravar e auditar esse oráculo é necessário
antes de usar os 124 casos restantes como defeitos da implementação.

A integração com os diagnósticos de construtores `const`, a exceção para
factories `external const` do SDK e a correção do intervalo de atribuição a
local `final` passou em [CI 35978288844](https://github.com/insinfo/dartforge/actions/runs/35978288844)
e [Pesado 35978288847](https://github.com/insinfo/dartforge/actions/runs/35978288847):
**5.656/26.133 (21,6%)** na posição exata, 5.367 mensagens iguais, 4.371
falsos positivos, 20.008 falsos negativos e 469 posições erradas, com
determinismo em 1/4/8 trabalhadores. `const_factory` ficou 15/15 sem
falsos positivos; `const_constructor_with_body`, 25/25 com dois falsos
positivos. `assignment_to_final_local` ficou 49/60 exatos, quatro com
posição errada, 38 falsos positivos e sete falsos negativos. O ganho de
49 posições exatas desse código decorre da correção do intervalo; os 40
falsos negativos eliminados incluem os 25 e 15 casos dos dois códigos
`const`. A classificação de atribuições a `const` e `final` de topo e
getters sem setter ainda não fazia parte dessa rodada.

As regras adicionais de atribuição (`assignment_to_const`, getter sem
setter e receptor tipado em atribuição composta) passaram na
[CI 35980099471](https://github.com/insinfo/dartforge/actions/runs/35980099471)
e no [Pesado 35980099480](https://github.com/insinfo/dartforge/actions/runs/35980099480):
**5.700/26.133 (21,8%)** na posição exata, 5.411 mensagens iguais,
4.378 falsos positivos, 19.964 falsos negativos, 469 posições erradas,
determinismo em 1/4/8. São 44 acertos exatos e 44 mensagens iguais a mais
que a rodada anterior. O mesmo Pesado mediu o nativo em **146/223** e
JIT × AOT em **223/223 saídas idênticas**; o caso 104 ainda falha nos dois
perfis antes da correção de passagem de variáveis de cases compartilhados.

Na [CI nativa 35981507325](https://github.com/insinfo/dartforge/actions/runs/35981507325)
e no [Pesado 35981507387](https://github.com/insinfo/dartforge/actions/runs/35981507387),
atribuições compostas, listas e literais de `Type` elevaram o SDK da fonte a
**151/223**. A [suíte nativa 35982482442](https://github.com/insinfo/dartforge/actions/runs/35982482442)
confirmou `210_constantes_de_ambiente` e **152/223**. A rodada combinada
[35982589082](https://github.com/insinfo/dartforge/actions/runs/35982589082)
mediu **157/223**: o seletor genérico liberou seis casos, inclusive
`104_sealed_exaustivo`, mas `69_list_de_lists_e_matriz` regrediu no
`fold<int>` após `expand`; a rodada seguinte corrigiu essa regressão.
O RTI da lista concreta e das fábricas redirecionadas foi validado na
[CI 35986485976](https://github.com/insinfo/dartforge/actions/runs/35986485976)
e na [suíte nativa 35986493960](https://github.com/insinfo/dartforge/actions/runs/35986493960):
**160/223**, sem regressões contra a rodada de 157. `69_list_de_lists_e_matriz`
voltou a passar, junto com `138_collection_hashmap_ordenado` e
`199_antigo_reified18`; `41_classes_ctor_nomeado` e `64_map_ordem_insercao`
permaneceram verdes após reificar tear-offs de construtor e `MapEntry<K,V>`.
Na integração `06a7516`, a [CI 35987446180](https://github.com/insinfo/dartforge/actions/runs/35987446180)
e o [Pesado 35987446121](https://github.com/insinfo/dartforge/actions/runs/35987446121)
passaram. O nativo com SDK da fonte manteve **160/223**; JavaScript
desenvolvimento e produção ficaram em **223/223**, e macros em **6/7**.
O job JIT × AOT teve **223/223 saídas idênticas**, mas apenas **93/223**
programas passaram no corpus padrão; 122 não produziram IR, portanto a
igualdade entre perfis não implica compatibilidade com a VM. Os ajustes de
RTI em cópias de lista, URI Unicode no LSP e execução de macros pelo
`build_runner` foram integrados depois dessa rodada e aguardam o próximo
Pesado combinado.

Os quatro diagnósticos adicionais de membros somente para leitura passaram
na [CI 35981633280](https://github.com/insinfo/dartforge/actions/runs/35981633280)
e no [Pesado 35981633118](https://github.com/insinfo/dartforge/actions/runs/35981633118):
**5.768/26.133 (22,1%)** exatos, 5.479 mensagens iguais, 4.354 falsos
positivos, 19.896 falsos negativos, 469 posições erradas e determinismo em
1/4/8 trabalhadores. Ante 5.700: +68 acertos e -24 falsos positivos.
As regras de inicialização de campos `final` e atribuição a método passaram
na [CI 35983725221](https://github.com/insinfo/dartforge/actions/runs/35983725221);
o [Pesado 35983725139](https://github.com/insinfo/dartforge/actions/runs/35983725139)
mediu **5.793/26.133 (22,2%)** exatos, 5.504 mensagens iguais, 4.352 falsos
positivos, 19.871 falsos negativos, 469 posições erradas e determinismo em
1/4/8 trabalhadores. São +25 acertos exatos e -2 falsos positivos. O próprio
relatório marca grupos do oráculo como desatualizados; esse placar é uma medida
de paridade com a versão gravada, não um certificado de correção total.
Na mesma [rodada combinada 35987446121](https://github.com/insinfo/dartforge/actions/runs/35987446121),
após os diagnósticos de atribuição e construtores, o analyzer alcançou
**5.827/26.133 (22,3%)** na posição exata, 5.538 mensagens iguais,
4.342 falsos positivos, 19.837 falsos negativos e 469 posições erradas,
com relatório idêntico em 1/4/8 trabalhadores. As correções posteriores
de `Enum.index` e setter de extensão explícita ainda não fazem parte
desse placar combinado.
Na integração `7d5e370`, a [CI 35989394377](https://github.com/insinfo/dartforge/actions/runs/35989394377)
e o [Pesado 35989394369](https://github.com/insinfo/dartforge/actions/runs/35989394369)
passaram: **5.832/26.133** diagnósticos exatos, 5.543 mensagens iguais,
4.338 falsos positivos, 19.832 falsos negativos e 469 posições erradas,
com determinismo em 1/4/8 trabalhadores. O nativo com SDK da fonte manteve
**160/223**, sem mudança de estado em nenhum caso frente à rodada anterior.
O builder executou as quatro aplicações do fixture e emitiu o resultado
estruturado da fase de declarações; a augmentation final ainda não existe.
No ramo isolado `ci/native-late`, a [CI 35990678510](https://github.com/insinfo/dartforge/actions/runs/35990678510)
passou e o [Pesado dirigido 35990678930](https://github.com/insinfo/dartforge/actions/runs/35990678930)
publicou **161/223** no SDK da fonte, sem regressões frente aos 160 casos
verdes anteriores. `56_late` passou após inicialização preguiçosa de locais,
globais e campos; os casos 34 e 94 permaneceram verdes. Capturas `late`
continuam no caminho anterior, com limitação documentada em `docs/NATIVO.md`.
Na integração `06eac49`, a [CI 35991837079](https://github.com/insinfo/dartforge/actions/runs/35991837079)
e o [Pesado 35991837078](https://github.com/insinfo/dartforge/actions/runs/35991837078)
passaram. O SDK da fonte confirmou **161/223** casos nativos, com `56_late`
recuperado e sem regressões frente à rodada anterior. O analyzer mediu
**5.835/26.133** diagnósticos na posição exata, 5.546 mensagens iguais,
4.334 falsos positivos, 19.829 falsos negativos e 469 posições erradas;
o relatório foi idêntico com 1, 4 e 8 trabalhadores. Esse placar inclui
`Enum.index` e o setter de extensão explícita, mas ainda não inclui os
commits posteriores de campos não nulos, métodos estáticos em extensões e
definições completas de macros.
Na integração `25ea825`, a [CI 35995133386](https://github.com/insinfo/dartforge/actions/runs/35995133386)
e o [Pesado 35995133542](https://github.com/insinfo/dartforge/actions/runs/35995133542)
passaram. O analyzer alcançou **5.867/26.133** diagnósticos exatos
(22,5%), 5.578 mensagens iguais, 4.321 falsos positivos, 19.798 falsos
negativos e 468 posições erradas, com determinismo em 1/4/8 trabalhadores.
São +32 exatos e -13 falsos positivos frente a `06eac49`. O nativo com
SDK da fonte manteve **161/223**, sem mudança de status em nenhum dos 223
programas, e JS desenvolvimento/produção passaram. O corpus de macros
permaneceu **6/7**: `410_json_codable` ainda estava em `PENDENTES` nessa
revisão, apesar de o builder já materializar a augmentation do fixture.
Na integração `301d1e5`, a [CI 35997168053](https://github.com/insinfo/dartforge/actions/runs/35997168053)
e o [Pesado 35997168127](https://github.com/insinfo/dartforge/actions/runs/35997168127)
passaram. O SDK da fonte manteve **161/223**, com os mesmos 223 estados da
rodada anterior; a paridade do analyzer manteve **5.867/26.133** e o
determinismo em 1/4/8 trabalhadores. Macros subiram para **7/7** em JS
desenvolvimento e produção: `410_json_codable` usa a entrada Forge preparada
com a augmentation materializada pelo builder. Isso valida o consumo do
arquivo gerado, mas ainda não comprova execução automática da anotação no
fluxo normal do compilador; o caso independente `411_pedido_independente`
foi acrescentado depois dessa integração para testar esse caminho.
Na integração `b72b4fb`, a [CI 35999991755](https://github.com/insinfo/dartforge/actions/runs/35999991755)
e o [Pesado 35999991786](https://github.com/insinfo/dartforge/actions/runs/35999991786)
passaram. O caso independente `411_pedido_independente` elevou macros a
**8/8** em JS desenvolvimento e produção: a anotação original executa pelo
executor provisório na VM e a augmentation fica em memória. O SDK da fonte
manteve **161/223**, JS desenvolvimento/produção **223/223**, e o analyzer
alcançou **5.870/26.133** diagnósticos exatos, 5.581 mensagens iguais,
4.319 falsos positivos, 19.795 falsos negativos e 468 posições erradas,
com determinismo em 1/4/8 trabalhadores. A CLI ganhou recarga R1 opcional
com preservação de heap e estáticos; a execução de `main` ainda se repete.
O executor nativo de macros, a recarga de SDK da fonte integrada e a
compatibilidade completa continuam pendentes.

A rodada `ci/analyzer-enum-invocacao` ([CI 36041568829](https://github.com/insinfo/dartforge/actions/runs/36041568829)
e [Pesado 36041568846](https://github.com/insinfo/dartforge/actions/runs/36041568846),
os dois verdes) levou `invalid_reference_to_generative_enum_constructor` a
**16/20** acertos no corpus, **0 FP** e 4 FN, todos alvos de redirecionamento
(`= E`, `= E.nomeado`, passo seguinte): cobertos `E()`, `new E()` e `const E()`
no intervalo do oráculo, com factories e `: this(...)` travados como negativos
em teste dirigido. A rodada intermediária
([Pesado 36038318228](https://github.com/insinfo/dartforge/actions/runs/36038318228))
acusou 3 FP em `const E.foo()` sem alvo (outro código), corrigidos antes do
verde. Placar geral: **5.959/26.133 (22,8%)** na posição exata, 5.670 mensagens
iguais, 4.393 falsos positivos, 19.706 falsos negativos, 468 posições erradas,
determinismo em 1/4/8. O código segue fora de `verified.txt` (falta 100%).

## Fechamento do dia 2026-09-23

Resumo de uma página. O detalhe de cada frente está nas seções 1 e 2.
Trabalho não pronto para o `main` fica em ramos `wip/*`: cada um tem o último
commit do dia, com uma mensagem detalhada do que foi feito e do que falta.

### O que entrou no `main` hoje (CI e Pesado verdes antes de cada merge)

* **Macros** (8cf79cd):
  * augmentations nas duas formas (3.6.2 experimental e 3.13.4);
  * `macro class`, `import augment` e `augment library` no parser;
  * cadeia de augmentation no outline, generalizando o `@patch` do SDK;
  * a API de macros reescrita por nós em Dart puro (`packages/macros`);
  * o hospedeiro em Rust (`crates/macros_host`) sobre o protocolo `dfexec/1`, o mesmo dos builders;
  * a augmentation do `@JsonCodable` sai idêntica à do CFE 3.6.2;
  * a materialização gera `.dart` comum para a toolchain oficial;
  * job `macros` no Pesado.
* **Inferência de tipos reescrita pela especificação** (67d5fa4):
  * `new_sali/core`: 15 avisos e 31 divergências em 634 mil expressões;
  * `frontend`: 28 avisos (17 legítimos) e 56 divergências em 1,26 milhão;
  * SDK: 18 diagnósticos, todos legítimos;
  * corpus da especificação: 83/95.
* **Paridade do analisador A1–A2** (6109201): saída no formato do `dart analyze`, tabela dos 1.030 códigos do analyzer 6.11, placar sobre 9.441 arquivos, regra de publicação.
* **Dart moderno P0–P5** (0acdf61): versões de linguagem 3.7–3.13 por biblioteca; dois SDKs de oráculo; `corpus/modern` 22/26.
* **Nativo com async, RTI e `super` em mixin**: corpus nativo **91/223**, e o JIT também 91/223, sem divergência.
* **Portão de custo zero**: razões pareadas, confirmação em segunda passada, média com duas casas decimais e tolerância de 10% (decisão do proprietário).
* **`crates/runtime/README.md`**: o que falta no coletor de lixo.
  * weak refs e `Expando`, finalizers, heap por isolate;
  * geracional e compactação;
  * custo das raízes e pausas.
* **Ambiente**:
  * `scripts/ci.ps1 -Scoreboard` baixa em `target/ci-scoreboard` e apaga depois;
  * `.gitignore` com `**/target/`;
  * `%TEMP%` limpo no fim do dia;
  * relatório local de disco em `RELATORIO-DISCO-C.md` (fora do git).

### Trabalho em andamento (ramos `wip/*`, não integrados)

| ramo | frente | estado no fim do dia | primeiro passo amanhã |
| --- | --- | --- | --- |
| `wip/paridade` (integrada por `ci/paridade-integracao`) | analisador em Rust (A1–A2 → L1) | acerto exato **5.343/26.133 (20,4%)**; `unused_local_variable` 2.231/2.315, `duplicate_definition` 783/1.317, `unused_import` 112/153, `expected_token` 421/1.724. CI 35967061037 e Pesado 35967060973 verdes; determinismo 1/4/8 | corrigir os falsos positivos e negativos restantes; recuperação sintática e checagens de tipo |
| `wip/inferencia` (de9b68b) | inferência (lacunas restantes) | b485cdf verde nos dois workflows (CI 35941571461, Pesado 35941571357). Avisos no `new_sali`: core **5**, frontend **19**, todos também dados pelo analyzer (17 em templates gerados, 6 `dead_code` já com `ignore`, 1 cast desnecessário por promoção de campo); divergências core **22**, frontend **45**; corpus **89/95**; sonda 6/7 (falta `unused_local_variable`) | `git merge main`; lacuna L03 (inferência horizontal em fases de dependência, gen14); depois L05, L20, L21, L28; `unused_local_variable` (sonda 7/7) e P6 |
| `wip/nativo-sdk-fonte` (70ec1e4) | nativo P5c/P5d (SDK compilado da fonte) | merge do `main` concluído (13 conflitos: async/RTI, Dart moderno, macros), compila; testes de emit_native, runtime e elements passam local. Último verde: Pesado 35930483005 — padrão 82/223, SDK da fonte 84/223 (108/223 local antes do merge). Rodada atual vermelha: o link de produção autocontido pega o lld do LLVM 20 do runner, que não lê bitcode do LLVM 22 | ligar a produção com o `lld-link` explícito do LLVM do `DARTFORGE_CLANG` (`-fuse-ld=` com caminho), push em `ci/nativo-d2`, conferir padrão 91/223 e o primeiro placar do SDK da fonte pós-merge |

### O que falta, por frente (ordem de prioridade)

1. **Nativo**: 132 dos 223 do corpus ainda falham.
   * Terminar P5c/P5d, o SDK compilado da fonte: DLL em cache no desenvolvimento, executável único estático com ThinLTO em produção.
   * Depois: extension types, `sync*`/`async*` restantes e isolates.
2. **Inferência**: integrar o `wip/inferencia` (divergências 22/45, corpus 89/95; no `main` ainda 31/56 e 83/95) e fechar o resto.
3. **Analisador e LSP em Rust**: subir de 20,4% para a paridade.
   * Primeiro a sintaxe (recuperação de erro igual à do parser oficial), depois os códigos de tipo.
   * Só publicar um código com 100% no corpus e zero falso positivo nos projetos reais.
   * Mensagens em inglês idênticas às do SDK.
4. **Macros**: executor nativo auto-hospedado. A macro compilada pelo nosso backend nativo roda dentro do `dfexec/1`, sem Node e sem VM de terceiros.
   * Fases 1–3 completas.
   * Cache por hash de entrada, para custo zero em quem não usa.
5. **Motor de build (substituto do `build_runner`)**: ngdart com 186 gerados e 0 diferentes. Falta o resto dos builders do `new_sali` e a invalidação fina.
6. **JIT**: acompanha o nativo (mesmo IR). Hot reload R1+ segue `docs/` (ORCv2, sessão persistente).
7. **Produção JS**: segue a pesquisa de otimização (`docs/PESQUISA-OTIMIZACAO.md`).
8. **Coletor de lixo**: os 9 itens de `crates/runtime/README.md`.
9. **Dart moderno**: `corpus/modern` 26/26 (JS04 fechou 347 e 350–352); `PENDENTES` vazio.

### Ambiente e máquina

* O D: (HD mecânico USB) é o gargalo. O cache de escrita foi ligado e **só vale depois de reiniciar o PC**; agora é seguro reiniciar.
* O Defender já tem exclusões para o D: e para as toolchains.
* Hardware recomendado: +8 GB de DDR4 no segundo slot e SSD SATA de 2,5" (ou NVMe de 1 TB).
* Regras em vigor:
  * no máximo 4 agentes;
  * testes pesados só no CI;
  * merge só com os dois workflows verdes;
  * temporários no D:.

---

## Migração para SSD — 2026-09-24

O trabalho continua em `E:/MyRustProjects/dartforge`, no ramo
`ssd/nativo-sdk-fonte` criado a partir do WIP nativo `70ec1e4`. A cópia com
`robocopy /MT:16` levou o código e o histórico Git, sem `target*`, `dist`,
`references`, worktrees e demais artefatos ignorados; quatro arquivos
`package_config.json` versionados foram restaurados depois da cópia.
`E:/DartSDKs` contém o LLVM 22.1.8 e os SDKs Dart 3.6.2, 3.13.4 e 3.14 dev;
`E:/Rust` contém o Rust 1.98.1 e o cache Cargo. `scripts/env.ps1` seleciona
essas cópias por padrão, respeitando as variáveis de ambiente explícitas.

Verificações no SSD: `cargo check --locked --offline --workspace` passou;
`cargo test --locked --offline -p dartforge-emit-native --lib` passou (24
testes, cinco medições ignoradas); `cargo test --locked --offline -p
dartforge-jit --lib` passou (11 testes, três ignorados), e o teste isolado
`reload::tests::ciclo_completo_com_ir_direto` passou com `LLVM-C.dll` no
`PATH`. O teste diferencial nativo isolado `01_print` passou contra a VM
(1/1), exercendo Clang, link e execução em `E:`. O placar do corpus nativo
com SDK da fonte ainda precisa ser medido no CI, conforme a regra de testes
pesados.
O fechamento geral mais recente permanece em `main:ESTADO.md`; abaixo está
o histórico detalhado preservado deste ramo.

O que **funciona hoje, verificado por execução**, e o que **falta**, nesta
ordem. Tudo aqui é medido; nada é estimativa salvo onde está escrito
"estimado". Os números são do `main` deste commit, na máquina do
proprietário (8 núcleos, 8 GB).

Alvos reais usados como critério:
* `C:/MyDartProjects/new_sali` — ngdart 8.0.0-dev.4 (frontend) + angel3
  (backend) + core, 1.258 arquivos, 8,5 MiB de Dart;
* `references/limitless_ui` — biblioteca de componentes ngdart do
  proprietário, com `example/` (24 componentes) e suíte e2e em puppeteer.

---

## 1. O que funciona

### 1.1 Front-end (análise) — `crates/frontend`, `elements`, `types`

| Fase | Estado | Evidência |
| --- | --- | --- |
| Léxico + sintaxe de Dart 3.6 | **completo** | 426/426 arquivos do `lib/` do SDK 3.6.2, 1.969/1.969 do corpus pub (26 pacotes), 1.258/1.258 do `new_sali` — `cargo test -p dartforge-frontend --test corpus -- --ignored` |
| Modelo de elementos, imports/exports, `part`, patches do SDK | **completo** | 36 bibliotecas do SDK carregadas com os patches do DDC fundidos, 269/269 supertipos resolvidos — `crates/elements/tests/sdk.rs` |
| Tipos: representação, hierarquia, subtipagem | **completo** | 83/83 casos normativos de `subtyping.md`; 25.179 anotações do SDK em 10.396 `TypeId` (hash-consing) |
| Inferência de corpos, fluxo, constantes | **funcional, com lacunas** (motor reescrito pela especificação, `crates/types/src/inferencia`) | 40/40 negativos do `analyzer`; medido contra o oráculo `package:analyzer` (`tools/type_oracle`): `new_sali/core` 15 avisos e 31 de 634.368 expressões divergentes, `frontend` 28 avisos (17 deles em templates gerados, que o analyzer também acusa) e 56 de 1.259.011; SDK da fonte (nativo) 18 diagnósticos, os mesmos do analyzer; corpus de conformidade 83/95 programas iguais ao oráculo (ver §2.1) |
| Versão de linguagem por biblioteca e recursos 3.7–3.13 | **3.7–3.13 de sintaxe completos**; inferência e fluxo 3.7–3.13 (P6) pendentes | ver §1.1.1 |

### 1.1.1 Dart 3.7–3.13 — `docs/VERSOES-LINGUAGEM.md`

3.6.2 é o **piso**, não o teto. Cada biblioteca tem a sua versão de
linguagem (marcador `// @dart = x.y` > `languageVersion` do pacote > a
corrente, **3.13**), resolvida uma vez no carregamento e consultada como bits
(`LibraryFeatures`); `dart:*` fica no piso (D1). O harness tem **dois SDKs de
oráculo** (3.6.2 e 3.13.4, cada um com o seu `dartdevc` e o seu
`dart_sdk.js`); os programas dizem a versão que exigem (`// requer-dart:`).

| recurso | versão | `corpus/modern` (VM e DDC 3.13.4; dev e produção) |
| --- | --- | --- |
| curingas `_` | 3.7 | 300–303 passam (1 negativo; `// @dart=3.6` volta a ligar `_`) |
| elementos null-aware | 3.8 | 310–311 passam (1 negativo); chave nula não avalia o valor |
| nomeados privados `{this._x}` | 3.12 | 320–323 passam (3 negativos) |
| atalhos de ponto | 3.10 | 330–332 passam (2 negativos) |
| construtores primários, `new`/`factory`, corpo `;`, `var`/`final` | 3.13 | 340–349 passam (4 negativos); 347 com os membros de extension type (JS04) |
| inferência por bounds, fluxo sólido, gerador | 3.7–3.10 | 350–352 passam (1 negativo; JS04) |

Placar no CI (Pesado 35904470774 e CI 35904470762, `ci/moderno` em 5a68e2d, os dois verdes): **22/26** em desenvolvimento
e em produção, **26/26** DDC×VM, 12 negativos recusados na mesma linha que o
CFE; os 4 que faltavam (347, 350–352) passaram na rodada JS04 (2026-09-27, local: 26/26 em
desenvolvimento e produção, `PENDENTES` vazio). Na rodada do CI:
`corpus/js` 223/223 em desenvolvimento e produção, determinismo idêntico
(produção e IR do nativo), nativo e JIT 82/223 (os do `main`) e o portão
**custo zero verde** — nada regrediu. O `corpus/js` compilado na 3.6 dá **JS idêntico
byte a byte** ao da base (222/222), e o parser continua aceitando 426/426 do
SDK e 1.969/1.969 do pub (cada pacote na versão do seu pubspec). Nenhum
recurso precisou de runtime novo. O nativo não roda o `corpus/modern`:
curinga liga nome, entrada de mapa null-aware é recusada e atalho de ponto só sai quando é
construção. Macros e augmentations: contratos em `docs/MACROS-PROTOCOLO.md`
e `docs/AUGMENTATIONS.md` (executor nativo auto-hospedado), sem código.

Custo para projeto 3.6 (regra governante): o portão `custo zero (tempo)` do
Pesado passou (corpus JS 9.318 → 9.208 ms, edição de corpo 32 → 32 ms); no A/B
local contra o `main` 6583c2b, mínimo de 3 por programa nos 223 do
`corpus/js`, 11.783 ms × 11.806 ms (+0,2%). Números e método em
`docs/VERSOES-LINGUAGEM.md` §7.

### 1.1.2 Augmentations e macros — `docs/AUGMENTATIONS.md`, `docs/MACROS-PROTOCOLO.md`

* **Augmentations** (P7): `augment` em classe, mixin, membros e funções de
  topo; bibliotecas de augmentation da forma 3.6 (`import augment` +
  `augment library`, experimento `macros`) e *parts* com imports da forma
  atual (`augmentations,enhanced-parts`); a fusão no outline generaliza o
  `@patch` do SDK (`elements/src/augmentation.rs`) e o `emit_js` emite a
  classe com os membros da cadeia. `corpus/macros/400–405`: **6/6** em
  desenvolvimento e produção, 6/6 DDC×VM (3.6.2 e 3.13.4), CI verde (Pesado
  35918855910, CI 35918855886). Divergências medidas dos dois CFEs em
  AUGMENTATIONS.md §4.
* **API de macros reescrita** (`packages/macros`, pacote `macros`, Dart puro):
  a superfície do `package:macros` 0.1.3-main.0 e o lado do executor
  (modelo, introspecção, builders com o texto do CFE, serviço `macro.*`). O
  `json.dart` do `package:json` 0.20.4, sem mudança, analisa contra ela com
  zero problemas no analyzer 3.6.2.
* **Hospedeiro** (`crates/macros_host`): detecção com custo zero, ordem do
  CFE, as três fases com recarga, modelo e consultas, montagem byte a byte,
  o serviço `macro.*` do `dfexec/1`, `Indisponivel` no produto.
  `dartforge macros --materialize` grava a augmentation
  (docs/MACROS-COMPATIBILIDADE.md). Placar em MACROS-PROTOCOLO.md §8.
* **Espera o executor nativo**: executar macros no `compile-js` (hoje: erro
  claro na anotação; `410_json_codable` em `corpus/macros/PENDENTES`).
* CI da rodada (`ci/macros` em ab2ad4a, os dois verdes): Pesado 35931208832
  (`macros` 6/7 dev e produção + 1 pendente, 7/7 DDC×VM; `corpus/js`
  223/223; `moderno` 22/26 como no `main`; custo zero verde) e CI 35931208819
  (inclusive `vm_executa_a_macro_e_bate_com_o_cfe` e
  `sessao_gravada_reproduz_o_texto_do_cfe` nos ignorados).

### 1.2 Emissão JavaScript — `crates/emit_js`

Emite **módulos ES6 no contrato do DDC** e liga contra o `dart:*` oficial
(`runtime/ddc/dart_sdk.js`, gerado de `ddc_platform.dill` por
`scripts/generate-dart-sdk.ps1`). O `dartdevc` é o oráculo do contrato.

* **Corpus diferencial: 213/213.** Cada programa é executado em `dart run
  --enable-asserts`, em `dartdevc`+Node e no DartForge+Node; stdout e
  código de saída comparados **byte a byte**. `cargo run -p
  dartforge-differential`.
* **`new_sali/core`: 7 de 14 testes reais** (`package:test`) rodam no Node
  com a mesma saída da VM. Os outros 7 dependem de `dart:io` (leitura de
  arquivo, fontes de PDF) — impossível no navegador por definição; é alvo
  do backend nativo.
* **`limitless_ui/example` (biblioteca de componentes ngdart do
  proprietário, 24 componentes, 483 módulos): a suíte e2e em puppeteer
  passa — **26/26**, o mesmo que a saída oficial do `dart2js`
  (`build_web_compilers --release`), em 4m11s contra 3m33s. Uma sonda que
  percorre as **53 rotas** da galeria recolhendo `onerror`/
  `unhandledrejection`/`console.error` dá **52/53** nos dois lados (a rota
  restante não tem o seletor que a sonda espera, e reprova igual no
  oficial). Foi essa sonda, não a suíte, que encontrou o último defeito
  corrigido (receita rti de tipo genérico cru: `raw|Caixa<@>`, nunca
  `raw|Caixa`). `scripts/limitless-ui.ps1`, `docs/LIMITLESS-UI.md`.
* **`new_sali/frontend` (ngdart + `dart:html` + `package:js`): a aplicação
  roda no navegador.** 616 módulos; `node --check` 616/616; no Edge
  headless os **11 passos do fluxo** (carga, carrossel, erro do IdP,
  submeter login, rotas públicas, guarda de rota, callback OIDC, sessão
  forjada) ficaram **idênticos à saída oficial do `build_web_compilers`**,
  inclusive as 7 chamadas HTTP que a aplicação faz. `scripts/serve.ps1
  -Fluxo` (dirige o Edge por CDP, separa erro da aplicação de erro de
  ambiente).

Construtos cobertos: classes (construtores de todo tipo, `const`
canonicalizado, estáticos, operadores, mixins, enums com membros,
genéricas, `noSuchMethod`, `late`), coleções e literais tipados com rti,
exceções, `async`/`await`/`async*`/`sync*`/`await for`, `dynamic` por
`dsend`, records e padrões, extensions, `typedef`, bibliotecas múltiplas
com prefixos/`show`/`hide`/`part`, interop (`package:js`,
`dart:js_interop`, `@JSName`, `@anonymous`), `dart:html`.

### 1.2.1 Perfil de **produção** — `crates/emit_js_production`

`dartforge-jsprod <entrada.dart> -o <saida.js>` escreve **um arquivo**, com
o runtime embutido e podado pelo mundo fechado. O plano, a referência
estudada e o que ele ainda não faz estão em `docs/JS-PRODUCAO.md`.

* **Corpus diferencial: 214/214**, com o **mesmo stdout da VM**, byte a
  byte — o mesmo placar do perfil de desenvolvimento.
  `cargo run --release -p dartforge-differential -- --production` compara os
  três (VM × nosso desenvolvimento × nossa produção) e agrupa as falhas em
  duas listas, porque as causas são diferentes: falha do desenvolvimento é
  construto que falta no emissor, falha da produção é poda ou montagem.
* **Poda do `dart_sdk.js`**: a varredura parte os 7.087.858 B em 14.735
  declarações de topo **sem perder um byte**, sub-divide as três que
  sozinhas referenciam o programa inteiro (as 549 constantes do `CT`, os
  315 KB de regras de subtipagem do `addRules`, o `copyProperties`), extrai
  as referências — inclusive os nomes de classe dentro das **receitas
  rti**, que nenhum analisador de JS enxerga — e roda o ponto fixo.
  No `01_print`: **6.922 KB → 1.474 KB**, 8.450 de 44.965 unidades vivas.
* **Empacotamento**: uma IIFE por módulo, em ordem topológica, com os
  `var L$…` içados. **Bibliotecas nunca são fundidas** — dois arquivos
  vendorizados byte a byte iguais continuam com estado global separado e
  tipos de identidade distinta (`docs/PESQUISA-OTIMIZACAO.md` §3).
* **Determinismo** por construção, com teste de unidade (duas montagens das
  mesmas entradas dão o mesmo arquivo byte a byte) e pelo modo
  `dartforge-differential determinism --production --workers 1,4,8`,
  que o backend nativo trouxe e que o perfil de produção passa.

Contra o oficial (`dart compile js -O4`, SDK 3.6.2), amostra do corpus por
`pwsh scripts/measure-js-production.ps1`:

| programa | dart2js | jsprod | dart2js | jsprod |
| --- | ---: | ---: | ---: | ---: |
| `01_print` | 34,1 KB | 1.527 KB | 2,58 s | **0,88 s** |
| `40_classes_basico` | 34,4 KB | 1.538 KB | 2,32 s | **0,60 s** |
| `60_list_basico` | 45,1 KB | 1.533 KB | 3,98 s | **0,76 s** |
| `80_async_await_basico` | 48,3 KB | 1.534 KB | 3,32 s | **0,68 s** |
| `120_convert_json` | 57,2 KB | 1.728 KB | 1,81 s | **0,41 s** |

**Somos 3 a 5× mais rápidos e 30 a 45× maiores**, e o tamanho tem piso fixo
de ~1,5 MB qualquer que seja o programa: é o que resta do `dart_sdk.js` do
DDC depois da poda. Ver §2.3 — fechar esse buraco é compilar o SDK pela
nossa trilha, não otimizar mais.

**Mundo fechado sobre a nossa trilha** (etapa 5, `crates/world`,
`docs/JS-PRODUCAO.md` §1.7). O código do usuário e dos pacotes é podado
**antes** da emissão por um RTA sobre o modelo de elementos: seletor por
nome, três níveis de classe e a fronteira com o `dart_sdk.js` por regra.
Um verificador sempre ligado confere o texto emitido e fecha o ponto fixo
mundo+texto. O modo stub (`--verify-stub`) denuncia na execução qualquer
chamada a código podado. Sem filtro, o emissor é byte a byte o de antes
(teste `identidade.rs` e o corpus inteiro contra o binário de `main`).

**Projeto real**: `new_sali/core`, `test/arvore_processo_item_test.dart`
(`package:test`, 351 módulos): **32.003 KB → 4.463 KB** e compilação de
4,3 s → 1,4 s, com a **mesma saída do `dart run`**. Os 7 testes do core
que rodam na web ficam entre 3,0 e 4,8 MB, todos iguais à VM e sem stub
executado. O custo do verificador é de 28-36 ms. O `limitless_ui/example`
só cai 6,5% (48,2 → 45,1 MB): a galeria alcança quase tudo, e seletor só por
nome é o limite (`docs/JS-PRODUCAO.md` §6.0).

**SDK compilado pela nossa trilha** (padrão desde 2026-10-01;
`docs/JS-PRODUCAO-SDK-PROPRIO.md`). O arquivo deixou de conter o
`dart_sdk.js`. As bibliotecas `dart:` saem do nosso emissor em modo SDK
(intrínsecos `JS()`, classes nativas, `dart:_runtime`), e o mundo fechado
passa a cobrir também o SDK. Por cima vêm a emissão enxuta e a troca de
nomes do `oxc`. `--sdk-ddc` volta ao caminho antigo.

Validação:

* corpus 238/238 iguais à VM, com o SDK próprio e com `--sdk-ddc`;
* `limitless_ui` 26/26 no e2e;
* `new_sali/frontend` com 11 passos sem erro no `flow.mjs`.

Tamanhos em bytes, com o mundo incremental, o renomeio de propriedades,
as receitas curtas, os `assert` desligados e a compressão do `oxc`
(`docs/JS-PRODUCAO-TAMANHO.md`):

| programa | antes | agora | `dart2js -O4` |
| --- | ---: | ---: | ---: |
| `01_print` | 1.370 KB | **156 KB** | 35 KB |
| `limitless_ui` | 45,1 MB | **6,94 MB** | 4,5 MB |
| `new_sali` | 24,9 MB | **12,36 MB** | 7,6 MB |

Tempo de compilação: o `new_sali` leva 35–45 s, contra 57 s do `dart2js`
(o mundo fechado caiu de 707 s para cerca de 1 s), e o `limitless_ui`
leva 13 s.

Pendência: o tamanho, que ainda está 1,6× acima do `dart2js` no `new_sali`. O que falta é mudança de contrato do emissor
(`docs/JS-PRODUCAO-TAMANHO.md` §7).

### 1.3 Latência e memória — `crates/dev`

`dartforge dev` mantém a sessão viva e recompila o mínimo.

| Cenário (`new_sali/core`, 2.302 unidades, 352 módulos) | Tempo |
| --- | --- |
| compilação completa (`compile-js`, 2ª vez) | **1,72 s** |
| **edição de corpo** (sessão viva) | **227 ms** — 1 unidade reanalisada, 1 módulo escrito |
| edição de API pública | 288 ms — 82 bibliotecas reemitidas, 3 módulos |

`new_sali/frontend` (3.183 unidades, 616 módulos): **6,5 s a frio**
(cache do SDK construído, saída vazia), 4,3 s morno, edição de corpo
**335 ms**. Contra a toolchain oficial no mesmo projeto e máquina:
`build_runner build` **2 min 59 s** e `dart2js` de produção **~4 min**
(esta última não é comparação de igual para igual — ver PLANO.md,
"Latência medida contra a toolchain oficial").

**Memória — a propriedade que o LSP do Dart não tem**: teste de platô com
20 edições sucessivas, `live_bytes` **+0,00 MB** (core 204,63 MB, pico
278 MB; frontend 438,15 MB, pico 568 MB). No mesmo projeto o `webdev`
chega a ~10 GB e o LSP do Dart a ~6 GB, crescendo a cada edição.

Trajetória medida no mesmo arquivo do `core` ao longo do dia: 12,4 s →
4,6 → 2,87 → 1,72 s (completa); 1,70 s → 227 ms (incremental).

### 1.4 LSP — `crates/lsp` + `editors/vscode`

Transporte JSON-RPC por stdio escrito aqui, `didChange` **incremental**
com conversão UTF-16 correta, `$/cancelRequest`, `publishDiagnostics` pelo
parser novo, `DocumentStore` como dono por documento. Extensão VS Code
(cliente TypeScript fino). Medição no `new_sali`, mesma sequência de 1.462
mensagens: **DartForge pico 21,3 MiB / platô 19,0 MiB; `dart
language-server` pico 640,5 MiB / platô 633,7 MiB**.

**Diagnósticos com paridade — `crates/diagnostics`, `crates/frontend`,
`crates/analysis`, `crates/parity`, `dartforge analyze`, LSP.** O
`Diagnostic` tem código, severidade e argumentos; a mensagem é o molde
oficial em inglês renderizado como o `formatList` do analyzer (**nada de
português na saída**). A tabela (`diagnostics/src/codigos_g.rs`) é gerada do
`analyzer-6.11.0` da cache do pub: 1.030 códigos — 542
`CompileTimeErrorCode`, 7 `StaticWarningCode`, 144 `WarningCode`, 8
`HintCode`, 48 `FfiCode`, 265 `ParserErrorCode`, 12 `ScannerErrorCode`, 4
`TodoCode` —, mais 7 códigos de construtor primário do 3.13.4 no fim
(`SUPLEMENTO_3_13` do gerador, 2026-09-25): 1.037 no total.
* **Sintaxe**: o lexer e o parser saem com o código, a mensagem e a posição
  do fasta, medidos em sondas: `;` que falta no token anterior, o resto no
  token corrente; fecho que falta no fim do arquivo é o `expected_token` do
  scanner com comprimento 1; `missing_identifier` ou
  `expected_identifier_but_got_keyword`; os recursos desligados pela versão
  são `experiment_not_enabled`.
* **Verificadores sem tipo** (`crates/analysis`): o
  `DuplicateDefinitionVerifier` e o `MemberDuplicateDefinitionVerifier`
  inteiros, a parte local do `UnusedLocalElementsVerifier` e o
  `ImportsVerifier` (`unused_import` e `unused_shown_name`, pelo lado seguro:
  112/153 no corpus, 0 FP nos projetos).
* **Regra de publicação** (`crates/analysis/verified.txt`, a mesma no CLI
  e no LSP): sintaxe sempre; semântica só com 100% no corpus e 0 FP nos
  três projetos reais. Publicados hoje: `enum_constant_same_name_as_enclosing`,
  `enum_with_name_values` e `values_declaration_in_enum`.
* **LSP (L1)**: cada documento passa pela sintaxe e pelos verificadores sem
  tipo, filtrados pela regra. O diagnóstico leva `code`, a severidade do
  analyzer e a mensagem com a correção.
  * Medido no `new_sali`, 1.258 arquivos abertos e editados
    (`memoria_lsp`): 20,03 MiB vivos (antes, 19,94), pico 20,51 MiB,
    780–810 ms, 1,90 M alocações (antes, 1,37 M).
* **Aceite de A2**: dados código, argumentos e intervalo, a sonda de 7 erros
  sai **byte a byte** igual ao JSON gravado do SDK 3.6.2
  (`crates/parity/tests/sonda.rs`).
  * Pela análise de hoje, 4 dos 7 batem: `non_bool_condition`,
    `not_assigned_potentially_non_nullable_local_variable`,
    `argument_type_not_assignable` e `unused_local_variable`.
  * Os outros 3 são do T1 (`types`): `undefined_function`,
    `return_of_invalid_type` (posição) e `unchecked_use_of_nullable_value`.
* **Placar no corpus** (`corpus/diagnostics`, oráculo gravado em disco):
  9.441 arquivos e 26.133 diagnósticos do oráculo, 376 s com 2
  trabalhadores.
  * **5.343 na posição exata (20,4%), 5.054 com mensagem igual**, medidos
    pelo Pesado 35967060973. Antes da integração eram 1.445 (5,5%).
    Posição errada 518; FP 4.369; FN 20.272.
  * `unused_local_variable` 2.231/2.315 (FP 9);
    `duplicate_definition` 783/1.317 (quase todo o resto é sintaxe de
    augmentation e construtor primário, que o 3.6.2 analisa de outro jeito);
    `expected_token` 421/1.724; `values_declaration_in_enum` 13/13;
    `conflicting_static_and_instance` 39/266 (os 227 que faltam dependem da
    interface herdada).
  * FN maiores: `expected_executable` 1.236 e
    `missing_const_final_var_or_type` 912 (recuperação do fasta),
    `type_argument_not_matching_bounds` 1.175 e `use_of_void_result` 479
    (T1).
  * 2 pânicos isolados (`part/self_test.dart` esgota a memória; um caso de
    `recursive_interface_inheritance` não termina). Cada lote roda num
    processo filho com teto de memória e de tempo.
* **Projetos reais**: o oráculo 3.6.2 dá **0** no `new_sali/core`, no
  `new_sali/frontend` e no `limitless_ui`. O nosso lado tem, internamente,
  **0, 15 e 183**, todos de tipo ou de `uri_has_not_been_generated`.
  Nenhum dos verificadores sem tipo tem FP nos três. **Publicados: 0.**
* **Mutações** dos três projetos: 45 mutantes (`nome`, `import`, `tipo`,
  `!` e `await`), com o oráculo regravado sobre cada um. Oráculo 190;
  **acertos 75**; FP 49; FN 106.
* **CI**: job `analise` do `heavy.yml`, contra o oráculo gravado (o runner
  não roda `dart analyze`), com o relatório idêntico em 1, 4 e 8
  trabalhadores (`determinismo`).
### 1.5 Backend nativo — `crates/emit_native` (feature `native`)

Trilha nova → HIR própria → LLVM IR → Clang → executável, com o runtime
Rust (GC por tracing). `dartforge compile-native` (compile com
`cargo build -p dartforge-cli --features native`); `dartforge aot` é o
apelido de produção do mesmo caminho.

**Rodada 2, P6 + RTI: corpus nativo 91/223 (Pesado 35904857766, CI
35904857783), JIT 91/223 sem divergência, os 91 também sob `--gc-stress`
(job novo do `heavy.yml`, que reprova se um programa só falha com
estresse).** `async`/`await` como máquina de estados com o quadro no heap,
sobre o `dart:async` **compilado da fonte** (com `dart:_internal` e a
`Duration`; só para quem usa `dart:async`), laço de eventos no runtime
(microtarefas antes de timers, timers na ordem da VM); tipos em tempo de
execução no desenho do dart2js (receitas, universo canônico, regras de
supertipo; `is`/`as`/padrões genéricos, o cast inteiro, `Type`); `super`
dentro de mixin. Um defeito do carregador corrigido: a parte de um arquivo
de patch do SDK era carregada como parte comum. Detalhes em
`docs/NATIVO-PLANO.md` §7.7–§7.8. Antes disto:

**Rodada 2, P1–P4 (α): corpus nativo 81/223 (Pesado 35871381320), JIT
81/223 com zero divergências JIT × AOT; os 81 passam também sob
`--gc-stress`.** Closures com captura em célula e a convenção uniforme de
chamada de valor função; símbolos estáveis pelo caminho da declaração
(`df.<biblioteca>.<dono>.<membro>`, teste T-ID); despacho por nome para
receptor sem tipo e operadores sobre `num`/`dynamic`; `switch` (comando e
expressão), padrões, enums, records com campo nomeado e `const` canônico;
cascata, `super`, mixins pela linearização, extensões. Detalhes e decisões
em `docs/NATIVO-PLANO.md` §7.4–§7.5. Antes disto:

**Corpus nativo: 50/222 (CI, run 35823269758), antes 7/214; sob
`--gc-stress`, os mesmos 50/222 (run 35823275126).** O backend passou a ter um
**contrato de representação e raízes** (`docs/NATIVO-PLANO.md` §6 — R, E,
N, G), implementado em quatro passos medidos no CI: 7/214 → 26/214 (N:
nada de `0` como substituto; construtor, atribuição e membros de verdade)
→ 48/214 (R: representação pelo tipo, caixas, locais tipados) → 48/222
(E + G: arestas pela representação, verificador da HIR, raízes do código
gerado — com o corpus crescido para 222 no `main`) → 50/222 (o
`--gc-stress` achou `isEven` testando a paridade do handle da caixa). Dos 71 `panic` de
*handle* do começo, **nenhum sobrou**; também sumiram os `RefCell already
borrowed`, os tempos esgotados e os estouros do teto de heap do corpus. O
que falha hoje falha como **erro de compilação com diagnóstico** ("não
suportado no backend nativo: <construto>"), não como escalar usado como
handle. Dois achados que mudaram o desenho: o backend rodava **sem
`dart:core`** (a seção `vm` do `libraries.json` usa `include` e o
`SdkLayout` não o seguia — todo `int` era `dynamic`), e a inferência não
tinha braço para `for-in`. O harness diferencial roda o corpus pelo
backend nativo comparando com a VM byte a byte:

```powershell
cargo build --release -p dartforge-differential
target\release\dartforge-differential.exe --native
```

Limites ligados por padrão, porque sem eles o corpus nativo em paralelo
tomou a memória da máquina: heap do runtime com teto de 256 MiB
(`DARTFORGE_HEAP_MAX_MB`), `--jobs 2` no modo nativo, `--exec-limit` de
5 s para executar o binário, e 4 MiB de saída capturada por processo.

O número saiu de 3/214 nesta sessão; o que mudou de fato foi a natureza do
gargalo — as falhas de "o Clang recusa o módulo" caíram de 172 para 13, e
o que sobra é programa que roda e imprime outra coisa. Ver
`docs/NATIVO.md` e `docs/NATIVO-PLANO.md`.

**Rodada 2, δ (NATIVO-PLANO §7.4):** strings com semântica UTF-16 na forma
da VM (`_OneByteString`/`_TwoByteString`) e `Ref` com `Smi` etiquetado
(R10) entraram sem regressão — nativo 50/223, JIT 50/223 com 0 divergências,
os mesmos 50 sob `--gc-stress` (Pesado 35849542217 e 35852784596). P5a: a
sobreposição `native_sdk/` carrega; os corpos das sete bibliotecas da fonte
têm 4 447 diagnósticos de inferência (medidos com um pedido a
`crates/types`). P5b: tabela dos 137 natives, 48 já no runtime. P5c/P5d
esperam P1–P4 (o lowering de closures, despacho e RTI para compilar o SDK).

O **lowering de exceções** existe: `throw`, `try`/`catch`/`finally`,
`rethrow`, com `finally` como sub-rotina e discriminador de razão (normal,
`return`, exceção, `break`, `continue`). O mecanismo escolhido é
**exceção pendente com verificação depois da chamada**, e não landing
pads: SEH/funclets no MSVC seria um segundo lowering, o runtime em Rust
não desenrola através de `extern "C"` — e é de lá que vem a maior parte
das exceções reais —, e exceção que atravessa `await` não pode depender da
pilha nativa. A justificativa e o custo no caminho feliz estão em
`docs/NATIVO-PLANO.md`.

**Determinismo verificado no corpus inteiro**: `dartforge-differential
determinism --native [--workers 1,4,8]` emite o LLVM IR de cada
programa, sem Clang, ligação nem execução, e exige o mesmo resumo (FNV-1a
de 128 bits) — ou o mesmo erro — programa a programa. Passa com 1, 4 e 8
trabalhadores nos 214 programas (184 com IR, 30 com erro de carga), em
**0,2–0,5 s por passada**. `--run` mantém o caminho antigo (compilar,
executar e comparar o relatório; o IR só com `DARTFORGE_KEEP_IR=1`). Ver §3.2.

**Cache de objeto**: o `.obj` de cada programa fica em
`native_cache/obj/` pela chave do IR + `clang --version` + bandeiras
(`crates/emit_native/src/cache_objeto.rs`); o mesmo IR dá o mesmo objeto
byte a byte. `compile-native --emit-ir` grava só o IR, `--digest` imprime o
resumo.

### 1.5.1 JIT — `dartforge run` e `dartforge reload` (R0)

O `crates/jit` (ORCv2) executa o **mesmo LLVM IR** que o AOT entrega ao
Clang (`emitir_ir`). Compile com `cargo build -p dartforge-cli --features jit`;
o executável passa a exigir a `LLVM-C.dll` da distribuição completa no
`PATH` (`docs/JIT.md`).

* `dartforge run <entrada.dart> [--timings]` emite o IR e o executa neste
  processo, sem trampolim: chamadas diretas, como no AOT.
* `dartforge reload <entrada.dart>` é **hot reload com estado** por padrão:
  a geração nova é publicada no programa vivo, no ponto seguro do laço de
  eventos, sem rodar o `main` de novo — heap, estáticos, timers e portas
  continuam (`crates/cli/src/jit.rs:237-256`; teste
  `crates/cli/tests/reload_state.rs`, no `ci.yml`). Edição que a recarga não
  sabe aplicar é recusada com o motivo e o programa reinicia; edição que não
  compila mantém a geração em execução. `--restart` é o R0 (reinício a
  quente sem estado, um processo por geração).
  *Histórico (até 2026-09-27):* aqui se lia que `reload` era só o R0 e que
  «o estado NÃO é preservado».
* `dartforge-differential --jit` compara o JIT com a VM (mesmo placar do
  `--native`); `--jit-aot` passa o MESMO IR também pelo AOT, lista todo
  `JIT≠AOT` (é defeito) e mede os tempos. No CI é o job `jit` do `heavy.yml`.

O runtime é **fonte única**: o mesmo `runtime_main.rs` vira a `.lib` do AOT e
o módulo `dartforge_runtime::abi` do JIT, com a tabela de símbolos gerada
(`crates/runtime/build.rs`). No corpus inteiro, JIT e AOT deram **222/222
idênticos** a partir do mesmo IR. O JIT até executar leva 43 ms por programa, e
o Clang + ligação 165 ms (medianas, Pesado 35827208951, placar 50/222 nos dois; `docs/JIT.md`). A
biblioteca `crates/jit` já serve de executor persistente: `compile_module`
para o cache, `add_compiled_module`, e várias execuções com estado limpo.

### 1.5.2 Runtime Dart — `crates/runtime`

O runtime em Rust que o executável nativo carrega: heap com **GC por
tracing preciso** (cada campo marca se é referência), `String` em UTF-16
para casar com a indexação do SDK, `int` de 64 bits com estouro modular
como na VM, e a fronteira de símbolos com o IR emitido.

Ganhou nesta sessão um **teto duro de heap** (256 MiB por padrão,
`DARTFORGE_HEAP_MAX_MB` ajusta, `0` desliga), separado do gatilho de
coleta e contando também a tabela de slots. Ao estourar, sai com uma linha
legível e código 255, sem `panic` atravessando `extern "C"`. Não é
detalhe de teste: sem ele, um programa do corpus em laço ia a 1,9 GB e
travava a máquina.

Com o contrato de raízes (G, `docs/NATIVO-PLANO.md` §6.5) o heap **coleta
em qualquer alocação** — o portão "só com frame aberto" saiu, porque o
código gerado agora abre um quadro de raízes por função e enraíza cada
valor `Ref` (SSA e locais). A exceção pendente, o rastro corrente e os
globais `Ref` são raízes; as tabelas laterais por handle (coleções
imutáveis, iterações ativas) moram no `Heap` e são purgadas a cada coleta;
toda extern que aloca mais de uma vez enraíza os temporários. `int`,
`double` e `bool` numa posição de referência são **caixas**
(`BoxedInt`/`BoxedDouble`/dois singletons de `bool`) que as coleções
normalizam para o escalar na entrada, com `==`/`identical` por valor como
na VM. `Heap::get` distingue as quatro falhas de handle (null, negativo,
além da tabela, já coletado) e `DARTFORGE_GC_OFF=1` desliga a coleta para
diagnóstico. `DARTFORGE_GC_STRESS=1` (e `--gc-stress` no harness) coleta
antes de toda alocação.

### 1.6 Infraestrutura

* `crates/differential` — harness paralelo com cache dos oráculos;
  `corpus/js/` com 213 programas verificados na VM.
* `docs/CONTRATO-DDC.md` — Dart e JS do `dartdevc` lado a lado para os 212.
* Determinismo (`docs/PESQUISA-OTIMIZACAO.md` §11): `dartforge-differential
  determinism [--native] [--workers 1,4,8]` exige relatório idêntico
  com qualquer número de trabalhadores; no nativo, o mesmo LLVM IR de cada
  programa — se a ordem de conclusão mudar o IR, o cache de objeto por hash
  erra e o Clang roda à toa. Verificado idêntico com 1, 4 e 8; no nativo,
  no corpus inteiro e sem precisar de `DARTFORGE_KEEP_IR` (§3.2).
* `scripts/` — `generate-dart-sdk.ps1`, `serve.ps1`/`flow.mjs` (Edge por
  CDP), `limitless-ui.ps1` (`-Preparar`/`-Montar`/`-Servir`/`-E2e`),
  `measure-lsp.ps1`.
* Cache do outline do SDK em `target/dartforge/sdk-<hash>.bin` (5 ms para
  ler, contra ~105 ms de reanálise).
* **CI no GitHub Actions** (§3.0): `ci.yml` (rápido, todo push) e
  `heavy.yml` (corpus JS, produção, nativo em fragmentos, determinismo).
  Fora do `ci.yml`, declarado no cabeçalho dele:
  * **`cargo fmt --check` e `clippy -D warnings`** — o workspace nunca foi
    formatado inteiro nem está limpo no clippy (o primeiro crate,
    `dartforge-elements`, já para com 3 erros). Voltam num commit **só de
    formatação/lint**, depois que as branches em andamento entrarem;
    fazê-lo antes conflita com todas elas.
  * **Ubuntu**: a trilha nova falha no Linux em `dartforge-dev` (`hashes`,
    `plato`), `dartforge-emit-js` (`basico`) e `dartforge-elements`
    (`sdk_cache`), e passa no Windows (registro de 2026-09-23 no cabeçalho do
    `ci.yml`, não reconferido). O `cargo test --workspace` continua só no
    Windows, mas o job `native-unix` do `ci.yml` (Ubuntu e macOS) compila a
    CLI com `--features native,jit` e roda os testes do `emit_native`, do
    `jit`, a recarga pela CLI, as regressões de `dart:io` e a distribuição
    num ambiente limpo (`.github/workflows/ci.yml:162-250`).
  * Sem exclusões de crate: a trilha velha, o `cranelift-jit` e o driver
    `native` saíram do workspace (§4), e com eles os 16 testes que falhavam
    e a falha registrada de `real_executable_prints_ints_and_bools`. Os
    quatro exemplos de memória têm nomes distintos (`memoria_<crate>`), e o
    CI roda `cargo test --workspace` com todos os alvos e os doctests.

### 1.7 `dartforge serve` e o gerador do ngdart

* `dartforge serve <entrada> -o <dir> [--web <dir>] [--port N]` — servidor
  HTTP próprio, cliente de recarga injetado na resposta do `index.html` (o
  arquivo no disco não é tocado) e canal WebSocket (RFC 6455) como o
  auto-refresh do webdev. Protocolo versionado em JSON (`ola`,
  `recarregar`, `erro`), separado do transporte. Verificado no
  `new_sali/frontend` com Edge headless por CDP
  (`scripts/verify-reload.mjs`): editar um componente recarrega o
  navegador em **1,5 s**, contra **1m20s** do `build_runner` para a mesma
  edição.
* Fontes geradas em memória (`crates/elements/src/gerado.rs`): os
  `.template.dart` do ngdart podem vir de uma tabela indexada pelo caminho
  natural, sem passar pelo disco. Geração imutável e trocada inteira — o
  navegador nunca vê meio estado. `DARTFORGE_GENERATED=build_runner` prova o
  encanamento: 284 templates lidos da memória dão 616 módulos byte a byte
  iguais aos da compilação que lê do disco.
* `crates/ng_generator` — o compilador do ngdart em Rust, com dois oráculos:
  o corpus próprio (`corpus/ngdart/`, uma forma por arquivo, com o
  `.template.dart` oficial ao lado) e os 945 arquivos que o `build_runner`
  gerou no `new_sali/frontend`
  (`cargo run -p dartforge-ng-generator --example oraculo -- <projeto>`).

  **new_sali/frontend: 186 arquivos gerados por nós (212 iguais byte a
  byte contando os `.css.shim.dart`), 0 diferentes, 114 pendentes. Corpus:
  99 de 106 oráculos conferidos, os 7 restantes recusados de propósito.**
  (Medido em 2026-09-23, rodada 3; antes dela: 162 gerados, corpus 94/100;
  antes da rodada 2: 145, corpus 58/66.)

  Cobre hoje:
  - biblioteca sem Angular, `@Directive` e `@Pipe` (o arquivo trivial);
  - template estático: elementos, texto, atributos, `<ng-content>` (com e
    sem `select`);
  - interpolação tipada como o `_TypeResolver` (chamada com o retorno do
    método, ternário/`!x`/`x!`/`??`/índice/binário `dynamic`), atributo
    interpolado (`interpolateString1`, `interpolate2`), locais de visão
    ancestral pela cadeia de `parentView`;
  - imutabilidade do `isImmutable` oficial (getter explícito, `a.b`, `!x` e
    chamada são mutáveis) e a guarda de diretivas: nó que casa um seletor
    de diretiva conhecida e não emitida é recusado;
  - ligações `[x]`, `[class.x]`, `[attr.x]`, `[style.x]` (constantes no
    `if (firstCheck)` compartilhado) e eventos gerais: tear-off, handler
    complexo (`_handleEvent_N`), atribuição, `$event`, evento próprio pelo
    `eventManager`;
  - pipes puros `$pipe.nome(..)` (instância na visão do componente,
    `pureProxyN` por chamada, também nas embutidas);
  - `@HostListener` em componente (forma simples), `@HostBinding('class.x')`
    em diretiva (o `XNgCd`), `#ref` em elemento HTML com `@ViewChild`
    estático, `providers: []` e `pipes:` sem uso;
  - componente filho completo: `@Input` (literal, constante, dinâmica,
    ordem de declaração), `@Output` (`subscription_N`), ciclo de vida e
    `onPush` do filho, injeção no construtor dele (nó, `ChangeDetectorRef`,
    serviços pela cadeia de injetores), `#ref` no filho com `@ViewChild`
    (`queryChangeDetectorRefs`), filho com `*`, projeção por seletor e
    consulta de conteúdo sem resultado;
  - diretivas de atributo por **metadados lidos do programa**
    (`metadados.rs`, o `_ComponentVisitor` do ngcompiler sobre o banco
    semântico: seletor, `providers` avaliados como constante, `@Input`/
    `@Output`/`@HostListener` herdados, ganchos, dependências com
    `@Inject`/`@Optional`/`@Self`/`@Host`) — sem catálogo escrito à mão:
    `ngforms` (`NgForm`, `NgModel`, acessores de valor, `select`/`option`,
    validadores) e as diretivas do próprio projeto, com os provedores na
    ordem das dependências, dependência de elemento acima (`@Host`, também
    de visão ancestral), `ngOnDestroy`, ouvintes do mesmo evento agrupados e
    o `injectorGetInternal`;
  - componente de seletor composto, diretiva em tag que não é HTML
    (`addShimE`), conteúdo que nenhuma projeção recebe e `@ContentChildren`
    do filho preenchido (o page header do new_sali);
  - injeção no construtor, ciclo de vida (os sete ganchos), `onPush`;
  - folhas de estilo: **Sass** (o subconjunto que os projetos usam) e o
    shim `_ngcontent-%ID%` na forma do `csslib` compacto, gerando também o
    `.css.shim.dart`.

  Duas coisas sustentam o "0 diferentes": o gerador **recusa** toda forma
  que não sabe traduzir (e o placar conta por motivo, para saber o que
  atacar), e o conversor de expressões reusa o parser Dart da trilha nova
  em vez de aproximar texto.

  `DARTFORGE_GENERATED=ng` compila com ele, fazendo antes uma carga de
  resolução (o equivalente ao `BuildStep.resolver`, lendo o nosso banco
  semântico); o que falta continua vindo do `build_runner`.

### 1.8 Motor de geração de código — `crates/build` (substituto do `build_runner`)

Contrato: `docs/BUILD-MOTOR.md`; protocolo do executor Dart:
`docs/BUILD-PROTOCOLO.md`; plano e fases: `docs/BUILD-RUST.md` §0. O motor
lê a configuração que o `build_runner` 2.4.15 leria, monta **o mesmo plano
de fases**, e para cada ação decide pela impressão digital do que ela
consultou se executa ou reaproveita; publica numa `Geracao` em memória.
Executores, nesta ordem: **nativo** (Rust), **Dart** (`dfexec/1`; pela VM
Dart quando pedido com `dartforge build --dart <exe>` ou
`DARTFORGE_BUILD_DART`, indisponível sem isso), **apoio** (o que o `build_runner` deixou no disco, com aviso
quando está mais velho que a fonte; erro com `dartforge build --strict`).

* **Plano igual ao oficial**: a forma canônica de `dartforge build --plan`
  é igual à do `.dart_tool/build/entrypoint/build.dart` (lido, nunca
  gerado) nos 9 casos do `corpus/builders` com builders, no
  `new_sali/frontend` e no `limitless_ui/example` (20 aplicações cada).
* **`corpus/builders`** (10 casos, oráculo do `build_runner` oficial,
  `scripts/corpus-builders.ps1`): todas as saídas dos manifestos são
  previstas pelo grafo, menos uma escrita por pós-processador; placar
  **0 iguais / 61 pendentes / 0 diferentes** (sem executor Dart nem apoio
  no corpus limpo); determinismo 1/4/8 idêntico; incremental = do zero nas
  12 edições (`crates/build/tests/corpus.rs`, `#[ignore]`, no `ci.yml` depois
  de `pub get`). **Pela VM** (`crates/build/tests/executor_vm.rs`, os
  builders do ecossistema executados de verdade, sem apoio no disco):
  **56 iguais / 1 pendente / 0 diferentes** — json_serializable, built_value,
  freezed, drift, mockito, riverpod_generator, sass_builder e os builders
  locais; o pendente é a saída do pós-processador — e incremental = do zero
  nas 12 edições. json_serializable: limpo 11,8 s (kernel e resumo do SDK
  compilados) ou 1,8 s (em cache), edições 67–106 ms na sessão viva, contra
  20,0 s / 4,2 s / ~4,4 s do `build_runner` (docs/BUILD-RUST.md, B7).
* **ngdart pelo motor** (estágio A: uma ação de pacote, `gerar_com_apoio`
  sobre o `Program` da sessão, pela API pública do `ng_generator`):
  `crates/build/tests/ng_transparencia.rs` — 60 saídas pelo motor iguais às
  do gerador chamado direto, 58 conferidas com o oráculo de `corpus/ngdart`,
  **0 diferentes**; incremental = do zero com edição de template e arquivo
  novo. No `new_sali/frontend`, `dartforge build --compare` (placar contra o
  apoio do `build_runner`): **151 iguais / 1.410 pendentes / 0 diferentes**
  (145 `.template.dart` e 6 `.css.shim.dart` nativos), e as contagens de
  saídas esperadas iguais às do `.dart_tool/build/generated` (773
  `.template.dart`, 192 `.css`, 192 `.css.map`, 202 `.css.shim.dart`, 202
  `.css.dart`). Na medição inicial, Sass nativo era **não verificado** e só
  7 de 181 `.css` sairiam iguais. Depois, o subconjunto `compressed` sem
  `sourceMaps` foi habilitado para publicação: 114 CSS do `new_sali` haviam
  sido conferidos byte a byte, e a [CI 36004370714](https://github.com/insinfo/dartforge/actions/runs/36004370714)
  confirmou o pedido sob demanda sem apoio no disco. `expanded`, mapas de
  fonte e sintaxe Sass fora do subconjunto continuam no apoio.
* **`compile-js` do `new_sali/frontend` com o motor** (padrão quando há
  `build_runner`; o `emit_js` não depende mais do `ng_generator`): 575 módulos,
  **574 byte a byte iguais** aos do `DARTFORGE_GENERATED=ng` de antes (579
  módulos); o do `limitless_ui` difere só por absorver os 4 `.css.dart` que o
  fluxo antigo lia do `.dart_tool/build/generated` (caminho fora do pub
  cache, então módulo próprio) e o motor publica no caminho natural, como os
  demais gerados do pacote.
* **Latência de edição no `new_sali/frontend`** (`scripts/measure-generation.ps1`,
  sessão viva, 3 repetições aplicadas e revertidas, apoio = o
  `.dart_tool/build/generated` que já existe, máquina carregada):

  | edição | motor | nativo (estágio A) | recarga das geradas | ações | saídas alteradas | unidades / módulos |
  |---|---|---|---|---|---|---|
  | texto no `.html` de um componente coberto | 0,68–0,91 s | 0,65–0,88 s | 0,28–0,37 s | 454 | 1 | 1 / 1 |
  | propriedade no `.scss` do componente | 0,66–0,72 s | 0,64–0,69 s | 0,28–0,32 s | 454 | 1 (`.css.shim.dart`) | 1 / 1 |
  | corpo no `.dart` do componente, fora do template | 0,67–0,91 s | 0,64–0,87 s | — | 454 | **0** (corte pela saída) | 1 / 1 |
  | corpo em `.dart` sem Angular | **< 1 ms** | — | — | **0** | 0 | 1 / 1 |

  O estágio A reexecuta o pacote inteiro (`gerar_com_apoio` ≈ 0,52 s +
  consultas ≈ 0,14 s) a cada edição de arquivo Angular: **acima dos 500 ms**
  da meta; o estágio B (ação por componente) depende do
  `docs/BUILD-PEDIDOS-GERADOR-NG.md`. O que o usuário espera ainda é
  dominado pela escrita do módulo de 41 MB em que o ciclo de imports do app
  funde as bibliotecas (3–13 s nesta máquina, com ou sem motor); sem o motor,
  a mesma sessão leva 0,42–0,47 s fora a escrita numa edição de corpo.
  `@Input` novo num filho não foi medido (só faz sentido no estágio B).
  Um primeiro corte do estágio B passou na [CI 36004370714](https://github.com/insinfo/dartforge/actions/runs/36004370714):
  HTML e CSS direto conhecidos regeneram somente os componentes que os
  consultaram; o teste incremental compara a sessão viva com uma geração do
  zero após editar ambos. Edição de `.dart` do pacote (B03): regenera o
  arquivo e o fecho de quem o importa/exporta (no `corpus/ngdart`, 209 das
  219 edições sem regenerar o pacote; incremental = do zero nas 219;
  `docs/BUILD-PEDIDOS-GERADOR-NG.md`, "Estágio B no motor"). Falta medir a
  latência no `new_sali` e os recursos SCSS encadeados.
* **Custo zero** (regra governante, PLANO.md): portão estrutural
  `crates/dev/tests/zero_cost.rs` verde — num projeto sem `build_runner`,
  nenhum motor construído (`instancias() == 0`), relatório sem motor e a
  **mesma contagem de alocações** por edição que uma sessão construída sem
  etapas; portão de tempo no `heavy.yml` (job `custo-zero`, commit × main,
  5 rodadas alternadas, tolerância 3%), verde nas duas rodadas:
  35847625760 — corpus JS inteiro **11,36 × 11,56 s** (razão 1,017), edição
  de corpo numa sessão sintética de 300 bibliotecas **34 × 35 ms** (1,029);
  35850781174 (depois do merge do `main`) — **11,53 × 11,62 s** (1,008) e
  **35 × 34 ms** (0,971). O ruído entre rodadas do mesmo binário chegou a 14%
  (11,1–12,7 s): a mediana de 5 cabe nos 3%, mas por pouco; se o portão
  oscilar, a tolerância sobe com a medição registrada aqui. Local no
  `new_sali/core` (`scripts/measure-zero-cost.ps1`, média do platô de 20
  edições de corpo, 5 rodadas alternadas, máquina carregada por outros
  agentes): base **536 ms** × atual **465 ms** de mediana (0,87; faixas
  419–714 e 393–613 ms) — sem piora; os 227 ms do §1.3 são de máquina
  livre.

---

## 2. O que falta

### 2.0 O compilador de visões do ngdart

Especificação e resultado da rodada de 2026-09-30:
`docs/NGDART-COMPILADOR-DE-VISOES.md`. Medido pelo oráculo do
`build_runner` (cópias em `E:\dftemp\ngdart`, `oraculo --all`: o pacote e
toda dependência com saída do ngdart, menos o próprio compilador, que o DDC
nunca pede):

| projeto | antes (iguais / diferentes / pendentes) | agora |
|---|---|---|
| new_sali/frontend (ngdart 8, com `limitless_ui` 1.0.0-dev.34, ngdart, ngforms, ngrouter, ngtest) | 791 / 6 / 25 | **821 / 0 / 1** |
| limitless_ui/example (ngx_dart 9, com `limitless_ui`, ngx_dart, ngx_forms, ngx_router) | 368 / 23 / 114 | **504 / 0 / 1** |

O pendente que sobra em cada um é o `web/scrollbar.css`, que o
`generate_for` do `build.yaml` exclui (o oficial não gera nada; o harness
não aplica o `generate_for`, o motor sim). Ou seja, nenhum `.template.dart`
ou `.css.shim.dart` desses projetos precisa vir do disco.

O `limitless_ui` atual usa o fork `ngx_dart` 9.0.0-dev.2 (`package:web` no
lugar de `dart:html`); o gerador ganhou o dialeto dele
(`crates/ng_generator/src/dialeto.rs`), e o motor registra o builder
`ngx_dart:ngx_dart` como nativo.

O Sass nativo ganhou o modo dart-sass 1.101.0–1.101.3 (os locks dos dois
projetos), e o emissor JS as baixas de interop do `package:web` que o
`ngx_dart` usa (construtor não-`external` de *extension type*,
`Function.toJS`, `isA<T>()`; casos 236–238 do `corpus/js`). Pelo motor
(`build --compare`): new_sali 1561 saídas iguais / 0 / 0, limitless 937 /
0 / 0. `dartforge serve` em cópias sem `.dart_tool/build`: os dois abrem no
Edge headless com 0 erros no console e o CSS servido.

### 2.1 Correção (ordem de prioridade)

1. **Lacunas de inferência de tipos** (o `dart analyze` oficial dá 0
   diagnósticos nos projetos: todo aviso nosso é falso positivo). Medido
   em 2026-09-23 com o oráculo (`tools/type_oracle`, método em
   `docs/FRONTEND-NEW-SALI.md`): `new_sali/core` 13.431 → **15** avisos,
   `frontend` 57.883 → **28** (17 em templates gerados, legítimos);
   divergências de tipo estático por expressão 137.428 → 31 (core) e
   316.756 → 56 (frontend); corpus `corpus/inference` 83/95 programas
   iguais ao oráculo (teste no CI). Os
   grupos restantes, por causa, estão em `docs/FRONTEND-NEW-SALI.md`.
   O contrato é `docs/INFERENCIA-ESPECIFICACAO.md` (regras do analyzer
   6.11 com arquivo:linha, e o que muda até 3.14), com o corpus de
   conformidade `corpus/inference/` (95 programas, tipos gravados pelo
   oráculo) e a fila de lacunas do motor em `docs/INFERENCIA-LACUNAS.md`.
2. **Escrita em disco** no `limitless_ui`: 233 s para 484 arquivos, contra
   10,5 s de compilação. É I/O do Windows com antivírus, não compilador —
   o `dartforge dev` já contorna (reescreveu 58 arquivos na recompilação),
   mas o `compile-js` de projeto grande ainda sofre.
3. **Tamanho do JS**: 48 MB (dev, sem tree shaking) contra 4,5 MB do
   `main.dart.js` do dart2js em release. A comparação só será válida
   contra o DDC em modo de desenvolvimento, ou depois do modo de produção
   (§2.3). Medição pendente.
4. **`new_sali/backend`** (angel3): não compila; usa `dart:io`,
   `dart:isolate`, `dart:ffi`. Alvo do backend nativo.
5. Divergências web×VM declaradas (7 programas do corpus): são do próprio
   DDC (bits de 32 bits, `1.0` imprimindo `1`, `-0.0`), não defeitos.

### 2.2 Latência (o caminho está medido, não é chute)

1. **`Program` e outline reconstruídos a cada compilação** — 160 ms no
   core, 277 ms no frontend, 55–83% do que resta numa edição. Precisa de
   **ids estáveis por biblioteca** (arenas de elementos por biblioteca)
   para reaproveitar o outline das bibliotecas intactas. Com isso a edição
   de corpo cai para a faixa de 60–80 ms (estimado a partir das fases já
   medidas).
2. **Camada de texto da emissão**: 9,76 M alocações por compilação
   completa, ~7,3 M delas em `String` por nó de expressão. A correção é o
   **buffer único por módulo** (`emit_expr` escreve direto e devolve a
   faixa, em vez de devolver `Js`). Estimado: −6 M alocações.
3. Emissão paralela por módulo (hoje é série).

### 2.3 Modo de produção

**Existe, e é o de §1.2.1**, com o mundo fechado sobre a nossa trilha. O
que ainda falta, na ordem do `docs/JS-PRODUCAO.md` §6:

* precisão do mundo: restrição pelo tipo do receptor e espécie de seletor,
  o que o `limitless_ui` pede;
* suíte e2e do `limitless_ui` com o bundle de produção;
* despacho direto por alvo único;
* minificação;
* deduplicação de funções na trilha tipada;
* code splitting (`deferred` virando `import()`).

Achado de passagem: a ordem dos encaminhadores de `noSuchMethod`
(`Ctx::unimplemented_abstract`, iteração de `HashMap`) muda entre execuções
do mesmo binário. É um não determinismo do emissor anterior a este trabalho.

E o limite estrutural, medido e registrado: **enquanto o runtime for o
`dart_sdk.js` do DDC, o piso é da ordem de 1 MB, não os 35 KB do
`dart2js`**. Ele chega lá porque compila o SDK do fonte com inferência
global; o nosso vem pré-compilado, com tabela de assinaturas, receitas rti
e métodos de extensão emitidos para qualquer uso possível — dá para apagar
a classe ou o membro inteiro, nunca a metade do metadado que sobra. Fechar
esses 30× é o passo 4 do PLANO (compilar o SDK pela nossa trilha), não uma
otimização a mais.

### 2.4 LSP

Além dos diagnósticos, já existem símbolos de documento e workspace, hover
e definição conservadores para nomes do mesmo arquivo e URIs relativas
(`docs/LSP.md`). Faltam referências entre arquivos, completion, rename,
code actions e a resolução semântica de imports e tipos pelo `trait Analisador`.
As versões de linguagem em cache são descartadas ao fechar ou reabrir o
documento; o teste dirigido cobre mudança do `package_config.json` entre
as duas aberturas.

Diagnósticos (ver §1.4): o `dartforge analyze` publica a sintaxe e 50
códigos verificados (2026-09-26); o editor recebe a sintaxe e, desses, os
que não dependem de tipos (o LSP ainda não roda `types`). O caminho até os
demais:

1. **T1 em `types`** (agente da inferência): código, argumentos e unidade
   de cada diagnóstico; ele apaga a `paridade/src/ponte.rs`.
2. **Sintaxe**: portar a recuperação do fasta — um erro por token no topo
   (`expected_executable`) e nos membros (`expected_class_member`), e a
   do comando (`missing_const_final_var_or_type`). Recurso de linguagem que
   o SDK 3.6.2 não conhece **não** pede um parser que o rejeite na versão
   3.6: o analyzer 3.13.4 relata `experiment_not_enabled` na mesma
   biblioteca, como nós; o oráculo desses arquivos passou a ser o 3.13.4
   (2026-09-25, ver o topo deste arquivo).
3. **A3 restante**: `unused_import` exato (o `ImportsTracking` pede a
   resolução completa), `unused_element` dos privados de topo e membros,
   `unused_field`, `dead_code`.
4. **Publicar**: `unused_local_variable` e `duplicate_definition` entram na
   lista quando fecharem 100% no corpus (hoje 96% e 55%).
### 2.5 Backend nativo

**Projetos reais (2026-10-01, docs/NATIVO-PROJETOS-REAIS.md).** O
`new_sali/backend` (angel3, `dart:io`, isolados, FFI, Postgres, Redis)
compila, liga e sobe no nativo; com banco e Redis descartáveis, 39 de 42
requisições de ponta a ponta (login OIDC, token, 31 rotas de API) iguais à
VM — as 3 restantes são o texto do stack trace num JSON de erro e linhas
de auditoria do mesmo banco. Treze causas corrigidas (C1–C14 do documento),
com os programas `corpus/native/120`–`128`. O placar do pub roda no CI
(`scripts/pub-scoreboard.py`, `.github/workflows/pub-scoreboard.yml`).

**Estado atual.** O SDK da fonte é o padrão e o corpus nativo passa por
ele (`docs/NATIVO-PLANO.md` §7.10 e §7.13, que registram 225/225, e JIT × AOT
225/225); o último placar registrado neste arquivo é **183/223** no SDK da
fonte (rodada `ci/native-sdk-is-selector`, acima). `async`/`await` e laço de
eventos (`lower/async_sm.rs`), genéricos reificados (`lower/rti.rs`),
`dart:io` e isolados (`corpus/native/01`–`07`), `dart:ffi`
(`corpus/native/08`, `12`–`19`) e o `dart:core` da fonte existem. O que
falta de API pública está em `docs/NATIVOS-PENDENTES.md` (observação de
arquivos, multicast, passagem de descritores por socket,
`RawSynchronousSocket`, heap snapshot).

> **Histórico (até 2026-09-27).** O texto abaixo é o diagnóstico de antes da
> rodada 2 e de P5; as frases «continuam faltando `async` e event loop,
> genéricos reificados, `dart:io`, isolates e o `dart:core` da seção `vm`» e
> «0% do IR é corpo de função do SDK» deixaram de valer: o SDK da fonte é
> compilado num módulo próprio (`docs/NATIVO.md` §1).

**Depois de P1–P4 (Pesado 35871381320): 142 dos 223 falham.** Quase todos
por membro do SDK sem implementação no runtime (`where`, `map`, `fold`,
`toStringAsFixed`, `sort`, `List.filled`/`List.generate`, `parse`,
`hashCode`…, que o P5 resolve com o SDK da fonte), `await`/`yield` (P6/P7)
e o `toString()` de objeto do programa dentro de uma coleção impressa (o
runtime não chama código Dart). Da linguagem, faltam os genéricos em tempo
de execução (RTI: `is List<int>` é diagnóstico) e `super` dentro de mixin.
A tabela abaixo é a de antes da rodada 2.

172 dos 222 programas do corpus (CI, run 35823269758), agrupados pelo
relatório do harness (`--native`). A família de "handle" (71 de 214 no
começo — escalar usado como handle, null desreferenciado, raiz faltando)
**zerou**, e a natureza do que falta mudou: quase tudo agora é construto
que o backend declara não suportar, com o nome do construto no placar.

| falhas | causa |
| --- | --- |
| 164 | **erro de compilação: não suportado** — em 85 grupos; os maiores: chamada de valor de função/closure (`f`, `cb`, `callback`, 9 + 6), `switch` (expressão 8, comando 6), aritmética sobre `num`/`dynamic` (8), `super` como valor (6), `await`/`yield` (5 + 4), cascata (4), constante de enum (4), `hashCode`/`Object.hash` (4), `int.parse`/`double.parse` (4), e membros do SDK sem implementação no runtime (`clear`, `sort`, `addAll`, `toStringAsFixed`, `abs`, `where`…) |
| 4 | roda, mas imprime diferente da VM (`27_operadores_logicos_curto_circuito`, `66_colecoes_literais_spread_if_for`, `156_antigo_generics_constants`, `178_antigo_nativo_mixins`) |
| 3 | nem carrega: `dart:js`, `dart:js_util`, `dart:js_interop` (não existem na seção `vm`) |
| 1 | o Clang ainda recusa o IR |

Os mesmos 50 passam sob `--gc-stress` (coleta antes de toda alocação; run
35823275126). A lista do que fazer agora é a coluna de construtos acima,
do maior grupo para o menor; closures (com captura em célula) e `switch`
desbloqueiam mais que qualquer outro item. Depois disso, o passo 6 do
contrato (raízes só onde há ponto de coleta, pela tabela de efeitos das
externs; raiz como `store` num quadro em memória) — só com medição.

O lowering de exceções existe (`throw`/`try`/`catch`/`finally`/`rethrow`,
com o `finally` como sub-rotina e discriminador de razão); falta acertar os
textos de `toString` dos erros do `dart:core`, que o corpus compara byte a
byte. Continuam faltando `async` e event loop, genéricos reificados,
`dart:io`, isolates e o `dart:core` da seção `vm` a partir da fonte. O
cache de objeto **por programa** existe (§3.2); **por módulo**, com o SDK
compartilhado entre programas (o resumo por biblioteca de
`docs/PESQUISA-OTIMIZACAO.md` §6), depende de separar o SDK em módulo
próprio com símbolos e ids estáveis — hoje não há o que separar: 0% do IR
é corpo de função do SDK (§3.2).

### 2.6 ngdart e geração de código

O compilador de templates próprio **existe e cobre 134 dos 300 arquivos**
do `new_sali/frontend` (§1.7). O que falta dele está na tabela do §2.0.
Enquanto não fecha, compilar um projeto ngdart ainda exige
`dart run build_runner build` uma vez (2m59s no `new_sali/frontend`) para
os arquivos pendentes — o motor de build (§1.8) usa o que ele deixou no
disco como **apoio** e avisa quando o apoio está mais velho que a fonte.

O que falta do motor (§1.8), em ordem: o **estágio B** do ngdart (uma ação
por componente, consultas finas), que depende dos acréscimos públicos
pedidos ao `ng_generator` em `docs/BUILD-PEDIDOS-GERADOR-NG.md` e é o que
leva a edição de componente para baixo de 500 ms; o Sass byte a byte do
`sass_builder` (mesmo documento, item 4), para o `.css` servido sair do
nativo; o executor Dart **auto-hospedado** (`dfexec/1`,
`docs/BUILD-PROTOCOLO.md`), compartilhado com as macros — o pela VM já tira
os pendentes do `corpus/builders`, mas a VM oficial não é o produto; e o `go_router_builder` no corpus (D-B4, exige Flutter).

Plano para substituir o `build_runner` por um motor em Rust:
`docs/BUILD-RUST.md`. O dado que o orienta: dos 9.879 artefatos que o
`build_runner` gera nesse projeto, **934 `.ddc.js`/`.ddc.dill` e ~7.980
de bookkeeping são do `build_web_compilers`** — o compilador que já
substituímos. Nos dois projetos do proprietário sobram três builders
(`ngdart`, `i18n`, `sass_builder`).

**Regra governante (PLANO.md): tudo nosso, compatível com o
ecossistema.** A VM oficial não faz parte do produto; `json_serializable`,
`freezed`, `drift`, `mockito`, `source_gen` e os demais têm de funcionar
**no nosso runtime**. Isso ordena as prioridades do backend nativo: o
alvo dominante é compilar e executar o `package:analyzer` (438 arquivos,
227.252 linhas, `dart:io`/`isolate`/`ffi`/`typed_data`), de onde esses
geradores dependem. Geradores nativos em Rust são aceleração opcional,
com saída byte a byte igual verificada por `corpus/builders/` (existe:
`corpus/builders/` com `freezed`, `riverpod_generator` e outros, §1.8). O `dart` oficial fica só como oráculo de comparação.

### 2.7 JIT

O R0 e a recarga com estado pela CLI funcionam (§1.5.1): `run` e `reload`
sobre o mesmo IR do `emit_native`, e os cenários de recarga foram migrados
para `crates/jit/tests/hot_reload.rs` («migrados da trilha antiga para o IR
do emit_native»). Falta:

* o que a recarga ainda recusa ou não confere (`docs/JIT.md`, «Escopo da
  versão 1»: mudança de assinatura, campos de classe, classe renumerada,
  ambiente de closure) — `docs/PESQUISA-HOT-RELOAD.md`.
  *Histórico (até 2026-09-27):* este item dizia que a recarga com estado
  (R1) faltava e que os cenários de `crates/jit/testes-pendentes/` não
  compilavam;
* preservar o contrato de `crates/jit/tests/execucao.rs` — o mesmo programa
  compila uma vez e executa pelos dois caminhos (ORCv2 e AOT) exigindo saída
  idêntica. Divergir em tempo de compilação é esperado; em resultado, é
  defeito;
* a recomendação de `docs/historico/CRANELIFT.md` continua: **não adotar**
  Cranelift. O experimento foi feito e medido (o crate saiu do workspace e
  está na branch `exploracao-inicial`); quem for mexer em JIT lê isso antes
  de repetir.

---

## 3. Como verificar tudo isto

### 3.0 No GitHub Actions — o caminho normal para o que é pesado

A máquina de desenvolvimento (8 GB, compartilhada por vários agentes) não
roda mais corpus nenhum: o repositório é público, os minutos de Actions são
gratuitos, e os runners Windows têm 4 núcleos e 16 GB. Dois workflows:

* **`ci.yml`** — todo push no `main` e em `ci/**`, todo PR; ~2,5 min.
  `cargo test --workspace` (todos os alvos e os doctests), os
  `#[ignore]` que o runner satisfaz (SDK Dart 3.6.2, Clang
  22.1.8, rustc) e `cargo doc -D warnings`. O que fica de fora e por quê
  está no cabeçalho dele e em §1.6.
* **`heavy.yml`** — push em `ci/**`, `workflow_dispatch` (suíte e número de
  fragmentos) e todo dia às 03:17 (Brasília) no `main`. Medido na rodada de
  2026-09-23 (`ci/infra`, 214 programas):

| job | o que roda | placar | tempo do job (harness) |
| --- | --- | --- | --- |
| compilar | release de `dartforge-differential`, `dartforge`, `dartforge-jsprod`; completa o cache dos oráculos (`verificar`) | — | 2,2 min |
| js (desenvolvimento) | `dartforge-differential --jobs 4` | **214/214** | 1,2 min (45 s) |
| js (produção) | `--production --jobs 4` | **214/214** e 214/214 | 2,3 min (1 min 48 s) |
| nativo K/2 | `--native --jobs 4 --shard K/2` | 4/107 e 3/107 | 1,5 e 1,7 min (28 e 30 s) |
| nativo (placar consolidado) | soma os fragmentos e funde o agrupamento de falhas | **7/214** | 0,5 min |
| determinismo (produção) | `determinism --production --workers 1,4,8` | idêntico | 7,5 min (7 min) |
| determinismo (nativo, IR) | `determinism --native --workers 1,4,8` | idêntico, 184 com IR | 0,6 min (5 s) |
| moderno (3.7–3.13) | `--corpus corpus/modern --production --jobs 4`, com o SDK 3.13.4 instalado por zip (cache pela versão); `PENDENTES` pode falhar, pendente que passa reprova | ver §1.1.1 | — |
| **rodada inteira** | | | **9,8 min** |

**Critério de cada job.** JS desenvolvimento e produção: código de saída do
harness 0, isto é, **todos os programas do corpus** passam — qualquer que
seja o tamanho do corpus, sem número fixo. Nativo: placar abaixo do total
é trabalho em andamento e não reprova; reprova se o harness quebrar
(código fora de {0, 1} ou relatório sem a linha de placar). Determinismo:
código 0. Cada job escreve placar, tempo, memória livre mínima e o
agrupamento de falhas no resumo da rodada (`$GITHUB_STEP_SUMMARY`), deixa
o placar numa anotação (aparece em `gh run view`) e sobe o relatório
completo como artefato `report-<job>` (14 dias).

**Convenção: uma branch `ci/<frente>` por frente de trabalho** (`ci/nativo`,
`ci/ngdart`, `ci/verificacao`, `ci/infra`…). Empurrar para ela roda o
`heavy.yml` inteiro e o `ci.yml` sobre aquele commit, sem mexer no `main`.
Rodadas de branches diferentes **nunca se cancelam** (o grupo de
concorrência inclui a ref); uma rodada nova na **mesma** branch cancela a
anterior. As branches `ci/**` são descartáveis: o push é forçado.

```powershell
pwsh scripts/ci.ps1 -Front native -Follow   # HEAD -> ci/native, acompanha até o fim
pwsh scripts/ci.ps1 -List                      # última rodada de cada ci/** e do main, com placar
pwsh scripts/ci.ps1 -Scoreboard <run-id>             # placar de cada job + relatórios baixados
pwsh scripts/ci.ps1 -Suite native -Ref ci/native -Shards 4   # workflow_dispatch
gh run view <run-id> --log-failed                # o log do que falhou
gh run download <run-id> -p 'report-*'          # relatórios completos
```

`-Suite` (workflow_dispatch) só funciona depois que o `heavy.yml` estiver
no `main`; até lá, `-Front`.

**Limites do plano gratuito, e o dimensionamento.**

* **20 jobs simultâneos na conta inteira** (Linux e Windows juntos); o
  excedente espera na fila, não falha. Uma rodada `todos` com N fragmentos
  tem pico de 4 + N jobs: com o **N = 2 padrão**, 6 — cabem três frentes ao
  mesmo tempo. N sai da medição (§3.2): o corpus nativo inteiro custa ~1 min
  de harness, e cada job gasta ~1,3 min só preparando o ambiente. Subir N
  (`-Shards`, ou `gh variable set NATIVE_SHARDS --body N`) quando um
  fragmento passar de ~20 min.
* **10 GB de cache por repositório**, e o que foi usado há mais tempo sai
  primeiro. Um branch só lê o próprio cache e o do `main`; por isso: o
  `rust-cache` só é **gravado no `main`** (as `ci/**` restauram o do main e
  não multiplicam entradas); o Clang 22.1.8 tem chave fixa pela versão
  (`llvm-22.1.8-windows-x64-clang-v2`); os oráculos (`dart run`,
  `dartdevc`+Node) têm chave pelo hash de `corpus/js/**` e o `restore-keys`
  traz o anterior, de modo que só programas alterados são recalculados
  (~55 KB). O SDK Dart não é cacheado: o `setup-dart` o baixa em ~10 s. O
  agendamento diário no `main` é o que mantém esses caches no escopo que
  todas as `ci/**` leem.

### Na máquina local

```powershell
pwsh scripts/generate-dart-sdk.ps1              # dart_sdk.js (3 s, uma vez)
cargo build --release -p dartforge-cli -p dartforge-differential
cargo run --release -p dartforge-differential # 214/214

# projeto real
cargo run --release -p dartforge-cli -- compile-js `
  C:/MyDartProjects/new_sali/frontend/web/main.dart -o saida `
  --packages C:/MyDartProjects/new_sali/frontend/.dart_tool/package_config.json
pwsh scripts/serve.ps1 -Dir saida -Web C:/MyDartProjects/new_sali/frontend/web -Flow

# sessão residente
cargo run --release -p dartforge-cli -- dev <entrada.dart> -o saida --packages <cfg>
```

## 3.1 Espaço em disco — vigiar

O `target/` do Cargo **não se limpa sozinho**: num dia de trabalho com
vários agentes chegou a **38 GB** (21 GB em `debug/deps`, 8 GB de
compilação incremental), tudo recriável e nada de código. Rode

```powershell
pwsh scripts/clean.ps1            # relata o que ocupa espaço
pwsh scripts/clean.ps1 -Clean    # apaga o lixo seguro (debug, target-*, incremental)
pwsh scripts/clean.ps1 -Clean -All  # também release e o cache de oráculos
```

Custo de recriar: `target/debug` ~10 min, `target/differential` (cache dos
oráculos `dart run`) ~10 min, `target/release` ~5 min. Sem custo, e
apagados sempre: `target/differential/nativo` (executáveis e `.ll` do corpus
nativo; o harness já apaga cada `.exe` depois de executar, salvo
`DARTFORGE_KEEP_EXE`) e as `.lib` do runtime em `target/native_cache/` fora
as 2 mais recentes (o próprio cache também poda). Só com `-Tudo`:
`target/native_cache/obj`, o cache de objeto, que se poda sozinho no teto de
`DARTFORGE_CACHE_OBJ_MB` (256 MB). `DARTFORGE_NATIVE_CACHE` muda o
diretório; sem ele, `$CARGO_TARGET_DIR/native_cache`. Worktrees de
agentes têm cada uma o seu `target/` — removê-las (`git worktree remove`)
depois de integrar o trabalho é parte da limpeza.

**Regra dos temporários: no SSD E:, nunca no C:.** O C: tem pouco espaço e o
`%TEMP%` chegou a 10,5 GB de sobras. Temporários grandes vão para
`target/tmp-*` ou `target/scratch-*` do repositório (os scripts de
navegador, `measure-lsp.ps1`, `verify-js-pruning.ps1` e `ci.ps1 -Scoreboard`
gravam lá); agentes e sessões longas definem antes
`$env:TEMP='<repo>\target\tmp-<frente>'; $env:TMP=$env:TEMP` e apagam a
pasta no fim. Todo teste ou comando que cria um temporário o apaga também
no caminho de erro, pânico ou tempo esgotado (`tempfile::tempdir`, ou um
guarda com `Drop` como `OutputDir` em `crates/jit/tests/execucao.rs` e
`DiretorioGeracoes` no `dartforge reload`). `clean.ps1 -Clean` apaga
`target/tmp-*`, `target/scratch-*` e as sobras antigas no `%TEMP%`
(`dartforge-*`, `dfserve*`, `df-recarga`, `lsp-stdout-*`).

### 3.2 Verificação rápida do backend nativo — medido

O corpus nativo foi registrado aqui como **10 programas em 9 minutos**
(~3 h por passada, ~10 h para o determinismo), e o determinismo só tinha
sido verificado num subconjunto. As três saídas propostas estão feitas,
seguindo `docs/PESQUISA-OTIMIZACAO.md` §11 (determinismo com 1, 4 e 8
trabalhadores) e §6 (resumo e cache por módulo):

1. **Determinismo não executa.** `dartforge-differential determinism
   --native` emite o LLVM IR de cada programa dentro do processo
   (`emitir_ir`, sem Clang, ligação nem execução) e compara, programa a
   programa, o resumo FNV-1a de 128 bits — ou a mensagem inteira do erro.
   Divergência lista todos os programas e grava o primeiro, emitido sozinho
   e `n` vezes em paralelo, em `target/differential/determinismo/`.
   `--run` mantém o caminho antigo. O relatório (uma linha
   `<hash> <bytes> <nome>` por programa) não tem tempos nem caminhos: um
   `diff` de dois relatórios diz quais programas mudaram de IR.
2. **Cache de objeto por programa** (`cache_objeto.rs`): mesma entrada,
   mesmo hash, mesmo `.obj`. O cache do runtime ganhou chave estável
   (versão do `rustc` + bandeiras + fonte), publicação atômica e uma
   compilação por processo — antes, um trabalhador podia ligar contra uma
   `.lib` pela metade.
3. A **amostra estratificada** não foi feita: com os números abaixo, a
   passada de determinismo inteira custa menos que escolher a amostra.

Números (release, máquina de 8 núcleos e 7,7 GB compartilhados):

| medida | valor |
| --- | --- |
| emissão de um programa (front-end + HIR + LLVM IR) | 1–40 ms; HIR e LLVM IR < 1 ms |
| LLVM IR por programa | 13–120 KB, média 30 KB (184 programas) |
| fração do IR que é corpo de função do SDK | **0%** |
| pico de memória de `compile-native` | 6 MB |
| determinismo IR, 214 programas, 1/4/8 trabalhadores | **0,5 / 0,2 / 0,2 s** por passada; 3,8 s o processo inteiro |
| pico do harness no determinismo IR, 8 emissões simultâneas | 10 MB |
| Clang `-O0` de um programa, morno | 33–84 ms |
| Clang com acerto no cache de objeto | 21 ms (é o `clang --version`) |
| ligação, morna | ~95 ms |
| runtime (`rustc -O`), uma vez por conteúdo | ~10 s |

> **Histórico (até 2026-09-27).** A tabela acima e o parágrafo abaixo são
> da medição de antes do SDK da fonte. Hoje o `include` é seguido
> (`crates/elements/src/sdk.rs:191-212`), o SDK da fonte é compilado uma vez
> por conteúdo num módulo próprio, com ids de classe fixados pela tabela do
> SDK compilado (`sdk_modulo::ids_de_classe_do_sdk`,
> `crates/emit_native/src/sdk_modulo.rs:220`) e símbolos estáveis por
> declaração (teste `t_id_simbolos_estaveis`), e o cache por módulo existe
> (`docs/NATIVO.md` §1.1). A linha «fração do IR que é corpo de função do
> SDK: 0%» vale só para o módulo do programa.

Por que era tão barato: a seção `vm` do `libraries.json` só declara
`dart:cli`, e o `include` de `vm_common` ainda não era seguido — o programa é
compilado sem o SDK, e é por isso também que 30 programas nem carregam
(`dart:math`, `dart:async`, `dart:collection`…). Quando o SDK entrar, cada
emissão analisa o SDK inteiro (no JS isso custa centenas de MB, §1.3); por
isso o harness separa emissões simultâneas de trabalhadores:
`DARTFORGE_IR_PARALLEL_MAX` (padrão 2) limita as emissões físicas, e a
ordem de conclusão continua variando, que é o que o teste precisa. O
cache por **módulo** — o SDK compilado uma vez e compartilhado entre
programas — só paga depois disso, e exige símbolos e ids de classe estáveis
(hoje são índices globais dependentes da ordem de carga).

Reprodutibilidade do objeto, medida: o Clang gravava o `TimeDateStamp` no
cabeçalho COFF, e o mesmo IR dava objetos diferentes no byte 4;
`-mno-incremental-linker-compatible` zera, e agora dois Clang sobre o mesmo
IR dão o mesmo `.obj`. O Clang também roda no diretório do objeto com o
nome relativo (o hash), para o `source_filename` não levar o caminho de
quem compilou.

**A passada executada completa roda no CI, em fragmentos, e foi medida**
(`heavy.yml`, §3.0; runner Windows de 4 núcleos e 16 GB, `--jobs 4`,
`DARTFORGE_HEAP_MAX_MB=256`): **7/214**, o mesmo placar da máquina local,
em **28 e 30 s de harness** para os dois fragmentos de 107 programas —
cada job inteiro, com a preparação, 1,5–1,7 min. Com 8 fragmentos, cada um
levou 5–10 s. A memória livre do runner nunca caiu abaixo de 12,8 de 16 GB.
Ou seja: os **9 minutos para 10 programas** eram da máquina local, não do
backend — sem oráculos em cache, disputando memória e disco com os outros
agentes. O tempo desta passada vai crescer quando mais programas rodarem
até o fim (hoje a maioria falha cedo); é aí que N sobe.

Medido também no CI, antes do cache do runtime com `OnceLock` e publicação
atômica chegar ao `main`: num runner novo, **62 dos 214 programas** caíam
em "compilação do runtime nativo com rustc falhou" — os trabalhadores
compilavam a mesma `.lib` ao mesmo tempo. Na máquina local o cache já
existia e a corrida não aparecia. Com a correção, a rodada seguinte, sem
aquecimento nenhum, deu 7/214 sem nenhuma dessas falhas.

## 4. Organização do repositório

* **Crates (todos em uso)**: `frontend` → `elements` → `types` →
  `emit_js` | `emit_native`, mais `emit_js_production` e `world` (produção
  JS), `ng_generator` (ngdart), `runtime` e `jit` (nativo), `abi`
  (`abi-info`), `dev`, `lsp`, `intern`, `diagnostics`, `instrument`,
  `differential` e `cli`. O workspace é `crates/*`, sem exclusões.
* **A trilha velha foi removida** em 2026-09-23: `lexer`, `syntax`,
  `parser`, `semantic`, `hir`, `codegen`, `linker`, `optimizer`,
  `packages`, `compiler`, `macros`, `llvm`, o driver `native` e o
  `cranelift-jit` (~65 mil linhas), com `tests/` da raiz (fixtures de
  conformidade, já convertidos em `corpus/js/*_antigo_*`) e os scripts
  `conformance*.ps1`/`benchmark-process.ps1`. Antes disso já tinham saído
  `web`, `ngdart` e `asmjit-jit`.
* **Branch `exploracao-inicial`** (`origin`, db75b90): preserva a árvore
  inteira antes da remoção, incluindo tudo o que foi removido.
* **Documentação**: `docs/` tem só o vigente; o que descreve a exploração
  inicial (incrementos 01–25, contratos do subconjunto antigo, CRANELIFT,
  ASMJIT, relatórios JSON e briefs de agentes já cumpridos) está em
  `docs/historico/`, com um README.
* Decisões e contratos: `PLANO.md` (governante), `docs/EMISSAO-DDC.md`,
  `docs/FRONTEND-ARQUITETURA.md`, `docs/NATIVO.md`, `docs/NATIVO-PLANO.md`,
  `docs/JIT.md`, `docs/JS-PRODUCAO.md`, `docs/LSP.md`,
  `docs/FRONTEND-NEW-SALI.md`, `docs/LIMITLESS-UI.md`,
  `docs/CONTRATO-DDC.md`, `docs/BUILD-RUST.md`, `docs/GERADOR-NG.md`.

**Armadilha registrada**: não rodar `compile-js` enquanto o `build_runner`
está rodando — o `--delete-conflicting-outputs` apaga a árvore de gerados
e o carregador falha em dezenas de `.template.dart`.
