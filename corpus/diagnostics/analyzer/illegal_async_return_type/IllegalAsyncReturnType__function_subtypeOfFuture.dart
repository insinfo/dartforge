abstract class SubFuture<T> implements Future<T> {}
SubFuture<int> f() async {
// [diag.illegalAsyncReturnType][column 1][length 14] Functions marked 'async' must have a return type which is a supertype of 'Future'.
  return 0;
}
