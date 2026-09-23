@Annotation(foo)
//          ^^^
// [diag.undefinedIdentifier] Undefined name 'foo'.
// [diag.constWithNonConstantArgument] Arguments of a constant creation must be constant expressions.
mixin M {
  static void foo() {}
}
class Annotation {
  const Annotation(dynamic d);
}
    