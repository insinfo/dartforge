class A<E> {
  void m() {
    const <E>{};
//         ^
// [diag.invalidTypeArgumentInConstSet] Constant set literals can't use a type parameter in a type argument, such as 'E'.
  }
}
