package e2e

import (
	"context"
	"encoding/json"
	"fmt"
	"net"
	"os"
	"path"
	"path/filepath"
	"strings"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
	appsv1 "k8s.io/api/apps/v1"
	corev1 "k8s.io/api/core/v1"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"k8s.io/apimachinery/pkg/runtime/schema"
)

const (
	MANIFEST_DIR      string = "../manifests/lb/sample"
	MANIFEST_DIR_CNI  string = "../manifests/cni/sample"
	MANIFEST_DIR_DUAL string = "../manifests/dual"
	GROUP             string = "sart.terassyi.net"
	VERSION           string = "v1alpha2"
)

var (
	clusterBGP schema.GroupVersionResource = schema.GroupVersionResource{
		Group:    GROUP,
		Version:  VERSION,
		Resource: "clusterbgps",
	}
	nodeBGP schema.GroupVersionResource = schema.GroupVersionResource{
		Group:    GROUP,
		Version:  VERSION,
		Resource: "nodebgps",
	}
	peerTemplate schema.GroupVersionResource = schema.GroupVersionResource{
		Group:    GROUP,
		Version:  VERSION,
		Resource: "peertemplates",
	}
	bgpPeer schema.GroupVersionResource = schema.GroupVersionResource{
		Group:    GROUP,
		Version:  VERSION,
		Resource: "bgppeers",
	}
	bgpAdvertisement schema.GroupVersionResource = schema.GroupVersionResource{
		Group:    GROUP,
		Version:  VERSION,
		Resource: "bgpadvertisements",
	}
	addressPool schema.GroupVersionResource = schema.GroupVersionResource{
		Group:    GROUP,
		Version:  VERSION,
		Resource: "addresspools",
	}
	addressBlock schema.GroupVersionResource = schema.GroupVersionResource{
		Group:    GROUP,
		Version:  VERSION,
		Resource: "addressblocks",
	}
)

func getManifestDir() string {
	target := os.Getenv("TARGET")
	manifestDirBase := "../manifests"
	switch target {
	case "kubernetes":
		return filepath.Join(manifestDirBase, "base", "sample")
	case "cni":
		return filepath.Join(manifestDirBase, "cni", "sample")
	default:
		return filepath.Join(manifestDirBase, "base", "sample")
	}
}

func testControllerWorkloads() {
	It("should confirm workloads is working well", func() {
		By("checking sartd-agent working")
		var ds appsv1.DaemonSet
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "kube-system", "get", "ds", "sartd-agent", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &ds)
			g.Expect(err).NotTo(HaveOccurred())
			if ds.Status.NumberAvailable != ds.Status.CurrentNumberScheduled {
				return fmt.Errorf("sartd-agent is not ready")
			}
			return nil
		}).Should(Succeed())

		By("checking sartd-bgp working")
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "kube-system", "get", "ds", "sartd-bgp", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &ds)
			g.Expect(err).NotTo(HaveOccurred())
			if ds.Status.NumberAvailable != ds.Status.CurrentNumberScheduled {
				return fmt.Errorf("sartd-agent is not ready")
			}
			return nil
		}).Should(Succeed())

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

func testClusterBGPA() {
	It("should install BGP related resource", func() {
		By("applying sample resource")
		Eventually(func(g Gomega) error {
			_, err := kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR, "peer_template.yaml"))
			g.Expect(err).NotTo(HaveOccurred())
			_, err = kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR, "cluster_bgp_a.yaml"))
			g.Expect(err).NotTo(HaveOccurred())
			return nil
		}).Should(Succeed())

	})

	It("should create ClusterBGP and BGPPeers", func() {
		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		ctx := context.Background()

		By("checking ClusterBGP")
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
			g.Expect(len(list.Items)).To(Equal(3))
			return nil
		}).Should(Succeed())

		By("checking BGPPeer")
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(bgpPeer).List(ctx, metav1.ListOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(list.Items)).To(Equal(3))
			return nil
		}).Should(Succeed())
	})

	It("should confirm that all bgp peer are established", func() {
		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		ctx := context.Background()

		By("checking sartd-bgp's peer state")
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

func testCreateLBAddressPool() {
	It("should create AddressPool", func() {
		By("applying LB address pool")
		Eventually(func(g Gomega) error {
			_, err := kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR, "lb_address_pool.yaml"))
			g.Expect(err).NotTo(HaveOccurred())
			return nil
		}).Should(Succeed())

		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		ctx := context.Background()

		By("getting AddressPool")

		addressPoolList, err := dynamicClient.Resource(addressPool).List(ctx, metav1.ListOptions{})
		Expect(err).NotTo(HaveOccurred())
		lbPoolList := []unstructured.Unstructured{}
		for _, obj := range addressPoolList.Items {
			t, found, err := unstructured.NestedString(obj.Object, "spec", "type")
			Expect(err).NotTo(HaveOccurred())
			Expect(found).To(BeTrue())
			if t == "service" {
				lbPoolList = append(lbPoolList, obj)
			}
		}
		Expect(len(lbPoolList)).To(Equal(2))

		By("getting AddressBlocks")
		Eventually(func(g Gomega) error {
			for _, pool := range lbPoolList {
				cidr, found, err := unstructured.NestedString(pool.Object, "spec", "cidr")
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(found).To(BeTrue())

				addressBlock, err := dynamicClient.Resource(addressBlock).Get(ctx, pool.GetName(), metav1.GetOptions{})
				g.Expect(err).NotTo(HaveOccurred())
				blockCidr, found, err := unstructured.NestedString(addressBlock.Object, "spec", "cidr")
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(found).To(BeTrue())

				g.Expect(blockCidr).To(Equal(cidr))
			}
			return nil
		}).Should(Succeed())
	})

}

