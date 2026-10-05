// @dart=2.19
var v = 1;
const k = 1;
void f(int x) {
  switch (x) {
    case v:
      break;
    case k + 1:
      break;
    case 1 ~/ 0:
      break;
  }
}
