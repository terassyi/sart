package test

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
)

func execInNetns(ns string, args ...string) ([]byte, []byte, *os.Process, error) {
	a := []string{"ip", "netns", "exec"}
	a = append(a, ns)
	a = append(a, args...)
	cmd := exec.Command("sudo", a...)

	outBuf := new(bytes.Buffer)
	errBuf := new(bytes.Buffer)

	// cmd.Stdout = outBuf
	// cmd.Stderr = errBuf
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stdout

	err := cmd.Run()

	return outBuf.Bytes(), errBuf.Bytes(), cmd.Process, err
}

func checkGobgpConfig(node string, asn uint32) error {
	nodeJson := make([]map[string]any, 0, 1)
	out, _, _, err := execInNetns(node, "gobgp", "neighbor", "-j")
	if err != nil {
		return err
	}
	err = json.Unmarshal(out, &nodeJson)
	if err != nil {
		return err
	}
	conf, ok := nodeJson[0]["conf"]
	if !ok {
		return fmt.Errorf("failed to get peer conf")
	}
	conff := conf.(map[string]any)
	if conff["peer_asn"] != float64(asn) {
		return fmt.Errorf("expected asn is %d, but found %d", asn, conff["peer_asn"])
	}
	return nil
}

func checkEstablished(node string) error {
	nodeJson := make([]map[string]any, 0, 1)
	out, _, _, err := execInNetns(node, "gobgp", "neighbor", "-j")
	if err != nil {
		return err
	}
	err = json.Unmarshal(out, &nodeJson)
	if err != nil {
		return err
	}
	state, ok := nodeJson[0]["state"]
	if !ok {
		return fmt.Errorf("failed to get gobgp peer status")
	}
	statej := state.(map[string]any)
	if statej["session_state"] != float64(6) {
		return fmt.Errorf("%s is not established. state is %d", node, statej["session_state"])
	}
	return nil
}
