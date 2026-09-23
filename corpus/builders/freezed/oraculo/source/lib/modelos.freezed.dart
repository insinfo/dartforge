// dart format width=80
// coverage:ignore-file
// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'modelos.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;

/// @nodoc
mixin _$Endereco {
  String get rua;
  int get numero;

  /// Create a copy of Endereco
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $EnderecoCopyWith<Endereco> get copyWith =>
      _$EnderecoCopyWithImpl<Endereco>(this as Endereco, _$identity);

  /// Serializes this Endereco to a JSON map.
  Map<String, dynamic> toJson();

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is Endereco &&
            (identical(other.rua, rua) || other.rua == rua) &&
            (identical(other.numero, numero) || other.numero == numero));
  }

  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  int get hashCode => Object.hash(runtimeType, rua, numero);

  @override
  String toString() {
    return 'Endereco(rua: $rua, numero: $numero)';
  }
}

/// @nodoc
abstract mixin class $EnderecoCopyWith<$Res> {
  factory $EnderecoCopyWith(Endereco value, $Res Function(Endereco) _then) =
      _$EnderecoCopyWithImpl;
  @useResult
  $Res call({String rua, int numero});
}

/// @nodoc
class _$EnderecoCopyWithImpl<$Res> implements $EnderecoCopyWith<$Res> {
  _$EnderecoCopyWithImpl(this._self, this._then);

  final Endereco _self;
  final $Res Function(Endereco) _then;

  /// Create a copy of Endereco
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? rua = null,
    Object? numero = null,
  }) {
    return _then(_self.copyWith(
      rua: null == rua
          ? _self.rua
          : rua // ignore: cast_nullable_to_non_nullable
              as String,
      numero: null == numero
          ? _self.numero
          : numero // ignore: cast_nullable_to_non_nullable
              as int,
    ));
  }
}

/// @nodoc
@JsonSerializable()
class _Endereco implements Endereco {
  const _Endereco({required this.rua, this.numero = 0});
  factory _Endereco.fromJson(Map<String, dynamic> json) =>
      _$EnderecoFromJson(json);

  @override
  final String rua;
  @override
  @JsonKey()
  final int numero;

  /// Create a copy of Endereco
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  _$EnderecoCopyWith<_Endereco> get copyWith =>
      __$EnderecoCopyWithImpl<_Endereco>(this, _$identity);

  @override
  Map<String, dynamic> toJson() {
    return _$EnderecoToJson(
      this,
    );
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _Endereco &&
            (identical(other.rua, rua) || other.rua == rua) &&
            (identical(other.numero, numero) || other.numero == numero));
  }

  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  int get hashCode => Object.hash(runtimeType, rua, numero);

  @override
  String toString() {
    return 'Endereco(rua: $rua, numero: $numero)';
  }
}

/// @nodoc
abstract mixin class _$EnderecoCopyWith<$Res>
    implements $EnderecoCopyWith<$Res> {
  factory _$EnderecoCopyWith(_Endereco value, $Res Function(_Endereco) _then) =
      __$EnderecoCopyWithImpl;
  @override
  @useResult
  $Res call({String rua, int numero});
}

/// @nodoc
class __$EnderecoCopyWithImpl<$Res> implements _$EnderecoCopyWith<$Res> {
  __$EnderecoCopyWithImpl(this._self, this._then);

  final _Endereco _self;
  final $Res Function(_Endereco) _then;

  /// Create a copy of Endereco
  /// with the given fields replaced by the non-null parameter values.
  @override
  @pragma('vm:prefer-inline')
  $Res call({
    Object? rua = null,
    Object? numero = null,
  }) {
    return _then(_Endereco(
      rua: null == rua
          ? _self.rua
          : rua // ignore: cast_nullable_to_non_nullable
              as String,
      numero: null == numero
          ? _self.numero
          : numero // ignore: cast_nullable_to_non_nullable
              as int,
    ));
  }
}

