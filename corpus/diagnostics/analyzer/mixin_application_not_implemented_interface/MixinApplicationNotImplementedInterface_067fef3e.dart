class A {}

mixin M1 implements A {}

mixin M2 on A {}

class X = Object with M1, M2;
