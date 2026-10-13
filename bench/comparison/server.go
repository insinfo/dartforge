// HTTP server of the comparison (RELATORIO-COMPARATIVO-NATIVO.md), the same
// protocol as bench/http/server.dart: GET / -> "hello", GET /json -> a small
// object, GET /quit stops; prints `port <n>` when listening.
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
	fmt.Printf("port %d\n", ouvinte.Addr().(*net.TCPAddr).Port)
	var n atomic.Int64
	mux := http.NewServeMux()
	mux.HandleFunc("/json", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		corpo, _ := json.Marshal(map[string]any{"message": "Hello, World!", "id": n.Add(1), "active": true, "items": []int{1, 2, 3}})
		w.Write(corpo)
	})
	mux.HandleFunc("/quit", func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte("bye"))
		go os.Exit(0)
	})
	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/plain")
		w.Write([]byte("hello"))
	})
	http.Serve(ouvinte, mux)
}
