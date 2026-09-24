f() {
  for (const x in [0, 1, 2]) {
//     ^^^^^
// [diag.forInWithConstVariable] A for-in loop variable can't be a 'const'.
    print(x);
  }
}
