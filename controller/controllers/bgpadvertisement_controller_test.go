package controllers

import (
	"testing"

	"github.com/stretchr/testify/assert"
	sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	v1 "k8s.io/apimachinery/pkg/apis/meta/v1"
)

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
						ObjectMeta: v1.ObjectMeta{Name: "peer1"},
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
						ObjectMeta: v1.ObjectMeta{Name: "peer2"},
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
						ObjectMeta: v1.ObjectMeta{Name: "peer3"},
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
				ObjectMeta: v1.ObjectMeta{Name: "adv1"},
				Spec: sartv1alpha1.BGPAdvertisementSpec{
					Network: "10.69.0.1/32",
				},
				Status: sartv1alpha1.BGPAdvertisementStatus{
					Nodes:     []string{"node1", "node2"},
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
						ObjectMeta: v1.ObjectMeta{Name: "peer1"},
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
						ObjectMeta: v1.ObjectMeta{Name: "peer2"},
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
						ObjectMeta: v1.ObjectMeta{Name: "peer3"},
						Spec: sartv1alpha1.BGPPeerSpec{
							Node:           "node3",
							Advertisements: []sartv1alpha1.Advertisement{},
						},
					},
				},
			},
			advertisement: &sartv1alpha1.BGPAdvertisement{
				ObjectMeta: v1.ObjectMeta{Name: "adv1"},
				Spec: sartv1alpha1.BGPAdvertisementSpec{
					Network: "10.69.0.1/32",
				},
				Status: sartv1alpha1.BGPAdvertisementStatus{
					Nodes:     []string{"node2", "node3"},
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
