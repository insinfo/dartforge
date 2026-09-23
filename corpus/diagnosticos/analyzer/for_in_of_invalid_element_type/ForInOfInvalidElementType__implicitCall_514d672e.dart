class C {
  void call(int a) {}
}
void foo(Iterable<C> iterable) {
  void Function(String) f;
  for (f in iterable) {
//          ^^^^^^^^
// [diag.forInOfInvalidElementType] The type 'Iterable<C>' used in the 'for' loop must implement 'Iterable' with a type argument that can be assigned to 'void Function(String)'.
    f;
  }
}
