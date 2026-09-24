Future<int> f(Future<String> a) async {
  return a;
//       ^
// [diag.returnOfInvalidTypeFromFunction] A value of type 'Future<String>' can't be returned from the function 'f' because it has a return type of 'Future<int>'.
}
