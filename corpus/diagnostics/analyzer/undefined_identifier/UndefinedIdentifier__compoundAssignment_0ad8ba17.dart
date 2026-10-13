set foo(int _) {}

void f() {
  foo += 0;
//^^^
// [diag.undefinedIdentifier] Undefined name 'foo'.
}
