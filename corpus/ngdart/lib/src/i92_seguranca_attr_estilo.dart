import 'package:ngdart/angular.dart';

/// Sonda: `[attr.x]` e `[style]` com contexto de segurança.
@Component(
  selector: 'i92-seguranca-attr-estilo',
  templateUrl: 'i92_seguranca_attr_estilo.html',
)
class I92SegurancaAttrEstilo {
  String url = '/x';
  String estilo = 'color: red';
  final String fixo = '/f';
}
