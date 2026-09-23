class A {
  const A(int p);
}
class B extends A {
  const B() : super();
//                  ^
// [diag.notEnoughPositionalArgumentsNameSingular] 1 positional argument expected by 'A.new', but 0 found.
}
