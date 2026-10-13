void f(double? dq, int x, List<int> l) {
  if (dq case 1) {}
  if (dq case 'a') {}
  if (x case > [1]) {}
  if (x case == indef) {}
  if (x case indef) {}
  if (x case > const [1]) {}
  if (l case [const A(n)]) {}
  if (x case const A(n)) {}
}
class A { const A(int i); }
int n = 0;
