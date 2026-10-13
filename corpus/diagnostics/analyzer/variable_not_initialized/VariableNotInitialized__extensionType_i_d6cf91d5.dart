extension type A(int it) {
  A.named(int it);
}

augment extension type A {
  augment external A.named(int it);
}
