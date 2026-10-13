class A {
  A([int a = 0]);
  factory A.redirect([int a]);
  augment factory A.redirect([int a]) = A;
}
