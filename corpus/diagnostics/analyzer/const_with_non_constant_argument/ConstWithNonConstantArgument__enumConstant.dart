var a = 42;

enum E {
  v(a);
//  ^
// [diag.constWithNonConstantArgument] Arguments of a constant creation must be constant expressions.
  const E(_);
}
