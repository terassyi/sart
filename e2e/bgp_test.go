package e2e

import (
	"encoding/json"
	"fmt"
	"strconv"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

// struct for binding json output
// ref: sart/sart/src/data/bgp.rs
type sartBgpInfo struct {
	ASN      uint32 `json:"asn"`
	Port     uint32 `json:"port"`
	RouterId string `json:"routerId"`
}

type sartPeerInfo struct {
	Name     string `json:"name"`
	Address  string `json:"address"`
	ASN      uint32 `json:"asn"`
	RouterId string `json:"routerId"`
	State    string `json:"state"`
}

type sartPathInfo struct {
	Nlri      string          `json:"nlri"`
	Family    sartFamily      `json:"family,omitempty"`
	Origin    uint32          `json:"origin"`
	NextHops  []string        `json:"nextHops"`
	Segments  []sartAsSegment `json:"segments"`
	LocalPref uint32          `json:"localPref"`
	Med       uint32          `json:"med"`
	PeerASN   uint32          `json:"peerAsn"`
	PeerAddr  string          `json:"peerAddr"`
	Best      bool            `json:"best"`
	Timestamp string          `json:"timestamp,omitempty"`
}

type sartFamily struct {
	Afi  string `json:"afi"`
	Safi string `json:"safi"`
}

type sartAsSegment struct {
	Type string   `json:"type"`
	Elm  []uint32 `json:"elm"`
}

type goBGPNeighbor struct {
	State goBGPState `json:"state"`
}

type goBGPState struct {
	NeighborAddress string `json:"neighbor_address"`
	PeerASN         int    `json:"peer_asn"`
	SessionState    int    `json:"session_state"`
}

type goBGPPath struct {
	Nlri   goBGPNlri `json:"nlri"`
	Best   bool      `json:"best"`
	Source string    `json:"source-id,omitempty"`
	Attrs  []any     `json:"attrs"`
}

type goBGPNlri struct {
	Prefix string `json:"prefix"`
}

type goBGPAsPaths struct {
	Type    uint32        `json:"type"`
	AsPaths []goBGPAsPath `json:"as_paths"`
}

type goBGPAsPath struct {
	SegmentType uint32   `json:"segment_type"`
	Num         uint32   `json:"num"`
	Asns        []uint32 `json:"asns"`
}

func testEstablishPeerWithFrr() {
	BeforeEach(func() {
		err := deployContainerlab("frr.yaml")
		Expect(err).NotTo(HaveOccurred())
	})
	AfterEach(func() {
		err := destroyContainerlab("frr.yaml")
		Expect(err).NotTo(HaveOccurred())
	})

	It("should establish a peer between frr and sart", func() {
		By("configuring and checking the sartd BGP speaker global settings")
		configureAndCheckSartdBGPGlobalSettings(65001, "169.254.0.2")

		By("adding a peer")
		_, err := execInContainer("clab-sart-sart", nil, "sart", "bgp", "neighbor", "add", "169.254.0.1", "65000")
		Expect(err).NotTo(HaveOccurred())

		By("checking a peer is established")
		confirmSartdBGPIsEstablished("clab-sart-sart", "169.254.0.1")
	})
}

func testEstablishPeerWithGoBGP() {
	BeforeEach(func() {
		err := deployContainerlab("gobgp.yaml")
		Expect(err).NotTo(HaveOccurred())
	})
	AfterEach(func() {
		err := destroyContainerlab("gobgp.yaml")
		Expect(err).NotTo(HaveOccurred())
	})

	It("should establish a peer between gobgp and sart", func() {
		By("configuring and checking the sartd BGP speaker global settings")
		configureAndCheckSartdBGPGlobalSettings(65001, "169.254.0.2")

		By("adding a peer")
		_, err := execInContainer("clab-sart-sart", nil, "sart", "bgp", "neighbor", "add", "169.254.0.1", "65000")
		Expect(err).NotTo(HaveOccurred())

		By("checking a peer is established")
		confirmSartdBGPIsEstablished("clab-sart-sart", "169.254.0.1")
	})
}

func testEstablishPeerWithIBGP() {
	BeforeEach(func() {
		err := deployContainerlab("ibgp.yaml")
		Expect(err).NotTo(HaveOccurred())
	})
	AfterEach(func() {
		err := destroyContainerlab("ibgp.yaml")
		Expect(err).NotTo(HaveOccurred())
	})

	It("should establish a peer between frr and sart", func() {
		By("configuring and checking the sartd BGP speaker global settings")
		configureAndCheckSartdBGPGlobalSettings(65000, "169.254.0.2")

		By("adding a peer")
		_, err := execInContainer("clab-sart-sart", nil, "sart", "bgp", "neighbor", "add", "169.254.0.1", "65000")
		Expect(err).NotTo(HaveOccurred())

		By("checking a peer is established")
		confirmSartdBGPIsEstablished("clab-sart-sart", "169.254.0.1")
	})
}

func testInCircleTopology() {
	BeforeEach(func() {
		err := deployContainerlab("circle.yaml")
		Expect(err).NotTo(HaveOccurred())
	})
	AfterEach(func() {
		err := destroyContainerlab("circle.yaml")
		Expect(err).NotTo(HaveOccurred())
	})
	It("should receive and advertise paths correctly", func() {
		By("configuring and checking the sartd BGP speaker global settings")
		configureAndCheckSartdBGPGlobalSettings(65001, "169.254.0.2")

		By("adding a peer 169.254.0.1")
		_, err := execInContainer("clab-sart-sart", nil, "sart", "bgp", "neighbor", "add", "169.254.0.1", "65000")
		Expect(err).NotTo(HaveOccurred())

		By("checking a peer is established")
		confirmSartdBGPIsEstablished("clab-sart-sart", "169.254.0.1")

		By("adding a peer 169.254.2.2")
		_, err = execInContainer("clab-sart-sart", nil, "sart", "bgp", "neighbor", "add", "169.254.2.2", "65002")
		Expect(err).NotTo(HaveOccurred())

		By("checking a peer is established")
		confirmSartdBGPIsEstablished("clab-sart-sart", "169.254.2.2")

		By("installing the path(10.0.0.0/24) to sartd bgp")
		err = installPathToSartBGP("clab-sart-sart", "10.0.0.0/24")
		Expect(err).NotTo(HaveOccurred())

		By("confirming gobgp received the path")
		Eventually(func() error {
			paths, err := getGoBGPPath("clab-sart-gobgp0")
			if err != nil {
				return err
			}
			expected, ok := paths["10.0.0.0/24"]
			if !ok {
				return fmt.Errorf("10.0.0.0/24 is not found")
			}
			if len(expected) != 2 {
				return fmt.Errorf("10.0.0.0/24 is expected to receive from both peers")
			}
			return nil
		}).Should(Succeed())

		By("installing the path from gobgp")
		err = installPathToGoBGP("clab-sart-gobgp0", "10.0.10.0/24")
		Expect(err).NotTo(HaveOccurred())

		By("confirming sart received the path")
		var path_10_0_10_0_24 []sartPathInfo
		Eventually(func() error {
			expected := make([]sartPathInfo, 0)
			paths, err := getSartBGPPath("clab-sart-sart")
			if err != nil {
				return err
			}
			for _, p := range paths {
				if p.Nlri == "10.0.10.0/24" {
					expected = append(expected, p)
				}
			}
			if len(expected) != 2 {
				return fmt.Errorf("10.0.10.0/24 is expected to receive from both peers")
			}
			path_10_0_10_0_24 = expected
			return nil
		}).Should(Succeed())

		By("checking the best path is selected correctly")
		for _, p := range path_10_0_10_0_24 {
			if p.Best {
				Expect(p.NextHops).To(Equal([]string{"169.254.0.1"}))
			}
		}

		By("deleting the path from sartd bgp")
		err = deletePathFormSartBGP("clab-sart-sart", "10.0.0.0/24")
		Expect(err).NotTo(HaveOccurred())

		By("confirming deleted the path")
		Eventually(func() error {
			paths, err := getGoBGPPath("clab-sart-gobgp0")
			if err != nil {
				return err
			}
			_, ok := paths["10.0.0.0/24"]
			if ok {
				return fmt.Errorf("10.0.0.0/24 must not be found")
			}
			return nil
		}).Should(Succeed())

		By("deleting the path from gobgp")
		err = deletePathFromGoBGP("clab-sart-gobgp0", "10.0.10.0/24")
		Expect(err).NotTo(HaveOccurred())

		By("confirming deleted the path from sart")
		Eventually(func() error {
			paths, err := getSartBGPPath("clab-sart-sart")
			if err != nil {
				return err
			}
			for _, p := range paths {
				if p.Nlri == "10.0.10.0/24" {
					return fmt.Errorf("10.0.10.0/24 must not be found")
				}
			}
			return nil
		}).Should(Succeed())

	})
}

func configureAndCheckSartdBGPGlobalSettings(asn uint32, routerId string) {
	Eventually(func() error {
		out, err := execInContainer("clab-sart-sart", nil, "sart", "bgp", "global", "get", "-ojson")
		if err != nil {
			return err
		}
		info := sartBgpInfo{}
		if err := json.Unmarshal(out, &info); err != nil {
			return err
		}
		if info.ASN != 0 {
			return fmt.Errorf("ASN must not be set")
		}
		return nil
	}).Should(Succeed())

	_, err := execInContainer("clab-sart-sart", nil, "sart", "bgp", "global", "set", "--asn", strconv.Itoa(int(asn)), "--router-id", routerId)
	Expect(err).NotTo(HaveOccurred())

	out, err := execInContainer("clab-sart-sart", nil, "sart", "bgp", "global", "get", "-ojson")
	Expect(err).NotTo(HaveOccurred())
	info := sartBgpInfo{}
	err = json.Unmarshal(out, &info)
	Expect(err).NotTo(HaveOccurred())
	Expect(info.ASN).To(Equal(asn))
	Expect(info.RouterId).To(Equal(routerId))
}

func confirmSartdBGPIsEstablished(containerName, peerAddr string) {
	Eventually(func() error {
		out, err := execInContainer(containerName, nil, "sart", "bgp", "neighbor", "get", peerAddr, "-ojson")
		if err != nil {
			return err
		}
		info := sartPeerInfo{}
		if err := json.Unmarshal(out, &info); err != nil {
			return err
		}
		if info.State != "Established" {
			return fmt.Errorf("expected state is Established, but actual is %s", info.State)
		}
		return nil
	}).Should(Succeed())
}

func testInCircleTopologyWithIBGP() {
	BeforeEach(func() {
		err := deployContainerlab("circle-ibgp.yaml")
		Expect(err).NotTo(HaveOccurred())
	})
	AfterEach(func() {
		// err := destroyContainerlab("circle-ibgp.yaml")
		// Expect(err).NotTo(HaveOccurred())
	})
	It("should receive and advertise paths correctly in IBGP topology", func() {
		By("configuring and checking the sartd BGP speaker global settings")
		configureAndCheckSartdBGPGlobalSettings(65000, "169.254.0.2")

		By("adding a peer 169.254.0.1")
		_, err := execInContainer("clab-sart-sart", nil, "sart", "bgp", "neighbor", "add", "169.254.0.1", "65000")
		Expect(err).NotTo(HaveOccurred())

		By("checking a peer is established")
		confirmSartdBGPIsEstablished("clab-sart-sart", "169.254.0.1")

		By("adding a peer 169.254.2.2")
		_, err = execInContainer("clab-sart-sart", nil, "sart", "bgp", "neighbor", "add", "169.254.2.2", "65000")
		Expect(err).NotTo(HaveOccurred())

		By("checking a peer is established")
		confirmSartdBGPIsEstablished("clab-sart-sart", "169.254.2.2")

		By("installing the path(10.0.0.0/24) to sartd bgp")
		err = installPathToSartBGP("clab-sart-sart", "10.0.0.0/24")
		Expect(err).NotTo(HaveOccurred())

		By("confirming gobgp received the path")
		Eventually(func() error {
			paths, err := getGoBGPPath("clab-sart-gobgp0")
			if err != nil {
				return err
			}
			expected, ok := paths["10.0.0.0/24"]
			if !ok {
				return fmt.Errorf("10.0.0.0/24 is not found")
			}
			if len(expected) != 1 {
				return fmt.Errorf("10.0.0.0/24 is expected to receive from only one peer")
			}
			return nil
		}).Should(Succeed())

		By("installing the path from gobgp")
		err = installPathToGoBGP("clab-sart-gobgp0", "10.0.10.0/24")
		Expect(err).NotTo(HaveOccurred())

		By("confirming sart received the path")
		var path_10_0_10_0_24 []sartPathInfo
		Eventually(func() error {
			expected := make([]sartPathInfo, 0)
			paths, err := getSartBGPPath("clab-sart-sart")
			if err != nil {
				return err
			}
			for _, p := range paths {
				if p.Nlri == "10.0.10.0/24" {
					expected = append(expected, p)
				}
			}
			if len(expected) != 1 {
				return fmt.Errorf("10.0.10.0/24 is expected to recieve only one peer")
			}
			path_10_0_10_0_24 = expected
			return nil
		}).Should(Succeed())

		By("checking the best path is selected correctly")
		for _, p := range path_10_0_10_0_24 {
			if p.Best {
				Expect(p.NextHops).To(Equal([]string{"169.254.0.1"}))
			}
		}

		By("deleting the path from sartd bgp")
		err = deletePathFormSartBGP("clab-sart-sart", "10.0.0.0/24")
		Expect(err).NotTo(HaveOccurred())

		By("confirming deleted the path")
		Eventually(func() error {
			paths, err := getGoBGPPath("clab-sart-gobgp0")
			if err != nil {
				return err
			}
			_, ok := paths["10.0.0.0/24"]
			if ok {
				return fmt.Errorf("10.0.0.0/24 must not be found")
			}
			return nil
		}).Should(Succeed())

		By("deleting the path from gobgp")
		err = deletePathFromGoBGP("clab-sart-gobgp0", "10.0.10.0/24")
		Expect(err).NotTo(HaveOccurred())

		By("confirming deleted the path from sart")
		Eventually(func() error {
			paths, err := getSartBGPPath("clab-sart-sart")
			if err != nil {
				return err
			}
			for _, p := range paths {
				if p.Nlri == "10.0.10.0/24" {
					return fmt.Errorf("10.0.10.0/24 must not be found")
				}
			}
			return nil
		}).Should(Succeed())

	})
}

func getSartBGPPath(containerName string) ([]sartPathInfo, error) {
	type paths struct {
		Paths []sartPathInfo `json:"paths"`
	}
	out, err := execInContainer(containerName, nil, "sart", "bgp", "global", "rib", "get", "-ojson")
	if err != nil {
		return nil, err
	}
	info := paths{}
	if err := json.Unmarshal(out, &info); err != nil {
		return nil, err
	}
	return info.Paths, nil
}

func installPathToSartBGP(containerName, prefix string) error {
	_, err := execInContainer(containerName, nil, "sart", "bgp", "global", "rib", "add", prefix)
	if err != nil {
		return err
	}
	return nil
}

func deletePathFormSartBGP(containerName, prefix string) error {
	_, err := execInContainer(containerName, nil, "sart", "bgp", "global", "rib", "del", prefix)
	if err != nil {
		return err
	}
	return nil
}

func getGoBGPPath(containerName string) (map[string][]goBGPPath, error) {
	type paths map[string][]goBGPPath
	out, err := execInContainer(containerName, nil, "gobgp", "global", "rib", "-j")
	if err != nil {
		return nil, err
	}
	info := paths{}
	if err := json.Unmarshal(out, &info); err != nil {
		return nil, err
	}
	return info, nil
}

func installPathToGoBGP(containerName, prefix string) error {
	_, err := execInContainer(containerName, nil, "gobgp", "global", "rib", "add", prefix)
	if err != nil {
		return err
	}
	return nil
}

func deletePathFromGoBGP(containerName, prefix string) error {
	_, err := execInContainer(containerName, nil, "gobgp", "global", "rib", "del", prefix)
	if err != nil {
		return err
	}
	return nil
}
