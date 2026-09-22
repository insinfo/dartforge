// Saída determinística: o mesmo programa tem de imprimir o mesmo texto
// compilado pela toolchain oficial e pelo DartForge.
import 'dart:convert';
import 'package:corpus_json_serializable/modelos.dart';

void main() {
  final p = Pessoa(
    id: 7,
    nome: 'Ana',
    situacao: Situacao.suspenso,
    nascimento: DateTime.utc(1990, 3, 14, 15, 9, 26),
    enderecos: [Endereco(rua: 'Central', numero: 42, cep: '28890-000')],
    pontuacoes: {'a': 1, 'b': 2},
    salario: null,
  );
  final json = jsonEncode(p.toJson());
  print(json);
  final volta = Pessoa.fromJson(jsonDecode(json) as Map<String, dynamic>);
  print('${volta.id} ${volta.nome} ${volta.situacao} ${volta.nascimento.toIso8601String()}');
  print('${volta.enderecos.first.rua}/${volta.enderecos.first.numero}/${volta.enderecos.first.cep}');
  print('${volta.pontuacoes} segredo=${volta.segredo} salario=${volta.salario}');
}
