class C {
  @override
  bool operator ==(dynamic other) => false;
//              ^^
// [diag.nonNullableEqualsParameter] The parameter type of '==' operators should be non-nullable.
}
class D extends C {
  @override
  bool operator ==(other) => false;
//              ^^
// [diag.nonNullableEqualsParameter] The parameter type of '==' operators should be non-nullable.
}
