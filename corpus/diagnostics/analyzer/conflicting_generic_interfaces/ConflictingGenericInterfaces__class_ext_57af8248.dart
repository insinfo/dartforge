class I<T> {}
class A implements I<Never> {}
class B implements I<Never> {}
class C extends A implements B {}
