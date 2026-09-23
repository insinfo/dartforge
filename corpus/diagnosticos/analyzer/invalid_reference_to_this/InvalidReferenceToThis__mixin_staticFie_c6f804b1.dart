mixin M {
  static late var f = this;
//                    ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
