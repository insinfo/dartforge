class A<T> {}

mixin M<T> on A<T> {}

class B<T> implements A<T> {}

class C<T> = B<T> with M<T>;
