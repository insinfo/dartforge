f(x) sync* {
  yield await x;
//      ^^^^^
// [diag.awaitInWrongContext] The await expression can only be used in an async function.
}
