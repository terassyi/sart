package e2e

import (
	"fmt"
	"os"
	"testing"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

func TestSart(t *testing.T) {
	RegisterFailHandler(Fail)
	RunSpecs(t, "Sart e2e test")
}

var _ = BeforeSuite(func() {
	fmt.Println("Preparing...")

	SetDefaultEventuallyPollingInterval(time.Second)
	SetDefaultEventuallyTimeout(1 * time.Minute)
})

var _ = Describe("End to End test for Sart", func() {
	BeforeEach(func() {
		fmt.Printf("START: %s\n", time.Now().Format(time.RFC3339))
	})
	AfterEach(func() {
		fmt.Printf("END: %s\n", time.Now().Format(time.RFC3339))
	})

	testTarget := os.Getenv("TARGET")

	switch testTarget {
	case "bgp":
		testBgp()
	case "kubernetes":
		testKubernetes()
	default:
		testBgp()
		testKubernetes()
	}
})

func testBgp() {
	Context("frr", testEstablishPeerWithFrr)
	Context("gobgp", testEstablishPeerWithGoBGP)
	Context("ibgp", testEstablishPeerWithIBGP)
	Context("cicle", testInCircleTopology)
	Context("cicle-ibgp", testInCircleTopologyWithIBGP)
}

func testKubernetes() {}
