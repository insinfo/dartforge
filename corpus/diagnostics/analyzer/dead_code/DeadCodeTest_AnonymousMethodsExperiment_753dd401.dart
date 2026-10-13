void f() {
  never..=> 1;
//     ^^^^^^^
// [diag.deadCode] Dead code.
}

Never get never => throw 0;
