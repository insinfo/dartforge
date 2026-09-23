// Entrada nova: o build incremental tem de descobrir o arquivo e gerar a
// parte dele sem refazer as outras.
import 'package:json_annotation/json_annotation.dart';

part 'produto.g.dart';

@JsonSerializable(fieldRename: FieldRename.snake)
class Produto {
  final String codigoInterno;
  final double precoUnitario;
  final List<String> etiquetas;
  Produto({required this.codigoInterno, required this.precoUnitario, this.etiquetas = const []});
  factory Produto.fromJson(Map<String, dynamic> json) => _$ProdutoFromJson(json);
  Map<String, dynamic> toJson() => _$ProdutoToJson(this);
}
