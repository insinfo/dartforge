Future<int> f(Future<Future<int>> a) async {
  return a;
//       ^
// [diag.returnOfInvalidTypeFromFunction] A value of type 'Future<Future<int>>' can't be returned from the function 'f' because it has a return type of 'Future<int>'.
}
