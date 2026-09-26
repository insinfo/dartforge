import 'package:ngdart/angular.dart';

/// Sonda: filho com entrada de função.
@Component(
  selector: 'i37-filho-callback',
  templateUrl: 'i37_filho_callback.html',
  directives: [],
)
class I37FilhoCallback {
  @Input()
  Function? acao;
}
