class A {
  const A();
  A.nc();
  const A.n(int x);
}
A f() => A.nc();
const k = A();
@A.nc()
int a = 0;
@A
int b = 0;
@A.n
int c = 0;
@A()
int d = 0;
@A.n(n)
int e = 0;
@f()
int g = 0;
@k
int h = 0;
@n
int i = 0;
@Indef()
int j = 0;
@A.zz()
int l = 0;
int n = 1;
