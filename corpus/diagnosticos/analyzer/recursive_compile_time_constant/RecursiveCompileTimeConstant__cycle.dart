const x = y + 1;
//    ^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
const y = x + 1;
//    ^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
