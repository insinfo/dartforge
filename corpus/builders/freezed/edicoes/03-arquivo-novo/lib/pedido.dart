// Entrada nova com freezed + json: as duas saídas source e a parte de cache
// aparecem sem mexer nas de modelos.dart.
import 'package:freezed_annotation/freezed_annotation.dart';

import 'modelos.dart';

part 'pedido.freezed.dart';
part 'pedido.g.dart';

@freezed
abstract class Pedido with _$Pedido {
  const factory Pedido({required int numero, required Pessoa cliente, @Default(<String, int>{}) Map<String, int> itens}) = _Pedido;
  factory Pedido.fromJson(Map<String, dynamic> json) => _$PedidoFromJson(json);
}
