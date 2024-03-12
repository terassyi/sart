package e2e

import (
	"context"
	"encoding/json"
	"fmt"
	"net"
	"net/netip"
	"os"
	"path"
	"strings"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
	appsv1 "k8s.io/api/apps/v1"
	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
)

func isCompact() bool {
	compact := os.Getenv("COMPACT")
	return compact == "true"
}

func testClusterBGPForCNI() {
	// compact := isCompact()

	It("should install BGP related resource", func() {
		By("applying BGP for spine0")
		Eventually(func(g Gomega) error {
			_, err := kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR_CNI, "peer_template.yaml"))
			g.Expect(err).NotTo(HaveOccurred())

			_, err = kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR_CNI, "cluster_bgp_spine0.yaml"))
			g.Expect(err).NotTo(HaveOccurred())
			return nil
		}).Should(Succeed())

		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		ctx := context.Background()

		By("checking ClusterBGP bgp=a")
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(clusterBGP).List(ctx, metav1.ListOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(list.Items)).To(Equal(1))
			return nil
		}).Should(Succeed())

		By("checking NodeBGP with label(bgp=a)")
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(nodeBGP).List(ctx, metav1.ListOptions{LabelSelector: "bgp=a"})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(list.Items)).To(Equal(4))
			return nil
		}).Should(Succeed())

		By("checking BGPPeer for spine0")
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(bgpPeer).List(ctx, metav1.ListOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(list.Items)).To(Equal(4))
			return nil
		}).Should(Succeed())

		By("checking BGPPeer status is Established state for spine0")
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(bgpPeer).List(ctx, metav1.ListOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			for _, p := range list.Items {
				res, found, err := unstructured.NestedSlice(p.Object, "status", "conditions")
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(found).To(BeTrue())
				nowCond := res[len(res)-1].(map[string]any)
				nowStatus, ok := nowCond["status"]
				g.Expect(ok).To(BeTrue())
				g.Expect(nowStatus.(string)).To(Equal("Established"))
			}
			return nil
		}).WithTimeout(5 * time.Minute).Should(Succeed())

		By("checking sartd-bgp's peer state for spine0")
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(bgpPeer).List(ctx, metav1.ListOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			for _, p := range list.Items {
				node, found, err := unstructured.NestedString(p.Object, "spec", "nodeBGPRef")
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(found).To(BeTrue())
				peerAddr, found, err := unstructured.NestedString(p.Object, "spec", "addr")
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(found).To(BeTrue())
				out, err := kubectl(nil, "-n", "kube-system", "get", "pod", "-l", "app=sartd", "--field-selector", fmt.Sprintf("spec.nodeName=%s", node), "-ojson")
				g.Expect(err).NotTo(HaveOccurred())

				var podList corev1.PodList
				err = json.Unmarshal(out, &podList)
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(len(podList.Items)).To(Equal(1))

				pod := podList.Items[0]
				peerBytes, err := kubectlExec(pod.Name, pod.Namespace, nil, "sart", "bgp", "neighbor", "get", peerAddr, "-ojson")
				g.Expect(err).NotTo(HaveOccurred())
				var peer sartPeerInfo
				err = json.Unmarshal(peerBytes, &peer)
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(peer.State).To(Equal("Established"))
			}
			return nil
		}).Should(Succeed())

		By("applying BGP for spine1")
		Eventually(func(g Gomega) error {
			_, err := kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR_CNI, "cluster_bgp_spine1.yaml"))
			g.Expect(err).NotTo(HaveOccurred())
			return nil
		}).Should(Succeed())

		By("checking BGPPeer for spine1")
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(bgpPeer).List(ctx, metav1.ListOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(list.Items)).To(Equal(8)) // spine0 + spine1
			return nil
		}).Should(Succeed())

		By("checking BGPPeer status is Established state for spine1")
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(bgpPeer).List(ctx, metav1.ListOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			for _, p := range list.Items {
				res, found, err := unstructured.NestedSlice(p.Object, "status", "conditions")
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(found).To(BeTrue())
				nowCond := res[len(res)-1].(map[string]any)
				nowStatus, ok := nowCond["status"]
				g.Expect(ok).To(BeTrue())
				g.Expect(nowStatus.(string)).To(Equal("Established"))
			}
			return nil
		}).WithTimeout(5 * time.Minute).Should(Succeed())

		By("checking sartd-bgp's peer state for spine1")
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(bgpPeer).List(ctx, metav1.ListOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			for _, p := range list.Items {
				node, found, err := unstructured.NestedString(p.Object, "spec", "nodeBGPRef")
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(found).To(BeTrue())
				peerAddr, found, err := unstructured.NestedString(p.Object, "spec", "addr")
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(found).To(BeTrue())
				out, err := kubectl(nil, "-n", "kube-system", "get", "pod", "-l", "app=sartd", "--field-selector", fmt.Sprintf("spec.nodeName=%s", node), "-ojson")
				g.Expect(err).NotTo(HaveOccurred())

				var podList corev1.PodList
				err = json.Unmarshal(out, &podList)
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(len(podList.Items)).To(Equal(1))

				pod := podList.Items[0]
				peerBytes, err := kubectlExec(pod.Name, pod.Namespace, nil, "sart", "bgp", "neighbor", "get", peerAddr, "-ojson")
				g.Expect(err).NotTo(HaveOccurred())
				var peer sartPeerInfo
				err = json.Unmarshal(peerBytes, &peer)
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(peer.State).To(Equal("Established"))
			}
			return nil
		}).Should(Succeed())

	})
}

