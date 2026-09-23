import 'dart:ffi';

final class MyStruct extends Struct {
  @Uint8()
  external int myField;
}

void main() {
  final pointer = Pointer<MyStruct>.fromAddress(0);
  pointer.refWithFinalizer(nullptr).myField = 1;
}
