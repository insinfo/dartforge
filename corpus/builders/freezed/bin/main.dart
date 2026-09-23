// Saída determinística: igualdade, copyWith, toString, JSON e união.
import 'dart:convert';

import 'package:corpus_freezed/modelos.dart';

void main() {
  const a = Pessoa(id: 1, nome: 'Ana', email: 'ana@x', endereco: Endereco(rua: 'Central'));
  final b = a.copyWith(nome: 'Bia', apelidos: ['b']);
  print(a);
  print(b);
  print('${a == Pessoa.fromJson(jsonDecode(jsonEncode(a.toJson())) as Map<String, dynamic>)} ${a == b} ${a.rotulo}');
  print(jsonEncode(b.toJson()));
  final formas = [const Forma.circulo(raio: 2), const Forma.retangulo(largura: 2, altura: 5)];
  for (final f in formas) {
    final json = jsonEncode(f.toJson());
    print('$json ${area(Forma.fromJson(jsonDecode(json) as Map<String, dynamic>))}');
  }
}
