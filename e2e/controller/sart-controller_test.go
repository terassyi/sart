package controller

import (
	"context"
	"fmt"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"

	sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	"github.com/terassyi/sart/e2e/container"
	"github.com/terassyi/sart/e2e/sartd"
	v1 "k8s.io/api/core/v1"
	discoveryv1 "k8s.io/api/discovery/v1"
	"k8s.io/apimachinery/pkg/util/json"
)

const (
	KubernetesServiceNameLabel string = "kubernetes.io/service-name"
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

		Eventually(func() error {
			nodeBGPList := &sartv1alpha1.NodeBGPList{}
			nodeBGPListOut, err := kubectl(nil, "get", "-n", "kube-system", "nodebgp", "-o", "json")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(nodeBGPListOut, nodeBGPList); err != nil {
				return err
			}
			if len(nodeBGPList.Items) != len(nodeList.Items) {
				return fmt.Errorf("want: %d, got: %d", len(nodeList.Items), len(nodeBGPList.Items))
			}
			return nil
		}).Should(Succeed())

		By("establishing BGPPeer")
		Eventually(func() error {
			peerList := &sartv1alpha1.BGPPeerList{}
			peerListOut, err := kubectl(nil, "get", "-n", "kube-system", "bgppeer", "-o", "json")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(peerListOut, peerList); err != nil {
				return err
			}
			for _, p := range peerList.Items {
				if p.Status != sartv1alpha1.BGPPeerStatusEstablished {
					return fmt.Errorf("%s: expect: Established, got: %s", p.Name, p.Status)
				}
			}
			return nil
		}).Should(Succeed())
	})

	It("should handle LoadBalancer externalTrafficPolicy=Cluster", func() {
		expectedAddr := "10.69.0.0"
		expectedPrefix := "10.69.0.0/32"

		ctx := context.Background()

		By("applying Deployment and Service")
		_, err := kubectl(nil, "apply", "-f", "manifests/lb_cluster.yaml")
		Expect(err).NotTo(HaveOccurred())

		By("getting Service")
		svc := &v1.Service{}
		Eventually(func() error {
			svcOut, err := kubectl(nil, "get", "-n", "test", "svc", "app-svc-cluster", "-o", "json")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(svcOut, svc); err != nil {
				return err
			}
			return nil
		}).Should(Succeed())

		By("existing BGPAdvertisement for LB")
		Eventually(func() error {

			adv := &sartv1alpha1.BGPAdvertisement{}
			advOut, err := kubectl(nil, "get", "bgpadvertisement", "-n", "kube-system", fmt.Sprintf("%s-%s-ipv4", svc.Namespace, svc.Name), "-o", "json")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(advOut, adv); err != nil {
				return err
			}
			if adv.Status.Condition != sartv1alpha1.BGPAdvertisementConditionAvailable {
				return fmt.Errorf("advertisement status is %s", adv.Status.Condition)
			}
			return nil
		}).Should(Succeed())

		By("receiving a path information by external bgp speaker")
		res, err := container.RunCommandInContainerWithOutput(ctx, "external-bgp", []string{"gobgp", "global", "rib", "-j"})
		Expect(err).NotTo(HaveOccurred())
		pathMap := make(map[string][]sartd.GoBGPPath)
		err = json.Unmarshal(res, &pathMap)
		Expect(err).NotTo(HaveOccurred())
		paths, ok := pathMap[expectedPrefix]
		Expect(ok).To(BeTrue())
		Expect(len(paths)).To(Equal(4))

		By("assigning LB address to Service")
		svcOut, err := kubectl(nil, "get", "-n", svc.Namespace, "svc", svc.Name, "-o", "json")
		Expect(err).NotTo(HaveOccurred())
		svc = &v1.Service{}
		err = json.Unmarshal(svcOut, svc)
		Expect(err).NotTo(HaveOccurred())
		Expect(len(svc.Status.LoadBalancer.Ingress)).To(Equal(1))
		Expect(svc.Status.LoadBalancer.Ingress[0].IP).To(Equal(expectedAddr))

		By("communicating with the app")
		err = container.RunCommandInContainer(ctx, "external-client", false, []string{"curl", fmt.Sprintf("http://%s", expectedAddr)})
		Expect(err).NotTo(HaveOccurred())
	})

	It("should handle LoadBalancer externalTrafficPolicy=Local", func() {
		expectedAddr := "10.69.0.1"
		expectedPrefix := "10.69.0.1/32"

		ctx := context.Background()

		By("applying Deployment and Service")
		_, err := kubectl(nil, "apply", "-f", "manifests/lb_local.yaml")
		Expect(err).NotTo(HaveOccurred())

		By("getting Service")
		svc := &v1.Service{}
		Eventually(func() error {
			svcOut, err := kubectl(nil, "get", "-n", "test", "svc", "app-svc-local", "-o", "json")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(svcOut, svc); err != nil {
				return err
			}
			return nil
		}).Should(Succeed())

		By("existing BGPAdvertisement for LB")
		Eventually(func() error {

			adv := &sartv1alpha1.BGPAdvertisement{}
			advOut, err := kubectl(nil, "get", "bgpadvertisement", "-n", "kube-system", fmt.Sprintf("%s-%s-ipv4", svc.Namespace, svc.Name), "-o", "json")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(advOut, adv); err != nil {
				return err
			}
			if adv.Status.Condition != sartv1alpha1.BGPAdvertisementConditionAvailable {
				return fmt.Errorf("advertisement status is %s", adv.Status.Condition)
			}
			return nil
		}).Should(Succeed())

		By("receiving path information by external bgp speaker")
		Eventually(func() error {
			res, err := container.RunCommandInContainerWithOutput(ctx, "external-bgp", []string{"gobgp", "global", "rib", "-j"})
			if err != nil {
				return err
			}
			pathMap := make(map[string][]sartd.GoBGPPath)
			err = json.Unmarshal(res, &pathMap)
			if err != nil {
				return err
			}
			paths, ok := pathMap[expectedPrefix]
			if !ok {
				return fmt.Errorf("path not found")
			}
			if len(paths) != 2 {
				return fmt.Errorf("want: 2, got: %d", len(paths))
			}
			return nil
		}).Should(Succeed())

		By("assigning LB address to Service")
		svcOut, err := kubectl(nil, "get", "-n", svc.Namespace, "svc", svc.Name, "-o", "json")
		Expect(err).NotTo(HaveOccurred())
		svc = &v1.Service{}
		err = json.Unmarshal(svcOut, svc)
		Expect(err).NotTo(HaveOccurred())
		Expect(len(svc.Status.LoadBalancer.Ingress)).To(Equal(1))
		Expect(svc.Status.LoadBalancer.Ingress[0].IP).To(Equal(expectedAddr))

		By("communicating with the app")
		err = container.RunCommandInContainer(ctx, "external-client", false, []string{"curl", fmt.Sprintf("http://%s", expectedAddr)})
		Expect(err).NotTo(HaveOccurred())
	})

	It("should release LB address", func() {

		expectedAddr := "10.69.0.1"
		expectedPrefix := "10.69.0.1/32"

		ctx := context.Background()

		By("getting the service")
		svc := &v1.Service{}
		svcOut, err := kubectl(nil, "get", "-n", "test", "svc", "app-svc-local", "-o", "json")
		Expect(err).NotTo(HaveOccurred())
		err = json.Unmarshal(svcOut, svc)
		Expect(err).NotTo(HaveOccurred())

		By("getting the advertisement")
		adv := &sartv1alpha1.BGPAdvertisement{}
		advOut, err := kubectl(nil, "get", "-n", "kube-system", "bgpadvertisement", fmt.Sprintf("%s-%s-ipv4", svc.Namespace, svc.Name), "-o", "json")
		Expect(err).NotTo(HaveOccurred())
		err = json.Unmarshal(advOut, adv)
		Expect(err).NotTo(HaveOccurred())

		By("deleting the service")
		_, err = kubectl(nil, "delete", "-n", "test", "svc", "app-svc-local")
		Expect(err).NotTo(HaveOccurred())

		By("not getting the advertisement")
		adv = &sartv1alpha1.BGPAdvertisement{}
		_, err = kubectl(nil, "get", "-n", "kube-system", "bgpadvertisement", fmt.Sprintf("%s-%s-ipv4", svc.Namespace, svc.Name), "-o", "json")
		Expect(err).To(HaveOccurred())

		By("not getting the path from external BGP speaker")
		Eventually(func() error {
			res, err := container.RunCommandInContainerWithOutput(ctx, "external-bgp", []string{"gobgp", "global", "rib", "-j"})
			if err != nil {
				return err
			}
			pathMap := make(map[string][]sartd.GoBGPPath)
			if err := json.Unmarshal(res, &pathMap); err != nil {
				return err
			}
			_, ok := pathMap[expectedPrefix]
			if !ok {
				return nil
			}
			return fmt.Errorf("should not be able to get the path")
		}).Should(Succeed())

		By("not communicating with the app")
		err = container.RunCommandInContainer(ctx, "external-client", false, []string{"curl", "-m", "2", fmt.Sprintf("http://%s", expectedAddr)})
		Expect(err).To(HaveOccurred())
	})

	It("should re-use LB address", func() {

		expectedAddr := "10.69.0.1"
		expectedPrefix := "10.69.0.1/32"

		ctx := context.Background()

		By("applying Deployment and Service")
		_, err := kubectl(nil, "apply", "-f", "manifests/lb_local.yaml")
		Expect(err).NotTo(HaveOccurred())

		By("getting Service")
		svc := &v1.Service{}
		Eventually(func() error {
			svcOut, err := kubectl(nil, "get", "-n", "test", "svc", "app-svc-local", "-o", "json")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(svcOut, svc); err != nil {
				return err
			}
			return nil
		}).Should(Succeed())

		By("existing BGPAdvertisement for LB")
		Eventually(func() error {

			adv := &sartv1alpha1.BGPAdvertisement{}
			advOut, err := kubectl(nil, "get", "bgpadvertisement", "-n", "kube-system", fmt.Sprintf("%s-%s-ipv4", svc.Namespace, svc.Name), "-o", "json")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(advOut, adv); err != nil {
				return err
			}
			if adv.Status.Condition != sartv1alpha1.BGPAdvertisementConditionAvailable {
				return fmt.Errorf("advertisement status is %s", adv.Status.Condition)
			}
			return nil
		}).Should(Succeed())

		By("receiving a path information by external bgp speaker")
		res, err := container.RunCommandInContainerWithOutput(ctx, "external-bgp", []string{"gobgp", "global", "rib", "-j"})
		Expect(err).NotTo(HaveOccurred())
		pathMap := make(map[string][]sartd.GoBGPPath)
		err = json.Unmarshal(res, &pathMap)
		Expect(err).NotTo(HaveOccurred())
		paths, ok := pathMap[expectedPrefix]
		Expect(ok).To(BeTrue())
		Expect(len(paths)).To(Equal(2))

		By("assigning LB address to Service")
		svcOut, err := kubectl(nil, "get", "-n", svc.Namespace, "svc", svc.Name, "-o", "json")
		Expect(err).NotTo(HaveOccurred())
		svc = &v1.Service{}
		err = json.Unmarshal(svcOut, svc)
		Expect(err).NotTo(HaveOccurred())
		Expect(len(svc.Status.LoadBalancer.Ingress)).To(Equal(1))
		Expect(svc.Status.LoadBalancer.Ingress[0].IP).To(Equal(expectedAddr))

		By("communicating with the app")
		err = container.RunCommandInContainer(ctx, "external-client", false, []string{"curl", "-m", "2", fmt.Sprintf("http://%s", expectedAddr)})
		Expect(err).NotTo(HaveOccurred())
	})

	It("should update endpoints", func() {

		expectedAddr := "10.69.0.1"
		expectedPrefix := "10.69.0.1/32"

		ctx := context.Background()

		By("getting Service")
		svc := &v1.Service{}
		svcOut, err := kubectl(nil, "get", "-n", "test", "svc", "app-svc-local", "-o", "json")
		Expect(err).NotTo(HaveOccurred())
		err = json.Unmarshal(svcOut, svc)
		Expect(err).NotTo(HaveOccurred())

		By("getting Endpoints")
		epsList := &discoveryv1.EndpointSliceList{}
		epsListOut, err := kubectl(nil, "get", "endpointslice", "-n", "test", "-o", "json")
		Expect(err).NotTo(HaveOccurred())
		err = json.Unmarshal(epsListOut, epsList)
		Expect(err).NotTo(HaveOccurred())
		var eps discoveryv1.EndpointSlice
		for _, e := range epsList.Items {
			if name, ok := e.Labels[KubernetesServiceNameLabel]; ok && name == svc.Name {
				eps = e
				break
			}
		}
		Expect(eps.Name).NotTo(BeEmpty())

		nodes := []string{}
		addrMap := make(map[string]struct{})
		for _, ep := range eps.Endpoints {
			nodes = append(nodes, *ep.NodeName)
		}

		nodeList := &v1.NodeList{}
		nodeListOut, err := kubectl(nil, "get", "nodes", "-o", "json")
		Expect(err).NotTo(HaveOccurred())
		err = json.Unmarshal(nodeListOut, nodeList)
		Expect(err).NotTo(HaveOccurred())
		for _, node := range nodeList.Items {
			for _, n := range nodes {
				if n == node.Name {
					for _, a := range node.Status.Addresses {
						if a.Type == v1.NodeInternalIP {
							addrMap[a.Address] = struct{}{}
						}
					}
				}
			}
		}

		By("getting paths from external BGP speaker")
		res, err := container.RunCommandInContainerWithOutput(ctx, "external-bgp", []string{"gobgp", "global", "rib", "-j"})
		Expect(err).NotTo(HaveOccurred())
		pathMap := make(map[string][]sartd.GoBGPPath)
		err = json.Unmarshal(res, &pathMap)
		Expect(err).NotTo(HaveOccurred())
		paths, ok := pathMap[expectedPrefix]
		Expect(ok).To(BeTrue())
		Expect(len(paths)).To(Equal(2))

		By("comparing endpoints and paths")
		Expect(len(addrMap)).To(Equal(len(paths)))
		for _, p := range paths {
			fmt.Println(addrMap)
			fmt.Println(paths)
			_, ok := addrMap[p.Source]
			Expect(ok).To(BeTrue())
		}

		By("executing rollout restart the Deployment")
		_, err = kubectl(nil, "-n", "test", "rollout", "restart", "deployment/app-local")
		Expect(err).NotTo(HaveOccurred())

		By("waiting rollout restart")
		_, err = kubectl(nil, "-n", "test", "wait", "--timeout=3m", "--for=condition=available", "deployment/app-local")
		Expect(err).NotTo(HaveOccurred())

		By("getting updated Endpoints")
		updatedEpsList := &discoveryv1.EndpointSliceList{}
		updatedEpsListOut, err := kubectl(nil, "get", "endpointslice", "-n", "test", "-o", "json")
		Expect(err).NotTo(HaveOccurred())
		err = json.Unmarshal(updatedEpsListOut, updatedEpsList)
		Expect(err).NotTo(HaveOccurred())
		var updatedEps discoveryv1.EndpointSlice
		for _, e := range updatedEpsList.Items {
			if name, ok := e.Labels[KubernetesServiceNameLabel]; ok && name == svc.Name {
				updatedEps = e
				break
			}
		}
		Expect(updatedEps.Name).NotTo(BeEmpty())

		By("getting updated paths")
		updatedRes, err := container.RunCommandInContainerWithOutput(ctx, "external-bgp", []string{"gobgp", "global", "rib", "-j"})
		Expect(err).NotTo(HaveOccurred())
		updatedPathMap := make(map[string][]sartd.GoBGPPath)
		err = json.Unmarshal(updatedRes, &updatedPathMap)
		Expect(err).NotTo(HaveOccurred())
		updatedPaths, ok := updatedPathMap[expectedPrefix]
		Expect(ok).To(BeTrue())
		Expect(len(updatedPaths)).To(Equal(2))

		updatedAddrMap := make(map[string]struct{})
		updatedNodes := []string{}
		for _, ep := range updatedEps.Endpoints {
			updatedNodes = append(updatedNodes, *ep.NodeName)
		}
		for _, node := range nodeList.Items {
			for _, n := range updatedNodes {
				if n == node.Name {
					for _, a := range node.Status.Addresses {
						if a.Type == v1.NodeInternalIP {
							updatedAddrMap[a.Address] = struct{}{}
						}
					}
				}
			}
		}

		By("comparing updated endpoints and paths")
		Expect(len(updatedAddrMap)).To(Equal(len(updatedPaths)))
		for _, p := range updatedPaths {
			_, ok := updatedAddrMap[p.Source]
			Expect(ok).To(BeTrue())
		}

		By("communicating with the app")
		err = container.RunCommandInContainer(ctx, "external-client", false, []string{"curl", fmt.Sprintf("http://%s", expectedAddr)})
		Expect(err).NotTo(HaveOccurred())
	})
}
