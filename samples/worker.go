package main

import (
	"context"
	"fmt"
	"sync"
)

// fanOut runs fn over jobs using n workers, returning when all jobs finish.
func fanOut[T any](ctx context.Context, jobs []T, n int, fn func(context.Context, T)) {
	ch := make(chan T)
	var wg sync.WaitGroup
	for i := 0; i < n; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for job := range ch {
				fn(ctx, job)
			}
		}()
	}
	for _, job := range jobs {
		ch <- job
	}
	close(ch)
	wg.Wait()
}

func main() {
	fanOut(context.Background(), []int{1, 2, 3}, 2, func(_ context.Context, n int) {
		fmt.Println("processed", n)
	})
}
