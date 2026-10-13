class C {}

f(C? c) {
  c..new;
//   ^^^
// [diag.undefinedGetter] The getter 'new' isn't defined for the type 'C?'.
}
