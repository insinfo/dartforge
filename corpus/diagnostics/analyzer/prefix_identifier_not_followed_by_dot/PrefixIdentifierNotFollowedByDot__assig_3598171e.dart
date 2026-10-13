import 'dart:math' as p;

class A {
  var p;
}

class B extends A {
  void f() {
    p = 1;
//  ^
// [diag.prefixIdentifierNotFollowedByDot] The name 'p' refers to an import prefix, so it must be followed by '.'.
  }
}
