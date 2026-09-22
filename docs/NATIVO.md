# Compilador Nativo (LLVM) — Trilha Nova

Documentação de arquitetura, modelo de objetos, ABI, convenções de despacho e medições do compilador nativo DartForge AOT/LLVM.

---

## 1. Arquitetura

O compilador nativo consome exclusivamente os artefatos semânticos gerados pela trilha nova:
- `Program` (`crates/elements/src/model.rs`)
- `OutlineTypes` (`crates/types/src/resolve.rs`)
- `BodyTypes` (`crates/types/src/resolved.rs`)
- `TypeTable` e `CoreTypes` (`crates/types/src/table.rs`)

O pipeline executa as seguintes etapas:
1. **Carregamento e Inferência**: O programa e o SDK `dart:core` (seção `vm` de `libraries.json`) são carregados em um `Program` unificado e os corpos são resolvidos na tabela lateral de tipos (`BodyTypes`).
2. **Lowering para HIR Própria** (`crates/emit_native/src/hir.rs`): Toda chamada é resolvida como estática (direta), de interface (por seletor na vtable) ou dinâmica. Casts implícitos tornam-se nós explícitos de conversão/verificação. Desugaring completo de açúcares sintáticos (cascatas, coalescência nula `??`, `??=`, laços `for-in`, desestruturação de padrões e máquinas de estados de funções `async`).
3. **Emissão de LLVM IR** (`crates/emit_native/src/llvm/`): A HIR é traduzida para LLVM IR textual 17+ com ponteiros opacos.
4. **Clang & Ligação com Cache de Objetos** (`crates/emit_native/src/cache.rs`): O LLVM IR é compilado para código de máquina via Clang (`-O2` ou `-O0`). O runtime Rust (`crates/runtime`) é compilado previamente para arquivo-objeto (`.obj`), sendo reutilizado por hash de módulo para reduzir o tempo de compilação em ordens de magnitude.

---

## 2. Modelo de Objetos

### Layout em Memória
Todo objeto gerenciado no heap alocado pelo runtime possui a seguinte estrutura:

```
+-------------------------------------------------------+
| class_id (i64)                                        |  Offset 0
+-------------------------------------------------------+
| type_args / metadata_ptr (i64 / *const TypeDesc)       |  Offset 8
+-------------------------------------------------------+
| field 0 (i64 bits) | is_ref 0 (bool / u8)             |  Offset 16
+-------------------------------------------------------+
| field 1 (i64 bits) | is_ref 1 (bool / u8)             |  Offset 32
+-------------------------------------------------------+
| ...                                                   |
+-------------------------------------------------------+
```

1. **Cabeçalho (Header)**:
   - `class_id` (i64): Identificador estável da classe concreta. Utilizado para consultas diretas de tipo, indexação de vtable e verificação rápida de `is`.
   - `metadata_ptr` (i64): Ponteiro para o descritor estático da classe e vetor canônico de argumentos de tipo genéricos (para tipos reificados em verificações `is` e `as`).
2. **Campos (Fields)**:
   - Indexados sequencialmente a partir do offset 0.
   - Cada campo armazena seus `bits` (i64) e uma marcação explícita de se o valor é referência gerenciada (`is_ref`), orientando o garbage collector por tracing preciso (`heap.rs`).

### Primitivos e Representações
- `int`: Inteiro de 64 bits (`i64`) em complemento de dois com estouro modular, idêntico à VM Dart (ao contrário do JS Number).
- `double`: Ponto flutuante IEEE 754 de 64 bits (`f64`).
- `bool`: Booleano representado como `i1` na IR do LLVM e `u8` na fronteira com runtime Rust.
- `String`: Sequência codificada em UTF-16 no runtime para compatibilidade estrita com indexação e propriedades do SDK Dart.
- `null`: Representado por handle zero (`0`) quando referência, ou tagged payload com tag de ausência.

### Despacho por Seletor e VTable
- **Chamada Estática**: Quando o tipo do receptor é conhecido e final ou o método é privado sem overrides, a chamada é emitida diretamente para o símbolo mangled da função.
- **Despacho por Seletor (Interface Call)**:
  - Cada seletor (nome do método + aridade + nomes de argumentos nomeados) recebe um índice numérico global estável (`selector_id`).
  - Cada classe concreta possui uma **vtable** preenchida com ponteiros de função indexados pelo `selector_id`.
  - Entradas não implementadas na vtable apontam para um stub de `noSuchMethod`.
- **Genéricos Reificados**:
  - Objetos genéricos armazenam seus tipos de tipo de instância (`type_args`).
  - Operações `o is List<int>` avaliam o `class_id` de `o` e, se compatível, comparam os argumentos de tipo canônicos com a tabela de tipos do runtime.

### Closures e Ambientes
- Uma closure é um par `(função_código, ambiente)`.
- Variáveis locais mutáveis capturadas são alocadas em `Cell`s no heap. Ambientes (`Environment`) contêm vetores de referências para essas células, garantindo visibilidade mútua de mutações mesmo após o frame léxico retornar.

### Exceções e StackTrace
- O compilador suporta `throw`, `rethrow` e blocos `try / on T / catch (e, s) / finally`.
- O runtime mantém um registro de exceção pendente e captura uma lista de frames (`StackTrace`) no ponto de emissão de `throw`.

---

## 3. Pendências para outras fases

*Nenhuma no momento.*

---

## 4. Medições de Desempenho e Recursos

| Programa | Front-end | HIR | LLVM IR | Clang | Link | Tempo Total | Pico de Memória |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| *(a ser preenchido após os primeiros benchmarks)* | - | - | - | - | - | - | - |


