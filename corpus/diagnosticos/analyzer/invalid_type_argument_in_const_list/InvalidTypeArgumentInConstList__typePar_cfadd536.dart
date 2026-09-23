class A<E> {
  void m() {
    const <E>[];
//         ^
// [diag.invalidTypeArgumentInConstList] Constant list literals can't use a type parameter in a type argument, such as 'E'.
  }
}
