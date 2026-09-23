import 'package:ngdart/angular.dart';

/// `pipes:` declarado e não usado; o `||` do template é OU lógico, não pipe.
@Component(
  selector: 'b19-pipes-sem-uso',
  templateUrl: 'b19_pipes_sem_uso.html',
  pipes: [commonPipes],
)
class B19PipesSemUso {
  bool a = false;
  bool b = false;
}