void foo<T extends num>(T a) {}
void bar() {
  foo<int>;
}
