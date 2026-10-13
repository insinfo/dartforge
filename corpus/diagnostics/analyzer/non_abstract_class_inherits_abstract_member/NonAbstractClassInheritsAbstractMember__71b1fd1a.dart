// %before-language-feature: class-modifiers
class A {
  var a;
}
abstract class M {
  get a;
}
class B extends A with M {}
class C extends B {}