func testClusterBGPB() {

	It("should install BGP related resources", func() {
		By("applying sample resource")
		_, err := kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR, "cluster_bgp_b.yaml"))
		Expect(err).NotTo(HaveOccurred())

		_, err = kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR, "bgp_peer.yaml"))
		Expect(err).NotTo(HaveOccurred())

		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		ctx := context.Background()

		By("checking ClusterBGP")
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(clusterBGP).List(ctx, metav1.ListOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(list.Items)).To(Equal(2))
			return nil
		}).Should(Succeed())

		By("checking NodeBGP with label(bgp=b)")
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(nodeBGP).List(ctx, metav1.ListOptions{LabelSelector: "bgp=b"})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(list.Items)).To(Equal(1))
			return nil
		}).Should(Succeed())

		By("checking BGPPeer")
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(bgpPeer).List(ctx, metav1.ListOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(list.Items)).To(Equal(4))
			return nil
		}).Should(Succeed())

		By("checking BGPPeer status is Established state")
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
	})

	It("should confirm that all bgp peer are established", func() {
		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		ctx := context.Background()

		By("checking sartd-bgp's peer state")
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

func testCreatingLoadBalancer() {
	It("should apply LoadBalancer and related resources", func() {
		By("applying sample resource")
		_, err := kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR, "lb.yaml"))
		Expect(err).NotTo(HaveOccurred())

		By("checking app-cluster working")
		var dp appsv1.Deployment
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test", "get", "deploy", "app-cluster", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &dp)
			g.Expect(err).NotTo(HaveOccurred())
			if dp.Status.Replicas != dp.Status.AvailableReplicas {
				return fmt.Errorf("app-cluster is not ready")
			}
			return nil
		}).Should(Succeed())

		By("checking app-cluster2 working")
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test", "get", "deploy", "app-cluster2", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &dp)
			g.Expect(err).NotTo(HaveOccurred())
			if dp.Status.Replicas != dp.Status.AvailableReplicas {
				return fmt.Errorf("app-cluster2 is not ready")
			}
			return nil
		}).Should(Succeed())

		By("checking app-local working")
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test", "get", "deploy", "app-local", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &dp)
			g.Expect(err).NotTo(HaveOccurred())
			if dp.Status.Replicas != dp.Status.AvailableReplicas {
				return fmt.Errorf("app-local is not ready")
			}
			return nil
		}).Should(Succeed())
	})
}

func testLoadBalancerConnectivity() {
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
			g.Expect(len(peers)).To(Equal(4))
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

	It("should communicate via app-svc-local", func() {
		By("checking app-svc-local gets addresses")
		var svc corev1.Service
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-local", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &svc)
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(svc.Status.LoadBalancer.Ingress)).To(Equal(1))
			return nil
		}).Should(Succeed())

		addr := svc.Status.LoadBalancer.Ingress[0].IP

		By("checking app-local working")
		var dp appsv1.Deployment
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test", "get", "deploy", "app-local", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &dp)
			g.Expect(err).NotTo(HaveOccurred())
			if dp.Status.Replicas != dp.Status.AvailableReplicas {
				return fmt.Errorf("app-local is not ready")
			}
			return nil
		}).Should(Succeed())

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
			g.Expect(len(peers)).To(Equal(int(dp.Status.Replicas)))
			for _, status := range peers {
				g.Expect(status.(string)).To(Equal("Advertised"))
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity to app-svc-local")
		Eventually(func(g Gomega) error {
			_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr)
			g.Expect(err).NotTo(HaveOccurred())
			return nil
		}).Should(Succeed())
	})

	It("should communicate via app-svc-cluster2", func() {

		By("checking app-svc-cluster2 gets addresses")
		var svc corev1.Service
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-cluster2", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &svc)
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(svc.Status.LoadBalancer.Ingress)).To(Equal(1))
			return nil
		}).Should(Succeed())

		addr := svc.Status.LoadBalancer.Ingress[0].IP

		ctx := context.Background()
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		By("getting AddressPool")
		block, err := dynamicClient.Resource(addressBlock).Get(ctx, "non-default-lb-pool", metav1.GetOptions{})
		Expect(err).NotTo(HaveOccurred())

		cidrStr, found, err := unstructured.NestedString(block.Object, "spec", "cidr")
		Expect(err).NotTo(HaveOccurred())
		Expect(found).To(BeTrue())

		By("checking allocated address")
		_, cidr, err := net.ParseCIDR(cidrStr)
		Expect(err).NotTo(HaveOccurred())
		a := net.ParseIP(addr)
		Expect(cidr.Contains(a)).To(BeTrue())

		By("checking the desired address is allocated")
		desired, ok := svc.Annotations["sart.terassyi.net/loadBalancerIPs"]
		Expect(ok).To(BeTrue())
		Expect(addr).To(Equal(desired))

		By("checking BGPAdvertisement")
		Eventually(func(g Gomega) error {
			advList, err := dynamicClient.Resource(bgpAdvertisement).Namespace(svc.Namespace).List(ctx, metav1.ListOptions{LabelSelector: fmt.Sprintf("kubernetes.io/service-name=%s", svc.Name)})
			g.Expect(err).NotTo(HaveOccurred())
			peers, found, err := unstructured.NestedMap(advList.Items[0].Object, "status", "peers")
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(found).To(BeTrue())
			g.Expect(len(peers)).To(Equal(4))
			for _, status := range peers {
				g.Expect(status.(string)).To(Equal("Advertised"))
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity to app-svc-cluster2")
		Eventually(func(g Gomega) error {
			_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr)
			g.Expect(err).NotTo(HaveOccurred())
			return nil
		}).Should(Succeed())
	})
}

func testAddressPool() {
	It("should change AddressPool", func() {
		By("getting app-svc-cluster gets addresses")
		var svc corev1.Service
		out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-cluster", "-ojson")
		Expect(err).NotTo(HaveOccurred())
		err = json.Unmarshal(out, &svc)
		Expect(err).NotTo(HaveOccurred())
		Expect(len(svc.Status.LoadBalancer.Ingress)).To(Equal(1))

		oldAddr := svc.Status.LoadBalancer.Ingress[0].IP

		By("patching to change allocating address pool")
		_, err = kubectl(nil, "-n", svc.Namespace, "patch", "svc", svc.Name, "-p", `{"metadata": {"annotations": {"sart.terassyi.net/addresspool": "non-default-lb-pool"}}}`)
		Expect(err).NotTo(HaveOccurred())

		By("checking the allocation changed")
		var newAddr string
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-cluster", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &svc)
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(svc.Status.LoadBalancer.Ingress)).To(Equal(1))

			newAddr = svc.Status.LoadBalancer.Ingress[0].IP
			g.Expect(oldAddr).NotTo(Equal(newAddr))
			return nil
		}).Should(Succeed())

		By("getting dynamic client")
		ctx := context.Background()
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		By("getting advertisement")
		Eventually(func(g Gomega) error {
			advList, err := dynamicClient.Resource(bgpAdvertisement).Namespace(svc.Namespace).List(ctx, metav1.ListOptions{LabelSelector: fmt.Sprintf("kubernetes.io/service-name=%s", svc.Name)})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(advList.Items)).To(Equal(1))

			cidr, found, err := unstructured.NestedString(advList.Items[0].Object, "spec", "cidr")
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(found).To(BeTrue())
			g.Expect(cidr).To(Equal(fmt.Sprintf("%s/32", newAddr)))
			return nil
		}).Should(Succeed())

		By("checking not to get the connectivity app-svc-cluster with old address")
		Eventually(func() error {
			_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", oldAddr)
			if err != nil {
				return nil
			}
			return fmt.Errorf("it must not be able to connect to old address")
		}).Should(Succeed())

		By("checking the connectivity to app-svc-cluster by new address")
		Eventually(func(g Gomega) error {
			_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", newAddr)
			g.Expect(err).NotTo(HaveOccurred())
			return nil
		}).Should(Succeed())
	})

	It("should add the additional address", func() {
		By("getting app-svc-cluster gets addresses")
		var svc corev1.Service
		out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-cluster", "-ojson")
		Expect(err).NotTo(HaveOccurred())
		err = json.Unmarshal(out, &svc)
		Expect(err).NotTo(HaveOccurred())
		Expect(len(svc.Status.LoadBalancer.Ingress)).To(Equal(1))

		By("patching to add allocating address pool")
		_, err = kubectl(nil, "-n", svc.Namespace, "patch", "svc", svc.Name, "-p", `{"metadata": {"annotations": {"sart.terassyi.net/addresspool": "non-default-lb-pool,default-lb-pool"}}}`)
		Expect(err).NotTo(HaveOccurred())

		By("checking the allocation added")
		addrs := make([]string, 0, 2)
		Eventually(func(g Gomega) error {
			addrs = make([]string, 0, 2)
			out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-cluster", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &svc)
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(svc.Status.LoadBalancer.Ingress)).To(Equal(2))
			for _, ingress := range svc.Status.LoadBalancer.Ingress {
				addrs = append(addrs, ingress.IP)
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity for both addresses")
		Eventually(func(g Gomega) error {
			_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addrs[0])
			g.Expect(err).NotTo(HaveOccurred())
			return nil
		}).Should(Succeed())

		Eventually(func(g Gomega) error {
			_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addrs[1])
			g.Expect(err).NotTo(HaveOccurred())
			return nil
		}).Should(Succeed())
	})
}

