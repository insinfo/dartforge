class A {}

class B extends A {
  B([super.a]);
//         ^
// [diag.superFormalParameterWithoutAssociatedPositional] No associated positional super constructor parameter.
}
