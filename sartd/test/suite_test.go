package test

import (
	"fmt"
	"testing"
	"time"

	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

func TestSartdBgp(t *testing.T) {

	RegisterFailHandler(Fail)
	RunSpecs(t, "SARTD-BGP test")
}

var _ = Describe("sartd-bgp", func() {

	BeforeEach(func() {
		fmt.Printf("START: %s\n", time.Now().Format(time.RFC3339))
	})
	AfterEach(func() {
		fmt.Printf("END: %s\n", time.Now().Format(time.RFC3339))
	})

	Context("establish", testEstablish)
	Context("update", testUpdate)
	Context("api", testApi)
	Context("integration fib api", testIntegrateFib)
})
