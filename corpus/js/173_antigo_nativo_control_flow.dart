// Convertido de tests/native/cases/control_flow.dart (fixture antigo do backend nativo).
bool effect(int n) { print(n); return true; }
void main() {
  var total = 0;
  for (var i = 0; i < 8; i++) {
    if (i == 2) { continue; }
    if (i == 6) { break; }
    total += i;
  }
  print(total);
  var n = 0;
  do { n++; if (n < 3) { continue; } print(n); } while (n < 4);
  while (n > 0) { n--; if (n == 2) { continue; } print(n); }
  print(false && effect(100)); print(true || effect(200));
  print(true && (false || effect(300)));
  var x = 4;
  { var inner = x + 1; { var x = inner; print(x); } }
  print(x);
}
