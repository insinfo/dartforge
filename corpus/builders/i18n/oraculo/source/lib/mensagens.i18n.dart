// GENERATED FILE, do not edit!
// ignore_for_file: annotate_overrides, non_constant_identifier_names, prefer_single_quotes, unused_element, unused_field
import 'package:i18n/i18n.dart' as i18n;

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

class Mensagens {
  const Mensagens();
  String get locale => "en";
  String get languageCode => "en";
  GeralMensagens get geral => GeralMensagens(this);
  MenuMensagens get menu => MenuMensagens(this);
}

class GeralMensagens {
  final Mensagens _parent;
  const GeralMensagens(this._parent);

  /// ```dart
  /// "Exemplo de mensagens"
  /// ```
  String get titulo => """Exemplo de mensagens""";

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

class MenuMensagens {
  final Mensagens _parent;
  const MenuMensagens(this._parent);

  /// ```dart
  /// "Abrir"
  /// ```
  String get abrir => """Abrir""";
  SubMenuMenuMensagens get subMenu => SubMenuMenuMensagens(this);
}

class SubMenuMenuMensagens {
  final MenuMensagens _parent;
  const SubMenuMenuMensagens(this._parent);

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

Map<String, String> get mensagensMap => {
      """geral.titulo""": """Exemplo de mensagens""",
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
