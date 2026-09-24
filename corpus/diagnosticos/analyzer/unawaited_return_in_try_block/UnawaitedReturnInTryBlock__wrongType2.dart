Future<int>? foo() async {
  return Future<Null>.value(null);
//       ^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.returnOfInvalidTypeFromFunction] A value of type 'Future<Null>' can't be returned from the function 'foo' because it has a return type of 'Future<int>?'.
}
