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
	corev1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/types"
	ctrl "sigs.k8s.io/controller-runtime"
)

var _ = Describe("handle BGPPeer", func() {
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

		nodeBGPReconciler := &NodeBGPReconciler{
			Client:              k8sClient,
			Scheme:              scheme,
			SpeakerEndpointPort: 5000,
			SpeakerType:         speaker.SpeakerTypeMock,
		}
		err = nodeBGPReconciler.SetupWithManager(mgr)
		Expect(err).NotTo(HaveOccurred())
		peerReconciler := &BGPPeerReconciler{
			Client:              k8sClient,
			Scheme:              scheme,
			SpeakerEndpointPort: 5000,
			SpeakerType:         speaker.SpeakerTypeMock,
		}
		err = peerReconciler.SetupWithManager(mgr)
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

	It("should create BGPPeer specified Node", func() {
		By("creating a BGPPeer resource")
		peer := &sartv1alpha1.BGPPeer{
			ObjectMeta: corev1.ObjectMeta{
				Name:      "peer1",
				Namespace: constants.Namespace,
			},
			Spec: sartv1alpha1.BGPPeerSpec{
				PeerAsn:      65000,
				PeerRouterId: "10.0.0.100",
				Node:         "node1",
				Family: sartv1alpha1.AddressFamily{
					Afi:  "ipv4",
					Safi: "unicast",
				},
			},
		}
		err := k8sClient.Create(ctx, peer)
		Expect(err).NotTo(HaveOccurred())

		By("completing local information")
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "peer1"}, peer); err != nil {
				return err
			}
			if peer.Spec.LocalAsn == 65000 && peer.Spec.LocalRouterId == "10.0.0.1" {
				return nil
			}
			return fmt.Errorf("local info is not matched")
		}).Should(Succeed())

		Expect(peer.Status).To(Equal(sartv1alpha1.BGPPeerStatusIdle))

	})

	It("should create BGPPeer not specified Node", func() {

	})

	It("should not create invalid BGPPeer", func() {

	})

})
