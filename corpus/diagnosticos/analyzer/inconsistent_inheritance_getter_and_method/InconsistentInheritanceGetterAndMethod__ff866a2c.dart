class S {
  int get foo => 0;
}

mixin M {
  int foo() => 0;
}

class C = S with M;
//    ^
// [diag.inconsistentInheritanceGetterAndMethod] 'foo' is inherited as a getter (from 'S') and also a method (from 'M').
