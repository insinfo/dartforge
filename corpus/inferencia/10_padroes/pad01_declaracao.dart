// R-PAD-01: declaração com padrão: os tipos das variáveis vêm do valor.
void main() {
  var (a, b) = (1, 'x');
  final [x, y] = [1, 2.5];
  var {'k': v} = {'k': 1.5};
  var (n: z) = (n: true);
  var (p, q: r) = (1, q: [1]);
  print([/*@*/a, /*@*/b, /*@*/x, /*@*/y, /*@*/v, /*@*/z, /*@*/p, /*@*/r]);
}
