import 'dart:io';
Future<void> main(List<String> arguments) async {
  print('before\ninside');
  print(String.fromCharCodes([0xD800, 10, 97]));
  switch (arguments.single) {
    case 'none': break;
    case 'stdout-get': identical(stdout, stdout); break;
    case 'stdout-terminal': stdout.hasTerminal; break;
    case 'stdout-write-empty': stdout.write(''); break;
    case 'stdout-flush': await stdout.flush(); break;
    case 'stdout-write': stdout.write('raw\nline\r\n'); break;
    case 'stderr-get': identical(stderr, stderr); break;
    case 'stderr-write': stderr.write('err\nline\r\n'); break;
    case 'stdin-get': identical(stdin, stdin); break;
    case 'platform': Platform.lineTerminator; break;
    case 'stdout-line-terminator':
      stdout.lineTerminator = Platform.lineTerminator;
      stdout.writeln('sink\nbody');
      break;
    default: throw ArgumentError(arguments);
  }
  print('after\ninside\r\nend\u0000é😀');
  print(7); print(true); print(1.0); print(null);
}
