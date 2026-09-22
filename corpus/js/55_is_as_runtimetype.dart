// is, is!, as (sucesso e falha), promoção, subtipos/interfaces/mixins, runtimeType, Object vs dynamic.
abstract class Forma {
  String nome();
}

mixin Colorido {
  String cor = 'azul';
}

class Quadrado extends Forma with Colorido {
  final int lado;
  Quadrado(this.lado);
  @override
  String nome() => 'quadrado';
}

class Circulo implements Forma {
  @override
  String nome() => 'circulo';
}

class Vazia {}

String classifica(Object o) {
  if (o is int) return 'int ${o + 1}';
  if (o is String) return 'string ${o.length}';
  if (o is List<int>) return 'lista de int ${o.length}';
  if (o is List) return 'lista ${o.length}';
  if (o is Forma) return 'forma ${o.nome()}';
  return 'outro';
}

void main() {
  final Object q = Quadrado(3);
  print(q is Quadrado);
  print(q is Forma);
  print(q is Colorido);
  print(q is Circulo);
  print(q is! Circulo);
  print(q is Object);
  if (q is Quadrado) {
    print(q.lado);
    print(q.cor);
  }
  if (q is Colorido) {
    print(q.cor.toUpperCase());
  }

  final Forma f = q as Forma;
  print(f.nome());
  final qq = q as Quadrado;
  print(qq.lado);
  try {
    final c = q as Circulo;
    print(c.nome());
  } catch (e) {
    print('as falhou: ${e is TypeError}');
  }
  try {
    final Object s = 'texto';
    final n = s as int;
    print(n);
  } catch (e) {
    print('as int falhou: ${e is TypeError}');
  }
  final dynamic d = q as dynamic;
  print(d.lado);
  print(d.nome());
  final Object? talvez = null;
  try {
    talvez as String;
    print('nao lancou');
  } catch (e) {
    print('null as String: ${e is TypeError}');
  }
  print(talvez as String?);

  print(Quadrado(1).runtimeType);
  print(Circulo().runtimeType);
  print(Vazia().runtimeType);
  print(Quadrado(1).runtimeType == Quadrado);
  print(Quadrado(1).runtimeType == Forma);
  print(Quadrado(1).runtimeType == Quadrado(2).runtimeType);
  final Type t = Circulo;
  print(t);
  print(t == Circulo().runtimeType);
  print(f.runtimeType);

  print(classifica(41));
  print(classifica('abc'));
  print(classifica(<int>[1, 2]));
  print(classifica(<String>['a']));
  print(classifica(Circulo()));
  print(classifica(Vazia()));

  final List<num> nums = <int>[1, 2];
  print(nums is List<int>);
  final List<num> nums2 = <num>[1, 2];
  print(nums2 is List<int>);
  print(nums2 is List<num>);
  print(nums2 is List<Object>);

  void funcao() {}
  print(funcao is Function);
  print(print is Function);
  print(classifica is String Function(Object));
  print(classifica is int Function(Object));
  print((() => 1) is Function);
  print(42 is Function);

  final Object o = 5;
  print(o is num);
  print(o is Comparable<num>);
  print(o is Pattern);
  final Object s = 'x';
  print(s is Pattern);
  print(s is Comparable<String>);
  print(null is Object);
  print(null is Object?);
  print(null is Null);
  print(<int>[] is Iterable<num>);
  print(<String, int>{} is Map<Object, num>);
  print(<String, int>{} is Map<int, int>);
}
