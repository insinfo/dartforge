void f(int x) {
  switch (x) {
    case int() || 0:
//             ^^^^
// [diag.deadCode] Dead code.
      break;
  }
}
