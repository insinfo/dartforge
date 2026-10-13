class S {
  int get foo => 0;
}

mixin M1 {
  int foo() => 0;
}

mixin M2 {
  int get foo => 0;
}

class C = S with M1, M2;
//    ^
// [diag.inconsistentInheritanceGetterAndMethod] 'foo' is inherited as a getter (from 'S') and also a method (from 'M1').
// [diag.inconsistentInheritanceGetterAndMethod] 'foo' is inherited as a getter (from 'M2') and also a method (from 'M1').
