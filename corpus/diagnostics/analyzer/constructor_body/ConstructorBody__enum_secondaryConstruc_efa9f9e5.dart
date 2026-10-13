enum E {
  v;
  const E();
  const factory E.named() {
//^^^^^
// [diag.constFactory] Only redirecting factory constructors can be declared to be 'const'.
    return v;
  }
}
