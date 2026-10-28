// Dynamic calls with named arguments: the runtime's named paths stay in
// production and match the VM.

class Target {
  String f(int a, {int b = 1, String? c}) => '$a $b $c';
}

class Catcher {
  @override
  dynamic noSuchMethod(Invocation i) =>
      '${i.memberName} ${i.positionalArguments} '
      '${i.namedArguments.keys.toList()} ${i.namedArguments.values.toList()}';
}

void main() {
  final dynamic t = Target();
  print(t.f(1, b: 2, c: 'x'));
  print(t.f(3, c: 'y'));
  try {
    t.f(1, d: 4);
  } on NoSuchMethodError {
    print('nsm');
  }
  final dynamic c = Catcher();
  print(c.m(1, x: 2, y: 3));
  print(Function.apply((int a, {int b = 0}) => a + b, [1], {#b: 41}));
}
