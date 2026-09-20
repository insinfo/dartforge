# Incremento 16 — anotações e FFI escalar estático

Alvo: Dart 3.6.2. Este incremento conecta declarações Dart `external` ao backend
LLVM e à ligação de objetos nativos. O runtime continua escrito em Rust; os
programas de teste C exercitam a fronteira ABI.

```dart
import 'dart:ffi' as ffi;

@ffi.Native<ffi.Int32 Function(ffi.Int32, ffi.Int32)>(symbol: 'somar_valores')
external int somarValores(int a, int b);

void main() {
  print(somarValores(20, 22));
}
```

Compile a implementação C/Rust para um objeto compatível com o host e forneça-o
ao driver. A opção pode ser repetida para vários objetos:

```powershell
clang -c examples/ffi/native.c -o native.obj
cargo run -p dartforge-cli -- aot examples/ffi/main.dart app.exe --link-object native.obj --optimize
./app.exe
```

## Contrato implementado

- `@Native` com assinatura explícita, `symbol` opcional e `isLeaf` booleano literal.
- Funções top-level `external`, sem corpo, parâmetros posicionais obrigatórios.
- Marcadores C `Int32`, `Int64` e `Void`; tipos Dart correspondentes `int` e `void`.
- Import direto de `dart:ffi`, sem filtros, com prefixo opcional.
- Conversão de argumentos Int32 por truncamento; retorno Int32 com extensão de sinal.
- Símbolos nativos preservados antes da renomeação de bibliotecas; conflitos de
  assinatura e nomes reservados do runtime são diagnosticados.
- Funções external são excluídas da fusão de corpos idênticos.

A resolução é estática neste subconjunto do DartForge. Isso não reproduz toda
a resolução de assets, resolvers e bibliotecas dinâmicas de `@Native` no SDK.
Símbolos ausentes falham na ligação. Objetos precisam corresponder à arquitetura
e ABI do host; a opção não habilita compilação cruzada.

Ponteiros, Double/Float, structs/unions, callbacks, varargs, NativeFinalizer,
DynamicLibrary e assets nativos permanecem pendentes. Handles do GC não são
endereços C. `isLeaf` é preservado, mas o runtime atual não implementa transições
de estado de threads como a VM Dart; callbacks e objetos gerenciados são rejeitados.
JavaScript e Wasm não recebem chamadas FFI silenciosamente.

## Anotações

O frontend conserva os metadados reconhecidos `@override`, `@deprecated` e
`@Deprecated('mensagem')` nas declarações suportadas. Isso não implementa reflexão,
um sistema de geração de código ou os avisos completos do analyzer. Anotações
customizadas, campos e parâmetros anotados ainda exigem expansão do frontend.

`using`, `Arena` e `Utf8` pertencem a `package:ffi`, não a `dart:ffi` nem a uma
construção especial da linguagem. Escopos de Arena e finalização por GC têm
contratos diferentes; este incremento não os anuncia como implementados.

Referências e verificações contra o SDK fixado:
[FFI-ANNOTATIONS-REFERENCIAS.md](FFI-ANNOTATIONS-REFERENCIAS.md).

## Validação local integrada

O teste diferencial executou as mesmas declarações @Native no Dart VM e AOT
3.6.2 e comparou a saída com DartForge O0/O2. Ambos passaram; o
[relatório](conformance-ffi-increment-16-17.json) registra as saídas e hashes das
fontes. Os testes Rust também verificam erro de ligação sem publicar saída,
caminhos com espaços e exclusão de bindings externos da fusão de funções.
