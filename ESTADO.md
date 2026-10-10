# Estado do DartForge — 2026-10-08

## Retomada de 2026-10-10

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
Evidência em bench/resultados/2026-10-10-modos-55beeb34-windows. Captura inicial
de hashes ocorreu após duas execuções, no começo da fase de medição; os 18
hashes permaneceram iguais no fechamento. Ponto de captura explícito no
manifesto; não alegar captura anterior à primeira execução. ARC somente com
parâmetro explícito --memoria=arc; tracing padrão. §§27–34 ainda incompletos.
CI 38057171425: Windows/Linux/mensagens passaram; macOS continua nos testes
emit_native com ignorados. Mudança local sem interromper essa rodada.

Saídas do Tarjan compartilhado passam por conferência independente em
otimizar/scc.rs::conferir: cobertura única dos nós, alcance interno direto/
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
§§27–34 nem aprova gate ARC/A0. ARC só --memoria=arc; tracing padrão.
CI 38057171425: Windows, Linux e mensagens passaram; macOS está nos testes
emit_native com ignorados. Alteração local para preservar essa rodada.

Preparação compartilhada de tipos/raízes validada também em AOT real:
python scripts/provar-arc-aot.py arc_retorno_da_fonte --debug passou as
16 execuções ARC/tracing × O0/O2 da revisão 5cb4e56d, sem mudanças nos fontes.
Quatro positivas e 12 controles por trap conferem sobrevivência/morte do Mint
na cadeia identidade(identidade(valor)); retirar retain, drop do retorno ou
drop intermediário reprova a prova. Auditoria, ARC puro e GC stress ligados;
compilação malsucedida ou saída inesperada não contam como negativo aprovado.
Evidência congelada em
bench/resultados/2026-10-10-raizes-retorno-cadeia-windows: 96 arquivos brutos,
manifesto com comandos/versões/hashes e três logs (AOT, 306 testes, 98 exemplos).
Artefatos anteriores de 12 casos permanecem preservados. Gerador Rust debug,
Clang 22.1.8, SDK 3.6.2, executáveis O0/O2; não é medição de desempenho nem
certificado global. Gate ARC/A0 reprovado; §§27–34 continuam incompletos.
ARC só --memoria=arc; tracing padrão. CI 38057171425 ainda ativa no
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
ARC só --memoria=arc; tracing padrão. CI 38057171425: Linux/mensagens passaram;
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
ARC apenas --memoria=arc; tracing padrão. CI 38057171425: Linux/mensagens
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
não remove contagens ou aprova o gate ARC/A0. ARC só com --memoria=arc;
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
aprova o gate ARC/A0. ARC só com --memoria=arc; tracing padrão.
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
aprova o gate ARC/A0. ARC só com --memoria=arc; tracing permanece padrão.
CI 38057171425: Linux e mensagens passaram; Windows/macOS continuam ativos.
Alteração local para preservar a rodada da revisão publicada b0ee6645.

GrafoHeapArc local em otimizar/arc/analise/grafo.rs extrai arestas de campos
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
exclusivo de --memoria=arc, tracing padrão. CI 38057171425 ainda executa nos
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
exclusivo de --memoria=arc; tracing padrão. CI 38057171425 ainda em execução
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
somente --memoria=arc; tracing permanece padrão. Gate ARC/A0 não aprovado.
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
§§27–34. ARC só com --memoria=arc, tracing padrão; gate ARC/A0 segue reprovado.

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
em otimizar/arc/analise/chamadas.rs. Alcance dos corpos é descoberto sem
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
eliminação de contagens. ARC só com --memoria=arc; tracing permanece padrão.
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
remove contagens. ARC só com --memoria=arc; tracing permanece padrão.
Não conclui S0/S1, §§27–34 ou o gate de desempenho ARC/A0.

ResumoHeapArc inicial em otimizar/arc/analise/resumos.rs separa retorno
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
exclusivo de --memoria=arc; esta etapa não conclui S0/S1 ou o gate ARC/A0.

CI 38051323600: Windows cancelado ao exceder limite do job de 1h,
conforme anotação do check 114210866777. Não houve falha de teste nesse
ponto: matrizes nativas concluídas, interrupção durante ng_transparencia
no passo dos ignorados. Log target/ci-windows-37497527.log. Linux passou;
macOS ainda ativo. Não considerar rodada integralmente aprovada.
Limite Windows ampliado para 90 minutos, como nos jobs Unix, para permitir
conclusão dos testes ignorados após compilação/matrizes. Alteração local,
sem nova rodada enquanto macOS da revisão publicada continua ativo.

Análise local de escape em otimizar/arc/analise/escape.rs agrupa publicações
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
alterada; somente --memoria=arc seleciona ARC. CI 38051323600 continua
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
ARC continua exclusivo de --memoria=arc. CI 38051323600 permanece ativa:
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

Extração HIR inicial em otimizar/arc/analise/hir.rs valida SSA e consome
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
--memoria=arc escolhe ARC na CLI. CI 38051323600 continua executando;
Linux passou, macOS passou retorno da fonte/instâncias zeradas/campos
tipados e chegou a campos locais; Windows executa recarga JIT.

Solver inicial de inclusão em otimizar/arc/analise/points_to.rs:
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
a ativação exclusiva por --memoria=arc. Não concluir S0/S1 ou o gate ARC/A0.
CI 38051323600: Linux terminou com sucesso; macOS passou pela seleção de
memória e retorno da fonte e executa instâncias zeradas; Windows passou
o teste geral e executa recarga JIT. Rodada ainda não concluída.

Base de domínio da §28.1 em otimizar/arc/analise/modelo.rs:
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
integração ao pipeline ou mudança da seleção explícita --memoria=arc.
CI 38051323600 continua ativa; macOS chegou à seleção de memória na CLI,
Linux chegou aos testes JIT e Windows continua no passo test.

Infraestrutura comum prevista na §33.2 agora em otimizar/scc.rs:
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
seleção de ARC continua exclusiva de --memoria=arc. Mudança ainda local.

Prova arc_retorno_da_fonte ampliada para repassar(valor) chamar
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
target/prova-retorno-da-fonte. Tracing compara o protocolo explícito do
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
Não altera ativação ARC: somente --memoria=arc escolhe essa política na CLI.

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

Prova AOT arc_retorno_da_fonte baixa uma identidade Object? da fonte Dart
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
bench/resultados/2026-10-10-retorno-da-fonte-windows: 72 arquivos brutos,
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
Fixture arc_retornos_guardados exercita a entrada integrada no caminho ARC
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
(sete ignorados) e 52 exemplos públicos. A fixture arc_retornos_guardados
passa a usar a publicação pelo módulo no caminho ARC automático; prova
AOT normal no Windows, com auditoria/puro/GC stress, terminou em zero e
imprimiu 9223372036854775807. Logs target/arc-publicacao-tabelas-*. Isso
valida a API explícita, sem certificar unwind Windows ou a integração
automática completa da fonte/SDK.

Contrato de ativação: ARC é opt-in por `--memoria=arc`; sem a opção,
tracing permanece padrão. Integração dos passes ao fluxo normal de
compilação deve respeitar essa seleção. C++ aparece nas fixtures de
exceções estrangeiras; compilador e runtime são implementados em Rust.

Correção da seleção na CLI registrada em 4fe197ef: AOT/compile-native/run/
reload partem de tracing, mesmo com DARTFORGE_MEMORIA=arc herdado; somente
a opção explícita escolhe ARC. run/reload agora reconhecem --memoria=arc.
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
dartforge_arc_objeto_owned_t_v1. Seu callback segue o protocolo estático de
tabelas, sem Dart/GC/exceções; função Rust unsafe documenta a validade da
memória devolvida. Catálogo confere a assinatura Rust completa e distingue
TabelaEstatica de um inteiro/endereço genérico. Produtor runtime genérico
recusa essa entrada sem proveniência do emissor, sem publicar mapas.

Regressões aprovadas: 253 testes do emissor, 182 do runtime, 52 exemplos
públicos do emissor e 42 do runtime; sete/três testes ignorados das respectivas
bibliotecas permanecem fora desta rodada. Exemplo arc_instancia_zerada usa
o thunk real emitido para uma biblioteca e registra somente na fábrica.
Matriz nativa Windows aprovou quatro positivos e 12 controles por trap,
ARC/tracing × O0/O2: owner vivo, campo zero, lookup/chamada do método e morte
após release; retirada do owner/tabela/release é detectada. Repetição final
após explicitar unsafe na assinatura Rust também aprovada. Logs em
target/arc-instancia-*; matriz e regressão da CLI adicionadas à CI local.
Fontes registradas em 7ab36e24. Evidência congelada em
bench/resultados/2026-10-10-instancia-zerada-windows: 80 arquivos brutos
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

Prova nativa reproduzível: scripts/provar-campos-tipados-aot.py recebe uma
CLI recompilada e executa a fixture campos_tipados_arc.dart em tracing
sem --memoria e ARC explícito, com/sem otimização. Gera também uma variante
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
bench/resultados/2026-10-10-campos-locais-windows: 16 execuções,
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

Prova AOT arc_campos_locais passou no Windows com llvm-embutido:
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
pendente e será exclusiva de --memoria=arc; tracing permanece o padrão.

