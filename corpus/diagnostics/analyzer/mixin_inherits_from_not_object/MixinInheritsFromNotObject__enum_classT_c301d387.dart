// %before-language-feature: class-modifiers
class A {}
class B = Object with A;
enum E with B {
  v
}
