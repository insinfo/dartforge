class P {
  int x = 0;
  String? name;
  List<int> items = [];
  P? next;
  void add(int v) => items.add(v);
  P self() => this;
  int operator +(P o) => x + o.x;
  int operator [](int i) => items[i];
  void operator []=(int i, int v) => items[i] = v;
}

class Dyn {
  noSuchMethod(Invocation i) => 'nsm:${i.memberName}:${i.positionalArguments}';
}

class Counter {
  int n = 0;
  void inc() => n++;
  Counter operator -() => Counter()..n = -n;
}

void main() {
  var c = Counter()..inc()..inc();
  print(c.n);
  print((-c).n);
  var p = P()
    ..x = 5
    ..add(1)
    ..add(2)
    ..name = 'p';
  print(p.x);
  print(p.items);
  print(p.name);
  print(p[1]);
  p[0] = 9;
  print(p.items);
  P? np;
  print(np?.x);
  print(np?.self().x);
  np?.add(1);
  print(np?.items ?? [0]);
  np ??= P();
  print(np.x);
  print(np!.x);
  np?..x = 7..add(3);
  print(np.x);
  print(np.next?.next?.x);
  print(p.name?.length);
  print(p.name!.length);
  print(p + np);
  dynamic d = P();
  d.x = 3;
  print(d.x);
  d.add(4);
  print(d.items);
  print(d.self().x);
  print(d + d);
  print(d[0]);
  d[0] = 8;
  print(d.items);
  dynamic dl = [1, 2];
  print(dl[0]);
  dl[0] = 5;
  print(dl);
  print(dl.length);
  dynamic ds = 'abc';
  print(ds.length);
  print(ds + 'd');
  print(ds == 'abc');
  print(ds.toUpperCase());
  dynamic dn = 5;
  print(dn + 1);
  print(dn * 2.5);
  print(dn > 3);
  print(-dn);
  print(dn.isEven);
  print(dn.toString());
  print(dn.hashCode == 5.hashCode);
  print(dn.runtimeType);
  dynamic df = (int x) => x + 1;
  print(df(1));
  dynamic dy = Dyn();
  print(dy.foo(1, 2));
  print(dy.bar);
  try {
    dn.naoExiste();
  } catch (e) {
    print(e is NoSuchMethodError);
  }
  Object o = 'str';
  if (o is String) print(o.length);
  if (o is! String) {
    print('nunca');
  } else {
    print(o.toUpperCase());
  }
  Object? on = null;
  print(on == null ? 'nulo' : on.toString());
  int? ni = 2;
  if (ni != null) print(ni + 1);
  print(ni?.isEven);
  String? sn;
  print(sn?.toUpperCase() ?? 'none');
  print(sn ??= 'set');
  print(sn);
  print(identical(p, p.self()));
  print(p.hashCode == p.hashCode);
  print(p.runtimeType);
  var l = <Object?>[1, 'a', 2.5, null, true];
  for (var e in l) {
    print(switch (e) { int() => 'i', String() => 's', double() => 'd', null => 'n', _ => 'o' });
  }
}
