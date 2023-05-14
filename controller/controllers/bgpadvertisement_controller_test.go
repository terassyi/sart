package controllers

import (
	"context"
	"fmt"
	"testing"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
	"github.com/stretchr/testify/assert"
	sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	"github.com/terassyi/sart/controller/pkg/constants"
	"github.com/terassyi/sart/controller/pkg/speaker"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	corev1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/types"
	ctrl "sigs.k8s.io/controller-runtime"
)

var (
	peer1 = &sartv1alpha1.BGPPeer{
		ObjectMeta: corev1.ObjectMeta{
			Name:      "test-peer1",
			Namespace: constants.Namespace,
		},
		Spec: sartv1alpha1.BGPPeerSpec{
			PeerAsn:       65000,
			PeerRouterId:  "10.0.0.100",
			LocalAsn:      65000,
			LocalRouterId: "10.0.0.1",
			Node:          "node1",
			Family: sartv1alpha1.AddressFamily{
				Afi:  "ipv4",
				Safi: "unicast",
			},
		},
	}
	peer2 = &sartv1alpha1.BGPPeer{
		ObjectMeta: corev1.ObjectMeta{
			Name:      "test-peer2",
			Namespace: constants.Namespace,
		},
		Spec: sartv1alpha1.BGPPeerSpec{
			PeerAsn:       65000,
			PeerRouterId:  "10.0.0.100",
			LocalAsn:      65000,
			LocalRouterId: "10.0.0.2",
			Node:          "node2",
			Family: sartv1alpha1.AddressFamily{
				Afi:  "ipv4",
				Safi: "unicast",
			},
		},
	}
	peer3 = &sartv1alpha1.BGPPeer{
		ObjectMeta: corev1.ObjectMeta{
			Name:      "test-peer3",
			Namespace: constants.Namespace,
		},
		Spec: sartv1alpha1.BGPPeerSpec{
			PeerAsn:       65000,
			PeerRouterId:  "10.0.0.100",
			LocalAsn:      65000,
			LocalRouterId: "10.0.0.3",
			Node:          "node3",
			Family: sartv1alpha1.AddressFamily{
				Afi:  "ipv4",
				Safi: "unicast",
			},
		},
	}
)

var peers = []*sartv1alpha1.BGPPeer{peer1, peer2, peer3}

