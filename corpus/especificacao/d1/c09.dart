const a = identical(1, 1);
const b = identical(1, 1.0);
const c = identical(const [1], const [1]);
int f(int x) => x;
const d = f(1);
const e = 'a'.toString();
const g = identical(1);
extension E on int { int get twice => this * 2; int m() => 0; int operator +(String s) => 0; }
const h = 1.twice;
const i = 1.m();