func testPodAddressPool() {
	It("should create AddressPools for pods", func() {
		By("applying AddressPools")
		_, err := kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR_CNI, "pool.yaml"))
		Expect(err).NotTo(HaveOccurred())

		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		ctx := context.Background()

		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(addressPool).List(ctx, metav1.ListOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(list.Items)).To(Equal(2))
			return nil
		}).Should(Succeed())
	})
}

func testCreatePods() {
	It("should create namespace", func() {
		By("applying Namespace")
		_, err := kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR_CNI, "namespace.yaml"))
		Expect(err).NotTo(HaveOccurred())
	})

	It("should create address block and related resource for pods", func() {
		By("applying Pods")
		_, err := kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR_CNI, "test_pod.yaml"))
		Expect(err).NotTo(HaveOccurred())

		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		ctx := context.Background()

		By("checking address blocks are created for each node")
		nodeCIDRMap := make(map[string]*net.IPNet)
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(addressBlock).List(ctx, metav1.ListOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(list.Items)).To(Equal(4))
			for _, block := range list.Items {
				poolName, found, err := unstructured.NestedString(block.Object, "spec", "poolRef")
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(found).To(BeTrue())
				g.Expect(poolName).To(Equal("default-pod-pool"))
				node, found, err := unstructured.NestedString(block.Object, "spec", "nodeRef")
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(found).To(BeTrue())
				cidrStr, found, err := unstructured.NestedString(block.Object, "spec", "cidr")
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(found).To(BeTrue())
				_, cidr, err := net.ParseCIDR(cidrStr)
				g.Expect(err).NotTo(HaveOccurred())
				nodeCIDRMap[node] = cidr
			}

			g.Expect(len(nodeCIDRMap)).To(Equal(4))
			return nil
		}).Should(Succeed())

		By("checking BGPAdvertisement is created for each block")
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(bgpAdvertisement).List(ctx, metav1.ListOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(list.Items)).To(Equal(4))
			for _, adv := range list.Items {
				nodeName := ""
				for node, cidr := range nodeCIDRMap {
					if strings.Contains(adv.GetName(), fmt.Sprintf("%s-", node)) {
						prefixStr, found, err := unstructured.NestedString(adv.Object, "spec", "cidr")
						g.Expect(err).NotTo(HaveOccurred())
						g.Expect(found).To(BeTrue())
						_, prefix, err := net.ParseCIDR(prefixStr)
						g.Expect(err).NotTo(HaveOccurred())
						g.Expect(cidr).To(Equal(prefix))

						nodeName = node
						break
					}
				}

				peers, found, err := unstructured.NestedMap(adv.Object, "status", "peers")
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(found).To(BeTrue())

				peerList, err := dynamicClient.Resource(bgpPeer).List(ctx, metav1.ListOptions{LabelSelector: fmt.Sprintf("bgppeer.sart.terassyi.net/node=%s", nodeName)})
				g.Expect(err).NotTo(HaveOccurred())

				for _, peer := range peerList.Items {
					peerStatus, ok := peers[peer.GetName()]
					if !ok {
						return fmt.Errorf("advertisement for peer: %s is not found: peers: %v", peer.GetName(), peers)
					}
					g.Expect(peerStatus).To(Equal("Advertised"))
					if peerStatus.(string) != "Advertised" {
						return fmt.Errorf("advertisement status: %s is not found: peers: %v", peerStatus.(string), peers)
					}
				}
			}
			return nil
		}).Should(Succeed())

		By("checking all pods are running")
		var podList corev1.PodList
		podAddrMap := make(map[string]net.IP)

		Eventually(func(g Gomega) error {
			list, err := kubectl(nil, "-n", "test", "get", "pod", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(list, &podList)
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(podList.Items)).To(Equal(5))

			for _, pod := range podList.Items {
				g.Expect(pod.Status.Phase).To(Equal(corev1.PodRunning))
				podAddr := net.ParseIP(pod.Status.PodIP)
				podAddrMap[pod.Name] = podAddr

				cidr, ok := nodeCIDRMap[pod.Spec.NodeName]
				g.Expect(ok).To(BeTrue())
				g.Expect(cidr.Contains(podAddr)).To(BeTrue())
			}
			return nil
		}).Should(Succeed())

	})

	It("should exist an address block on each nodes", func() {
		By("checking all pods are running")
		var podList corev1.PodList
		podAddrMap := make(map[string]net.IP)

		Eventually(func(g Gomega) error {
			list, err := kubectl(nil, "-n", "test", "get", "pod", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(list, &podList)
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(podList.Items)).To(Equal(5))

			for _, pod := range podList.Items {
				g.Expect(pod.Status.Phase).To(Equal(corev1.PodRunning))
				podAddr := net.ParseIP(pod.Status.PodIP)
				podAddrMap[pod.Name] = podAddr
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity")
		Eventually(func(g Gomega) error {
			for src, srcAddr := range podAddrMap {
				for dst, dstAddr := range podAddrMap {
					for i := 0; i < 10; i++ {
						_, err := kubectlExec(src, "test", nil, "ping", "-c", "1", dstAddr.String())
						if err != nil {
							return fmt.Errorf("src: %s(%s) dst: %s(%s): %v", src, srcAddr, dst, dstAddr, err)
						}
					}
				}
			}
			return nil
		}).Should(Succeed())
	})
}

func testDeletePod() {
	It("should delete a pod on sart-worker3", func() {
		By("get pods on sart-worker3")
		var podList corev1.PodList
		out, err := kubectl(nil, "-n", "test", "get", "pod", "--field-selector=spec.nodeName=sart-worker3", "-ojson")
		Expect(err).NotTo(HaveOccurred())
		err = json.Unmarshal(out, &podList)
		Expect(err).NotTo(HaveOccurred())
		Expect(len(podList.Items)).To(Equal(2))

		By("finding the target pod to delete")
		var deleteTarget corev1.Pod

		pod0Addr, err := netip.ParseAddr(podList.Items[0].Status.PodIP)
		Expect(err).NotTo(HaveOccurred())
		pod1Addr, err := netip.ParseAddr(podList.Items[1].Status.PodIP)
		Expect(err).NotTo(HaveOccurred())

		if pod0Addr.Compare(pod1Addr) > 0 {
			// pod0Addr > pod1Addr
			deleteTarget = podList.Items[1]
		} else {
			deleteTarget = podList.Items[0]
		}

		By("deleting the pod that has the lower address")
		_, err = kubectl(nil, "-n", deleteTarget.Namespace, "delete", "pod", deleteTarget.Name)
		Expect(err).NotTo(HaveOccurred())

		By("checking the target pod is deleted")
		Eventually(func(g Gomega) error {
			_, err := kubectl(nil, "-n", deleteTarget.Namespace, "get", "pod", deleteTarget.Name, "-ojson")
			if err != nil {
				// should be not found
				return nil
			}
			return fmt.Errorf("deleted pod(%s) should not exist", deleteTarget.Name)

		}).Should(Succeed())

		By("creating the pod on sart-worker3 again")
		_, err = kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR_CNI, "test_pod.yaml"))
		Expect(err).NotTo(HaveOccurred())

		By("checking the recreated pod has same address")
		// When using a bit allocator pool, allocator always picks a lowest address in the pool
		// So in this case, the recreated pod should be given the same address.
		Eventually(func(g Gomega) error {
			var recreatedPod corev1.Pod
			out, err := kubectl(nil, "-n", deleteTarget.Namespace, "get", "pod", deleteTarget.Name, "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &recreatedPod)
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(deleteTarget.Status.PodIP).To(Equal(recreatedPod.Status.PodIP))
			return nil
		}).Should(Succeed())

	})
}

func testNonDefaultPool() {
	It("should create pod with non-default-pod-pool", func() {
		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		ctx := context.Background()

		By("getting the non-default AddressPool")
		Eventually(func(g Gomega) error {
			_, err := dynamicClient.Resource(addressPool).Get(ctx, "non-default-pod-pool", metav1.GetOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			return nil
		}).Should(Succeed())

		By("applying a pod with non-default-pod-pool")
		_, err = kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR_CNI, "test_pod_another_pool.yaml"))
		Expect(err).NotTo(HaveOccurred())

		By("checking the AddressBlock for non-default-pod-pool")
		var cidr net.IPNet
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(addressBlock).List(ctx, metav1.ListOptions{LabelSelector: "sart.terassyi.net/addresspool=non-default-pod-pool"})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(list.Items)).To(Equal(1))

			poolRef, found, err := unstructured.NestedString(list.Items[0].Object, "spec", "poolRef")
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(found).To(BeTrue())
			g.Expect(poolRef).To(Equal("non-default-pod-pool"))

			cidrStr, found, err := unstructured.NestedString(list.Items[0].Object, "spec", "cidr")
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(found).To(BeTrue())

			_, actualCIDR, err := net.ParseCIDR(cidrStr)
			g.Expect(err).NotTo(HaveOccurred())
			cidr = *actualCIDR
			return nil
		}).Should(Succeed())

		By("checking the pod is running")
		var pod corev1.Pod
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test", "get", "pod", "test-worker-non-default", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &pod)
			g.Expect(err).NotTo(HaveOccurred())

			g.Expect(pod.Status.Phase).To(Equal(corev1.PodRunning))
			return nil
		}).Should(Succeed())

		By("checking the allocated address")
		addr := net.ParseIP(pod.Status.PodIP)
		Expect(cidr.Contains(addr)).To(BeTrue())

		By("checking the connectivity")
		var dstPod corev1.Pod
		out, err := kubectl(nil, "-n", "test", "get", "pod", "test-cp", "-ojson")
		Expect(err).NotTo(HaveOccurred())
		err = json.Unmarshal(out, &dstPod)
		Expect(err).NotTo(HaveOccurred())

		Eventually(func(g Gomega) error {
			for i := 0; i < 10; i++ {
				_, err := kubectlExec("test-worker-non-default", "test", nil, "ping", "-c", "1", dstPod.Status.PodIP)
				g.Expect(err).NotTo(HaveOccurred())
			}
			return nil
		}).Should(Succeed())
	})
}

