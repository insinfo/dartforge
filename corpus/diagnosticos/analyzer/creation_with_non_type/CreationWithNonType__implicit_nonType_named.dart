void NonType() {}
f() {
  NonType.named();
//        ^^^^^
// [diag.undefinedMethod] The method 'named' isn't defined for the type 'void Function()'.
}
