// @dart=2.19
class Eq { const Eq(); bool operator ==(Object o) => true; }
void f(int x, int y, String s) {
  switch (x) {
    case 1: break;
    case y: break;
    case 'a': break;
  }
  switch (s) {
    case const Eq(): break;
  }
}
