enum E {
  v;
  abstract int foo;
  augment int get foo => 0;
  augment void set foo(int _) {}
}
