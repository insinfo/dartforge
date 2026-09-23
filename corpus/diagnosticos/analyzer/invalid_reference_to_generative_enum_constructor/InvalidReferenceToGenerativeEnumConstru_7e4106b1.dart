enum E {
  v();

  factory E.named() => v;
}

void f() {
  E.named;
  E.named();
}
