class A {
  int add() => 7;
}
class B	extends A {
//    ^
// [diag.invalidImplementationOverride] 'A.add' ('int Function()') isn't a valid concrete implementation of 'B.add' ('int Function([int, int])').
  int add([int a = 0, int b = 0]);
}
