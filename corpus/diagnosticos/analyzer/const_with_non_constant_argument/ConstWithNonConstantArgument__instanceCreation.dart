class A {
  const A(a);
}
f(p) { return const A(p); }
//                    ^
// [diag.constWithNonConstantArgument] Arguments of a constant creation must be constant expressions.
