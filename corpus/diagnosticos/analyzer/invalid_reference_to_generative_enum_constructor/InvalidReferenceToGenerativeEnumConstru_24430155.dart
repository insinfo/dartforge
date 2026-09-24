enum E {
  v.named();

  const E.named();
}

void f() {
  E.named;
//^^^^^^^
// [diag.invalidReferenceToGenerativeEnumConstructorTearoff] Generative enum constructors can't be torn off.
}
