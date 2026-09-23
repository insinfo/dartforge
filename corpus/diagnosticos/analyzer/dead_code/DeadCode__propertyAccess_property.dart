Never doNotReturn() => throw 0;

test() => doNotReturn().hashCode;
//                      ^^^^^^^^^
// [diag.deadCode] Dead code.
