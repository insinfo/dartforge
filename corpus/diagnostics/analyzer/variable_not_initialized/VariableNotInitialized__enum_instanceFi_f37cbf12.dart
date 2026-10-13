enum A {
  e;
  const int v;
//^^^^^
// [diag.constInstanceField] Only static fields can be declared as const.
//          ^
// [diag.constNotInitialized] The constant 'v' must be initialized.
// [diag.nonFinalFieldInEnum] Enums can only declare final fields.
}
