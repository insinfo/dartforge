m() {
  bool x = true;
  if(!x) {}
//       ^^
// [diag.deadCode] Dead code.
}
