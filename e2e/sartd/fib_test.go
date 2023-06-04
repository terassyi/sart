package main

import (
	"context"
	"encoding/json"
	"fmt"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
	"github.com/terassyi/sart/e2e/container"
	"github.com/terassyi/sart/e2e/gobgp"
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
			node2State := &gobgp.GoBGPNeighbor{}
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
			node3State := &gobgp.GoBGPNeighbor{}
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
		pathMap := make(map[string][]gobgp.GoBGPPath)
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

		By("deleting the path from node3")
		err = container.RunCommandInContainer(ctx, "node2", false, []string{"gobgp", "global", "rib", "del", "all"})
		Expect(err).NotTo(HaveOccurred())

		Eventually(func() error {
			if err := container.RunCommandInContainer(ctx, "node3", false, []string{"ping", "-c", "1", "10.0.10.3"}); err != nil {
				return nil
			}
			return fmt.Errorf("should not be able to pass ping")
		}).Should(Succeed())

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
			node2State := &gobgp.GoBGPNeighbor{}
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
			node3State := &gobgp.GoBGPNeighbor{}
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
			pathMap := make(map[string][]gobgp.GoBGPPath)
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
			pathMap := make(map[string][]gobgp.GoBGPPath)
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

	It("should install and uninstall received paths in another table", func() {
		ctx := context.Background()

		defer simpleTopologyWithClientAnotherTable.Remove(ctx)

		By("creating topology")
		err := simpleTopologyWithClientAnotherTable.Build(ctx)
		Expect(err).NotTo(HaveOccurred())

		By("getting GoBGP peer state with node2")
		Eventually(func(g Gomega) error {
			res, err := container.RunCommandInContainerWithOutput(ctx, "node2", []string{"gobgp", "neighbor", "10.0.0.2", "-j"})
			if err != nil {
				return err
			}
			node2State := &gobgp.GoBGPNeighbor{}
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
			node3State := &gobgp.GoBGPNeighbor{}
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
		pathMap := make(map[string][]gobgp.GoBGPPath)
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

	It("should subscribe routes from the kernel route table and publish it", func() {
		ctx := context.Background()

		defer simpleTopologyWithClientSubscribeKernel.Remove(ctx)

		By("creating topology")
		err := simpleTopologyWithClientSubscribeKernel.Build(ctx)
		Expect(err).NotTo(HaveOccurred())

		By("getting GoBGP peer state with node2")
		Eventually(func(g Gomega) error {
			res, err := container.RunCommandInContainerWithOutput(ctx, "node2", []string{"gobgp", "neighbor", "10.0.0.2", "-j"})
			if err != nil {
				return err
			}
			node2State := &gobgp.GoBGPNeighbor{}
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
			node3State := &gobgp.GoBGPNeighbor{}
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
		pathMap := make(map[string][]gobgp.GoBGPPath)
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

		By("subscribing advertisable routes from other process")
		err = container.RunCommandInContainer(ctx, "node1", false, []string{"ip", "route", "add", "5.5.5.5/32", "via", "10.0.11.2", "table", "100"})
		Expect(err).NotTo(HaveOccurred())

		By("checking that the subscribed route is advertised to other peers")
		Eventually(func() error {
			res, err = container.RunCommandInContainerWithOutput(ctx, "node2", []string{"gobgp", "global", "rib", "-a", "ipv4", "-j"})
			if err != nil {
				return err
			}
			pathMap = make(map[string][]gobgp.GoBGPPath)
			err = json.Unmarshal(res, &pathMap)
			if err != nil {
				return err
			}

			paths, ok = pathMap["5.5.5.5/32"]
			if !ok {
				return fmt.Errorf("5.5.5.5/32 must be registered")
			}
			if len(paths) != 1 {
				return fmt.Errorf("expected path len is 1, but actual: %d", len(paths))
			}
			if paths[0].Source != "10.0.0.2" {
				return fmt.Errorf("expected source is 10.0.0.2, but actual: %s", paths[0].Source)
			}
			return nil
		}, "1m").WithPolling(1 * time.Second).Should(Succeed())

		By("communicating with node5")
		err = container.RunCommandInContainer(ctx, "node4", false, []string{"ping", "-c", "1", "5.5.5.5"})
		Expect(err).NotTo(HaveOccurred())

		By("subscribing the deleted route by other process")
		err = container.RunCommandInContainer(ctx, "node1", false, []string{"ip", "route", "del", "5.5.5.5/32", "table", "100"})
		Expect(err).NotTo(HaveOccurred())

		By("checking that the subscribed route is advertised to other peers")
		Eventually(func() error {
			res, err = container.RunCommandInContainerWithOutput(ctx, "node2", []string{"gobgp", "global", "rib", "-a", "ipv4", "-j"})
			if err != nil {
				return err
			}
			pathMap = make(map[string][]gobgp.GoBGPPath)
			err = json.Unmarshal(res, &pathMap)
			if err != nil {
				return err
			}

			paths, ok = pathMap["5.5.5.5/32"]
			if ok {
				return fmt.Errorf("5.5.5.5/32 must be deleted")
			}
			return nil
		}, "1m").WithPolling(1 * time.Second).Should(Succeed())

		By("communicating with node5")
		err = container.RunCommandInContainer(ctx, "node4", false, []string{"ping", "-c", "1", "5.5.5.5"})
		Expect(err).To(HaveOccurred())
	})

}
