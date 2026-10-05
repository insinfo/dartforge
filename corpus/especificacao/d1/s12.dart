const A2 = A3 + 1;
const A3 = A2 + 1;
class C0 {
  static const X = const C1();
}
class C1 {
  const C1() : x = C0.X;
  final x;
}
