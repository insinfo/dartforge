// A `?.` chain that ends in a call the native backend lowers inline: `List.add`
// (scalar and reference elements), the `int`/`double` getters and conversions,
// `int.compareTo` and `num.toInt`. With a null receiver the whole chain is null:
// the inline paths lowered their receiver as a chain of its own, and
// `testData?.unlinkedKeyGet.add(key)` of the analyzer's `FileState` (the
// analyzer-based builders through the native executor) called `add` on null.
// The doubles print with `toStringAsFixed`: on the web `4.0` prints as `4`.
class Box {
  int n = 3;
  double d = 2.5;
  num m = 7;
  List<int> ints = [1];
  List<String> strs = ['a'];
  List<bool> flags = [true];
}

class T {
  Box box = Box();
  int n = 4;
  double d = 1.5;
  num m = 9;
  List<int> ints = [1];
  List<double> doubles = [1.0];
  List<String> strs = ['a'];
}

T? maybe(bool present) => present ? T() : null;

void main() {
  for (final present in [false, true]) {
    final t = maybe(present);
    final log = <Object?>[];
    t?.strs.add('x');
    t?.ints.add(2);
    t?.doubles.add(2.0);
    t?.box.strs.add('y');
    t?.box.ints.add(5);
    t?.box.flags.add(false);
    log.add(t?.n.isEven);
    log.add(t?.n.toDouble().toStringAsFixed(1));
    log.add(t?.d.isNaN);
    log.add(t?.d.toInt());
    log.add(t?.n.compareTo(2));
    log.add(t?.m.toInt());
    log.add(t?.box.n.abs());
    log.add(t?.box.m.toInt());
    t?.box.n = 10;
    t?.box.n += 1;
    t?.strs[0] = 'z';
    log.add(t?.box.n);
    log.add(t?.strs);
    log.add(t?.ints);
    log.add(t?.doubles.map((x) => x.toStringAsFixed(1)).toList());
    log.add(t?.box.strs);
    log.add(t?.box.ints);
    log.add(t?.box.flags);
    print(log);
  }
}
