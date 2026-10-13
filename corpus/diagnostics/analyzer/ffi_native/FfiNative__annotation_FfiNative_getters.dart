import 'dart:ffi';

base class NativeFieldWrapperClass1 {}

base class Paragraph extends NativeFieldWrapperClass1 {
  @Native<Double Function(Pointer<Void>)>(symbol: 'Paragraph::ideographicBaseline', isLeaf: true)
  external double get ideographicBaseline;

  @Native<Void Function(Pointer<Void>, Double)>(symbol: 'Paragraph::ideographicBaseline', isLeaf: true)
  external set ideographicBaseline(double d);
}
