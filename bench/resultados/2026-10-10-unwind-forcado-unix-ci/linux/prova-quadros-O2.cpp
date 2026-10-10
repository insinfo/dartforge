// Prova do cleanup emitido contra exceção C++ e heap real do runtime.
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <unwind.h>

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
static bool forcar_unwind;
static bool classe_dart_no_forcado;
static void* topo_anterior;
static _Unwind_Exception objeto_forcado{};
static void* topo();


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

// Ao fim da travessia, todos os cleanups já devem ter executado.
static _Unwind_Reason_Code parar_forcado(
    int versao, _Unwind_Action acoes, _Unwind_Exception_Class classe,
    _Unwind_Exception* objeto, _Unwind_Context*, void* argumento) {
    conferir(versao == 1 && objeto == &objeto_forcado && argumento == &objeto_forcado
                 && classe == objeto_forcado.exception_class, 20,
             "objeto original do unwind forcado mudou");
    conferir((acoes & _UA_FORCE_UNWIND) != 0 && (acoes & _UA_CLEANUP_PHASE) != 0, 21,
             "fase incorreta no unwind forcado");
    if ((acoes & _UA_END_OF_STACK) == 0) return _URC_NO_REASON;
    conferir(chamadas_print == 1 && destruicoes_local == 1, 22,
             "unwind forcado executou catch ou esqueceu destrutor local");
    conferir(topo() == topo_anterior && dartforge_exception_pending() == 0, 23,
             "unwind forcado alterou pendencia ou abandonou raizes");
    conferir(dartforge_arc_observar_heap_v1(endereco_owner) == 1, 24,
             "unwind forcado deixou owner ou coletou antes do fim");
    dartforge_arc_collect();
    conferir(dartforge_arc_observar_heap_v1(endereco_owner) == 0, 25,
             "unwind forcado deixou Mint vivo apos coleta");
    std::puts("forced-cleanup-ok");
    std::fflush(stdout);
    std::_Exit(0);
}

static void limpar_forcado(_Unwind_Reason_Code, _Unwind_Exception*) {
    conferir(false, 26, "objeto de unwind forcado foi descartado antes do fim");
}

// Hook de fault injection com a ABI Borrow de print_handle.
extern "C" void prova_print_estrangeira(std::int64_t endereco) {
    ++chamadas_print;
    if (chamadas_print != 1) return;
    endereco_owner = endereco;
    conferir(dartforge_arc_observar_heap_v1(endereco) == 3, 11,
             "Mint deve estar vivo e ter owner antes do unwind");
    Local local;
    if (forcar_unwind) {
        // Mesmo com a classe Dart, FORCE_UNWIND não autoriza tratar o catch.
        // O objeto é de fault injection; não cria pendência nem payload Dart.
        objeto_forcado.exception_class = classe_dart_no_forcado
            ? UINT64_C(0x4441525446524745) : UINT64_C(0x4446544553544621);
        objeto_forcado.exception_cleanup = limpar_forcado;
        _Unwind_ForcedUnwind(&objeto_forcado, parar_forcado, &objeto_forcado);
        conferir(false, 27, "unwind forcado retornou ao lancador");
    }
    throw Marcador(42);
}

static void* topo() {
    void* resultado;
    std::memcpy(&resultado, static_cast<unsigned char*>(dartforge_contexto()) + 8,
                sizeof(resultado));
    return resultado;
}

int main(int argc, char** argv) {
    if (argc >= 2 && std::strcmp(argv[1], "arc") == 0) dartforge_memoria_arc_v1();
    classe_dart_no_forcado = argc >= 3 && std::strcmp(argv[2], "forcado_dart") == 0;
    forcar_unwind = classe_dart_no_forcado || (argc >= 3 && std::strcmp(argv[2], "forcado") == 0);
    void* anterior = topo();
    topo_anterior = anterior;
    bool capturada = false;
    try {
        prova_owner_estrangeiro();
    } catch (const Marcador& erro) {
        conferir(!forcar_unwind, 28, "catch C++ interceptou unwind forcado");
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
