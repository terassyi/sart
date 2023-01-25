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

func testUpdate() {
	It("should send update messages to advertise and withdraw paths", func() {
		ctx, _ := context.WithTimeout(context.Background(), time.Minute)

		By("preparing the topology")
		cmd := exec.Command("./simple_rib/topology.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		err := cmd.Run()
		Expect(err).NotTo(HaveOccurred())

		By("starting frr daemon for gobgp")
		cmd = exec.Command("sudo", "./simple_rib/frr_run.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		cmd.Run()
		// Expect(err).NotTo(HaveOccurred())

		By("starting gobgp daemon in netns")
		go func(context.Context) {
			_, _, _, err = execInNetns("spine1", "gobgpd", "-f", "simple_rib/gobgp_spine1.conf")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		go func(context.Context) {
			_, _, _, err = execInNetns("spine2", "gobgpd", "-f", "simple_rib/gobgp_spine2.conf")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		time.Sleep(time.Second)

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
			_, _, _, err := execInNetns("core", "../target/debug/sartd", "-f", "simple/config.yaml")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		By("checking to establish peers")
		Eventually(func(g Gomega) error {
			if err := checkGobgpConfig("spine1", 100); err != nil {
				return err
			}
			if err := checkGobgpConfig("spine2", 100); err != nil {
				return err
			}
			return nil
		}, "1m").Should(Succeed())

		By("adding paths by spine2")
		_, _, _, err = execInNetns("spine2", "gobgp", "global", "rib", "add", "-a", "ipv4", "10.0.0.0/24", "origin", "igp")
		Expect(err).NotTo(HaveOccurred())
		_, _, _, err = execInNetns("spine2", "gobgp", "global", "rib", "add", "-a", "ipv4", "10.1.0.0/24", "origin", "igp")
		Expect(err).NotTo(HaveOccurred())

		time.Sleep(time.Second)

		By("checking spine1 received paths advertised by spine2 from core(sartd-bgp)")
		out, _, _, err := execInNetns("spine1", "gobgp", "global", "rib", "-a", "ipv4", "-j")
		var res map[string]any
		err = json.Unmarshal(out, &res)
		Expect(err).NotTo(HaveOccurred())
		pref1 := res["10.0.0.0/24"].([]any)
		Expect(len(pref1)).To(Equal(1))
		pref2 := res["10.1.0.0/24"].([]any)
		Expect(len(pref2)).To(Equal(1))

		By("deleting a path by peer2")
		_, _, _, err = execInNetns("spine2", "gobgp", "global", "rib", "del", "-a", "ipv4", "10.0.0.0/24")
		Expect(err).NotTo(HaveOccurred())

		time.Sleep(time.Second)

		By("checking spine1 received a withdrawn message from core")
		out, _, _, err = execInNetns("spine1", "gobgp", "global", "rib", "-a", "ipv4", "-j")
		var res2 map[string]any
		err = json.Unmarshal(out, &res2)
		Expect(err).NotTo(HaveOccurred())
		_, ok := res2["10.0.0.0/24"].([]any)
		Expect(ok).To(BeFalse())
		pref2 = res2["10.1.0.0/24"].([]any)
		Expect(len(pref2)).To(Equal(1))

		By("stopping frr daemon for gobgp")
		cmd = exec.Command("sudo", "./simple_rib/frr_stop.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		err = cmd.Run()
		// Expect(err).NotTo(HaveOccurred())

		By("cleaning up the topology")
		cmd = exec.Command("./simple/clean_topology.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		err = cmd.Run()
		Expect(err).NotTo(HaveOccurred())
	})
}
