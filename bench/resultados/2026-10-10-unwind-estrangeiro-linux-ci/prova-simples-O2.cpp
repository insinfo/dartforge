// Prova do cleanup emitido contra exceção C++ e heap real do runtime.
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>

extern "C" {
void prova_owner_estrangeiro();
void dartforge_memoria_arc_v1();
void dartforge_arc_collect();
std::uint8_t dartforge_arc_observar_heap_v1(std::int64_t);
std::uint8_t dartforge_exception_pending();
void* dartforge_contexto();
}

static std::int64_t endereco_owner;
static int chamadas_print;
static int destruicoes_local;
static int destruicoes_excecao;
static const void* endereco_excecao;

static void conferir(bool condicao, int codigo, const char* mensagem) {
    if (!condicao) {
        std::fprintf(stderr, "%s\n", mensagem);
        std::exit(codigo);
    }
}

struct Marcador {
    int valor;
    explicit Marcador(int v) : valor(v) { endereco_excecao = this; }
    Marcador(const Marcador& outro) : valor(outro.valor) { endereco_excecao = this; }
    ~Marcador() { ++destruicoes_excecao; }
};

struct Local {
    ~Local() { ++destruicoes_local; }
};

// Hook de fault injection com a ABI Borrow de print_handle.
extern "C" void prova_print_estrangeira(std::int64_t endereco) {
    ++chamadas_print;
    if (chamadas_print != 1) return;
    endereco_owner = endereco;
    conferir(dartforge_arc_observar_heap_v1(endereco) == 3, 11,
             "Mint deve estar vivo e ter owner antes do unwind");
    Local local;
    throw Marcador(42);
}

static void* topo() {
    void* resultado;
    std::memcpy(&resultado, static_cast<unsigned char*>(dartforge_contexto()) + 8,
                sizeof(resultado));
    return resultado;
}

int main(int argc, char** argv) {
    if (argc == 2 && std::strcmp(argv[1], "arc") == 0) dartforge_memoria_arc_v1();
    void* anterior = topo();
    bool capturada = false;
    try {
        prova_owner_estrangeiro();
    } catch (const Marcador& erro) {
        capturada = true;
        conferir(erro.valor == 42 && &erro == endereco_excecao, 12,
                 "objeto nativo da excecao mudou no cleanup");
        conferir(chamadas_print == 1, 13, "catch Dart recebeu excecao estrangeira");
        conferir(destruicoes_local == 1, 14, "destrutor C++ local nao executou uma vez");
        conferir(topo() == anterior, 15, "quadro de raizes abandonado no contexto");
        conferir(dartforge_exception_pending() == 0, 16, "pendencia Dart foi modificada");
        conferir(dartforge_arc_observar_heap_v1(endereco_owner) == 1, 17,
                 "cleanup deve consumir owner sem coletar o Mint");
        dartforge_arc_collect();
        conferir(dartforge_arc_observar_heap_v1(endereco_owner) == 0, 18,
                 "Mint permaneceu vivo apos cleanup e coleta");
    }
    conferir(capturada && destruicoes_excecao == 1, 19,
             "excecao C++ nao foi capturada e destruida uma vez");
    std::puts("foreign-cleanup-ok");
    return 0;
}
