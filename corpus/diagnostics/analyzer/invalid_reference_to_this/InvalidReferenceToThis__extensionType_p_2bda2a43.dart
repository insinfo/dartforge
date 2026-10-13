extension type E(int it) {
  this : it = this.hashCode;
//       ^^
// [diag.fieldInitializedInParameterAndInitializer] Fields can't be initialized in both the parameter list and the initializers.
//            ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
