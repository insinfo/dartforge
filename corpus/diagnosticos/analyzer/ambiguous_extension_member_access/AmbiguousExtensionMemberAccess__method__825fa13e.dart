extension E1 on int? { void foo() {} }
extension E2 on int? { void foo() {} }
extension E on int { void foo() {} }
void f() {
  0.foo();
}
