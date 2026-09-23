// R-FLU-07: variável escrita dentro de closure não é promovível; lida em
// closure, a promoção vale se não houver escrita depois.
void f(int? x, int? y, int? z) {
  if (x != null) {
    () {
      print(/*@*/x);
    };
  }
  void Function() g = () {
    y = null;
  };
  if (y != null) print(/*@*/y);
  if (z != null) {
    () {
      print(/*@*/z);
    };
    z = null;
  }
  g();
}

void main() => f(1, 2, 3);
