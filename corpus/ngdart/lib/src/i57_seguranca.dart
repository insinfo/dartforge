import 'package:ngdart/angular.dart';

/// Sonda: propriedades com contexto de segurança (URL, recurso, HTML) ligadas e interpoladas.
@Component(
  selector: 'i57-seguranca',
  templateUrl: 'i57_seguranca.html',
  directives: [coreDirectives],
)
class I57Seguranca {
  int id = 1;
  String url = 'a.png';
  String html = '<b>x</b>';
}
