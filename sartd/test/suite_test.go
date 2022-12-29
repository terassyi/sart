package test

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"testing"
	"time"

	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

func TestSartdBgp(t *testing.T) {

	RegisterFailHandler(Fail)
	RunSpecs(t, "SARTD-BGP test")
}

var _ = Describe("sartd-bgp", func() {

	BeforeEach(func() {
		fmt.Printf("START: %s\n", time.Now().Format(time.RFC3339))
	})
	AfterEach(func() {
		fmt.Printf("END: %s\n", time.Now().Format(time.RFC3339))
	})

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
			spine1 := make([]map[string]any, 0, 1)
			out, _, _, err := execInNetns("spine1", "gobgp", "neighbor", "-j")
			err = json.Unmarshal(out, &spine1)
			g.Expect(err).NotTo(HaveOccurred())
			conf, ok := spine1[0]["conf"]
			g.Expect(ok).To(BeTrue())
			conff := conf.(map[string]any)
			g.Expect(conff["peer_asn"]).To(Equal(float64(100)))

			spine2 := make([]map[string]any, 0, 1)
			out, _, _, err = execInNetns("spine2", "gobgp", "neighbor", "-j")
			err = json.Unmarshal(out, &spine2)
			g.Expect(err).NotTo(HaveOccurred())
			conf, ok = spine2[0]["conf"]
			g.Expect(ok).To(BeTrue())
			conff = conf.(map[string]any)
			g.Expect(conff["peer_asn"]).To(Equal(float64(100)))

			return nil
		}, "1m").Should(Succeed())

		By("starting sartd-bgp")
		go func(context.Context) {
			out, _, _, err := execInNetns("core", "../target/debug/sartd", "-f", "simple/config.yaml")
			Expect(err).NotTo(HaveOccurred())
			fmt.Println(string(out))
		}(ctx)

		By("checking to establish peers")
		time.Sleep(time.Second * 5)

		Eventually(func(g Gomega) error {
			spine1 := make([]map[string]any, 0, 1)
			out, _, _, err := execInNetns("spine1", "gobgp", "neighbor", "-j")
			err = json.Unmarshal(out, &spine1)
			g.Expect(err).NotTo(HaveOccurred())
			state, ok := spine1[0]["state"]
			g.Expect(ok).To(BeTrue())
			statej := state.(map[string]any)
			if statej["session_state"] != float64(6) {
				return fmt.Errorf("spine1 is not established. state is %d", statej["session_state"])
			}

			spine2 := make([]map[string]any, 0, 1)
			out, _, _, err = execInNetns("spine2", "gobgp", "neighbor", "-j")
			err = json.Unmarshal(out, &spine2)
			g.Expect(err).NotTo(HaveOccurred())
			state, ok = spine2[0]["state"]
			g.Expect(ok).To(BeTrue())
			statej = state.(map[string]any)
			if statej["session_state"] != float64(6) {
				return fmt.Errorf("spine1 is not established. state is %d", statej["session_state"])
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
			out, _, _, err := execInNetns("core", "../target/debug/sartd", "-f", "simple/config.yaml")
			Expect(err).NotTo(HaveOccurred())
			fmt.Println(string(out))
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
			spine1 := make([]map[string]any, 0, 1)
			out, _, _, err := execInNetns("spine1", "gobgp", "neighbor", "-j")
			err = json.Unmarshal(out, &spine1)
			g.Expect(err).NotTo(HaveOccurred())
			conf, ok := spine1[0]["conf"]
			g.Expect(ok).To(BeTrue())
			conff := conf.(map[string]any)
			g.Expect(conff["peer_asn"]).To(Equal(float64(100)))

			spine2 := make([]map[string]any, 0, 1)
			out, _, _, err = execInNetns("spine2", "gobgp", "neighbor", "-j")
			err = json.Unmarshal(out, &spine2)
			g.Expect(err).NotTo(HaveOccurred())
			conf, ok = spine2[0]["conf"]
			g.Expect(ok).To(BeTrue())
			conff = conf.(map[string]any)
			g.Expect(conff["peer_asn"]).To(Equal(float64(100)))
			return nil
		}, "1m").Should(Succeed())

		By("checking to establish peers")

		Eventually(func(g Gomega) error {
			spine1 := make([]map[string]any, 0, 1)
			out, _, _, err := execInNetns("spine1", "gobgp", "neighbor", "-j")
			err = json.Unmarshal(out, &spine1)
			g.Expect(err).NotTo(HaveOccurred())
			state, ok := spine1[0]["state"]
			g.Expect(ok).To(BeTrue())
			statej := state.(map[string]any)
			if statej["session_state"] != float64(6) {
				return fmt.Errorf("spine1 is not established. state is %d", statej["session_state"])
			}

			spine2 := make([]map[string]any, 0, 1)
			out, _, _, err = execInNetns("spine2", "gobgp", "neighbor", "-j")
			err = json.Unmarshal(out, &spine2)
			g.Expect(err).NotTo(HaveOccurred())
			state, ok = spine2[0]["state"]
			g.Expect(ok).To(BeTrue())
			statej = state.(map[string]any)
			if statej["session_state"] != float64(6) {
				return fmt.Errorf("spine2 is not established. state is %d", statej["session_state"])
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
			spine1 := make([]map[string]any, 0, 1)
			out, _, _, err := execInNetns("spine1", "gobgp", "neighbor", "-j")
			err = json.Unmarshal(out, &spine1)
			g.Expect(err).NotTo(HaveOccurred())
			conf, ok := spine1[0]["conf"]
			g.Expect(ok).To(BeTrue())
			conff := conf.(map[string]any)
			g.Expect(conff["peer_asn"]).To(Equal(float64(100)))

			spine2 := make([]map[string]any, 0, 1)
			out, _, _, err = execInNetns("spine2", "gobgp", "neighbor", "-j")
			err = json.Unmarshal(out, &spine2)
			g.Expect(err).NotTo(HaveOccurred())
			conf, ok = spine2[0]["conf"]
			g.Expect(ok).To(BeTrue())
			conff = conf.(map[string]any)
			g.Expect(conff["peer_asn"]).To(Equal(float64(100)))

			return nil
		}, "1m").Should(Succeed())

		By("starting sartd-bgp")
		go func(context.Context) {
			out, _, _, err := execInNetns("core", "../target/debug/sartd", "-f", "simple_ipv6/config.yaml")
			Expect(err).NotTo(HaveOccurred())
			fmt.Println(string(out))
		}(ctx)

		By("checking to establish peers")

		Eventually(func(g Gomega) error {
			spine1 := make([]map[string]any, 0, 1)
			out, _, _, err := execInNetns("spine1", "gobgp", "neighbor", "-j")
			err = json.Unmarshal(out, &spine1)
			g.Expect(err).NotTo(HaveOccurred())
			state, ok := spine1[0]["state"]
			g.Expect(ok).To(BeTrue())
			statej := state.(map[string]any)
			if statej["session_state"] != float64(6) {
				return fmt.Errorf("spine1 is not established. state is %d", statej["session_state"])
			}

			spine2 := make([]map[string]any, 0, 1)
			out, _, _, err = execInNetns("spine2", "gobgp", "neighbor", "-j")
			err = json.Unmarshal(out, &spine2)
			g.Expect(err).NotTo(HaveOccurred())
			state, ok = spine2[0]["state"]
			g.Expect(ok).To(BeTrue())
			statej = state.(map[string]any)
			if statej["session_state"] != float64(6) {
				return fmt.Errorf("spine1 is not established. state is %d", statej["session_state"])
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

	// It("should reestablish peers", func() {

	// 	ctx, _ := context.WithTimeout(context.Background(), time.Minute)

	// 	cmd := exec.Command("./simple/topology.sh")
	// 	cmd.Stdout = os.Stdout
	// 	cmd.Stderr = os.Stderr
	// 	err := cmd.Run()
	// 	Expect(err).NotTo(HaveOccurred())

	// 	By("starting gobgp daemon in netns")

	// 	cmd1 := exec.Command("sudo", "ip", "netns", "exec", "spine1", "gobgpd", "-f", "simple/gobgp_spine1.conf")
	// 	go func() {
	// 		cmd1.Run()
	// 	}()

	// 	cmd2 := exec.Command("sudo", "ip", "netns", "exec", "spine2", "gobgpd", "-f", "simple/gobgp_spine2.conf")
	// 	go func() {
	// 		cmd2.Run()
	// 	}()

	// 	By("configuring peers")
	// 	time.Sleep(time.Second)
	// 	_, _, _, err = execInNetns("spine1", "gobgp", "neighbor", "add", "10.0.0.1", "as", "100")
	// 	Expect(err).NotTo(HaveOccurred())
	// 	_, _, _, err = execInNetns("spine2", "gobgp", "neighbor", "add", "10.0.1.1", "as", "100")
	// 	Expect(err).NotTo(HaveOccurred())

	// 	By("starting sartd-bgp")
	// 	go func(context.Context) {
	// 		out, _, _, err := execInNetns("core", "../target/debug/sartd", "-f", "simple/config.yaml")
	// 		Expect(err).NotTo(HaveOccurred())
	// 		fmt.Println(string(out))
	// 	}(ctx)

	// 	By("checking to establish peers")
	// 	Eventually(func(g Gomega) error {
	// 		spine1 := make([]map[string]any, 0, 1)
	// 		out, _, _, err := execInNetns("spine1", "gobgp", "neighbor", "-j")
	// 		err = json.Unmarshal(out, &spine1)
	// 		g.Expect(err).NotTo(HaveOccurred())
	// 		fmt.Println(spine1)
	// 		state, ok := spine1[0]["state"]
	// 		g.Expect(ok).To(BeTrue())
	// 		statej := state.(map[string]any)
	// 		if statej["session_state"] != float64(6) {
	// 			return fmt.Errorf("spine1 is not established. state is %d", statej["session_state"])
	// 		}

	// 		spine2 := make([]map[string]any, 0, 1)
	// 		out, _, _, err = execInNetns("spine2", "gobgp", "neighbor", "-j")
	// 		err = json.Unmarshal(out, &spine2)
	// 		g.Expect(err).NotTo(HaveOccurred())
	// 		state, ok = spine2[0]["state"]
	// 		g.Expect(ok).To(BeTrue())
	// 		statej = state.(map[string]any)
	// 		if statej["session_state"] != float64(6) {
	// 			return fmt.Errorf("spine1 is not established. state is %d", statej["session_state"])
	// 		}
	// 		return nil
	// 	}, "1m").Should(Succeed())

	// 	By("stopping gobgp daemon")
	// 	cmd1.Process.Kill()
	// 	Expect(err).NotTo(HaveOccurred())
	// 	cmd2.Process.Kill()
	// 	Expect(err).NotTo(HaveOccurred())

	// 	By("restarting gobgp daemon")
	// 	go func(context.Context) {
	// 		_, _, _, err = execInNetns("spine1", "gobgpd", "-f", "simple/gobgp_spine1.conf")
	// 		Expect(err).NotTo(HaveOccurred())
	// 	}(ctx)

	// 	go func(context.Context) {
	// 		_, _, _, err = execInNetns("spine2", "gobgpd", "-f", "simple/gobgp_spine2.conf")
	// 		Expect(err).NotTo(HaveOccurred())
	// 	}(ctx)

	// 	By("configuring peers")
	// 	time.Sleep(time.Second)
	// 	_, _, _, err = execInNetns("spine1", "gobgp", "neighbor", "add", "10.0.0.1", "as", "100")
	// 	Expect(err).NotTo(HaveOccurred())
	// 	_, _, _, err = execInNetns("spine2", "gobgp", "neighbor", "add", "10.0.1.1", "as", "100")
	// 	Expect(err).NotTo(HaveOccurred())

	// 	By("checking to establish peers after restarting")
	// 	Eventually(func(g Gomega) error {
	// 		spine1 := make([]map[string]any, 0, 1)
	// 		out, _, _, err := execInNetns("spine1", "gobgp", "neighbor", "-j")
	// 		err = json.Unmarshal(out, &spine1)
	// 		g.Expect(err).NotTo(HaveOccurred())
	// 		state, ok := spine1[0]["state"]
	// 		g.Expect(ok).To(BeTrue())
	// 		statej := state.(map[string]any)
	// 		if statej["session_state"] != float64(6) {
	// 			return fmt.Errorf("spine1 is not established. state is %d", statej["session_state"])
	// 		}

	// 		spine2 := make([]map[string]any, 0, 1)
	// 		out, _, _, err = execInNetns("spine2", "gobgp", "neighbor", "-j")
	// 		err = json.Unmarshal(out, &spine2)
	// 		g.Expect(err).NotTo(HaveOccurred())
	// 		state, ok = spine2[0]["state"]
	// 		g.Expect(ok).To(BeTrue())
	// 		statej = state.(map[string]any)
	// 		if statej["session_state"] != float64(6) {
	// 			return fmt.Errorf("spine1 is not established. state is %d", statej["session_state"])
	// 		}
	// 		return nil
	// 	}, "1m").Should(Succeed())

	// 	By("cleaning up the topology")
	// 	cmd = exec.Command("./simple/clean_topology.sh")
	// 	cmd.Stdout = os.Stdout
	// 	cmd.Stderr = os.Stderr
	// 	err = cmd.Run()
	// 	Expect(err).NotTo(HaveOccurred())
	// })
})
