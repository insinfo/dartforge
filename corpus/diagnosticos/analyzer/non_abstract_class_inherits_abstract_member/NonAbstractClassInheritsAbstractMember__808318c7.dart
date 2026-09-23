// %before-language-feature: class-modifiers
class A {
  noSuchMethod(v) => '';
}
class B extends Object with A {
  m(p);
}
