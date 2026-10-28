// Private member symbols that are only property keys carry no description
// in production; the names a program can observe (dynamic calls,
// `Invocation.memberName`, tear-offs, lazy statics) keep theirs.

abstract class Shape {
  int _area();
  String _label(String prefix);
}

class Square implements Shape {
  final int _side;
  Square(this._side);
  @override
  int _area() => _side * _side;
  @override
  String _label(String prefix) => '$prefix square ${_area()}';
  int get _twice => _area() * 2;
}

class Recorder implements Shape {
  @override
  dynamic noSuchMethod(Invocation i) {
    print('nsm ${i.memberName} ${i.isMethod} ${i.positionalArguments}');
    return i.memberName == #_label ? 'label' : 0;
  }
}

mixin _Counter {
  int _count = 0;
  void _bump() => _count++;
}

class Clicks with _Counter {
  int get total => _count;
}

int _calls = 0;
final int _lazy = _init();
int _init() => ++_calls * 10;

Iterable<T> _twice<T>(T value) sync* {
  yield value;
  yield value;
}

void main() {
  final s = Square(3);
  print([s._area(), s._label('a'), s._twice]);
  final Shape r = Recorder();
  r._area();
  print(r._label('b'));
  dynamic d = s;
  print(d._area());
  print(d._label('c'));
  try {
    d._missing(1);
  } on NoSuchMethodError catch (e) {
    print(e.toString().contains('_missing'));
  }
  final tear = s._label;
  print(tear('t'));
  print(tear == s._label);
  final c = Clicks();
  c._bump();
  c._bump();
  print(c.total);
  print([_lazy, _lazy, _calls]);
  print(_twice<String>('x').toList());
  print(<Object>[s, r, c].map((o) => o.runtimeType).toList());
}
