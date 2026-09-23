class A {
  final int v;
//          ^
// [diag.finalNotInitialized] The final variable 'v' must be initialized.

  factory A() => throw 0;
}
