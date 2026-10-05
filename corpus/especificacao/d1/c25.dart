class O { const O(); }
const a = '${const O()}';
const b = '${[1]}';
const c = 'a' 'b' '${1}';
const d = const O() == const O();
const e = 1.0 == 1;
const f = [1] == [1];
class Q { const Q(); bool operator ==(Object o) => true; int get hashCode => 0; }
const g = const Q() == const Q();
const h = const Q() != null;
