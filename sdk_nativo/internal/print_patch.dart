// Substitui `_internal/vm/lib/print_patch.dart` (sobreposição `sdk_nativo/`,
// P5c). Na VM, `printToConsole` chama a closure `_printClosure` que o
// embedder (`dart:io`/`builtin`) instala; aqui ela é um native do runtime
// que escreve a linha e o fim de linha, como o `print` da VM.
//
// As funções `_dartforge*` são os erros que o runtime do DartForge lança
// (índice fora da faixa…) construídos como os objetos Dart da fonte: o
// registro de `dart:_internal` entrega os endereços delas ao runtime
// (`dartforge_registrar_ajudante`, `crates/runtime/src/nativos_listas.rs`).

part of "internal_patch.dart";

@patch
@pragma("vm:external-name", "DartForge_imprimir")
external void printToConsole(String line);

@pragma("vm:entry-point")
Object _dartforgeErroDeIndice(int indice, Object? alvo, int tamanho) =>
    new IndexError.withLength(indice, tamanho, indexable: alvo, name: "index");

@pragma("vm:entry-point")
Object _dartforgeErroLate(String nome, int codigo) {
  if (codigo == 0) return new LateError.fieldNI(nome);
  if (codigo == 1) return new LateError.fieldAI(nome);
  if (codigo == 2) return new LateError.localNI(nome);
  if (codigo == 3) return new LateError.localAI(nome);
  if (codigo == 4) return new LateError.fieldADI(nome);
  return new LateError.localADI(nome);
}