func testExternalTrafficPolicy() {
	It("should update advertisement when externalTrafficPolicy changes", func() {
		By("checking app-svc-cluster2's externalTrafficPolicy")
		var svc corev1.Service
		out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-cluster2", "-ojson")
		Expect(err).NotTo(HaveOccurred())
		err = json.Unmarshal(out, &svc)
		Expect(err).NotTo(HaveOccurred())
		Expect(svc.Spec.ExternalTrafficPolicy).To(Equal(corev1.ServiceExternalTrafficPolicyCluster))

		By("getting dynamic client")
		ctx := context.Background()
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		By("checking existing advertisement")
		Eventually(func(g Gomega) error {
			advList, err := dynamicClient.Resource(bgpAdvertisement).Namespace(svc.Namespace).List(ctx, metav1.ListOptions{LabelSelector: fmt.Sprintf("kubernetes.io/service-name=%s", svc.Name)})
			g.Expect(err).NotTo(HaveOccurred())
			peers, found, err := unstructured.NestedMap(advList.Items[0].Object, "status", "peers")
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(found).To(BeTrue())
			g.Expect(len(peers)).To(Equal(4))
			for _, status := range peers {
				g.Expect(status.(string)).To(Equal("Advertised"))
			}
			return nil
		}).Should(Succeed())

		By("getting the backend deployment")
		dpName, ok := svc.Spec.Selector["app"]
		Expect(ok).To(BeTrue())

		var dp appsv1.Deployment
		out, err = kubectl(nil, "-n", svc.Namespace, "get", "deploy", dpName, "-ojson")
		Expect(err).NotTo(HaveOccurred())
		err = json.Unmarshal(out, &dp)
		Expect(err).NotTo(HaveOccurred())

		By("getting backend pod list")
		var podList corev1.PodList
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", svc.Namespace, "get", "pod", "-l", fmt.Sprintf("app=%s", dpName), "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &podList)
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(podList.Items)).To(Equal(int(dp.Status.Replicas)))
			return nil
		}).Should(Succeed())

		By("getting backend nodes")
		nodes := make([]string, 0)
		for _, pod := range podList.Items {
			nodes = append(nodes, pod.Spec.NodeName)
		}

		peers := make([]string, 0)
		By("getting BGPPeer list")
		for _, node := range nodes {
			nb, err := dynamicClient.Resource(nodeBGP).Get(ctx, node, metav1.GetOptions{})
			Expect(err).NotTo(HaveOccurred())
			res, found, err := unstructured.NestedSlice(nb.Object, "spec", "peers")
			Expect(err).NotTo(HaveOccurred())
			Expect(found).To(BeTrue())
			for _, r := range res {
				peer := r.(map[string]any)
				name, ok := peer["name"]
				Expect(ok).To(BeTrue())
				peers = append(peers, name.(string))
			}
		}

		By("patching to externalTrafficPolicy=Local")
		_, err = kubectl(nil, "-n", svc.Namespace, "patch", "svc", svc.Name, "-p", `{"spec": {"externalTrafficPolicy": "Local"}}`)
		Expect(err).NotTo(HaveOccurred())

		By("checking new advertisement")
		Eventually(func(g Gomega) error {
			advList, err := dynamicClient.Resource(bgpAdvertisement).Namespace(svc.Namespace).List(ctx, metav1.ListOptions{LabelSelector: fmt.Sprintf("kubernetes.io/service-name=%s", svc.Name)})
			g.Expect(err).NotTo(HaveOccurred())
			targets, found, err := unstructured.NestedMap(advList.Items[0].Object, "status", "peers")
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(found).To(BeTrue())
			g.Expect(len(targets)).To(Equal(int(dp.Status.Replicas)))
			for _, p := range peers {
				status, ok := targets[p]
				g.Expect(ok).To(BeTrue())
				g.Expect(status.(string)).To(Equal("Advertised"))
			}
			return nil
		}).Should(Succeed())
	})

	It("should handle to rollout restart with externalTrafficPolicy=Cluster", func() {
		By("checking app-svc-cluster gets addresses")
		var svc corev1.Service
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-cluster", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &svc)
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(svc.Status.LoadBalancer.Ingress)).NotTo(Equal(0))
			return nil
		}).Should(Succeed())

		addr := svc.Status.LoadBalancer.Ingress[0].IP

		By("doing rollout restart for app-cluster")
		_, err := kubectl(nil, "-n", svc.Namespace, "rollout", "restart", "deploy/app-cluster")
		Expect(err).NotTo(HaveOccurred())

		By("waiting rollout restart completed")
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", svc.Namespace, "rollout", "status", "deploy/app-cluster")
			g.Expect(err).NotTo(HaveOccurred())
			if !strings.Contains(string(out), "successfully") {
				return fmt.Errorf("restarting yet")
			}
			return nil
		}).Should(Succeed())

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
			g.Expect(len(peers)).To(Equal(4))
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

	It("should handle to rollout restart with externalTrafficPolicy=Local", func() {
		By("checking app-svc-local gets addresses")
		var svc corev1.Service
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-local", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &svc)
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(svc.Status.LoadBalancer.Ingress)).NotTo(Equal(0))
			return nil
		}).Should(Succeed())

		addr := svc.Status.LoadBalancer.Ingress[0].IP

		By("doing rollout restart for app-local")
		_, err := kubectl(nil, "-n", svc.Namespace, "rollout", "restart", "deploy/app-local")
		Expect(err).NotTo(HaveOccurred())

		By("waiting rollout restart completed")
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", svc.Namespace, "rollout", "status", "deploy/app-local")
			g.Expect(err).NotTo(HaveOccurred())
			if !strings.Contains(string(out), "successfully") {
				return fmt.Errorf("restarting yet")
			}
			return nil
		}).Should(Succeed())

		By("getting pods after rollout restart")
		dpName, ok := svc.Spec.Selector["app"]
		Expect(ok).To(BeTrue())

		var dp appsv1.Deployment
		out, err := kubectl(nil, "-n", svc.Namespace, "get", "deploy", dpName, "-ojson")
		Expect(err).NotTo(HaveOccurred())
		err = json.Unmarshal(out, &dp)
		Expect(err).NotTo(HaveOccurred())

		By("getting backend pod list")
		var podList corev1.PodList
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", svc.Namespace, "get", "pod", "-l", fmt.Sprintf("app=%s", dpName), "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &podList)
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(podList.Items)).To(Equal(int(dp.Status.Replicas)))
			return nil
		}).Should(Succeed())

		By("getting backend nodes")
		nodes := make([]string, 0)
		for _, pod := range podList.Items {
			nodes = append(nodes, pod.Spec.NodeName)
		}

		By("getting dynamic client")
		ctx := context.Background()
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		peers := make([]string, 0)
		By("getting BGPPeer list")
		for _, node := range nodes {
			nb, err := dynamicClient.Resource(nodeBGP).Get(ctx, node, metav1.GetOptions{})
			Expect(err).NotTo(HaveOccurred())
			res, found, err := unstructured.NestedSlice(nb.Object, "spec", "peers")
			Expect(err).NotTo(HaveOccurred())
			Expect(found).To(BeTrue())
			for _, r := range res {
				peer := r.(map[string]any)
				name, ok := peer["name"]
				Expect(ok).To(BeTrue())
				peers = append(peers, name.(string))
			}
		}

		By("checking new advertisement")
		Eventually(func(g Gomega) error {
			advList, err := dynamicClient.Resource(bgpAdvertisement).Namespace(svc.Namespace).List(ctx, metav1.ListOptions{LabelSelector: fmt.Sprintf("kubernetes.io/service-name=%s", svc.Name)})
			g.Expect(err).NotTo(HaveOccurred())
			targets, found, err := unstructured.NestedMap(advList.Items[0].Object, "status", "peers")
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(found).To(BeTrue())
			g.Expect(len(targets)).To(Equal(int(dp.Status.Replicas)))
			for _, p := range peers {
				status, ok := targets[p]
				g.Expect(ok).To(BeTrue())
				g.Expect(status.(string)).To(Equal("Advertised"))
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity to app-svc-local")
		Eventually(func(g Gomega) error {
			_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr)
			g.Expect(err).NotTo(HaveOccurred())
			return nil
		}).Should(Succeed())
	})
}

