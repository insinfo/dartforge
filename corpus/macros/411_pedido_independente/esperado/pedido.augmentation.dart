augment library 'package:caso_pedido/pedido.dart';

import 'dart:core' as prefix0;

augment class Pedido {
  external Pedido.fromJson(prefix0.Map<prefix0.String, prefix0.Object?> json);
  external prefix0.Map<prefix0.String, prefix0.Object?> toJson();
  augment Pedido.fromJson(prefix0.Map<prefix0.String, prefix0.Object?> json, )
      : this.codigo = json[r'codigo'] as prefix0.String,
        this.total = json[r'total'] as prefix0.double;
  augment prefix0.Map<prefix0.String, prefix0.Object?> toJson() {
    final json = <prefix0.String, prefix0.Object?>{};
    json[r'codigo'] = this.codigo;
    json[r'total'] = this.total;
    return json;
  }
}
