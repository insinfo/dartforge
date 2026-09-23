augment library 'package:caso_json/modelos.dart';

import 'dart:core' as prefix0;
import 'package:caso_json/modelos.dart' as prefix1;

augment class Endereco {
  external Endereco.fromJson(prefix0.Map<prefix0.String, prefix0.Object?> json);
  external prefix0.Map<prefix0.String, prefix0.Object?> toJson();
  augment Endereco.fromJson(prefix0.Map<prefix0.String, prefix0.Object?> json, )
      : this.rua = json[r'rua'] as prefix0.String,
        this.numero = json[r'numero'] as prefix0.int?;
  augment prefix0.Map<prefix0.String, prefix0.Object?> toJson() {
    final json = <prefix0.String, prefix0.Object?>{};
    json[r'rua'] = this.rua;
    if (this.numero != null) {
      json[r'numero'] = this.numero!;
    }
    return json;
  }
}
augment class Usuario {
  external Usuario.fromJson(prefix0.Map<prefix0.String, prefix0.Object?> json);
  external prefix0.Map<prefix0.String, prefix0.Object?> toJson();
  augment Usuario.fromJson(prefix0.Map<prefix0.String, prefix0.Object?> json, )
      : this.nome = json[r'nome'] as prefix0.String,
        this.idade = json[r'idade'] as prefix0.int,
        this.apelido = json[r'apelido'] as prefix0.String?,
        this.notas = [ for (final item in json[r'notas'] as prefix0.List<prefix0.Object?>) item as prefix0.int],
        this.tags = { for (final item in json[r'tags'] as prefix0.List<prefix0.Object?>)item as prefix0.String},
        this.pesos = { for (final prefix0.MapEntry(:key, :value) in (json[r'pesos'] as prefix0.Map<prefix0.String, prefix0.Object?>).entries) key: value as prefix0.double},
        this.casa = prefix1.Endereco.fromJson(json[r'casa'] as prefix0.Map<prefix0.String, prefix0.Object?>),
        this.outros = json[r'outros'] == null ? null : [ for (final item in json[r'outros'] as prefix0.List<prefix0.Object?>) prefix1.Endereco.fromJson(item as prefix0.Map<prefix0.String, prefix0.Object?>)],
        this.criado = prefix0.DateTime.parse(json[r'criado'] as prefix0.String),
        this.ativo = json[r'ativo'] as prefix0.bool;
  augment prefix0.Map<prefix0.String, prefix0.Object?> toJson() {
    final json = <prefix0.String, prefix0.Object?>{};
    json[r'nome'] = this.nome;
    json[r'idade'] = this.idade;
    if (this.apelido != null) {
      json[r'apelido'] = this.apelido!;
    }
    json[r'notas'] = [ for (final item in this.notas) item];
    json[r'tags'] = [ for (final item in this.tags) item];
    json[r'pesos'] = { for (final prefix0.MapEntry(:key, :value) in this.pesos.entries) key: value};
    json[r'casa'] = this.casa.toJson();
    if (this.outros != null) {
      json[r'outros'] = [ for (final item in this.outros!) item.toJson()];
    }
    json[r'criado'] = this.criado.toIso8601String();
    json[r'ativo'] = this.ativo;
    return json;
  }
}
augment class SoSaida {
  external prefix0.Map<prefix0.String, prefix0.Object?> toJson();
  augment prefix0.Map<prefix0.String, prefix0.Object?> toJson() {
    final json = <prefix0.String, prefix0.Object?>{};
    json[r'valor'] = this.valor;
    return json;
  }
}
augment class SoEntrada {
  external SoEntrada.fromJson(prefix0.Map<prefix0.String, prefix0.Object?> json);
  augment SoEntrada.fromJson(prefix0.Map<prefix0.String, prefix0.Object?> json, )
      : this.codigo = json[r'codigo'] as prefix0.String;
}
