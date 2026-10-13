// HTTP server of the comparison (RELATORIO-COMPARATIVO-NATIVO.md), the same
// protocol as bench/http/server.dart, through `node:http`.
import * as http from "node:http";

let n = 0;
const servidor = http.createServer((req, res) => {
  if (req.url === "/json") {
    n++;
    res.setHeader("Content-Type", "application/json");
    res.end(JSON.stringify({ message: "Hello, World!", id: n, active: true, items: [1, 2, 3] }));
  } else if (req.url === "/quit") {
    res.end("bye");
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
  console.log("port " + a.port);
});
