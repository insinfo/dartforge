class A {
  num get g => 7;
}
class B	extends A {
//    ^
// [diag.invalidImplementationOverride] 'A.g' ('num Function()') isn't a valid concrete implementation of 'B.g' ('int Function()').
  int get g;
}
