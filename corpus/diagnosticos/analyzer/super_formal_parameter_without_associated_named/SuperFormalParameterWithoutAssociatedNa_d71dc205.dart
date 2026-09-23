class A {}

class B extends A {
  B({required super.a}) : super();
//                  ^
// [diag.superFormalParameterWithoutAssociatedNamed] No associated named super constructor parameter.
}
