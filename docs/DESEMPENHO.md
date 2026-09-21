# Desempenho de compilação — metodologia, linha de base e plano

Este documento existe porque nenhuma afirmação de desempenho vale sem três coisas:
a medição por fase, o contador de trabalho realizado e um teste que prove que o
reuso não mudou o resultado. Tudo aqui é reprodutível com
`cargo bench -p dartforge-compiler --bench incremental`.

Nada neste documento compara DartForge com DDC ou dart2js. Essa comparação exige
programas semanticamente equivalentes, com a mesma semântica de inteiros, casts,
genéricos reificados e null safety, e está registrada separadamente em
[BENCHMARKS.md](BENCHMARKS.md). Aqui o DartForge é comparado **consigo mesmo**.

## Os quatro cenários que orientam a arquitetura

| Cenário | Objetivo |
| --- | --- |
| Compilação inicial, sem cache | Poucas passagens, poucas alocações, paralelismo. |
| Nova execução, com cache em disco | Reaproveitar análise e artefatos sem reconstruir. |
| Edição com o compilador aberto | Recalcular só as unidades semanticamente afetadas. |
| Compilação otimizada de produção | Trocar tempo de compilação por tamanho e velocidade do JS. |

Hoje o DartForge atende bem o primeiro e parcialmente o segundo. O terceiro —
o que realmente importa para uma aplicação ngdart grande — ainda não existe: uma
edição de uma linha custa o mesmo que uma compilação fria.

## Instrumentação

`LinkStats` (crate `dartforge-linker`) cronometra cada fase **no próprio trecho**,
nunca por subtração, e conta o trabalho realizado: unidades, bytes de fonte,
tokens, classes, funções e bytes emitidos. `CompileReport` (crate
`dartforge-compiler`) acrescenta `load_ns` — descoberta do grafo, leitura das
fontes e resolução de pacotes — e o tempo total.

O crate `dartforge-instrument` fornece um alocador global contador (bytes vivos,
pico e número de alocações). É o único ponto do workspace que usa `unsafe`, num
`impl GlobalAlloc` de 40 linhas; todos os demais crates mantêm
`unsafe_code = "forbid"`. Ele é instalado apenas no binário de benchmark.

O número de alocações é tão importante quanto o tempo: ele não depende da carga
da máquina e denuncia trabalho quadrático que a mediana esconde.

## Corpus

24 bibliotecas independentes com 16 funções públicas cada, mais uma entrada que
importa todas e chama uma função de cada: 25 unidades, ~38 KB, ~14.300 tokens,
392 funções, ~425 linhas. Pequeno de propósito: o objetivo é detectar custo
**por unidade** e custo **quadrático**, não medir throughput de disco.

## Linha de base e o primeiro ganho

Medições na mesma máquina, perfil `bench` (release com símbolos), 20 amostras
após 3 aquecimentos.

### Antes: tabelas de símbolos clonadas por fluxo de controle

| Fase | Compilação fria |
| --- | --- |
| load (descoberta + leitura) | 2,03 ms |
| lex | 0,30 ms |
| outline | 0,07 ms |
| parse | 1,27 ms |
| macros | 0,32 ms |
| **análise semântica** | **14,57 ms** |
| emissão | 0,48 ms |
| **total (mediana)** | **21,04 ms** |
| alocações por compilação | 311.676 |

A análise consumia 69% do tempo e o compilador alocava cerca de **8 blocos por
byte de fonte**. Esse perfil é a assinatura de trabalho quadrático, não de um
parser lento: acelerar o parser em 10× reduziria o total em 6%.

### A causa

`Validator`, a estrutura da análise semântica, derivava `Clone` e era clonada
por inteiro a cada ramificação de fluxo: `if`, `while`, `for`, `&&`, `||`, cada
braço de `switch` e cada cascata. A estrutura carregava a tabela de **todas** as
classes (com os mapas de campos e métodos de cada uma), a tabela de **todas** as
funções de topo e a lista de extensions. Cada ramo do programa copiava o
programa inteiro: custo O(declarações × ramificações).

### A correção

Essas três tabelas são construídas antes da análise e nunca mudam depois. Elas
passaram a ser `Rc<...>`; a construção usa `Rc::make_mut`, que não copia nada
enquanto a contagem é 1. O que continua sendo copiado por fluxo é apenas o
estado que o fluxo realmente altera: escopos locais e promoções de tipo.

### Depois da primeira correção

| Fase | Compilação fria | Variação |
| --- | --- | --- |
| load (descoberta + leitura) | 2,17 ms | — |
| lex | ~0,30 ms | — |
| outline | ~0,07 ms | — |
| parse | 0,99 ms | — |
| macros | ~0,30 ms | — |
| **análise semântica** | **1,12 ms** | **13,0× mais rápida** |
| emissão | ~0,50 ms | — |
| **total (mediana)** | **6,04 ms** | **3,5× mais rápido** |
| p95 | 6,30 ms | |
| alocações por compilação | 16.411 | **19,0× menos** |
| pico de bytes vivos | 3,0 MiB | |

