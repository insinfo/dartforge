import 'dart:ffi';
import 'dart:typed_data';

@Native<Void Function(Pointer<Void>)>(isLeaf: true)
external void myNative(Pointer<Void> buffer);

main() {
  final buffer = Int8List(2);
  myNative(buffer[0].address.cast());
}

extension on Int8List {
  int operator [](int index) => 0;
}
