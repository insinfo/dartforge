// GENERATED FILE, do not edit!
// ignore_for_file: annotate_overrides, non_constant_identifier_names, prefer_single_quotes, unused_element, unused_field
import 'package:i18n/i18n.dart' as i18n;
import 'mensagens.i18n.dart';

String get _languageCode => 'en';
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

class MensagensEn extends Mensagens {
  const MensagensEn();
  String get locale => "en";
  String get languageCode => "en";
  GeralMensagensEn get geral => GeralMensagensEn(this);
  MenuMensagensEn get menu => MenuMensagensEn(this);
}

class GeralMensagensEn extends GeralMensagens {
  final MensagensEn _parent;
  const GeralMensagensEn(this._parent) : super(_parent);

  /// ```dart
  /// "Sample messages"
  /// ```
  String get titulo => """Sample messages""";

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
  /// "Key with spaces"
  /// ```
  String get comEspacos => """Key with spaces""";

  /// ```dart
  /// "Key with hyphen"
  /// ```
  String get comHifen => """Key with hyphen""";

  /// ```dart
  /// "Hello, $nome!"
  /// ```
  String saudacao(String nome) => """Hello, $nome!""";

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

class MenuMensagensEn extends MenuMensagens {
  final MensagensEn _parent;
  const MenuMensagensEn(this._parent) : super(_parent);

  /// ```dart
  /// "Open"
  /// ```
  String get abrir => """Open""";
  SubMenuMenuMensagensEn get subMenu => SubMenuMenuMensagensEn(this);
}

class SubMenuMenuMensagensEn extends SubMenuMenuMensagens {
  final MenuMensagensEn _parent;
  const SubMenuMenuMensagensEn(this._parent) : super(_parent);

  /// ```dart
  /// "Close"
  /// ```
  String get fechar => """Close""";

  /// ```dart
  /// "Outro texto comprido para testar a quebra do mapa gerado."
  /// ```
  String get detalheMuitoLongoDoSubmenuQueQuebra =>
      """Outro texto comprido para testar a quebra do mapa gerado.""";
}

Map<String, String> get mensagensEnMap => {
      """geral.titulo""": """Sample messages""",
      """geral.vazio""": """""",
      """geral.descricaoLonga""":
          """Um texto comprido o bastante para passar das oitenta colunas no getter gerado.""",
      """geral.com espacos""": """Key with spaces""",
      """geral.com-hifen""": """Key with hyphen""",
      """geral.preco""": """Custa \$5""",
      """geral.exemplo""": """// Uso
final m = Mensagens();

print(m.geral.titulo);
""",
      """menu.abrir""": """Open""",
      """menu.subMenu.fechar""": """Close""",
      """menu.subMenu.detalheMuitoLongoDoSubmenuQueQuebra""":
          """Outro texto comprido para testar a quebra do mapa gerado.""",
    };
