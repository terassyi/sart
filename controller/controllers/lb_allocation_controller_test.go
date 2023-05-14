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
	discoveryv1 "k8s.io/api/discovery/v1"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	corev1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/types"
	"k8s.io/apimachinery/pkg/util/intstr"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
)

var (
	endpointReady    = true
	endpointNotReady = false
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
			Scheme:     scheme,
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

		defer func() {
			if err := k8sClient.Delete(ctx, pool); err != nil {
				Fail(err.Error())
			}
		}()

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

		By("creating EndpointSlice")
		nodes := []string{"node1", "node2"}
		eps := &discoveryv1.EndpointSlice{
			ObjectMeta: corev1.ObjectMeta{
				Name:      "lb-cluster1",
				Namespace: "default",
				Labels: map[string]string{
					constants.KubernetesServiceNameLabel: "lb-cluster1",
				},
				Annotations: map[string]string{},
			},
			AddressType: "IPv4",
			Endpoints: []discoveryv1.Endpoint{
				{
					Addresses: []string{"10.100.0.1"},
					NodeName:  &nodes[0],
					Conditions: discoveryv1.EndpointConditions{
						Ready: &endpointReady,
					},
				},
				{
					Addresses: []string{"10.100.0.2"},
					NodeName:  &nodes[1],
					Conditions: discoveryv1.EndpointConditions{
						Ready: &endpointReady,
					},
				},
			},
		}
		err = k8sClient.Create(ctx, eps)
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
		advertisedNodes := []string{"node1", "node2", "node3"}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: svcToAdvertisementName(svc, "ipv4")}, adv); err != nil {
				return err
			}
			if adv.Status.Condition != sartv1alpha1.BGPAdvertisementConditionUnavailable {
				return fmt.Errorf("advertisement status is not advertising: got %v", adv.Status.Condition)
			}
			sort.Strings(adv.Spec.Nodes)
			if !reflect.DeepEqual(adv.Spec.Nodes, advertisedNodes) {
				return fmt.Errorf("want: %v, got: %v", advertisedNodes, adv.Spec.Nodes)
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

		By("changing the advertisement status to available")
		adv.Status.Condition = sartv1alpha1.BGPAdvertisementConditionAvailable
		err = k8sClient.Status().Update(ctx, adv)
		Expect(err).NotTo(HaveOccurred())

		By("assigning an address to Service")
		svc = &v1.Service{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: "lb-cluster1"}, svc); err != nil {
				return err
			}
			if len(svc.Status.Conditions) < 1 {
				return fmt.Errorf("expected at least one condition")
			}
			assignedCondition := svc.Status.Conditions[len(svc.Status.Conditions)-1]
			if assignedCondition.Type != ServiceConditionTypeAdvertised {
				return fmt.Errorf("want: %v, got: %v", corev1.ConditionStatus(ServiceConditionReasonAdvertised), assignedCondition.Reason)
			}
			if len(svc.Status.LoadBalancer.Ingress) == 0 {
				return fmt.Errorf("expected at least one ingress")
			}
			lbStatus := svc.Status.LoadBalancer.Ingress[0]
			if lbStatus.IP != allocInfo.addr.String() {
				return fmt.Errorf("want: %s, got: %s", allocInfo.addr, lbStatus.IP)
			}
			return nil
		}).Should(Succeed())

	})

	It("should handle LoadBalancer externalTrafficPolicy=Local", func() {
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

		defer func() {
			if err := k8sClient.Delete(ctx, pool); err != nil {
				Fail(err.Error())
			}
		}()

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
				Name:      "lb-local1",
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
					"app": "app2",
				},
				Type:                  v1.ServiceTypeLoadBalancer,
				ExternalTrafficPolicy: v1.ServiceExternalTrafficPolicyTypeLocal,
			},
		}
		err = k8sClient.Create(ctx, svc)
		Expect(err).NotTo(HaveOccurred())

		By("creating EndpointSlice")
		nodes := []string{"node1", "node2"}
		eps := &discoveryv1.EndpointSlice{
			ObjectMeta: corev1.ObjectMeta{
				Name:      "lb-local1",
				Namespace: "default",
				Labels: map[string]string{
					constants.KubernetesServiceNameLabel: "lb-local1",
				},
				Annotations: map[string]string{},
			},
			AddressType: "IPv4",
			Endpoints: []discoveryv1.Endpoint{
				{
					Addresses: []string{"10.100.0.1"},
					NodeName:  &nodes[0],
					Conditions: discoveryv1.EndpointConditions{
						Ready: &endpointReady,
					},
				},
				{
					Addresses: []string{"10.100.0.2"},
					NodeName:  &nodes[1],
					Conditions: discoveryv1.EndpointConditions{
						Ready: &endpointReady,
					},
				},
			},
		}
		err = k8sClient.Create(ctx, eps)
		Expect(err).NotTo(HaveOccurred())

		By("getting service")
		svc = &v1.Service{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: "lb-local1"}, svc); err != nil {
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
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: svcToAdvertisementName(svc, "ipv4")}, adv); err != nil {
				return err
			}
			if adv.Status.Condition != sartv1alpha1.BGPAdvertisementConditionUnavailable {
				return fmt.Errorf("advertisement status is not unavailable : got %v", adv.Status.Condition)
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
		adv.Status.Condition = sartv1alpha1.BGPAdvertisementConditionAvailable
		err = k8sClient.Status().Update(ctx, adv)
		Expect(err).NotTo(HaveOccurred())

		By("assigning an address to Service")
		svc = &v1.Service{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: "lb-local1"}, svc); err != nil {
				return err
			}
			if len(svc.Status.Conditions) < 1 {
				return fmt.Errorf("expected at least one condition")
			}
			assignedCondition := svc.Status.Conditions[len(svc.Status.Conditions)-1]
			if assignedCondition.Type != ServiceConditionTypeAdvertised {
				return fmt.Errorf("want: %v, got: %v", corev1.ConditionStatus(ServiceConditionReasonAdvertised), assignedCondition.Reason)
			}
			if len(svc.Status.LoadBalancer.Ingress) == 0 {
				return fmt.Errorf("expected at least one ingress")
			}
			lbStatus := svc.Status.LoadBalancer.Ingress[0]
			if lbStatus.IP != allocInfo.addr.String() {
				return fmt.Errorf("want: %s, got: %s", allocInfo.addr, lbStatus.IP)
			}
			return nil
		}).Should(Succeed())
	})

	It("should handle LoadBalancer with specifying LB address", func() {
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

		defer func() {
			if err := k8sClient.Delete(ctx, pool); err != nil {
				Fail(err.Error())
			}
		}()

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
				Name:      "lb-cluster2",
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
				LoadBalancerIP:        "10.0.10.12",
			},
		}
		err = k8sClient.Create(ctx, svc)
		Expect(err).NotTo(HaveOccurred())

		By("creating EndpointSlice")
		endpointNodes := []string{"node2"}
		eps := &discoveryv1.EndpointSlice{
			ObjectMeta: corev1.ObjectMeta{
				Name:      "lb-cluster2",
				Namespace: "default",
				Labels: map[string]string{
					constants.KubernetesServiceNameLabel: "lb-cluster2",
				},
				Annotations: map[string]string{},
			},
			AddressType: "IPv4",
			Endpoints: []discoveryv1.Endpoint{
				{
					Addresses: []string{"10.100.0.2"},
					NodeName:  &endpointNodes[0],
					Conditions: discoveryv1.EndpointConditions{
						Ready: &endpointReady,
					},
				},
			},
		}
		err = k8sClient.Create(ctx, eps)
		Expect(err).NotTo(HaveOccurred())

		By("getting service")
		svc = &v1.Service{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: "lb-cluster2"}, svc); err != nil {
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
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: svcToAdvertisementName(svc, "ipv4")}, adv); err != nil {
				return err
			}
			if adv.Status.Condition != sartv1alpha1.BGPAdvertisementConditionUnavailable {
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
		adv.Status.Condition = sartv1alpha1.BGPAdvertisementConditionAvailable
		err = k8sClient.Status().Update(ctx, adv)
		Expect(err).NotTo(HaveOccurred())

		By("assigning an address to Service")
		svc = &v1.Service{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: "lb-cluster2"}, svc); err != nil {
				return err
			}
			if len(svc.Status.Conditions) < 1 {
				return fmt.Errorf("expected at least one condition")
			}
			assignedCondition := svc.Status.Conditions[len(svc.Status.Conditions)-1]
			if assignedCondition.Type != ServiceConditionTypeAdvertised {
				return fmt.Errorf("want: %v, got: %v", corev1.ConditionStatus(ServiceConditionReasonAdvertised), assignedCondition.Reason)
			}
			if len(svc.Status.LoadBalancer.Ingress) == 0 {
				return fmt.Errorf("expected at least one ingress")
			}
			lbStatus := svc.Status.LoadBalancer.Ingress[0]
			if lbStatus.IP != allocInfo.addr.String() {
				return fmt.Errorf("want: %s, got: %s", allocInfo.addr, lbStatus.IP)
			}
			return nil
		}).Should(Succeed())

		By("checking the assigned address")
		ingress := svc.Status.LoadBalancer.Ingress[0]
		Expect(ingress.IP).To(Equal("10.0.10.12"))

	})

	It("should handle LoadBalancer when deleting", func() {
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

		defer func() {
			if err := k8sClient.Delete(ctx, pool); err != nil {
				Fail(err.Error())
			}
		}()

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
				Name:      "lb-local2",
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
					"app": "app2",
				},
				Type:                  v1.ServiceTypeLoadBalancer,
				ExternalTrafficPolicy: v1.ServiceExternalTrafficPolicyTypeLocal,
			},
		}
		err = k8sClient.Create(ctx, svc)
		Expect(err).NotTo(HaveOccurred())

		By("creating EndpointSlice")
		nodes := []string{"node1", "node2"}
		eps := &discoveryv1.EndpointSlice{
			ObjectMeta: corev1.ObjectMeta{
				Name:      "lb-local2",
				Namespace: "default",
				Labels: map[string]string{
					constants.KubernetesServiceNameLabel: "lb-local2",
				},
				Annotations: map[string]string{},
			},
			AddressType: "IPv4",
			Endpoints: []discoveryv1.Endpoint{
				{
					Addresses: []string{"10.100.0.1"},
					NodeName:  &nodes[0],
				},
				{
					Addresses: []string{"10.100.0.2"},
					NodeName:  &nodes[1],
				},
			},
		}
		err = k8sClient.Create(ctx, eps)
		Expect(err).NotTo(HaveOccurred())

		By("getting service")
		svc = &v1.Service{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: "lb-local2"}, svc); err != nil {
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
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: svcToAdvertisementName(svc, "ipv4")}, adv); err != nil {
				return err
			}
			if adv.Status.Condition != sartv1alpha1.BGPAdvertisementConditionUnavailable {
				return fmt.Errorf("advertisement status is not unavailable: got %v", adv.Status.Condition)
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
		adv.Status.Condition = sartv1alpha1.BGPAdvertisementConditionAvailable
		err = k8sClient.Status().Update(ctx, adv)
		Expect(err).NotTo(HaveOccurred())

		By("assigning an address to Service")
		svc = &v1.Service{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: "lb-local2"}, svc); err != nil {
				return err
			}
			if len(svc.Status.Conditions) < 1 {
				return fmt.Errorf("expected at least one condition")
			}
			assignedCondition := svc.Status.Conditions[len(svc.Status.Conditions)-1]
			if assignedCondition.Type != ServiceConditionTypeAdvertised {
				return fmt.Errorf("want: %v, got: %v", corev1.ConditionStatus(ServiceConditionReasonAdvertised), assignedCondition.Reason)
			}
			if len(svc.Status.LoadBalancer.Ingress) == 0 {
				return fmt.Errorf("expected at least one ingress")
			}
			lbStatus := svc.Status.LoadBalancer.Ingress[0]
			if lbStatus.IP != allocInfo.addr.String() {
				return fmt.Errorf("want: %s, got: %s", allocInfo.addr, lbStatus.IP)
			}
			return nil
		}).Should(Succeed())

		By("deleting LoadBalancer")
		err = k8sClient.Delete(ctx, svc)
		Expect(err).NotTo(HaveOccurred())

		Eventually(func() error {
			deletedSvc := &v1.Service{}
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: svc.Namespace, Name: svc.Name}, deletedSvc); err != nil {
				if apierrors.IsNotFound(err) {
					return nil
				}
				return err
			}
			return fmt.Errorf("expected not to be able to get")
		}).Should(Succeed())

		By("deleting BGPAdvertisement")
		Eventually(func() error {
			deletedAdv := &sartv1alpha1.BGPAdvertisement{}
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: adv.Namespace, Name: adv.Name}, deletedAdv); err != nil {
				if apierrors.IsNotFound(err) {
					return nil
				}
				return err
			}
			return fmt.Errorf("expected not to be able to get")
		}).Should(Succeed())

		By("deleting alloc info")
		Eventually(func() error {
			if _, ok = allocMap[types.NamespacedName{Namespace: svc.Namespace, Name: svc.Name}.String()]; !ok {
				return nil
			}
			return fmt.Errorf("should not get")
		}).Should(Succeed())

		protocolAllocator, ok = allocatorMap["default"]
		Expect(ok).To(BeTrue())
		allocator, ok = protocolAllocator["ipv4"]
		Expect(ok).To(BeTrue())
		Expect(allocator.IsAllocated(allocInfo.addr)).To(BeFalse())
	})

	It("should handle LoadBalancer externalTrafficPolicy=Local when endpoints are changed", func() {
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

		defer func() {
			if err := k8sClient.Delete(ctx, pool); err != nil {
				Fail(err.Error())
			}
		}()

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
				Name:      "lb-local3",
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
					"app": "app2",
				},
				Type:                  v1.ServiceTypeLoadBalancer,
				ExternalTrafficPolicy: v1.ServiceExternalTrafficPolicyTypeLocal,
			},
		}
		err = k8sClient.Create(ctx, svc)
		Expect(err).NotTo(HaveOccurred())

		By("creating EndpointSlice")
		nodes := []string{"node1", "node2"}
		eps := &discoveryv1.EndpointSlice{
			ObjectMeta: corev1.ObjectMeta{
				Name:      "lb-local3",
				Namespace: "default",
				Labels: map[string]string{
					constants.KubernetesServiceNameLabel: "lb-local3",
				},
				Annotations: map[string]string{},
			},
			AddressType: "IPv4",
			Endpoints: []discoveryv1.Endpoint{
				{
					Addresses: []string{"10.100.0.1"},
					NodeName:  &nodes[0],
					Conditions: discoveryv1.EndpointConditions{
						Ready: &endpointReady,
					},
				},
				{
					Addresses: []string{"10.100.0.2"},
					NodeName:  &nodes[1],
				},
			},
		}
		err = k8sClient.Create(ctx, eps)
		Expect(err).NotTo(HaveOccurred())

		By("getting service")
		svc = &v1.Service{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: "lb-local3"}, svc); err != nil {
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
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: svcToAdvertisementName(svc, "ipv4")}, adv); err != nil {
				return err
			}
			if adv.Status.Condition != sartv1alpha1.BGPAdvertisementConditionUnavailable {
				return fmt.Errorf("advertisement status is not unavailable: got %v", adv.Status.Condition)
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
		adv.Status.Condition = sartv1alpha1.BGPAdvertisementConditionAvailable
		err = k8sClient.Status().Update(ctx, adv)
		Expect(err).NotTo(HaveOccurred())

		By("assigning an address to Service")
		svc = &v1.Service{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: "lb-local3"}, svc); err != nil {
				return err
			}
			if len(svc.Status.Conditions) < 1 {
				return fmt.Errorf("expected at least one condition")
			}
			assignedCondition := svc.Status.Conditions[len(svc.Status.Conditions)-1]
			if assignedCondition.Type != ServiceConditionTypeAdvertised {
				return fmt.Errorf("want: %v, got: %v", corev1.ConditionStatus(ServiceConditionReasonAdvertised), assignedCondition.Reason)
			}
			if len(svc.Status.LoadBalancer.Ingress) == 0 {
				return fmt.Errorf("expected at least one ingress")
			}
			lbStatus := svc.Status.LoadBalancer.Ingress[0]
			if lbStatus.IP != allocInfo.addr.String() {
				return fmt.Errorf("want: %s, got: %s", allocInfo.addr, lbStatus.IP)
			}
			return nil
		}).Should(Succeed())

		By("updating backend nodes")
		err = k8sClient.Get(ctx, types.NamespacedName{Namespace: eps.Namespace, Name: eps.Name}, eps)
		Expect(err).NotTo(HaveOccurred())
		newEps := eps.DeepCopy()
		newNodes := []string{"node1", "node3"}
		newEps.Endpoints[1].Addresses = []string{"10.0.100.3"}
		newEps.Endpoints[1].NodeName = &newNodes[1]
		err = k8sClient.Update(ctx, newEps)
		Expect(err).NotTo(HaveOccurred())

		By("checking BGPAdvertisement is updated")
		Eventually(func() error {
			updatedAdv := &sartv1alpha1.BGPAdvertisement{}
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: adv.Namespace, Name: adv.Name}, updatedAdv); err != nil {
				return err
			}
			sort.Strings(updatedAdv.Spec.Nodes)
			if !reflect.DeepEqual(updatedAdv.Spec.Nodes, newNodes) {
				return fmt.Errorf("updated advertised nodes is not matched. want: %v, got: %v", newNodes, updatedAdv.Spec.Nodes)
			}
			if updatedAdv.Status.Condition != sartv1alpha1.BGPAdvertisementConditionAvailable {
				return fmt.Errorf("status is not matched. want: %s, got: %s", sartv1alpha1.BGPAdvertisementConditionAvailable, updatedAdv.Status.Condition)
			}
			return nil
		}).Should(Succeed())
	})

	It("should handle multiple LoadBalancers", func() {

		nodes := []string{"node1", "node2"}

		By("creating default2 namespace")
		default2Ns := &v1.Namespace{
			ObjectMeta: corev1.ObjectMeta{
				Name: "default2",
			},
		}
		err := k8sClient.Create(ctx, default2Ns)
		Expect(err).NotTo(HaveOccurred())

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
		err = k8sClient.Create(ctx, pool)
		Expect(err).NotTo(HaveOccurred())

		defer func() {
			if err := k8sClient.Delete(ctx, pool); err != nil {
				Fail(err.Error())
			}
		}()

		pool = &sartv1alpha1.AddressPool{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, client.ObjectKey{Name: "default"}, pool); err != nil {
				return err
			}
			return nil
		}).Should(Succeed())

		By("creating LoadBalancers")
		svcList := []*v1.Service{
			{
				ObjectMeta: corev1.ObjectMeta{
					Name:      "lb-local4",
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
						"app": "app2",
					},
					Type:                  v1.ServiceTypeLoadBalancer,
					ExternalTrafficPolicy: v1.ServiceExternalTrafficPolicyTypeLocal,
				},
			},
			{
				ObjectMeta: corev1.ObjectMeta{
					Name:      "lb-cluster3",
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
						"app": "app2",
					},
					Type:                  v1.ServiceTypeLoadBalancer,
					ExternalTrafficPolicy: v1.ServiceExternalTrafficPolicyTypeCluster,
				},
			},
			{
				ObjectMeta: corev1.ObjectMeta{
					Name:      "lb-cluster4",
					Namespace: "default2",
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
						"app": "app3",
					},
					Type:                  v1.ServiceTypeLoadBalancer,
					ExternalTrafficPolicy: v1.ServiceExternalTrafficPolicyTypeCluster,
				},
			},
		}
		for _, svc := range svcList {
			err := k8sClient.Create(ctx, svc)
			Expect(err).NotTo(HaveOccurred())
		}

		By("creating EndpointSlices")
		epsList := []*discoveryv1.EndpointSlice{
			{
				ObjectMeta: corev1.ObjectMeta{
					Name:      "lb-local4",
					Namespace: "default",
					Labels: map[string]string{
						constants.KubernetesServiceNameLabel: "lb-local4",
					},
					Annotations: map[string]string{},
				},
				AddressType: "IPv4",
				Endpoints: []discoveryv1.Endpoint{
					{
						Addresses: []string{"10.100.0.1"},
						NodeName:  &nodes[0],
						Conditions: discoveryv1.EndpointConditions{
							Ready: &endpointReady,
						},
					},
					{
						Addresses: []string{"10.100.0.2"},
						NodeName:  &nodes[1],
						Conditions: discoveryv1.EndpointConditions{
							Ready: &endpointReady,
						},
					},
				},
			},
			{
				ObjectMeta: corev1.ObjectMeta{
					Name:      "lb-cluster3",
					Namespace: "default",
					Labels: map[string]string{
						constants.KubernetesServiceNameLabel: "lb-cluster3",
					},
					Annotations: map[string]string{},
				},
				AddressType: "IPv4",
				Endpoints: []discoveryv1.Endpoint{
					{
						Addresses: []string{"10.100.0.2"},
						NodeName:  &nodes[1],
						Conditions: discoveryv1.EndpointConditions{
							Ready: &endpointReady,
						},
					},
				},
			},
			{
				ObjectMeta: corev1.ObjectMeta{
					Name:      "lb-cluster4",
					Namespace: "default",
					Labels: map[string]string{
						constants.KubernetesServiceNameLabel: "lb-cluster4",
					},
					Annotations: map[string]string{},
				},
				AddressType: "IPv4",
				Endpoints: []discoveryv1.Endpoint{
					{
						Addresses: []string{"10.100.0.4"},
						NodeName:  &nodes[0],
						Conditions: discoveryv1.EndpointConditions{
							Ready: &endpointReady,
						},
					},
				},
			},
		}
		for _, eps := range epsList {
			err := k8sClient.Create(ctx, eps)
			Expect(err).NotTo(HaveOccurred())
		}

		By("getting BGPAdvertisements")
		Eventually(func() error {
			for _, svc := range svcList {
				adv := &sartv1alpha1.BGPAdvertisement{}
				if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: svc.Namespace, Name: svcToAdvertisementName(svc, "ipv4")}, adv); err != nil {
					return err
				}
			}
			return nil
		}).Should(Succeed())

		By("changing BGPAdvertisement status")
		advs := []*sartv1alpha1.BGPAdvertisement{}
		for _, svc := range svcList {
			adv := &sartv1alpha1.BGPAdvertisement{}
			err := k8sClient.Get(ctx, types.NamespacedName{Namespace: svc.Namespace, Name: svcToAdvertisementName(svc, "ipv4")}, adv)
			Expect(err).NotTo(HaveOccurred())
			advs = append(advs, adv)
		}
		for _, adv := range advs {
			newAdv := adv.DeepCopy()
			newAdv.Status.Condition = sartv1alpha1.BGPAdvertisementConditionAvailable
			err = k8sClient.Status().Update(ctx, newAdv)
			Expect(err).NotTo(HaveOccurred())
		}

		By("collecting assigned addresses")
		lbAddrs := []string{}
		for _, svc := range svcList {
			a, ok := allocMap[types.NamespacedName{Namespace: svc.Namespace, Name: svc.Name}.String()]
			Expect(ok).To(BeTrue())
			Expect(len(a)).To(Equal(1))
			allocInfo := a[0]
			lbAddrs = append(lbAddrs, allocInfo.addr.String())
		}

		By("checking assigned correctly")
		Eventually(func() error {
			for i := 0; i < len(svcList); i++ {
				svc := &v1.Service{}
				if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: svcList[i].Namespace, Name: svcList[i].Name}, svc); err != nil {
					return err
				}
				if len(svc.Status.LoadBalancer.Ingress) < 1 {
					return fmt.Errorf("expected at least one entry")
				}
				if svc.Status.LoadBalancer.Ingress[0].IP != lbAddrs[i] {
					return fmt.Errorf("assinged address is not matched: svc: %s, want: %s, got: %s", types.NamespacedName{Namespace: svcList[i].Namespace, Name: svcList[i].Name}, lbAddrs[i], svc.Status.LoadBalancer.Ingress[0].IP)
				}
			}
			return nil
		}).Should(Succeed())

		By("deleting lb-cluster3")
		releasedAlloc := allocMap[types.NamespacedName{Namespace: svcList[1].Namespace, Name: svcList[1].Name}.String()]
		err = k8sClient.Delete(ctx, svcList[1])
		Expect(err).NotTo(HaveOccurred())

		Eventually(func() error {
			if _, ok := allocMap[types.NamespacedName{Namespace: svcList[1].Namespace, Name: svcList[1].Name}.String()]; !ok {
				return nil
			}
			return fmt.Errorf("expected not to able to get")
		}).Should(Succeed())

		By("creating new LoadBalancers")
		newSvcList := []*v1.Service{
			{
				ObjectMeta: corev1.ObjectMeta{
					Name:      "lb-local5",
					Namespace: "default2",
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
						"app": "app3",
					},
					Type:                  v1.ServiceTypeLoadBalancer,
					ExternalTrafficPolicy: v1.ServiceExternalTrafficPolicyTypeCluster,
				},
			},
			{
				ObjectMeta: corev1.ObjectMeta{
					Name:      "lb-local6",
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
						"app": "app3",
					},
					Type:                  v1.ServiceTypeLoadBalancer,
					ExternalTrafficPolicy: v1.ServiceExternalTrafficPolicyTypeCluster,
				},
			},
		}
		for _, svc := range newSvcList {
			err := k8sClient.Create(ctx, svc)
			Expect(err).NotTo(HaveOccurred())
		}
		By("creating EndpointSlices")
		newEpsList := []*discoveryv1.EndpointSlice{
			{
				ObjectMeta: corev1.ObjectMeta{
					Name:      "lb-local5",
					Namespace: "default2",
					Labels: map[string]string{
						constants.KubernetesServiceNameLabel: "lb-local5",
					},
					Annotations: map[string]string{},
				},
				AddressType: "IPv4",
				Endpoints: []discoveryv1.Endpoint{
					{
						Addresses: []string{"10.100.0.1"},
						NodeName:  &nodes[0],
						Conditions: discoveryv1.EndpointConditions{
							Ready: &endpointReady,
						},
					},
					{
						Addresses: []string{"10.100.0.2"},
						NodeName:  &nodes[1],
						Conditions: discoveryv1.EndpointConditions{
							Ready: &endpointReady,
						},
					},
				},
			},
			{
				ObjectMeta: corev1.ObjectMeta{
					Name:      "lb-local6",
					Namespace: "default",
					Labels: map[string]string{
						constants.KubernetesServiceNameLabel: "lb-local6",
					},
					Annotations: map[string]string{},
				},
				AddressType: "IPv4",
				Endpoints: []discoveryv1.Endpoint{
					{
						Addresses: []string{"10.100.0.1"},
						NodeName:  &nodes[0],
						Conditions: discoveryv1.EndpointConditions{
							Ready: &endpointReady,
						},
					},
					{
						Addresses: []string{"10.100.0.2"},
						NodeName:  &nodes[1],
					},
				},
			},
		}
		for _, eps := range newEpsList {
			err = k8sClient.Create(ctx, eps)
			Expect(err).NotTo(HaveOccurred())
		}

		By("getting BGPAdvertisements")
		Eventually(func() error {
			for _, svc := range newSvcList {
				adv := &sartv1alpha1.BGPAdvertisement{}
				if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: svc.Namespace, Name: svcToAdvertisementName(svc, "ipv4")}, adv); err != nil {
					return err
				}
			}
			return nil
		}).Should(Succeed())

		By("changing BGPAdvertisement status")
		newAdvs := []*sartv1alpha1.BGPAdvertisement{}
		for _, svc := range newSvcList {
			adv := &sartv1alpha1.BGPAdvertisement{}
			err := k8sClient.Get(ctx, types.NamespacedName{Namespace: svc.Namespace, Name: svcToAdvertisementName(svc, "ipv4")}, adv)
			Expect(err).NotTo(HaveOccurred())
			newAdvs = append(newAdvs, adv)
		}
		for _, adv := range newAdvs {
			newAdv := adv.DeepCopy()
			newAdv.Status.Condition = sartv1alpha1.BGPAdvertisementConditionAvailable
			err = k8sClient.Status().Update(ctx, newAdv)
			Expect(err).NotTo(HaveOccurred())
		}

		By("collecting assigned addresses")
		newLbAddrs := []string{}
		for _, svc := range newSvcList {
			a, ok := allocMap[types.NamespacedName{Namespace: svc.Namespace, Name: svc.Name}.String()]
			Expect(ok).To(BeTrue())
			Expect(len(a)).To(Equal(1))
			allocInfo := a[0]
			newLbAddrs = append(newLbAddrs, allocInfo.addr.String())
		}
		sort.Strings(newLbAddrs)

		By("checking LB addresses are assigned correctly")
		Expect(newLbAddrs[0]).To(Equal(releasedAlloc[0].addr.String()))
	})

	It("should create multiple AddressPools", func() {
		By("creating an test1 address pool")
		pool1 := &sartv1alpha1.AddressPool{
			ObjectMeta: corev1.ObjectMeta{
				Name: "test1",
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
		err := k8sClient.Create(ctx, pool1)
		Expect(err).NotTo(HaveOccurred())

		pool1 = &sartv1alpha1.AddressPool{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, client.ObjectKey{Name: "test1"}, pool1); err != nil {
				return err
			}
			return nil
		}).Should(Succeed())

		By("existing an allocator for test1")
		cidr1, err := netip.ParsePrefix("10.0.10.0/24")
		Expect(err).NotTo(HaveOccurred())
		Eventually(func() error {
			allocators, ok := allocatorMap["test1"]
			if !ok {
				return fmt.Errorf("should get allocator")
			}
			ipv4Allocator, ok := allocators["ipv4"]
			if !ok {
				return fmt.Errorf("should get ipv4")
			}
			if !reflect.DeepEqual(ipv4Allocator, allocator.New(&cidr1)) {
				return fmt.Errorf("should equal")
			}
			return nil
		}).WithPolling(1 * time.Second).Should(Succeed())

		By("creating an test1 address pool")
		pool2 := &sartv1alpha1.AddressPool{
			ObjectMeta: corev1.ObjectMeta{
				Name: "test2",
			},
			Spec: sartv1alpha1.AddressPoolSpec{
				Type: "lb",
				Cidrs: []sartv1alpha1.Cdir{
					{
						Protocol: "ipv4",
						Prefix:   "10.0.20.0/24",
					},
				},
				Disable: false,
			},
		}
		err = k8sClient.Create(ctx, pool2)
		Expect(err).NotTo(HaveOccurred())

		pool2 = &sartv1alpha1.AddressPool{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, client.ObjectKey{Name: "test2"}, pool2); err != nil {
				return err
			}
			return nil
		}).Should(Succeed())

		By("existing an allocator for test1")
		cidr2, err := netip.ParsePrefix("10.0.20.0/24")
		Expect(err).NotTo(HaveOccurred())
		Eventually(func() error {
			allocators, ok := allocatorMap["test2"]
			if !ok {
				return fmt.Errorf("should get allocator")
			}
			ipv4Allocator, ok := allocators["ipv4"]
			if !ok {
				return fmt.Errorf("should get ipv4")
			}
			if !reflect.DeepEqual(ipv4Allocator, allocator.New(&cidr2)) {
				return fmt.Errorf("should equal")
			}
			return nil
		}).WithPolling(1 * time.Second).Should(Succeed())

		By("deleting AddressPools")
		err = k8sClient.Delete(ctx, pool1)
		Expect(err).NotTo(HaveOccurred())
		err = k8sClient.Delete(ctx, pool2)
		Expect(err).NotTo(HaveOccurred())

		By("not existing an allocator")
		Eventually(func() error {
			_, ok := allocatorMap["test1"]
			if ok {
				return fmt.Errorf("should not get allocator for test1")
			}
			_, ok = allocatorMap["test2"]
			if ok {
				return fmt.Errorf("should not get allocator for test2")
			}
			return nil
		}).WithPolling(1 * time.Second).Should(Succeed())
	})

	It("should create multiple LoadBalancers with multiple AddressPools", func() {

		nodes := []string{"node2", "node3"}

		By("creating address pools")
		pools := []*sartv1alpha1.AddressPool{
			{
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
			},
			{
				ObjectMeta: corev1.ObjectMeta{
					Name: "not-default",
				},
				Spec: sartv1alpha1.AddressPoolSpec{
					Type: "lb",
					Cidrs: []sartv1alpha1.Cdir{
						{
							Protocol: "ipv4",
							Prefix:   "10.0.20.0/24",
						},
					},
					Disable: false,
				},
			},
		}
		for _, pool := range pools {
			err := k8sClient.Create(ctx, pool)
			Expect(err).NotTo(HaveOccurred())
		}

		defer func() {
			for _, pool := range pools {
				if err := k8sClient.Delete(ctx, pool); err != nil {
					Fail(err.Error())
				}
			}
		}()

		By("creating LoadBalancers")
		svcList := []*v1.Service{
			{
				ObjectMeta: corev1.ObjectMeta{
					Name:      "lb-local7",
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
						"app": "app2",
					},
					Type:                  v1.ServiceTypeLoadBalancer,
					ExternalTrafficPolicy: v1.ServiceExternalTrafficPolicyTypeLocal,
				},
			},
			{
				ObjectMeta: corev1.ObjectMeta{
					Name:      "lb-cluster5",
					Namespace: "default",
					Annotations: map[string]string{
						constants.AnnotationAddressPool: "not-default",
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
						"app": "app3",
					},
					Type:                  v1.ServiceTypeLoadBalancer,
					ExternalTrafficPolicy: v1.ServiceExternalTrafficPolicyTypeCluster,
				},
			},
		}
		for _, svc := range svcList {
			err := k8sClient.Create(ctx, svc)
			Expect(err).NotTo(HaveOccurred())
		}

		By("creating EndpointSlices")
		epsList := []*discoveryv1.EndpointSlice{
			{
				ObjectMeta: corev1.ObjectMeta{
					Name:      "lb-local7",
					Namespace: "default",
					Labels: map[string]string{
						constants.KubernetesServiceNameLabel: "lb-local7",
					},
					Annotations: map[string]string{},
				},
				AddressType: "IPv4",
				Endpoints: []discoveryv1.Endpoint{
					{
						Addresses: []string{"10.100.0.1"},
						NodeName:  &nodes[0],
						Conditions: discoveryv1.EndpointConditions{
							Ready: &endpointReady,
						},
					},
					{
						Addresses: []string{"10.100.0.2"},
						NodeName:  &nodes[1],
						Conditions: discoveryv1.EndpointConditions{
							Ready: &endpointReady,
						},
					},
				},
			},
			{
				ObjectMeta: corev1.ObjectMeta{
					Name:      "lb-cluster5",
					Namespace: "default",
					Labels: map[string]string{
						constants.KubernetesServiceNameLabel: "lb-cluster5",
					},
					Annotations: map[string]string{},
				},
				AddressType: "IPv4",
				Endpoints: []discoveryv1.Endpoint{
					{
						Addresses: []string{"10.100.0.4"},
						NodeName:  &nodes[0],
						Conditions: discoveryv1.EndpointConditions{
							Ready: &endpointReady,
						},
					},
				},
			},
		}
		for _, eps := range epsList {
			err := k8sClient.Create(ctx, eps)
			Expect(err).NotTo(HaveOccurred())
		}

		By("getting BGPAdvertisements")
		Eventually(func() error {
			for _, svc := range svcList {
				adv := &sartv1alpha1.BGPAdvertisement{}
				if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: svc.Namespace, Name: svcToAdvertisementName(svc, "ipv4")}, adv); err != nil {
					return err
				}
			}
			return nil
		}).Should(Succeed())

		By("changing BGPAdvertisement status")
		advs := []*sartv1alpha1.BGPAdvertisement{}
		for _, svc := range svcList {
			adv := &sartv1alpha1.BGPAdvertisement{}
			err := k8sClient.Get(ctx, types.NamespacedName{Namespace: svc.Namespace, Name: svcToAdvertisementName(svc, "ipv4")}, adv)
			Expect(err).NotTo(HaveOccurred())
			advs = append(advs, adv)
		}
		for _, adv := range advs {
			newAdv := adv.DeepCopy()
			newAdv.Status.Condition = sartv1alpha1.BGPAdvertisementConditionAvailable
			err := k8sClient.Status().Update(ctx, newAdv)
			Expect(err).NotTo(HaveOccurred())
		}

		By("collecting assigned addresses")
		lbAddrs := []string{}
		for _, svc := range svcList {
			a, ok := allocMap[types.NamespacedName{Namespace: svc.Namespace, Name: svc.Name}.String()]
			Expect(ok).To(BeTrue())
			Expect(len(a)).To(Equal(1))
			allocInfo := a[0]
			lbAddrs = append(lbAddrs, allocInfo.addr.String())
		}

		By("checking the address cidr")
		Expect(len(lbAddrs)).To(Equal(2))
		pool1Cidr, err := netip.ParsePrefix("10.0.10.0/24")
		Expect(err).NotTo(HaveOccurred())
		pool2Cidr, err := netip.ParsePrefix("10.0.20.0/24")
		Expect(err).NotTo(HaveOccurred())
		cidrs := []netip.Prefix{pool1Cidr, pool2Cidr}
		for i := 0; i < len(lbAddrs); i++ {
			addr, err := netip.ParseAddr(lbAddrs[i])
			Expect(err).NotTo(HaveOccurred())
			res := cidrs[i].Contains(addr)
			Expect(res).To(BeTrue())
		}

		By("checking assigned correctly")
		Eventually(func() error {
			for i := 0; i < len(svcList); i++ {
				svc := &v1.Service{}
				if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: svcList[i].Namespace, Name: svcList[i].Name}, svc); err != nil {
					return err
				}
				if len(svc.Status.LoadBalancer.Ingress) < 1 {
					return fmt.Errorf("expected at least one entry")
				}
				if svc.Status.LoadBalancer.Ingress[0].IP != lbAddrs[i] {
					return fmt.Errorf("assinged address is not matched: svc: %s, want: %s, got: %s", types.NamespacedName{Namespace: svcList[i].Namespace, Name: svcList[i].Name}, lbAddrs[i], svc.Status.LoadBalancer.Ingress[0].IP)
				}
			}
			return nil
		}).Should(Succeed())
	})

	It("should change externalTrafficPolicy from Cluster to Local", func() {
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

		defer func() {
			if err := k8sClient.Delete(ctx, pool); err != nil {
				Fail(err.Error())
			}
		}()

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
				Name:      "lb-cluster6",
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

		By("creating EndpointSlice")
		nodes := []string{"node1", "node2"}
		eps := &discoveryv1.EndpointSlice{
			ObjectMeta: corev1.ObjectMeta{
				Name:      "lb-cluster6",
				Namespace: "default",
				Labels: map[string]string{
					constants.KubernetesServiceNameLabel: "lb-cluster6",
				},
				Annotations: map[string]string{},
			},
			AddressType: "IPv4",
			Endpoints: []discoveryv1.Endpoint{
				{
					Addresses: []string{"10.100.0.1"},
					NodeName:  &nodes[0],
					Conditions: discoveryv1.EndpointConditions{
						Ready: &endpointReady,
					},
				},
				{
					Addresses: []string{"10.100.0.2"},
					NodeName:  &nodes[1],
					Conditions: discoveryv1.EndpointConditions{
						Ready: &endpointReady,
					},
				},
			},
		}
		err = k8sClient.Create(ctx, eps)
		Expect(err).NotTo(HaveOccurred())

		By("getting service")
		svc = &v1.Service{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: "lb-cluster6"}, svc); err != nil {
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
		advertisedNodes := []string{"node1", "node2", "node3"}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: svcToAdvertisementName(svc, "ipv4")}, adv); err != nil {
				return err
			}
			if adv.Status.Condition != sartv1alpha1.BGPAdvertisementConditionUnavailable {
				return fmt.Errorf("advertisement status is not advertising: got %v", adv.Status.Condition)
			}
			sort.Strings(adv.Spec.Nodes)
			if !reflect.DeepEqual(adv.Spec.Nodes, advertisedNodes) {
				return fmt.Errorf("want: %v, got: %v", advertisedNodes, adv.Spec.Nodes)
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

		By("changing the advertisement status to available")
		adv.Status.Condition = sartv1alpha1.BGPAdvertisementConditionAvailable
		err = k8sClient.Status().Update(ctx, adv)
		Expect(err).NotTo(HaveOccurred())

		By("assigning an address to Service")
		svc = &v1.Service{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: "lb-cluster6"}, svc); err != nil {
				return err
			}
			if len(svc.Status.Conditions) < 1 {
				return fmt.Errorf("expected at least one condition")
			}
			assignedCondition := svc.Status.Conditions[len(svc.Status.Conditions)-1]
			if assignedCondition.Type != ServiceConditionTypeAdvertised {
				return fmt.Errorf("want: %v, got: %v", corev1.ConditionStatus(ServiceConditionReasonAdvertised), assignedCondition.Reason)
			}
			if len(svc.Status.LoadBalancer.Ingress) == 0 {
				return fmt.Errorf("expected at least one ingress")
			}
			lbStatus := svc.Status.LoadBalancer.Ingress[0]
			if lbStatus.IP != allocInfo.addr.String() {
				return fmt.Errorf("want: %s, got: %s", allocInfo.addr, lbStatus.IP)
			}
			return nil
		}).Should(Succeed())

		By("changing the externalTrafficPolicy to Local")
		svc.Spec.ExternalTrafficPolicy = v1.ServiceExternalTrafficPolicyTypeLocal
		err = k8sClient.Update(ctx, svc)
		Expect(err).NotTo(HaveOccurred())

		By("checking advertised nodes are changed")
		adv = &sartv1alpha1.BGPAdvertisement{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: svcToAdvertisementName(svc, "ipv4")}, adv); err != nil {
				return err
			}
			sort.Strings(adv.Spec.Nodes)
			if !reflect.DeepEqual(adv.Spec.Nodes, nodes) {
				return fmt.Errorf("want: %v, got: %v", nodes, adv.Spec.Nodes)
			}
			return nil
		}).Should(Succeed())
	})

	It("should change externalTrafficPolicy from Local to Cluster", func() {

		allNodes := []string{"node1", "node2", "node3"}

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

		defer func() {
			if err := k8sClient.Delete(ctx, pool); err != nil {
				Fail(err.Error())
			}
		}()

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
				Name:      "lb-local8",
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
					"app": "app2",
				},
				Type:                  v1.ServiceTypeLoadBalancer,
				ExternalTrafficPolicy: v1.ServiceExternalTrafficPolicyTypeLocal,
			},
		}
		err = k8sClient.Create(ctx, svc)
		Expect(err).NotTo(HaveOccurred())

		By("creating EndpointSlice")
		nodes := []string{"node1", "node2"}
		eps := &discoveryv1.EndpointSlice{
			ObjectMeta: corev1.ObjectMeta{
				Name:      "lb-local8",
				Namespace: "default",
				Labels: map[string]string{
					constants.KubernetesServiceNameLabel: "lb-local8",
				},
				Annotations: map[string]string{},
			},
			AddressType: "IPv4",
			Endpoints: []discoveryv1.Endpoint{
				{
					Addresses: []string{"10.100.0.1"},
					NodeName:  &nodes[0],
					Conditions: discoveryv1.EndpointConditions{
						Ready: &endpointReady,
					},
				},
				{
					Addresses: []string{"10.100.0.2"},
					NodeName:  &nodes[1],
					Conditions: discoveryv1.EndpointConditions{
						Ready: &endpointReady,
					},
				},
			},
		}
		err = k8sClient.Create(ctx, eps)
		Expect(err).NotTo(HaveOccurred())

		By("getting service")
		svc = &v1.Service{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: "lb-local8"}, svc); err != nil {
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
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: svcToAdvertisementName(svc, "ipv4")}, adv); err != nil {
				return err
			}
			if adv.Status.Condition != sartv1alpha1.BGPAdvertisementConditionUnavailable {
				return fmt.Errorf("advertisement status is not unavailable : got %v", adv.Status.Condition)
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
		adv.Status.Condition = sartv1alpha1.BGPAdvertisementConditionAvailable
		err = k8sClient.Status().Update(ctx, adv)
		Expect(err).NotTo(HaveOccurred())

		By("assigning an address to Service")
		svc = &v1.Service{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: "lb-local8"}, svc); err != nil {
				return err
			}
			if len(svc.Status.Conditions) < 1 {
				return fmt.Errorf("expected at least one condition")
			}
			assignedCondition := svc.Status.Conditions[len(svc.Status.Conditions)-1]
			if assignedCondition.Type != ServiceConditionTypeAdvertised {
				return fmt.Errorf("want: %v, got: %v", corev1.ConditionStatus(ServiceConditionReasonAdvertised), assignedCondition.Reason)
			}
			if len(svc.Status.LoadBalancer.Ingress) == 0 {
				return fmt.Errorf("expected at least one ingress")
			}
			lbStatus := svc.Status.LoadBalancer.Ingress[0]
			if lbStatus.IP != allocInfo.addr.String() {
				return fmt.Errorf("want: %s, got: %s", allocInfo.addr, lbStatus.IP)
			}
			return nil
		}).Should(Succeed())

		By("changing the externalTrafficPolicy to Cluster")
		svc.Spec.ExternalTrafficPolicy = v1.ServiceExternalTrafficPolicyTypeCluster
		err = k8sClient.Update(ctx, svc)
		Expect(err).NotTo(HaveOccurred())

		By("checking advertised nodes are changed")
		adv = &sartv1alpha1.BGPAdvertisement{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: "default", Name: svcToAdvertisementName(svc, "ipv4")}, adv); err != nil {
				return err
			}
			sort.Strings(adv.Spec.Nodes)
			if !reflect.DeepEqual(adv.Spec.Nodes, allNodes) {
				return fmt.Errorf("want: %v, got: %v", allNodes, adv.Spec.Nodes)
			}
			return nil
		}).Should(Succeed())
	})

})
