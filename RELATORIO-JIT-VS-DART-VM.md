# JIT do DartForge × Dart VM — 07/11/2026

A avaliação no **Mac M4/arm64, 10 CPUs e 16 GiB**, macOS 27.2,
mostra que o JIT executa corretamente os programas selecionados, usou menos
memória na sessão de alocação com SDK preparado e permanece mais lento
que a Dart VM. O preparo inicial do SDK tem seu custo de memória separado.
Nos 17 kernels, os tempos posteriores ao aquecimento foram de **1,36× a
13,21×** os da VM 3.6.2; a mediana das 17 razões foi **2,60×**. Essa
mediana descreve este conjunto de programas; não estima qualquer aplicação.
Chamadas virtuais e dados tipados mistos são as primeiras prioridades de
perfilamento. A primeira saída do `hello` teve mediana de **295 ms**, contra
**77 ms** na VM 3.6.2. O preparo inicial do SDK, com cache nativo vazio,
elevou a mediana do JIT para **12,17 s**.

Este relatório reúne a matriz inicial de desempenho do JIT v12 em duas
máquinas, com 657 processos medidos em cada uma. A liberação geral
para produção continua pendente. A suíte completa da linguagem no JIT,
HTTP e aplicações reais com geração de código não foram cobertos por
estas coortes. A mesma matriz de medição também terminou no Windows e
está apresentada separadamente abaixo. O Windows é uma máquina física Samsung
com i3-1215U e cerca de 7,71 GiB. Sua validação completa da v12 terminou
com uma falha por corpus externo ausente; o executável completo do corpus
passou em seis testes após fornecer os mesmos arquivos do Mac. Ambos os
resultados estão preservados. No Windows, passaram os 57 processos
iniciais dos 19 programas e outros 57 de repetições com cronômetros
desligados, com VM 3.6.2 principal e VM 3.13.4 secundária. O protocolo
de memória passou nas três execuções, com 61 pausas reais por runtime.
A matriz Windows também foi concluída; os resultados e seus limites estão
na seção própria abaixo.

## Perfil, isolamento e validação

O executável preservado foi compilado em release com `native,jit`, Rust
1.98.1 e LLVM 22.1.8. O JIT usa tracing, raízes padrão, linguagem 3.6 e o
perfil de desenvolvimento, com asserts ligados. Ambas as VMs foram
executadas com `--enable-asserts` e a mesma configuração de pacotes de
linguagem 3.6. A referência principal é a **VM 3.6.2**; a **VM 3.13.5** é
uma referência secundária apresentada em coluna própria.

Todos os executores receberam os mesmos bytes de programa e as mesmas
entradas. Os módulos de `bench/performance/` e o `hello` original foram
copiados sem alterar os bytes; wrappers compartilhados verificaram os
checksums. Houve 57 processos de correção inicial, mais 57 de validação
das repetições sem cronômetros e 19 do JIT com o cache relocado. As 42
entradas do cache relocado permaneceram idênticas. A calibração e os
controles do coletor terminaram antes das coortes.

As medições foram sequenciais e alternaram a ordem dos executores em
quadrado latino. Não houve build, teste, oráculo ou outro benchmark no Mac
durante as coortes. A política v2 registra explicitamente que a validação
na outra máquina física Windows podia continuar: ela não compartilha
CPU, RAM ou disco com este Mac. O plano anterior, que esperava ambos os
hosts, foi preservado. Não se misturaram resultados dos dois hosts.

| Coorte | Processos | Verificação |
| --- | ---: | --- |
| Início e processo completo: 18 programas × 9 rodadas × 3 executores | 486 | Saída bruta e código de saída |
| Execução: 17 kernels × 3 sessões × 3 executores | 153 | Primeiro kernel e nove posteriores, mais cinco aquecimentos; checksum em cada chamada |
| Cache nativo do SDK vazio: 1 programa × 3 rodadas × 3 executores | 9 | Saída bruta; os três processos JIT produziram o conjunto completo de 42 artefatos do SDK |
| Memória: 3 sessões × 3 executores, com 30 ciclos por sessão | 9 | 549 snapshots em barreiras e 540 janelas de CPU; saídas brutas idênticas |

