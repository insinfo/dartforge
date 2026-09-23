class A {
  A(int a);
}

class B extends A {
  B(dynamic super.a);
//                ^
// [diag.superFormalParameterTypeIsNotSubtypeOfAssociated] The type 'dynamic' of this parameter isn't a subtype of the type 'int' of the associated super constructor parameter.
}
