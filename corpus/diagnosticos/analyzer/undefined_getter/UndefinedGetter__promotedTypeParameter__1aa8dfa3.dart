void f<X extends num, Y extends X>(Y y) {
  if (y is int) {
    y.isEven;
//    ^^^^^^
// [diag.undefinedGetter] The getter 'isEven' isn't defined for the type 'Y'.
  }
}
