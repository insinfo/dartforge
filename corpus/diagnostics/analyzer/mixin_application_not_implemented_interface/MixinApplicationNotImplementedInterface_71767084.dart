class B<T> {}

mixin M1<T> implements B<T> {}
mixin M2<T> on B<T> {}

class A<T> with M1<T> {}
augment class A<T> with M2 {}
