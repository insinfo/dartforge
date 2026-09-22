import 'package:ngdart/angular.dart';

/// Uma interpolação só, de um campo `String`, sem mais nada:
/// `TextBinding`, `detectChangesInternal` e `interpolateString0`.
@Component(
  selector: 'a01-interpolacao',
  templateUrl: 'a01_interpolacao.html',
)
class A01Interpolacao {
  String mensagem = 'oi';
}
