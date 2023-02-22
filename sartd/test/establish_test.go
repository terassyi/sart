package test

import (
	"context"
	"os"
	"os/exec"
	"time"

	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

func testEstablish() {
	It("should establish bgp peer acrively", func() {

		ctx, _ := context.WithTimeout(context.Background(), time.Minute)

		cmd := exec.Command("./simple/topology.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		err := cmd.Run()
		Expect(err).NotTo(HaveOccurred())

		By("starting gobgp daemon in netns")
		go func(context.Context) {
			_, _, _, err = execInNetns("spine1", "gobgpd", "-f", "simple/gobgp_spine1.conf")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		go func(context.Context) {
			_, _, _, err = execInNetns("spine2", "gobgpd", "-f", "simple/gobgp_spine2.conf")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		time.Sleep(time.Second)

		By("configuring peers")
		_, _, _, err = execInNetns("spine1", "gobgp", "neighbor", "add", "10.0.0.1", "as", "100")
		Expect(err).NotTo(HaveOccurred())
		_, _, _, err = execInNetns("spine2", "gobgp", "neighbor", "add", "10.0.1.1", "as", "100")
		Expect(err).NotTo(HaveOccurred())

		By("checking configurations")

		Eventually(func(g Gomega) error {
			if err := checkGobgpConfig("spine1", 100); err != nil {
				return err
			}
			if err := checkGobgpConfig("spine2", 100); err != nil {
				return err
			}
			return nil
		}, "1m").Should(Succeed())

		By("starting sartd-bgp")
		go func(context.Context) {
			_, _, _, err := execInNetns("core", "../target/debug/sartd", "bgp", "-f", "simple/config.yaml")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		By("checking to establish peers")
		Eventually(func(g Gomega) error {
			if err := checkEstablished("spine1"); err != nil {
				return err
			}
			if err := checkEstablished("spine2"); err != nil {
				return err
			}
			return nil
		}, "1m").Should(Succeed())

		By("cleaning up the topology")
		cmd = exec.Command("./simple/clean_topology.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		err = cmd.Run()
		Expect(err).NotTo(HaveOccurred())
	})

	It("should establish bgp peer passively", func() {

		ctx, _ := context.WithTimeout(context.Background(), time.Minute)

		cmd := exec.Command("./simple/topology.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		err := cmd.Run()
		Expect(err).NotTo(HaveOccurred())

		By("starting sartd-bgp")
		go func(context.Context) {
			_, _, _, err := execInNetns("core", "../target/debug/sartd", "bgp", "-f", "simple/config.yaml")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		time.Sleep(time.Second)

		By("starting gobgp daemon in netns")
		go func(context.Context) {
			_, _, _, err = execInNetns("spine1", "gobgpd", "-f", "simple/gobgp_spine1.conf")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		go func(context.Context) {
			_, _, _, err = execInNetns("spine2", "gobgpd", "-f", "simple/gobgp_spine2.conf")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		time.Sleep(time.Second)

		By("configuring peers")
		_, _, _, err = execInNetns("spine1", "gobgp", "neighbor", "add", "10.0.0.1", "as", "100")
		Expect(err).NotTo(HaveOccurred())
		_, _, _, err = execInNetns("spine2", "gobgp", "neighbor", "add", "10.0.1.1", "as", "100")
		Expect(err).NotTo(HaveOccurred())

		By("checking configurations")

		Eventually(func(g Gomega) error {
			if err := checkGobgpConfig("spine1", 100); err != nil {
				return err
			}
			if err := checkGobgpConfig("spine2", 100); err != nil {
				return err
			}
			return nil
		}, "1m").Should(Succeed())

		By("checking to establish peers")
		Eventually(func(g Gomega) error {
			if err := checkEstablished("spine1"); err != nil {
				return err
			}
			if err := checkEstablished("spine2"); err != nil {
				return err
			}
			return nil
		}, "1m").Should(Succeed())

		By("cleaning up the topology")
		cmd = exec.Command("./simple/clean_topology.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		err = cmd.Run()
		Expect(err).NotTo(HaveOccurred())
	})

	It("should establish ipv6 peers", func() {

		ctx, _ := context.WithTimeout(context.Background(), time.Minute)

		cmd := exec.Command("./simple_ipv6/topology.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		err := cmd.Run()
		Expect(err).NotTo(HaveOccurred())

		By("starting gobgp daemon in netns")
		go func(context.Context) {
			_, _, _, err = execInNetns("spine1", "gobgpd", "-f", "simple_ipv6/gobgp_spine1.conf")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		go func(context.Context) {
			_, _, _, err = execInNetns("spine2", "gobgpd", "-f", "simple_ipv6/gobgp_spine2.conf")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		time.Sleep(time.Second)

		By("configuring peers")
		_, _, _, err = execInNetns("spine1", "gobgp", "neighbor", "add", "2001:db8:1::1", "as", "100")
		Expect(err).NotTo(HaveOccurred())
		_, _, _, err = execInNetns("spine2", "gobgp", "neighbor", "add", "2001:db8:2::1", "as", "100")
		Expect(err).NotTo(HaveOccurred())

		By("checking configurations")

		Eventually(func(g Gomega) error {
			if err := checkGobgpConfig("spine1", 100); err != nil {
				return err
			}
			if err := checkGobgpConfig("spine2", 100); err != nil {
				return err
			}
			return nil
		}, "1m").Should(Succeed())

		By("starting sartd-bgp")
		go func(context.Context) {
			_, _, _, err := execInNetns("core", "../target/debug/sartd", "bgp", "-f", "simple_ipv6/config.yaml")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		By("checking to establish peers")
		Eventually(func(g Gomega) error {
			if err := checkEstablished("spine1"); err != nil {
				return err
			}
			if err := checkEstablished("spine2"); err != nil {
				return err
			}
			return nil
		}, "1m").Should(Succeed())

		By("cleaning up the topology")
		cmd = exec.Command("./simple_ipv6/clean_topology.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		err = cmd.Run()
		Expect(err).NotTo(HaveOccurred())
	})

	It("should reestablish peers", func() {

		ctx, _ := context.WithTimeout(context.Background(), time.Minute)

		cmd := exec.Command("./simple/topology.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		err := cmd.Run()
		Expect(err).NotTo(HaveOccurred())

		By("starting gobgp daemon in netns")
		go func(context.Context) {
			_, _, _, err = execInNetns("spine1", "gobgpd", "-f", "simple/gobgp_spine1.conf")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		go func(context.Context) {
			_, _, _, err = execInNetns("spine2", "gobgpd", "-f", "simple/gobgp_spine2.conf")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		By("configuring peers")
		time.Sleep(time.Second)
		_, _, _, err = execInNetns("spine1", "gobgp", "neighbor", "add", "10.0.0.1", "as", "100")
		Expect(err).NotTo(HaveOccurred())
		_, _, _, err = execInNetns("spine2", "gobgp", "neighbor", "add", "10.0.1.1", "as", "100")
		Expect(err).NotTo(HaveOccurred())

		By("checking configurations")
		Eventually(func(g Gomega) error {
			if err := checkGobgpConfig("spine1", 100); err != nil {
				return err
			}
			if err := checkGobgpConfig("spine2", 100); err != nil {
				return err
			}
			return nil
		}, "1m").Should(Succeed())

		By("starting sartd-bgp")
		go func(context.Context) {
			_, _, _, err := execInNetns("core", "../target/debug/sartd", "bgp", "-f", "simple/config.yaml")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		By("checking to establish peers")
		Eventually(func(g Gomega) error {
			if err := checkEstablished("spine1"); err != nil {
				return err
			}
			if err := checkEstablished("spine2"); err != nil {
				return err
			}
			return nil
		}, "1m").Should(Succeed())

		By("delete neighbor by gobgp")
		_, _, _, err = execInNetns("spine1", "gobgp", "neighbor", "del", "10.0.0.1")
		Expect(err).NotTo(HaveOccurred())
		_, _, _, err = execInNetns("spine2", "gobgp", "neighbor", "del", "10.0.1.1")
		Expect(err).NotTo(HaveOccurred())

		By("restarting peer by gobgp")
		time.Sleep(time.Second)
		_, _, _, err = execInNetns("spine1", "gobgp", "neighbor", "add", "10.0.0.1", "as", "100")
		Expect(err).NotTo(HaveOccurred())
		_, _, _, err = execInNetns("spine2", "gobgp", "neighbor", "add", "10.0.1.1", "as", "100")
		Expect(err).NotTo(HaveOccurred())

		By("checking to establish peers after restarting")
		Eventually(func(g Gomega) error {
			if err := checkEstablished("spine1"); err != nil {
				return err
			}
			if err := checkEstablished("spine2"); err != nil {
				return err
			}
			return nil
		}, "1m").Should(Succeed())

		By("cleaning up the topology")
		cmd = exec.Command("./simple/clean_topology.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		err = cmd.Run()
		Expect(err).NotTo(HaveOccurred())
	})
}
