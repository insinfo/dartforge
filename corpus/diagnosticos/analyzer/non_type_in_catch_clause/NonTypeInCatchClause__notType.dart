var T = 0;
f() {
  try {
  } on T catch (e) {
//     ^
// [diag.nonTypeInCatchClause] The name 'T' isn't a type and can't be used in an on-catch clause.
    e;
  }
}
