class A<T> {
  A(T a);
}

class B extends A<int> {
  B(num super.a);
//            ^
// [diag.superFormalParameterTypeIsNotSubtypeOfAssociated] The type 'num' of this parameter isn't a subtype of the type 'int' of the associated super constructor parameter.
}
