Stream<String> f() async* {
  yield 0;
//      ^
// [diag.yieldOfInvalidType] A yielded value of type 'int' must be assignable to 'String'.
}