func testBGPLabelChange() {
	It("should check ClusterBGP's status", func() {
		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		ctx := context.Background()

		By("getting ClusterBGP bgp=a")
		cbA, err := dynamicClient.Resource(clusterBGP).Get(ctx, "clusterbgp-sample-a", metav1.GetOptions{})
		Expect(err).NotTo(HaveOccurred())
		nodes, found, err := unstructured.NestedStringSlice(cbA.Object, "status", "nodes")
		Expect(err).NotTo(HaveOccurred())
		Expect(found).To(BeTrue())
		Expect(nodes).To(Equal([]string{"sart-worker", "sart-worker2", "sart-worker3"}))

		By("getting ClusterBGP bgp=a")
		cbB, err := dynamicClient.Resource(clusterBGP).Get(ctx, "clusterbgp-sample-b", metav1.GetOptions{})
		Expect(err).NotTo(HaveOccurred())
		nodes, found, err = unstructured.NestedStringSlice(cbB.Object, "status", "nodes")
		Expect(err).NotTo(HaveOccurred())
		Expect(found).To(BeTrue())
		Expect(nodes).To(Equal([]string{"sart-control-plane"}))
	})

	It("should change Node's label", func() {
		By("changing sart-control-plane's bgp label")
		_, err := kubectl(nil, "label", "node", "--overwrite", "sart-control-plane", "bgp=a")
		Expect(err).NotTo(HaveOccurred())

		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		ctx := context.Background()

		By("getting ClusterBGP bgp=a")
		Eventually(func(g Gomega) error {
			cbA, err := dynamicClient.Resource(clusterBGP).Get(ctx, "clusterbgp-sample-a", metav1.GetOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			nodes, found, err := unstructured.NestedStringSlice(cbA.Object, "status", "nodes")
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(found).To(BeTrue())
			g.Expect(nodes).To(Equal([]string{"sart-control-plane", "sart-worker", "sart-worker2", "sart-worker3"}))
			return nil
		}).Should(Succeed())

		By("getting ClusterBGP bgp=b")
		Eventually(func(g Gomega) error {
			cbB, err := dynamicClient.Resource(clusterBGP).Get(ctx, "clusterbgp-sample-b", metav1.GetOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			nodes, found, err := unstructured.NestedStringSlice(cbB.Object, "status", "nodes")
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(found).To(BeTrue())
			g.Expect(nodes).To(Equal([]string{}))
			return nil
		}).Should(Succeed())

		By("checking NodeBGP has `bgp=a`'s peer definition")
		var peerName string
		Eventually(func(g Gomega) error {
			nb, err := dynamicClient.Resource(nodeBGP).Get(ctx, "sart-control-plane", metav1.GetOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			peers, found, err := unstructured.NestedSlice(nb.Object, "spec", "peers")
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(found).To(BeTrue())
			g.Expect(len(peers)).To(Equal(2))

			var peerMap map[string]any
			peerMap, ok := peers[1].(map[string]any)
			g.Expect(ok).To(BeTrue())

			peerName, ok = peerMap["name"].(string)
			g.Expect(ok).To(BeTrue())

			return nil
		}).Should(Succeed())

		By("blocking to create sart-control-plane's BGPPeer by webhook")
		Eventually(func(g Gomega) error {
			_, err := dynamicClient.Resource(bgpPeer).Get(ctx, peerName, metav1.GetOptions{})
			if err != nil {
				if apierrors.IsNotFound(err) {
					return nil
				}
				return err
			}

			return fmt.Errorf("%s must not exist", peerName)
		}).WithTimeout(10 * time.Second).WithPolling(time.Second).Should(Succeed())
	})
}

func testDeleteClusterBGP() {
	It("should delete ClusterBGP b", func() {
		By("deleting ClusterBGP")
		_, err := kubectl(nil, "delete", "clusterbgp", "clusterbgp-sample-b")
		Expect(err).NotTo(HaveOccurred())
	})
}

func testDeleteBGPPeer() {
	It("should delete BGPPeer", func() {
		By("deleting bgppeer-sample-sart-cp")
		_, err := kubectl(nil, "delete", "bgppeer", "bgppeer-sample-sart-cp")
		Expect(err).NotTo(HaveOccurred())

		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())
		ctx := context.Background()

		By("checking bgppeer-sample-sart-cp doesn't exist")
		Eventually(func(g Gomega) error {
			_, err := dynamicClient.Resource(bgpPeer).Get(ctx, "bgppeer-sample-sart-cp", metav1.GetOptions{})
			if err != nil {
				if apierrors.IsNotFound(err) {
					return nil
				}
				return err
			}
			return fmt.Errorf("bgppeer-sample-sart-cp must not exist")
		}).WithTimeout(10 * time.Second).WithPolling(time.Second).Should(Succeed())

		By("checking NodeBGP has `bgp=a`'s peer definition")
		Eventually(func(g Gomega) error {
			nb, err := dynamicClient.Resource(nodeBGP).Get(ctx, "sart-control-plane", metav1.GetOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			peers, found, err := unstructured.NestedSlice(nb.Object, "spec", "peers")
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(found).To(BeTrue())
			g.Expect(len(peers)).To(Equal(1))

			return nil
		}).Should(Succeed())

	})

	It("should create new BGPPeer by clusterbgp-sample-a", func() {
		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())
		ctx := context.Background()

		By("checking NodeBGP has `bgp=a`'s peer definition")
		var peerName string
		nb, err := dynamicClient.Resource(nodeBGP).Get(ctx, "sart-control-plane", metav1.GetOptions{})
		Expect(err).NotTo(HaveOccurred())
		peers, found, err := unstructured.NestedSlice(nb.Object, "spec", "peers")
		Expect(err).NotTo(HaveOccurred())
		Expect(found).To(BeTrue())
		Expect(len(peers)).To(Equal(1))

		var peerMap map[string]any
		peerMap, ok := peers[0].(map[string]any)
		Expect(ok).To(BeTrue())

		peerName, ok = peerMap["name"].(string)
		Expect(ok).To(BeTrue())

		By("checking new BGPPeer is created")
		Eventually(func(g Gomega) error {
			_, err := dynamicClient.Resource(bgpPeer).Get(ctx, peerName, metav1.GetOptions{})
			g.Expect(err).NotTo(HaveOccurred())

			return nil
		}).Should(Succeed())
	})

	It("should replace target peers in BGPAdvertisement", func() {
		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())
		ctx := context.Background()

		By("getting the peer name")
		var peerName string
		nb, err := dynamicClient.Resource(nodeBGP).Get(ctx, "sart-control-plane", metav1.GetOptions{})
		Expect(err).NotTo(HaveOccurred())
		peers, found, err := unstructured.NestedSlice(nb.Object, "spec", "peers")
		Expect(err).NotTo(HaveOccurred())
		Expect(found).To(BeTrue())
		Expect(len(peers)).To(Equal(1))

		var peerMap map[string]any
		peerMap, ok := peers[0].(map[string]any)
		Expect(ok).To(BeTrue())

		peerName, ok = peerMap["name"].(string)
		Expect(ok).To(BeTrue())

		By("getting app-svc-cluster gets addresses")
		var svc corev1.Service
		out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-cluster", "-ojson")
		Expect(err).NotTo(HaveOccurred())
		err = json.Unmarshal(out, &svc)
		Expect(err).NotTo(HaveOccurred())

		By("checking BGPAdvertisement")
		Eventually(func(g Gomega) error {
			advList, err := dynamicClient.Resource(bgpAdvertisement).Namespace(svc.Namespace).List(ctx, metav1.ListOptions{LabelSelector: fmt.Sprintf("kubernetes.io/service-name=%s", svc.Name)})
			g.Expect(err).NotTo(HaveOccurred())
			peers, found, err := unstructured.NestedMap(advList.Items[0].Object, "status", "peers")
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(found).To(BeTrue())
			g.Expect(len(peers)).To(Equal(4))
			for _, status := range peers {
				g.Expect(status.(string)).To(Equal("Advertised"))
			}
			_, ok := peers[peerName]
			g.Expect(ok).To(BeTrue())
			return nil
		}).Should(Succeed())

		By("communicating with app-svc-cluster")
		addr := svc.Status.LoadBalancer.Ingress[0].IP
		By("checking the connectivity to app-svc-cluster")
		Eventually(func() error {
			_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr)
			if err != nil {
				return err
			}
			return nil
		}).Should(Succeed())
	})
}

