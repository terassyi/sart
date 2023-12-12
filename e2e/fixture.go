package e2e

import (
	"bytes"
	"os"
	"os/exec"
	"path"

	"k8s.io/client-go/dynamic"
	"sigs.k8s.io/controller-runtime/pkg/client/config"
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

func kubectl(input []byte, args ...string) ([]byte, error) {
	homeDir := os.Getenv("HOME")
	kubeConfigPath := path.Join(homeDir, ".kube/config")
	cmdArgs := []string{"--kubeconfig", kubeConfigPath}
	cmdArgs = append(cmdArgs, args...)
	c := exec.Command(path.Join(BIN, "kubectl"), cmdArgs...)
	if input != nil {
		c.Stdin = bytes.NewReader(input)
	}
	c.Stderr = os.Stdout
	return c.Output()
}

func kubectlExec(name, namespace string, input []byte, args ...string) ([]byte, error) {
	cmdArgs := []string{"-n", namespace, "exec", name, "--"}
	cmdArgs = append(cmdArgs, args...)
	return kubectl(input, cmdArgs...)
}

func getDynamicClient() (*dynamic.DynamicClient, error) {
	conf, err := config.GetConfig()
	if err != nil {
		return nil, err
	}

	return dynamic.NewForConfig(conf)
}