Os **657 processos de medição** e seus verificadores terminaram com código
0. Os tempos individuais, falhas dos ensaios preparatórios e todos os
stdout/stderr relevantes foram preservados. Não houve descarte de amostras
nem reinício para substituir resultados desfavoráveis. As guardas do código
v12, binários, entradas e índice Git passaram antes/depois das coortes.

## Tempo inicial e processo completo

A primeira saída é observada pelo host desde a criação do processo. Nos
kernels ela é `READY`, antes do trabalho; no `hello` é a saída original,
sem adicionar imports, cronômetro ou barreira ao programa. Inclui loader,
frontend, emissão e geração de código quando aplicáveis. O processo
completo inclui também o encerramento. São medianas de nove processos
novos, com artefatos nativos do SDK reutilizados.

| `hello` | JIT | VM 3.6.2 | VM 3.13.5 |
| --- | ---: | ---: | ---: |
| Primeira saída | 295,23 ms | 77,24 ms | 79,46 ms |
| Processo completo | 298,36 ms | 90,66 ms | 91,54 ms |
| CPU total da invocação | 293,23 ms | 141,98 ms | 124,31 ms |
| Pico RSS nativo da linhagem¹ | 115,03 MiB | 155,91 MiB | 172,38 MiB |

Intervalos observados da primeira saída: JIT **285,84–776,85 ms**; VM 3.6.2 **74,85–78,38 ms**; VM 3.13.5 **76,52–79,97 ms**. Todas as nove amostras, incluindo a amostra JIT de cerca de 777 ms, entraram na análise. Não há estimativa de P95.

O JIT levou **3,82×** o tempo da VM 3.6.2 até a primeira saída nesse
programa. O arquivo `summary-v1.json` contém também os tempos iniciais e
completos dos outros 17 programas; o resultado do `hello` não os representa.

### Primeiro preparo do SDK

Nesta coorte, cada invocação JIT começou com um diretório próprio de cache
nativo inexistente. Ao terminar, os 42 artefatos esperados estavam
presentes, incluindo a biblioteca dinâmica e o catálogo de IDs. A medição
inclui essa construção. As VMs executaram o mesmo programa como controles
pareados; seus SDKs distribuídos já fornecem os artefatos da própria VM.
É cache de artefatos vazio, **sem alegação de cache de páginas do sistema
operacional frio**. São três processos por executor.

| `hello`, preparo inicial | JIT | VM 3.6.2 | VM 3.13.5 |
| --- | ---: | ---: | ---: |
| Primeira saída | 12166,06 ms | 78,20 ms | 87,00 ms |
| Processo completo | 12182,79 ms | 91,88 ms | 100,52 ms |
| CPU total com filhos aguardados | 26668,73 ms | 144,80 ms | 128,03 ms |
| Pico RSS nativo da linhagem¹ | 1615,63 MiB | 156,78 MiB | 172,42 MiB |

O primeiro preparo chegou a **1,58 GiB** no máximo RSS mediano informado
para a linhagem. Esse valor caracteriza o preparo/compilação do SDK e
precisa ser considerado na instalação e na invalidação desse cache. Ele
não é o consumo em repouso do aplicativo. O pico somado de toda a árvore
de processos durante essa compilação não foi medido.

## Execução após aquecimento

Cada sessão mede uma primeira chamada, executa cinco chamadas adicionais
de aquecimento e mede nove posteriores. Cada chamada verifica o checksum.
A tabela usa a mediana das nove medições em cada sessão e depois a mediana
das três sessões. Inclui o trabalho e as alocações dentro do kernel, além
da chamada assíncrona compartilhada pelo wrapper. São tempos do programa;
não são `execute_ns` do compilador comparados ao tempo total da VM.

