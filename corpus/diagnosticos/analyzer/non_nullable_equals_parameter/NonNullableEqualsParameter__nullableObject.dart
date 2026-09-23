class C {
  @override
  bool operator ==(Object? other) => false;
//              ^^
// [diag.nonNullableEqualsParameter] The parameter type of '==' operators should be non-nullable.
}
