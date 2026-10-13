// %before-language-feature: class-modifiers
class M {}
abstract class A {
  m();
}
abstract class B = A with M;
