import 'dart:ffi';

@Native<Void Function(Pointer<Void>)>(isLeaf: true)
external void myNative(Pointer<Void> buffer);

main() {
  final myUnion = Union.create<MyUnion>();
  myNative(myUnion.arr.address.cast());
  myNative(myUnion.arr.address.cast<Void>());
}
final class MyUnion extends Union {
  @Int8()
  external int value;
  @Array(2)
  external Array<Int8> arr;
}
