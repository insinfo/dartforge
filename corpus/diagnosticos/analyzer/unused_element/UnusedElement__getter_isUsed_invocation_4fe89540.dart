abstract class A {
  String get _debugName;

  String toString() {
    return _debugName;
  }
}

class B extends A {
  @override
  String get _debugName => "B";
}

class C extends B {
  String get _debugName => "C";
}
