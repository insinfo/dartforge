# Incremento 09 — heap gerenciado e fusão opcional

Alvo mantido em **Dart 3.6.2**, licença MIT e documentação Rust em português.
Três subagentes trabalharam no runtime, emissor LLVM e otimização; a integração
incluiu revisão independente de raízes e consulta às referências fixadas.

## Implementação

- Classes com construtor implícito, campos, herança e despacho virtual nativo.
  Adaptadores preservam assinaturas de overrides com parâmetros contravariantes
  e retornos covariantes dentro do subconjunto. Inicialização respeita campos
  derivados antes dos campos da base.
- Strings imutáveis UTF-8 com concatenação, igualdade por conteúdo e impressão;
  objetos mantêm identidade por handle. Referências nullable usam zero e
  preservam `??`, `!`, retornos e promoções já validados semanticamente.
- Runtime Rust com GC preciso por marcação iterativa e varredura, bitmap e
  buffers reutilizados, lista livre, coleta de ciclos e contadores. Raízes SSA,
  argumentos e receivers sobrevivem a alocações aninhadas. Funções sem raízes
  omitem a criação e remoção de frames.
- Corpos `=>` em funções e métodos tipados, inclusive avaliação com descarte
  do resultado em funções `void`, conforme teste oficial SDK 3.6.2.
- Flag `--merge-identical-functions` nos três comandos de emissão, desligada
  por padrão. Compara estrutura completa, conserva resolução e assinaturas,
  evita captura de nomes, redireciona chamadas e remove definições redundantes.
  APIs antigas foram preservadas; opções completas fazem parte da chave do cache.
- Quatro clones Swift registrados e ignorados pelo Git. A branch com fontes do
  SwiftWasm foi selecionada explicitamente, pois a padrão é de distribuição.

## Medições

[JSON da fusão](dados/benchmark-merge-increment-09.json): 200 funções iguais, 15 amostras
de 10 compilações em memória por configuração, executadas sem outro build ativo.

| Modo | Funções emitidas, incluindo main | Bytes JS | Mediana de compilação |
| --- | ---: | ---: | ---: |
| Padrão | 201 | 18.814 | 0,47510 ms |
| Fusão | 2 | 214 | 0,89655 ms |

O custo da fusão aumentou nessa medição; a redução é de saída duplicada em um
corpus deliberadamente favorável. Não demonstra ganhos em aplicações reais.
O [microbenchmark do runtime](../../crates/runtime/README.md) mede alocação, raízes,
coleta de vivos/mortos e evidencia o custo elevado do modo GC stress. Não há
evidência de superioridade geral sobre DDC/dart2js/Dart AOT neste incremento.

## Validação registrada

- `cargo fmt --all -- --check`, Clippy workspace/all-targets com `-D warnings`,
  rustdoc incluindo privados com `-D warnings` e build release aprovados.
- **213 testes** aprovados com `cargo test --locked --workspace -- --include-ignored`,
  incluindo execução real JS/nativa, teste GC de ciclo com 20 mil objetos e doctests.
- [Comparação nativa](dados/conformance-native-increment-09.json): **12 programas × O0/O2**,
  fusão ligada e GC stress, sem divergências contra VM/AOT oficiais 3.6.2. Inclui
  strings/objetos entre bibliotecas, retornos e ciclos durante 600 chamadas.
- [Comparação JS](dados/conformance-js-increment-09.json): **26 programas** com fusão,
  sem divergências contra dart2js 3.6.2 O2.
- Demonstrações locais em `dist/native/managed-09.exe` e `dist/merged-09.mjs`.
  A segunda não contém mais a definição duplicada `soma2` e imprime `42`, `3`, `7`.

Relatórios foram coletados no working tree do incremento; o JSON nativo registra
a revisão base e o status sujo explicitamente. Seus tempos com GC stress e uma
amostra servem como registro da execução, não como benchmark de produção.

## Limites e sequência

Raízes temporárias são mantidas até retorno: laços longos podem reter memória.
GC não é geracional, concorrente ou compactador; o limiar usa contagem de objetos,
não bytes. A capacidade máxima de slots permanece reservada para reutilização.
Esses são pontos concretos para otimizar com análise de vida útil e medições.

Permanecem ausentes extensions nativas, construtores explícitos, generics,
closures, coleções, exceções capturáveis e a biblioteca padrão completa. Strings
UTF-8 do subconjunto não implementam a API UTF-16 inteira do Dart. A fusão não
faz alpha-renaming nem prova algébrica; stack traces podem usar a representante.
LLVM/Clang continuam dependências externas, embora frontend e runtime sejam Rust.

Próximos trabalhos prioritários: slots de raízes com vida útil limitada, gatilho
de GC por memória, cache do runtime/objetos por ABI e toolchain, IDs de símbolos
na IR e ampliação gradual da semântica com oráculos fixados no SDK 3.6.2.
