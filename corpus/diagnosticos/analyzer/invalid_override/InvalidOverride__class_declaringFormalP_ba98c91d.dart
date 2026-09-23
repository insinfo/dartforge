class A {}
class B extends A {}

abstract class I {
  set foo(A value);
}

class C(covariant var B foo) implements I;
