import 'dart:async';

// J03: the middle generation does not have the Zeta class; the live object
// (in `z`, which lives in the globals area) still belongs to it, and the next
// generation, which brings it back, must give it the same id.
var ticks = 0;

String rotulo() => 'vb';

void main() {
  print('main');
  Timer.periodic(Duration(milliseconds: 40), (t) {
    ticks += 1;
    print('${rotulo()} $ticks');
  });
}
