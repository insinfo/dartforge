f(Object e) async {
  await for (var id in e) {
//                     ^
// [diag.forInOfInvalidType] The type 'Object' used in the 'for' loop must implement 'Stream'.
    id;
  }
}
