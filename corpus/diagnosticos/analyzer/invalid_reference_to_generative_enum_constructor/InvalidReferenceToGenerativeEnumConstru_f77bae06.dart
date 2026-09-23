enum E {
  v
}

void f() {
  E.new;
//^^^^^
// [diag.invalidReferenceToGenerativeEnumConstructorTearoff] Generative enum constructors can't be torn off.
}