var _ = Describe("handle BGPAdvertisement", func() {
	ctx := context.Background()
	var cancel context.CancelFunc

	BeforeEach(func() {
		mgr, err := ctrl.NewManager(cfg, ctrl.Options{
			Scheme:             scheme,
			LeaderElection:     false,
			MetricsBindAddress: "0",
		})
		Expect(err).NotTo(HaveOccurred())

		bgpAdvertisementReconciler := &BGPAdvertisementReconciler{
			Client:              k8sClient,
			Scheme:              scheme,
			SpeakerEndpointPort: 5000,
			SpeakerType:         speaker.SpeakerTypeMock,
			advMap:              make(map[string]map[string]bool),
		}
		err = bgpAdvertisementReconciler.SetupWithManager(mgr)
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
		for _, p := range peers {
			err := k8sClient.Delete(ctx, p)
			Expect(err).NotTo(HaveOccurred())
		}

		speaker.ClearMockSpeakerStore()

		cancel()
		time.Sleep(10 * time.Millisecond)
	})

	It("should handle BGPAdvertisement to one BGPPeer", func() {
		By("creating BGPAdvertisement")
		adv := &sartv1alpha1.BGPAdvertisement{
			ObjectMeta: corev1.ObjectMeta{
				Name:      "adv1",
				Namespace: constants.Namespace,
			},
			Spec: sartv1alpha1.BGPAdvertisementSpec{
				Network:  "10.0.10.1/32",
				Type:     "service",
				Protocol: "ipv4",
				Nodes: []string{
					"node1",
				},
			},
		}
		err := k8sClient.Create(ctx, adv)
		Expect(err).NotTo(HaveOccurred())

		By("checking status is available")
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: adv.Namespace, Name: adv.Name}, adv); err != nil {
				return err
			}
			if adv.Status.Condition != sartv1alpha1.BGPAdvertisementConditionAvailable {
				return fmt.Errorf("available is expected")
			}
			return nil
		}).Should(Succeed())

		By("peer has advertised path information")

		peer := &sartv1alpha1.BGPPeer{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: peer1.Namespace, Name: peer1.Name}, peer); err != nil {
				return err
			}
			if len(peer.Spec.Advertisements) < 1 {
				return fmt.Errorf("want: 1, got: %d", len(peer.Spec.Advertisements))
			}
			return nil
		}).Should(Succeed())

		By("checking advertised to peer")
		Eventually(func() error {
			if _, err := speaker.GetMockSpeakerPath(fmt.Sprintf("%s:5000", peer1.Spec.LocalRouterId), "10.0.10.1/32"); err != nil {
				return err
			}
			return nil
		}).Should(Succeed())
	})

	It("should handle BGPAdvertisement to all BGPPeer", func() {
		By("creating BGPAdvertisement")
		adv := &sartv1alpha1.BGPAdvertisement{
			ObjectMeta: corev1.ObjectMeta{
				Name:      "adv2",
				Namespace: constants.Namespace,
			},
			Spec: sartv1alpha1.BGPAdvertisementSpec{
				Network:  "10.0.10.2/32",
				Type:     "service",
				Protocol: "ipv4",
				Nodes: []string{
					"node1",
					"node2",
					"node3",
				},
			},
		}
		err := k8sClient.Create(ctx, adv)
		Expect(err).NotTo(HaveOccurred())

		By("checking status is available")
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: adv.Namespace, Name: adv.Name}, adv); err != nil {
				return err
			}
			if adv.Status.Condition != sartv1alpha1.BGPAdvertisementConditionAvailable {
				return fmt.Errorf("available is expected")
			}
			return nil
		}).Should(Succeed())

		By("peer has advertised path information")

		peer := &sartv1alpha1.BGPPeer{}
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: peer1.Namespace, Name: peer1.Name}, peer); err != nil {
				return err
			}
			if len(peer.Spec.Advertisements) < 1 {
				return fmt.Errorf("want: 1, got: %d", len(peer.Spec.Advertisements))
			}
			return nil
		}).Should(Succeed())

		By("checking advertised to peer")
		Eventually(func() error {
			if _, err := speaker.GetMockSpeakerPath(fmt.Sprintf("%s:5000", peer1.Spec.LocalRouterId), "10.0.10.2/32"); err != nil {
				return err
			}
			return nil
		}).Should(Succeed())
	})

	It("should withdraw the advertisement", func() {
		By("creating BGPAdvertisement")
		adv := &sartv1alpha1.BGPAdvertisement{
			ObjectMeta: corev1.ObjectMeta{
				Name:      "adv3",
				Namespace: constants.Namespace,
			},
			Spec: sartv1alpha1.BGPAdvertisementSpec{
				Network:  "10.0.10.3/32",
				Type:     "service",
				Protocol: "ipv4",
				Nodes: []string{
					"node1",
					"node2",
				},
			},
		}
		err := k8sClient.Create(ctx, adv)
		Expect(err).NotTo(HaveOccurred())

		By("getting and the advertisement and checking its status is available")
		adv = &sartv1alpha1.BGPAdvertisement{}
		Eventually(func() error {
			err = k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "adv3"}, adv)
			if err != nil {
				return err
			}
			if adv.Status.Condition != sartv1alpha1.BGPAdvertisementConditionAvailable {
				return fmt.Errorf("status is not available")
			}
			return nil
		}).Should(Succeed())

		By("withdrawing the advertisement")
		err = k8sClient.Delete(ctx, adv)
		Expect(err).NotTo(HaveOccurred())
		Eventually(func() error {
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: adv.Namespace, Name: adv.Name}, adv); err != nil {
				if apierrors.IsNotFound(err) {
					return nil
				}
				return err
			}
			return fmt.Errorf("should not exist")
		}).Should(Succeed())

		By("checking the advertisement is deleted from peer")
		Eventually(func() error {
			p1 := &sartv1alpha1.BGPPeer{}
			p2 := &sartv1alpha1.BGPPeer{}
			err := k8sClient.Get(ctx, types.NamespacedName{Namespace: peer1.Namespace, Name: peer1.Name}, p1)
			if err != nil {
				return err
			}
			err = k8sClient.Get(ctx, types.NamespacedName{Namespace: peer2.Namespace, Name: peer2.Name}, p2)
			if err != nil {
				return err
			}
			Expect(err).NotTo(HaveOccurred())

			contain := false
			for _, p := range []*sartv1alpha1.BGPPeer{p1, p2} {
				for _, a := range p.Spec.Advertisements {
					if a.Namespace == adv.Namespace && a.Name == adv.Name {
						contain = true
						break
					}
				}
			}
			if contain {
				return fmt.Errorf("all peers shouldn't have it")
			}
			return nil
		}).Should(Succeed())
	})

	It("should update advertising nodes", func() {
		By("creating BGPAdvertisement")
		adv := &sartv1alpha1.BGPAdvertisement{
			ObjectMeta: corev1.ObjectMeta{
				Name:      "adv4",
				Namespace: constants.Namespace,
			},
			Spec: sartv1alpha1.BGPAdvertisementSpec{
				Network:  "10.0.10.4/32",
				Type:     "service",
				Protocol: "ipv4",
				Nodes: []string{
					"node1",
					"node2",
				},
			},
		}
		err := k8sClient.Create(ctx, adv)
		Expect(err).NotTo(HaveOccurred())

		By("getting and the advertisement and checking its status is available")
		adv = &sartv1alpha1.BGPAdvertisement{}
		Eventually(func() error {
			err = k8sClient.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: "adv4"}, adv)
			if err != nil {
				return err
			}
			if adv.Status.Condition != sartv1alpha1.BGPAdvertisementConditionAvailable {
				return fmt.Errorf("status is not advertised")
			}
			return nil
		}).Should(Succeed())

		By("updating advertising nodes")
		adv.Spec.Nodes = []string{
			"node1",
			"node3",
		}
		err = k8sClient.Update(ctx, adv)
		Expect(err).NotTo(HaveOccurred())
		err = k8sClient.Status().Update(ctx, adv)
		Expect(err).NotTo(HaveOccurred())

		By("checking removed from the old node")
		Eventually(func() error {
			p2 := &sartv1alpha1.BGPPeer{}
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: peer2.Namespace, Name: peer2.Name}, p2); err != nil {
				return err
			}
			have := false
			for _, a := range p2.Spec.Advertisements {
				if a.Namespace == adv.Namespace && a.Name == adv.Name {
					have = true
					break
				}
			}
			if have {
				return fmt.Errorf("peer2 shouldn't have it")
			}
			return nil
		}).Should(Succeed())

		By("checking removed from the new node")
		Eventually(func() error {
			p1 := &sartv1alpha1.BGPPeer{}
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: peer1.Namespace, Name: peer1.Name}, p1); err != nil {
				return err
			}
			have := false
			for _, a := range p1.Spec.Advertisements {
				if a.Namespace == adv.Namespace && a.Name == adv.Name {
					have = true
					break
				}
			}
			if !have {
				return fmt.Errorf("peer1 should have it")
			}
			p3 := &sartv1alpha1.BGPPeer{}
			if err := k8sClient.Get(ctx, types.NamespacedName{Namespace: peer3.Namespace, Name: peer3.Name}, p3); err != nil {
				return err
			}
			have = false
			for _, a := range p3.Spec.Advertisements {
				if a.Namespace == adv.Namespace && a.Name == adv.Name {
					have = true
					break
				}
			}
			if !have {
				return fmt.Errorf("peer3 should have it")
			}
			return nil
		}).Should(Succeed())
	})
})

