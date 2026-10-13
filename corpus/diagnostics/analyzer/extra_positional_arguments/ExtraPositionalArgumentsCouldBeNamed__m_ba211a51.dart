extension type const A(int it) {}

@A(0, 1)
//    ^
// [diag.extraPositionalArguments] Too many positional arguments: 1 expected, but 2 found.
void f() {}
