// Gerador de carga HTTP do comparativo (RELATORIO-COMPARATIVO-NATIVO.md):
// `conexoes` clientes com keep-alive pedem `url` em laço por `segundos` e o
// resultado sai como `req/s <n> p50_us <n> p99_us <n> erros <n>`. Substitui o
// `wrk` onde ele não está instalado (macOS sem gerenciador de pacotes).
package main

import (
	"fmt"
	"io"
	"net/http"
	"os"
	"sort"
	"strconv"
	"sync"
	"time"
)

func main() {
	url := os.Args[1]
	conexoes, _ := strconv.Atoi(os.Args[2])
	segundos, _ := strconv.ParseFloat(os.Args[3], 64)
	transporte := &http.Transport{MaxIdleConns: conexoes, MaxIdleConnsPerHost: conexoes, MaxConnsPerHost: conexoes, DisableCompression: true}
	cliente := &http.Client{Transport: transporte}
	fim := time.Now().Add(time.Duration(segundos * float64(time.Second)))
	var mu sync.Mutex
	var lat []time.Duration
	erros := 0
	var wg sync.WaitGroup
	for c := 0; c < conexoes; c++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			local := make([]time.Duration, 0, 1<<16)
			e := 0
			for time.Now().Before(fim) {
				t0 := time.Now()
				r, err := cliente.Get(url)
				if err != nil {
					e++
					continue
				}
				io.Copy(io.Discard, r.Body)
				r.Body.Close()
				if r.StatusCode != 200 {
					e++
					continue
				}
				local = append(local, time.Since(t0))
			}
			mu.Lock()
			lat = append(lat, local...)
			erros += e
			mu.Unlock()
		}()
	}
	wg.Wait()
	sort.Slice(lat, func(i, j int) bool { return lat[i] < lat[j] })
	p := func(q float64) int64 {
		if len(lat) == 0 {
			return 0
		}
		return lat[int(q*float64(len(lat)-1))].Microseconds()
	}
	fmt.Printf("req/s %.0f p50_us %d p99_us %d erros %d\n", float64(len(lat))/segundos, p(0.50), p(0.99), erros)
}
