extension type E(int i) {
  abstract int foo;
  augment int get foo => 0;
  augment set foo(int _) {}
}
