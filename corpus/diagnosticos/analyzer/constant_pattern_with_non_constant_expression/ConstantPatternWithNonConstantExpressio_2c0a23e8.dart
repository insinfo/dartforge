void f(Object? x) {
  if (x case foo) {}
//           ^^^
// [diag.undefinedIdentifier] Undefined name 'foo'.
}
