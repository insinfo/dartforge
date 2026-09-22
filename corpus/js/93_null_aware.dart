// Operadores null-aware: ?. ?? ??= ! ?.. ?[ ...? encadeamento e efeitos colaterais só quando necessário.
class Endereco {
  final String? rua;
  final Cidade? cidade;
  Endereco({this.rua, this.cidade});
}

class Cidade {
  final String nome;
  final String? estado;
  Cidade(this.nome, [this.estado]);
  String saudacao() {
    print('saudacao chamada em $nome');
    return 'bem-vindo a $nome';
  }
}

class Pessoa {
  final String nome;
  final Endereco? endereco;
  int visitas = 0;
  Pessoa(this.nome, [this.endereco]);
  void visita() {
    visitas++;
    print('$nome visitou');
  }
}

String padrao(String rotulo) {
  print('padrao($rotulo) avaliado');
  return 'padrao-$rotulo';
}

void main() {
  final com = Pessoa('ana', Endereco(rua: 'A', cidade: Cidade('Rio', 'RJ')));
  final semCidade = Pessoa('bia', Endereco(rua: 'B'));
  final semEnd = Pessoa('caio');
  Pessoa? ninguem;

  print(com.endereco?.cidade?.nome);
  print(semCidade.endereco?.cidade?.nome);
  print(semEnd.endereco?.cidade?.nome);
  print(ninguem?.endereco?.cidade?.nome);
  print(com.endereco?.cidade?.estado?.toLowerCase());
  print(com.endereco?.cidade?.nome.length);
  print(semEnd.endereco?.rua?.length);

  print(com.endereco?.cidade?.saudacao());
  print(semCidade.endereco?.cidade?.saudacao());
  ninguem?.visita();
  com.visita();
  semEnd.visita();
  print(com.visitas);
  print(ninguem?.visitas);

  print(semEnd.endereco?.rua ?? 'sem rua');
  print(com.endereco?.rua ?? 'sem rua');
  print(ninguem?.nome ?? padrao('a'));
  String? vazio;
  print(vazio ?? padrao('c'));
  print(vazio ?? vazio ?? 'terceiro');
  int? zero = 0;
  print(zero ?? 1);
  print((zero == 0 ? null : zero) ?? -1);

  String? cache;
  cache ??= padrao('d');
  print(cache);
  cache ??= padrao('e');
  print(cache);
  final mapa = <String, int>{};
  mapa['x'] ??= 1;
  mapa['x'] ??= 2;
  print(mapa);
  final contadores = <String, List<int>>{};
  (contadores['a'] ??= []).add(1);
  (contadores['a'] ??= []).add(2);
  print(contadores);

  String? talvez = 'valor';
  print(talvez!.length);
  print(talvez!);
  final Object? obj = 'texto';
  print((obj as String?)!.toUpperCase());
  int? nulo;
  try {
    print(nulo!);
  } catch (e) {
    print('bang em null: ${e is TypeError}');
  }

  List<int>? lista;
  print(lista?[0]);
  print(lista?.length);
  lista = [10, 20];
  print(lista?[1]);
  print(lista?.first);
  Map<String, int>? mapaNulo;
  print(mapaNulo?['k']);
  print([1, ...?lista, 2]);
  List<int>? nada;
  print([1, ...?nada, 2]);
  print({...?mapaNulo, 'z': 0});
  print({0, ...?nada, ...?lista});

  StringBuffer? sb;
  sb
    ?..write('nunca')
    ..write('x');
  print(sb);
  sb = StringBuffer();
  sb
    ?..write('agora')
    ..write('!');
  print(sb);

  final valores = <int?>[1, null, 3, null];
  print(valores.map((v) => v ?? 0).toList());
  print(valores.whereType<int>().toList());
  print(valores.map((v) => v?.isOdd).toList());
  print(valores.firstWhere((v) => v == null));
  print(valores.length);
  var total = 0;
  for (final v in valores) {
    total += v ?? 0;
  }
  print(total);
  print(valores.first?.toString() ?? 'nulo');
  print(valores[1]?.toString() ?? 'nulo');
}
