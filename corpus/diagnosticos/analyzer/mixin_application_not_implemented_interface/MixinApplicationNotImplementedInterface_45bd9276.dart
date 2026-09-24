mixin M1 {}
mixin M2 on M1 {}

class A with M1 {}
augment class A with M2 {}
