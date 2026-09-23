class Foo<T> {
  Foo.bar();
}
void f() {
  new Foo.bar<int>.baz();
//    ^^^^^^^
// [diag.newWithNonType] The name 'bar' isn't a class.
}
