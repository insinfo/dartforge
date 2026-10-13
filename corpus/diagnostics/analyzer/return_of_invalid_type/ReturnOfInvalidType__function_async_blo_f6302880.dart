Future<Null> f(void a) async {
  return a;
//       ^
// [diag.returnOfInvalidTypeFromFunction] A value of type 'void' can't be returned from the function 'f' because it has a return type of 'Future<Null>'.
}
