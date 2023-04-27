package controllers

import (
	"context"
	"fmt"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
	sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	"github.com/terassyi/sart/controller/pkg/speaker"
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
		cancel()
		time.Sleep(10 * time.Millisecond)
	})

	It("should create NodeBGP resources when Node found", func() {
		By("listing NodeBGP resources")
		Eventually(func() error {
			nodeBGPList := &sartv1alpha1.NodeBGPList{}
			err := k8sClient.List(ctx, nodeBGPList)
			if err != nil {
				return err
			}
			if len(nodeBGPList.Items) != 2 {
				return fmt.Errorf("expected number of resources is 2")
			}
			return nil
		})

	})
})