func TestAdvDiff(t *testing.T) {
	t.Parallel()
	for _, tt := range []struct {
		name          string
		peerList      sartv1alpha1.BGPPeerList
		advertisement *sartv1alpha1.BGPAdvertisement
		added         []string
		removed       []string
	}{
		{
			name: "case1",
			peerList: sartv1alpha1.BGPPeerList{
				Items: []sartv1alpha1.BGPPeer{
					{
						ObjectMeta: corev1.ObjectMeta{Name: "peer1"},
						Spec: sartv1alpha1.BGPPeerSpec{
							Node: "node1",
							Advertisements: []sartv1alpha1.Advertisement{
								{
									Name:   "adv1",
									Prefix: "10.69.0.1/32",
								},
							},
						},
					},
					{
						ObjectMeta: corev1.ObjectMeta{Name: "peer2"},
						Spec: sartv1alpha1.BGPPeerSpec{
							Node: "node2",
							Advertisements: []sartv1alpha1.Advertisement{
								{
									Name:   "adv1",
									Prefix: "10.69.0.1/32",
								},
							},
						},
					},
					{
						ObjectMeta: corev1.ObjectMeta{Name: "peer3"},
						Spec: sartv1alpha1.BGPPeerSpec{
							Node: "node3",
							Advertisements: []sartv1alpha1.Advertisement{
								{
									Name:   "adv1",
									Prefix: "10.69.0.1/32",
								},
							},
						},
					},
				},
			},
			advertisement: &sartv1alpha1.BGPAdvertisement{
				ObjectMeta: corev1.ObjectMeta{Name: "adv1"},
				Spec: sartv1alpha1.BGPAdvertisementSpec{
					Network: "10.69.0.1/32",
					Nodes:   []string{"node1", "node2"},
				},
				Status: sartv1alpha1.BGPAdvertisementStatus{
					Condition: sartv1alpha1.BGPAdvertisementConditionAvailable,
				},
			},
			added:   []string{},
			removed: []string{"peer3"},
		},
		{
			name: "case2",
			peerList: sartv1alpha1.BGPPeerList{
				Items: []sartv1alpha1.BGPPeer{
					{
						ObjectMeta: corev1.ObjectMeta{Name: "peer1"},
						Spec: sartv1alpha1.BGPPeerSpec{
							Node: "node1",
							Advertisements: []sartv1alpha1.Advertisement{
								{
									Name:   "adv1",
									Prefix: "10.69.0.1/32",
								},
							},
						},
					},
					{
						ObjectMeta: corev1.ObjectMeta{Name: "peer2"},
						Spec: sartv1alpha1.BGPPeerSpec{
							Node: "node2",
							Advertisements: []sartv1alpha1.Advertisement{
								{
									Name:   "adv1",
									Prefix: "10.69.0.1/32",
								},
							},
						},
					},
					{
						ObjectMeta: corev1.ObjectMeta{Name: "peer3"},
						Spec: sartv1alpha1.BGPPeerSpec{
							Node:           "node3",
							Advertisements: []sartv1alpha1.Advertisement{},
						},
					},
				},
			},
			advertisement: &sartv1alpha1.BGPAdvertisement{
				ObjectMeta: corev1.ObjectMeta{Name: "adv1"},
				Spec: sartv1alpha1.BGPAdvertisementSpec{
					Network: "10.69.0.1/32",
					Nodes:   []string{"node2", "node3"},
				},
				Status: sartv1alpha1.BGPAdvertisementStatus{
					Condition: sartv1alpha1.BGPAdvertisementConditionAvailable,
				},
			},
			added:   []string{"peer3"},
			removed: []string{"peer1"},
		},
	} {
		tt := tt
		t.Run(tt.name, func(t *testing.T) {
			addedP, removedP := advDiff(tt.peerList, tt.advertisement)
			added := []string{}
			removed := []string{}
			for _, a := range addedP {
				added = append(added, a.Name)
			}
			for _, r := range removedP {
				removed = append(removed, r.Name)
			}
			assert.Equal(t, tt.added, added)
			assert.Equal(t, tt.removed, removed)
		})
	}

}
