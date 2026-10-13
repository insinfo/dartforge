enum E { e1, e2 }
Object f(E e, bool b) => switch (e) {
  E.e1 when b => 0,
  E.e2 => 1,
  E.e1 => 2,
};
