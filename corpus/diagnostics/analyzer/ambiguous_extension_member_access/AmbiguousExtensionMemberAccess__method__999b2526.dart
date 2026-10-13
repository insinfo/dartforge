extension E on int? { void foo() {} }
extension E1 on int { void foo() {} }
extension E2 on int { void foo() {} }
extension E3 on int { void foo() {} }
void f() {
  0.foo();
//  ^^^
// [diag.ambiguousExtensionMemberAccessThreeOrMore] A member named 'foo' is defined in extension 'E1', extension 'E2', and extension 'E3', and none are more specific.
}
