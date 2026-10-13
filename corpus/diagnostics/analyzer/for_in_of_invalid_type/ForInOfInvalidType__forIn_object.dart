f(Object e) async {
  for (var id in e) {
//               ^
// [diag.forInOfInvalidType] The type 'Object' used in the 'for' loop must implement 'Iterable'.
    id;
  }
}
