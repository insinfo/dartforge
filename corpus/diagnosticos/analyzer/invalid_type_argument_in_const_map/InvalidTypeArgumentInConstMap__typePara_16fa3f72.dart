class A<E> {
  void m() {
    const <String, List<E Function()>>{};
//                      ^
// [diag.invalidTypeArgumentInConstMap] Constant map literals can't use a type parameter in a type argument, such as 'E'.
  }
}