func testNonDefaultPoolInNamespace() {
	It("should create pods in test-non-default namespace", func() {
		By("applying a namespace and pods")
		_, err := kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR_CNI, "test_pod_in_namespace.yaml"))
		Expect(err).NotTo(HaveOccurred())

		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		ctx := context.Background()

		By("checking the AddressBlock for non-default-pod-pool")
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(addressBlock).List(ctx, metav1.ListOptions{LabelSelector: "sart.terassyi.net/addresspool=non-default-pod-pool"})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(list.Items)).To(Equal(3))
			return nil
		}).Should(Succeed())

		By("checking all pods are running")
		var podList corev1.PodList
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test-non-default", "get", "pod", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &podList)
			g.Expect(err).NotTo(HaveOccurred())

			for _, pod := range podList.Items {
				g.Expect(pod.Status.Phase).To(Equal(corev1.PodRunning))
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity")
		Eventually(func(g Gomega) error {
			for _, src := range podList.Items {
				for _, dst := range podList.Items {
					for i := 0; i < 10; i++ {
						_, err := kubectlExec(src.Name, src.Namespace, nil, "ping", "-c", "1", dst.Status.PodIP)
						g.Expect(err).NotTo(HaveOccurred())
					}

				}
			}
			return nil
		}).Should(Succeed())

	})
}

