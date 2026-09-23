import 'package:meta/meta.dart';

class A {
  A({@experimental int a = 0}) {}
  A.named() : this(a: 0);
}
