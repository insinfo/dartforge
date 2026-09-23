Future<int> foo() async {
  return '';
//       ^^
// [diag.returnOfInvalidTypeFromFunction] A value of type 'String' can't be returned from the function 'foo' because it has a return type of 'Future<int>'.
}
