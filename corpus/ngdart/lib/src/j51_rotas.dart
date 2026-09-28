class J51Caminho {
  final String valor;
  const J51Caminho(this.valor);
  String url() => '/$valor';
}

/// Classe só de estáticos, exportada para o template (o `DemoRoutePaths` do
/// limitless_ui).
class J51Rotas {
  static const String titulo = 'Rotas';
  static const inicio = J51Caminho('inicio');
  static String get agora => 'agora';
  static void registrar(String v) {}
}

const String j51Versao = '1.0';

String j51Formatar(String v) => '<$v>';

enum J51Modo { claro, escuro }
