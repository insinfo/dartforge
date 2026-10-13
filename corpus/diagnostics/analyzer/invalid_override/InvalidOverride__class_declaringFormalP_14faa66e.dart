class A {}
class B extends A {}

class C(covariant var A foo);
class D(var B foo) implements C;
