class C {
  const factory C();
//^^^^^
// [diag.constFactory] Only redirecting factory constructors can be declared to be 'const'.
//              ^
// [diag.factoryWithoutBody] A non-redirecting 'factory' constructor must have a body.
}
