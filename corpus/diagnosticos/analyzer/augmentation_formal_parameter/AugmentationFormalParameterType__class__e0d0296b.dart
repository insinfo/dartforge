class A {
  A({int? n1});
}
class B extends A {
  B({n1});
  augment B({super.n1});
//                 ^^
// [diag.superFormalParameterTypeIsNotSubtypeOfAssociated] The type 'dynamic' of this parameter isn't a subtype of the type 'int?' of the associated super constructor parameter.
}