Nenhum teste mudou de resultado: a alteração é de representação, não de
semântica. `cargo test --workspace` continua verde.

## Segunda e terceira correções: paralelismo onde o trabalho é independente

Com a análise semântica corrigida, o perfil passou a ser dominado por syscalls e
por trabalho por unidade. A medição do interior do `load` mostrou onde:

| Operação | 25 arquivos |
| --- | --- |
| `read_to_string` de todos os arquivos | 0,92 ms |
| `canonicalize` de todos os arquivos | 0,79 ms |
| `load` completo | 2,11 ms |

81% da carga é syscall por arquivo. Duas mudanças, ambas preservando ordem e
determinismo:

1. **Front-end por unidade em paralelo** (`crates/linker`): tokenização, índice
   de declarações e análise sintática passaram a rodar em paralelo entre
   unidades, com limiar de 4 unidades. A coleta preserva a ordem das unidades e
   o diagnóstico relatado é sempre o da menor unidade que falhou — coletar
   diretamente num `Result` devolveria um erro arbitrário conforme o
   escalonamento das threads.
2. **Pré-busca paralela das importações** (`crates/packages`): resolver uma URI
   é aritmética de caminhos, então todos os alvos de uma unidade podem ser
   canonicalizados e lidos de uma vez. O laço sequencial continua decidindo tudo
   na ordem escrita; erros ficam guardados e são relatados no mesmo ponto.

Nenhuma das duas afrouxa a verificação por conteúdo exato do cache.

## Quarta correção: revalidação incremental do grafo

O acerto de cache gastava 1,15 ms dos seus 1,22 ms relendo e canonicalizando os
25 arquivos. A carga completa refaz a busca em largura: canonicaliza cada
candidato, relê tudo e reextrai as diretivas de cada arquivo.

`dartforge_packages::revalidate` reaproveita a **estrutura** do grafo quando só
os corpos mudaram:

1. Confere ambiente, entrada canônica e o conteúdo exato do
   `package_config.json` registrado no grafo.
2. Relê em paralelo todos os arquivos já conhecidos — pelos caminhos canônicos
   que o grafo guarda, sem canonicalizar de novo.
3. Arquivos byte a byte idênticos mantêm suas arestas.
4. Arquivo alterado tem as diretivas reextraídas e comparadas **estruturalmente**
   com as anteriores: mesmas URIs na mesma ordem, mesmos combinadores, mesmos
   prefixos, mesmos `part`, mesmo `part of`. Os spans e o fim do prefixo são
   adotados do texto novo, que é o que o front-end vai analisar.
5. Qualquer divergência devolve `None` e força a carga completa.

Comparar apenas o prefixo de diretivas seria incorreto: uma diretiva
acrescentada logo depois das anteriores deixa os primeiros bytes idênticos.
O teste `a_directive_appended_after_the_previous_prefix_is_detected` fixa isso.

A verificação continua sendo por conteúdo exato. Nenhum atalho por mtime ou
tamanho foi introduzido, e o teste que prova isso roda também pelo caminho novo.

| Cenário | Antes | Depois |
| --- | --- | --- |
| acerto de cache (mediana) | 1,22 ms | **0,40 ms** |
| acerto de cache (alocações) | 1.064 | **124** |

## Onde o ganho aparece para quem usa

Dois comandos expõem o trabalho acima:

```
dartforge compile entrada.dart saida.mjs --timings
dartforge watch   entrada.dart saida.mjs [--interval <ms>]
```

`--timings` imprime o relatório por fase e os contadores de trabalho em JSON,
para que qualquer afirmação de desempenho possa ser conferida na própria
máquina. `watch` mantém o compilador aberto — é o cenário "edição com o
compilador já aberto" — reaproveitando a estrutura do grafo entre
recompilações e reescrevendo a saída apenas quando o JavaScript muda.

A detecção continua sendo por releitura e comparação de conteúdo, nunca por
mtime ou tamanho; por isso `watch` sonda em intervalo em vez de observar
eventos do sistema de arquivos. Trocar a sondagem por um observador nativo é
uma otimização legítima, mas não pode substituir a verificação por conteúdo.

## Perfil atual, por cenário

