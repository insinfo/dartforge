Never doNotReturn() => throw 0;

test() => doNotReturn() is int;
//        ^^^^^^^^^^^^^^^^^^^^
// [diag.unnecessaryTypeCheckTrue] Unnecessary type check; the result is always 'true'.
//                         ^^^^
// [diag.deadCode] Dead code.
