package v1alpha1

import (
	"context"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
)

var _ = Describe("NodeBGP webhook", func() {
	ctx := context.Background()

	It("should deny to create NodeBGP resources without ownerReference", func() {
		nodeBgp := NodeBGP{
			ObjectMeta: metav1.ObjectMeta{
				Name:      "no-owner-reference",
				Namespace: "kube-system",
			},
			Spec: NodeBGPSpec{
				Asn:      6000,
				RouterId: "1.1.1.1",
				Endpoint: "localhost",
			},
		}
		err := k8sClient.Create(ctx, &nodeBgp)
		Expect(err).To(HaveOccurred())
	})

	It("should accept to create NodeBGP with ownerReference", func() {
		nodeBgp := NodeBGP{
			ObjectMeta: metav1.ObjectMeta{
				Name:      "owner-reference",
				Namespace: "kube-system",
				OwnerReferences: []metav1.OwnerReference{
					{
						APIVersion: "v1",
						Kind:       "Node",
						Name:       "test",
						UID:        "test",
					},
				},
			},
			Spec: NodeBGPSpec{
				Asn:      6000,
				RouterId: "1.1.1.1",
				Endpoint: "localhost",
			},
		}
		err := k8sClient.Create(ctx, &nodeBgp)
		Expect(err).NotTo(HaveOccurred())

	})

})
