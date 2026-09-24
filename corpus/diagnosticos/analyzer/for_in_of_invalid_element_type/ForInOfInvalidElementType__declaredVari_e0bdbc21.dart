f() {
  for (int i in <String>[]) {
//              ^^^^^^^^^^
// [diag.forInOfInvalidElementType] The type 'List<String>' used in the 'for' loop must implement 'Iterable' with a type argument that can be assigned to 'int'.
    i;
  }
}
