const a = 1 + (2 * 'a'.foo);
const b = ['a'.foo, 1 ~/ 0];
const c = {1 ~/ 0: 'a'.foo};
const d = ('a'.foo, 1 ~/ 0);
const e = (1 ~/ 0) + 'a'.foo;
