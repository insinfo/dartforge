class Annotation {
  const Annotation(Object obj);
}
class Bar {}
class Foo {
  @Annotation(Bar)
  set Bar(int value) {}
}
