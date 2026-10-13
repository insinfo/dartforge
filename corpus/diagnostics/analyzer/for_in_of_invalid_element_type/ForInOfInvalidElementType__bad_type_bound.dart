class Foo<T extends Iterable<int>> {
  void method(T iterable) {
    for (String i in iterable) {
//                   ^^^^^^^^
// [diag.forInOfInvalidElementType] The type 'Iterable<int>' used in the 'for' loop must implement 'Iterable' with a type argument that can be assigned to 'String'.
      i;
    }
  }
}
