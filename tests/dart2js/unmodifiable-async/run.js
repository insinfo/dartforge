// A saída do dart2js mira o navegador: sem `self` ela nem carrega, e sem os
// ganchos abaixo o erro assíncrono é engolido em vez de encerrar o processo.
globalThis.self = globalThis;
process.on('uncaughtException', (error) => {
  console.log('UNCAUGHT: ' + error);
  process.exit(1);
});
process.on('unhandledRejection', (error) => {
  console.log('UNHANDLED: ' + error);
  process.exit(1);
});
require(require('path').resolve(process.argv[2]));
