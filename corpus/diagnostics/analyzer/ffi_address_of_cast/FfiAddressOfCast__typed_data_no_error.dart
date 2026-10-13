import 'dart:ffi';
import 'dart:typed_data';

@Native<Void Function(Pointer<Void>)>(isLeaf: true)
external void myNative(Pointer<Void> buffer);

main() {
  final buffer = Int8List(2);
  myNative(buffer.address.cast());
}
