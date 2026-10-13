// HTTP load generator of the comparison (RELATORIO-COMPARATIVO-NATIVO.md):
// `connections` keep-alive clients request `url` in a loop for `seconds` and
// the result comes out as `req/s <n> p50_us <n> p99_us <n> erros <n>`. Replaces
// `wrk` where it is not installed (macOS without a package manager).
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
	connections, _ := strconv.Atoi(os.Args[2])
	seconds, _ := strconv.ParseFloat(os.Args[3], 64)
	transport := &http.Transport{MaxIdleConns: connections, MaxIdleConnsPerHost: connections, MaxConnsPerHost: connections, DisableCompression: true}
	cliente := &http.Client{Transport: transport}
	deadline := time.Now().Add(time.Duration(seconds * float64(time.Second)))
	var mu sync.Mutex
	var lat []time.Duration
	erros := 0
	var wg sync.WaitGroup
	for c := 0; c < connections; c++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			local := make([]time.Duration, 0, 1<<16)
			e := 0
			for time.Now().Before(deadline) {
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
	fmt.Printf("req/s %.0f p50_us %d p99_us %d erros %d\n", float64(len(lat))/seconds, p(0.50), p(0.99), erros)
}
