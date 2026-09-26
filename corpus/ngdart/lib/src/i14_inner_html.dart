import 'package:ngdart/angular.dart';

/// Sonda: `[innerHtml]` e `[href]`: contexto de segurança.
@Component(
  selector: 'i14-inner-html',
  templateUrl: 'i14_inner_html.html',
  directives: [coreDirectives],
)
class I14InnerHtml {
  String html = '<b>x</b>';
  String url = 'http://a';
}
