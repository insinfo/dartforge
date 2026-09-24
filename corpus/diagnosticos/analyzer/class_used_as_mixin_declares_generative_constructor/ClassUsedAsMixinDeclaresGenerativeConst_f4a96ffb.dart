// %before-language-feature: class-modifiers
class A {
  factory A() => throw 0;
}
class B extends Object with A {}
