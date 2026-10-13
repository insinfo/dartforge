void NonType<T>() {}
f() {
  NonType<int>.named();
//             ^^^^^
// [diag.undefinedMethod] The method 'named' isn't defined for the type 'void Function()'.
}
