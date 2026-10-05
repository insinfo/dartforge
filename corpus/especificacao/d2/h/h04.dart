class A {
  A();
  augment const A();
}
enum E {
  v;
  factory E.named() => v;
}
augment enum E {
  ;
  augment E.named();
}
