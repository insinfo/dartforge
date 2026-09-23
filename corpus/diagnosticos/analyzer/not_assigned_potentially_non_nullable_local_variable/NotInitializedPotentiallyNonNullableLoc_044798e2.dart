void f(bool b) {
  int v;
  while (true) {
    if (b) {
      v = 0;
      break;
    } else {
      v = 0;
      break;
    }
    v;
//  ^^
// [diag.deadCode] Dead code.
  }
  v;
}
