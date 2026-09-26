// Substitui `html/dartium/nativewrappers.dart` (sobreposição `sdk_nativo/`).
//
// Na VM, `NativeFieldWrapperClass1` tem um campo nativo invisível que o
// embedder lê e grava pela API C (`Dart_GetNativeInstanceField`/
// `Dart_SetNativeInstanceField`): o ponteiro do `File*`, do `Namespace*`,
// do lister de diretório… Aqui o campo é declarado: é o primeiro campo do
// layout de toda subclasse, e os natives do runtime
// (`crates/runtime/src/io_arquivos.rs`) o leem e gravam pela posição 0.

library nativewrappers;

base class NativeFieldWrapperClass1 {
  @pragma("vm:entry-point")
  int _campoNativo = 0;
}

base class NativeFieldWrapperClass2 extends NativeFieldWrapperClass1 {}

base class NativeFieldWrapperClass3 extends NativeFieldWrapperClass2 {}

base class NativeFieldWrapperClass4 extends NativeFieldWrapperClass3 {}

int _getNativeField(NativeFieldWrapperClass1 object) => object._campoNativo;
