m() {
  bool x = true;
  if(x || false) {}
//     ^^^^^^^^
// [diag.deadCode] Dead code.
}
