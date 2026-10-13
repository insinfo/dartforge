class A {
  A({required int a});
}

class B extends A {
  B({required num super.a});
//                      ^
// [diag.superFormalParameterTypeIsNotSubtypeOfAssociated] The type 'num' of this parameter isn't a subtype of the type 'int' of the associated super constructor parameter.
}
