package e2e

import (
	"bytes"
	"os/exec"
	"path"
)

const (
	BIN              string = "bin/"
	CONTAINERLAB_BIN string = BIN + "containerlab"
	TOPOLOGY_DIR     string = "topology/"
)

func deployContainerlab(file string) error {
	path := path.Join(TOPOLOGY_DIR, file)
	return exec.Command(CONTAINERLAB_BIN, "-t", path, "deploy").Run()
}

func destroyContainerlab(file string) error {
	path := path.Join(TOPOLOGY_DIR, file)
	return exec.Command(CONTAINERLAB_BIN, "-t", path, "destroy").Run()
}

func execInContainer(target string, input []byte, cmd string, args ...string) ([]byte, error) {
	execArgs := []string{"exec", target, cmd}
	execArgs = append(execArgs, args...)
	c := exec.Command("docker", execArgs...)
	if input != nil {
		c.Stdin = bytes.NewReader(input)
	}
	return c.Output()
}
