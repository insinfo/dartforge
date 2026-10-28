// A class whose recipe is only the target of `is` tests needs no entry in
// the production name table; every type the program can print keeps its
// name, as on the VM.

class OnlyTested {}

class Printed {}

class InList {}

class CastTarget {}

class Generic<T> {}

const String zero = '${0}';

void main() {
  final values = <Object>[Printed(), 1, 'x', Generic<InList>()];
  for (final v in values) {
    print([v is OnlyTested, v is Printed, v is Generic<InList>]);
  }
  print(Printed().runtimeType);
  print(<InList>[].runtimeType);
  print(Generic<InList>().runtimeType);
  try {
    (values[0] as dynamic) as CastTarget;
  } on TypeError catch (e) {
    final m = e.toString();
    print([m.contains('CastTarget'), m.contains('Printed')]);
  }
  print([zero, zero.length, '${0x1F}', identical(zero, '0')]);
}
