enum E {
  v
}

void f() {
  new E();
//    ^
// [diag.invalidReferenceToGenerativeEnumConstructor] Generative enum constructors can only be used to create an enum constant.
}
