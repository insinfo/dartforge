set foo(int _) {}

void f() {
  ++foo;
//  ^^^
// [diag.undefinedIdentifier] Undefined name 'foo'.
}