/// @nodoc
mixin _$Pessoa {
  int get id;
  String get nome;
  @JsonKey(name: 'e_mail')
  String? get email;
  List<String> get apelidos;
  Endereco? get endereco;

  /// Create a copy of Pessoa
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $PessoaCopyWith<Pessoa> get copyWith =>
      _$PessoaCopyWithImpl<Pessoa>(this as Pessoa, _$identity);

  /// Serializes this Pessoa to a JSON map.
  Map<String, dynamic> toJson();

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is Pessoa &&
            (identical(other.id, id) || other.id == id) &&
            (identical(other.nome, nome) || other.nome == nome) &&
            (identical(other.email, email) || other.email == email) &&
            const DeepCollectionEquality().equals(other.apelidos, apelidos) &&
            (identical(other.endereco, endereco) ||
                other.endereco == endereco));
  }

  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  int get hashCode => Object.hash(runtimeType, id, nome, email,
      const DeepCollectionEquality().hash(apelidos), endereco);

  @override
  String toString() {
    return 'Pessoa(id: $id, nome: $nome, email: $email, apelidos: $apelidos, endereco: $endereco)';
  }
}

/// @nodoc
abstract mixin class $PessoaCopyWith<$Res> {
  factory $PessoaCopyWith(Pessoa value, $Res Function(Pessoa) _then) =
      _$PessoaCopyWithImpl;
  @useResult
  $Res call(
      {int id,
      String nome,
      @JsonKey(name: 'e_mail') String? email,
      List<String> apelidos,
      Endereco? endereco});

  $EnderecoCopyWith<$Res>? get endereco;
}

/// @nodoc
class _$PessoaCopyWithImpl<$Res> implements $PessoaCopyWith<$Res> {
  _$PessoaCopyWithImpl(this._self, this._then);

  final Pessoa _self;
  final $Res Function(Pessoa) _then;

  /// Create a copy of Pessoa
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? id = null,
    Object? nome = null,
    Object? email = freezed,
    Object? apelidos = null,
    Object? endereco = freezed,
  }) {
    return _then(_self.copyWith(
      id: null == id
          ? _self.id
          : id // ignore: cast_nullable_to_non_nullable
              as int,
      nome: null == nome
          ? _self.nome
          : nome // ignore: cast_nullable_to_non_nullable
              as String,
      email: freezed == email
          ? _self.email
          : email // ignore: cast_nullable_to_non_nullable
              as String?,
      apelidos: null == apelidos
          ? _self.apelidos
          : apelidos // ignore: cast_nullable_to_non_nullable
              as List<String>,
      endereco: freezed == endereco
          ? _self.endereco
          : endereco // ignore: cast_nullable_to_non_nullable
              as Endereco?,
    ));
  }

  /// Create a copy of Pessoa
  /// with the given fields replaced by the non-null parameter values.
  @override
  @pragma('vm:prefer-inline')
  $EnderecoCopyWith<$Res>? get endereco {
    if (_self.endereco == null) {
      return null;
    }

    return $EnderecoCopyWith<$Res>(_self.endereco!, (value) {
      return _then(_self.copyWith(endereco: value));
    });
  }
}

/// @nodoc
@JsonSerializable()
class _Pessoa extends Pessoa {
  const _Pessoa(
      {required this.id,
      required this.nome,
      @JsonKey(name: 'e_mail') this.email,
      final List<String> apelidos = const <String>[],
      this.endereco})
      : _apelidos = apelidos,
        super._();
  factory _Pessoa.fromJson(Map<String, dynamic> json) => _$PessoaFromJson(json);

  @override
  final int id;
  @override
  final String nome;
  @override
  @JsonKey(name: 'e_mail')
  final String? email;
  final List<String> _apelidos;
  @override
  @JsonKey()
  List<String> get apelidos {
    if (_apelidos is EqualUnmodifiableListView) return _apelidos;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableListView(_apelidos);
  }

