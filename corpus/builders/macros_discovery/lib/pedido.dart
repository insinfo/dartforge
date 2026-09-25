import 'package:json/json.dart';

@JsonCodable()
class Pedido {
  final String codigo;
  final double total;
}
