// %before-language-feature: class-modifiers
class M {}
abstract class A {
  m();
}
class B = A with M;
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'A.m'.
