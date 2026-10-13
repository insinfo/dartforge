import 'dart:ffi';
import 'dart:typed_data';

@Native<Void Function(Pointer<Void>)>()
external void myNonLeafNative(Pointer<Void> buffer);

main() {
  final buffer = Int8List(2);
  myNonLeafNative(buffer.address.cast());
//                       ^^^^^^^
// [diag.addressPosition] The '.address' expression can only be used as argument to a leaf native external call.
}
