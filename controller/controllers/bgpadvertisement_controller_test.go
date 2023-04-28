package controllers

import (
	"context"
	"testing"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
	"github.com/stretchr/testify/assert"
	sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	"github.com/terassyi/sart/controller/pkg/constants"
	"github.com/terassyi/sart/controller/pkg/speaker"
	corev1 "k8s.io/apimachinery/pkg/apis/meta/v1"
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
		Status: sartv1alpha1.BGPPeerStatusEstablished,
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
		Status: sartv1alpha1.BGPPeerStatusEstablished,
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
		Status: sartv1alpha1.BGPPeerStatusEstablished,
	}
)

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
		err = k8sClient.Create(ctx, peer1)
		Expect(err).NotTo(HaveOccurred())
		err = k8sClient.Create(ctx, peer2)
		Expect(err).NotTo(HaveOccurred())
		err = k8sClient.Create(ctx, peer3)
		Expect(err).NotTo(HaveOccurred())

	})

	AfterEach(func() {
		err := k8sClient.Delete(ctx, peer1)
		Expect(err).NotTo(HaveOccurred())
		err = k8sClient.Delete(ctx, peer2)
		Expect(err).NotTo(HaveOccurred())
		err = k8sClient.Delete(ctx, peer3)
		Expect(err).NotTo(HaveOccurred())

		speaker.ClearMockSpeakerStore()

		cancel()
		time.Sleep(10 * time.Millisecond)
	})

	It("should handle BGPAdvertisement", func() {

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
					Condition: sartv1alpha1.BGPAdvertisementConditionAdvertising,
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
					Condition: sartv1alpha1.BGPAdvertisementConditionAdvertising,
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
