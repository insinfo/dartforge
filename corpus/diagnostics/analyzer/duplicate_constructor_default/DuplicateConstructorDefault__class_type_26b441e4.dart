class C {
  factory C() => throw 0;
  factory () => throw 0;
//^^^^^^^
// [diag.duplicateConstructorDefault] The unnamed constructor is already defined.
}
