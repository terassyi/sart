package v1alpha1

import (
	"context"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
)

var _ = Describe("BGPAdvertisement webhook", func() {
	ctx := context.Background()

	It("should accept to create BGPAdvertisement with ownerReference", func() {
		adv := BGPAdvertisement{
			ObjectMeta: metav1.ObjectMeta{
				Name:      "test",
				Namespace: "default",
				OwnerReferences: []metav1.OwnerReference{
					{
						APIVersion: "v1",
						Kind:       "Service",
						Name:       "test-svc",
						UID:        "test",
					},
				},
			},
			Spec: BGPAdvertisementSpec{
				Network:   "10.0.0.0/32",
				Type:      "service",
				Protocol:  "ipv4",
				Origin:    "igp",
				LocalPref: 100,
				Nodes:     []string{"node1"},
			},
		}
		err := k8sClient.Create(ctx, &adv)
		Expect(err).NotTo(HaveOccurred())
	})

	It("should deny to create BGPAdvertisement with ownerReference", func() {
		adv := BGPAdvertisement{
			ObjectMeta: metav1.ObjectMeta{
				Name:      "test2",
				Namespace: "default",
			},
			Spec: BGPAdvertisementSpec{
				Network:   "10.0.0.0/32",
				Type:      "service",
				Protocol:  "ipv4",
				Origin:    "igp",
				LocalPref: 100,
				Nodes:     []string{"node1"},
			},
		}
		err := k8sClient.Create(ctx, &adv)
		Expect(err).To(HaveOccurred())
	})
})
