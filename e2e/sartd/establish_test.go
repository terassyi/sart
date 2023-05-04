package e2e

import (
	"context"
	"encoding/json"
	"fmt"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
	"github.com/terassyi/sart/e2e/container"
)

func testEstablish() {
	It("should establish bgp peer automatically", func() {
		ctx := context.Background()

		defer func() {
			err := simpleTopology.Remove(ctx)
			Expect(err).NotTo(HaveOccurred())
		}()

		By("creating topology and configuring")
		err := simpleTopology.Build(ctx)
		Expect(err).NotTo(HaveOccurred())

		By("getting GoBGP peer state with node2")
		Eventually(func(g Gomega) error {
			res, err := container.RunCommandInContainerWithOutput(ctx, "node2", []string{"gobgp", "neighbor", "10.0.0.2", "-j"})
			if err != nil {
				return err
			}
			node2State := &GoBGPNeighbor{}
			if err := json.Unmarshal(res, node2State); err != nil {
				return err
			}
			if node2State.State.SessionState != 6 {
				return fmt.Errorf("Session is not Established")
			}
			return nil
		}, "5m").WithPolling(1 * time.Second).Should(Succeed())

		By("getting GoBGP peer state with node3")
		Eventually(func(g Gomega) error {
			res, err := container.RunCommandInContainerWithOutput(ctx, "node3", []string{"gobgp", "neighbor", "10.0.1.2", "-j"})
			if err != nil {
				return err
			}
			node3State := &GoBGPNeighbor{}
			if err := json.Unmarshal(res, node3State); err != nil {
				return err
			}
			if node3State.State.SessionState != 6 {
				return fmt.Errorf("Session is not Established")
			}
			return nil
		}, "5m").WithPolling(1 * time.Second).Should(Succeed())

	})
	It("should establish bgp peer manually", func() {
		ctx := context.Background()

		defer func() {
			err := simpleTopologyWithoutConfig.Remove(ctx)
			Expect(err).NotTo(HaveOccurred())
		}()

		By("creating topology")
		err := simpleTopologyWithoutConfig.Build(ctx)
		Expect(err).NotTo(HaveOccurred())

		By("configuring global settings")
		err = container.RunCommandInContainer(ctx, "node1", false, []string{"sart", "bgp", "global", "set", "--asn", "65000", "--router-id", "10.0.0.2"})
		Expect(err).NotTo(HaveOccurred())

		By("adding peer settings")
		err = container.RunCommandInContainer(ctx, "node1", false, []string{"sart", "bgp", "neighbor", "add", "10.0.0.3", "65100"})
		Expect(err).NotTo(HaveOccurred())
		err = container.RunCommandInContainer(ctx, "node1", false, []string{"sart", "bgp", "neighbor", "add", "10.0.1.3", "65200"})
		Expect(err).NotTo(HaveOccurred())

		By("getting GoBGP peer state with node2")
		Eventually(func(g Gomega) error {
			res, err := container.RunCommandInContainerWithOutput(ctx, "node2", []string{"gobgp", "neighbor", "10.0.0.2", "-j"})
			if err != nil {
				return err
			}
			node2State := &GoBGPNeighbor{}
			if err := json.Unmarshal(res, node2State); err != nil {
				return err
			}
			if node2State.State.SessionState != 6 {
				return fmt.Errorf("Session is not Established")
			}
			return nil
		}, "5m").WithPolling(1 * time.Second).Should(Succeed())

		By("getting GoBGP peer state with node3")
		Eventually(func(g Gomega) error {
			res, err := container.RunCommandInContainerWithOutput(ctx, "node3", []string{"gobgp", "neighbor", "10.0.1.2", "-j"})
			if err != nil {
				return err
			}
			node3State := &GoBGPNeighbor{}
			if err := json.Unmarshal(res, node3State); err != nil {
				return err
			}
			if node3State.State.SessionState != 6 {
				return fmt.Errorf("Session is not Established")
			}
			return nil
		}, "5m").WithPolling(1 * time.Second).Should(Succeed())

	})
}
