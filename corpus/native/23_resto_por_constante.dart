// `a % c` com divisor constante em linha (`lower/operadores.rs`): o resto do
// Dart em [0, |c|), com negativos, divisor negativo, ±1 e os extremos de 64 bits.
void main() {
  const mn = -9223372036854775808;
  const mx = 9223372036854775807;
  for (final a in [0, 1, -1, 7, -7, 13, -13, mn, mx, mn + 1, 1000000007, -1000000007]) {
    print('$a: ${a % 3} ${a % -3} ${a % 1} ${a % -1} ${a % 2} ${a % -2} ${a % 10} ${a % 1024} ${a % mx} ${a % -mx}');
  }
  var s = 0;
  for (var i = -50; i < 50; i++) {
    s = s * 31 + i % 7 + (i % -5) * 3;
  }
  print(s);
}
