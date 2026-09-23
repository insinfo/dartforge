// %before-language-feature: class-modifiers
abstract class M {
  m();
}
abstract class A {}
class B = A with M;
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'M.m'.