func testReleaseAddressBlock() {
	It("should release and re-create AddressBlocks", func() {
		By("getting a pod in test-non-default on sart-worker2")
		var pod corev1.Pod
		out, err := kubectl(nil, "-n", "test-non-default", "get", "pod", "test-worker2-non-default", "-ojson")
		Expect(err).NotTo(HaveOccurred())
		err = json.Unmarshal(out, &pod)
		Expect(err).NotTo(HaveOccurred())

		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		ctx := context.Background()

		By("getting an AddressBlock associated with sart-worker2")
		// var workerAddressBlock unstructured.Unstructured
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(addressBlock).List(ctx, metav1.ListOptions{LabelSelector: "addressblock.sart.terassyi.net/node=sart-worker2,sart.terassyi.net/addresspool=non-default-pod-pool"})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(list.Items)).To(Equal(1))
			return nil
		}).Should(Succeed())

		By("deleting the pod in test-non-default namespace on sart-worker2")
		_, err = kubectl(nil, "-n", pod.Namespace, "delete", "pod", pod.Name)
		Expect(err).NotTo(HaveOccurred())

		By("checking the deleted pod doesn't exist")
		Eventually(func(g Gomega) error {
			_, err := kubectl(nil, "-n", pod.Namespace, "get", "pod", pod.Name, "-ojson")
			if err != nil {
				// should be not found
				return nil
			}
			return fmt.Errorf("deleted pod(%s) should not exist", pod.Name)
		}).Should(Succeed())

		By("checking the AddressBlock that is associated with sart-worker is released")
		// Once the AddressBlock has been unused, it must be released and removed
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(addressBlock).List(ctx, metav1.ListOptions{LabelSelector: "addressblock.sart.terassyi.net/node=sart-worker2,sart.terassyi.net/addresspool=non-default-pod-pool"})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(list.Items)).NotTo(Equal(1))
			return nil
		}).Should(Succeed())
	})
}

