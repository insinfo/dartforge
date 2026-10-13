Future<void> f() async {
  return 0;
//       ^
// [diag.returnOfInvalidTypeFromFunction] A value of type 'int' can't be returned from the function 'f' because it has a return type of 'Future<void>'.
}
