class A { const A(); static int sm() => 0; int im() => 0; static const k = 1; static var sv = 1; }
const a = A.sm;
const b = A.k;
const c = A.sv;
const d = const A().im;
const e = A.new;
const f = A;
const g = #foo.bar;
const h = (1, a: 'x');
const i = List<int>;
const j = A.undefinedThing;
const k = undefinedTop;
const l = dynamic;
