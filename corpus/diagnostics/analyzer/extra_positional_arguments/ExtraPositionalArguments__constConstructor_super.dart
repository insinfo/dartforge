class A {
  const A();
}
class B extends A {
  const B() : super(0);
//                  ^
// [diag.extraPositionalArguments] Too many positional arguments: 0 expected, but 1 found.
}
