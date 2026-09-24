import 'dart:ffi';

@Native<Void Function(Pointer<Void>)>(isLeaf: true)
external void myNative(Pointer<Void> buffer);

main() {
  final myStruct = Struct.create<MyStruct>();
  myNative(myStruct.arr[0].address.cast());
}

final class MyStruct extends Struct {
  @Array(2)
  external Array<Int8> arr;
}

extension on Array<Int8> {
  int operator [](int index) => 0;
}