func testClusterBGPC() {

	nodes := []string{"sart-control-plane", "sart-worker", "sart-worker2", "sart-worker3"}

	It("should create ClusterBGP bgp=c", func() {
		By("labeling bgp2=c")
		for _, node := range nodes {
			_, err := kubectl(nil, "label", "node", "--overwrite", node, "bgp2=c")
			Expect(err).NotTo(HaveOccurred())
		}

		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())
		ctx := context.Background()

		By("checking NodeBGP with label(bgp=a)")
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(nodeBGP).List(ctx, metav1.ListOptions{LabelSelector: "bgp2=c"})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(list.Items)).To(Equal(4))
			return nil
		}).Should(Succeed())

		By("creating ClusterBGP bgp2=c")
		_, err = kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR, "cluster_bgp_c.yaml"))
		Expect(err).NotTo(HaveOccurred())

		By("checking ClusterBGP has all NodeBGP in status")
		Eventually(func(g Gomega) error {
			cb, err := dynamicClient.Resource(clusterBGP).Get(ctx, "clusterbgp-sample-c", metav1.GetOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			desiredNodes, found, err := unstructured.NestedStringSlice(cb.Object, "status", "desiredNodes")
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(found).To(BeTrue())
			g.Expect(desiredNodes).To(Equal(nodes))

			actualNodes, found, err := unstructured.NestedStringSlice(cb.Object, "status", "nodes")
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(found).To(BeTrue())
			g.Expect(actualNodes).To(Equal(nodes))

			return nil
		}).Should(Succeed())

		By("checking BGPPeer")
		Eventually(func(g Gomega) error {
			bpList, err := dynamicClient.Resource(bgpPeer).List(ctx, metav1.ListOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(bpList.Items)).To(Equal(8))
			return nil
		}).Should(Succeed())

		By("checking BGPPeer status is Established")
		Eventually(func(g Gomega) error {
			bpList, err := dynamicClient.Resource(bgpPeer).List(ctx, metav1.ListOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			for _, bp := range bpList.Items {
				conds, found, err := unstructured.NestedSlice(bp.Object, "status", "conditions")
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(found).To(BeTrue())
				cond, ok := conds[len(conds)-1].(map[string]any)
				g.Expect(ok).To(BeTrue())
				status, ok := cond["status"].(string)
				g.Expect(ok).To(BeTrue())
				g.Expect(status).To(Equal("Established"))
			}
			return nil
		}).Should(Succeed())
	})

	It("should confirm that all bgp peer are established", func() {
		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		ctx := context.Background()

		By("checking sartd-bgp's peer state for clusterbgp-sample-c")
		Eventually(func(g Gomega) error {
			list, err := dynamicClient.Resource(bgpPeer).List(ctx, metav1.ListOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			for _, p := range list.Items {
				cbRef, found, err := unstructured.NestedString(p.Object, "spec", "clusterBGPRef")
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(found).To(BeTrue())
				if cbRef != "clusterbgp-sample-c" {
					continue
				}

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

	It("should change the routeing policy", func() {
		By("changing the client0's route")
		_, err := execInContainer("clab-sart-client0", nil, "ip", "route", "change", "default", "src", "6.6.6.6", "nexthop", "via", "192.168.0.1", "weight", "1", "nexthop", "via", "192.168.1.1", "weight", "1")
		Expect(err).NotTo(HaveOccurred())

		By("changing k8s node's routes")
		_, err = execInContainer("sart-control-plane", nil, "ip", "route", "change", "6.6.6.6/32", "nexthop", "via", "169.254.1.1", "weight", "1", "nexthop", "via", "169.253.1.1", "weight", "1")
		Expect(err).NotTo(HaveOccurred())
		_, err = execInContainer("sart-worker", nil, "ip", "route", "change", "6.6.6.6/32", "nexthop", "via", "169.254.2.1", "weight", "1", "nexthop", "via", "169.253.2.1", "weight", "1")
		Expect(err).NotTo(HaveOccurred())
		_, err = execInContainer("sart-worker2", nil, "ip", "route", "change", "6.6.6.6/32", "nexthop", "via", "169.254.3.1", "weight", "1", "nexthop", "via", "169.253.3.1", "weight", "1")
		Expect(err).NotTo(HaveOccurred())
		_, err = execInContainer("sart-worker3", nil, "ip", "route", "change", "6.6.6.6/32", "nexthop", "via", "169.254.4.1", "weight", "1", "nexthop", "via", "169.253.4.1", "weight", "1")
		Expect(err).NotTo(HaveOccurred())
	})

	It("should communicate via app-svc-cluster", func() {

		By("checking app-svc-cluster gets addresses")
		var svc corev1.Service
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-cluster", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &svc)
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(svc.Status.LoadBalancer.Ingress)).To(Equal(2))
			return nil
		}).Should(Succeed())

		addr1 := svc.Status.LoadBalancer.Ingress[0].IP
		addr2 := svc.Status.LoadBalancer.Ingress[1].IP

		By("checking BGPAdvertisement for addr[0]")
		ctx := context.Background()
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())
		Eventually(func(g Gomega) error {
			advList, err := dynamicClient.Resource(bgpAdvertisement).Namespace(svc.Namespace).List(ctx, metav1.ListOptions{LabelSelector: fmt.Sprintf("kubernetes.io/service-name=%s", svc.Name)})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(advList.Items)).To(Equal(2))

			for _, adv := range advList.Items {
				peers, found, err := unstructured.NestedMap(adv.Object, "status", "peers")
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(found).To(BeTrue())

				g.Expect(len(peers)).To(Equal(8))
				for _, status := range peers {
					statusStr, ok := status.(string)
					g.Expect(ok).To(BeTrue())
					g.Expect(statusStr).To(Equal("Advertised"))
				}
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity to app-svc-cluster by addr1")
		Eventually(func(g Gomega) error {
			for i := 0; i < 100; i++ {
				_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr1)
				g.Expect(err).NotTo(HaveOccurred())
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity to app-svc-cluster by addr2")
		Eventually(func(g Gomega) error {
			for i := 0; i < 100; i++ {
				_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr2)
				g.Expect(err).NotTo(HaveOccurred())

			}
			return nil
		}).Should(Succeed())
	})

	It("should communicate via app-svc-cluster2", func() {

		By("checking app-svc-cluster2 gets addresses")
		var svc corev1.Service
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-cluster2", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &svc)
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(svc.Status.LoadBalancer.Ingress)).To(Equal(1))
			return nil
		}).Should(Succeed())

		addr := svc.Status.LoadBalancer.Ingress[0].IP

		By("checking BGPAdvertisement for addr")
		ctx := context.Background()
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())
		Eventually(func(g Gomega) error {
			advList, err := dynamicClient.Resource(bgpAdvertisement).Namespace(svc.Namespace).List(ctx, metav1.ListOptions{LabelSelector: fmt.Sprintf("kubernetes.io/service-name=%s", svc.Name)})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(advList.Items)).To(Equal(1))

			for _, adv := range advList.Items {
				peers, found, err := unstructured.NestedMap(adv.Object, "status", "peers")
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(found).To(BeTrue())

				g.Expect(len(peers)).To(Equal(4))
				for _, status := range peers {
					statusStr, ok := status.(string)
					g.Expect(ok).To(BeTrue())
					g.Expect(statusStr).To(Equal("Advertised"))
				}
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity to app-svc-cluster2")
		Eventually(func(g Gomega) error {
			for i := 0; i < 100; i++ {
				_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr)
				g.Expect(err).NotTo(HaveOccurred())
			}
			return nil
		}).Should(Succeed())
	})

	It("should communicate via app-svc-local", func() {

		By("checking app-svc-local gets addresses")
		var svc corev1.Service
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-local", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &svc)
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(svc.Status.LoadBalancer.Ingress)).To(Equal(1))
			return nil
		}).Should(Succeed())

		addr := svc.Status.LoadBalancer.Ingress[0].IP

		By("checking BGPAdvertisement for addr")
		ctx := context.Background()
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())
		Eventually(func(g Gomega) error {
			advList, err := dynamicClient.Resource(bgpAdvertisement).Namespace(svc.Namespace).List(ctx, metav1.ListOptions{LabelSelector: fmt.Sprintf("kubernetes.io/service-name=%s", svc.Name)})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(advList.Items)).To(Equal(1))

			for _, adv := range advList.Items {
				peers, found, err := unstructured.NestedMap(adv.Object, "status", "peers")
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(found).To(BeTrue())

				g.Expect(len(peers)).To(Equal(4))
				for _, status := range peers {
					statusStr, ok := status.(string)
					g.Expect(ok).To(BeTrue())
					g.Expect(statusStr).To(Equal("Advertised"))
				}
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity to app-svc-local")
		Eventually(func(g Gomega) error {
			for i := 0; i < 100; i++ {
				_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr)
				g.Expect(err).NotTo(HaveOccurred())
			}
			return nil
		}).Should(Succeed())
	})
}