func testRecoverAllocationsAfterRestart() {
	It("should recover existing allocations on the node after restarting sartd-agent", func() {
		By("getting existing pods on sart-worker3")
		var podList corev1.PodList
		out, err := kubectl(nil, "-n", "test", "get", "pod", "-ojson", "--field-selector=spec.nodeName=sarto-worker3")
		Expect(err).NotTo(HaveOccurred())
		err = json.Unmarshal(out, &podList)
		Expect(err).NotTo(HaveOccurred())
		podAddrMap := make(map[string]corev1.Pod)
		for _, pod := range podList.Items {
			podAddrMap[pod.Status.PodIP] = pod
		}

		By("getting existing pods on sart-worker")
		var podListOnWorker corev1.PodList
		out, err = kubectl(nil, "-n", "test", "get", "pod", "-ojson", "--field-selector=spec.nodeName=sarto-worker")
		Expect(err).NotTo(HaveOccurred())
		err = json.Unmarshal(out, &podListOnWorker)
		Expect(err).NotTo(HaveOccurred())

		By("restarting sartd-agent pods")
		_, err = kubectl(nil, "-n", "kube-system", "rollout", "restart", "ds/sartd-agent")
		Expect(err).NotTo(HaveOccurred())

		By("waiting for rollout restart")
		_, err = kubectl(nil, "-n", "kube-system", "rollout", "status", "ds/sartd-agent")
		Expect(err).NotTo(HaveOccurred())

		By("applying new pod on sart-worker3")
		_, err = kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR_CNI, "test_pod2.yaml"))
		Expect(err).NotTo(HaveOccurred())

		By("checking all pods are running")
		var newPod corev1.Pod
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test", "get", "pod", "test-worker3-3", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &newPod)
			g.Expect(err).NotTo(HaveOccurred())

			g.Expect(newPod.Status.Phase).To(Equal(corev1.PodRunning))
			return nil
		}).Should(Succeed())

		By("checking new pod is allocated new address")
		_, found := podAddrMap[newPod.Status.PodIP]
		Expect(found).To(BeFalse())

		By("checking the connectivity")
		Eventually(func(g Gomega) error {
			for _, dst := range podListOnWorker.Items {
				for i := 0; i < 10; i++ {
					_, err := kubectlExec(newPod.Name, newPod.Namespace, nil, "ping", "-c", "1", dst.Status.PodIP)
					g.Expect(err).NotTo(HaveOccurred())
				}
			}
			return nil
		}).Should(Succeed())

		By("restarting sartd-bgp pods")
		_, err = kubectl(nil, "-n", "kube-system", "rollout", "restart", "ds/sartd-agent")
		Expect(err).NotTo(HaveOccurred())

		By("waiting for rollout restart")
		_, err = kubectl(nil, "-n", "kube-system", "rollout", "status", "ds/sartd-bgp")
		Expect(err).NotTo(HaveOccurred())

		By("checking the connectivity")
		Eventually(func(g Gomega) error {
			for _, dst := range podListOnWorker.Items {
				for i := 0; i < 10; i++ {
					_, err := kubectlExec(newPod.Name, newPod.Namespace, nil, "ping", "-c", "1", dst.Status.PodIP)
					g.Expect(err).NotTo(HaveOccurred())
				}
			}
			return nil
		}).Should(Succeed())

		By("restarting sart-controller pods")
		_, err = kubectl(nil, "-n", "kube-system", "rollout", "restart", "deploy/sart-controller")
		Expect(err).NotTo(HaveOccurred())

		By("waiting for rollout restart")
		_, err = kubectl(nil, "-n", "kube-system", "rollout", "status", "deploy/sart-controller")
		Expect(err).NotTo(HaveOccurred())

		By("checking the connectivity")
		Eventually(func(g Gomega) error {
			for _, dst := range podListOnWorker.Items {
				for i := 0; i < 10; i++ {
					_, err := kubectlExec(newPod.Name, newPod.Namespace, nil, "ping", "-c", "1", dst.Status.PodIP)
					g.Expect(err).NotTo(HaveOccurred())
				}
			}
			return nil
		}).Should(Succeed())
	})

}

