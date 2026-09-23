f(bool e) {
  for (var id in e) {
//               ^
// [diag.forInOfInvalidType] The type 'bool' used in the 'for' loop must implement 'Iterable'.
    id;
  }
}
