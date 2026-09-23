class A {}
mixin M {}
typedef B = A;
class C = A with M implements B;
