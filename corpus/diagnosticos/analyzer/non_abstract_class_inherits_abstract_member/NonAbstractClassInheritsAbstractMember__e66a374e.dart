// %before-language-feature: class-modifiers
class A {
  var a;
}
abstract class M {
  set a(dynamic v);
}
class B extends A with M {}
class C extends B {}
