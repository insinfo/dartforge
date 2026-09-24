// %before-language-feature: class-modifiers
abstract class M {
  m();
}
abstract class A {}
abstract class B = A with M;
