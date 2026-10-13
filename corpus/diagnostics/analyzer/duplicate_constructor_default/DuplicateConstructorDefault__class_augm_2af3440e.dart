class A {
  A();
}

augment class A {
  A();
//^
// [diag.duplicateConstructorDefault] The unnamed constructor is already defined.
}
