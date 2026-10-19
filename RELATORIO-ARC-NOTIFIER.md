# Teste de notifier ARC e guardas de campos — 19/10/2026

A falha restante do notifier era um falso negativo do teste. Ele exigia erro na
preparação conservadora de `Owner.update` e procurava uma leitura escalar nesse
mesmo corpo. No HIR atual, esse método já usa o protocolo ARC de campo late-Ref;
as leituras escalares de `holds` ficam em `Derived.notify`, com o guarda físico
presente. Não foi encontrada necessidade de alterar o motor ARC nessa análise.

A regressão foi corrigida sem relaxar a representação dos campos:

- As leituras late-Ref originais preservam receptor, índice, resultado e helper.
- Cada leitura escalar real é localizada em `Owner.update` ou `Derived.notify`,
  conforme o inlining. Sua preparação deve manter receptor, índice e resultado
  i64, usando o accessor escalar conferido com o vetor correto de layouts.
- O vetor deve admitir a classe Derived e seu tamanho declarado. O teste confere
  contagem das entradas e código de representação escalar i64.
- Um controle altera a imagem para não haver layout escalar admissível. A
  preparação do corpo real deve falhar com `no matching layout declared`.
  Os controles sintéticos anteriores de contradição de layouts continuam ativos.

Os 14 testes dirigidos passaram após o refinamento final de localização dos
campos. Na rodada ampla anterior a esse refinamento: **770 passaram, 1 falhou,
9 ignorados**. A única falha restante é a especialização do kernel de cor,
exatamente a outra falha já existente na base anterior.

Um compilador de produção congelado de `ad52385d` compilou a fixture Dart
inalterada em tracing e ARC. Os dois binários produziram exatamente as saídas da
VM Dart 3.6.2 nas rodadas normal e de GC stress. ARC também usou
`DARTFORGE_ARC_CHECK=1` e `DARTFORGE_ARC_NURSERY=0`. Não se atribui uma correção
de produção, uma prova completa de segurança ARC ou ganho de desempenho a esse
ajuste de teste. Execuções em macOS ARM64 com LLVM 22.1.8 e SDK macOS 26.5.

Os quatro checks do `CONTRIBUTING.md` foram executados: formatação e Clippy ainda
falham em problemas anteriores; testes do workspace param no linker padrão
Apple LLVM 21 lendo bitcode LLVM 22; documentação foi gerada com avisos.

Reprodução, comandos, hashes e resultados:
[bench/results/2026-10-19-arc-notifier/](bench/results/2026-10-19-arc-notifier/README.md).
