# Especialização guardada do kernel de cores — 19/10/2026

A última falha nativa ocorria porque os switches de `convertFormatValue` leem o
campo `index` dos enums. Um token de identidade não prova o conteúdo imutável
desse campo. A análise anterior dobrava somente `from == to`, deixando uma
cópia praticamente do tamanho original e corretamente recusada pelo orçamento.

A nova especialização mantém a leitura e compara o índice observado com o valor
esperado. A correspondência segue ao ramo especializado; a divergência chama o
corpo original com os parâmetros reais, retornando seu resultado e estado de
exceção pendente. Não se remove a avaliação dos argumentos/getters, não se altera
a ABI e não se acrescenta limpeza de exceções nesse caminho.

Só se admite esse encaminhamento através de um prefixo de entrada único sem
efeitos observáveis. Chamadas, alocações, escritas, junções, ciclos e receptores
não certificados impedem a transformação. Operandos e definições devem estar
disponíveis no prefixo. A remoção de arestas de switch também atualiza os phis de
destinos ainda vivos. Análise limitada a 32 guardas, 32 blocos e 256 instruções de
prefixo; limites gerais de cópias e crescimento não foram aumentados.

## Validação funcional

- **777 testes nativos passaram, nenhum falhou, nove continuam ignorados**.
- **15 testes dirigidos passaram**. Os controles incluem efeitos, alocação,
  receptores desconhecidos, ciclos/junções, IDs esgotados, phis e limites de análise.
- Um teste executa código nativo com acerto e falha da guarda, índice observado
  deliberadamente diferente da previsão, retorno Ref e exceção pendente. O miss
  mantém o resultado genérico e a exceção.
- A fixture de formatos constantes consome 89 saídas, incluindo inteiros extremos,
  NaNs/payloads, zero negativo, identidade e contagem de avaliações. A cópia
  guardada foi conferida no LLVM do binário ON.
- Essa fixture, a de padrões enum e as 2.736 saídas de cores coincidem com VM e
  AOT oficiais do Dart 3.6.2 em OFF/ON, também sob GC stress. Produtor/entradas
  permaneceram congelados durante cada comparação.
- No HIR do contexto real: 3.652 instruções/518 ramos passam a 94/14, incluindo as
  guardas e encaminhamento genérico. O limite de crescimento segue 2.048.

## Desempenho e tamanho, separados da correção

O programa comum `bench/comparison/color_numeric_performance.dart` permaneceu
inalterado. Um milhão de conversões por amostra; cinco amostras por processo,
primeira descartada, três processos por perfil em ordem rotacionada. Os checksums
coincidiram. A tabela usa a mediana das três medianas quentes por processo.

| Perfil | Tempo por milhão de conversões |
|---|---:|
| DartForge OFF | 32.373 µs |
| DartForge ON | 30.338,5 µs |
| Dart AOT oficial | 5.743 µs |

ON registrou cerca de **6,3% menos tempo** neste kernel; ainda levou cerca de
**5,3 vezes** o tempo oficial. Os arquivos DartForge ficaram com os mesmos
3.096.624 bytes, mas a seção nativa `__text` cresceu **1.368 bytes**. A seção
Mach-O do launcher oficial não inclui seu snapshot AOT anexado e não foi usada
para alegar equivalência de tamanho de código.

O LLVM dos artefatos medidos confirma ausência da cópia em OFF e chamada à cópia
protegida em ON. Uma primeira rodada foi preservada: 32.567/30.403,5 µs;
os hashes dos executáveis coincidem com os da rodada final. Não se extrapola o
resultado para todo o dart_ui, File Explorer, HTTP/JSON ou outros sistemas.

Execuções em macOS ARM64, LLVM 22.1.8 e SDK macOS 26.5. A opção
`DARTFORGE_CONSTANT_SPECIALIZATION=1` continua experimental e desligada por
padrão. Os quatro checks do `CONTRIBUTING.md` foram executados: `fmt`/Clippy ainda
falham em problemas anteriores, o teste geral para no linker padrão Apple LLVM
21 lendo bitcode LLVM 22, e a documentação foi gerada com avisos.

Scripts, comandos, hashes, resultados e amostras:
[bench/results/2026-10-19-guarded-color/](bench/results/2026-10-19-guarded-color/README.md).
