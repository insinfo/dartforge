class C(int x) {
  C.named();
}

augment class C {
  augment C.named() : this.missing();
//                    ^^^^^^^^^^^^^^
// [diag.redirectGenerativeToMissingConstructor] The constructor 'C.missing' couldn't be found in 'C'.
}
