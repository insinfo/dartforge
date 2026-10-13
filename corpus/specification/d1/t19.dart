abstract class I { const factory I() = Impl; }
class Impl implements I { const Impl() : assert(false, 'boom'); }
const i = const I();
const s = const Symbol('a b');
const d = const Duration(seconds: 1);
const e = d.inSeconds;
