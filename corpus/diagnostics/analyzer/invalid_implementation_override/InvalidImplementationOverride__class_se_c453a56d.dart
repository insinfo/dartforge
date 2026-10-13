class A {
  set c(int i) {}
}

class B extends A {
//    ^
// [diag.invalidImplementationOverrideSetter] The setter 'A.c' ('void Function(int)') isn't a valid concrete implementation of 'B.c' ('void Function(num)').
  set c(num i);
}