| Kernel | JIT ms | VM 3.6.2 ms | VM 3.13.5 ms | JIT / VM 3.6.2 |
| --- | ---: | ---: | ---: | ---: |
| `mandelbrot` | 24,537 | 8,346 | 8,601 | 2,94× |
| `collatz` | 100,356 | 40,682 | 40,400 | 2,47× |
| `virtual_calls` | 17,259 | 1,307 | 1,303 | 13,21× |
| `closures` | 55,459 | 20,062 | 19,713 | 2,76× |
| `recursion` | 7,853 | 4,404 | 4,421 | 1,78× |
| `list_growth` | 6,317 | 4,652 | 4,806 | 1,36× |
| `list_sort` | 338,454 | 159,088 | 160,848 | 2,13× |
| `integer_map` | 41,498 | 19,637 | 19,878 | 2,11× |
| `string_set` | 14,614 | 10,206 | 10,306 | 1,43× |
| `string_build` | 41,193 | 26,616 | 27,828 | 1,55× |
| `temporary_objects` | 15,718 | 5,319 | 5,834 | 2,96× |
| `escaping_objects` | 46,790 | 17,498 | 16,959 | 2,67× |
| `objects_in_maps` | 12,369 | 4,752 | 5,078 | 2,60× |
| `typed_bytes` | 37,784 | 13,800 | 13,618 | 2,74× |
| `typed_mixed` | 38,291 | 6,729 | 6,822 | 5,69× |
| `json_roundtrip` | 7,153 | 2,861 | 2,714 | 2,50× |
| `async_await` | 873,655 | 302,286 | 256,678 | 2,89× |

O JIT ficou mais lento nos 17 casos. `virtual_calls` (**13,21×**) e
`typed_mixed` (**5,69×**) justificam investigar despacho e acesso a dados
tipados primeiro. Esses testes identificam sintomas; não isolam a causa.

As rodadas não provam regime estável em todos os casos. Em `objects_in_maps`
na VM 3.6.2, a mediana das rodadas 7–9 foi **6,283 ms**, contra **3,575 ms**
nas rodadas 1–3, cerca de 76% maior. Os dados retêm essa variação; o caso
precisa de uma sessão mais longa para separar aquecimento, GC e outros
efeitos. Os intervalos de cada rodada, medianas por sessão, primeiro kernel
e pico RSS por programa estão no resumo e nos dados brutos. Não há conclusão
de cauda ou projeção de throughput de servidor.

## Memória em sessão longa

O programa inicia com uma barreira antes das alocações. Cada um dos 30
ciclos retém 1.024 objetos com 8 MiB de payload lógico em `Uint8List`, toca
os dados, verifica o checksum após uma barreira, limpa a coleção e executa
pressão de alocação igual nos três executores. Há pausa de 100 ms antes de
cada snapshot. Não se solicita coleta forçada específica de um backend.

Os valores abaixo são medianas de três sessões. **Footprint físico e RSS
são contadores distintos**; cada linha compara a mesma métrica. O tempo
total desse protocolo inclui 61 pausas do host e não mede throughput.

| Métrica / fase | JIT MiB | VM 3.6.2 MiB | VM 3.13.5 MiB |
| --- | ---: | ---: | ---: |
| Footprint antes das alocações | 90,66 | 108,16 | 121,95 |
| Footprint na primeira retenção | 99,86 | 116,77 | 130,33 |
| Footprint após primeira liberação/pressão | 105,33 | 142,81 | 156,64 |
| Footprint na 30ª retenção | 100,56 | 158,88 | 173,02 |
| Footprint após 30ª liberação/pressão | 105,58 | 158,88 | 173,02 |
| RSS antes das alocações | 128,02 | 149,19 | 160,11 |
| RSS após 30ª liberação/pressão | 143,28 | 199,91 | 211,20 |
| Pico amostrado de footprint² | 108,11 | 158,92 | 173,03 |
| Pico amostrado de RSS² | 145,81 | 199,95 | 211,22 |
| Pico nativo vitalício de footprint do processo | 108,20 | 158,94 | 173,05 |

O JIT usou menos memória nesse ensaio; seu footprint final foi cerca de
34% menor que o da VM 3.6.2. Os picos RSS das sessões dos 17 kernels também
foram menores no JIT neste conjunto, com os valores completos no resumo.
Os 30 ciclos não demonstram ausência de vazamentos em qualquer aplicação
ou sessão mais longa. Capacidade de heap retida pelo alocador e páginas
não devolvidas ao sistema não equivalem, por si, a objetos ainda vivos.
O primeiro preparo do SDK tem o custo de memória separado mostrado acima.

