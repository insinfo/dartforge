void f() {
  never..(_) => 1;
//     ^^^^^^^^^^^
// [diag.deadCode] Dead code.
}

Never get never => throw 0;
