void f() {
  void _() {}
//^^^^^^^^^^^
// [diag.deadCode] Dead code.
  int _(int _) => 42;
//^^^^^^^^^^^^^^^^^^^
// [diag.deadCode] Dead code.
  String _(int _) => "42";
//^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.deadCode] Dead code.
}
