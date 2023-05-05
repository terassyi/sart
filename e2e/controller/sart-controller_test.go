package controller

import (
	"fmt"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"

	// sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	v1 "k8s.io/api/core/v1"
	"k8s.io/apimachinery/pkg/util/json"
)

var _ = Describe("sart-controller", func() {
	BeforeEach(func() {
		fmt.Printf("START: %s\n", time.Now().Format(time.RFC3339))
	})
	AfterEach(func() {
		fmt.Printf("END: %s\n", time.Now().Format(time.RFC3339))
	})

	Context("sart-controller", testSartController)
})

func testSartController() {
	It("should apply resources correctly", func() {
		By("applying ClusterBGP resource")
		_, err := kubectl(nil, "apply", "-f", "manifests/cluster_bgp.yaml")
		Expect(err).NotTo(HaveOccurred())

		By("applying BGPPeer resources")
		_, err = kubectl(nil, "apply", "-f", "manifests/peer.yaml")
		Expect(err).NotTo(HaveOccurred())

		By("applying AddressPool for LB")
		_, err = kubectl(nil, "apply", "-f", "manifests/addresspool.yaml")
		Expect(err).NotTo(HaveOccurred())

		By("checking NodeBGP resources are created")
		nodesOut, err := kubectl(nil, "get", "nodes", "-o", "json")
		Expect(err).NotTo(HaveOccurred())
		nodeList := &v1.NodeList{}
		err = json.Unmarshal(nodesOut, nodeList)
		Expect(err).NotTo(HaveOccurred())

		// Eventually(func() error {
		// 	nodeBGPList := &sartv1alpha1.NodeBGPList{}
		// 	nodeBGPListOut, err := kubectl(nil, "get", "-n", "kube-system", "nodebgp")
		// 	if err != nil {
		// 		return err
		// 	}
		// 	if err := json.Unmarshal(nodeBGPListOut, nodeBGPList); err != nil {
		// 		return err
		// 	}
		// 	if len(nodeBGPList.Items) != len(nodeList.Items) {
		// 		return fmt.Errorf("want: %d, got: %d", len(nodeBGPList.Items))
		// 	}
		// 	return nil
		// }).Should(Succeed())

	})

	It("should handle LoadBalancer externalTrafficPolicy=Cluster", func() {

	})
}
