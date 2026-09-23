enum E {
  v;
  factory E.foo({int x = 0});
//                     ^
// [diag.defaultValueInRedirectingFactoryConstructor][context 1] Default values aren't allowed in factory constructors that redirect to another constructor.
}

augment enum E {
  ;
  augment factory E.foo({int x}) = EBox.foo;
//                  ^^^
// [context 1] The redirecting factory is here.
}

extension type EBox(E it) implements E {
  EBox.foo({int x = 0}) : this(E.v);
}
