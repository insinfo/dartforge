class A {
  A([int? p1]);
}
class B extends A {
  B([p1]);
  augment B([super.p1]);
//                 ^^
// [diag.superFormalParameterTypeIsNotSubtypeOfAssociated] The type 'dynamic' of this parameter isn't a subtype of the type 'int?' of the associated super constructor parameter.
}
