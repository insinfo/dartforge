void nonPrefix() {}
f() {
  nonPrefix.Class();
//          ^^^^^
// [diag.undefinedMethod] The method 'Class' isn't defined for the type 'void Function()'.
}
