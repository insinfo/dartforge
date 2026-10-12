// O mesmo protocolo de bench/http/servidor.dart: GET / -> "hello",
// GET /json -> um objeto pequeno, GET /sair encerra; imprime `porta <n>`.
import Foundation
import NIOCore
import NIOHTTP1
import NIOPosix

final class Contador: @unchecked Sendable {
    private let trava = NSLock()
    private var n = 0
    func proximo() -> Int { trava.lock(); defer { trava.unlock() }; n += 1; return n }
}

final class Tratador: ChannelInboundHandler, @unchecked Sendable {
    typealias InboundIn = HTTPServerRequestPart
    typealias OutboundOut = HTTPServerResponsePart
    let contador: Contador
    var caminho = "/"
    var keepAlive = true
    init(_ c: Contador) { contador = c }

    func channelRead(context: ChannelHandlerContext, data: NIOAny) {
        switch unwrapInboundIn(data) {
        case .head(let cab):
            caminho = cab.uri
            keepAlive = cab.isKeepAlive
        case .body:
            break
        case .end:
            var tipo = "text/plain"
            var texto = "hello"
            if caminho == "/json" {
                tipo = "application/json"
                let obj: [String: Any] = ["mensagem": "Hello, World!", "id": contador.proximo(), "ativo": true, "itens": [1, 2, 3]]
                texto = String(decoding: try! JSONSerialization.data(withJSONObject: obj), as: UTF8.self)
            } else if caminho == "/sair" {
                texto = "tchau"
            }
            var corpo = context.channel.allocator.buffer(capacity: texto.utf8.count)
            corpo.writeString(texto)
            var cab = HTTPHeaders()
            cab.add(name: "Content-Type", value: tipo)
            cab.add(name: "Content-Length", value: String(corpo.readableBytes))
            context.write(wrapOutboundOut(.head(HTTPResponseHead(version: .http1_1, status: .ok, headers: cab))), promise: nil)
            context.write(wrapOutboundOut(.body(.byteBuffer(corpo))), promise: nil)
            let sair = caminho == "/sair"
            context.writeAndFlush(wrapOutboundOut(.end(nil))).whenComplete { _ in
                if sair { exit(0) }
                if !self.keepAlive { context.close(promise: nil) }
            }
        }
    }
}

let porta = CommandLine.arguments.count > 1 ? Int(CommandLine.arguments[1])! : 0
let grupo = MultiThreadedEventLoopGroup(numberOfThreads: System.coreCount)
let contador = Contador()
let canal = try ServerBootstrap(group: grupo)
    .serverChannelOption(ChannelOptions.backlog, value: 256)
    .serverChannelOption(ChannelOptions.socketOption(.so_reuseaddr), value: 1)
    .childChannelInitializer { canal in
        canal.pipeline.configureHTTPServerPipeline().flatMap { canal.pipeline.addHandler(Tratador(contador)) }
    }
    .childChannelOption(ChannelOptions.socketOption(.so_reuseaddr), value: 1)
    .bind(host: "127.0.0.1", port: porta).wait()
print("porta \(canal.localAddress!.port!)")
fflush(stdout)
try canal.closeFuture.wait()
