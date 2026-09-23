class C {}
f(p) {
  f(C.m);
//    ^
// [diag.undefinedGetter] The getter 'm' isn't defined for the type 'C'.
}