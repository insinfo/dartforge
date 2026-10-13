enum E {
  v;
  factory E.foo([int x]);
}

augment enum E {
  ;
  augment factory E.foo([int x = 0]) = EBox.foo;
//                             ^
// [diag.defaultValueInRedirectingFactoryConstructor] Default values aren't allowed in factory constructors that redirect to another constructor.
}

extension type EBox(E it) implements E {
  EBox.foo([int x = 0]) : this(E.v);
}
