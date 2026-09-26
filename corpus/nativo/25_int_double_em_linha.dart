// Membros de `int` e `double` em linha (`lower/intrinsecos.rs`): mesmo valor
// que o SDK, com os extremos (NaN, ±0, ±infinito, -2^63) e o `toInt` fora
// da faixa, que lança pelo membro do SDK.
void main() {
  const mn = -9223372036854775808;
  const mx = 9223372036854775807;
  for (final i in [0, 1, -1, 2, -2, 7, -7, mn, mx, mn + 1]) {
    print('$i: ${i.isEven} ${i.isOdd} ${i.isNegative} ${i.toDouble()} ${i.abs()} '
        '${i.toInt()} ${i.floor()} ${i.ceil()} ${i.round()} ${i.truncate()}');
  }
  final ds = [0.0, -0.0, 1.5, -1.5, 2.9, -2.9, double.nan, double.infinity, double.negativeInfinity,
      double.maxFinite, double.minPositive, 9.2233720368547748e18, -9.223372036854775808e18, 1e300];
  for (final d in ds) {
    final neg = d.isNegative;
    print('$d: ${d.isNaN} ${d.isInfinite} ${d.isFinite} $neg ${d.toDouble()} ${d.abs()} '
        '${(-d).abs()} ${(-d).isNegative}');
    for (final f in [() => d.toInt(), () => d.truncate()]) {
      try {
        print('  ${f()}');
      } on UnsupportedError catch (e) {
        print('  erro: ${e.message}');
      }
    }
  }
  // Num laço com acumulador (o caso do desempenho).
  var s = 0.0;
  var pares = 0;
  for (var i = -1000; i < 1000; i++) {
    s += i.toDouble() * 0.5;
    if (i.isEven) pares++;
    s += (i * 1.25).toInt() + i.abs();
  }
  print('$s $pares');
}
