// toJson/fromJson manual de classes aninhadas, listas de objetos, toEncodable, campos nullable, round-trip e igualdade.
import 'dart:convert';

class Endereco {
  final String rua;
  final int numero;
  final String? complemento;
  Endereco(this.rua, this.numero, [this.complemento]);

  Map<String, Object?> toJson() => {
        'rua': rua,
        'numero': numero,
        if (complemento != null) 'complemento': complemento,
      };

  factory Endereco.fromJson(Map<String, Object?> j) =>
      Endereco(j['rua'] as String, j['numero'] as int, j['complemento'] as String?);

  @override
  bool operator ==(Object o) =>
      o is Endereco && o.rua == rua && o.numero == numero && o.complemento == complemento;
  @override
  int get hashCode => Object.hash(rua, numero, complemento);
  @override
  String toString() => 'Endereco($rua, $numero, $complemento)';
}

class Pessoa {
  final String nome;
  final double altura;
  final bool ativo;
  final Endereco? endereco;
  final List<String> tags;
  final Map<String, int> notas;
  Pessoa(this.nome, this.altura, this.ativo, this.endereco, this.tags, this.notas);

  Map<String, Object?> toJson() => {
        'nome': nome,
        'altura': altura,
        'ativo': ativo,
        'endereco': endereco?.toJson(),
        'tags': tags,
        'notas': notas,
      };

  factory Pessoa.fromJson(Map<String, Object?> j) => Pessoa(
        j['nome'] as String,
        (j['altura'] as num).toDouble(),
        j['ativo'] as bool,
        j['endereco'] == null ? null : Endereco.fromJson(j['endereco'] as Map<String, Object?>),
        (j['tags'] as List).cast<String>(),
        (j['notas'] as Map).cast<String, int>(),
      );

  @override
  bool operator ==(Object o) =>
      o is Pessoa &&
      o.nome == nome &&
      o.altura == altura &&
      o.ativo == ativo &&
      o.endereco == endereco &&
      o.tags.join(',') == tags.join(',') &&
      o.notas.toString() == notas.toString();
  @override
  int get hashCode => Object.hash(nome, altura, ativo, endereco);
  @override
  String toString() => 'Pessoa($nome, ${altura.toStringAsFixed(2)}, $ativo, $endereco, $tags, $notas)';
}

class Equipe {
  final String nome;
  final List<Pessoa> membros;
  Equipe(this.nome, this.membros);
  Map<String, Object?> toJson() => {'nome': nome, 'membros': membros};
  factory Equipe.fromJson(Map<String, Object?> j) => Equipe(
        j['nome'] as String,
        (j['membros'] as List).map((m) => Pessoa.fromJson(m as Map<String, Object?>)).toList(),
      );
}

class Cor {
  final int r, g, b;
  Cor(this.r, this.g, this.b);
  @override
  String toString() => 'Cor($r,$g,$b)';
}

enum Nivel { baixo, medio, alto }

