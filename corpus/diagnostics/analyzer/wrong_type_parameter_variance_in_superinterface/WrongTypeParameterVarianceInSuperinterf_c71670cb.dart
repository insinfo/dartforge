class A<X> {}
mixin M {}
class B<X> = Object with M implements A<X>;
