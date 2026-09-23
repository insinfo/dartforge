# Runtime gerenciado AOT

O heap é um tracing GC preciso, não geracional e sem compactação: coleta por marcação iterativa e varredura, inclusive ciclos sem raízes. Handles i64 positivos identificam slots, 0 é null. A lista livre reutiliza slots; handles mortos não podem ser usados (não há contador de geração). A implementação não oferece ABI multithread nem isolamento entre runtimes.

Cada função emitida que registra referências abre um frame para parâmetros e resultados SSA. Funções sem referências omitem push/pop do GC, evitando esse custo em recursão puramente escalar. As raízes permanecem até seu retorno, portanto temporários de loops podem permanecer vivos durante a função inteira. Esta retenção conservadora de duração não transforma escalares em raízes: o tracing de campos usa tags exatas. Pop não coleta; o chamador registra imediatamente a referência retornada antes de alocar novamente. Campos int?/bool? usam slots escalares de presença/payload; referências usam um slot com is_ref=1.

A coleta automática ocorre antes de alocar, após um limiar de 256 alocações (ajustado pelo número de objetos vivos) ou ao ultrapassar o limiar de bytes estimados (mínimo 1 MiB, após coleta duas vezes os bytes vivos). DARTFORGE_GC_STRESS força coleta antes de toda alocação. O resultado da nova alocação precisa ser enraizado antes da próxima. dartforge_gc_collect permite coleta explícita.

## ABI

- dartforge_gc_push_frame(slot_count:i64) -> i64; dartforge_gc_set_root(frame:i64, slot:i64, handle:i64); dartforge_gc_pop_frame(frame:i64); dartforge_gc_collect().
- dartforge_object_new(class_id:i64, field_count:i64) -> i64; dartforge_object_get(handle:i64,index:i64) -> i64; dartforge_object_set(handle:i64,index:i64,bits:i64,is_ref:i8); dartforge_object_class(handle:i64) -> i64.
- dartforge_string_new(ptr:*const u8,len:i64) -> i64; dartforge_string_concat(a:i64,b:i64) -> i64; dartforge_string_equal(a:i64,b:i64) -> i8; dartforge_print_string(handle:i64).
- Strings são UTF-8 e imutáveis. Concatenação exige operandos não nulos; igualdade e impressão aceitam handle zero. Não há normalização Unicode.
- O único acesso inseguro à memória é a leitura da constante UTF-8 estrangeira em string_new. O emissor garante região legível de len bytes; ponteiro pode ser ignorado para len=0. As demais operações de heap são Rust seguro e validam os índices. Quebrar o contrato causa falha explícita, não execução de Dart com comportamento definido.

O driver embarca heap.rs e os fragmentos do runtime (nucleo.rs, gc_raizes.rs, excecoes.rs, saida.rs, strings.rs, colecoes.rs, closures.rs, na ordem de FRAGMENTOS em build.rs) em um único arquivo standalone. O workspace testa o mesmo módulo heap.rs com unsafe proibido. Não há dependência de Dart VM, GC Dart, arena que apenas cresce ou runtime Swift.

## Referências consultadas

Dart SDK tag 3.6.2, revisão b0cc5495e0f5e8ae150825a5352e708cb49e65ff: runtime/vm/heap/heap.h e runtime/vm/heap/marker.h. O segundo descreve marcação de objetos alcançáveis e raízes do coletor mark-sweep. O Dart VM possui gerações/concurrency que este runtime ainda não implementa. Nenhum código dessas referências foi copiado; a implementação Rust usa raízes e tags explícitas adequadas ao emissor LLVM atual.

## Revisão e microbenchmark histórico da ABI append

Marcação usa bitmap por slot e pilha de trabalho reutilizados, sem hash por objeto. A varredura visita o máximo histórico de slots, inclusive slots livres; não há redução automática da capacidade. Frames procuram do topo para baixo: o caso normal do emissor encontra imediatamente o frame corrente, mas registrar em um ancestral custa profundidade. Raízes duplicadas ainda são retidas e processadas; funções longas e loops podem consumir memória linear no número de registros. Esta descrição registra a ABI append anterior; o incremento de slots fixos abaixo a substitui no código emitido.

Coleta em stress a cada alocação com N referências retidas provoca trabalho quadrático; esse modo verifica corretude, não representa configuração de produção. No modo normal, o limiar adaptativo evita varrer cada alocação. Os contadores HeapStats são cumulativos: allocations, collections, reclaimed, roots_scanned, slots_scanned e snapshot live_objects/reserved_slots. Não são medição exata de bytes residentes nem do alocador do sistema.

Reprodução: cargo test -p dartforge-runtime --release gc_microbenchmark -- --ignored --nocapture. Medição única em Windows x64/rustc1.98.1, 2026-09-20, sem comparação com Dart ou Swift:

