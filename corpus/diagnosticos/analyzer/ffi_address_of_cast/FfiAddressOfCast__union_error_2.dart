import 'dart:ffi';

@Native<Void Function(Pointer<Void>)>()
external void myNonLeafNative(Pointer<Void> buffer);

main() {
  final myUnion = Union.create<MyUnion>();
  myNonLeafNative(myUnion.value.address.cast());
//                              ^^^^^^^
// [diag.addressPosition] The '.address' expression can only be used as argument to a leaf native external call.
}
final class MyUnion extends Union {
  @Int8()
  external int value;
  @Array(2)
  external Array<Int8> arr;
}
