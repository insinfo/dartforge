UnknownType getValue() => UnknownType();
class A {
  factory A() {
    foo();
    return throw 0;
  }
  static baz() => X();
  static var f = Y();
  var g = Z();
}
