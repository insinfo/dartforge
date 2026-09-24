class A {}
class B extends A {}

class C {
  /// Not covariant-by-declaration here.
  void foo(B b) {}
}

abstract class I {
  /// Is covariant-by-declaration here.
  void foo(covariant A a);
}

/// Is covariant-by-declaration here.
class D extends C implements I {}
