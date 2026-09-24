const y = const C();
//    ^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
class C {
  const C() : x = y;
//      ^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
  final x;
}