Alocação Owned/inicialização de campos registrada no commit bee4dceb.
Evidência Windows congelada em
bench/resultados/2026-10-10-alocacao-campos-windows: 12 execuções,
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
Prova AOT arc_alocacao_campos passou no Windows com llvm-embutido:
ARC/tracing × O0/O2, quatro positivas e oito controles por trap sem owner
do objeto ou sem release do filho. HIR cria objeto de 40 campos/Mint;
confere bits 85, -0.0, true e byte 255, aresta Ref no índice 39, owner do
retorno e morte final de objeto/filho após liberação/coleta. Logs
target/arc-objetos-{suite,doc}.log e target/arc-alocacao-campos-matriz.log.
CI nova preparada para essa matriz, ainda não publicada. Não certifica
lowering Dart completo, origem de receivers para getters, guardas late/tipo,
registro de métodos, construtores, versões/pins, suspensão ou desempenho.
Não integra o passe ao pipeline padrão; ARC permanece exclusivo de
--memoria=arc e tracing continua o padrão.

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
congelados em bench/resultados/2026-10-10-keepalive-campos-ci, com hashes,
IDs/digests dos artefatos e etapas. Windows/Linux estavam completos com
sucesso; macOS ainda em execução ao registrar. Não certifica o workflow
inteiro, o script compartilhado novo ou mudanças posteriores de pending,
Phi/cleanup/laços. Limites de HIR explícita e tracing de comparação mantidos.

Prova AOT arc_cleanup_laco passou no Windows: ARC/tracing × O0/O2 ×
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
Evidência congelada em bench/resultados/2026-10-10-cleanup-laco-windows:
16 execuções e 80 arquivos brutos. Manifesto registra base 66fd85db,
modificações locais, hashes SHA-256 e blobs das fontes novas; não atribui
as fontes modificadas durante a prova à revisão base.

scripts/provar-arc-aot.py reúne campos, pending e laços, com saída positiva
exata 1 LF e controles exigindo trap de instrução, rejeitando erro de carga,
abort e falha de geração. Registra comandos, saídas, versões e hashes;
temporários ficam em target e são removidos ao encerrar. As três matrizes
locais passaram: 20 positivas e 28 negativas, 240 hashes conferidos. Laços
também passaram com llvm-embutido. Teste do exemplo passou (1 teste), YAML
confere 48 steps válidos. CI preparada para usar o script e guardar logs e
manifestos nas três plataformas; essa revisão ainda não foi publicada.
Na CI 38034929874 da fonte ed5897e9, Windows e Linux terminaram com sucesso;
macOS ainda em execução. A etapa de campos passou nas três plataformas,
mas essa execução não cobre o script novo nem pending/laços novos.
ARC continua exclusivo de --memoria=arc; tracing permanece o padrão.

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
continuam pendentes. ARC permanece exclusivo de --memoria=arc.

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
exclusivo de --memoria=arc.

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
target/arc-keepalive-pending-{example,build}.log e target/prova-keepalive-pending.
Matriz nova preparada na CI Windows/Linux/macOS, ainda não publicada nesta
retomada para não cancelar a validação em curso de ed5897e9. Não certifica
alocador de record no lowering (harness usa ABI legada com retain imediato),
fonte Dart, suspensão/recarga ou unwind. Tracing mantém tokens explícitos
somente no harness de comparação; ARC no produto continua exclusivo de
--memoria=arc.

Prova AOT arc_keepalive_campos passou no Windows: ARC/tracing × O0/O2,
quatro execuções positivas e oito controles negativos sem retenção ou sem
liberação final, todos recusados por trap (-1073741795). HIR de leitura Ref
seguida de substituição da aresta recebe keepalive automaticamente e retorna
Owned; o harness LLVM cria instância de 40 campos e Mint mortal, observa
owner 1→3→0 e morte do receiver após sair da função, sem alocar entre morte
e observação. Compilação usa o driver normal e as ABIs auditadas de objeto,
campo Ref e caixa Owned. Teste do exemplo confere retenção antes do setter
e preparação idempotente. Logs target/arc-keepalive-campos-{build,example}.log
e target/prova-keepalive-campos. CI foi ampliada para executar a matriz e
preservar IR/stdout/stderr/exit em Windows, Linux e macOS; execução desses
novos jobs ainda pendente. No tracing, o harness mantém o mesmo corpo de
tokens explícitos para testar compatibilidade runtime; não ativa preparação
ARC no fluxo padrão. Não prova lowering de fonte nem keepalive pending.
Evidência Windows congelada em
bench/resultados/2026-10-10-keepalive-campos-windows: fonte 663fe2ab,
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
continuam pendentes. Preparação permanece exclusiva de --memoria=arc.

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
exclusivo de --memoria=arc. Não é nova prova AOT nem medição de desempenho.
Passaram 233 testes do emissor (7 ignorados), incluindo 125 testes ARC,
e 52 exemplos de documentação. Logs target/arc-keepalive-{focado,suite,doc}.log.

ABI dartforge_arc_ler_campo_ref_v1 confere instância viva, índice e marca Ref
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
--memoria=arc. Sem nova prova AOT. Passaram 181 testes runtime (3 ignorados),
2 testes do catálogo, 41 exemplos runtime, 229 testes do emissor (7 ignorados)
e 52 exemplos do emissor. Logs target/arc-leitura-ref-runtime-{suite,doc}.log
e target/arc-leitura-ref-emissor-{suite,doc}.log.

ABIs dartforge_arc_gravar_campo_{escalar,ref}_v1 distinguem valor escalar
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
integração no fluxo normal nem nova prova AOT. ARC só por --memoria=arc.
Passaram 180 testes runtime (3 ignorados), 2 testes do catálogo e 40 exemplos
runtime; 225 testes do emissor (7 ignorados) e 52 exemplos de documentação.
Logs target/arc-gravacao-runtime-{focado,suite,doc}.log e
target/arc-gravacao-emissor-{suite,doc}.log.

ABI dartforge_arc_objeto_owned_v1 aloca INSTANCIA zerada e publica um único
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
normal continuam pendentes, sempre exclusivas de --memoria=arc.
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
--memoria=arc; tracing continua o padrão.

Provas AOT da fonte f81c7d2b preservadas em
bench/resultados/2026-10-10-unwind-boxing-unix-ci: 72 execuções Unix, 114
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
de --memoria=arc. Passaram 223 testes do emissor (7 ignorados), 52 exemplos
de documentação e cargo check --locked -p dartforge-jit --all-targets;
logs target/arc-layouts-{suite,doc,jit-check}.log.

ABI dartforge_arc_ler_campo_escalar_v1 copia bits somente após conferir
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
e target/arc-campo-emissor-{suite,doc}.log. ARC continua exclusivo de --memoria=arc.

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
preparação ARC permanece exclusiva de --memoria=arc.

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
sempre condicionada a --memoria=arc.

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
restrito a --memoria=arc; contratos de loads/campos e borrows exigem cobertura.

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
e será exclusiva de --memoria=arc; tracing continua sem essa preparação.

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
condicionada a --memoria=arc continuam pendentes.
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
somente com --memoria=arc; tracing permanece o modo sem essa opção.
A CLI já traduz a opção para DARTFORGE_MEMORIA, consultada antes de preparar
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
bench/resultados/2026-10-10-unwind-forcado-unix-ci: fonte 34198a54,
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
Evidência congelada em bench/resultados/2026-10-10-rastro-owned-estresse-unix-ci,
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
DARTFORGE_ARC_CONFERIR=1, DARTFORGE_ARC_BERCARIO=0 e DARTFORGE_GC_STRESS=1,
nomes efetivamente consumidos pelo runtime. Morte final do Mint, liberação dos
dois quadros, identidade C++, destrutores e restauração de raízes passaram.
Evidência congelada em bench/resultados/2026-10-10-unwind-estrangeiro-linux-ci,
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

Revisão dos ambientes de prova: o runtime lê DARTFORGE_ARC_CONFERIR,
DARTFORGE_ARC_BERCARIO e DARTFORGE_GC_STRESS. Atribuições anteriores a
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
`arc_unwind_estrangeiro` gera HIR com contrato auditado de print_handle Borrow,
catch preparado e Mint Owned; depois injeta o hook C++ por troca explícita de
símbolo no IR. Hook conserva assinatura/ownership, lança objeto C++ e não cria
pendência Dart. Fixture confere identidade nativa, destrutores, catch Dart não
executado, topo de raízes restaurado, owner consumido e Mint morto após coleta.
É fault injection, não chamada C++ proveniente do lowering FFI completo.
CI gera objetos LLVM O0/O2 e executa ARC/tracing sob ARC_CONFERIR/GC_STRESS,
com conferência de efeitos na geração; artefatos IR/C++/stdout são publicados.
Execução nativa desta fixture ainda pendente; teste HIR Windows e sintaxe C++
no WSL passaram. Não valida forced unwind, finally ou inlining observado.
Diagnóstico `dartforge_arc_observar_heap_v1` consulta páginas registradas e
inventário de owners sem dereferenciar endereço externo nem criar referência:
bit 0 bloco vivo, bit 1 owner do código. Catálogo parcial passa a 29 contratos,
argumento i64 Native (pode já estar liberado), resultado u8 escalar, efeitos0/0/0.
Teste cobre tracing/ARC × estresse direto ligado/desligado, dois tokens,
último release/coleta e endereços fora do heap; 33 doctests iniciais passaram.
Conferência de efeitos também precisa de unwind: snapshot de profundidade na
entrada e restauração no pouso abandonam externs atravessadas e suas proibições
de coleta. Não conferem saída normal nem alteram pendência. Teste nested do
runtime e corpo LLVM em DARTFORGE_EFEITOS=conferir passaram, objetos Linux O0/O2;
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
`bench/resultados/2026-10-09-excecao-owned-unix-ci`: fonte 7418650d,
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
DARTFORGE_EFEITOS=conferir, execução ARC_CONFERIR=1, BER=0, GC_STRESS=1.
Logs/IR/stdout `target/rastro-aot-*`. CI Unix usa o mesmo exemplo e requer
nova execução desta fonte. Não observa morte final em AOT nem identidade do
objeto nativo de unwind; não cobre rastro implicitamente capturado da pilha,
SEH/statepoints ou limpeza de owners do catch em unwind estrangeiro.
Prova anterior de captura Owned da exceção, fonte 7418650d, passou nas oito
variantes de cada plataforma Linux/macOS na CI 38013924479. Downloads em
`target/ci-38013924479/{linux,macos}`: 16 stdout MAX+LF conferidos byte a byte,
IR com getter Owned e resume. Job Linux completo terminou com sucesso.

