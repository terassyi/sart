package test

import (
	"context"
	"encoding/json"
	"os"
	"os/exec"
	"time"

	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

func testIntegrateFib() {
	It("should integrate with the fib api", func() {
		ctx, _ := context.WithTimeout(context.Background(), time.Minute)

		By("preparing the topology")
		cmd := exec.Command("./multi_path_ebgp/topology.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		err := cmd.Run()
		Expect(err).NotTo(HaveOccurred())

		By("starting frr daemon for gobgp")
		cmd = exec.Command("sudo", "./multi_path_ebgp/frr_run.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		cmd.Run()
		// Expect(err).NotTo(HaveOccurred())

		defer func() {
			By("stopping frr daemon for gobgp")
			cmd = exec.Command("sudo", "./multi_path_ebgp/frr_stop.sh")
			cmd.Stdout = os.Stdout
			cmd.Stderr = os.Stderr
			err = cmd.Run()
			// Expect(err).NotTo(HaveOccurred())

			By("cleaning up the topology")
			cmd = exec.Command("./multi_path_ebgp/clean_topology.sh")
			cmd.Stdout = os.Stdout
			cmd.Stderr = os.Stderr
			err = cmd.Run()
			Expect(err).NotTo(HaveOccurred())
		}()

		By("starting gobgp daemon in netns")
		go func(context.Context) {
			_, _, _, err = execInNetns("node2", "gobgpd", "-f", "multi_path_ebgp/gobgp_node2.conf")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)
		go func(context.Context) {
			_, _, _, err = execInNetns("node3", "gobgpd", "-f", "multi_path_ebgp/gobgp_node3.conf")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)
		go func(context.Context) {
			_, _, _, err = execInNetns("node4", "gobgpd", "-f", "multi_path_ebgp/gobgp_node4.conf")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		By("checking configurations")
		Eventually(func(g Gomega) error {
			if err := checkGobgpConfig("node2", 65010); err != nil {
				return err
			}
			if err := checkGobgpConfig("node3", 65010); err != nil {
				return err
			}
			if err := checkGobgpConfig("node4", 65010); err != nil {
				return err
			}
			return nil
		}, "1m").Should(Succeed())

		By("starting sartd-fib")
		go func(context.Context) {
			_, _, _, err := execInNetns("node1", "../target/debug/sartd", "fib")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		time.Sleep(time.Second)

		By("starting sartd-bgp")
		go func(context.Context) {
			_, _, _, err := execInNetns("node1", "../target/debug/sartd", "bgp", "-f", "multi_path_ebgp/config.yaml", "--fib", "localhost:5001")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		By("checking to establish peers")
		Eventually(func(g Gomega) error {
			if err := checkEstablished("node2"); err != nil {
				return err
			}
			if err := checkEstablished("node3"); err != nil {
				return err
			}
			if err := checkEstablished("node4"); err != nil {
				return err
			}
			return nil
		}, "1m").Should(Succeed())

		By("adding paths by node4")
		_, _, _, err = execInNetns("node4", "gobgp", "global", "rib", "add", "-a", "ipv4", "10.0.2.0/24", "origin", "igp")
		Expect(err).NotTo(HaveOccurred())

		By("adding paths by node2")
		_, _, _, err = execInNetns("node2", "gobgp", "global", "rib", "add", "-a", "ipv4", "10.100.0.0/24", "origin", "igp")
		Expect(err).NotTo(HaveOccurred())
		_, _, _, err = execInNetns("node2", "gobgp", "global", "rib", "add", "-a", "ipv4", "10.69.0.0/24", "origin", "igp")
		Expect(err).NotTo(HaveOccurred())

		time.Sleep(time.Second)

		By("checking node4 received paths advertised by node2 from node1(sartd-bgp)")
		out, _, _, err := execInNetns("node4", "gobgp", "global", "rib", "-a", "ipv4", "-j")
		var res2 map[string]any
		err = json.Unmarshal(out, &res2)
		Expect(err).NotTo(HaveOccurred())
		pref1 := res2["10.100.0.0/24"].([]any)
		Expect(len(pref1)).To(Equal(1))
		pref2 := res2["10.69.0.0/24"].([]any)
		Expect(len(pref2)).To(Equal(1))

		By("adding paths by node3")
		_, _, _, err = execInNetns("node3", "gobgp", "global", "rib", "add", "-a", "ipv4", "10.69.0.0/24", "origin", "igp")
		Expect(err).NotTo(HaveOccurred())

		By("adding paths by node1")
		_, _, _, err = execInNetns("node1", "../../sart/target/debug/sart", "bgp", "global", "rib", "add", "10.68.0.1/32", "-t", "origin=igp")
		Expect(err).NotTo(HaveOccurred())

		By("sending ping to 10.68.0.1 from node4")
		Eventually(func(g Gomega) error {
			_, _, _, err := execInNetns("node4", "ping", "-c", "1", "10.68.0.1")
			if err != nil {
				return err
			}
			return nil
		}, "5s").Should(Succeed())

		By("sending ping to 10.68.0.1 from node4")
		Eventually(func(g Gomega) error {
			_, _, _, err := execInNetns("node2", "ping", "-c", "1", "10.68.0.1")
			if err != nil {
				return err
			}
			return nil
		}, "5s").Should(Succeed())

		By("sending ping to 10.69.0.100 from node4")
		Eventually(func(g Gomega) error {
			_, _, _, err := execInNetns("node4", "ping", "-c", "1", "10.69.0.100")
			if err != nil {
				return err
			}
			return nil
		}, "5s").Should(Succeed())

		By("sending ping to 10.100.0.2 from node4")
		Eventually(func(g Gomega) error {
			_, _, _, err := execInNetns("node4", "ping", "-c", "1", "10.100.0.2")
			if err != nil {
				return err
			}
			return nil
		}, "5s").Should(Succeed())

	})

}
