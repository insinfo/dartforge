class C(int x) {
  C.named(int x);
}

augment class C {
  augment C.named(int x) : this(x);
}
