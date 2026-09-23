import 'package:ngdart/angular.dart';

/// Dois `*ngIf` irmãos, o primeiro com um aninhado dentro: se a numeração
/// for por nível, o segundo irmão colide com o aninhado.
@Component(
  selector: 'f02-dois-ifs-irmaos',
  templateUrl: 'f02_dois_ifs_irmaos.html',
  directives: [coreDirectives],
)
class F02DoisIfsIrmaos {
  bool a = true;
  bool b = true;
  bool c = true;
}
