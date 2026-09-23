void nonPrefix() {}
f() {
  nonPrefix.Class<int>.named();
//          ^^^^^
// [diag.undefinedGetter] The getter 'Class' isn't defined for the type 'void Function()'.
}
