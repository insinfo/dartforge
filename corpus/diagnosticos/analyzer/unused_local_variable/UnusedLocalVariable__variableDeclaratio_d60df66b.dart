typedef Foo();
main() {
  var v;
  v ??= doSomething();
}
doSomething() => 42;
