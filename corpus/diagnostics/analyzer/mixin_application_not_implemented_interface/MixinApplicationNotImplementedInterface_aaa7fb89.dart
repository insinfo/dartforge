abstract class A<T> {}
class B {}
mixin M<T> on A<T> {}
class C extends A<int> with M {}
