class A {
  factory A() { return this; }
//                     ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
