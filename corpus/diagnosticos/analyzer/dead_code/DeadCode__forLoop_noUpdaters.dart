Never foo() => throw "Never";

test() {
  int i = 0;
  for (foo(); (i = 42) < 0;) {}
// [diag.deadCode][column 15][length 29] Dead code.
  return i;
}
