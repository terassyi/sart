package test

import (
	"bytes"
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

	cmd.Stdout = outBuf
	cmd.Stderr = errBuf

	err := cmd.Run()

	return outBuf.Bytes(), errBuf.Bytes(), cmd.Process, err
}
