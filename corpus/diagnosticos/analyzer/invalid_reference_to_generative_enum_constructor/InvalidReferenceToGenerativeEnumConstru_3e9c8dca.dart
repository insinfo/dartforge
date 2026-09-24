enum E {
  v.named();

  const E.named();
}

void f() {
  E.named();
//^^^^^^^
// [diag.invalidReferenceToGenerativeEnumConstructor] Generative enum constructors can only be used to create an enum constant.
}
