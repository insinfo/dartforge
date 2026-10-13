class C {}

f(C? c1, C c2) {
  (c1 ?? c2).new;
//           ^^^
// [diag.undefinedGetter] The getter 'new' isn't defined for the type 'C'.
}
