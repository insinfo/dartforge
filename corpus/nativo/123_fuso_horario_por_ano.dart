// O fuso local de um instante é o das regras do fuso no ANO do instante,
// como a VM (runtime/vm/os_win.cc: `LocalTime` com
// `GetTimeZoneInformationForYear`; no Unix, `localtime_r`). O nativo no
// Windows usava o estado de agora para qualquer instante: no fuso de
// Brasília, `DateTime(2019)` saía 00:00 onde a VM dá 01:00 (a meia-noite
// caiu no horário de verão daquele ano) e os instantes de verão de anos
// passados vinham com o deslocamento errado (o `equatable` do pub;
// docs/NATIVO-PROJETOS-REAIS.md). Num fuso sem horário de verão (o runner
// do CI, UTC) as linhas só conferem o caminho.

void main() {
  for (final y in [1970, 1999, 2000, 2018, 2019, 2020, 2024]) {
    for (final m in [1, 2, 3, 6, 10, 11, 12]) {
      final d = DateTime(y, m, 1);
      print('$y-$m ${d.hour} ${d.timeZoneOffset} ${d.timeZoneName} '
          '${d.millisecondsSinceEpoch}');
    }
  }
  final u = DateTime.utc(2019, 1, 1, 3).toLocal();
  print('${u.year}-${u.month}-${u.day} ${u.hour}:${u.minute} ${u.timeZoneOffset}');
  print(DateTime.fromMillisecondsSinceEpoch(0).timeZoneOffset);
}
