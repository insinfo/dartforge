class MyErr implements Exception {
  final String msg;
  MyErr(this.msg);
  String toString() => 'MyErr($msg)';
}
int f(int x) {
  if (x < 0) throw ArgumentError('neg');
  return x;
}
void g() {
  try {
    throw MyErr('a');
  } finally {
    print('fin g');
  }
}
void main() {
  try {
    f(-1);
  } on ArgumentError catch (e) {
    print('arg: ${e.message}');
  }
  try {
    throw MyErr('x');
  } on MyErr catch (e, st) {
    print(e);
    print(st is StackTrace);
  } catch (e) {
    print('other');
  } finally {
    print('finally');
  }
  try {
    g();
  } catch (e) {
    print('caught $e');
  }
  try {
    try {
      throw 'str';
    } catch (e) {
      print('inner $e');
      rethrow;
    }
  } catch (e) {
    print('outer $e');
  }
  try {
    var l = [1];
    print(l[5]);
  } on RangeError catch (e) {
    print('range');
  }
  try {
    int? n;
    print(n!);
  } catch (e) {
    print(e is TypeError);
  }
  try {
    dynamic d = 'x';
    int i = d;
    print(i);
  } catch (e) {
    print('cast');
  }
  try {
    assert(1 > 2, 'msg');
    print('no assert');
  } catch (e) {
    print('assert');
  }
  try {
    throw Exception('boom');
  } catch (e) {
    print(e);
  }
  try {
    throw StateError('st');
  } on StateError catch (e) {
    print(e.message);
  }
  try {
    throw UnsupportedError('x');
  } catch (e) {
    print(e);
  }
  print(StackTrace.current is StackTrace);
  Object? r;
  try {
    r = f(3);
  } catch (_) {
    r = null;
  }
  print(r);
  try {
    throw MyErr('z');
  } on String {
    print('string');
  } on Exception catch (e) {
    print('exc $e');
  }
  try {
    throw FormatException('fmt');
  } on FormatException catch (e) {
    print(e.message);
  }
  try {
    try {
      throw 1;
    } finally {
      print('f1');
    }
  } catch (e) {
    print('c $e');
  }
  int loopCatch() {
    for (var i = 0; i < 3; i++) {
      try {
        if (i == 1) return i;
      } finally {
        print('fin $i');
      }
    }
    return -1;
  }
  print(loopCatch());
}
