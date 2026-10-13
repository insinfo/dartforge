// Core collections: List<int>, Map<int, int>, Set<String>.
import 'common.dart';

int sieve(int n) {
  final prime = List<bool>.filled(n + 1, true);
  var c = 0;
  for (var i = 2; i <= n; i++) {
    if (prime[i]) {
      c++;
      for (var j = i * i; j <= n; j += i) {
        prime[j] = false;
      }
    }
  }
  return c;
}

// `list_int` in three kernels: a million `add`s, ten million indexed reads
// and the sort, measured separately (each one pays different costs).
List<int> listAdd(int n) {
  final l = <int>[];
  for (var i = 0; i < n; i++) {
    l.add(i * 7 % 1000);
  }
  return l;
}

int listRead(List<int> l) {
  var s = 0;
  for (var r = 0; r < 10; r++) {
    for (var i = 0; i < l.length; i++) {
      s += l[i];
    }
  }
  return s;
}

/// Sorts a copy; the check (order and multiset) stays out of the
/// measurement, in `checkSorted`.
List<int> sortCopy(List<int> origin) => List<int>.of(origin)..sort();

/// The sort's data: negatives, 64-bit extremes, the transition between
/// small and large `int` (2^62), and runs that are already sorted, reversed
/// and equal.
List<int> sortData(int n) {
  const mn = -9223372036854775808, mx = 9223372036854775807;
  final l = <int>[];
  var x = 88172645463325252;
  for (var i = 0; i < n; i++) {
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    switch (i % 8) {
      case 0:
        l.add(x);
      case 1:
        l.add(x % 1000 - 500);
      case 2:
        l.add((1 << 62) + (x % 3) - 1);
      case 3:
        l.add((i ~/ 8).isEven ? mn : mx);
      case 4:
        l.add(i);
      case 5:
        l.add(n - i);
      default:
        l.add(42);
    }
  }
  return l;
}

void checkSorted(List<int> origin, List<int> sorted) {
  if (origin.length != sorted.length) throw StateError('length');
  for (var i = 1; i < sorted.length; i++) {
    if (sorted[i - 1] > sorted[i]) throw StateError('out of order at $i');
  }
  final count = <int, int>{};
  for (final v in origin) {
    count[v] = (count[v] ?? 0) + 1;
  }
  for (final v in sorted) {
    final c = (count[v] ?? 0) - 1;
    if (c < 0) throw StateError('extra element $v');
    count[v] = c;
  }
}

int map(int n) {
  final m = <int, int>{};
  for (var i = 0; i < n; i++) {
    m[i * 31 % n] = i;
  }
  var s = 0;
  for (var i = 0; i < n; i++) {
    s += m[i] ?? 0;
  }
  return s;
}

int set(int n) {
  final c = <String>{};
  for (var i = 0; i < n; i++) {
    c.add('k${i % 5000}');
  }
  return c.length;
}

void main() {
  measure('sieve', () => sieve(2000000));
  final added = listAdd(1000000);
  measure('list_add', () => listAdd(1000000).length);
  measure('list_read', () => listRead(added));
  final data = sortData(1000000);
  List<int> last = const [];
  measure('list_sort', () {
    last = sortCopy(data);
    return last[last.length ~/ 2];
  });
  checkSorted(data, last);
  print('sort checked: ${last.first} ${last.last}');
  measure('map', () => map(500000));
  measure('set_str', () => set(300000));
}
