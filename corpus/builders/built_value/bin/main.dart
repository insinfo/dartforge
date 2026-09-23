// Saída determinística: builders, igualdade, toString, memoized e JSON.
import 'dart:convert';

import 'package:corpus_built_value/modelos.dart';

void main() {
  final pedido = Pedido((b) => b
    ..numero = 10
    ..itens.add(Item((i) => i
      ..nome = 'caneta'
      ..cor = Cor.azul
      ..etiquetas.addAll(['escritorio'])))
    ..itens.add(Item((i) => i
      ..nome = 'papel'
      ..quantidade = 3
      ..cor = Cor.vermelho
      ..nota = 'A4'))
    ..descontos['cupom'] = 5);
  print(pedido);
  print('total=${pedido.total}');
  final json = jsonEncode(serializers.serializeWith(Pedido.serializer, pedido));
  print(json);
  final volta = serializers.deserializeWith(Pedido.serializer, jsonDecode(json));
  print('${volta == pedido} ${volta.hashCode == pedido.hashCode}');
  final outro = pedido.rebuild((b) => b..numero = 11);
  print('${outro.numero} ${outro == pedido} ${Cor.values.map((c) => c.name).toList()}');
}
