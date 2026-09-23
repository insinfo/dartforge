extension type A({int x = 0}) {
  factory A.foo({int x});
}

augment extension type A {
  augment factory A.foo({int x = 0}) = A;
//                             ^
// [diag.defaultValueInRedirectingFactoryConstructor] Default values aren't allowed in factory constructors that redirect to another constructor.
}
