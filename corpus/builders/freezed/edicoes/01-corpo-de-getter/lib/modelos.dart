// freezed e json_serializable na mesma classe: o freezed (build_to source,
// runs_before json_serializable) escreve `.freezed.dart`; o json_serializable
// lê a classe `_Pessoa` que só existe nessa saída e escreve a parte que o
// combining_builder junta em `.g.dart`. É uma cadeia entre builders.
import 'package:freezed_annotation/freezed_annotation.dart';

part 'modelos.freezed.dart';
part 'modelos.g.dart';

@freezed
abstract class Endereco with _$Endereco {
  const factory Endereco({required String rua, @Default(0) int numero}) = _Endereco;
  factory Endereco.fromJson(Map<String, dynamic> json) => _$EnderecoFromJson(json);
}

@freezed
abstract class Pessoa with _$Pessoa {
  const Pessoa._();
  const factory Pessoa({
    required int id,
    required String nome,
    @JsonKey(name: 'e_mail') String? email,
    @Default(<String>[]) List<String> apelidos,
    Endereco? endereco,
  }) = _Pessoa;
  factory Pessoa.fromJson(Map<String, dynamic> json) => _$PessoaFromJson(json);

  String get rotulo => '$nome#$id';
}

@Freezed(unionKey: 'tipo')
sealed class Forma with _$Forma {
  const factory Forma.circulo({required double raio}) = Circulo;
  const factory Forma.retangulo({required double largura, required double altura}) = Retangulo;
  factory Forma.fromJson(Map<String, dynamic> json) => _$FormaFromJson(json);
}

double area(Forma f) => switch (f) {
      Circulo(:final raio) => 3 * raio * raio,
      Retangulo(:final largura, :final altura) => largura * altura,
    };
