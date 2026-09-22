// coleções tipadas: covariância (List<int> is List<num>), List<Object>, cast, TypeError via dynamic, List<dynamic>, aninhadas.
void adicionaNum(List<num> xs, num v) => xs.add(v);

void main() {
  final ints = <int>[1, 2, 3];
  print(ints is List<num>);
  print(ints is List<Object>);
  print(ints is List<int>);
  print(ints is List<String>);
  print(ints is Iterable<num>);
  print(ints is List<dynamic>);

  // adicionar tipo errado via covariância lança TypeError
  final List<num> comoNum = ints;
  comoNum.add(4);
  print(ints);
  try {
    comoNum.add(2.5);
    print('não lançou');
  } catch (e) {
    print('add double em List<int> via List<num>: ${e is TypeError}');
  }
  try {
    adicionaNum(ints, 0.5);
  } catch (e) {
    print('via função: ${e is TypeError}');
  }
  print(ints);

  // List<Object> aceita mistura
  final objs = <Object>[1, 'a', 2.5, [1], {'k': 'v'}, true];
  print(objs);
  print(objs.length);
  print(objs is List<Object>);
  print(objs is List<int>);
  print(objs.whereType<int>().toList());

  // dynamic e atribuição em coleção tipada
  final dyn = <dynamic>[1, 'x'];
  print(dyn is List<dynamic>);
  print(dyn is List<Object?>);
  print(dyn is List<int>);
  final List<int> tipada = [1];
  dynamic valor = 'texto';
  try {
    tipada.add(valor);
  } catch (e) {
    print('add dynamic errado: ${e is TypeError}');
  }
  valor = 2;
  tipada.add(valor);
  print(tipada);

  // <int>[] vs []
  final vazioInt = <int>[];
  final vazioSemTipo = [];
  print(vazioInt is List<int>);
  print(vazioSemTipo is List<int>);
  print(vazioSemTipo is List<dynamic>);
  vazioSemTipo.add('qualquer');
  vazioSemTipo.add(1);
  print(vazioSemTipo);

  // inferência de literal
  final inferido = [1, 2.5];
  print(inferido is List<num>);
  print(inferido is List<int>);
  print(inferido is List<double>);
  final misto = [1, 'a'];
  print(misto is List<Object>);
  print(misto is List<num>);
  final comNulo = [1, null];
  print(comNulo is List<int?>);
  print(comNulo is List<int>);

  // cast<num>()
  final numsCast = ints.cast<num>();
  print(numsCast is List<num>);
  print(numsCast.length);
  print(numsCast.map((n) => n * 2).toList());
  try {
    numsCast.add(1.5);
  } catch (e) {
    print('cast view ainda protege a lista original: ${e is TypeError}');
  }
  print(ints);
  final objsCast = <Object>[1, 2].cast<int>();
  print(objsCast.map((x) => x + 1).toList());
  print(objsCast is List<int>);

  // List.from vs List.of e tipo resultante
  final deFrom = List<num>.from(ints);
  deFrom.add(0.5);
  print(deFrom);
  final deOf = List.of(ints);
  print(deOf is List<int>);

  // Map<String, List<int>> aninhado
  final grupos = <String, List<int>>{};
  grupos['pares'] = [];
  grupos['ímpares'] = [];
  for (var i = 1; i <= 6; i++) {
    grupos[i.isEven ? 'pares' : 'ímpares']!.add(i);
  }
  print(grupos);
  print(grupos['pares']!.length);
  print(grupos is Map<String, List<num>>);
  print(grupos is Map<String, List<int>>);
  print(grupos is Map<Object, Object>);
  print(grupos.values.expand((l) => l).toList());

  // Map covariância no valor
  final Map<String, num> mapaNum = <String, int>{'a': 1};
  try {
    mapaNum['b'] = 1.5;
  } catch (e) {
    print('map covariante: ${e is TypeError}');
  }
  print(mapaNum);

  // Set tipado
  final si = <int>{1};
  print(si is Set<num>);
  print(si is Set<String>);
  final Set<num> sn = si;
  try {
    sn.add(0.5);
  } catch (e) {
    print('set covariante: ${e is TypeError}');
  }

  // is com genérico de função
  final fs = <int Function(int)>[(x) => x];
  print(fs is List<Function>);
  print(fs is List<int Function(int)>);
  print(fs is List<num Function(int)>);
  print(fs is List<int Function(num)>);

  // lista de records e de listas tipadas
  final recs = <(int, String)>[(1, 'a')];
  print(recs is List<(num, Object)>);
  print(recs is List<(String, int)>);
  final matriz = <List<int>>[
    [1],
    [2, 3]
  ];
  print(matriz is List<List<num>>);
  print(matriz is List<Iterable<int>>);
  print(matriz is List<List<String>>);
}
