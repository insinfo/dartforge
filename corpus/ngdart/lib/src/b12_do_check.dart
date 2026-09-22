import 'package:ngdart/angular.dart';

/// `DoCheck`: o que o oficial emite na visão-hospedeira por causa dele.
@Component(
  selector: 'b12-do-check',
  templateUrl: 'b12_do_check.html',
)
class B12DoCheck implements DoCheck {
  @override
  void ngDoCheck() {}
}
