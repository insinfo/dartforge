// %before-language-feature: class-modifiers
abstract class M {}
abstract class A {}
abstract class I {
  m();
}
abstract class B = A with M implements I;
