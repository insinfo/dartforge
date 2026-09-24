Never doNotReturn() => throw 0;

test() => doNotReturn() as int;
//                         ^^^^
// [diag.deadCode] Dead code.
