package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"log"
	"net"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"
)

var (
	port = flag.Int("port", 8080, "Port")
)

type Resp struct {
	Hostname      string `json:"hostname"`
	RemoteAddress string `json:"remote_address"`
}

func main() {

	flag.Parse()

	hostname, err := os.Hostname()
	if err != nil {
		panic(err)
	}

	listener, err := net.Listen("tcp", fmt.Sprintf(":%d", *port))
	if err != nil {
		panic(err)
	}

	handler := &Handler{
		hostname: hostname,
	}

	server := &http.Server{
		Addr:    fmt.Sprintf(":%d", port),
		Handler: handler,
	}

	go func() {
		log.Fatal(server.Serve(listener))
	}()

	log.Printf("Running on port %d.\n", *port)
	signals := make(chan os.Signal, 1)
	signal.Notify(signals, syscall.SIGTERM)
	sig := <-signals
	log.Printf("Shutting down after receiving signal: %s.\n", sig)

	time.Sleep(60 * time.Second)
}

type Handler struct {
	hostname string
}

func (h *Handler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	resp := Resp{
		Hostname:      h.hostname,
		RemoteAddress: r.RemoteAddr,
	}
	log.Printf("Hostname: %s RemoteAddress: %s\n", resp.Hostname, resp.RemoteAddress)

	header := w.Header()
	header.Set("Content-Type", "application/json")

	data, err := json.Marshal(resp)
	if err != nil {
		log.Fatal(err)
	}

	w.Write(data)

}
