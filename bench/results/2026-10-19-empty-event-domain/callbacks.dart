import 'dart:async';
void main() {
  print('before');
  scheduleMicrotask(() => print('microtask'));
  Timer.run(() => print('timer'));
}
