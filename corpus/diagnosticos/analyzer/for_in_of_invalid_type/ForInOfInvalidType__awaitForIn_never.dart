f(Never e) async {
  await for (var id in e) {
// [diag.deadCode][column 14][length 26] Dead code.
    id;
  }
}
