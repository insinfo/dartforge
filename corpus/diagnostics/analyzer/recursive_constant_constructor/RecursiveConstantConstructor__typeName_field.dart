class A {
  const A();
//      ^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
  final m = const A();
}
