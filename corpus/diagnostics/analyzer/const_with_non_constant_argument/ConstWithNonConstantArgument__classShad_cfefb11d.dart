class Annotation {
  const Annotation(Object obj);
}

class Bar {}

class Foo {
  @Annotation(Bar)
//            ^^^
// [diag.undefinedIdentifier] Undefined name 'Bar'.
// [diag.constWithNonConstantArgument] Arguments of a constant creation must be constant expressions.
  set Bar(int value) {}
}
