class A<E> {
  void m() {
    const <void Function(List<E>), String>{};
//                            ^
// [diag.invalidTypeArgumentInConstMap] Constant map literals can't use a type parameter in a type argument, such as 'E'.
  }
}
