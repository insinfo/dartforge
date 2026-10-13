class A<T> {
  A(T a);
}

class B extends A<num> {
  B(int super.a);
}
