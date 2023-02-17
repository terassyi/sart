package test

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"time"

	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

func testApi() {
	It("should add local path via gRPC", func() {
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
			_, _, _, err := execInNetns("core", "../target/debug/sartd")
			Expect(err).NotTo(HaveOccurred())
		}(ctx)

		By("configuring local settings")
		_, _, _, err = execInNetns("core", "simple_rib/grpcurl_set_local.sh")
		Expect(err).NotTo(HaveOccurred())

		time.Sleep(time.Millisecond * 100)
		By("getting bgp global information via api")
		out, _, _, err := execInNetns("core", "simple_rib/grpcurl_bgp_info.sh")
		Expect(err).NotTo(HaveOccurred())
		var resBgpInfo map[string]any
		err = json.Unmarshal(out, &resBgpInfo)
		Expect(err).NotTo(HaveOccurred())
		localAsn := resBgpInfo["info"].(map[string]any)["asn"].(float64)
		Expect(localAsn).To(Equal(float64(100)))
		localRouterId := resBgpInfo["info"].(map[string]any)["routerId"].(string)
		Expect(localRouterId).To(Equal("1.1.1.1"))

		By("configuring peer settings")
		_, _, _, err = execInNetns("core", "simple_rib/grpcurl_add_peer.sh")
		Expect(err).NotTo(HaveOccurred())

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

		By("getting peer information via api")
		out, _, _, err = execInNetns("core", "simple_rib/grpcurl_get_neighbor.sh")
		Expect(err).NotTo(HaveOccurred())
		var resPeer map[string]any
		err = json.Unmarshal(out, &resPeer)
		Expect(err).NotTo(HaveOccurred())
		Expect(resPeer["peer"].(map[string]any)["asn"].(float64)).To(Equal(float64(200)))
		Expect(resPeer["peer"].(map[string]any)["routerId"].(string)).To(Equal("2.2.2.2"))
		Expect(resPeer["peer"].(map[string]any)["state"].(string)).To(Equal("ESTABLISHED"))

		By("adding paths by core")
		_, _, _, err = execInNetns("core", "simple_rib/grpcurl_add_path.sh")
		Expect(err).NotTo(HaveOccurred())

		time.Sleep(time.Second)

		By("checking spine1 received paths advertised by core(sartd-bgp)")
		out, _, _, err = execInNetns("spine1", "gobgp", "global", "rib", "-a", "ipv4", "-j")
		var res map[string]any
		err = json.Unmarshal(out, &res)
		Expect(err).NotTo(HaveOccurred())
		pref1 := res["10.0.0.0/24"].([]any)
		Expect(len(pref1)).To(Equal(1))
		pref2 := res["10.1.0.0/24"].([]any)
		Expect(len(pref2)).To(Equal(1))
		pref3 := res["10.2.0.0/24"].([]any)
		Expect(len(pref3)).To(Equal(1))

		By("getting path information via api")
		out, _, _, err = execInNetns("core", "simple_rib/grpcurl_get_path.sh")
		var resPaths map[string]any
		fmt.Println(string(out))
		err = json.Unmarshal(out, &resPaths)
		Expect(err).NotTo(HaveOccurred())
		paths := resPaths["paths"].([]any)
		Expect(len(paths)).To(Equal(3))

		By("deleting a path by core")
		_, _, _, err = execInNetns("core", "simple_rib/grpcurl_delete_path.sh")
		Expect(err).NotTo(HaveOccurred())

		time.Sleep(time.Second)

		By("checking spine1 received a withdrawn message from core")
		out, _, _, err = execInNetns("spine1", "gobgp", "global", "rib", "-a", "ipv4", "-j")
		var res2 map[string]any
		err = json.Unmarshal(out, &res2)
		Expect(err).NotTo(HaveOccurred())
		pref1 = res2["10.1.0.0/24"].([]any)
		Expect(len(pref1)).To(Equal(1))
		pref2 = res2["10.2.0.0/24"].([]any)
		Expect(len(pref2)).To(Equal(1))

		By("adding paths by spine2")
		_, _, _, err = execInNetns("spine2", "gobgp", "global", "rib", "add", "-a", "ipv4", "10.100.0.0/24", "origin", "igp")
		Expect(err).NotTo(HaveOccurred())

		time.Sleep(time.Second)

		By("checking spine1 received a withdrawn message from core")
		out, _, _, err = execInNetns("spine1", "gobgp", "global", "rib", "-a", "ipv4", "-j")
		var res3 map[string]any
		err = json.Unmarshal(out, &res3)
		Expect(err).NotTo(HaveOccurred())
		pref1 = res3["10.100.0.0/24"].([]any)
		Expect(len(pref1)).To(Equal(1))

		By("deleting peer(10.0.1.2) via api")
		_, _, _, err = execInNetns("core", "simple_rib/grpcurl_delete_peer.sh")
		Expect(err).NotTo(HaveOccurred())

		time.Sleep(time.Second * 2)

		By("checking spine2 is down")
		Eventually(func(g Gomega) error {
			if err := checkDesiredState("spine1", 6); err != nil {
				return err
			}
			if err := checkDesiredState("spine2", 1); err != nil {
				return err
			}
			return nil
		}, "1m").Should(Succeed())

		By("checking spine1 received a withdrawn message from core")
		out2, _, _, err := execInNetns("spine1", "gobgp", "global", "rib", "-a", "ipv4", "-j")
		var res4 map[string]any
		err = json.Unmarshal(out2, &res4)
		Expect(err).NotTo(HaveOccurred())
		preff1 := res4["10.100.0.0/24"]
		Expect(preff1).To(BeNil())

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
