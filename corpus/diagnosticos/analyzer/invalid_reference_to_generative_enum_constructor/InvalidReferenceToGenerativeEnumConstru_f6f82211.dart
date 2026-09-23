enum E {
  v;

  const factory E.named() = E;
//                          ^
// [diag.invalidReferenceToGenerativeEnumConstructor] Generative enum constructors can only be used to create an enum constant.
  const E();
}
