class A {
  factory A([Object p = this]) => throw 0;
//                      ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
