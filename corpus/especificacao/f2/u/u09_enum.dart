enum _NaoUsado { a }
enum _E { usado, naoUsado }
enum _V { x, y }
enum Pub { a, _priv, b }
enum Ctor {
  a, b.n();
  const Ctor();
  const Ctor.n();
  const Ctor.naoUsado();
  factory Ctor.f() => a;
}
void main() {
  print(_E.usado);
  print(_V.values);
}
