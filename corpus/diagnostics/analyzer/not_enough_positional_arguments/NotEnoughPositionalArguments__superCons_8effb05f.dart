class A {
  const A.named(int p);
}
class B extends A {
  const B() : super.named();
//                        ^
// [diag.notEnoughPositionalArgumentsNameSingular] 1 positional argument expected by 'named', but 0 found.
}
