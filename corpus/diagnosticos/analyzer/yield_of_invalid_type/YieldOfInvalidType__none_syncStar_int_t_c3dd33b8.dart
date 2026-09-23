Iterable<String> f() sync* {
  yield 0;
//      ^
// [diag.yieldOfInvalidType] A yielded value of type 'int' must be assignable to 'String'.
}