  @override
  final Endereco? endereco;

  /// Create a copy of Pessoa
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  _$PessoaCopyWith<_Pessoa> get copyWith =>
      __$PessoaCopyWithImpl<_Pessoa>(this, _$identity);

  @override
  Map<String, dynamic> toJson() {
    return _$PessoaToJson(
      this,
    );
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _Pessoa &&
            (identical(other.id, id) || other.id == id) &&
            (identical(other.nome, nome) || other.nome == nome) &&
            (identical(other.email, email) || other.email == email) &&
            const DeepCollectionEquality().equals(other._apelidos, _apelidos) &&
            (identical(other.endereco, endereco) ||
                other.endereco == endereco));
  }

  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  int get hashCode => Object.hash(runtimeType, id, nome, email,
      const DeepCollectionEquality().hash(_apelidos), endereco);

  @override
  String toString() {
    return 'Pessoa(id: $id, nome: $nome, email: $email, apelidos: $apelidos, endereco: $endereco)';
  }
}

/// @nodoc
abstract mixin class _$PessoaCopyWith<$Res> implements $PessoaCopyWith<$Res> {
  factory _$PessoaCopyWith(_Pessoa value, $Res Function(_Pessoa) _then) =
      __$PessoaCopyWithImpl;
  @override
  @useResult
  $Res call(
      {int id,
      String nome,
      @JsonKey(name: 'e_mail') String? email,
      List<String> apelidos,
      Endereco? endereco});

  @override
  $EnderecoCopyWith<$Res>? get endereco;
}

/// @nodoc
class __$PessoaCopyWithImpl<$Res> implements _$PessoaCopyWith<$Res> {
  __$PessoaCopyWithImpl(this._self, this._then);

  final _Pessoa _self;
  final $Res Function(_Pessoa) _then;

  /// Create a copy of Pessoa
  /// with the given fields replaced by the non-null parameter values.
  @override
  @pragma('vm:prefer-inline')
  $Res call({
    Object? id = null,
    Object? nome = null,
    Object? email = freezed,
    Object? apelidos = null,
    Object? endereco = freezed,
  }) {
    return _then(_Pessoa(
      id: null == id
          ? _self.id
          : id // ignore: cast_nullable_to_non_nullable
              as int,
      nome: null == nome
          ? _self.nome
          : nome // ignore: cast_nullable_to_non_nullable
              as String,
      email: freezed == email
          ? _self.email
          : email // ignore: cast_nullable_to_non_nullable
              as String?,
      apelidos: null == apelidos
          ? _self._apelidos
          : apelidos // ignore: cast_nullable_to_non_nullable
              as List<String>,
      endereco: freezed == endereco
          ? _self.endereco
          : endereco // ignore: cast_nullable_to_non_nullable
              as Endereco?,
    ));
  }

  /// Create a copy of Pessoa
  /// with the given fields replaced by the non-null parameter values.
  @override
  @pragma('vm:prefer-inline')
  $EnderecoCopyWith<$Res>? get endereco {
    if (_self.endereco == null) {
      return null;
    }

    return $EnderecoCopyWith<$Res>(_self.endereco!, (value) {
      return _then(_self.copyWith(endereco: value));
    });
  }
}

Forma _$FormaFromJson(Map<String, dynamic> json) {
  switch (json['tipo']) {
    case 'circulo':
      return Circulo.fromJson(json);
    case 'retangulo':
      return Retangulo.fromJson(json);

    default:
      throw CheckedFromJsonException(
          json, 'tipo', 'Forma', 'Invalid union type "${json['tipo']}"!');
  }
}

/// @nodoc
mixin _$Forma {
  /// Serializes this Forma to a JSON map.
  Map<String, dynamic> toJson();

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType && other is Forma);
  }

  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() {
    return 'Forma()';
  }
}

