class C {
  const C.named();
  const factory();
}

augment class C {
  augment const factory() = C.named;
}
