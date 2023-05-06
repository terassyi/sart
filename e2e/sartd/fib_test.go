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

func testFib() {
	It("should install and uninstall received paths", func() {
		ctx := context.Background()

		defer simpleTopologyWithClient.Remove(ctx)

		By("creating topology")
		err := simpleTopologyWithClient.Build(ctx)
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

		By("propagating paths received from peers")
		err = container.RunCommandInContainer(ctx, "node2", false, []string{"gobgp", "global", "rib", "add", "10.0.10.0/24", "-a", "ipv4", "origin", "igp"})
		Expect(err).NotTo(HaveOccurred())
		err = container.RunCommandInContainer(ctx, "node2", false, []string{"gobgp", "global", "rib", "add", "10.0.0.0/24", "-a", "ipv4", "origin", "igp"})
		Expect(err).NotTo(HaveOccurred())
		err = container.RunCommandInContainer(ctx, "node3", false, []string{"gobgp", "global", "rib", "add", "10.0.1.0/24", "-a", "ipv4", "origin", "igp"})
		Expect(err).NotTo(HaveOccurred())

		res, err := container.RunCommandInContainerWithOutput(ctx, "node3", []string{"gobgp", "global", "rib", "-a", "ipv4", "-j"})
		Expect(err).NotTo(HaveOccurred())
		pathMap := make(map[string][]GoBGPPath)
		err = json.Unmarshal(res, &pathMap)
		Expect(err).NotTo(HaveOccurred())

		paths, ok := pathMap["10.0.1.0/24"]
		Expect(ok).To(BeTrue())
		Expect(len(paths)).To(Equal(1))

		paths, ok = pathMap["10.0.10.0/24"]
		Expect(ok).To(BeTrue())
		Expect(len(paths)).To(Equal(1))

		Expect(paths[0].Source).To(Equal("10.0.0.2"))

		By("checking connectivity to node4")
		err = container.RunCommandInContainer(ctx, "node3", false, []string{"ping", "-c", "1", "10.0.10.3"})
		Expect(err).NotTo(HaveOccurred())

		By("checking connectivity to node5")
		err = container.RunCommandInContainer(ctx, "node1", false, []string{"sart", "bgp", "global", "rib", "add", "10.0.11.0/24"})
		Expect(err).NotTo(HaveOccurred())
	})

	It("should install and uninstall multi path information", func() {
		ctx := context.Background()

		defer multiPathSimpleWithZebra.Remove(ctx)

		By("creating topology")
		err := multiPathSimpleWithZebra.Build(ctx)
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

		By("advertising paths")
		err = container.RunCommandInContainer(ctx, "node4", false, []string{"gobgp", "global", "rib", "add", "10.0.3.0/24", "-a", "ipv4", "origin", "igp"})
		Expect(err).NotTo(HaveOccurred())
		err = container.RunCommandInContainer(ctx, "node1", false, []string{"sart", "bgp", "global", "rib", "add", "10.0.11.0/24"})
		Expect(err).NotTo(HaveOccurred())

		Eventually(func() error {
			res, err := container.RunCommandInContainerWithOutput(ctx, "node2", []string{"gobgp", "global", "rib", "-a", "ipv4", "-j"})
			if err != nil {
				return err
			}
			pathMap := make(map[string][]GoBGPPath)
			err = json.Unmarshal(res, &pathMap)
			if err != nil {
				return err
			}

			paths, ok := pathMap["10.0.3.0/24"]
			if !ok {
				return fmt.Errorf("expected true")
			}
			if len(paths) < 1 {
				return fmt.Errorf("one path required")
			}
			return nil
		}, "5m").WithPolling(1 * time.Second).Should(Succeed())

		Eventually(func() error {
			res, err := container.RunCommandInContainerWithOutput(ctx, "node3", []string{"gobgp", "global", "rib", "-a", "ipv4", "-j"})
			if err != nil {
				return err
			}
			pathMap := make(map[string][]GoBGPPath)
			err = json.Unmarshal(res, &pathMap)
			if err != nil {
				return err
			}
			paths, ok := pathMap["10.0.3.0/24"]
			if !ok {
				return fmt.Errorf("expected true")
			}
			if len(paths) < 1 {
				return fmt.Errorf("one path required")
			}
			return nil
		}, "5m").WithPolling(1 * time.Second).Should(Succeed())

		By("checking connectivity to node4")
		err = container.RunCommandInContainer(ctx, "node5", false, []string{"ping", "-c", "1", "10.0.3.3"})
		Expect(err).NotTo(HaveOccurred())
	})
}
