class I<T> {}
typedef A = I<String>;
mixin M implements I<String> {}
class C = A with M;