| Cenário | Mediana | p95 | Alocações | Observação |
| --- | --- | --- | --- | --- |
| frio (sem sessão) | 5,31–5,65 ms | 8,71 ms | 16.714 | linha de base |
| sem edição (acerto de cache) | **0,40 ms** | 0,43 ms | **124** | revalidação incremental do grafo |
| edição de comentário | 5,68 ms | 7,12 ms | 16.744 | recompila tudo |
| edição de corpo | 5,52 ms | 6,32 ms | 16.742 | recompila tudo |
| edição de assinatura | 5,12 ms | 6,19 ms | 16.755 | recompila tudo |
| edição de constante | 4,79 ms | 5,79 ms | 16.744 | recompila tudo |
| edição de import | 5,40 ms | 6,29 ms | 16.432 | recompila tudo |

Acumulado desta rodada: compilação fria **21,04 ms → 5,17 ms (4,1×)**, acerto de
cache **1,49 ms → 0,40 ms (3,7×)**, alocações por compilação fria
**311.676 → 16.716 (18,6×)** e por acerto de cache **756 → 124**.

Perfil por fase da compilação fria, com todas as fases cronometradas no próprio
trecho:

| Fase | Tempo | Fração |
| --- | --- | --- |
| load (descoberta + leitura) | 1,21 ms | 23% |
| análise semântica | 1,21 ms | 23% |
| parse | 0,57 ms | 11% |
| emissão | 0,52 ms | 10% |
| namespaces/exports | 0,37 ms | 7% |
| lex | 0,31 ms | 6% |
| macros | 0,30 ms | 6% |
| combinação das unidades | 0,05 ms | 1% |
| não atribuído | 0,73 ms | 14% |

O perfil deixou de ter um gargalo isolado. Isso muda a conclusão: **o próximo
ganho relevante não vem de acelerar uma fase, e sim de não executar fase
nenhuma para as unidades que não mudaram.**

Duas leituras diretas destes números:

1. **Não há incrementalidade.** Trocar um comentário custa o mesmo que compilar
   do zero. O cache atual é tudo-ou-nada: ou o grafo inteiro é byte a byte
   idêntico, ou todo o trabalho é refeito.
2. **A leitura do grafo dominava o caminho quente** e foi corrigida pela
   revalidação incremental. O que resta no caminho quente é a releitura dos
   arquivos, obrigatória para a verificação por conteúdo exato.

O acerto de cache não usa mtime nem tamanho por decisão explícita: o teste
`dependency_change_is_detected_even_with_same_size_and_mtime` prova que uma
edição com mtime restaurado e tamanho idêntico ainda invalida o cache. Acelerar
a leitura não pode passar por afrouxar essa garantia.

## Próximos passos, em ordem de impacto medido

1. **Incrementalidade do front-end.** Hoje o reuso para na estrutura do grafo:
   lex, parse, análise e emissão são refeitos para as 25 unidades mesmo quando
   uma mudou. O obstáculo é concreto e conhecido: a AST empresta `&str` do texto
   da fonte, então nenhum resultado sobrevive entre solicitações sem que a
   sessão seja dona do texto. A saída é o *interning* de nomes com `SymbolId` e
   arenas por unidade, que elimina o empréstimo e habilita cache por unidade.
   É um trabalho de fôlego e vem antes de qualquer outra otimização.
2. **Incrementalidade semântica.** Separar o resumo da biblioteca (declarações,
   assinaturas, supertipos, membros e constantes observáveis) dos corpos, com
   impressão digital estável por declaração. Uma edição que não muda o resumo
   não pode invalidar a análise de quem depende dela. É o único item capaz de
   tornar a edição de um corpo O(1) em vez de O(projeto).
3. **Emissão modular.** Um módulo ESM por biblioteca, com nomes estáveis e texto
   em cache por biblioteca, para que uma edição reemita um módulo e não 78 KB.
4. **Leitura do grafo por níveis, em paralelo.** A descoberta é sequencial por
   natureza, mas cada nível da busca em largura é independente.
5. **Interning de nomes e tipos.** As 16.411 alocações restantes ainda são ~2,5
   por token; nomes repetidos (`int`, `Future`, nomes de classes) merecem
   `SymbolId`.
6. **Paralelismo por unidade** no front-end (lex, outline, parse), depois de (1),
   porque paralelizar trabalho que não deveria ser refeito é otimizar o desperdício.

Cada item só entra depois de aparecer no relatório por fase. A ordem acima é a
ordem dos números medidos, não uma lista de preferências.

## Teste de validade do cache

A propriedade central é: **o resultado observável de uma compilação incremental
é idêntico ao de uma compilação limpa das mesmas entradas.** Os testes de
`crates/compiler/src/session.rs` já cobrem edição com mtime e tamanho
preservados, troca de aresta de import, troca de opção de otimização,
remapeamento de `package_config.json`, erro seguido de recuperação e orçamento
de retenção. Qualquer mecanismo novo de reuso precisa entrar nessa suíte antes
de ser considerado pronto — e a suíte precisa crescer para sequências de
edições, exclusões e renomeações, não apenas edições isoladas.
