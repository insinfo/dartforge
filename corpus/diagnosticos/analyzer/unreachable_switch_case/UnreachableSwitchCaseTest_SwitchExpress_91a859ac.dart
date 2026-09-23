int f(Object? x) {
  return switch (x) {
    Unresolved() => 0,
//  ^^^^^^^^^^
// [diag.undefinedClass] Undefined class 'Unresolved'.
    _ => -1,
  };
}
