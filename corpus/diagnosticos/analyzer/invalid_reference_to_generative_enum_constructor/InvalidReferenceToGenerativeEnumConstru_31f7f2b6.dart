enum E {
  v;
//^
// [diag.enumConstantInvokesFactoryConstructor] An enum value can't invoke a factory constructor.

  const factory E() = E.named;
//                    ^^^^^^^
// [diag.invalidReferenceToGenerativeEnumConstructor] Generative enum constructors can only be used to create an enum constant.
  const E.named();
}
