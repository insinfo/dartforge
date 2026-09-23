import 'dart:ffi';

@Native<Void Function(Pointer<Void>)>(isLeaf: true)
external void myNative(Pointer<Void> buffer);

main() {
  final myStruct = Struct.create<MyStruct>();
  myNative(myStruct.arr.address.cast());
  myNative(myStruct.arr.address.cast<Void>());
}

final class MyStruct extends Struct {
  @Int8()
  external int value;
  @Array(2)
  external Array<Int8> arr;
}
