class C {
  const C();
  const factory C.named() {
//^^^^^
// [diag.constFactory] Only redirecting factory constructors can be declared to be 'const'.
    return const C();
  }
}
