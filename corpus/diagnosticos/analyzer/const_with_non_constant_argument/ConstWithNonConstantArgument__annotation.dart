class A {
  const A(int p);
}
var v = 42;
@A(v)
// ^
// [diag.constWithNonConstantArgument] Arguments of a constant creation must be constant expressions.
main() {
}
