class A {
  const A({int x = 0});
}
class B extends A {
  const B() : super(0);
//                  ^
// [diag.extraPositionalArgumentsCouldBeNamed] Too many positional arguments: 0 expected, but 1 found.
}
