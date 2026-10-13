class C {
  final int? x;
  const C({this.x}) : assert(x == null || x >= 0);
}
const c = const C();
