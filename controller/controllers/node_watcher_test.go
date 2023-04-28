package controllers

import (
	"context"
	"fmt"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
	sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	"github.com/terassyi/sart/controller/pkg/constants"
	"github.com/terassyi/sart/controller/pkg/speaker"
	corev1 "k8s.io/api/core/v1"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	"k8s.io/apimachinery/pkg/types"
	ctrl "sigs.k8s.io/controller-runtime"
)

var _ = Describe("Node Watcher", func() {
	ctx := context.Background()
	var cancel context.CancelFunc

	BeforeEach(func() {
		mgr, err := ctrl.NewManager(cfg, ctrl.Options{
			Scheme:             scheme,
			LeaderElection:     false,
			MetricsBindAddress: "0",
		})
		Expect(err).NotTo(HaveOccurred())

		nodeWatcher := &NodeWatcher{
			Client:              k8sClient,
			SpeakerEndpointPort: 5000,
			SpeakerType:         speaker.SpeakerTypeMock,
		}
		err = nodeWatcher.SetupWithManager(mgr)
		Expect(err).NotTo(HaveOccurred())

		ctx, cancel = context.WithCancel(context.TODO())
		go func() {
			err := mgr.Start(ctx)
			if err != nil {
				panic(err)
			}
		}()
		time.Sleep(100 * time.Millisecond)

	})

	AfterEach(func() {
		speaker.ClearMockSpeakerStore()
		cancel()
		time.Sleep(10 * time.Millisecond)
	})

	It("should create NodeBGP resources when Node found", func() {
		By("listing NodeBGP resources")
		nodeBGPList := &sartv1alpha1.NodeBGPList{}
		Eventually(func() error {
			err := k8sClient.List(ctx, nodeBGPList)
			if err != nil {
				return err
			}
			if len(nodeBGPList.Items) < 2 {
				return fmt.Errorf("expected number of resources is 2")
			}
			return nil
		}).Should(Succeed())

		By("getting node1's NodeBGP")
		nodeBgp1 := &sartv1alpha1.NodeBGP{}
		err := k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "node1"}, nodeBgp1)
		Expect(err).NotTo(HaveOccurred())
		Expect(nodeBgp1.Spec.Asn).To(Equal(uint32(65000)))

		node1 := &corev1.Node{}
		err = k8sClient.Get(ctx, types.NamespacedName{Name: "node1"}, node1)
		Expect(err).NotTo(HaveOccurred())
		node1Addr, ok := getNodeInternalIp(node1)
		Expect(ok).To(BeTrue())
		Expect(nodeBgp1.Spec.RouterId).To(Equal(node1Addr))

		By("getting node2's NodeBGP")
		nodeBgp2 := &sartv1alpha1.NodeBGP{}
		err = k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "node2"}, nodeBgp2)
		Expect(err).NotTo(HaveOccurred())
		Expect(nodeBgp2.Spec.Asn).To(Equal(uint32(65000)))

		node2 := &corev1.Node{}
		err = k8sClient.Get(ctx, types.NamespacedName{Name: "node2"}, node2)
		Expect(err).NotTo(HaveOccurred())
		node2Addr, ok := getNodeInternalIp(node2)
		Expect(ok).To(BeTrue())
		Expect(nodeBgp2.Spec.RouterId).To(Equal(node2Addr))

		// create new node
		By("adding new corev1.Node")
		node3 := &corev1.Node{}
		node3.Name = "node3"
		node3.SetLabels(map[string]string{
			constants.LabelKeyAsn: "65000",
		})
		node3.Status.Addresses = []corev1.NodeAddress{
			{
				Type:    corev1.NodeInternalIP,
				Address: "10.0.0.3",
			},
		}
		err = k8sClient.Create(ctx, node3)
		Expect(err).NotTo(HaveOccurred())

		nodeBgp3 := &sartv1alpha1.NodeBGP{}
		Eventually(func() error {
			err := k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "node3"}, nodeBgp3)
			if err != nil {
				return err
			}
			return nil
		}).Should(Succeed())

		Expect(nodeBgp3.Spec.Asn).To(Equal(uint32(65000)))
		Expect(err).NotTo(HaveOccurred())
		node3Addr, ok := getNodeInternalIp(node3)
		Expect(ok).To(BeTrue())
		Expect(nodeBgp3.Spec.RouterId).To(Equal(node3Addr))

		By("deleting a node")
		err = k8sClient.Delete(ctx, node3)
		Expect(err).NotTo(HaveOccurred())

		Eventually(func() error {
			deleted := &sartv1alpha1.NodeBGP{}
			err := k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "node3"}, deleted)
			if err != nil {
				if apierrors.IsNotFound(err) {
					return nil
				}
				return err
			}
			return fmt.Errorf("found")
		}).Should(Succeed())
	})
})
