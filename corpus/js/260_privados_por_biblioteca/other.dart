import 'main.dart';

class Bar {
  void _f() {}
}

String describe(Bar bar) => bar._f.runtimeType.toString();

class C2 extends C1 {
  int _field = 99;
  int otherField() => _field;
}

Object? readDynamically(dynamic c) => c._field;
