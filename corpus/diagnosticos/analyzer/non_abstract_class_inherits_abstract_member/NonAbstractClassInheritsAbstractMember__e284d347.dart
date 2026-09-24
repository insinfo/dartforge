// %before-language-feature: class-modifiers
class A {
  String toString([String prefix = '']) => '${prefix}Hello';
}
class C {}
class B extends A with C {}
