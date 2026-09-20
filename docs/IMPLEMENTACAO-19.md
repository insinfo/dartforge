# Incremento 19 — records e desestruturação

Alvo: Dart 3.6.2. Records são valores estruturais imutáveis com campos posicionais
e/ou nomeados, distintos de classes e de listas. O trabalho desta etapa amplia
o frontend, análise e JavaScript; o lowering LLVM permanece diagnosticado.

## Contrato

- Tipos de records preservam posições e nomes canônicos dos campos nomeados.
  A ordem dos nomes não define um tipo diferente.
- Literais preservam a ordem total de avaliação da fonte, incluindo campos
  nomeados intercalados com posicionais. Os getters posicionais começam em $1.
- Desestruturações locais var/final avaliam o inicializador uma vez e respeitam
  escopo, uso antes da declaração e imutabilidade dos bindings final.
- Records integram tipos anuláveis, coleções e substituição de parâmetros
  genéricos. Descritores em execução usam os valores reais de cada campo,
  mesmo quando a anotação estática foi ampliada para Object.
- Igualdade compara formas e campos estruturalmente, inclusive para records
  guardados como Object. Listas armazenadas em records conservam sua identidade.
- Impressão dos campos suportados segue a forma do SDK, inclusive records
  aninhados. Const records, padrões aninhados/atribuições por padrão e hashCode
  ficam fora deste recorte e precisam de implementação própria.

O corpus em tests/conformance/modules/records19 exercita imports, ordem de
avaliação, igualdade, retornos múltiplos, campos nomeados, genéricos e nulabilidade.
As 27 linhas do resultado foram conferidas executando o Dart VM 3.6.2.

## Macros e geração de código

O [protótipo oficial de macros](https://dart.googlesource.com/macros/) informa
que o projeto foi cancelado em janeiro de 2025. Não faz parte da linguagem estável
Dart 3.6.2; @JsonCodable não é uma API de serialização implementada neste projeto.
O clone de referência está em references/dart-macros, revisão
89aecb1e3c10373ea795f003268d7cd48c9ed431, ignorado pelo Git do DartForge.

Uma extensão experimental futura do DartForge exigiria contrato próprio, execução
determinística, cache por dependências e diagnósticos com origem. Isso não deve
ser anunciado como compatibilidade com uma funcionalidade estável do Dart.

## Extensions e próximos passos

Métodos de extensão do subconjunto existente continuam disponíveis. O exemplo
com RegExp depende da biblioteca padrão, que ainda não implementa essa API.
Extension types são um recurso diferente: exigem identidade estática separada,
validação de membros/interfaces e apagamento para o tipo de representação.
Continuam pendentes, junto com classes genéricas e a expansão de patterns.
Null safety não autoriza remover checagens sem prova; as verificações reificadas
de escritas covariantes e casts continuam necessárias.

## Referências consultadas

- [Records](https://dart.dev/language/records) e [extension types](https://dart.dev/language/extension-types).
- SDK fixado em b0cc5495e0f5e8ae150825a5352e708cb49e65ff:
  [literais e getters](https://github.com/dart-lang/sdk/blob/b0cc5495e0f5e8ae150825a5352e708cb49e65ff/tests/language/records/simple/literals_and_field_access_test.dart)
  e [igualdade e hashCode](https://github.com/dart-lang/sdk/blob/b0cc5495e0f5e8ae150825a5352e708cb49e65ff/tests/language/records/simple/equals_and_hashcode_test.dart).
  Consultar esses testes não significa suportar todas as APIs usadas por eles.

## Validação local

- Workspace completo: **357 testes e doctests aprovados**, com testes ignorados
  por padrão habilitados, incluindo Node, LLVM, GC e FFI. Log: target/test19.log.
- Depois, a integração records19 foi ampliada e executada novamente: **4 testes
  aprovados**, incluindo uma nova regressão de sombreamento na fusão de funções
  e chamada de callback armazenado em record. Total: 358 testes distintos.
- Formatação, Clippy de todos os targets com -D warnings e rustdoc incluindo
  itens privados com -D warnings aprovados.
- Cabeçalhos de for com desestruturação têm diagnóstico antecipado; apenas
  declarações locais simples em blocos são suportadas neste recorte.
- **42 casos diferenciais aprovados** contra dart2js 3.6.2 -O2, com folding e
  fusão habilitados: [resultados completos](conformance-js-increment-19.json).

O benchmark pipeline inclui um corpus separado de records. A medição cobre
compilação em processo aquecido e descarte da saída, sem comparação com DDC/dart2js.
Na medição local de 20/09/2026, o corpus de 287 bytes teve mediana de 27,819 µs
e p95 de 38,807 µs, calculados sobre médias de 21 lotes de 100 compilações, após
100 compilações de aquecimento. A saída continha 13.674 bytes. A variação entre
lotes e o tamanho reduzido do corpus impedem extrapolar esse baseline para apps.
Metadados e amostras: [benchmark-increment-19.json](benchmark-increment-19.json).
Nenhuma execução de CI foi solicitada ou acompanhada nesta etapa.
