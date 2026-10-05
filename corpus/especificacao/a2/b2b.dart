const c = 1;
final fi = 1;
class A { static const sc = 1; final int ff = 0; int get g => 0; void m() { ff = 1; g = 2; } }
void f(List<int> l) {
  const x = 0;
  final y = 0;
  x = 1;
  x++;
  --x;
  x += 1;
  for (x in l) {}
  y = 1;
  for (y in l) {}
  c = 2;
  c++;
  fi = 2;
  fi++;
  A.sc = 3;
  (x) = 1;
  print(y);
}
