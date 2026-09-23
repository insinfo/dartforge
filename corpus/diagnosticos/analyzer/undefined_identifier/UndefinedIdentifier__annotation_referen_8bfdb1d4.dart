@Annotation(foo)
//          ^^^
// [diag.undefinedIdentifier] Undefined name 'foo'.
// [diag.constWithNonConstantArgument] Arguments of a constant creation must be constant expressions.
class C {
  static void foo() {}
}
class Annotation {
  const Annotation(dynamic d);
}
    