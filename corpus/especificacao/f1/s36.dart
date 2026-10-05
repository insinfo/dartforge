class Annotation {
  const Annotation(dynamic d);
}
class C<@Annotation(foo) T> {
  static void foo() {}
}
void f<@Annotation(bar) T>(@Annotation(baz) int p) {}
@Annotation(qux)
void g() {}
@undef
void h() {}
@undef.x
void i() {}
class Foo {
  @Annotation(Bar)
  set Bar(int value) {}
}
class Bar {}
