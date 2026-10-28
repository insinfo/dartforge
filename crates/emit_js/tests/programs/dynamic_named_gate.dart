// Without a dynamic call that passes named arguments, the runtime's named
// paths are folded away in production; failed dynamic calls, noSuchMethod
// and the invocation's (empty) named arguments still match the VM.

class Target {
  int add(int a, [int b = 0]) => a + b;
  String greet(String name) => 'hi $name';
}

class Catcher {
  @override
  dynamic noSuchMethod(Invocation i) =>
      '${i.memberName} ${i.positionalArguments} ${i.namedArguments.isEmpty} '
      '${i.namedArguments.length} ${i.isMethod} ${i.isGetter}';
}

void main() {
  final dynamic t = Target();
  print(t.add(1, 2));
  print(t.add(5));
  print(t.greet('x'));
  for (final call in <void Function()>[
    () => t.add(),
    () => t.add('a'),
    () => t.missing(1),
    () => t.greet(1, 2, 3),
    () => (t.add as dynamic)(1, 2, 3),
  ]) {
    try {
      call();
      print('no error');
    } on NoSuchMethodError catch (e) {
      print('nsm ${e.toString().contains('add') || e.toString().contains('missing') || e.toString().contains('greet') || e.toString().contains('call')}');
    } on TypeError {
      print('type error');
    }
  }
  final dynamic c = Catcher();
  print(c.anything(1, 'two'));
  print(c.field);
  final dynamic f = (int x) => x * 2;
  print(f(21));
}
