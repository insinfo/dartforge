import 'dart:async';
void f() {
  final c = Completer<int?>();
  c.complete();
  c.complete(null);
}
