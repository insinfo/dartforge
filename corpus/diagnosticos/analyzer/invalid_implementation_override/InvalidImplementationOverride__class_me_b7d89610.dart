class A {
  int add(int a) => a;
}
class B	extends A {
//    ^
// [diag.invalidImplementationOverride] 'A.add' ('int Function(int)') isn't a valid concrete implementation of 'B.add' ('int Function(num)').
  int add(num a);
}
