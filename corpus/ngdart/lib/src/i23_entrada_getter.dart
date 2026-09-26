import 'package:ngdart/angular.dart';

/// Sonda: getters em ligação e interpolação.
@Component(
  selector: 'i23-entrada-getter',
  templateUrl: 'i23_entrada_getter.html',
  directives: [coreDirectives],
)
class I23EntradaGetter {
  List<int> xs = [1];
  String get rotulo => 'r';
  bool get vazio => xs.isEmpty;
  int get total => xs.length;
}
