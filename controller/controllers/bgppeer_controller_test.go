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

var peerNames = []string{"peer1", "peer2", "peer3"}

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
		// speaker.ClearMockSpeakerStore()
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

		By("checking registering the peer")
		sp, err := speaker.GetMockSpeakerPeer(fmt.Sprintf("%s:5000", peer.Spec.LocalRouterId), peer.Spec.PeerRouterId)
		Expect(err).NotTo(HaveOccurred())
		Expect(sp).To(Equal(&speaker.PeerInfo{
			LocalAsn:      65000,
			LocalRouterId: "10.0.0.1",
			PeerAsn:       65000,
			PeerRouterId:  "10.0.0.100",
			Protocol:      "ipv4",
			State:         speaker.PeerStateIdle,
		}))

		By("checking that NodeBGP has peer information")
		Eventually(func() error {
			nb := &sartv1alpha1.NodeBGP{}
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "node1"}, nb); err != nil {
				return err
			}
			exist := false
			for _, nbp := range nb.Spec.Peers {
				if nbp.Asn == peer.Spec.PeerAsn && nbp.RouterId == peer.Spec.PeerRouterId {
					exist = true
					break
				}
			}
			if !exist {
				return fmt.Errorf("not exist")
			}
			return nil
		}).Should(Succeed())

		By("changing peer state to Established")
		// state transition may not be work well, some time.
		// TODO: fix to watch state changes.
		sp.State = speaker.PeerStateOpenConfirm
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "peer1"}, peer); err != nil {
				return err
			}
			if peer.Status != sartv1alpha1.BGPPeerStatus(speaker.PeerStateOpenConfirm) {
				return fmt.Errorf("OpenConfirm is expected: actual %v", peer.Status)
			}
			return nil
		}).Should(Succeed())
		sp.State = speaker.PeerStateOpenConfirm
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "peer1"}, peer); err != nil {
				return err
			}
			if peer.Status != sartv1alpha1.BGPPeerStatus(speaker.PeerStateOpenConfirm) {
				return fmt.Errorf("OpenConfirm is expected: actual %v", peer.Status)
			}
			return nil
		}).Should(Succeed())

		sp.State = speaker.PeerStateEstablished
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "peer1"}, peer); err != nil {
				return err
			}
			if peer.Status != sartv1alpha1.BGPPeerStatus(speaker.PeerStateEstablished) {
				return fmt.Errorf("Established is expected")
			}
			return nil
		}).Should(Succeed())

	})

	It("should create BGPPeer not specified Node", func() {
		By("creating a BGPPeer resource")
		peer := &sartv1alpha1.BGPPeer{
			ObjectMeta: corev1.ObjectMeta{
				Name:      "peer2",
				Namespace: constants.Namespace,
			},
			Spec: sartv1alpha1.BGPPeerSpec{
				PeerAsn:       65000,
				PeerRouterId:  "10.0.0.100",
				LocalAsn:      65000,
				LocalRouterId: "10.0.0.2", // node2
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
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "peer2"}, peer); err != nil {
				return err
			}
			if peer.Spec.Node == "node2" {
				return nil
			}
			return fmt.Errorf("local info is not matched")
		}).Should(Succeed())

		Expect(peer.Status).To(Equal(sartv1alpha1.BGPPeerStatusIdle))

		By("checking registering the peer")
		sp, err := speaker.GetMockSpeakerPeer(fmt.Sprintf("%s:5000", peer.Spec.LocalRouterId), peer.Spec.PeerRouterId)
		Expect(err).NotTo(HaveOccurred())
		Expect(sp).To(Equal(&speaker.PeerInfo{
			LocalAsn:      65000,
			LocalRouterId: "10.0.0.2",
			PeerAsn:       65000,
			PeerRouterId:  "10.0.0.100",
			Protocol:      "ipv4",
			State:         speaker.PeerStateIdle,
		}))

		By("changing peer state to Established")
		sp.State = speaker.PeerStateOpenSent
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "peer2"}, peer); err != nil {
				return err
			}
			if peer.Status != sartv1alpha1.BGPPeerStatus(speaker.PeerStateOpenSent) {
				return fmt.Errorf("OpenSent is expected: actual %v", peer.Status)
			}
			return nil
		}).Should(Succeed())

		sp.State = speaker.PeerStateEstablished
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "peer2"}, peer); err != nil {
				return err
			}
			if peer.Status != sartv1alpha1.BGPPeerStatus(speaker.PeerStateEstablished) {
				return fmt.Errorf("Established is expected")
			}
			return nil
		}).Should(Succeed())
	})

	It("should create multiple BGPPeer in same NodeBGP", func() {
		By("checking the existence of peer1")
		peer1 := &sartv1alpha1.BGPPeer{}
		err := k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "peer1"}, peer1)
		Expect(err).NotTo(HaveOccurred())

		By("creating peer3 to node1")
		peer3 := &sartv1alpha1.BGPPeer{
			ObjectMeta: corev1.ObjectMeta{
				Name:      "peer3",
				Namespace: constants.Namespace,
			},
			Spec: sartv1alpha1.BGPPeerSpec{
				PeerAsn:      65000,
				PeerRouterId: "10.0.0.101",
				Node:         "node1",
				Family: sartv1alpha1.AddressFamily{
					Afi:  "ipv4",
					Safi: "unicast",
				},
			},
		}
		err = k8sClient.Create(ctx, peer3)
		Expect(err).NotTo(HaveOccurred())

		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "peer3"}, peer3); err != nil {
				return err
			}
			return nil
		}).Should(Succeed())

		By("completing local information")
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "peer3"}, peer3); err != nil {
				return err
			}
			if peer3.Spec.LocalAsn == 65000 && peer3.Spec.LocalRouterId == "10.0.0.1" {
				return nil
			}
			return fmt.Errorf("local info is not matched")
		}).Should(Succeed())

		By("checking NodeBGP has two peers")
		Eventually(func() error {
			nb := &sartv1alpha1.NodeBGP{}
			err = k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "node1"}, nb)
			if err != nil {
				return err
			}
			if len(nb.Spec.Peers) != 2 {
				return fmt.Errorf("2 peers are expected")
			}
			return nil
		}).Should(Succeed())

		By("checking registering the peer")
		sp, err := speaker.GetMockSpeakerPeer(fmt.Sprintf("%s:5000", peer3.Spec.LocalRouterId), peer3.Spec.PeerRouterId)
		Expect(err).NotTo(HaveOccurred())
		Expect(sp).To(Equal(&speaker.PeerInfo{
			LocalAsn:      65000,
			LocalRouterId: "10.0.0.1",
			PeerAsn:       65000,
			PeerRouterId:  "10.0.0.101",
			Protocol:      "ipv4",
			State:         speaker.PeerStateIdle,
		}))
	})

	It("should delete BGPPeer resource correctly", func() {
		By("getting peer3")
		peer := &sartv1alpha1.BGPPeer{}
		err := k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "peer3"}, peer)
		Expect(err).NotTo(HaveOccurred())

		By("deleting peer3")
		err = k8sClient.Delete(ctx, peer)
		Expect(err).NotTo(HaveOccurred())

		By("checking deleted from NodeBGP peers")
		Expect(err).NotTo(HaveOccurred())
		Eventually(func() error {
			nb := &sartv1alpha1.NodeBGP{}
			err = k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "node1"}, nb)
			if err != nil {
				return err
			}
			if len(nb.Spec.Peers) > 1 {
				return fmt.Errorf("expected: 1, actual: %d", len(nb.Spec.Peers))
			}
			return nil
		}).Should(Succeed())
	})
})
