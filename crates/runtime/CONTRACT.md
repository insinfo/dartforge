# Capturas e listas gerenciadas

Esta API Rust é infraestrutura do runtime. **Ainda não implementa lowering de closures
ou listas Dart para LLVM**, chamada indireta de closures, assinatura de função ou
verificação de tipos genéricos. A ABI C escalar existente permanece inalterada.

`TaggedValue` contém `bits: i64` e `is_ref: bool`. Somente a tag de referência faz
o GC seguir os bits como handle; zero é referência null. Escalares com bits iguais
a um handle não mantêm objetos vivos. Handles identificam objetos vivos e podem ser
reutilizados depois da coleta; não devem ser conservados fora do protocolo de raízes.

| Estrutura | Contrato |
| --- | --- |
| `Cell` | Captura mutável. Ambientes diferentes podem compartilhar a mesma célula e observar suas alterações. |
| `Environment` | Capturas ordenadas e imutáveis. Valores capturados mutáveis são referências para células. |
| `Closure` | Novo objeto com identidade própria, ID simbólico de código e referência para um ambiente. Duas criações com o mesmo código continuam distintas. Não executa código. |
| `List` | Sequência expansível de valores com tags. Leitura/substituição por índice e acréscimo; capacidade e comprimento são distintos. |

As APIs `create_cell`, `create_environment`, `create_closure` e `create_list`
protegem internamente as referências recebidas durante suas alocações. O chamador
deve fornecer handles vivos e enraizar o resultado antes de uma próxima operação
que possa coletar. `list_push` protege a própria lista durante eventual coleta
causada pelo crescimento; depois da operação o chamador continua responsável por
sua raiz. `cell_set` e `list_set` não alocam nem coletam.

Frames podem terminar enquanto closures permanecem vivas: a cadeia closure →
ambiente → célula preserva a captura escapada. O tracing iterativo também segue
referências em listas e recupera ciclos inalcançáveis, inclusive ciclos envolvendo
closures. Capacidade de buffers de listas e ambientes entra na estimativa de bytes;
essa estimativa não é RSS nem mede todos os metadados do alocador.

Tipos de handles e índices inválidos causam `panic` na API Rust atual. Isso é um
contrato interno de compilador/runtime, não uma implementação das exceções de Dart.
Não há listas fixas/imutáveis, iteradores, remoção, crescimento por setter de length,
execução de código por `code_id` ou API pública FFI para essas estruturas ainda.

Referências consultadas no SDK Dart **3.6.2**, commit
`b0cc5495e0f5e8ae150825a5352e708cb49e65ff`:

- `runtime/vm/object.h`: separação de `Context`, `Closure` e objetos de coleção.
- `sdk/lib/core/list.dart`: distinção entre listas fixas e expansíveis e acesso por índice.
- `runtime/vm/compiler/backend/flow_graph_compiler.cc`: mapas de referências do coletor.

Estas fontes orientam o contrato; não houve cópia do código do SDK ou de seu layout
binário. Os testes próprios usam coleta antes de cada alocação para verificar escape,
alias de células, identidade, ciclos e precisão das tags.