¹ A calibração real verificou a unidade em bytes e CPU de filhos/netos
aguardados. Neste macOS, `wait4.ru_maxrss` representa o **máximo entre os
processos da linhagem aguardada**. Não se rotulou esse contador como RSS
exclusivo do pai nem como soma da árvore. CPU total pode exceder tempo de
parede por incluir threads e filhos paralelos. ² Picos por amostragem são
limites inferiores; o intervalo periódico do coletor é 25 ms. Durante o
protocolo após `READY`, os nove alvos não tinham filhos vivos, e o coletor
foi excluído dos contadores. O pico nativo de footprint cobre a vida do
processo até a última barreira, não apenas uma fase.

## O que investigar a seguir

O código atual de `dartforge run` emite IR com
[`optimize: false`](crates/cli/src/jit.rs#L139). No perfil de raízes padrão,
a máquina-alvo LLVM usa
[`LLVMCodeGenLevelNone`](crates/jit/src/ffi.rs#L1259). Isso sustenta uma
hipótese de trabalho para parte da distância de desempenho; esta avaliação
não mede causalmente o ganho de trocar essas opções. Qualquer alteração
precisa preservar as garantias do GC e passar pelos mesmos controles.

As próximas frentes são perfilar chamadas virtuais e dados tipados mistos;
isolar frontend/emissão/carregamento no custo inicial; avaliar distribuição
ou reutilização dos artefatos do SDK; ampliar sessões de alocação/GC; medir
um aplicativo real e ampliar as sessões no Mac e no Windows. Uma coorte diagnóstica
com `--timings` poderá separar fases, mantendo-a fora das amostras primárias.
A suíte completa da linguagem sob JIT continua necessária para prontidão de
produção. Esta rodada não alterou o compilador para obter estes números.

## Diagnóstico adicional do início no Mac

Depois das coortes primárias, foram executados **12 processos separados**
com `--timings`: três para cada um de `hello`, chamadas virtuais, dados
tipados mistos e async/await. Os programas mantiveram seus bytes e usaram
`check`, com um kernel por processo. As saídas completas coincidiram com
a referência oficial preservada, os códigos foram 0 e as guardas de
fontes, índice, SDK e bytes dos 42 artefatos do cache passaram. Essa
rodada serve para localizar custos, sem substituir a comparação principal
ou medir execução após aquecimento. No `hello`, a mediana do frontend
foi **132,29 ms** e a da sessão JIT completa, **35,05 ms**. A primeira
observação até a saída foi de **938,88 ms**, com **645,59 ms** na criação
da sessão; ela foi mantida, junto às demais de 277,04 e 278,28 ms. A causa
desse primeiro custo maior não foi isolada.

A instrumentação v15 acrescenta `prepare_ir_ns`, `sdk_library_ns` e
`module_total_ns` ao JSON, preservando os campos anteriores. O primeiro
cobre o preparo completo do IR; o segundo, a seleção/verificação ou
compilação dos artefatos nativos do SDK; o terceiro inclui toda a
incorporação do módulo, além das duas fases internas antes expostas.
O registro de callbacks pode materializar código nessa incorporação.
O preparo e a seleção dos artefatos precedem `jit_total_ns`.
A documentação em [docs/JIT.md](docs/JIT.md#medição-por-fase) agora explica
os intervalos inclusivos e a fronteira do cronômetro externo.

O release v15 compilou e outros **12 processos isolados** passaram nos
mesmos programas, com saídas completas iguais às oficiais e verificação
independente dos intervalos. As medianas dos três processos `hello` foram:

| Intervalo | Mediana |
| --- | ---: |
| Preparo completo do IR | 157,83 ms |
| Frontend, interno ao preparo | 138,81 ms |
| Seleção/verificação dos artefatos nativos do SDK | 80,01 ms |
| Sessão JIT completa, incluindo execução e destruição | 36,13 ms |
| Criação da sessão, interna ao total JIT | 23,34 ms |
| Incorporação completa do módulo, interna ao total JIT | 10,31 ms |

Os intervalos internos não se somam novamente aos totais. Não se somam
medianas para reconstruir uma invocação. As primeiras saídas foram
**1.369,11 / 289,14 / 291,37 ms**, sem descartar a primeira. Há apenas
três processos por programa; isso não demonstra estabilidade nem ganho
ou regressão frente ao binário anterior. Os 80 ms identificam uma
frente de investigação do cache já preparado, sem provar qual operação
interna é responsável nem antecipar ganho de uma alteração.

Passaram também **seis controles reais de `--ir`**, com e sem `--timings`:
entrada válida, IR inválido e arquivo ausente. O programa válido preservou
stdout vazio; os dois erros conservaram integralmente stdout, stderr e
código de saída, sem emitir um JSON de sucesso. Não se usaram os tempos
desses controles como desempenho, pois a compilação dos testes já estava
em andamento. A v15 depois concluiu os **121 testes/exemplos do
CLI `native,jit` em 20 alvos**, sem falhas nem ignorados, além de
formatação, Clippy rigoroso, pré-requisitos e documentação privada do CLI
sem avisos. A suíte completa do workspace permanece vinculada à v13;
não se repetiu essa suíte inteira para os novos campos de diagnóstico.
Os logs finais e o executável estão em
`/Volumes/ssd_dock/tmp/dartforge-production-v15-jit-timings-2026-11-07-evidence/`.

As evidências desta etapa estão em
`/Volumes/ssd_dock/tmp/dartforge-jit-phase-diagnostic-2026-11-07-evidence/`.
A primeira adaptação parou antes de executar um alvo por referenciar um
helper ausente na pasta v14; a versão seguinte usou o helper preservado
v13 e as guardas de fonte/índice v14. Essa falha foi preservada. No resumo
derivado inicial v2, os rótulos herdados `*_ns` receberam valores em ms;
os campos das 12 linhas brutas continuam em ns. O verificador preserva
esse aviso e fornece um resumo separado com chaves `*Ms`. A versão v3
usa `summaryMs` com rótulos e unidades conferidos. Nenhuma linha bruta
foi alterada.

## Medições no Windows — 07/11/2026

A mesma matriz inicial foi concluída na máquina física Samsung com
**i3-1215U, 6 núcleos/8 processadores lógicos e cerca de 7,71 GiB**. São
**657 processos nas quatro coortes completas**: 486 de início, 153 de
execução, nove de primeiro preparo do SDK e nove de memória. Os três
processos da tentativa inicial interrompida também foram preservados,
separadamente das coortes completas. Coletores e os quatro verificadores
independentes terminaram em código 0. A referência principal é a VM
**3.6.2** e a secundária a **3.13.4**. Esses resultados permanecem separados
dos obtidos no Mac.

Os mesmos bytes dos programas e módulos do Mac foram usados, com pacotes
de linguagem 3.6, JIT release/development/tracing e asserts ligados nos
três executores. A ordem foi balanceada e sequencial. O inventário inicial
de cada coorte confirmou ausência de outros builds/testes/oráculos ou
benchmarks nesse Windows; o trabalho na outra máquina Mac podia continuar.
O plano de energia registrado foi **SAMSUNG MODE**, sem alteração pelo
coletor. O inventário preserva o status bruto da bateria e a configuração
do host; ele não identifica a causa das variações de velocidade.

### Início e primeiro preparo do SDK

Medianas de nove processos por executor, com cache nativo do SDK
reutilizado. O `hello` conserva seus bytes originais e não recebe
`READY`, imports ou cronômetros adicionais. O tempo inclui o lançamento,
a criação suspensa/atribuição ao job/retomada e a observação do pipe, sem
subtrair a instrumentação.

| `hello`, SDK preparado | JIT | VM 3.6.2 | VM 3.13.4 |
| --- | ---: | ---: | ---: |
| Primeira saída, ms | 399,80 | 151,71 | 158,64 |
| Processo e descendentes encerrados, ms | 406,87 | 173,02 | 174,61 |
| CPU total do job, ms | 390,62 | 265,62 | 234,38 |

A primeira saída do JIT levou **2,64×** o tempo da VM 3.6.2 nesse programa.
Intervalos das nove amostras: JIT **393,30–525,50 ms**; VM 3.6.2
**137,89–155,24 ms**; VM 3.13.4 **145,50–167,79 ms**. Nenhuma amostra foi
excluída. Os outros 17 programas estão discriminados no resumo bruto.

Com o cache nativo inicialmente inexistente, as três invocações JIT
produziram os **52 arquivos esperados** do SDK Windows. A mediana até a
primeira saída foi **42.339,37 ms**, com processo completo de **42.481,63 ms**
e CPU total do job de **84.171,88 ms**, incluindo compiladores filhos. As
VMs pareadas chegaram à primeira saída em 154,07 e 171,71 ms; seus SDKs
distribuídos já fornecem os artefatos próprios. Não se tratou de cache de
páginas do sistema frio. O pico de RAM durante esse preparo Windows não
foi medido; o máximo RSS de 1,58 GiB informado anteriormente pertence ao
ensaio Mac e mantém seu escopo separado.

### Execução após as repetições de aquecimento

São três sessões por programa e executor: primeira chamada, cinco
aquecimentos adicionais e nove chamadas posteriores, com checksum em
todas. Cada célula usa a mediana por sessão e depois a mediana das três
sessões. A mediana das 17 razões JIT/VM 3.6.2 foi **1,98×**. O JIT ficou
mais lento em 16 casos; `string_build` ficou próximo do empate, com
razão 0,98×. As razões descrevem este protocolo, sem representar uma
aplicação nem demonstrar regime estável.

| Kernel | JIT ms | VM 3.6.2 ms | VM 3.13.4 ms | JIT / VM 3.6.2 |
| --- | ---: | ---: | ---: | ---: |
| `mandelbrot` | 21,571 | 9,416 | 10,112 | 2,29× |
| `collatz` | 105,504 | 62,428 | 66,610 | 1,69× |
| `virtual_calls` | 23,036 | 1,391 | 1,395 | 16,56× |
| `closures` | 83,058 | 9,918 | 10,331 | 8,37× |
| `recursion` | 11,335 | 4,808 | 5,103 | 2,36× |
| `list_growth` | 11,020 | 8,697 | 9,310 | 1,27× |
| `list_sort` | 1429,005 | 237,328 | 234,740 | 6,02× |
| `integer_map` | 59,415 | 42,882 | 43,812 | 1,39× |
| `string_set` | 18,214 | 13,016 | 13,711 | 1,40× |
| `string_build` | 55,632 | 56,635 | 58,680 | 0,98× |
| `temporary_objects` | 17,611 | 9,429 | 7,393 | 1,87× |
| `escaping_objects` | 52,672 | 32,564 | 31,188 | 1,62× |
| `objects_in_maps` | 17,519 | 8,852 | 10,542 | 1,98× |
| `typed_bytes` | 28,065 | 14,514 | 14,547 | 1,93× |
| `typed_mixed` | 42,481 | 6,152 | 7,260 | 6,91× |
| `json_roundtrip` | 9,041 | 4,063 | 3,900 | 2,23× |
| `async_await` | 4046,393 | 254,619 | 215,759 | 15,89× |

**Há variação grande dentro de algumas sessões.** No `list_sort` da VM
3.13.4, sessão 2, a mediana das últimas três chamadas foi **774,959 ms**,
contra **219,401 ms** nas primeiras três, cerca de 3,53×. No `async_await`
da VM 3.6.2, sessão 1, passou de **245,800 para 737,443 ms**, cerca de
3,00×. Na primeira sessão JIT Windows de `list_sort`, a primeira chamada
durou 428,186 ms e as chamadas posteriores ficaram na ordem de 1,2–1,5 s.
Todos esses valores foram preservados. A causa não foi isolada; sessões
mais longas e perfilamento de GC/escalonamento/CPU são necessários antes
de afirmar throughput estável. Não houve ajuste do plano de energia,
afinidade ou descarte de chamadas para alterar o resultado.

### Memória no Windows

As nove sessões de 30 ciclos foram verificadas em **549 barreiras e 540
janelas de CPU**. A VM moderna manteve dois processos — lançador
`dart.exe` e `dartvm.exe` — e ambos entraram em cada snapshot. O JIT e a
VM 3.6.2 mantiveram um processo cada. A identidade e a lista completa
dos membros do job permaneceram consistentes em cada sessão.

Valores em MiB, medianas de três sessões por executor. **Comprometimento
privado não é memória residente.** A soma dos working sets pode contar
páginas compartilhadas mais de uma vez e não equivale ao footprint físico
do Mac. Os picos por barreiras são limites inferiores; os picos nativos
individuais foram preservados e não somados em um pico da árvore.

| Métrica | JIT MiB | VM 3.6.2 MiB | VM 3.13.4 MiB |
| --- | ---: | ---: | ---: |
| Comprometimento privado antes das alocações | 27,50 | 153,56 | 170,92 |
| Comprometimento privado após o 30º ciclo | 42,73 | 201,87 | 221,08 |
| Working set somado antes das alocações | 39,96 | 154,30 | 181,62 |
| Working set somado após o 30º ciclo | 55,41 | 204,97 | 232,26 |
| Maior comprometimento privado observado nas barreiras | 45,36 | 204,26 | 221,08 |
| Maior working set somado observado nas barreiras | 57,98 | 205,19 | 232,26 |

O JIT usou menos memória nesses dois contadores e nesse ensaio. Isso não
prova ausência de vazamentos em outros programas, inclusive aplicações
assíncronas, nem em sessões maiores. O tempo desse protocolo contém 61
pausas de 100 ms por sessão e não mede throughput. A CPU do job inclui
todos os threads e descendentes ativos/encerrados; seus ticks nativos de
100 ns não implicam resolução de medição de 100 ns.

### Falhas preparatórias preservadas e evidências

A primeira tentativa de início conservou três processos completos e
status 1: a guarda de timestamps recusou a atualização intencional da
data do marcador de uso do SDK. Os 52 arquivos permaneceram byte a byte
idênticos. A nova composição verifica seus hashes depois de cada processo.
A rotina de captura, executáveis, programas, flags e parsing independente
de valores foram mantidos. A coorte v2 tem nove rodadas completas; os
três resultados da tentativa interrompida continuam preservados à parte.

Também foram guardadas as falhas de preparação por manifesto local
ausente e pela suposição inicial de um único PID na VM moderna. Os
suplementos corrigidos passaram. Os coletores verificaram a atribuição
ao job antes de executar o alvo, limpeza por timeout/pipe herdado, CPU
de filhos/netos e uma alocação conhecida de 64 MiB feita em um filho.

Os 1.463 arquivos brutos do início foram conferidos individualmente.
As demais coortes têm seus arquivos brutos, hashes, relatórios e
verificadores preservados. Os arquivos gerados dos caches nativos foram
omitidos dos pacotes; seus layouts, hashes por execução e guardas de bytes
estão registrados. O executável Windows tem SHA-256
`9ba761afb31b0c3d251c0705ba4053036650b874d84c04c7455a884b779b11c4`.
A fonte congelada v12 tem manifesto
`fd3bddac96ccc9c9d8c5b15e48f68501560f3a8f27cd6372e3e76416323a2fe5`.

A análise derivada é
`/tmp/dartforge-jit-vs-vm-windows-v1/final-windows-assessment.json`.
As evidências completas estão em
`/Volumes/ssd_dock/tmp/dartforge-jit-windows-complete-2026-11-07-evidence/`.

Continuam pendentes a suíte completa da linguagem no JIT, aplicações
reais/HTTP e a investigação das variações e dos gargalos observados.
A aprovação geral de produção permanece pendente.

## Qualificação dos coletores no Windows

A calibração confirmou uma alocação/liberação conhecida de 64 MiB, a CPU
nativa e os contadores do .NET. Sete controles verificaram subprocessos,
saída parcial/grande, código de erro, timeout, limpeza de um filho com pipe
herdado e contabilização da CPU de filhos/netos. Uma segunda calibração
confirmou que uma alocação de 64 MiB feita em um filho entra no total da
árvore. Esses controles conferem o coletor; seus valores não são amostras
de desempenho dos runtimes.

A primeira tentativa do protocolo conservou status 1 ao encontrar dois
processos na VM moderna: `dart.exe` e `dartvm.exe`. O coletor agora enumera
todos os membros do job. O suplemento passou nas três sessões de 61 pausas,
com respostas brutas idênticas; a falha original não foi substituída.

No Windows, memória comprometida privada e working set foram apresentados
separadamente. Somar working sets pode contar páginas compartilhadas mais
de uma vez; essa soma não será apresentada como RAM física única nem como
o footprint do Mac. As definições seguem a [documentação dos contadores do
Windows](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex).
Os dados da preparação, inclusive falhas do coletor, scripts e executável
preservado, estão em
`/Volumes/ssd_dock/tmp/dartforge-jit-windows-preparation-2026-11-07-evidence/`.
Os drivers e verificadores foram concluídos e usados nas quatro coortes
Windows descritas acima, com todas as execuções e verificações primárias
encerradas em código 0.

## Identidade e reprodução

- Fonte v12: manifesto de 27.115 entradas, SHA-256
  `fd3bddac96ccc9c9d8c5b15e48f68501560f3a8f27cd6372e3e76416323a2fe5`.
- Compilador nativo/JIT: SHA-256
  `51b0ac87c5024fe0a8d8a99df44672a54fbd6edd372cf6adba0bc5aabf7d8038`.
- Perfil e flags: `plan.json`, `qualification-v1/environment.json`,
  `host-isolation-policy-v2.json` e comandos completos de cada processo em
  `execution-*/report.json` e `memory-measurements-v2/report.json`.
- Hashes dos programas: `workload-v2-inputs.json`, `memory-v2-inputs.json`.
  Os SDKs têm seus arquivos `version`, `bin/` e `lib/` integralmente
  registrados em `sdk-launch-inputs-v2.json`; LLVM e Clang em
  `native-runtime-inputs-v1.json`.

Os comandos efetivamente usados estão nos arquivos brutos. A forma da
comparação principal foi:

```sh
/path/to/preserved/dartforge run --sdk /path/to/dart-3.6.2/lib \
  --packages /path/to/workloads-v2/.dart_tool/package_config.json \
  --memory=tracing --language-version=3.6 /path/to/program.dart -- check
/path/to/dart-3.6.2/bin/dart --enable-asserts \
  --packages=/path/to/workloads-v2/.dart_tool/package_config.json \
  /path/to/program.dart check
```

O `hello` não recebe `check`; os kernels posteriores usam `steady`. O
coletor define caches próprios e remove flags experimentais do ambiente.
Os modos de medição recusam executar sem auditoria recente de isolamento,
verificador anterior do LSP e calibração dos contadores.

As evidências foram copiadas para
[/Volumes/ssd_dock/tmp/dartforge-jit-vs-vm-2026-11-07-evidence/](/Volumes/ssd_dock/tmp/dartforge-jit-vs-vm-2026-11-07-evidence/collection.json).
Há 1.808 arquivos no conjunto, com manifesto de hashes, dados brutos,
wrappers, coletores, calibrações, verificadores, o executável preservado e
o arquivo da fonte v12. As cópias geradas do cache binário do SDK foram
omitidas do arquivo de evidências; seus hashes por sessão estão preservados.
Os SDKs do Dart permanecem nos caminhos registrados no host.

Arquivo `benchmark-evidence.tar.gz`, SHA-256:
`5f1d66da03dd5319127e756b54d96c3d6014a21e4313d01acbdc50181e475e2c`.
O arquivo `collection.json` registra também os hashes do compilador,
manifesto e arquivo da fonte copiados para o SSD. Os dados originais estão
em `/tmp/dartforge-jit-vs-vm-production-v1/`.
