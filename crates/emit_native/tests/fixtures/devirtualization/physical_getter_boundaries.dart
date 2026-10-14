class Base {
  final String text;
  Base(this.text);
  String get value => text;
  String get failure => throw StateError('expected getter failure');
}

class Derived extends Base {
  Derived(super.text);
  @override
  String get value => 'derived:$text';
}

int counter = 0;
int namedEffects = 0;
int namedValue() {
  namedEffects++;
  return 1;
}
bool choose() {
  counter++;
  return counter % 2 == 1;
}

dynamic factory() => Base('factory');

void readUnknown(dynamic receiver) {
  print(receiver.value);
}

void probeMixed() {
  dynamic receiver = choose() ? Base('base') : Derived('override');
  print(receiver.value);
}

void main() {
  dynamic direct = Base('direct');
  print(direct.value);
  try {
    print(direct.failure);
  } on StateError catch (error) {
    print(error.message);
  }
  print(factory().value);
  dynamic absent = null;
  try {
    print(absent.value);
  } on NoSuchMethodError {
    print('nullable dispatch preserved');
  }
  try {
    direct.value(named: namedValue());
  } on NoSuchMethodError {
    print('named call rejection preserved');
    print(namedEffects);
  }
  readUnknown(Base('unknown base'));
  readUnknown(Derived('unknown override'));
  probeMixed();
  probeMixed();
}
