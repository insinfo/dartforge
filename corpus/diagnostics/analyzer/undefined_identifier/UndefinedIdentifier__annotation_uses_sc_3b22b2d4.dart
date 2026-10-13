extension E<@Annotation.function(foo) @Annotation.type(B) T> on C {}
//                               ^^^
// [diag.undefinedIdentifier] Undefined name 'foo'.
// [diag.constWithNonConstantArgument] Arguments of a constant creation must be constant expressions.
class C {
  static void foo() {}
  static void B() {}
}
class B {}
class Annotation {
  const Annotation.function(void Function() f);
  const Annotation.type(Type t);
}
