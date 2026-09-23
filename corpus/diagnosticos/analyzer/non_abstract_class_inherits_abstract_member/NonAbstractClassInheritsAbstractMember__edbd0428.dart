// %before-language-feature: class-modifiers
class A {
  m() {}
}
abstract class M {
  m();
}
class B extends A with M {}
class C extends B {}
