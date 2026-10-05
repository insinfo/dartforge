enum Pub { a, _priv }
enum _E { x, y }
enum _F { x, y }
void main() {
  print(Pub.values);
  print(_E.x.index);
  for (var e in _F.values) { print(e); }
}
