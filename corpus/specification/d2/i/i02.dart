class P {
  const P();
  P.nc();
}
int n = 0;
void f() {
  const [P.nc()];
  const [new P()];
  const {P.nc(): 1};
  const (P.nc(),);
  const P p = const P.nc();
  print(p);
}
