class Box {
  int value;
  Box(this.value);
}

final shared = Box(1);
int mutate() {
  shared.value = 9;
  return 1;
}
int combine(int captured) => mutate() + captured;
int Function() callback = () => 0;
int combineCallback(int captured) => callback() + captured;

late String uninitializedTop;
String trace = '';
class Probe {
  final String label;
  final int value;
  final bool fails;
  Probe(this.label, this.value, {this.fails = false});
  int get read {
    trace += label;
    if (fails) throw StateError('argument');
    return value;
  }
}
int effect() {
  trace += 'body';
  return 1;
}
int afterEffect(int value) => effect() + value;
int reverse(int first, int second) => second * 100 + first;
int choose(bool enabled, int value) => enabled ? value : 0;
int chooseAfterEffect(int value) => effect() > 0 ? value : 0;
int drop(int first, int unused) => first;
int ignoreString(String unused) => 7;

void main() {
  final local = shared;
  print(combine(local.value));
  print(local.value);
  var captured = 1;
  callback = () {
    captured = 9;
    return 1;
  };
  print(combineCallback(captured));
  print(captured);

  final left = Probe('left', 2);
  final right = Probe('right', 3);
  trace = '';
  print(afterEffect(left.read));
  print(trace);
  trace = '';
  final failed = Probe('throw', 0, fails: true);
  try {
    afterEffect(failed.read);
  } on StateError {
    print(trace);
  }
  trace = '';
  print(reverse(left.read, right.read));
  print(trace);
  trace = '';
  print(choose(false, left.read));
  print(trace);
  trace = '';
  print(chooseAfterEffect(right.read));
  print(trace);
  trace = '';
  print(drop(7, right.read));
  print(trace);
  print(afterEffect(4));
  late String uninitializedLocal;
  try {
    ignoreString(uninitializedLocal);
    print('missing local late error');
  } catch (_) {
    print('local late error');
  }
  try {
    ignoreString(uninitializedTop);
    print('missing top late error');
  } catch (_) {
    print('top late error');
  }
}