func testRestartAgent() {
	It("should rollout restart sartd-agent", func() {
		svcMap := make(map[string][]string)
		svcs := []string{"app-svc-cluster", "app-svc-cluster2", "app-svc-local"}

		By("getting existing allocations")
		var svc corev1.Service
		for _, s := range svcs {
			out, err := kubectl(nil, "-n", "test", "get", "svc", s, "-ojson")
			Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &svc)
			Expect(err).NotTo(HaveOccurred())
			ips := make([]string, 0)
			for _, ingress := range svc.Status.LoadBalancer.Ingress {
				ips = append(ips, ingress.IP)
			}
			svcMap[s] = ips
		}

		By("rolling restart sartd-agent")
		_, err := kubectl(nil, "-n", "kube-system", "rollout", "restart", "ds/sartd-agent")
		Expect(err).NotTo(HaveOccurred())

		By("waiting for rollout restart")
		_, err = kubectl(nil, "-n", "kube-system", "rollout", "status", "ds/sartd-agent")
		Expect(err).NotTo(HaveOccurred())

		By("checking the connectivity to app-svc-cluster")
		addrs, ok := svcMap["app-svc-cluster"]
		Expect(ok).To(BeTrue())
		Expect(len(addrs)).To(Equal(2))

		Eventually(func(g Gomega) error {
			for i := 0; i < 100; i++ {
				_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addrs[0])
				g.Expect(err).NotTo(HaveOccurred())
			}
			return nil
		}).Should(Succeed())

		Eventually(func(g Gomega) error {
			for i := 0; i < 100; i++ {
				_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addrs[1])
				g.Expect(err).NotTo(HaveOccurred())
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity to app-svc-cluster2")
		addrs, ok = svcMap["app-svc-cluster2"]
		Expect(ok).To(BeTrue())
		Expect(len(addrs)).To(Equal(1))
		addr := addrs[0]

		Eventually(func(g Gomega) error {
			for i := 0; i < 100; i++ {
				_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr)
				g.Expect(err).NotTo(HaveOccurred())
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity to app-svc-local")
		addrs, ok = svcMap["app-svc-local"]
		Expect(ok).To(BeTrue())
		Expect(len(addrs)).To(Equal(1))
		addr = addrs[0]

		Eventually(func(g Gomega) error {
			for i := 0; i < 100; i++ {
				_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr)
				g.Expect(err).NotTo(HaveOccurred())
			}
			return nil
		}).Should(Succeed())
	})
}

func testRestartBGP() {
	It("should restart one of a sartd-bgp pod", func() {
		By("getting sart-worker's bgp pod")
		out, err := kubectl(nil, "-n", "kube-system", "get", "pod", "-l", "app=sartd", "--field-selector", "spec.nodeName=sart-worker", "-ojson")
		Expect(err).NotTo(HaveOccurred())
		var podList corev1.PodList
		err = json.Unmarshal(out, &podList)
		Expect(err).NotTo(HaveOccurred())
		Expect(len(podList.Items)).To(Equal(1))
		pod := podList.Items[0]

		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		ctx := context.Background()

		By("getting sart-worker's NodeBGP")
		nb, err := dynamicClient.Resource(nodeBGP).Get(ctx, "sart-worker", metav1.GetOptions{})
		Expect(err).NotTo(HaveOccurred())
		backoff, found, err := unstructured.NestedInt64(nb.Object, "status", "backoff")
		Expect(err).NotTo(HaveOccurred())
		Expect(found).To(BeTrue())

		By("getting BGPAdvertisement")
		baList, err := dynamicClient.Resource(bgpAdvertisement).List(ctx, metav1.ListOptions{LabelSelector: "kubernetes.io/service-name=app-svc-cluster"})
		Expect(err).NotTo(HaveOccurred())
		Expect(len(baList.Items)).To(Equal(2))

		By("deleting sart-worker's bgp pod")
		_, err = kubectl(nil, "-n", "kube-system", "delete", "pod", pod.Name)
		Expect(err).NotTo(HaveOccurred())

		By("checking NodeBGP's backoff")
		Eventually(func(g Gomega) error {
			nb, err := dynamicClient.Resource(nodeBGP).Get(ctx, "sart-worker", metav1.GetOptions{})
			g.Expect(err).NotTo(HaveOccurred())
			newBackoff, found, err := unstructured.NestedInt64(nb.Object, "status", "backoff")
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(found).To(BeTrue())
			g.Expect(newBackoff).To(Equal(backoff + 1))
			return nil
		}).Should(Succeed())

		By("checking BGPPeer's backoff count")
		peers, found, err := unstructured.NestedSlice(nb.Object, "spec", "peers")
		Expect(err).NotTo(HaveOccurred())
		Expect(found).To(BeTrue())
		Eventually(func(g Gomega) error {
			for _, peerIf := range peers {
				peer, ok := peerIf.(map[string]any)
				g.Expect(ok).To(BeTrue())
				name, ok := peer["name"].(string)
				g.Expect(ok).To(BeTrue())

				bp, err := dynamicClient.Resource(bgpPeer).Get(ctx, name, metav1.GetOptions{})
				g.Expect(err).NotTo(HaveOccurred())

				backoff, found, err := unstructured.NestedInt64(bp.Object, "status", "backoff")
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(found).To(BeTrue())
				g.Expect(backoff).NotTo(Equal(0))
			}
			return nil
		}).Should(Succeed())
	})

	It("should rollout restart sartd-bgp", func() {
		svcMap := make(map[string][]string)
		svcs := []string{"app-svc-cluster", "app-svc-cluster2", "app-svc-local"}

		By("getting existing allocations")
		var svc corev1.Service
		for _, s := range svcs {
			out, err := kubectl(nil, "-n", "test", "get", "svc", s, "-ojson")
			Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &svc)
			Expect(err).NotTo(HaveOccurred())
			ips := make([]string, 0)
			for _, ingress := range svc.Status.LoadBalancer.Ingress {
				ips = append(ips, ingress.IP)
			}
			svcMap[s] = ips
		}

		By("rolling restart sartd-bgp")
		_, err := kubectl(nil, "-n", "kube-system", "rollout", "restart", "ds/sartd-bgp")
		Expect(err).NotTo(HaveOccurred())

		By("waiting for rollout restart")
		_, err = kubectl(nil, "-n", "kube-system", "rollout", "status", "ds/sartd-bgp")
		Expect(err).NotTo(HaveOccurred())

		By("checking the connectivity to app-svc-cluster")
		addrs, ok := svcMap["app-svc-cluster"]
		Expect(ok).To(BeTrue())
		Expect(len(addrs)).To(Equal(2))

		Eventually(func(g Gomega) error {
			for i := 0; i < 100; i++ {
				_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addrs[0])
				g.Expect(err).NotTo(HaveOccurred())
			}
			return nil
		}).Should(Succeed())

		Eventually(func(g Gomega) error {
			for i := 0; i < 100; i++ {
				_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addrs[1])
				g.Expect(err).NotTo(HaveOccurred())
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity to app-svc-cluster2")
		addrs, ok = svcMap["app-svc-cluster2"]
		Expect(ok).To(BeTrue())
		Expect(len(addrs)).To(Equal(1))
		addr := addrs[0]

		Eventually(func(g Gomega) error {
			for i := 0; i < 100; i++ {
				_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr)
				g.Expect(err).NotTo(HaveOccurred())
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity to app-svc-local")
		addrs, ok = svcMap["app-svc-local"]
		Expect(ok).To(BeTrue())
		Expect(len(addrs)).To(Equal(1))
		addr = addrs[0]

		Eventually(func(g Gomega) error {
			for i := 0; i < 100; i++ {
				_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr)
				g.Expect(err).NotTo(HaveOccurred())
			}
			return nil
		}).Should(Succeed())

		By("checking BGPAdvertisement for app-svc-cluster")
		addrs, ok = svcMap["app-svc-cluster"]
		Expect(ok).To(BeTrue())
		Expect(len(addrs)).To(Equal(2))

		ctx := context.Background()
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		Eventually(func(g Gomega) error {
			advList, err := dynamicClient.Resource(bgpAdvertisement).Namespace(svc.Namespace).List(ctx, metav1.ListOptions{LabelSelector: "kubernetes.io/service-name=app-svc-cluster"})
			g.Expect(err).NotTo(HaveOccurred())
			g.Expect(len(advList.Items)).To(Equal(2))

			for _, adv := range advList.Items {
				peers, found, err := unstructured.NestedMap(adv.Object, "status", "peers")
				g.Expect(err).NotTo(HaveOccurred())
				g.Expect(found).To(BeTrue())

				g.Expect(len(peers)).To(Equal(8))
				for _, status := range peers {
					statusStr, ok := status.(string)
					g.Expect(ok).To(BeTrue())
					g.Expect(statusStr).To(Equal("Advertised"))
				}
			}
			return nil
		}).Should(Succeed())

	})
}

func testRestartController() {
	svcMap := make(map[string][]string)
	svcs := []string{"app-svc-cluster", "app-svc-cluster2", "app-svc-local"}

	It("should restart controller", func() {

		By("getting existing allocations")
		var svc corev1.Service
		for _, s := range svcs {
			out, err := kubectl(nil, "-n", "test", "get", "svc", s, "-ojson")
			Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &svc)
			Expect(err).NotTo(HaveOccurred())
			ips := make([]string, 0)
			for _, ingress := range svc.Status.LoadBalancer.Ingress {
				ips = append(ips, ingress.IP)
			}
			svcMap[s] = ips
		}

		By("rolling restart sart-controller")
		_, err := kubectl(nil, "-n", "kube-system", "rollout", "restart", "deploy/sart-controller")
		Expect(err).NotTo(HaveOccurred())

		By("waiting for rollout restart")
		_, err = kubectl(nil, "-n", "kube-system", "rollout", "status", "deploy/sart-controller")
		Expect(err).NotTo(HaveOccurred())

		By("checking the connectivity to app-svc-cluster")
		addrs, ok := svcMap["app-svc-cluster"]
		Expect(ok).To(BeTrue())
		Expect(len(addrs)).To(Equal(2))

		Eventually(func(g Gomega) error {
			for i := 0; i < 100; i++ {
				_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addrs[0])
				g.Expect(err).NotTo(HaveOccurred())
			}
			return nil
		}).Should(Succeed())

		Eventually(func(g Gomega) error {
			for i := 0; i < 100; i++ {
				_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addrs[1])
				g.Expect(err).NotTo(HaveOccurred())
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity to app-svc-cluster2")
		addrs, ok = svcMap["app-svc-cluster2"]
		Expect(ok).To(BeTrue())
		Expect(len(addrs)).To(Equal(1))
		addr := addrs[0]

		Eventually(func(g Gomega) error {
			for i := 0; i < 100; i++ {
				_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr)
				g.Expect(err).NotTo(HaveOccurred())
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity to app-svc-local")
		addrs, ok = svcMap["app-svc-local"]
		Expect(ok).To(BeTrue())
		Expect(len(addrs)).To(Equal(1))
		addr = addrs[0]

		Eventually(func(g Gomega) error {
			for i := 0; i < 100; i++ {
				_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr)
				g.Expect(err).NotTo(HaveOccurred())
			}
			return nil
		}).Should(Succeed())
	})

	It("should create new LoadBalancer", func() {
		_, err := kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR, "lb_another.yaml"))
		Expect(err).NotTo(HaveOccurred())

		By("checking app-cluster3 working")
		var dp appsv1.Deployment
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test", "get", "deploy", "app-cluster3", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &dp)
			g.Expect(err).NotTo(HaveOccurred())
			if dp.Status.Replicas != dp.Status.AvailableReplicas {
				return fmt.Errorf("app-cluster is not ready")
			}
			return nil
		}).Should(Succeed())

		By("checking app-cluster2 working")
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test", "get", "deploy", "app-local2", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &dp)
			g.Expect(err).NotTo(HaveOccurred())
			if dp.Status.Replicas != dp.Status.AvailableReplicas {
				return fmt.Errorf("app-local2 is not ready")
			}
			return nil
		}).Should(Succeed())

		By("checking app-svc-cluster3 gets addresses")
		var svc corev1.Service
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-cluster3", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &svc)
			g.Expect(err).NotTo(HaveOccurred())
			if len(svc.Status.LoadBalancer.Ingress) != 1 {
				return fmt.Errorf("LoadBalancer address must be 1")
			}
			return nil
		}).Should(Succeed())

		addr := svc.Status.LoadBalancer.Ingress[0].IP

		By("checking the allocated address is new one")
		for _, svcAddrs := range svcMap {
			Expect(svcAddrs).ShouldNot(ContainElement(ContainSubstring(addr)))
		}

		By("checking app-svc-local2 gets addresses")
		Eventually(func(g Gomega) error {
			out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-local2", "-ojson")
			g.Expect(err).NotTo(HaveOccurred())
			err = json.Unmarshal(out, &svc)
			g.Expect(err).NotTo(HaveOccurred())
			if len(svc.Status.LoadBalancer.Ingress) != 1 {
				return fmt.Errorf("LoadBalancer address must be 1")
			}
			return nil
		}).Should(Succeed())

		addr = svc.Status.LoadBalancer.Ingress[0].IP

		By("checking the allocated address is new one")
		for _, svcAddrs := range svcMap {
			Expect(svcAddrs).ShouldNot(ContainElement(ContainSubstring(addr)))
		}
	})
}
