class A<E> {
  final Map<String, List<E Function()>> x;
  const A([this.x = const <String, List<E Function()>>{}]);
//                                      ^
// [diag.invalidTypeArgumentInConstMap] Constant map literals can't use a type parameter in a type argument, such as 'E'.
}
