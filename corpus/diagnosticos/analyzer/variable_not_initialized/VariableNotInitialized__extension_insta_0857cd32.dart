extension A on int {
  const int v;
//^^^^^
// [diag.constInstanceField] Only static fields can be declared as const.
//          ^
// [diag.constNotInitialized] The constant 'v' must be initialized.
// [diag.extensionDeclaresInstanceField] Extensions can't declare instance fields.
}