void main() {
  var e1 = Endereco('Rua A', 10, 'apto 2');
  var e2 = Endereco('Rua B', 20);
  print(jsonEncode(e1));
  print(jsonEncode(e2));
  print(jsonEncode(e1.toJson()));
  print(Endereco.fromJson(jsonDecode(jsonEncode(e1)) as Map<String, Object?>));
  print(Endereco.fromJson(jsonDecode(jsonEncode(e2)) as Map<String, Object?>));
  print(Endereco.fromJson(jsonDecode(jsonEncode(e1)) as Map<String, Object?>) == e1);
  print(Endereco.fromJson(jsonDecode(jsonEncode(e2)) as Map<String, Object?>) == e2);
  print(Endereco.fromJson(jsonDecode(jsonEncode(e1)) as Map<String, Object?>) == e2);
  print(Endereco.fromJson({'rua': 'X', 'numero': 1}).complemento);

  var p1 = Pessoa('Ana', 1.65, true, e1, ['a', 'b'], {'mat': 9, 'fis': 7});
  var p2 = Pessoa('Bruno', 1.825, false, null, [], {});
  var j1 = jsonEncode(p1);
  var j2 = jsonEncode(p2);
  print(j1);
  print(j2);
  var d1 = jsonDecode(j1) as Map<String, Object?>;
  print(d1['nome']);
  print(d1['altura']);
  print(d1['ativo']);
  print((d1['endereco'] as Map)['rua']);
  print((d1['tags'] as List).length);
  print((d1['notas'] as Map)['mat']);
  var d2 = jsonDecode(j2) as Map<String, Object?>;
  print(d2['endereco']);
  print(d2.containsKey('endereco'));
  print(d2['tags']);
  print(d2['notas']);
  var r1 = Pessoa.fromJson(d1);
  var r2 = Pessoa.fromJson(d2);
  print(r1);
  print(r2);
  print(r1 == p1);
  print(r2 == p2);
  print(r1 == p2);
  print(r1.endereco == e1);
  print(r2.endereco);
  print(r1.tags);
  print(r1.notas);
  print(jsonEncode(r1) == j1);
  print(jsonEncode(r2) == j2);
  print(jsonEncode(Pessoa.fromJson(jsonDecode(jsonEncode(r1)) as Map<String, Object?>)) == j1);

  var eq = Equipe('Alfa', [p1, p2]);
  var je = jsonEncode(eq);
  print(je);
  print(je.length);
  var re = Equipe.fromJson(jsonDecode(je) as Map<String, Object?>);
  print(re.nome);
  print(re.membros.length);
  print(re.membros[0]);
  print(re.membros[1].nome);
  print(re.membros[0] == p1);
  print(re.membros[1] == p2);
  print(jsonEncode(re) == je);
  print(jsonEncode(Equipe('Vazia', [])));
  print(Equipe.fromJson(jsonDecode('{"nome":"X","membros":[]}') as Map<String, Object?>).membros.isEmpty);
  print(jsonEncode([eq, Equipe('Beta', [p2])]).length);
  print(jsonEncode({'equipes': [eq]}).startsWith('{"equipes":[{"nome":"Alfa"'));

  print(jsonEncode(Cor(1, 2, 3), toEncodable: (o) => o is Cor ? [o.r, o.g, o.b] : o));
  print(jsonEncode({'c': Cor(1, 2, 3), 'n': 5}, toEncodable: (o) => o is Cor ? {'r': o.r, 'g': o.g, 'b': o.b} : o));
  print(jsonEncode([Cor(0, 0, 0), Cor(255, 255, 255)], toEncodable: (o) => o.toString()));
  print(jsonEncode(Nivel.alto, toEncodable: (o) => o is Nivel ? o.name : o));
  print(jsonEncode({'n': Nivel.baixo, 'l': [Nivel.medio]}, toEncodable: (o) => (o as Nivel).index));
  print(jsonEncode({'e': e1, 'c': Cor(9, 8, 7)}, toEncodable: (o) => o is Cor ? '#${o.r}${o.g}${o.b}' : 'nunca'));
  print(jsonEncode(Duration(minutes: 2), toEncodable: (o) => (o as Duration).inSeconds));
  print(jsonEncode(DateTime.utc(2024, 1, 2), toEncodable: (o) => (o as DateTime).toIso8601String()));
  print(jsonEncode(Uri.parse('http://h.com/p'), toEncodable: (o) => o.toString()));
  print(jsonEncode({'set': {1, 2}}, toEncodable: (o) => (o as Set).toList()));
  var enc = JsonEncoder((o) => o is Cor ? '${o.r}-${o.g}-${o.b}' : o);
  print(enc.convert([Cor(1, 1, 1), 'x']));
  print(JsonEncoder.withIndent(' ', (o) => o.toString()).convert({'c': Cor(2, 2, 2)}));
  try {
    jsonEncode(Cor(1, 2, 3));
  } on JsonUnsupportedObjectError catch (e) {
    print(e.unsupportedObject);
  }
  try {
    jsonEncode({'x': Cor(1, 2, 3)}, toEncodable: (o) => Cor(0, 0, 0));
  } on JsonUnsupportedObjectError catch (e) {
    print('resultado nao codificavel: ${e.unsupportedObject}');
  }
  var ciclo = <String, Object?>{};
  ciclo['eu'] = ciclo;
  try {
    jsonEncode(ciclo);
  } on JsonCyclicError catch (e) {
    print('ciclo detectado ${e.unsupportedObject == ciclo}');
  }
  var lista = [p1, p2];
  var listaJson = jsonEncode(lista);
  var volta = (jsonDecode(listaJson) as List).map((m) => Pessoa.fromJson(m as Map<String, Object?>)).toList();
  print(volta.length);
  print(volta.map((p) => p.nome).toList());
  print(volta.map((p) => p.altura.toStringAsFixed(3)).toList());
  print(volta.map((p) => p.endereco?.numero).toList());
  print(volta.map((p) => p.tags.length).toList());
  print(volta.map((p) => p.notas.length).toList());
  print(volta[0] == p1 && volta[1] == p2);
  print(volta.map((p) => p.hashCode == p.hashCode).toList());
  print(jsonEncode(volta) == listaJson);
  var j = jsonEncode({'a': null, 'b': [null], 'c': {'d': null}});
  print(j);
  var dj = jsonDecode(j) as Map;
  print(dj['a']);
  print(dj['b']);
  print(dj['c']);
  print(dj.containsKey('a'));
  print(Endereco.fromJson({'rua': 'R', 'numero': 1, 'complemento': null}).complemento);
  print(jsonEncode(Endereco('R', 1, null)));
  print(jsonEncode(Endereco('R', 1, 'c')));
}
