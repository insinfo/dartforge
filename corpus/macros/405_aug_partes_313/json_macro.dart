part of 'main.dart';

import 'dart:core' as prefix0;

augment class Usuario {
  external Usuario.fromJson(prefix0.Map<prefix0.String, prefix0.Object?> json);
  external prefix0.Map<prefix0.String, prefix0.Object?> toJson();
  augment Usuario.fromJson(prefix0.Map<prefix0.String, prefix0.Object?> json, )
      : this.nome = json[r'nome'] as prefix0.String,
        this.idade = json[r'idade'] as prefix0.int,
        this.apelido = json[r'apelido'] as prefix0.String?,
        this.notas = [ for (final item in json[r'notas'] as prefix0.List<prefix0.Object?>) item as prefix0.int];
  augment prefix0.Map<prefix0.String, prefix0.Object?> toJson() {
    final json = <prefix0.String, prefix0.Object?>{};
    json[r'nome'] = this.nome;
    json[r'idade'] = this.idade;
    if (this.apelido != null) {
      json[r'apelido'] = this.apelido!;
    }
    json[r'notas'] = [ for (final item in this.notas) item];
    return json;
  }
}
