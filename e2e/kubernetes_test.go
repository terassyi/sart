package e2e

import (
	"context"
	"encoding/json"
	"fmt"
	"net"
	"path"
	"strings"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
	appsv1 "k8s.io/api/apps/v1"
	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"k8s.io/apimachinery/pkg/runtime/schema"
)

const (
	MANIFEST_DIR string = "../manifests/sample"
	GROUP        string = "sart.terassyi.net"
	VERSION      string = "v1alpha2"
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

func testController() {

	It("should confirm workloads is working well", func() {
		By("checking sartd-agent working")
		var ds appsv1.DaemonSet
		Eventually(func() error {
			out, err := kubectl(nil, "-n", "kube-system", "get", "ds", "sartd-agent", "-ojson")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(out, &ds); err != nil {
				return err
			}
			if ds.Status.NumberAvailable != ds.Status.CurrentNumberScheduled {
				return fmt.Errorf("sartd-agent is not ready")
			}
			return nil
		}).Should(Succeed())

		By("checking sartd-bgp working")
		Eventually(func() error {
			out, err := kubectl(nil, "-n", "kube-system", "get", "ds", "sartd-bgp", "-ojson")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(out, &ds); err != nil {
				return err
			}
			if ds.Status.NumberAvailable != ds.Status.CurrentNumberScheduled {
				return fmt.Errorf("sartd-agent is not ready")
			}
			return nil
		}).Should(Succeed())

		By("checking sart-controller working")
		var dp appsv1.Deployment
		Eventually(func() error {
			out, err := kubectl(nil, "-n", "kube-system", "get", "deploy", "sart-controller", "-ojson")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(out, &dp); err != nil {
				return err
			}
			if *dp.Spec.Replicas != dp.Status.AvailableReplicas {
				return fmt.Errorf("sart-controller is not ready")
			}
			return nil
		}).Should(Succeed())
	})

	It("should install BGP related resource", func() {
		By("applying sample resource")
		_, err := kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR, "peer_template.yaml"))
		Expect(err).NotTo(HaveOccurred())

		_, err = kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR, "cluster_bgp.yaml"))
		Expect(err).NotTo(HaveOccurred())

		_, err = kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR, "bgp_peer.yaml"))
		Expect(err).NotTo(HaveOccurred())

		_, err = kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR, "lb_address_pool.yaml"))
		Expect(err).NotTo(HaveOccurred())

		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		ctx := context.Background()

		By("checking ClusterBGP")
		Eventually(func() error {
			list, err := dynamicClient.Resource(clusterBGP).List(ctx, metav1.ListOptions{})
			if err != nil {
				return err
			}
			if len(list.Items) != 2 {
				return fmt.Errorf("expect 2 resource")
			}
			return nil
		}).Should(Succeed())

		By("checking NodeBGP with label(bgp=a)")
		Eventually(func() error {
			list, err := dynamicClient.Resource(nodeBGP).List(ctx, metav1.ListOptions{LabelSelector: "bgp=a"})
			if err != nil {
				return err
			}
			if len(list.Items) != 3 {
				return fmt.Errorf("expect 3 resource")
			}
			return nil
		}).Should(Succeed())

		By("checking NodeBGP with label(bgp=b)")
		Eventually(func() error {
			list, err := dynamicClient.Resource(nodeBGP).List(ctx, metav1.ListOptions{LabelSelector: "bgp=b"})
			if err != nil {
				return err
			}
			if len(list.Items) != 1 {
				return fmt.Errorf("expect 1 resource")
			}
			return nil
		}).Should(Succeed())

		By("checking BGPPeer")
		Eventually(func() error {
			list, err := dynamicClient.Resource(bgpPeer).List(ctx, metav1.ListOptions{})
			if err != nil {
				return err
			}
			if len(list.Items) != 4 {
				return fmt.Errorf("expect 4 resource")
			}
			return nil
		}).Should(Succeed())
	})

	It("should confirm that all bgp peer are established", func() {
		By("preparing dynamic client")
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		ctx := context.Background()

		By("checking BGPPeer status is Established state")
		Eventually(func() error {
			list, err := dynamicClient.Resource(bgpPeer).List(ctx, metav1.ListOptions{})
			if err != nil {
				return err
			}
			for _, p := range list.Items {
				res, found, err := unstructured.NestedSlice(p.Object, "status", "conditions")
				if err != nil {
					return err
				}
				if !found {
					return fmt.Errorf("BGPPeer is not found")
				}
				nowCond := res[len(res)-1].(map[string]any)
				nowStatus, ok := nowCond["status"]
				if !ok {
					return fmt.Errorf("BGPPeer.status.condition[len(cond)-1].status is not found")
				}
				if nowStatus.(string) != "Established" {
					return fmt.Errorf("%s is not Established, actual state is %s", p.GetName(), nowStatus.(string))
				}
			}
			return nil
		}).WithTimeout(5 * time.Minute).Should(Succeed())

		By("checking sartd-bgp's peer state")
		Eventually(func() error {
			list, err := dynamicClient.Resource(bgpPeer).List(ctx, metav1.ListOptions{})
			if err != nil {
				return err
			}
			for _, p := range list.Items {
				node, found, err := unstructured.NestedString(p.Object, "spec", "nodeBGPRef")
				if err != nil {
					return err
				}
				if !found {
					return fmt.Errorf("BGPPeer.spec.nodeBGPRef is not found")
				}
				peerAddr, found, err := unstructured.NestedString(p.Object, "spec", "addr")
				if err != nil {
					return err
				}
				if !found {
					return fmt.Errorf("BGPPeer.spec.addr is not found")
				}
				out, err := kubectl(nil, "-n", "kube-system", "get", "pod", "-l", "app=sartd", "--field-selector", fmt.Sprintf("spec.nodeName=%s", node), "-ojson")
				if err != nil {
					fmt.Println("oops")
					return err
				}
				var podList corev1.PodList
				if err := json.Unmarshal(out, &podList); err != nil {
					return err
				}
				if len(podList.Items) != 1 {
					return fmt.Errorf("only one pod is expected")
				}
				pod := podList.Items[0]
				peerBytes, err := kubectlExec(pod.Name, pod.Namespace, nil, "sart", "bgp", "neighbor", "get", peerAddr, "-ojson")
				if err != nil {
					return err
				}
				var peer sartPeerInfo
				if err := json.Unmarshal(peerBytes, &peer); err != nil {
					return err
				}
				if peer.State != "Established" {
					return fmt.Errorf("%s's state is not Established, but %s", p.GetName(), peer.State)
				}
			}
			return nil
		}).Should(Succeed())
	})

	It("should create AddressBlocks", func() {
		By("getting AddressPool")
		ctx := context.Background()
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		addressPoolList, err := dynamicClient.Resource(addressPool).List(ctx, metav1.ListOptions{})
		Expect(err).NotTo(HaveOccurred())
		Expect(len(addressPoolList.Items)).To(Equal(2))

		By("getting AddressBlocks")
		for _, pool := range addressPoolList.Items {
			cidr, found, err := unstructured.NestedString(pool.Object, "spec", "cidr")
			Expect(err).NotTo(HaveOccurred())
			Expect(found).To(BeTrue())

			addressBlock, err := dynamicClient.Resource(addressBlock).Get(ctx, pool.GetName(), metav1.GetOptions{})
			Expect(err).NotTo(HaveOccurred())
			blockCidr, found, err := unstructured.NestedString(addressBlock.Object, "spec", "cidr")
			Expect(err).NotTo(HaveOccurred())
			Expect(found).To(BeTrue())

			Expect(blockCidr).To(Equal(cidr))
		}
	})

	It("should apply LoadBalancer and related resources", func() {
		By("applying sample resource")
		_, err := kubectl(nil, "apply", "-f", path.Join(MANIFEST_DIR, "lb.yaml"))
		Expect(err).NotTo(HaveOccurred())

		By("checking app-cluster working")
		var dp appsv1.Deployment
		Eventually(func() error {
			out, err := kubectl(nil, "-n", "test", "get", "deploy", "app-cluster", "-ojson")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(out, &dp); err != nil {
				return err
			}
			if *dp.Spec.Replicas != dp.Status.AvailableReplicas {
				return fmt.Errorf("app-cluster is not ready")
			}
			return nil
		}).Should(Succeed())

		By("checking app-cluster2 working")
		Eventually(func() error {
			out, err := kubectl(nil, "-n", "test", "get", "deploy", "app-cluster2", "-ojson")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(out, &dp); err != nil {
				return err
			}
			if *dp.Spec.Replicas != dp.Status.AvailableReplicas {
				return fmt.Errorf("app-cluster2 is not ready")
			}
			return nil
		}).Should(Succeed())

		By("checking app-local working")
		Eventually(func() error {
			out, err := kubectl(nil, "-n", "test", "get", "deploy", "app-local", "-ojson")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(out, &dp); err != nil {
				return err
			}
			if *dp.Spec.Replicas != dp.Status.AvailableReplicas {
				return fmt.Errorf("app-local is not ready")
			}
			return nil
		}).Should(Succeed())
	})

	It("should communicate via app-svc-cluster", func() {

		By("checking app-svc-cluster gets addresses")
		var svc corev1.Service
		Eventually(func() error {
			out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-cluster", "-ojson")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(out, &svc); err != nil {
				return err
			}
			if len(svc.Status.LoadBalancer.Ingress) != 1 {
				return fmt.Errorf("LoadBalancer address must be 1")
			}
			return nil
		}).Should(Succeed())

		addr := svc.Status.LoadBalancer.Ingress[0].IP

		By("checking BGPAdvertisement")
		ctx := context.Background()
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())
		Eventually(func() error {
			advList, err := dynamicClient.Resource(bgpAdvertisement).Namespace(svc.Namespace).List(ctx, metav1.ListOptions{LabelSelector: fmt.Sprintf("kubernetes.io/service-name=%s", svc.Name)})
			if err != nil {
				return err
			}
			peers, found, err := unstructured.NestedMap(advList.Items[0].Object, "status", "peers")
			if err != nil {
				return err
			}
			if !found {
				return fmt.Errorf("BGPAdvertisement.status.peers is not found")
			}
			if len(peers) != 4 {
				return fmt.Errorf("advertisement for app-svc-cluster is expected to have 4 peers as targets")
			}
			for n, status := range peers {
				if status.(string) != "Advertised" {
					return fmt.Errorf("%s is not Advertised, actual is %s", n, status.(string))
				}
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity to app-svc-cluster")
		Eventually(func() error {
			_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr)
			if err != nil {
				return err
			}
			return nil
		}).Should(Succeed())
	})

	It("should communicate via app-svc-local", func() {
		By("checking app-svc-local gets addresses")
		var svc corev1.Service
		Eventually(func() error {
			out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-local", "-ojson")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(out, &svc); err != nil {
				return err
			}
			if len(svc.Status.LoadBalancer.Ingress) != 1 {
				return fmt.Errorf("LoadBalancer address must be 1")
			}
			return nil
		}).Should(Succeed())

		addr := svc.Status.LoadBalancer.Ingress[0].IP

		By("checking app-local working")
		var dp appsv1.Deployment
		Eventually(func() error {
			out, err := kubectl(nil, "-n", "test", "get", "deploy", "app-local", "-ojson")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(out, &dp); err != nil {
				return err
			}
			if *dp.Spec.Replicas != dp.Status.AvailableReplicas {
				return fmt.Errorf("app-local is not ready")
			}
			return nil
		}).Should(Succeed())

		By("checking BGPAdvertisement")
		ctx := context.Background()
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())
		Eventually(func() error {
			advList, err := dynamicClient.Resource(bgpAdvertisement).Namespace(svc.Namespace).List(ctx, metav1.ListOptions{LabelSelector: fmt.Sprintf("kubernetes.io/service-name=%s", svc.Name)})
			if err != nil {
				return err
			}
			peers, found, err := unstructured.NestedMap(advList.Items[0].Object, "status", "peers")
			if err != nil {
				return err
			}
			if !found {
				return fmt.Errorf("BGPAdvertisement.status.peers is not found")
			}
			if len(peers) != int(*dp.Spec.Replicas) {
				return fmt.Errorf("advertisement for app-svc-local is expected to have 4 peers as targets")
			}
			for n, status := range peers {
				if status.(string) != "Advertised" {
					return fmt.Errorf("%s is not Advertised, actual is %s", n, status.(string))
				}
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity to app-svc-local")
		Eventually(func() error {
			_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr)
			if err != nil {
				return err
			}
			return nil
		}).Should(Succeed())
	})

	It("should communicate via app-svc-cluster2", func() {

		By("checking app-svc-cluster2 gets addresses")
		var svc corev1.Service
		Eventually(func() error {
			out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-cluster2", "-ojson")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(out, &svc); err != nil {
				return err
			}
			if len(svc.Status.LoadBalancer.Ingress) != 1 {
				return fmt.Errorf("LoadBalancer address must be 1")
			}
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
		Eventually(func() error {
			advList, err := dynamicClient.Resource(bgpAdvertisement).Namespace(svc.Namespace).List(ctx, metav1.ListOptions{LabelSelector: fmt.Sprintf("kubernetes.io/service-name=%s", svc.Name)})
			if err != nil {
				return err
			}
			peers, found, err := unstructured.NestedMap(advList.Items[0].Object, "status", "peers")
			if err != nil {
				return err
			}
			if !found {
				return fmt.Errorf("BGPAdvertisement.status.peers is not found")
			}
			if len(peers) != 4 {
				return fmt.Errorf("advertisement for app-svc-cluster2 is expected to have 4 peers as targets")
			}
			for n, status := range peers {
				if status.(string) != "Advertised" {
					return fmt.Errorf("%s is not Advertised, actual is %s", n, status.(string))
				}
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity to app-svc-cluster2")
		Eventually(func() error {
			_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr)
			if err != nil {
				return err
			}
			return nil
		}).Should(Succeed())
	})

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
		Eventually(func() error {
			out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-cluster", "-ojson")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(out, &svc); err != nil {
				return nil
			}
			if len(svc.Status.LoadBalancer.Ingress) != 1 {
				return fmt.Errorf("svc.Status.LoadBalancer.Ingress must be 1")
			}

			newAddr = svc.Status.LoadBalancer.Ingress[0].IP
			if oldAddr == newAddr {
				return fmt.Errorf("address must changed")
			}
			return nil
		}).Should(Succeed())

		By("getting dynamic client")
		ctx := context.Background()
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())

		By("getting advertisement")
		Eventually(func() error {
			advList, err := dynamicClient.Resource(bgpAdvertisement).Namespace(svc.Namespace).List(ctx, metav1.ListOptions{LabelSelector: fmt.Sprintf("kubernetes.io/service-name=%s", svc.Name)})
			if err != nil {
				return err
			}
			if len(advList.Items) != 1 {
				return fmt.Errorf("advertisement must be 1, but actual is %d", len(advList.Items))
			}
			cidr, found, err := unstructured.NestedString(advList.Items[0].Object, "spec", "cidr")
			if err != nil {
				return err
			}
			if !found {
				return fmt.Errorf("BGPAdvertisement.status.peers is not found")
			}
			if fmt.Sprintf("%s/32", newAddr) != cidr {
				return fmt.Errorf("expected %s, actual %s", fmt.Sprintf("%s/32", newAddr), cidr)
			}
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
		Eventually(func() error {
			_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", newAddr)
			if err != nil {
				return err
			}
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
		Eventually(func() error {
			out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-cluster", "-ojson")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(out, &svc); err != nil {
				return nil
			}
			if len(svc.Status.LoadBalancer.Ingress) != 2 {
				return fmt.Errorf("svc.Status.LoadBalancer.Ingress must be 2")
			}
			for _, ingress := range svc.Status.LoadBalancer.Ingress {
				addrs = append(addrs, ingress.IP)
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity for both addresses")
		Eventually(func() error {
			_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addrs[0])
			if err != nil {
				return err
			}
			return nil
		}).Should(Succeed())

		Eventually(func() error {
			_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addrs[1])
			if err != nil {
				return err
			}
			return nil
		}).Should(Succeed())
	})

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
		Eventually(func() error {
			advList, err := dynamicClient.Resource(bgpAdvertisement).Namespace(svc.Namespace).List(ctx, metav1.ListOptions{LabelSelector: fmt.Sprintf("kubernetes.io/service-name=%s", svc.Name)})
			if err != nil {
				return err
			}
			peers, found, err := unstructured.NestedMap(advList.Items[0].Object, "status", "peers")
			if err != nil {
				return err
			}
			if !found {
				return fmt.Errorf("BGPAdvertisement.status.peers is not found")
			}
			if len(peers) != 4 {
				return fmt.Errorf("advertisement for app-svc-cluster2 is expected to have 4 peers as targets")
			}
			for n, status := range peers {
				if status.(string) != "Advertised" {
					return fmt.Errorf("%s is not Advertised, actual is %s", n, status.(string))
				}
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
		Eventually(func() error {
			out, err := kubectl(nil, "-n", svc.Namespace, "get", "pod", "-l", fmt.Sprintf("app=%s", dpName), "-ojson")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(out, &podList); err != nil {
				return err
			}
			if len(podList.Items) != int(*dp.Spec.Replicas) {
				return fmt.Errorf("replicas mismatch")
			}
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
		Eventually(func() error {
			advList, err := dynamicClient.Resource(bgpAdvertisement).Namespace(svc.Namespace).List(ctx, metav1.ListOptions{LabelSelector: fmt.Sprintf("kubernetes.io/service-name=%s", svc.Name)})
			if err != nil {
				return err
			}
			targets, found, err := unstructured.NestedMap(advList.Items[0].Object, "status", "peers")
			if err != nil {
				return err
			}
			if !found {
				return fmt.Errorf("BGPAdvertisement.status.peers is not found")
			}
			if len(targets) != int(*dp.Spec.Replicas) {
				return fmt.Errorf("advertisement for app-svc-cluster2 is expected to have %d peers as targets", *dp.Spec.Replicas)
			}
			for _, p := range peers {
				status, ok := targets[p]
				if !ok {
					return fmt.Errorf("%s must be advertised", p)
				}
				if status.(string) != "Advertised" {
					return fmt.Errorf("%s is not Advertised, actual is %s", p, status.(string))
				}
			}
			return nil
		}).Should(Succeed())
	})

	It("should handle to rollout restart with externalTrafficPolicy=Cluster", func() {
		By("checking app-svc-cluster gets addresses")
		var svc corev1.Service
		Eventually(func() error {
			out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-cluster", "-ojson")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(out, &svc); err != nil {
				return err
			}
			if len(svc.Status.LoadBalancer.Ingress) == 0 {
				return fmt.Errorf("LoadBalancer address must exist")
			}
			return nil
		}).Should(Succeed())

		addr := svc.Status.LoadBalancer.Ingress[0].IP

		By("doing rollout restart for app-cluster")
		_, err := kubectl(nil, "-n", svc.Namespace, "rollout", "restart", "deploy/app-cluster")
		Expect(err).NotTo(HaveOccurred())

		By("waiting rollout restart completed")
		Eventually(func() error {
			out, err := kubectl(nil, "-n", svc.Namespace, "rollout", "status", "deploy/app-cluster")
			if err != nil {
				return err
			}
			if !strings.Contains(string(out), "successfully") {
				return fmt.Errorf("restarting yet")
			}
			return nil
		}).Should(Succeed())

		By("checking BGPAdvertisement")
		ctx := context.Background()
		dynamicClient, err := getDynamicClient()
		Expect(err).NotTo(HaveOccurred())
		Eventually(func() error {
			advList, err := dynamicClient.Resource(bgpAdvertisement).Namespace(svc.Namespace).List(ctx, metav1.ListOptions{LabelSelector: fmt.Sprintf("kubernetes.io/service-name=%s", svc.Name)})
			if err != nil {
				return err
			}
			peers, found, err := unstructured.NestedMap(advList.Items[0].Object, "status", "peers")
			if err != nil {
				return err
			}
			if !found {
				return fmt.Errorf("BGPAdvertisement.status.peers is not found")
			}
			if len(peers) != 4 {
				return fmt.Errorf("advertisement for app-svc-cluster is expected to have 4 peers as targets")
			}
			for n, status := range peers {
				if status.(string) != "Advertised" {
					return fmt.Errorf("%s is not Advertised, actual is %s", n, status.(string))
				}
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity to app-svc-cluster")
		Eventually(func() error {
			_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr)
			if err != nil {
				return err
			}
			return nil
		}).Should(Succeed())
	})

	It("should handle to rollout restart with externalTrafficPolicy=Local", func() {
		By("checking app-svc-local gets addresses")
		var svc corev1.Service
		Eventually(func() error {
			out, err := kubectl(nil, "-n", "test", "get", "svc", "app-svc-local", "-ojson")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(out, &svc); err != nil {
				return err
			}
			if len(svc.Status.LoadBalancer.Ingress) == 0 {
				return fmt.Errorf("LoadBalancer address must exist")
			}
			return nil
		}).Should(Succeed())

		addr := svc.Status.LoadBalancer.Ingress[0].IP

		By("doing rollout restart for app-local")
		_, err := kubectl(nil, "-n", svc.Namespace, "rollout", "restart", "deploy/app-local")
		Expect(err).NotTo(HaveOccurred())

		By("waiting rollout restart completed")
		Eventually(func() error {
			out, err := kubectl(nil, "-n", svc.Namespace, "rollout", "status", "deploy/app-local")
			if err != nil {
				return err
			}
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
		Eventually(func() error {
			out, err := kubectl(nil, "-n", svc.Namespace, "get", "pod", "-l", fmt.Sprintf("app=%s", dpName), "-ojson")
			if err != nil {
				return err
			}
			if err := json.Unmarshal(out, &podList); err != nil {
				return err
			}
			if len(podList.Items) != int(*dp.Spec.Replicas) {
				return fmt.Errorf("replicas mismatch")
			}
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
		Eventually(func() error {
			advList, err := dynamicClient.Resource(bgpAdvertisement).Namespace(svc.Namespace).List(ctx, metav1.ListOptions{LabelSelector: fmt.Sprintf("kubernetes.io/service-name=%s", svc.Name)})
			if err != nil {
				return err
			}
			targets, found, err := unstructured.NestedMap(advList.Items[0].Object, "status", "peers")
			if err != nil {
				return err
			}
			if !found {
				return fmt.Errorf("BGPAdvertisement.status.peers is not found")
			}
			if len(targets) != int(*dp.Spec.Replicas) {
				return fmt.Errorf("advertisement for app-svc-cluster2 is expected to have %d peers as targets", *dp.Spec.Replicas)
			}
			for _, p := range peers {
				status, ok := targets[p]
				if !ok {
					return fmt.Errorf("%s must be advertised", p)
				}
				if status.(string) != "Advertised" {
					return fmt.Errorf("%s is not Advertised, actual is %s", p, status.(string))
				}
			}
			return nil
		}).Should(Succeed())

		By("checking the connectivity to app-svc-local")
		Eventually(func() error {
			_, err := execInContainer("clab-sart-client0", nil, "curl", "-m", "1", addr)
			if err != nil {
				return err
			}
			return nil
		}).Should(Succeed())
	})
}
