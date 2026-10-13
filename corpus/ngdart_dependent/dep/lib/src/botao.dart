import 'package:ngdart/angular.dart';

@Component(
  selector: 'dep-botao',
  templateUrl: 'botao.html',
)
class DepBotao {
  @Input()
  String rotulo = '';
}
