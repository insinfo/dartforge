// Comparativo entre linguagens (RELATORIO-COMPARATIVO-NATIVO.md): os mesmos
// quatro núcleos em cada uma; `scripts/comparar-linguagens.py` compila e mede.
package main

import (
	"fmt"
	"strconv"
	"strings"
	"time"
)

type No struct{ esq, dir *No }

func (n *No) conta() int {
	c := 1
	if n.esq != nil {
		c += n.esq.conta()
	}
	if n.dir != nil {
		c += n.dir.conta()
	}
	return c
}

func arvore(d int) *No {
	if d == 0 {
		return &No{}
	}
	return &No{arvore(d - 1), arvore(d - 1)}
}

func arvores(maxD int) int {
	longa := arvore(maxD)
	total := 0
	for d := 4; d <= maxD; d += 2 {
		it := 1 << (maxD - d + 4)
		for i := 0; i < it; i++ {
			total += arvore(d).conta()
		}
	}
	return total + longa.conta()
}

func fib(n int) int {
	if n < 2 {
		return n
	}
	return fib(n-1) + fib(n-2)
}

func numerico(n int) int {
	s := 0
	for i := 0; i < n; i++ {
		s = (s + i*i) % 1000003
	}
	return s
}

func textos(n int) int {
	m := map[string]int{}
	for i := 0; i < n; i++ {
		k := "k" + strconv.Itoa(i%5000)
		m[k] = m[k] + i
	}
	s := 0
	for _, v := range m {
		s += v % 1000
	}
	return s
}

func medir(nome string, f func() int) {
	r := 0
	t := []string{}
	for i := 0; i < 4; i++ {
		t0 := time.Now()
		r = f()
		t = append(t, strconv.FormatInt(time.Since(t0).Milliseconds(), 10))
	}
	fmt.Printf("%s: %s ms | %d\n", nome, strings.Join(t, " "), r)
}

func main() {
	medir("fib", func() int { return fib(32) })
	medir("arvores", func() int { return arvores(14) })
	medir("numerico", func() int { return numerico(30000000) })
	medir("textos", func() int { return textos(1000000) })
}
