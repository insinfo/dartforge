import 'dart:ffi';
main() {
  DynamicLibrary.open('dontcare')
      .lookup<NativeFunction<Void Function(Pointer<Opaque>)>>('dontcare')
      .asFunction<void Function(Pointer<Opaque>)>(isLeaf: true);
}
