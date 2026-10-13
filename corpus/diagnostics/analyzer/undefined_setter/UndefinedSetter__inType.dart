class A {}
f(a) {
  if(a is A) {
    a.m = 0;
//    ^
// [diag.undefinedSetter] The setter 'm' isn't defined for the type 'A'.
  }
}
