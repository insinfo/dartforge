// Replaces `html/dartium/nativewrappers.dart` (overlay `native_sdk/`).
//
// On the VM, `NativeFieldWrapperClass1` has an invisible native field that the
// embedder reads and writes through the C API (`Dart_GetNativeInstanceField`/
// `Dart_SetNativeInstanceField`): the pointer of the `File*`, of the
// `Namespace*`, of the directory lister… Here the field is declared: it is the
// first field of the layout of every subclass, and the runtime's natives
// (`crates/runtime/src/io_files.rs`) read and write it at position 0.

library nativewrappers;

base class NativeFieldWrapperClass1 {
  @pragma("vm:entry-point")
  int _nativeField = 0;
}

base class NativeFieldWrapperClass2 extends NativeFieldWrapperClass1 {}

base class NativeFieldWrapperClass3 extends NativeFieldWrapperClass2 {}

base class NativeFieldWrapperClass4 extends NativeFieldWrapperClass3 {}

int _getNativeField(NativeFieldWrapperClass1 object) => object._nativeField;