Captura Owned do StackTrace: `dartforge_arc_rastro_owned_v1` retém o rastro
antes de devolver o handle, sem safepoint entre leitura e retenção. Quando
não há rastro corrente, cria um StackTrace sem instalá-lo para um throw futuro.
`dartforge_arc_lancar_com_rastro_ref_v1` publica valor/rastro Ref emprestados
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

Leitura Owned da exceção: nova ABI `dartforge_arc_excecao_owned_v1` conserva
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
com DARTFORGE_EFEITOS=conferir na geração; CI ativa essa conferência para os
próximos oito casos Unix. Logs `target/ownership-excecao-owned-*` e
`target/excecao-owned-*`. Execução Unix da leitura Owned ainda pendente.

Resume simples/misto executado também em macOS ARM64: CI 38011679848, fonte
5445efd54f24100d4cf3b1cd821de340bd115d85, passos AOT Linux/macOS terminaram
com sucesso nas 16 execuções. Artefatos baixados e conferidos byte a byte:
normal MAX+LF, erro vazio nessa fonte anterior à leitura Owned. Perfil misto
conferiu seletor catch no runtime real, com heap e cleanup/resume interno.
Congelado em `bench/resultados/2026-10-09-arc-resume-misto-unix-ci`, 32 IR/stdout,
fonte/jobs/digests/hashes. Sem prova direta de inlining, estrangeira/forced
unwind com owners do catch, rastro/identidade nativa/morte final ou SEH/statepoints.
Não valida lowering completo, §§27–34 completos nem gate ARC/A0.


Fechamento parcial da CI 38010046028: Linux terminou com sucesso, incluindo
as quatro provas ARC resume. macOS falhou no novo doctest de emit_all, antes
da prova AOT: o exemplo exigia target triple explícito, mas o cabeçalho macOS
é genérico e o gerador completa o alvo. Corrigida a asserção para presença de
declarações. Prova de geração do corpo escolhe Linux x86-64 ou aarch64 conforme
a arquitetura nativa registrada no LLVM, sem exigir backend x86 no runner ARM.
CI Unix passa a testar emit_native com llvm-embutido explicitamente, cobrindo
a geração O0/O2 dos dois corpos de cleanup/catch. Log macOS terminal obtido
pela API do job 114087818227 em `target/ci-38010046028/macos-falha.log`.
Não valida execução macOS; requer nova rodada com a correção e a variante mista.


Primeira execução AOT Linux do resume ARC integrado: o passo da CI 38010046028
(job 114087818147, fonte f626bb6887cdefb079eeba0910625111905e63bc) passou nas
quatro variantes automático ARC/tracing × normal/erro. Runtime real com Mint,
ARC_CONFERIR=1, BER=0, GC_STRESS=1; códigos zero e stdout byte exato.
Download auditado: propagador usa personalidade de cleanup, landingpad cleanup,
release antes de resume do par recebido, sem df.lancar nesse pouso.
Congelado em `bench/resultados/2026-10-09-arc-resume-linux-ci`, oito IR/stdout,
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
A prova AOT `arc_retornos_guardados` aceita `automatico misto`: o caller
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
`dartforge_personalidade_cleanup_itanium`, emite landingpad cleanup e retoma
o par original. Drops ARC precedem resume; não chama df.lancar nesse pouso.
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
preservado. Artefatos em `bench/resultados/2026-10-09-itanium-acoes-mistas-linux`;
logs `target/ownership-itanium-acoes-*`. A estrangeira é objeto de fixture,
não throw C++ de um objeto de linguagem. Sem heap/pendência/rastro/drops ARC
ou fonte Dart. O emissor ainda usa o protocolo legado; filtros tipados,
statepoints, SEH/funclets e integração no pipeline continuam pendentes.
Forced unwind tem somente prova unitária; macOS somente compilação.
Não altera o gate ARC/A0 nem conclui o objetivo completo.

