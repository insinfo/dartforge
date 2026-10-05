class P {
  final int v;
  const P(this.v);
  P.nc(this.v);
}
int n() => 1;
var a = const P(n());
var b = const P.nc(n());
var c = const P.zz(n());
const d = P.zz(1);
const e = [P.nc(1), P.zz(2)];
