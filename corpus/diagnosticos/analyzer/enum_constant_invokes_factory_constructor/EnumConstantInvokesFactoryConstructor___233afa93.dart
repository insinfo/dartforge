enum E {
  e1,
  e2.named();
//   ^^^^^
// [diag.enumConstantInvokesFactoryConstructor] An enum value can't invoke a factory constructor.

  const E();
  const factory E.named() = ET.named;
}

extension type const ET(E it) implements E {
  const ET.named() : this(E.e1);
}
