import 'package:meta/meta.dart';

class A {
  @experimental
  A() {
    foo();
  }

  @experimental
  void foo() {}
}
