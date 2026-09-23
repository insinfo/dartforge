class A {
  A([int x = 0]);
  factory A.foo([int x = 0]);
//                     ^
// [diag.defaultValueInRedirectingFactoryConstructor][context 1] Default values aren't allowed in factory constructors that redirect to another constructor.
}

augment class A {
  augment factory A.foo([int x]) = A;
//                  ^^^
// [context 1] The redirecting factory is here.
}
