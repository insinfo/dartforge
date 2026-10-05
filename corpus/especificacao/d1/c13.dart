class G<T> {
  final T x = y;
  const G();
}
const dynamic y = 1;
const a = const G<String>();
class H {
  final int x;
  const H(dynamic v) : x = v;
}
const b = const H('a');
class I {
  final int x;
  const I(String v) : x = v;
}
const c = const I('a');
