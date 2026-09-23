class C {
  C();
  new ();
//^^^
// [diag.duplicateConstructorDefault] The unnamed constructor is already defined.
}
