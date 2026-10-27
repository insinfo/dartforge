// Calling the value of a getter through an explicit receiver evaluates the
// arguments before reading the getter; implicit `this`, static and top-level
// getters are read first. An implicit `call` tear-off applies to a cascade,
// a conditional or an `??` as a whole, not to the cascade target or the
// operands, and to the value assigned through static, top-level and
// extension setters and index operators. `p.loadLibrary` can be torn off, and
// an index assignment through a deferred prefix checks that it is loaded.
import 'lazy.dart' deferred as lazy;

String effects = '';

String get arg {
  effects += 'A';
  return 'a';
}

class Target {
  void Function(String) get m {
    effects += 'G';
    return (_) {};
  }

  dynamic get untyped {
    effects += 'G';
    return (String _) {};
  }

  void viaThis() {
    effects = '';
    this.m(arg);
    print('explicit this: $effects');
    effects = '';
    m(arg);
    print('implicit this: $effects');
  }
}

void Function(String) get topLevel {
  effects += 'G';
  return (_) {};
}

class C {
  void call() {}
  void other() {}
}

class A {}

Object? lastSet;

set topLevelSetter(void Function() f) => lastSet = f;

class Holder {
  static set staticSetter(void Function() f) => lastSet = f;
}

class Box {
  Object? value;
}

extension Store on Box {
  set stored(void Function() f) => value = f;
  operator []=(String key, void Function() f) => value = f;
}

void context(void Function() f) => print('function: ${f is void Function()}');

Future<void> main() async {
  var t = Target();
  effects = '';
  t.m(arg);
  print('explicit receiver: $effects');
  effects = '';
  t.untyped(arg);
  print('dynamic getter: $effects');
  t.viaThis();
  effects = '';
  topLevel(arg);
  print('top-level: $effects');

  var c = C();
  context(c..other());
  context((c)..other());
  var b = DateTime.now().year > 2000;
  context(b ? c : c);
  C? maybe;
  context(maybe ?? c);
  topLevelSetter = C();
  print(lastSet is void Function());
  Holder.staticSetter = C();
  print(lastSet is void Function());
  var box = Box();
  box.stored = C();
  print(box.value is void Function());
  box.value = null;
  box['k'] = C();
  print(box.value is void Function());

  var load = lazy.loadLibrary;
  try {
    lazy.values[0] = 1;
    print('assigned before loading');
  } catch (e) {
    print('not loaded yet');
  }
  await load();
  lazy.values[1] = 2;
  print(lazy.values);
}
