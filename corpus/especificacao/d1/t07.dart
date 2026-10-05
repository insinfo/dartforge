const a = 1 is num;
const b = 1 as String;
class G<T> {
  final bool r;
  final Object? c;
  const G(Object? o) : r = o is T, c = o as T;
}
const g = const G<int>('x');
const h = const G<int>(1);
