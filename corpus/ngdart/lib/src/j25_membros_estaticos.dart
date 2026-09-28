import 'package:ngdart/angular.dart';

/// Membros estáticos do componente lidos sem qualificação no template (os
/// `static const String apiSnippet` das páginas do limitless_ui).
@Component(
  selector: 'j25-membros-estaticos',
  templateUrl: 'j25_membros_estaticos.html',
)
class J25MembrosEstaticos {
  static const String constante = 'c';
  static final String fixo = 'f';
  static String mutavel = 'm';
  static int contador = 0;
  static String get calculado => 'g$contador';
  static String? talvez;
}
