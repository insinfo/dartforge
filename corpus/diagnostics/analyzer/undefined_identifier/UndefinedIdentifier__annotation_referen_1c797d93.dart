extension E<@Annotation(foo) T> on T {
//                      ^^^
// [diag.undefinedIdentifier] Undefined name 'foo'.
// [diag.constWithNonConstantArgument] Arguments of a constant creation must be constant expressions.
  static void foo() {}
}
class Annotation {
  const Annotation(dynamic d);
}
