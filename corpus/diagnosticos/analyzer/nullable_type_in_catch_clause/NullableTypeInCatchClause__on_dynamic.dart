class A {}
f() {
  try {
  } on dynamic {
//     ^^^^^^^
// [diag.nullableTypeInCatchClause] A potentially nullable type can't be used in an 'on' clause because it isn't valid to throw a nullable expression.
  }
}