Personalidade Itanium de cleanup: nova entrada Unix
`dartforge_personalidade_cleanup_itanium` separa cleanup puro de catch-all
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
em `bench/resultados/2026-10-09-itanium-cleanup-resume-linux`; logs
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
par do landingpad e reinicializa o objeto de unwind em df.lancar; SEH
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
em `bench/resultados/2026-10-09-arc-invokes-automaticos-aot`; logs
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
caller→intermediária→folha, rejeição atômica e emissão de df.lancar
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
Nova prova `arc_retornos_guardados`: quatro execuções AOT ARC/tracing sob
stress e auditoria, código 0. Normal imprime i64::MAX após liberar a cópia
de retorno e coletar; erro desenrola do callee por df.lancar, chega ao
landingpad do invoke no caller, limpa a exceção e libera o token da fábrica.
Artefatos em `bench/resultados/2026-10-09-arc-guardas-unwind-aot`; logs
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
release→df.lancar no LLVM, idempotência e preservação atômica na rejeição
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
`dartforge_arc_lancar_ref_v1` recebe borrow Ref, não consome token do código
e publica a raiz da exceção pendente; pode alocar o rastro. Catálogo agora
com 25 contratos, incluindo `dartforge_exception_clear`. O teste do runtime
passou em ARC e tracing com auditoria/stress: Mint vivo após release do
código e coleta enquanto pendente; morto após clear e nova coleta.
As variantes `retorno-mortal-erro-1/2/3` executaram os três retornos de erro
da prova AOT em ARC e tracing: seis códigos 0, saídas com zero/uma/duas
linhas, LLVM com clear antes dos drops inseridos. Artefatos e hashes em
`bench/resultados/2026-10-09-arc-excecao-ref-aot`.
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
A prova `arc_slots_fortes` usa agora esse resumo, sem contrato manual de
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
otimizar::arc`: 72 testes passaram. Ainda não integra o passe no pipeline
comum: contratos de chamadas e proveniência produzidos pelo lowering
continuam necessários.

Prova AOT do cleanup automático de saídas: a variante
`retorno-mortal-auto-cleanup` de `arc_slots_fortes` deixa os três retornos
de erro sem drops manuais. `inserir_arc_saidas_dart` insere exatamente
três liberações e nenhuma cópia; LLVM b1/b3/b5 libera v8/v14/v18 antes
do retorno. ARC com auditoria, berçário desligado e GC stress, e tracing
com GC stress, terminaram com código 0 e saída idêntica (Mint i64::MAX,
42, null). O controle sem cleanup continua rejeitado antes do IR pelo
token v8 pendente. Artefatos em
`bench/resultados/2026-10-09-arc-cleanup-saidas-aot`.
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
O benchmark isolado do módulo ARC real (`bench/arc/metadados.rs`) reduziu
o tempo de retain/release/consulta em cerca de 13%, confirmado numa segunda
rodada. O ganho dirigido em árvores/lista ficou perto de 2%, com dispersão;
a rodada completa ficou perto da média anterior, sem provar ganho relevante.

Benchmark completo auditável, 32 núcleos e sete execuções alternadas no núcleo
P: A1/A0 0,983, B0/A0 0,959, B1/A0 0,950, ARC/A0 2,107, A0/Dart 1,260.
378 execuções, resultados iguais em todos os modos e repetições, sem falha
nem núcleo ausente. Amostras brutas, protocolo e hashes dos 54 executáveis:
`bench/resultados/2026-10-09-modos-windows/`. ARC continua distante de A0
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

O Pesado passou a ativar também os quatro testes de `mapas_dirigidos` na
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
execuções. Dados e limites: `bench/resultados/2026-10-09-arc-faixas/`.
Isso confirma o ganho dirigido. A rodada completa depois da junção deu
A1/A0 0,991, B0/A0 0,969, B1/A0 0,952, ARC/A0 2,108 e A0/Dart 1,270,
com todas as 378 execuções válidas e resultados iguais. Não houve ganho
demonstrado na média ARC (antes 2,107); dados em
`bench/resultados/2026-10-09-modos-faixas-windows/`. A validação do corpus
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
`otimizar::preparar_para_emissao`, com o modo de memória definido antes dos
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
`DARTFORGE_EXCECOES=tabelas`. O corpus completo remoto ainda usa `719e94cc`,
anterior à separação do passe.
Validação local do passe separado, código de `a80d1b84`: A1, A1 sob
`--gc-stress`, B1 e B1 sob `--gc-stress`, todos 238/238 e código de saída 0.
O harness foi recompilado com `llvm-embutido`; SHA-256
`3F5B4C1286C66C8BFB25880798B3C90F5C5DDCF753FC4C675204A99CE3798738`.
Relatórios em `target/validacao-a80d1b84/`, incluindo a tentativa inicial
inválida (`DARTFORGE_MEMORIA=rastreamento`, rejeitada antes de compilar),
seguida das quatro rodadas válidas com `tracing`. São verificações de
correção; os tempos totais do harness não medem desempenho dos modos.

A base `otimizar::arc::vivacidade` calcula vivacidade reversa por ponto fixo
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
também passou com `DARTFORGE_EXCECOES=tabelas`, em 33,39 s, emitindo o
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
`bench/resultados/2026-10-09-arc-paginas`. O runtime voltou à versão
anterior, mantendo a correção do overflow de geração.
O runtime restaurado foi recompilado e conferido em release: 102 testes
passaram, zero falhas, três microbenchmarks ignorados, com
`DF_ARC_SEMENTES=3000` nos três modos de grafos. A CLI também foi reconstruída
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
`DARTFORGE_ARC_CONFERIR=1`. HIR, contratos por extern, slots persistentes,
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
Rodada local ampliada de `corpus/nativo` em ARC com estresse, auditoria e
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
original), sem auditoria, com estresse/ciclos e `DARTFORGE_GC_RASTRO=1`:
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
Registro auditável em `bench/resultados/2026-10-09-arc-corte-raizes`, com
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
`bench/resultados/2026-10-09-modos-5bbfc80f-windows`.
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
Teste de ciclo com `DARTFORGE_GC_RASTRO=1`: 1/1, com métricas emitidas
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
`bench/resultados/2026-10-09-arc-rastro-condicional`. Afinidade/configuração
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
`bench/resultados/2026-10-09-arc-iniciais-diretas`. Não há ganho amplo
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
roteiro em `bench/resultados/2026-10-09-finalizador-callback`. Não cobre
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
`dartforge_arc_quadro_{abrir,copiar,mover,fechar}_v1`. Declarada no LLVM
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
`bench/resultados/2026-10-09-arc-unwind-local`. Não é corpus completo,
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
Inspeção de simplificar::pura confirma que a ABI dartforge_arc_quadro_*
não é classificada como chamada pura: efeitos de GC/exceção nulos não
autorizam sua remoção. Isso é inspeção, não prova de execução integrada.

ABI mínima ARC implementada em arc_abi.rs: dartforge_arc_retain/release,
dartforge_arc_collect e dartforge_arc_verificar_abi. Tokens do código têm
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

Entrada LLVM ARC passa a conferir dartforge_arc_verificar_abi(1) depois de
ativar o modo e antes dos registros/código Dart, tanto autossuficiente quanto
modo SDK/preparar_isolado. Resultado diferente de 1 termina com llvm.trap e
unreachable, sem converter incompatibilidade em exceção Dart ignorável.
Tracing não emite a conferência. Isso só confere versão de tokens e modo do
heap; não certifica o contrato de cada módulo/extern ou retornos owned.
Suíte release do emissor: 116 aprovados, sete manuais ignorados
(`target/emissor-arc-conferencia-abi-release.log`). Validação compilada na
fonte eaa457d8: guarda extraída de df.preparar_isolado no IR real; stubs
retornando 1/0 compilam e respectivamente continuam/terminam por llvm.trap
(0xC000001D). Dois AOT ARC otimizados, checagem/tabelas, gc_d04_finally sob
estresse, auditados/ciclos sempre, produzem stdout igual ao Dart e stderr
vazio. Dez processos raiz: nove zeros e uma falha fatal esperada. CLI
release/JIT compilada com sucesso em 8m59s, hash F63880596B253DB5...2B22FFC0.
Evidência e script exato em bench/resultados/2026-10-09-arc-abi-entrada-local.
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
`dartforge_arc_global_receber_v1(id, valor)`: não desreferencia o ID, consome
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

Prova AOT dirigida no exemplo `arc_slots_fortes`: HIR explícita
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
`bench/resultados/2026-10-09-arc-slots-fortes-aot`: IR, saídas, hashes,
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

Prova AOT mortal no exemplo `arc_slots_mortais`: a produtora aloca um alvo
não permanente, transfere seu token SSA ao global forte e retorna antes das
coletas. Um contêiner weak com token próprio observa o alvo sem torná-lo raiz
no chamador; auxiliares LLVM conferem vivo antes e morto depois da remoção do
global. ARC com ARC_CONFERIR=1/ARC_BERCARIO=0 e tracing, ambos com GC_STRESS=1,
passaram com código zero e saída `1`, `null`. Imagens otimizadas reais pelo
driver. Controles negativos ARC: `sem-owner` abortou antes de imprimir;
`sem-liberar` imprimiu `1` e abortou na segunda verificação; ambos com
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

O relatório Linux expôs falha de preparação do oráculo DDC: gerar-dart-sdk.ps1
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
para ligar o produtor à prova AOT arc_slots_fortes; antes da publicação dos
planos, a prova ainda precisa representar invoke/cleanup de impressão no CFG.

Revisão da integração da prova AOT após 23fd911a encontrou lacuna no modelo
excepcional do plano ARC. EfeitoTokens.pode_falhar exige atualmente entrada
em TabelasDaFuncao.invocacoes, e a validação exige chamada no fim do bloco,
condição false e pouso. Esse é o formato de desenrolamento Dart; externs
runtime pending retornam normalmente com pendência e exigem leitura/conferência
explícita antes da bifurcação. otimizar/tabelas.rs::e_sitio não transforma
CallRuntime em invoke. Os testes com gc_collect/record fieldAt e pousos
sintéticos provam a transferência lógica do token, mas não essa integração
com a emissão real de externs pending. Não publicar esse formato como tabelas
LLVM de impressão. Próxima dependência: representar e verificar saídas de
pendência runtime separadamente de invoke Dart, incluindo disponibilidade do
resultado só no sucesso e cleanup por aresta, então ligar arc_slots_fortes.

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

Prova AOT arc_slots_fortes agora passa a HIR final por
produzir_e_verificar_tokens antes de emitir LLVM. Impressões têm leitura
exception_pending I8, comparação e bifurcação explícita; no erro, drop do
owner carregado e limpeza do global. Produtor gera classes das primitivas,
contratos auditados e mapa de pendências. Só literal permanente e caixa de
42 (Smi imediato) recebem contratos Trivial específicos fornecidos pelo harness.
ARC com ARC_CONFERIR=1/ARC_BERCARIO=0/GC_STRESS=1 e tracing com GC_STRESS=1
executaram com código zero e saída slots ARC/42/null. Variante sem-cleanup
recusada com v8 não consumido no caminho [0, 1], código 1, antes de gravar IR.
Evidência congelada em `bench/resultados/2026-10-09-arc-slots-verificados-aot`
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
em `bench/resultados/2026-10-09-arc-retorno-mortal-aot`. Prova sobrevivência
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
`bench/resultados/2026-10-09-arc-retorno-inserido-aot`, incluindo LLVM,
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
Variante sem-cleanup saiu 1 por token v8 não consumido no caminho [0,1];
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
`cargo test --locked --release -p dartforge-cli --features jit --test reload_estado -- --ignored --nocapture`,
com `DARTFORGE_SDK_DA_FONTE=1`: dois testes passaram, zero falhas ou
ignorados, cinco filtrados (os não ignorados), em 14,43 s. Executados
`cli_preserva_estatico_apos_editar_o_mesmo_arquivo_dart` e
`cli_preserva_o_estado_do_espaco_unificado_em_tres_recargas`. A listagem
`--ignored --list` do binário novo também seleciona esses dois pelo filtro
do CI. SHA-256 do harness `reload_estado-a53f60c187219ad4.exe`:
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
| **Nativo AOT** (`crates/emit_native`, `runtime`) | corpus 238/238 em Windows, Linux x86-64 e macOS arm64; `--gc-stress` 238/238; `dart:io` 130/130 nos três; `new_sali/backend` compila e responde 39/42 rotas no e2e; produção com LTO e o SDK da fonte | `bench/desempenho`: A0/dart = 1,22 (média geométrica; JSON, objetos e textos 1,5–2,4×, numérico e listas abaixo do Dart); e2e do backend precisa do banco |
| **JIT** (`crates/jit`, `dartforge run`/`reload`) | 238/238 e JIT × AOT sem divergência nos três sistemas; recarga de estado e `io_regressao` verdes | recarga com mudança de layout em todos os casos do R0 |
| **Exceções por tabelas (A1)** | 238/238 (+ `--gc-stress`); o pouso pega-tudo do `$ent` saiu (`a4b5ac42`): A1/A0 = 1,001 | tamanho: +3,2% no `new_sali/backend` (meta −3%); segue opt-in |
| **Raízes por mapas (B0/B1)** | forma nova por `"deopt"` (§14.12, abaixo): raiz no registrador preservado, sem derrame por chamada. Windows: corpus 238/238 em desenvolvimento com `--gc-stress` e as três conferências, e 238/238 em produção com o conferidor do RS4GC | testes dirigidos (`mapas_dirigidos`) caem num esgotamento de registradores do LLVM em `dart:convert` no modo de desenvolvimento (em curso); B1; Linux/macOS com a forma nova; o `bench/desempenho` inteiro; commit |
| **ARC** (`--memoria=arc`) | ARC puro correto: corpus 238/238 com auditoria, `--gc-stress`, ciclos em toda drenagem e JIT × AOT; grafos aleatórios contra oráculo (3000 sementes); efêmeros pelo ponto fixo | desempenho: ARC/A0 = 2,000274 em 55beeb34 (árvores 12,82×, lista ligada 5,28×); gate reprovado. Metadados por página já disponíveis; análise, provas, políticas e integração dos §§27–34 continuam incompletas. Evidência: `bench/resultados/2026-10-10-modos-55beeb34-windows` |
| **JS desenvolvimento** (`crates/emit_js`, contrato do DDC) | corpus diferencial **238/238** byte a byte contra `dart run` (medido hoje, 60 s); `limitless_ui` 26/26 no e2e; `new_sali/frontend` com os 11 passos do fluxo iguais ao oficial | — |
| **JS produção** (`crates/emit_js_producao`) | corpus `--producao` 238/238; `new_sali` 12.082.759 bytes, `limitless_ui` 6.693.718 bytes (brutos) | piso de ~1 MB enquanto o runtime for o `dart_sdk.js` do DDC: compilar o SDK pela nossa trilha (PLANO passo 4); precisão do mundo fechado, minificação, *code splitting* (`docs/JS-PRODUCAO.md` §6) |
| **Analisador** (`crates/analise`, `types`) | placar **22.900/23.012** (99,5%, posição exata), FP 17, FN 95; projetos reais sem diagnóstico a mais; CLI e LSP publicam os 167 códigos de `verificados.txt` | os 17 FP e 95 FN; conferir nos projetos reais os 391 códigos com zero FP no corpus para entrarem em `verificados.txt` |
| **LSP** (`crates/lsp`) | oráculo contra o `dart language-server` 3.6.2 (`r6`): símbolos, dobras, tokens, correções, hover, rename, highlight, selectionRange e workspace/symbol 100%; references 96%, implementation 96%, completar top-1/top-5 97%, assistências 98% | `SnippetTextEdit` nas assistências; latência de references/implementation (~290/227 ms contra 3/2 ms do Dart); incremental I5–I6; formatação (porte do `dart_style`, fora do escopo) |
| **Geração de código e ngdart** (`crates/build`, `dartforge serve`) | builders pelo executor nativo; `limitless_ui` e `new_sali/frontend` pela nossa trilha | compilador de visões do ngdart (§2.0) |

### Nativo: modos de raízes e exceções contra o padrão

`scripts/medir-modos-desempenho.py` (`bench/desempenho` em produção, 32 núcleos, 7 execuções alternadas,
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

1. Mapas: o esgotamento em `dart:convert` (desenvolvimento), os testes dirigidos, B1, o `bench/desempenho`
   inteiro com os cinco modos; commit e CI (Linux/macOS com a forma nova).
2. ARC: metadados por bloco (§19.1), retain/release em linha, drenagens sem `HashSet` por objeto; medir
   até ficar perto de A0; atualizar e commitar a `ARC-CICLOS-ESPECIFICACAO.md`.
3. Analisador: os 17 FP e os FN; ampliar `verificados.txt`.
4. LSP: snippets, latência de references/implementation, incremental I5–I6.

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
| memória ARC (`--memoria=arc`, auditoria a cada sincronização) | 238/238; ciclos em toda sincronização 238/238; `--gc-stress` 238/238; JIT 238/238 | 238/238 | 238/238 |
| `dart:io` (`corpus/nativo`) | 130/130 | 130/130 | 130/130 (era 129: a espera com prazo do laço de eventos passava do prazo; agora pelo `kevent`) |
| builders pelo executor nativo | todos | — | — |

* **ARC** (`docs/ARC-IMPLEMENTACAO.md`): o RC das ocorrências fortes entre objetos velhos, com as raízes
  adiadas e o berçário pela coleta menor; a gravação num velho conta na hora, o acesso cru tira uma foto;
  na completa, o ponto fixo global (§22.3: alcance pelas raízes com a regra condicional dos efêmeros, lote
  único dos inalcançáveis); *trial deletion* lateral com `DARTFORGE_ARC_CICLOS=sempre`; reclamação pelas
  marcas do RC. Grafos aleatórios com semente contra um oráculo independente (3000 sementes por modo); a
  primeira rodada achou dois vazamentos, corrigidos (o zero que volta a subir não virava candidato; o ciclo
  valor→portador de um efêmero ficava vivo pela chave). Opt-in por `--memoria=arc` no AOT e no JIT; o
  rastreamento continua o padrão, com o IR de sempre.
* **Exceções por tabelas no Linux**: a primeira execução (A1/B1) parou em 37 programas; o lançamento passava
  por uma função Rust com a guarda de abortar na variante `panic=unwind` do runtime. O `@df.lancar` chama o
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
* A CLI e o LSP publicam só os códigos de `crates/analise/verificados.txt` (167); 391 códigos já têm zero
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
| Casos s01–s12 do T5 (`corpus/especificacao/t2/t5`) | — | 12/12 iguais |

Oráculo do LSP (`crates/lsp/oraculo`, `dart language-server` 3.6.2 contra o `dartforge-lsp`,
projetos `app`, `args`, `path`, `string_scanner`; rodada `r6` de 2026-10-06, com as reprises por
método depois dela; a métrica do top-5 do completar passou a contar lista vazia contra lista vazia
como igual):

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
| assistências (refactor) | 26% | 98% |
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
  fora do `PATH`); `reload_estado` e `io_regressao` 7/7.
* **Etapa 1 (A1) verificada e arquivada** (§13.16): corpus/js 238/238, corpus/nativo igual a A0 com e
  sem `--gc-stress`; no `new_sali/backend` o `.text` não cai e as tabelas somam 1,57 MB (+3,2% contra a
  meta de −3%). O padrão segue `checagem` com o E1.1.
* **Etapa 2 (B0) verificada e arquivada** (§14.11): corpus B0 com e sem `--gc-stress` igual a A0, os 4
  testes dirigidos de `mapas_dirigidos` verdes com cada sabotagem pegando, produção B0 correta (ThinLTO
  distribuído, o passe depois da otimização); no mesmo pipeline o `.text` do `bench/desempenho` em B0
  é 25% maior, mais 100 KB de mapa. As raízes por mapas ficam experimentais.
* Defeitos achados ao rodar e consertados: pouso com `fake.use` fora dos mapas, chamada C variádica em
  função `gc`, conferidores do RS4GC e da sombra (apelidos de raiz, ajudantes, parâmetros), quadro morto
  e exceção que atravessa Rust sem porta (agora acusados), `__tmp_use` e RS4GC antes da LTO.
* CI: produção sem o Clang (N15: o gerador embutido e a LTO completa do `lld`), `__Unwind_GetGR` na
  `libSystem.tbd` do macOS, o placar consolidado do Pesado sem os relatórios dos modos e o job dos modos
  com o harness do gerador embutido.

**JS de produção (2026-10-06, `518e5388`).**
* `this` repetido como local (§8.4 de `JS-PRODUCAO-TAMANHO.md`): `new_sali` 12.361.194 → 12.082.759
  bytes, `limitless_ui` 6.946.316 → 6.693.718; corpus `--producao` 238/238, `limitless_ui` 26/26 no e2e,
  `new_sali` com o `fluxo.mjs` nos 11 passos sem erro.
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

* `scripts/comparar-placares.py` (dois placares, código a código) e
  `scripts/remover-funcao.py`.
* Oráculos em `E:\dftemp`: `cmp.py` (corpus contra `dartforge analyze`), `oraculo_lsp.py`,
  `oraculo_multi.py`, `oraculo_diag.py` (publicações de diagnóstico), `lsp\dif_metodo.py`
  (diferenças por amostra de um método do oráculo do LSP).
* Placar: `DARTFORGE_PARIDADE_AMOSTRAS=100000 ./target/release/dartforge-paridade placar --detalhes`;
  projetos reais: `dartforge-paridade projetos`.

---

## Fechamento do dia 2026-10-02

### O que entrou no `main` hoje

| commit | o quê |
| --- | --- |
| `f6d11d9e`, `5325e977`, `2d2571ab`, `c47214be` | JS de produção: SDK próprio como padrão, mundo fechado incremental, `oxc_minifier`, nomes de propriedade minificados, rti só para tipos testados, parâmetros nomeados com chave curta, `--omitir-checagens`/`-O4` opcional, `assert` desligado por padrão (`--enable-asserts`) |
| `40e33061`, `fbeded9b` | Nativo: HIR em paralelo e poda da HIR; SDK de produção em ThinLTO, LTO `-O1` no programa em partes, corpo de objeto `memory(none)`, trampolins da FFI só das assinaturas pedidas (3.591 → 44), literais constantes grandes por tabela |
| `36584c8e` | Parser: `part of` com experimento macros |
| `bc0f9c7d` | `docs/ANALYZER-ESPECIFICACAO.md` (436 códigos com perda, achados T1–T8) e `docs/LSP-ESPECIFICACAO.md` |
| este commit | `docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md` (chave `--raizes=sombra\|mapas` e `--excecoes=checagem\|tabelas`, etapas com critério de abandono) |

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
nos 209 pacotes do pub-cache (`scripts/paridade-pub-cache.py`). O LSP já
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

**Oráculo.** Regravado com o Dart 3.6.2 do Linux (`dartforge-paridade
oraculo`): os mesmos registros de antes, com os caminhos absolutos da raiz
trocados por `<raiz>` nas mensagens, para o oráculo não depender da
máquina. A regravação agora limpa o cache do `dart analyze` (o cache morno
omitia 7 registros) e mantém os 566 arquivos de `sintaxe-nova.json` quando
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
emita algo errado e lista os candidatos. `crates/analise/verificados.txt`
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
`scripts/hooks/commit-msg` e o job `mensagens` do CI). Os conflitos de
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
é o 3.13.4. O comando novo `dartforge-paridade sintaxe-nova` roda o 3.13.4
sobre os grupos 3.6 **sem mudar a versão do pacote**, escolhe pelo próprio
oráculo os arquivos em que ele acusa um recurso desconhecido do 3.6.2
(`dot-shorthands`, `primary-constructors`, `private-named-parameters`) e
troca só os registros deles: **498 em `analyzer` e 68 em `linguagem`**
(`corpus/diagnosticos/sintaxe-nova.json`). O `oraculo` respeita a lista ao
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
imprimir as amostras com `--detalhes`.

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
`emit_native/driver.rs`, `diferencial`, `scripts/env.ps1`).

**Suíte local inteira verde** (`cargo test --workspace`), com o ambiente
que o CI prepara: `scripts/env.ps1` (põe o `bin` do LLVM no `PATH` — sem
ele, os testes do JIT caem com `STATUS_DLL_NOT_FOUND` ao carregar
`LLVM-C.dll`), `scripts/gerar-dart-sdk.ps1` (`runtime/ddc/dart_sdk.js`, que
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
builders existe pela VM Dart (`pacotes/build_executor`, `crates/build/src/vm.rs`,
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
determinismo em 1/4/8. O código segue fora de `verificados.txt` (falta 100%).

## Fechamento do dia 2026-09-23

Resumo de uma página. O detalhe de cada frente está nas seções 1 e 2.
Trabalho não pronto para o `main` fica em ramos `wip/*`: cada um tem o último
commit do dia, com uma mensagem detalhada do que foi feito e do que falta.

### O que entrou no `main` hoje (CI e Pesado verdes antes de cada merge)

* **Macros** (8cf79cd):
  * augmentations nas duas formas (3.6.2 experimental e 3.13.4);
  * `macro class`, `import augment` e `augment library` no parser;
  * cadeia de augmentation no outline, generalizando o `@patch` do SDK;
  * a API de macros reescrita por nós em Dart puro (`pacotes/macros`);
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
* **Dart moderno P0–P5** (0acdf61): versões de linguagem 3.7–3.13 por biblioteca; dois SDKs de oráculo; `corpus/moderno` 22/26.
* **Nativo com async, RTI e `super` em mixin**: corpus nativo **91/223**, e o JIT também 91/223, sem divergência.
* **Portão de custo zero**: razões pareadas, confirmação em segunda passada, média com duas casas decimais e tolerância de 10% (decisão do proprietário).
* **`crates/runtime/README.md`**: o que falta no coletor de lixo.
  * weak refs e `Expando`, finalizers, heap por isolate;
  * geracional e compactação;
  * custo das raízes e pausas.
* **Ambiente**:
  * `scripts/ci.ps1 -Placar` baixa em `target/ci-placar` e apaga depois;
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
9. **Dart moderno**: `corpus/moderno` 26/26 (JS04 fechou 347 e 350–352); `PENDENTES` vazio.

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
| Inferência de corpos, fluxo, constantes | **funcional, com lacunas** (motor reescrito pela especificação, `crates/types/src/inferencia`) | 40/40 negativos do `analyzer`; medido contra o oráculo `package:analyzer` (`tools/oraculo_tipos`): `new_sali/core` 15 avisos e 31 de 634.368 expressões divergentes, `frontend` 28 avisos (17 deles em templates gerados, que o analyzer também acusa) e 56 de 1.259.011; SDK da fonte (nativo) 18 diagnósticos, os mesmos do analyzer; corpus de conformidade 83/95 programas iguais ao oráculo (ver §2.1) |
| Versão de linguagem por biblioteca e recursos 3.7–3.13 | **3.7–3.13 de sintaxe completos**; inferência e fluxo 3.7–3.13 (P6) pendentes | ver §1.1.1 |

### 1.1.1 Dart 3.7–3.13 — `docs/VERSOES-LINGUAGEM.md`

3.6.2 é o **piso**, não o teto. Cada biblioteca tem a sua versão de
linguagem (marcador `// @dart = x.y` > `languageVersion` do pacote > a
corrente, **3.13**), resolvida uma vez no carregamento e consultada como bits
(`LibraryFeatures`); `dart:*` fica no piso (D1). O harness tem **dois SDKs de
oráculo** (3.6.2 e 3.13.4, cada um com o seu `dartdevc` e o seu
`dart_sdk.js`); os programas dizem a versão que exigem (`// requer-dart:`).

| recurso | versão | `corpus/moderno` (VM e DDC 3.13.4; dev e produção) |
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
recurso precisou de runtime novo. O nativo não roda o `corpus/moderno`:
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
* **API de macros reescrita** (`pacotes/macros`, pacote `macros`, Dart puro):
  a superfície do `package:macros` 0.1.3-main.0 e o lado do executor
  (modelo, introspecção, builders com o texto do CFE, serviço `macro.*`). O
  `json.dart` do `package:json` 0.20.4, sem mudança, analisa contra ela com
  zero problemas no analyzer 3.6.2.
* **Hospedeiro** (`crates/macros_host`): detecção com custo zero, ordem do
  CFE, as três fases com recarga, modelo e consultas, montagem byte a byte,
  o serviço `macro.*` do `dfexec/1`, `Indisponivel` no produto.
  `dartforge macros --materializar` grava a augmentation
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
`scripts/gerar-dart-sdk.ps1`). O `dartdevc` é o oráculo do contrato.

* **Corpus diferencial: 213/213.** Cada programa é executado em `dart run
  --enable-asserts`, em `dartdevc`+Node e no DartForge+Node; stdout e
  código de saída comparados **byte a byte**. `cargo run -p
  dartforge-diferencial`.
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
  inclusive as 7 chamadas HTTP que a aplicação faz. `scripts/servir.ps1
  -Fluxo` (dirige o Edge por CDP, separa erro da aplicação de erro de
  ambiente).

Construtos cobertos: classes (construtores de todo tipo, `const`
canonicalizado, estáticos, operadores, mixins, enums com membros,
genéricas, `noSuchMethod`, `late`), coleções e literais tipados com rti,
exceções, `async`/`await`/`async*`/`sync*`/`await for`, `dynamic` por
`dsend`, records e padrões, extensions, `typedef`, bibliotecas múltiplas
com prefixos/`show`/`hide`/`part`, interop (`package:js`,
`dart:js_interop`, `@JSName`, `@anonymous`), `dart:html`.

### 1.2.1 Perfil de **produção** — `crates/emit_js_producao`

`dartforge-jsprod <entrada.dart> -o <saida.js>` escreve **um arquivo**, com
o runtime embutido e podado pelo mundo fechado. O plano, a referência
estudada e o que ele ainda não faz estão em `docs/JS-PRODUCAO.md`.

* **Corpus diferencial: 214/214**, com o **mesmo stdout da VM**, byte a
  byte — o mesmo placar do perfil de desenvolvimento.
  `cargo run --release -p dartforge-diferencial -- --producao` compara os
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
  `dartforge-diferencial determinismo --producao --trabalhadores 1,4,8`,
  que o backend nativo trouxe e que o perfil de produção passa.

Contra o oficial (`dart compile js -O4`, SDK 3.6.2), amostra do corpus por
`pwsh scripts/medir-js-producao.ps1`:

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

**Mundo fechado sobre a nossa trilha** (etapa 5, `crates/mundo`,
`docs/JS-PRODUCAO.md` §1.7). O código do usuário e dos pacotes é podado
**antes** da emissão por um RTA sobre o modelo de elementos: seletor por
nome, três níveis de classe e a fronteira com o `dart_sdk.js` por regra.
Um verificador sempre ligado confere o texto emitido e fecha o ponto fixo
mundo+texto. O modo stub (`--verificar-stub`) denuncia na execução qualquer
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
* `new_sali/frontend` com 11 passos sem erro no `fluxo.mjs`.

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
`crates/analise`, `crates/paridade`, `dartforge analyze`, LSP.** O
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
* **Verificadores sem tipo** (`crates/analise`): o
  `DuplicateDefinitionVerifier` e o `MemberDuplicateDefinitionVerifier`
  inteiros, a parte local do `UnusedLocalElementsVerifier` e o
  `ImportsVerifier` (`unused_import` e `unused_shown_name`, pelo lado seguro:
  112/153 no corpus, 0 FP nos projetos).
* **Regra de publicação** (`crates/analise/verificados.txt`, a mesma no CLI
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
  (`crates/paridade/tests/sonda.rs`).
  * Pela análise de hoje, 4 dos 7 batem: `non_bool_condition`,
    `not_assigned_potentially_non_nullable_local_variable`,
    `argument_type_not_assignable` e `unused_local_variable`.
  * Os outros 3 são do T1 (`types`): `undefined_function`,
    `return_of_invalid_type` (posição) e `unchecked_use_of_nullable_value`.
* **Placar no corpus** (`corpus/diagnosticos`, oráculo gravado em disco):
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
* **CI**: job `analise` do `pesado.yml`, contra o oráculo gravado (o runner
  não roda `dart analyze`), com o relatório idêntico em 1, 4 e 8
  trabalhadores (`determinismo`).
### 1.5 Backend nativo — `crates/emit_native` (feature `nativo`)

Trilha nova → HIR própria → LLVM IR → Clang → executável, com o runtime
Rust (GC por tracing). `dartforge compile-native` (compile com
`cargo build -p dartforge-cli --features nativo`); `dartforge aot` é o
apelido de produção do mesmo caminho.

**Rodada 2, P6 + RTI: corpus nativo 91/223 (Pesado 35904857766, CI
35904857783), JIT 91/223 sem divergência, os 91 também sob `--gc-stress`
(job novo do `pesado.yml`, que reprova se um programa só falha com
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
cargo build --release -p dartforge-diferencial
target\release\dartforge-diferencial.exe --nativo
```

Limites ligados por padrão, porque sem eles o corpus nativo em paralelo
tomou a memória da máquina: heap do runtime com teto de 256 MiB
(`DARTFORGE_HEAP_MAX_MB`), `--jobs 2` no modo nativo, `--limite-exec` de
5 s para executar o binário, e 4 MiB de saída capturada por processo.

O número saiu de 3/214 nesta sessão; o que mudou de fato foi a natureza do
gargalo — as falhas de "o Clang recusa o módulo" caíram de 172 para 13, e
o que sobra é programa que roda e imprime outra coisa. Ver
`docs/NATIVO.md` e `docs/NATIVO-PLANO.md`.

**Rodada 2, δ (NATIVO-PLANO §7.4):** strings com semântica UTF-16 na forma
da VM (`_OneByteString`/`_TwoByteString`) e `Ref` com `Smi` etiquetado
(R10) entraram sem regressão — nativo 50/223, JIT 50/223 com 0 divergências,
os mesmos 50 sob `--gc-stress` (Pesado 35849542217 e 35852784596). P5a: a
sobreposição `sdk_nativo/` carrega; os corpos das sete bibliotecas da fonte
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

**Determinismo verificado no corpus inteiro**: `dartforge-diferencial
determinismo --nativo [--trabalhadores 1,4,8]` emite o LLVM IR de cada
programa, sem Clang, ligação nem execução, e exige o mesmo resumo (FNV-1a
de 128 bits) — ou o mesmo erro — programa a programa. Passa com 1, 4 e 8
trabalhadores nos 214 programas (184 com IR, 30 com erro de carga), em
**0,2–0,5 s por passada**. `--executar` mantém o caminho antigo (compilar,
executar e comparar o relatório; o IR só com `DARTFORGE_KEEP_IR=1`). Ver §3.2.

**Cache de objeto**: o `.obj` de cada programa fica em
`native_cache/obj/` pela chave do IR + `clang --version` + bandeiras
(`crates/emit_native/src/cache_objeto.rs`); o mesmo IR dá o mesmo objeto
byte a byte. `compile-native --emit-ir` grava só o IR, `--resumo` imprime o
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
  `crates/cli/tests/reload_estado.rs`, no `ci.yml`). Edição que a recarga não
  sabe aplicar é recusada com o motivo e o programa reinicia; edição que não
  compila mantém a geração em execução. `--reiniciar` é o R0 (reinício a
  quente sem estado, um processo por geração).
  *Histórico (até 2026-09-27):* aqui se lia que `reload` era só o R0 e que
  «o estado NÃO é preservado».
* `dartforge-diferencial --jit` compara o JIT com a VM (mesmo placar do
  `--nativo`); `--jit-aot` passa o MESMO IR também pelo AOT, lista todo
  `JIT≠AOT` (é defeito) e mede os tempos. No CI é o job `jit` do `pesado.yml`.

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

* `crates/diferencial` — harness paralelo com cache dos oráculos;
  `corpus/js/` com 213 programas verificados na VM.
* `docs/CONTRATO-DDC.md` — Dart e JS do `dartdevc` lado a lado para os 212.
* Determinismo (`docs/PESQUISA-OTIMIZACAO.md` §11): `dartforge-diferencial
  determinismo [--nativo] [--trabalhadores 1,4,8]` exige relatório idêntico
  com qualquer número de trabalhadores; no nativo, o mesmo LLVM IR de cada
  programa — se a ordem de conclusão mudar o IR, o cache de objeto por hash
  erra e o Clang roda à toa. Verificado idêntico com 1, 4 e 8; no nativo,
  no corpus inteiro e sem precisar de `DARTFORGE_KEEP_IR` (§3.2).
* `scripts/` — `gerar-dart-sdk.ps1`, `servir.ps1`/`fluxo.mjs` (Edge por
  CDP), `limitless-ui.ps1` (`-Preparar`/`-Montar`/`-Servir`/`-E2e`),
  `medir-lsp.ps1`.
* Cache do outline do SDK em `target/dartforge/sdk-<hash>.bin` (5 ms para
  ler, contra ~105 ms de reanálise).
* **CI no GitHub Actions** (§3.0): `ci.yml` (rápido, todo push) e
  `pesado.yml` (corpus JS, produção, nativo em fragmentos, determinismo).
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
    Windows, mas o job `nativo-unix` do `ci.yml` (Ubuntu e macOS) compila a
    CLI com `--features nativo,jit` e roda os testes do `emit_native`, do
    `jit`, a recarga pela CLI, as regressões de `dart:io` e a distribuição
    num ambiente limpo (`.github/workflows/ci.yml:162-250`).
  * Sem exclusões de crate: a trilha velha, o `cranelift-jit` e o driver
    `native` saíram do workspace (§4), e com eles os 16 testes que falhavam
    e a falha registrada de `real_executable_prints_ints_and_bools`. Os
    quatro exemplos de memória têm nomes distintos (`memoria_<crate>`), e o
    CI roda `cargo test --workspace` com todos os alvos e os doctests.

### 1.7 `dartforge serve` e o gerador do ngdart

* `dartforge serve <entrada> -o <dir> [--web <dir>] [--porta N]` — servidor
  HTTP próprio, cliente de recarga injetado na resposta do `index.html` (o
  arquivo no disco não é tocado) e canal WebSocket (RFC 6455) como o
  auto-refresh do webdev. Protocolo versionado em JSON (`ola`,
  `recarregar`, `erro`), separado do transporte. Verificado no
  `new_sali/frontend` com Edge headless por CDP
  (`scripts/verificar-recarga.mjs`): editar um componente recarrega o
  navegador em **1,5 s**, contra **1m20s** do `build_runner` para a mesma
  edição.
* Fontes geradas em memória (`crates/elements/src/gerado.rs`): os
  `.template.dart` do ngdart podem vir de uma tabela indexada pelo caminho
  natural, sem passar pelo disco. Geração imutável e trocada inteira — o
  navegador nunca vê meio estado. `DARTFORGE_GERADOS=build_runner` prova o
  encanamento: 284 templates lidos da memória dão 616 módulos byte a byte
  iguais aos da compilação que lê do disco.
* `crates/gerador_ng` — o compilador do ngdart em Rust, com dois oráculos:
  o corpus próprio (`corpus/ngdart/`, uma forma por arquivo, com o
  `.template.dart` oficial ao lado) e os 945 arquivos que o `build_runner`
  gerou no `new_sali/frontend`
  (`cargo run -p dartforge-gerador-ng --example oraculo -- <projeto>`).

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

  `DARTFORGE_GERADOS=ng` compila com ele, fazendo antes uma carga de
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
quando está mais velho que a fonte; erro com `dartforge build --estrito`).

* **Plano igual ao oficial**: a forma canônica de `dartforge build --plano`
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
  sobre o `Program` da sessão, pela API pública do `gerador_ng`):
  `crates/build/tests/ng_transparencia.rs` — 60 saídas pelo motor iguais às
  do gerador chamado direto, 58 conferidas com o oráculo de `corpus/ngdart`,
  **0 diferentes**; incremental = do zero com edição de template e arquivo
  novo. No `new_sali/frontend`, `dartforge build --comparar` (placar contra o
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
  `build_runner`; o `emit_js` não depende mais do `gerador_ng`): 575 módulos,
  **574 byte a byte iguais** aos do `DARTFORGE_GERADOS=ng` de antes (579
  módulos); o do `limitless_ui` difere só por absorver os 4 `.css.dart` que o
  fluxo antigo lia do `.dart_tool/build/generated` (caminho fora do pub
  cache, então módulo próprio) e o motor publica no caminho natural, como os
  demais gerados do pacote.
* **Latência de edição no `new_sali/frontend`** (`scripts/medir-geracao.ps1`,
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
  `crates/dev/tests/custo_zero.rs` verde — num projeto sem `build_runner`,
  nenhum motor construído (`instancias() == 0`), relatório sem motor e a
  **mesma contagem de alocações** por edição que uma sessão construída sem
  etapas; portão de tempo no `pesado.yml` (job `custo-zero`, commit × main,
  5 rodadas alternadas, tolerância 3%), verde nas duas rodadas:
  35847625760 — corpus JS inteiro **11,36 × 11,56 s** (razão 1,017), edição
  de corpo numa sessão sintética de 300 bibliotecas **34 × 35 ms** (1,029);
  35850781174 (depois do merge do `main`) — **11,53 × 11,62 s** (1,008) e
  **35 × 34 ms** (0,971). O ruído entre rodadas do mesmo binário chegou a 14%
  (11,1–12,7 s): a mediana de 5 cabe nos 3%, mas por pouco; se o portão
  oscilar, a tolerância sobe com a medição registrada aqui. Local no
  `new_sali/core` (`scripts/medir-custo-zero.ps1`, média do platô de 20
  edições de corpo, 5 rodadas alternadas, máquina carregada por outros
  agentes): base **536 ms** × atual **465 ms** de mediana (0,87; faixas
  419–714 e 393–613 ms) — sem piora; os 227 ms do §1.3 são de máquina
  livre.

---

## 2. O que falta

### 2.0 O compilador de visões do ngdart

Especificação e resultado da rodada de 2026-09-30:
`docs/NGDART-COMPILADOR-DE-VISOES.md`. Medido pelo oráculo do
`build_runner` (cópias em `E:\dftemp\ngdart`, `oraculo --todos`: o pacote e
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
(`crates/gerador_ng/src/dialeto.rs`), e o motor registra o builder
`ngx_dart:ngx_dart` como nativo.

O Sass nativo ganhou o modo dart-sass 1.101.0–1.101.3 (os locks dos dois
projetos), e o emissor JS as baixas de interop do `package:web` que o
`ngx_dart` usa (construtor não-`external` de *extension type*,
`Function.toJS`, `isA<T>()`; casos 236–238 do `corpus/js`). Pelo motor
(`build --comparar`): new_sali 1561 saídas iguais / 0 / 0, limitless 937 /
0 / 0. `dartforge serve` em cópias sem `.dart_tool/build`: os dois abrem no
Edge headless com 0 erros no console e o CSS servido.

### 2.1 Correção (ordem de prioridade)

1. **Lacunas de inferência de tipos** (o `dart analyze` oficial dá 0
   diagnósticos nos projetos: todo aviso nosso é falso positivo). Medido
   em 2026-09-23 com o oráculo (`tools/oraculo_tipos`, método em
   `docs/FRONTEND-NEW-SALI.md`): `new_sali/core` 13.431 → **15** avisos,
   `frontend` 57.883 → **28** (17 em templates gerados, legítimos);
   divergências de tipo estático por expressão 137.428 → 31 (core) e
   316.756 → 56 (frontend); corpus `corpus/inferencia` 83/95 programas
   iguais ao oráculo (teste no CI). Os
   grupos restantes, por causa, estão em `docs/FRONTEND-NEW-SALI.md`.
   O contrato é `docs/INFERENCIA-ESPECIFICACAO.md` (regras do analyzer
   6.11 com arquivo:linha, e o que muda até 3.14), com o corpus de
   conformidade `corpus/inferencia/` (95 programas, tipos gravados pelo
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
com os programas `corpus/nativo/120`–`128`. O placar do pub roda no CI
(`scripts/pub-placar.py`, `.github/workflows/pub-placar.yml`).

**Estado atual.** O SDK da fonte é o padrão e o corpus nativo passa por
ele (`docs/NATIVO-PLANO.md` §7.10 e §7.13, que registram 225/225, e JIT × AOT
225/225); o último placar registrado neste arquivo é **183/223** no SDK da
fonte (rodada `ci/native-sdk-is-selector`, acima). `async`/`await` e laço de
eventos (`lower/async_sm.rs`), genéricos reificados (`lower/rti.rs`),
`dart:io` e isolados (`corpus/nativo/01`–`07`), `dart:ffi`
(`corpus/nativo/08`, `12`–`19`) e o `dart:core` da fonte existem. O que
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
relatório do harness (`--nativo`). A família de "handle" (71 de 214 no
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
pedidos ao `gerador_ng` em `docs/BUILD-PEDIDOS-GERADOR-NG.md` e é o que
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
* **`pesado.yml`** — push em `ci/**`, `workflow_dispatch` (suíte e número de
  fragmentos) e todo dia às 03:17 (Brasília) no `main`. Medido na rodada de
  2026-09-23 (`ci/infra`, 214 programas):

| job | o que roda | placar | tempo do job (harness) |
| --- | --- | --- | --- |
| compilar | release de `dartforge-diferencial`, `dartforge`, `dartforge-jsprod`; completa o cache dos oráculos (`verificar`) | — | 2,2 min |
| js (desenvolvimento) | `dartforge-diferencial --jobs 4` | **214/214** | 1,2 min (45 s) |
| js (produção) | `--producao --jobs 4` | **214/214** e 214/214 | 2,3 min (1 min 48 s) |
| nativo K/2 | `--nativo --jobs 4 --fragmento K/2` | 4/107 e 3/107 | 1,5 e 1,7 min (28 e 30 s) |
| nativo (placar consolidado) | soma os fragmentos e funde o agrupamento de falhas | **7/214** | 0,5 min |
| determinismo (produção) | `determinismo --producao --trabalhadores 1,4,8` | idêntico | 7,5 min (7 min) |
| determinismo (nativo, IR) | `determinismo --nativo --trabalhadores 1,4,8` | idêntico, 184 com IR | 0,6 min (5 s) |
| moderno (3.7–3.13) | `--corpus corpus/moderno --producao --jobs 4`, com o SDK 3.13.4 instalado por zip (cache pela versão); `PENDENTES` pode falhar, pendente que passa reprova | ver §1.1.1 | — |
| **rodada inteira** | | | **9,8 min** |

**Critério de cada job.** JS desenvolvimento e produção: código de saída do
harness 0, isto é, **todos os programas do corpus** passam — qualquer que
seja o tamanho do corpus, sem número fixo. Nativo: placar abaixo do total
é trabalho em andamento e não reprova; reprova se o harness quebrar
(código fora de {0, 1} ou relatório sem a linha de placar). Determinismo:
código 0. Cada job escreve placar, tempo, memória livre mínima e o
agrupamento de falhas no resumo da rodada (`$GITHUB_STEP_SUMMARY`), deixa
o placar numa anotação (aparece em `gh run view`) e sobe o relatório
completo como artefato `relatorio-<job>` (14 dias).

**Convenção: uma branch `ci/<frente>` por frente de trabalho** (`ci/nativo`,
`ci/ngdart`, `ci/verificacao`, `ci/infra`…). Empurrar para ela roda o
`pesado.yml` inteiro e o `ci.yml` sobre aquele commit, sem mexer no `main`.
Rodadas de branches diferentes **nunca se cancelam** (o grupo de
concorrência inclui a ref); uma rodada nova na **mesma** branch cancela a
anterior. As branches `ci/**` são descartáveis: o push é forçado.

```powershell
pwsh scripts/ci.ps1 -Frente nativo -Acompanhar   # HEAD -> ci/nativo, acompanha até o fim
pwsh scripts/ci.ps1 -Listar                      # última rodada de cada ci/** e do main, com placar
pwsh scripts/ci.ps1 -Placar <run-id>             # placar de cada job + relatórios baixados
pwsh scripts/ci.ps1 -Suite nativo -Ref ci/nativo -Fragmentos 4   # workflow_dispatch
gh run view <run-id> --log-failed                # o log do que falhou
gh run download <run-id> -p 'relatorio-*'        # relatórios completos
```

`-Suite` (workflow_dispatch) só funciona depois que o `pesado.yml` estiver
no `main`; até lá, `-Frente`.

**Limites do plano gratuito, e o dimensionamento.**

* **20 jobs simultâneos na conta inteira** (Linux e Windows juntos); o
  excedente espera na fila, não falha. Uma rodada `todos` com N fragmentos
  tem pico de 4 + N jobs: com o **N = 2 padrão**, 6 — cabem três frentes ao
  mesmo tempo. N sai da medição (§3.2): o corpus nativo inteiro custa ~1 min
  de harness, e cada job gasta ~1,3 min só preparando o ambiente. Subir N
  (`-Fragmentos`, ou `gh variable set FRAGMENTOS_NATIVO --body N`) quando um
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
pwsh scripts/gerar-dart-sdk.ps1              # dart_sdk.js (3 s, uma vez)
cargo build --release -p dartforge-cli -p dartforge-diferencial
cargo run --release -p dartforge-diferencial # 214/214

# projeto real
cargo run --release -p dartforge-cli -- compile-js `
  C:/MyDartProjects/new_sali/frontend/web/main.dart -o saida `
  --packages C:/MyDartProjects/new_sali/frontend/.dart_tool/package_config.json
pwsh scripts/servir.ps1 -Dir saida -Web C:/MyDartProjects/new_sali/frontend/web -Fluxo

# sessão residente
cargo run --release -p dartforge-cli -- dev <entrada.dart> -o saida --packages <cfg>
```

## 3.1 Espaço em disco — vigiar

O `target/` do Cargo **não se limpa sozinho**: num dia de trabalho com
vários agentes chegou a **38 GB** (21 GB em `debug/deps`, 8 GB de
compilação incremental), tudo recriável e nada de código. Rode

```powershell
pwsh scripts/limpar.ps1            # relata o que ocupa espaço
pwsh scripts/limpar.ps1 -Limpar    # apaga o lixo seguro (debug, target-*, incremental)
pwsh scripts/limpar.ps1 -Limpar -Tudo  # também release e o cache de oráculos
```

Custo de recriar: `target/debug` ~10 min, `target/diferencial` (cache dos
oráculos `dart run`) ~10 min, `target/release` ~5 min. Sem custo, e
apagados sempre: `target/diferencial/nativo` (executáveis e `.ll` do corpus
nativo; o harness já apaga cada `.exe` depois de executar, salvo
`DARTFORGE_KEEP_EXE`) e as `.lib` do runtime em `target/native_cache/` fora
as 2 mais recentes (o próprio cache também poda). Só com `-Tudo`:
`target/native_cache/obj`, o cache de objeto, que se poda sozinho no teto de
`DARTFORGE_CACHE_OBJ_MB` (256 MB). `DARTFORGE_CACHE_NATIVO` muda o
diretório; sem ele, `$CARGO_TARGET_DIR/native_cache`. Worktrees de
agentes têm cada uma o seu `target/` — removê-las (`git worktree remove`)
depois de integrar o trabalho é parte da limpeza.

**Regra dos temporários: no SSD E:, nunca no C:.** O C: tem pouco espaço e o
`%TEMP%` chegou a 10,5 GB de sobras. Temporários grandes vão para
`target/tmp-*` ou `target/scratch-*` do repositório (os scripts de
navegador, `medir-lsp.ps1`, `verificar-poda-js.ps1` e `ci.ps1 -Placar`
gravam lá); agentes e sessões longas definem antes
`$env:TEMP='<repo>\target\tmp-<frente>'; $env:TMP=$env:TEMP` e apagam a
pasta no fim. Todo teste ou comando que cria um temporário o apaga também
no caminho de erro, pânico ou tempo esgotado (`tempfile::tempdir`, ou um
guarda com `Drop` como `OutputDir` em `crates/jit/tests/execucao.rs` e
`DiretorioGeracoes` no `dartforge reload`). `limpar.ps1 -Limpar` apaga
`target/tmp-*`, `target/scratch-*` e as sobras antigas no `%TEMP%`
(`dartforge-*`, `dfserve*`, `df-recarga`, `lsp-stdout-*`).

### 3.2 Verificação rápida do backend nativo — medido

O corpus nativo foi registrado aqui como **10 programas em 9 minutos**
(~3 h por passada, ~10 h para o determinismo), e o determinismo só tinha
sido verificado num subconjunto. As três saídas propostas estão feitas,
seguindo `docs/PESQUISA-OTIMIZACAO.md` §11 (determinismo com 1, 4 e 8
trabalhadores) e §6 (resumo e cache por módulo):

1. **Determinismo não executa.** `dartforge-diferencial determinismo
   --nativo` emite o LLVM IR de cada programa dentro do processo
   (`emitir_ir`, sem Clang, ligação nem execução) e compara, programa a
   programa, o resumo FNV-1a de 128 bits — ou a mensagem inteira do erro.
   Divergência lista todos os programas e grava o primeiro, emitido sozinho
   e `n` vezes em paralelo, em `target/diferencial/determinismo/`.
   `--executar` mantém o caminho antigo. O relatório (uma linha
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
`DARTFORGE_IR_PARALELO_MAX` (padrão 2) limita as emissões físicas, e a
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
(`pesado.yml`, §3.0; runner Windows de 4 núcleos e 16 GB, `--jobs 4`,
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
  `emit_js` | `emit_native`, mais `emit_js_producao` e `mundo` (produção
  JS), `gerador_ng` (ngdart), `runtime` e `jit` (nativo), `abi`
  (`abi-info`), `dev`, `lsp`, `intern`, `diagnostics`, `instrument`,
  `diferencial` e `cli`. O workspace é `crates/*`, sem exclusões.
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
