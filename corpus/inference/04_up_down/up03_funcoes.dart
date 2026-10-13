// R-UP-03: UP de tipos de função: parâmetros por DOWN, retorno por UP;
// formas diferentes dão Function; função e interface dão Object.
int f1(int x) => x;
double f2(num x) => 1.5;
int g(int x, int y) => x;
void h(String s) {}
void main(List<String> args) {
  var b = args.isEmpty;
  var p = /*@*/b ? f1 : f2;
  var q = /*@*/b ? f1 : g;
  var r = /*@*/b ? f1 : 1;
  var s = /*@*/b ? f1 : h;
  print([p, q, r, s]);
}