func testSwitchModeToDual() {
	It("should change the mode from cni to dual", func() {
		By("patching sartd-agent")
		_, err := kubectl(nil, "patch", "-n", "kube-system", "daemonset", "sartd-agent", "--type=strategic", "--patch-file", path.Join(MANIFEST_DIR_DUAL, "agent-patch.yaml"))
		Expect(err).NotTo(HaveOccurred())
		By("waiting for rollout restart")
		_, err = kubectl(nil, "-n", "kube-system", "rollout", "status", "ds/sartd-agent")
		Expect(err).NotTo(HaveOccurred())

		By("patching sart-controller")
		_, err = kubectl(nil, "patch", "-n", "kube-system", "deployment", "sart-controller", "--type=strategic", "--patch-file", path.Join(MANIFEST_DIR_DUAL, "controller-patch.yaml"))
		Expect(err).NotTo(HaveOccurred())
		By("waiting for rollout restart")
		_, err = kubectl(nil, "-n", "kube-system", "rollout", "status", "deploy/sart-controller")
		Expect(err).NotTo(HaveOccurred())

		By("checking sart-controller working")
		var dp appsv1.Deployment
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "kube-system", "get", "deploy", "sart-controller", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &dp)
			g.Expect(err).NotTo(HaveOccurred())
			if dp.Status.Replicas != dp.Status.AvailableReplicas {
				return fmt.Errorf("sart-controller is not ready")
			}
			return nil
		}).Should(Succeed())
	})
}

func testLBConnectivityWithDualMode() {
	It("should communicate via app-svc-cluster", func() {
		By("checking app-svc-cluster gets addresses")
		var svc corev1.Service
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-cluster", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &svc)
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(svc.Status.LoadBalancer.Ingress)).To(Equal(1))
			return nil
		}).Should(Succeed())

		addr := svc.Status.LoadBalancer.Ingress[0].IP

		By("checking BGPAdvertisement")
		ctx := context.Background()
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())
		Eventually(func(g Gomega) error {
			advList, err := dynamicClient.Resource(bgpAdvertisement).Namespace(svc.Namespace).List(ctx, metav1.ListOptions{LabelSelector: fmt.Sprintf("kubernetes.io/service-name=%s", svc.Name)})
			g.Expect(err).NotTo(HaveOccurred())
			peers, found, err := unstructured.NestedMap(advList.Items[0].Object, "status", "peers")
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(found).To(BeTrue())
			g.Expect(len(peers)).To(Equal(8))
			for _, status := range peers {
				g.Expect(status.(string)).To(Equal("Advertised"))
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity to app-svc-cluster")
		Eventually(func(g Gomega) error {
			_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr)
			g.Expect(err).NotTo(HaveOccurred())
			return nil
		}).Should(Succeed())

	})

}
