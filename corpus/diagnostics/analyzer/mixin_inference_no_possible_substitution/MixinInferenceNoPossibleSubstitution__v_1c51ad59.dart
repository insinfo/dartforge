class A<T> {}

mixin M<T> on A<T> {}

class X extends A<int> with M {}
