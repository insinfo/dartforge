import 'package:ngdart/angular.dart';
import 'package:dep_ng/dep_ng.dart';

@Component(
  selector: 'app',
  template: '<dep-botao [rotulo]="texto"></dep-botao>',
  directives: [DepBotao],
)
class App {
  String texto = 'ok';
}