| Configuração | Alocar+enraizar+GC automático | Coletar vivos | Coletar mortos | Coletas | Raízes visitadas |
| --- | --- | --- | --- | --- | --- |
| 100.000 objetos, normal | 7.814.400 ns | 2.102.900 ns | 6.436.200 ns | 8 | 193.184 |
| 2.000 objetos, stress | 7.567.900 ns | 6.900 ns | 38.700 ns | 2.002 | 2.001.000 |

Ambos terminam com zero objetos vivos; slots reservados permanecem reutilizáveis. Números são amostra local, sujeitos a ruído e sem promessa de vantagem de desempenho. Testes adicionais cobrem ciclo de 20.000 objetos sem recursão, identidade de objetos, transferência de handles no retorno, Unicode/NUL e comparação nullable.

Swift revision 6e75592c4025239130e370877ab0fab4006b675b, docs/SIL/ARCOptimization.md e stdlib/public/runtime/HeapObject.cpp consultados: operações retain/release e otimização de ownership pertencem ao modelo ARC do Swift. Copiar esse modelo não resolve os ciclos fortes permitidos por Dart; aqui tracing alcançável coleta esses ciclos sem exigir weak/unowned do programa. Nenhum código Swift foi copiado.

Correção de contrato: string_equal aceita zero em qualquer operando (dois zeros iguais); print_string(0) imprime null. string_concat continua exigindo strings não nulas. Ao alocar Value::Object pré-preenchido diretamente pela API Rust, suas referências de entrada precisam estar enraizadas antes da coleta anterior à alocação, como exige a ABI. O emissor usa object_new com campos zerados seguido por object_set.



## Slots fixos e contabilização por bytes

A ABI atual recebe slot_count na abertura do frame e set_root substitui um slot existente, inclusive por zero. Não redimensiona frame durante loops; índice inválido é erro de contrato. API Rust push_frame()/root() e símbolo legado gc_root ainda existem para regressões antigas, mas o emissor usa slots fixos. O runtime não deduplica raízes de slots distintos: cópias locais são independentes e precisam continuar protegidas.

HeapStats inclui live_roots/peak_roots (slots não nulos), root_slots/peak_root_slots (tamanho reservado dos frames ativos), estimated_bytes/peak_estimated_bytes (size_of<Value> por objeto vivo mais String.capacity ou capacidade do vetor de campos multiplicada por size_of<(i64,bool)>). Não inclui capacidade vazia da tabela de handles, bitmap, pilha de tracing, raízes, overhead do alocador ou RSS. A contagem é subtraída quando o payload é coletado. Multiplicação/soma de payload usa checked arithmetic; thresholds saturam. Objetos vivos podem exceder os limiares e não são descartados artificialmente.

DARTFORGE_GC_STATS=1 imprime exatamente um JSON em stderr após dartforge_entry retornar com sucesso; sem configuração não imprime estatísticas. Não força coleta final. Chave dartforge_gc contém allocations, collections, reclaimed, live_objects, reserved_slots, root_slots, peak_root_slots, live_roots, peak_roots, live_bytes e peak_live_bytes. Os dois campos bytes expõem as estimativas acima, não memória residente.

Reprodução: cargo test -p dartforge-runtime --release fixed_slot_microbenchmark -- --ignored --nocapture. Amostra única local Windows/rustc1.98.1 em 2026-09-20, 100.000 strings transientes e uma raiz substituída:

| Modo | Alocar+substituir+GC automático | Coleta final medida | Coletas | Recolhidos | Pico slots raiz | Pico bytes estimados |
| --- | --- | --- | --- | --- | --- | --- |
| Normal | 3.895.100 ns | 1.900 ns | 391 | 99.999 | 1 | 12.336 |
| Stress | 3.582.100 ns | 0 ns | 100.001 | 99.999 | 1 | 96 |

Zero ns é a resolução observada do relógio para intervalo curto, não ausência de trabalho. A diferença entre modos não sustenta comparação de desempenho: amostra curta e sem repetição estatística. Evidência principal: pico de raízes constante e coleta de 99.999 objetos; slots de objetos reservados ficam em 257 (normal) ou 2 (stress), em vez de crescer com 100.000 iterações.

As referências Dart3.6.2 e Swift listadas acima motivam separar raízes, identidade e ownership; slots estáticos são protocolo próprio entre emissor LLVM e tracing runtime, não adoção de ARC Swift.

## Valores canônicos de enum

`dartforge_enum_get` usa chave (ID nominal da classe, ordinal). O cache mantém
cada singleton alcançável até o encerramento do processo e protege a string do
nome durante sua criação. `permanent_roots` contabiliza essas raízes separadamente
dos slots dos frames; `roots_scanned` inclui ambos. Frames podem terminar com zero
raízes enquanto enums continuam vivos. A estimativa de bytes inclui os payloads
gerenciados, mas não o overhead do HashMap de canonização.
