class C {
  final x;
  const C() : x = y;
}
const y = const C();
const z = y;
