class A {}

class B extends A {
  B({super.a});
//         ^
// [diag.superFormalParameterWithoutAssociatedNamed] No associated named super constructor parameter.
}
