import 'dart:async';

var ticks = 0;

String rotulo() => 'v1';

void main() {
  print('main');
  Timer.periodic(Duration(milliseconds: 40), (t) {
    ticks += 1;
    print('${rotulo()} $ticks');
    if (rotulo() == 'v2' || ticks > 2000) t.cancel();
  });
}