/// @nodoc
class $FormaCopyWith<$Res> {
  $FormaCopyWith(Forma _, $Res Function(Forma) __);
}

/// @nodoc
@JsonSerializable()
class Circulo implements Forma {
  const Circulo({required this.raio, final String? $type})
      : $type = $type ?? 'circulo';
  factory Circulo.fromJson(Map<String, dynamic> json) =>
      _$CirculoFromJson(json);

  final double raio;

  @JsonKey(name: 'tipo')
  final String $type;

  /// Create a copy of Forma
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $CirculoCopyWith<Circulo> get copyWith =>
      _$CirculoCopyWithImpl<Circulo>(this, _$identity);

  @override
  Map<String, dynamic> toJson() {
    return _$CirculoToJson(
      this,
    );
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is Circulo &&
            (identical(other.raio, raio) || other.raio == raio));
  }

  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  int get hashCode => Object.hash(runtimeType, raio);

  @override
  String toString() {
    return 'Forma.circulo(raio: $raio)';
  }
}

/// @nodoc
abstract mixin class $CirculoCopyWith<$Res> implements $FormaCopyWith<$Res> {
  factory $CirculoCopyWith(Circulo value, $Res Function(Circulo) _then) =
      _$CirculoCopyWithImpl;
  @useResult
  $Res call({double raio});
}

/// @nodoc
class _$CirculoCopyWithImpl<$Res> implements $CirculoCopyWith<$Res> {
  _$CirculoCopyWithImpl(this._self, this._then);

  final Circulo _self;
  final $Res Function(Circulo) _then;

  /// Create a copy of Forma
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  $Res call({
    Object? raio = null,
  }) {
    return _then(Circulo(
      raio: null == raio
          ? _self.raio
          : raio // ignore: cast_nullable_to_non_nullable
              as double,
    ));
  }
}

/// @nodoc
@JsonSerializable()
class Retangulo implements Forma {
  const Retangulo(
      {required this.largura, required this.altura, final String? $type})
      : $type = $type ?? 'retangulo';
  factory Retangulo.fromJson(Map<String, dynamic> json) =>
      _$RetanguloFromJson(json);

  final double largura;
  final double altura;

  @JsonKey(name: 'tipo')
  final String $type;

  /// Create a copy of Forma
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $RetanguloCopyWith<Retangulo> get copyWith =>
      _$RetanguloCopyWithImpl<Retangulo>(this, _$identity);

  @override
  Map<String, dynamic> toJson() {
    return _$RetanguloToJson(
      this,
    );
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is Retangulo &&
            (identical(other.largura, largura) || other.largura == largura) &&
            (identical(other.altura, altura) || other.altura == altura));
  }

  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  int get hashCode => Object.hash(runtimeType, largura, altura);

  @override
  String toString() {
    return 'Forma.retangulo(largura: $largura, altura: $altura)';
  }
}

/// @nodoc
abstract mixin class $RetanguloCopyWith<$Res> implements $FormaCopyWith<$Res> {
  factory $RetanguloCopyWith(Retangulo value, $Res Function(Retangulo) _then) =
      _$RetanguloCopyWithImpl;
  @useResult
  $Res call({double largura, double altura});
}

/// @nodoc
class _$RetanguloCopyWithImpl<$Res> implements $RetanguloCopyWith<$Res> {
  _$RetanguloCopyWithImpl(this._self, this._then);

  final Retangulo _self;
  final $Res Function(Retangulo) _then;

  /// Create a copy of Forma
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  $Res call({
    Object? largura = null,
    Object? altura = null,
  }) {
    return _then(Retangulo(
      largura: null == largura
          ? _self.largura
          : largura // ignore: cast_nullable_to_non_nullable
              as double,
      altura: null == altura
          ? _self.altura
          : altura // ignore: cast_nullable_to_non_nullable
              as double,
    ));
  }
}

// dart format on
