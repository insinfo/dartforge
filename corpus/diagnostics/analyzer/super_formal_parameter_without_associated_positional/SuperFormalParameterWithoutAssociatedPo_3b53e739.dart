class A {
  A(int a);
}

class B extends A {
  B(super.a, super.b);
//                 ^
// [diag.superFormalParameterWithoutAssociatedPositional] No associated positional super constructor parameter.
}
