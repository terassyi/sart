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
	SetDefaultEventuallyTimeout(5 * time.Minute)
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
		fmt.Println("target not set")
		os.Exit(1)
	}
})

func testBgp() {
	Context("frr", testEstablishPeerWithFrr)
	Context("gobgp", testEstablishPeerWithGoBGP)
	Context("ibgp", testEstablishPeerWithIBGP)
	Context("cicle", testInCircleTopology)
	Context("cicle-ibgp", testInCircleTopologyWithIBGP)
	Context("multipath", testInCircleTopologyWithMultiPath)
}

func testKubernetes() {
	Context("workloads", testControllerWorkloads)
	Context("bgp=a", testClusterBGPA)
	Context("bgp=b", testClusterBGPB)
	Context("create load-balancer", testCreatingLoadBalancer)
	Context("load-balancer connectivity", testLoadBalancerConnectivity)
	Context("address pool", testAddressPool)
	Context("externalTrafficPolicy", testExternalTrafficPolicy)
	Context("bgp change", testBGPLabelChange)
	Context("delete cluster bgp", testDeleteClusterBGP)
	Context("delete bgp peer", testDeleteBGPPeer)
	Context("bgp2=c", testClusterBGPC)
	Context("restart agent", testRestartAgent)
	Context("restart controller", testRestartController)
	Context("restart bgp", testRestartBGP)
	// Context("controller", testController)
}
