class C {
  factory C({int a}) => C._();
//               ^
// [diag.missingDefaultValueForParameter] The parameter 'a' can't have a value of 'null' because of its type, but the implicit default value is 'null'.
  C._();
}
