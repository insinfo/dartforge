class A {
  int x = 0;
  const new();
//      ^^^
// [diag.constConstructorWithNonFinalField] Can't define a const constructor for a class with non-final fields.
}
