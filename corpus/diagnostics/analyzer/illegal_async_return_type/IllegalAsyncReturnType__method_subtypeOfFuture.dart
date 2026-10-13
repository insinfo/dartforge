abstract class SubFuture<T> implements Future<T> {}
class C {
  SubFuture<int> m() async {
//^^^^^^^^^^^^^^
// [diag.illegalAsyncReturnType] Functions marked 'async' must have a return type which is a supertype of 'Future'.
    return 0;
  }
}
