// Exercise eager batched table registration in parent and child isolates.
import 'dart:isolate';

abstract class Counter {
  int get value;
}

class Counter0 implements Counter {
  @override
  int get value => 1;
}

class Counter1 implements Counter {
  @override
  int get value => 2;
}

class Counter2 implements Counter {
  @override
  int get value => 3;
}

class Counter3 implements Counter {
  @override
  int get value => 4;
}

class Counter4 implements Counter {
  @override
  int get value => 5;
}

class Counter5 implements Counter {
  @override
  int get value => 6;
}

class Counter6 implements Counter {
  @override
  int get value => 7;
}

class Counter7 implements Counter {
  @override
  int get value => 8;
}

class Counter8 implements Counter {
  @override
  int get value => 9;
}

int total() {
  final counters = <Counter>[Counter0(), Counter1(), Counter2(), Counter3(), Counter4(), Counter5(), Counter6(), Counter7(), Counter8()];
  var result = 0;
  for (final counter in counters) {
    result += counter.value;
  }
  return result;
}

void child(SendPort reply) {
  reply.send(total());
}

Future<void> main() async {
  print('parent=${total()}');
  final messages = ReceivePort();
  await Isolate.spawn(child, messages.sendPort);
  print('child=${await messages.first}');
  messages.close();
}
