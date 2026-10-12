// Servidor HTTP do comparativo (RELATORIO-COMPARATIVO-NATIVO.md), o mesmo
// protocolo de bench/http/servidor.dart: GET / -> "hello", GET /json -> um
// objeto pequeno, GET /sair encerra; imprime `porta <n>` ao ouvir.
package main

import (
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"os"
	"sync/atomic"
)

func main() {
	porta := "0"
	if len(os.Args) > 1 {
		porta = os.Args[1]
	}
	ouvinte, err := net.Listen("tcp", "127.0.0.1:"+porta)
	if err != nil {
		panic(err)
	}
	fmt.Printf("porta %d\n", ouvinte.Addr().(*net.TCPAddr).Port)
	var n atomic.Int64
	mux := http.NewServeMux()
	mux.HandleFunc("/json", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		corpo, _ := json.Marshal(map[string]any{"mensagem": "Hello, World!", "id": n.Add(1), "ativo": true, "itens": []int{1, 2, 3}})
		w.Write(corpo)
	})
	mux.HandleFunc("/sair", func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte("tchau"))
		go os.Exit(0)
	})
	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/plain")
		w.Write([]byte("hello"))
	})
	http.Serve(ouvinte, mux)
}
