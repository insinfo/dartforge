// Callbacks the JavaScript event loop calls after main returns: the Dart
// microtasks, futures and timers they schedule run after each one, in order,
// and a Dart timer that schedules a JavaScript timer keeps the program going.
import 'dart:async';
import 'dart:js_interop';

@JS('setTimeout')
external void setTimeout(JSFunction f, int ms);

void main() {
  var n = 0;
  setTimeout((() {
    print('cb1');
    scheduleMicrotask(() => print('micro in cb1'));
    Future(() => print('future in cb1'));
    Timer(Duration(milliseconds: 5), () {
      print('dart timer from cb1');
      setTimeout((() => print('js timer from the dart timer')).toJS, 5);
    });
  }).toJS, 1);
  setTimeout((() {
    print('cb2 ${n++}');
    Future.microtask(() => print('micro in cb2 $n'));
  }).toJS, 40);
  print('end main');
}
