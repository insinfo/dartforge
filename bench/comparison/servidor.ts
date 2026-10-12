// Servidor HTTP do comparativo (RELATORIO-COMPARATIVO-NATIVO.md), o mesmo
// protocolo de bench/http/servidor.dart, pelo `node:http`.
import * as http from "node:http";

let n = 0;
const servidor = http.createServer((req, res) => {
  if (req.url === "/json") {
    n++;
    res.setHeader("Content-Type", "application/json");
    res.end(JSON.stringify({ mensagem: "Hello, World!", id: n, ativo: true, itens: [1, 2, 3] }));
  } else if (req.url === "/sair") {
    res.end("tchau");
    servidor.close();
    process.exit(0);
  } else {
    res.setHeader("Content-Type", "text/plain");
    res.end("hello");
  }
});
const porta = process.argv.length > 2 ? parseInt(process.argv[2]) : 0;
servidor.listen(porta, "127.0.0.1", () => {
  const a = servidor.address() as any;
  console.log("porta " + a.port);
});
