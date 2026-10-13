class A {
  const int v;
//^^^^^
// [diag.constInstanceField] Only static fields can be declared as const.
//          ^
// [diag.constNotInitialized] The constant 'v' must be initialized.
}
