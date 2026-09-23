enum E {
  v
}

void f() {
  const E();
//      ^
// [diag.invalidReferenceToGenerativeEnumConstructor] Generative enum constructors can only be used to create an enum constant.
}
