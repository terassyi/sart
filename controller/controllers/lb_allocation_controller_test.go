package controllers

import (
	"context"
	"fmt"
	"net/netip"
	"reflect"
	"sort"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
	sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	"github.com/terassyi/sart/controller/pkg/allocator"
	"github.com/terassyi/sart/controller/pkg/constants"
	"github.com/terassyi/sart/controller/pkg/speaker"
	v1 "k8s.io/api/core/v1"
	corev1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/types"
	"k8s.io/apimachinery/pkg/util/intstr"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
)

var _ = Describe("handle BGPPeer", func() {
	ctx := context.Background()
	var cancel context.CancelFunc

	allocatorMap := make(map[string]map[string]allocator.Allocator)
	allocMap := make(map[string][]alloc)

	BeforeEach(func() {
		mgr, err := ctrl.NewManager(cfg, ctrl.Options{
			Scheme:             scheme,
			LeaderElection:     false,
			MetricsBindAddress: "0",
		})
		Expect(err).NotTo(HaveOccurred())

		lbAllocationReconciler := &LBAllocationReconciler{
			Client:     k8sClient,
			Allocators: allocatorMap,
			allocMap:   allocMap,
			recover:    false,
		}
		err = lbAllocationReconciler.SetupWithManager(mgr)
		Expect(err).NotTo(HaveOccurred())

		ctx, cancel = context.WithCancel(context.TODO())
		go func() {
			err := mgr.Start(ctx)
			if err != nil {
				panic(err)
			}
		}()
		time.Sleep(100 * time.Millisecond)
		// create BGPPeer
		for _, p := range peers {
			pp := p.DeepCopy()
			err = k8sClient.Create(ctx, pp)
			Expect(err).NotTo(HaveOccurred())
		}
		time.Sleep(100 * time.Millisecond)
		for _, p := range peers {
			pp := &sartv1alpha1.BGPPeer{}
			err = k8sClient.Get(ctx, types.NamespacedName{Namespace: p.Namespace, Name: p.Name}, pp)
			Expect(err).NotTo(HaveOccurred())

			pp.Status = sartv1alpha1.BGPPeerStatusEstablished
			err = k8sClient.Status().Update(ctx, pp)
			Expect(err).NotTo(HaveOccurred())
		}

		// create mock speaker entries
		n1 := speaker.New(speaker.SpeakerTypeMock, "10.0.0.1", 5000)
		n2 := speaker.New(speaker.SpeakerTypeMock, "10.0.0.2", 5000)
		n3 := speaker.New(speaker.SpeakerTypeMock, "10.0.0.3", 5000)
		// create mock peer entries
		err = n1.AddPeer(ctx, speaker.PeerInfo{
			LocalAsn:      65000,
			LocalRouterId: "10.0.0.1",
			PeerAsn:       65000,
			PeerRouterId:  "10.0.0.100",
			Protocol:      "ipv4",
			State:         speaker.PeerStateEstablished,
		})
		Expect(err).NotTo(HaveOccurred())
		err = n2.AddPeer(ctx, speaker.PeerInfo{
			LocalAsn:      65000,
			LocalRouterId: "10.0.0.2",
			PeerAsn:       65000,
			PeerRouterId:  "10.0.0.100",
			Protocol:      "ipv4",
			State:         speaker.PeerStateEstablished,
		})
		Expect(err).NotTo(HaveOccurred())
		err = n3.AddPeer(ctx, speaker.PeerInfo{
			LocalAsn:      65000,
			LocalRouterId: "10.0.0.3",
			PeerAsn:       65000,
			PeerRouterId:  "10.0.0.100",
			Protocol:      "ipv4",
			State:         speaker.PeerStateEstablished,
		})
		Expect(err).NotTo(HaveOccurred())
		time.Sleep(100 * time.Millisecond)
	})

	AfterEach(func() {
		speaker.ClearMockSpeakerStore()
		allocatorMap = make(map[string]map[string]allocator.Allocator)
		allocMap = make(map[string][]alloc)
		for _, p := range peers {
			err := k8sClient.Delete(ctx, p)
			Expect(err).NotTo(HaveOccurred())
		}

		cancel()
		time.Sleep(10 * time.Millisecond)
	})

	It("should handle AddressPool", func() {
		By("creating an address pool")
		pool := &sartv1alpha1.AddressPool{
			ObjectMeta: corev1.ObjectMeta{
				Name: "test",
			},
			Spec: sartv1alpha1.AddressPoolSpec{
				Type: "lb",
				Cidrs: []sartv1alpha1.Cdir{
					{
						Protocol: "ipv4",
						Prefix:   "10.0.10.0/24",
					},
				},
				Disable: false,
			},
		}
		err := k8sClient.Create(ctx, pool)
		Expect(err).NotTo(HaveOccurred())

		pool = &sartv1alpha1.AddressPool{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, client.ObjectKey{Name: "test"}, pool); err != nil {
				return err
			}
			return nil
		}).Should(Succeed())

		By("existing an allocator")
		cidr, err := netip.ParsePrefix("10.0.10.0/24")
		Expect(err).NotTo(HaveOccurred())
		Eventually(func() error {
			allocators, ok := allocatorMap["test"]
			if !ok {
				return fmt.Errorf("should get allocator")
			}
			ipv4Allocator, ok := allocators["ipv4"]
			if !ok {
				return fmt.Errorf("should get ipv4")
			}
			if !reflect.DeepEqual(ipv4Allocator, allocator.New(&cidr)) {
				return fmt.Errorf("should equal")
			}
			return nil
		}).WithPolling(1 * time.Second).Should(Succeed())

		By("deleting AddressPool")
		err = k8sClient.Delete(ctx, pool)
		Expect(err).NotTo(HaveOccurred())

		By("not existing an allocator")
		Eventually(func() error {
			_, ok := allocatorMap["test"]
			if ok {
				return fmt.Errorf("should not get allocator")
			}
			return nil
		}).WithPolling(1 * time.Second).Should(Succeed())
	})

	It("should handle LoadBalancer externalTrafficPolicy=Cluster", func() {
		By("creating an address pool")
		pool := &sartv1alpha1.AddressPool{
			ObjectMeta: corev1.ObjectMeta{
				Name: "default",
			},
			Spec: sartv1alpha1.AddressPoolSpec{
				Type: "lb",
				Cidrs: []sartv1alpha1.Cdir{
					{
						Protocol: "ipv4",
						Prefix:   "10.0.10.0/24",
					},
				},
				Disable: false,
			},
		}
		err := k8sClient.Create(ctx, pool)
		Expect(err).NotTo(HaveOccurred())

		pool = &sartv1alpha1.AddressPool{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, client.ObjectKey{Name: "default"}, pool); err != nil {
				return err
			}
			return nil
		}).Should(Succeed())

		By("creating LoadBalancer")
		svc := &v1.Service{
			ObjectMeta: corev1.ObjectMeta{
				Name:      "lb-cluster1",
				Namespace: "default",
				Annotations: map[string]string{
					constants.AnnotationAddressPool: "default",
				},
			},
			Spec: v1.ServiceSpec{
				Ports: []v1.ServicePort{
					{
						Name:       "http",
						Port:       80,
						TargetPort: intstr.IntOrString{IntVal: 80},
					},
				},
				Selector: map[string]string{
					"app": "app1",
				},
				Type:                  v1.ServiceTypeLoadBalancer,
				ExternalTrafficPolicy: v1.ServiceExternalTrafficPolicyTypeCluster,
			},
		}
		err = k8sClient.Create(ctx, svc)
		Expect(err).NotTo(HaveOccurred())

		By("getting service")
		svc = &v1.Service{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: "lb-cluster1"}, svc); err != nil {
				return err
			}
			pool, ok := svc.Annotations[constants.AnnotationAllocatedFromPool]
			if !ok {
				return fmt.Errorf("not allocated")
			}
			if pool != "default" {
				return fmt.Errorf("pool name is not matched")
			}
			return nil
		}).Should(Succeed())

		By("existing an advertisement")
		adv := &sartv1alpha1.BGPAdvertisement{}
		nodes := []string{"node1", "node2", "node3"}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: svcToAdvertisementName(svc, "ipv4")}, adv); err != nil {
				return err
			}
			if adv.Status.Condition != sartv1alpha1.BGPAdvertisementConditionAdvertising {
				return fmt.Errorf("advertisement status is not advertising: got %v", adv.Status.Condition)
			}
			sort.Strings(adv.Spec.Nodes)
			if !reflect.DeepEqual(adv.Spec.Nodes, nodes) {
				return fmt.Errorf("want: %v, got: %v", nodes, adv.Spec.Nodes)
			}
			return nil
		}).Should(Succeed())

		By("checking the allocator and allocation info")
		a, ok := allocMap[types.NamespacedName{Namespace: svc.Namespace, Name: svc.Name}.String()]
		Expect(ok).To(BeTrue())
		Expect(len(a)).To(Equal(1))
		allocInfo := a[0]

		protocolAllocator, ok := allocatorMap["default"]
		Expect(ok).To(BeTrue())
		allocator, ok := protocolAllocator["ipv4"]
		Expect(ok).To(BeTrue())
		Expect(allocator.IsAllocated(allocInfo.addr)).To(BeTrue())

		By("changing the advertisement status to advertised")
		adv.Status.Condition = sartv1alpha1.BGPAdvertisementConditionAdvertised
		err = k8sClient.Status().Update(ctx, adv)
		Expect(err).NotTo(HaveOccurred())

		By("assigning an address to Service")
		svc = &v1.Service{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: "lb-cluster1"}, svc); err != nil {
				return err
			}

			return nil
		}).Should(Succeed())

	})

	It("should handle LoadBalancer externalTrafficPolicy=Local", func() {

	})

	It("should handle LoadBalancer when deleting", func() {

	})

	It("should handle LoadBalancer externalTrafficPolicy=Local when endpoints are changed", func() {

	})

})
