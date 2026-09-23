extension E1 on int { void foo() {} }
extension E on int? { void foo() {} }
extension E2 on int { void foo() {} }
void f() {
  0.foo();
//  ^^^
// [diag.ambiguousExtensionMemberAccessTwo] A member named 'foo' is defined in 'extension E1 on int' and 'extension E2 on int', and neither is more specific.
}
