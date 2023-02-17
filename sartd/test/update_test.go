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
	It("should send update messages to advertise and withdraw paths with ebgp", func() {
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

	It("should send update messages to advertise and withdraw paths with ibgp", func() {
		ctx, _ := context.WithTimeout(context.Background(), time.Minute)

		By("preparing the topology")
		cmd := exec.Command("./simple_rib_ibgp/topology.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		err := cmd.Run()
		Expect(err).NotTo(HaveOccurred())

		By("starting frr daemon for gobgp")
		cmd = exec.Command("sudo", "./simple_rib_ibgp/frr_run.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		cmd.Run()
		// Expect(err).NotTo(HaveOccurred())

		By("starting gobgp daemon in netns")
		go func(context.Context) {
			_, _, _, err = execInNetns("spine1", "gobgpd", "-f", "simple_rib_ibgp/gobgp_spine1.conf")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		go func(context.Context) {
			_, _, _, err = execInNetns("spine2", "gobgpd", "-f", "simple_rib_ibgp/gobgp_spine2.conf")
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
			_, _, _, err := execInNetns("core", "../target/debug/sartd", "-f", "simple_rib_ibgp/config.yaml")
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

		time.Sleep(time.Second)

		By("checking spine1 didn't receive paths advertised by spine2 from core(sartd-bgp)")
		out, _, _, err := execInNetns("spine1", "gobgp", "global", "rib", "-a", "ipv4", "-j")
		var res map[string]any
		err = json.Unmarshal(out, &res)
		Expect(err).NotTo(HaveOccurred())
		Expect(res).To(Equal(make(map[string]any)))

		By("deleting a path by peer2")
		_, _, _, err = execInNetns("spine2", "gobgp", "global", "rib", "del", "-a", "ipv4", "10.0.0.0/24")
		Expect(err).NotTo(HaveOccurred())

		time.Sleep(time.Second)

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
	It("should send update messages to advertise and withdraw multiple paths with ebgp", func() {
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

		time.Sleep(time.Second)

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

		By("starting sartd-bgp")
		go func(context.Context) {
			_, _, _, err := execInNetns("node1", "../target/debug/sartd", "-f", "multi_path_ebgp/config.yaml")
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

		By("adding paths by node2")
		_, _, _, err = execInNetns("node2", "gobgp", "global", "rib", "add", "-a", "ipv4", "10.0.0.0/24", "origin", "igp")
		Expect(err).NotTo(HaveOccurred())
		_, _, _, err = execInNetns("node2", "gobgp", "global", "rib", "add", "-a", "ipv4", "10.1.0.0/24", "origin", "igp")
		Expect(err).NotTo(HaveOccurred())

		time.Sleep(time.Second)

		By("checking node4 received paths advertised by node2 from node1(sartd-bgp)")
		out, _, _, err := execInNetns("node4", "gobgp", "global", "rib", "-a", "ipv4", "-j")
		var res map[string]any
		err = json.Unmarshal(out, &res)
		Expect(err).NotTo(HaveOccurred())
		pref1 := res["10.0.0.0/24"].([]any)
		Expect(len(pref1)).To(Equal(1))
		pref2 := res["10.1.0.0/24"].([]any)
		Expect(len(pref2)).To(Equal(1))

		By("adding paths by node3")
		_, _, _, err = execInNetns("node3", "gobgp", "global", "rib", "add", "-a", "ipv4", "10.1.0.0/24", "origin", "igp")
		Expect(err).NotTo(HaveOccurred())

		By("deleting a path by node2")
		_, _, _, err = execInNetns("node2", "gobgp", "global", "rib", "del", "-a", "ipv4", "10.0.0.0/24")
		Expect(err).NotTo(HaveOccurred())

		time.Sleep(time.Second)

		By("checking node4 received paths advertised by node2 from node1(sartd-bgp)")
		out, _, _, err = execInNetns("node4", "gobgp", "global", "rib", "-a", "ipv4", "-j")
		var res2 map[string]any
		err = json.Unmarshal(out, &res2)
		Expect(err).NotTo(HaveOccurred())
		preff1 := res["10.0.0.0/24"].([]any)
		Expect(len(preff1)).To(Equal(1))
		preff2 := res["10.1.0.0/24"].([]any)
		Expect(len(preff2)).To(Equal(1))

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
	})

	It("should send update messages to advertise and withdraw multiple paths with ebgp and ibgp", func() {
		ctx, _ := context.WithTimeout(context.Background(), time.Minute)

		By("preparing the topology")
		cmd := exec.Command("./multi_path_i_ebgp/topology.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		err := cmd.Run()
		Expect(err).NotTo(HaveOccurred())

		By("starting frr daemon for gobgp")
		cmd = exec.Command("sudo", "./multi_path_i_ebgp/frr_run.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		cmd.Run()
		// Expect(err).NotTo(HaveOccurred())

		By("starting gobgp daemon in netns")
		go func(context.Context) {
			_, _, _, err = execInNetns("node2", "gobgpd", "-f", "multi_path_i_ebgp/gobgp_node2.conf")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)
		go func(context.Context) {
			_, _, _, err = execInNetns("node3", "gobgpd", "-f", "multi_path_i_ebgp/gobgp_node3.conf")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)
		go func(context.Context) {
			_, _, _, err = execInNetns("node4", "gobgpd", "-f", "multi_path_i_ebgp/gobgp_node4.conf")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		time.Sleep(time.Second)

		By("checking configurations")
		Eventually(func(g Gomega) error {
			if err := checkGobgpConfig("node2", 65000); err != nil {
				return err
			}
			if err := checkGobgpConfig("node3", 65000); err != nil {
				return err
			}
			if err := checkGobgpConfig("node4", 65000); err != nil {
				return err
			}
			return nil
		}, "1m").Should(Succeed())

		By("starting sartd-bgp")
		go func(context.Context) {
			_, _, _, err := execInNetns("node1", "../target/debug/sartd", "-f", "multi_path_i_ebgp/config.yaml")
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

		By("adding paths by node2")
		_, _, _, err = execInNetns("node2", "gobgp", "global", "rib", "add", "-a", "ipv4", "10.0.0.0/24", "origin", "igp")
		Expect(err).NotTo(HaveOccurred())
		_, _, _, err = execInNetns("node2", "gobgp", "global", "rib", "add", "-a", "ipv4", "10.1.0.0/24", "origin", "igp")
		Expect(err).NotTo(HaveOccurred())

		time.Sleep(time.Second)

		By("checking node3 didn't receive paths advertised by node2 from node1(sartd-bgp)")
		out, _, _, err := execInNetns("node3", "gobgp", "global", "rib", "-a", "ipv4", "-j")
		var res map[string]any
		err = json.Unmarshal(out, &res)
		Expect(err).NotTo(HaveOccurred())
		Expect(res).To(Equal(make(map[string]any)))

		By("checking node4 received paths advertised by node2 from node1(sartd-bgp)")
		out, _, _, err = execInNetns("node4", "gobgp", "global", "rib", "-a", "ipv4", "-j")
		var res2 map[string]any
		err = json.Unmarshal(out, &res2)
		Expect(err).NotTo(HaveOccurred())
		pref1 := res2["10.0.0.0/24"].([]any)
		Expect(len(pref1)).To(Equal(1))
		pref2 := res2["10.1.0.0/24"].([]any)
		Expect(len(pref2)).To(Equal(1))

		By("adding paths by node3")
		_, _, _, err = execInNetns("node3", "gobgp", "global", "rib", "add", "-a", "ipv4", "10.1.0.0/24", "origin", "igp")
		Expect(err).NotTo(HaveOccurred())

		By("deleting a path by node2")
		_, _, _, err = execInNetns("node2", "gobgp", "global", "rib", "del", "-a", "ipv4", "10.1.0.0/24")
		Expect(err).NotTo(HaveOccurred())

		time.Sleep(time.Second)

		By("checking node4 received paths advertised by node2 from node1(sartd-bgp)")
		out, _, _, err = execInNetns("node4", "gobgp", "global", "rib", "-a", "ipv4", "-j")
		var res3 map[string]any
		err = json.Unmarshal(out, &res3)
		Expect(err).NotTo(HaveOccurred())
		preff1 := res3["10.0.0.0/24"].([]any)
		Expect(len(preff1)).To(Equal(1))
		preff2 := res3["10.1.0.0/24"].([]any)
		Expect(len(preff2)).To(Equal(1))

		By("stopping frr daemon for gobgp")
		cmd = exec.Command("sudo", "./multi_path_i_ebgp/frr_stop.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		err = cmd.Run()
		// Expect(err).NotTo(HaveOccurred())

		By("cleaning up the topology")
		cmd = exec.Command("./multi_path_i_ebgp/clean_topology.sh")
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		err = cmd.Run()
		Expect(err).NotTo(HaveOccurred())
	})

	It("should send update messages to advertise initially installed paths", func() {
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
			_, _, _, err := execInNetns("core", "../target/debug/sartd", "-f", "simple_rib/config_init_path.yaml")
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

		By("checking spine1 and spine2 received paths advertised by core(sartd-bgp)")
		out, _, _, err := execInNetns("spine1", "gobgp", "global", "rib", "-a", "ipv4", "-j")
		var res map[string]any
		err = json.Unmarshal(out, &res)
		Expect(err).NotTo(HaveOccurred())
		pref1 := res["10.1.0.0/24"].([]any)
		Expect(len(pref1)).To(Equal(1))
		pref2 := res["10.2.0.0/24"].([]any)
		Expect(len(pref2)).To(Equal(1))
		pref3 := res["192.168.0.10/32"].([]any)
		Expect(len(pref3)).To(Equal(1))

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

	It("should send withdrawn messages received from released peers", func() {
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
			_, _, _, err := execInNetns("core", "../target/debug/sartd", "-f", "simple_rib/config_init_path.yaml")
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

		By("checking spine1 and spine2 received paths advertised by core(sartd-bgp)")
		out, _, _, err := execInNetns("spine1", "gobgp", "global", "rib", "-a", "ipv4", "-j")
		var res map[string]any
		err = json.Unmarshal(out, &res)
		Expect(err).NotTo(HaveOccurred())
		pref1 := res["10.1.0.0/24"].([]any)
		Expect(len(pref1)).To(Equal(1))
		pref2 := res["10.2.0.0/24"].([]any)
		Expect(len(pref2)).To(Equal(1))
		pref3 := res["192.168.0.10/32"].([]any)
		Expect(len(pref3)).To(Equal(1))

		By("adding paths by spine2")
		_, _, _, err = execInNetns("spine2", "gobgp", "global", "rib", "add", "-a", "ipv4", "10.10.0.0/24", "origin", "igp")
		Expect(err).NotTo(HaveOccurred())
		_, _, _, err = execInNetns("spine2", "gobgp", "global", "rib", "add", "-a", "ipv4", "10.11.0.0/24", "origin", "igp")
		Expect(err).NotTo(HaveOccurred())

		time.Sleep(time.Second)

		By("checking spine1 received paths advertised by spine2 from core(sartd-bgp)")
		out, _, _, err = execInNetns("spine1", "gobgp", "global", "rib", "-a", "ipv4", "-j")
		var res2 map[string]any
		err = json.Unmarshal(out, &res2)
		Expect(err).NotTo(HaveOccurred())
		preff1 := res2["10.10.0.0/24"].([]any)
		Expect(len(preff1)).To(Equal(1))
		preff2 := res2["10.11.0.0/24"].([]any)
		Expect(len(preff2)).To(Equal(1))

		By("delete neighbor by gobgp")
		_, _, _, err = execInNetns("spine2", "gobgp", "neighbor", "del", "10.0.1.1")
		Expect(err).NotTo(HaveOccurred())

		time.Sleep(time.Second)
		By("checking spine1 has paths advertised by spine2 from core(sartd-bgp)")
		out, _, _, err = execInNetns("spine1", "gobgp", "global", "rib", "-a", "ipv4", "-j")
		var res3 map[string]any
		err = json.Unmarshal(out, &res3)
		Expect(err).NotTo(HaveOccurred())
		prefff1 := res3["10.1.0.0/24"].([]any)
		Expect(len(prefff1)).To(Equal(1))
		prefff2 := res3["10.2.0.0/24"].([]any)
		Expect(len(prefff2)).To(Equal(1))
		prefff3 := res3["10.10.0.0/24"]
		Expect(prefff3).To(BeNil())
		prefff4 := res3["10.11.0.0/24"]
		Expect(prefff4).To(BeNil())
		prefff5 := res3["192.168.0.10/32"].([]any)
		Expect(len(prefff5)).To(Equal(1))

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
