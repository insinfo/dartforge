// built_value: classe de valor com builder, coleção imutável, enum de classe
// e serializadores. A parte `.built_value.g.part` vai para o cache e o
// combining_builder a junta em `modelos.g.dart`.
library modelos;

import 'package:built_collection/built_collection.dart';
import 'package:built_value/built_value.dart';
import 'package:built_value/serializer.dart';
import 'package:built_value/standard_json_plugin.dart';

part 'modelos.g.dart';

class Cor extends EnumClass {
  static const Cor azul = _$azul;
  static const Cor verde = _$verde;
  @BuiltValueEnumConst(wireName: 'VERMELHO')
  static const Cor vermelho = _$vermelho;

  const Cor._(String name) : super(name);
  static BuiltSet<Cor> get values => _$values;
  static Cor valueOf(String name) => _$valueOf(name);
  static Serializer<Cor> get serializer => _$corSerializer;
}

abstract class Item implements Built<Item, ItemBuilder> {
  String get nome;
  int get quantidade;
  Cor get cor;
  String? get nota;
  BuiltList<String> get etiquetas;

  Item._();
  factory Item([void Function(ItemBuilder) updates]) = _$Item;
  static Serializer<Item> get serializer => _$itemSerializer;

  @BuiltValueHook(initializeBuilder: true)
  static void _padroes(ItemBuilder b) => b..quantidade = 1;
}

abstract class Pedido implements Built<Pedido, PedidoBuilder> {
  int get numero;
  BuiltList<Item> get itens;
  BuiltMap<String, int> get descontos;

  Pedido._();
  factory Pedido([void Function(PedidoBuilder) updates]) = _$Pedido;
  static Serializer<Pedido> get serializer => _$pedidoSerializer;

  @memoized
  int get total => itens.fold(0, (s, i) => s + i.quantidade);
}

@SerializersFor([Pedido])
final Serializers serializers =
    (_$serializers.toBuilder()..addPlugin(StandardJsonPlugin())).build();
