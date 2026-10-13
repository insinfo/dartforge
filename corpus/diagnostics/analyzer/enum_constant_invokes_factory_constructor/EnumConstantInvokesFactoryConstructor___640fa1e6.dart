enum E {
  e1.primary(),
  e2;
//^^
// [diag.enumConstantInvokesFactoryConstructor] An enum value can't invoke a factory constructor.

  const E.primary();
  const factory E() = ET.named;
}

extension type const ET(E it) implements E {
  const ET.named() : this(E.e1);
}
