// Convertido de tests/native/cases/edge_cfg.dart (fixture antigo do backend nativo).
// diverge-ddc: overflow de int em 64 bits só existe na VM
// Fixture original DartForge; saída conferida com Dart VM 3.6.2 e AOT LLVM O0/O2.
int mark(int n) { print(n); return n; }
bool check(int n) { print(n); return n < 3; }
bool combine(bool a, bool b) { return a == b; }
int nested(int limit) {
  var limit = 4;
  var total = 0;
  for (var i=0; i<limit; i++) {
    var j=0;
    do {
      j++;
      if (j == 1) { continue; }
      while (true) {
        total += i + j;
        if (i == 2) { return total; }
        break;
      }
      if (j == 3) { break; }
    } while (check(j));
    if (i == 0) { continue; }
  }
  return total;
}
void done() { return print(42); }
void main() {
  print(nested(99));
  print(combine(check(1) && check(4), check(2) || check(8)));
  print(mark(7) == true);
  print(false != mark(8));
  var n=1073741824; n=n*n; n=n*8;
  print(n); print(-n); print(n-1); print(n*2);
  done();
}
