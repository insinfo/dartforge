class A {}

class B extends A {
  B({required super.a});
//                  ^
// [diag.superFormalParameterWithoutAssociatedNamed] No associated named super constructor parameter.
}
