// GENERATED FILE, do not edit!
// ignore_for_file: annotate_overrides, non_constant_identifier_names, prefer_single_quotes, unused_element, unused_field
import 'package:i18n/i18n.dart' as i18n;
import 'mensagens.i18n.dart';

String get _languageCode => 'pt';
String _plural(
  int count, {
  String? zero,
  String? one,
  String? two,
  String? few,
  String? many,
  String? other,
}) =>
    i18n.plural(
      count,
      _languageCode,
      zero: zero,
      one: one,
      two: two,
      few: few,
      many: many,
      other: other,
    );
String _ordinal(
  int count, {
  String? zero,
  String? one,
  String? two,
  String? few,
  String? many,
  String? other,
}) =>
    i18n.ordinal(
      count,
      _languageCode,
      zero: zero,
      one: one,
      two: two,
      few: few,
      many: many,
      other: other,
    );
String _cardinal(
  int count, {
  String? zero,
  String? one,
  String? two,
  String? few,
  String? many,
  String? other,
}) =>
    i18n.cardinal(
      count,
      _languageCode,
      zero: zero,
      one: one,
      two: two,
      few: few,
      many: many,
      other: other,
    );

class MensagensPtBR extends Mensagens {
  const MensagensPtBR();
  String get locale => "pt_BR";
  String get languageCode => "pt";
  GeralMensagensPtBR get geral => GeralMensagensPtBR(this);
  MenuMensagensPtBR get menu => MenuMensagensPtBR(this);
}

class GeralMensagensPtBR extends GeralMensagens {
  final MensagensPtBR _parent;
  const GeralMensagensPtBR(this._parent) : super(_parent);

  /// ```dart
  /// "Exemplo brasileiro"
  /// ```
  String get titulo => """Exemplo brasileiro""";

  String get vazio => """""";

  /// ```dart
  /// "42"
  /// ```
  String get numero => """42""";

  /// ```dart
  /// "true"
  /// ```
  String get ligado => """true""";

  /// ```dart
  /// "Um texto comprido o bastante para passar das oitenta colunas no getter gerado."
  /// ```
  String get descricaoLonga =>
      """Um texto comprido o bastante para passar das oitenta colunas no getter gerado.""";

  /// ```dart
  /// "Chave com espaços"
  /// ```
  String get comEspacos => """Chave com espaços""";

  /// ```dart
  /// "Chave com hífen"
  /// ```
  String get comHifen => """Chave com hífen""";

  /// ```dart
  /// "Olá, $nome!"
  /// ```
  String saudacao(String nome) => """Olá, $nome!""";

  /// ```dart
  /// "Você tem $n itens na lista de compras que ainda não foram conferidos."
  /// ```
  String contagem(int n) =>
      """Você tem $n itens na lista de compras que ainda não foram conferidos.""";

  /// ```dart
  /// "Custa \$5"
  /// ```
  String get preco => """Custa \$5""";

  /// ```dart
  /// """
  /// // Uso
  /// final m = Mensagens();
  ///
  /// print(m.geral.titulo);
  /// """
  /// ```
  String get exemplo => """// Uso
final m = Mensagens();

print(m.geral.titulo);
""";
}

class MenuMensagensPtBR extends MenuMensagens {
  final MensagensPtBR _parent;
  const MenuMensagensPtBR(this._parent) : super(_parent);

  /// ```dart
  /// "Abrir"
  /// ```
  String get abrir => """Abrir""";
  SubMenuMenuMensagensPtBR get subMenu => SubMenuMenuMensagensPtBR(this);
}

class SubMenuMenuMensagensPtBR extends SubMenuMenuMensagens {
  final MenuMensagensPtBR _parent;
  const SubMenuMenuMensagensPtBR(this._parent) : super(_parent);

  /// ```dart
  /// "Fechar"
  /// ```
  String get fechar => """Fechar""";

  /// ```dart
  /// "Outro texto comprido para testar a quebra do mapa gerado."
  /// ```
  String get detalheMuitoLongoDoSubmenuQueQuebra =>
      """Outro texto comprido para testar a quebra do mapa gerado.""";
}

Map<String, String> get mensagensPtBRMap => {
      """geral.titulo""": """Exemplo brasileiro""",
      """geral.vazio""": """""",
      """geral.descricaoLonga""":
          """Um texto comprido o bastante para passar das oitenta colunas no getter gerado.""",
      """geral.com espacos""": """Chave com espaços""",
      """geral.com-hifen""": """Chave com hífen""",
      """geral.preco""": """Custa \$5""",
      """geral.exemplo""": """// Uso
final m = Mensagens();

print(m.geral.titulo);
""",
      """menu.abrir""": """Abrir""",
      """menu.subMenu.fechar""": """Fechar""",
      """menu.subMenu.detalheMuitoLongoDoSubmenuQueQuebra""":
          """Outro texto comprido para testar a quebra do mapa gerado.""",
    };
