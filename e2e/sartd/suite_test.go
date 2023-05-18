package main

import (
	"fmt"
	"testing"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

func TestSartdBgp(t *testing.T) {

	RegisterFailHandler(Fail)
	RunSpecs(t, "SARTD TEST")
}

var _ = Describe("sartd", func() {

	BeforeEach(func() {
		fmt.Printf("START: %s\n", time.Now().Format(time.RFC3339))
	})
	AfterEach(func() {
		fmt.Printf("END: %s\n", time.Now().Format(time.RFC3339))
	})

	Context("Establish", testEstablish)
	Context("Update", testUpdate)
	Context("Fib", testFib)
})
