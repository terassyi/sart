package container

import (
	"bytes"
	"context"
	"fmt"
	"net/netip"
	"os/exec"
	"strings"
)

func CreateDockerNetwork(ctx context.Context, name, cidr string) error {
	cmd := exec.CommandContext(ctx, "docker", "network", "create", "--subnet", cidr, name)
	fmt.Println(cmd.Args)
	e := new(bytes.Buffer)
	cmd.Stderr = e
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("%s", e.String())
	}
	return nil
}

func RemoveDockerNetwork(ctx context.Context, name string) error {
	cmd := exec.CommandContext(ctx, "docker", "network", "rm", name)
	fmt.Println(cmd.Args)
	e := new(bytes.Buffer)
	cmd.Stderr = e
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("%s", e.String())
	}
	return nil
}

func CreateDockerContainer(ctx context.Context, name, image, network, address string, privileged bool, volumes []string, args []string, commands []string) error {
	a := []string{"run", "--rm", "--name", name, "-d", "--network", network}
	if address != "" {
		a = append(a, []string{"--ip", address}...)
	}
	if privileged {
		a = append(a, "--privileged")
	}
	if len(volumes) > 0 {
		a = append(a, "--volume")
		a = append(a, volumes...)
	}
	a = append(a, image)
	cmd := exec.CommandContext(ctx, "docker", append(a, args...)...)
	fmt.Println(cmd.Args)
	e := new(bytes.Buffer)
	cmd.Stderr = e
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("%s\n%v", e.String(), err)
	}
	for _, c := range commands {
		a := []string{"exec", "-d", name}
		cmd := exec.CommandContext(ctx, "docker", append(a, strings.Split(c, " ")...)...)
		fmt.Println(cmd.Args)
		e := new(bytes.Buffer)
		cmd.Stderr = e
		if err := cmd.Run(); err != nil {
			return fmt.Errorf("%s", e.String())
		}
	}
	return nil
}

func RemoveDockerContainer(ctx context.Context, name string) error {
	cmd := exec.CommandContext(ctx, "docker", "rm", name, "-f")
	fmt.Println(cmd.Args)
	e := new(bytes.Buffer)
	cmd.Stderr = e
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("%s", e.String())
	}
	return nil
}

func ConnectNetwork(ctx context.Context, network, address, container string) error {
	if address == "" {
		cmd := exec.CommandContext(ctx, "docker", "network", "connect", network, container)
		return cmd.Run()
	}
	addr, err := netip.ParseAddr(address)
	if err != nil {
		return err
	}
	var cmd *exec.Cmd
	if addr.Is4() {
		cmd = exec.CommandContext(ctx, "docker", "network", "connect", "--ip", address, network, container)
	} else {
		cmd = exec.CommandContext(ctx, "docker", "network", "connect", "--ip6", address, network, container)
	}
	fmt.Println(cmd.Args)
	e := new(bytes.Buffer)
	cmd.Stderr = e
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("%s", e.String())
	}
	return nil
}

func RunCommandInContainer(ctx context.Context, target string, detach bool, args []string) error {
	a := []string{"exec"}
	if detach {
		a = append(a, "-d")
	}
	a = append(a, target)
	cmd := exec.CommandContext(ctx, "docker", append(a, args...)...)
	e := new(bytes.Buffer)
	cmd.Stderr = e
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("%s", e.String())
	}
	return nil
}

func RunCommandInContainerWithOutput(ctx context.Context, target string, args []string) ([]byte, error) {
	a := []string{"exec", target}
	cmd := exec.CommandContext(ctx, "docker", append(a, args...)...)
	o := new(bytes.Buffer)
	e := new(bytes.Buffer)
	cmd.Stdout = o
	cmd.Stderr = e
	if err := cmd.Run(); err != nil {
		return nil, fmt.Errorf("%s", e.String())
	}
	return o.Bytes(), nil
}

func BuildContainer(ctx context.Context, name, tag, file, path string) error {
	a := func() []string {
		if file == "" {
			return []string{"build", "-t", fmt.Sprintf("%s:%s", name, tag), path}
		}
		return []string{"build", "-t", fmt.Sprintf("%s:%s", name, tag), "-f", file, path}
	}()
	cmd := exec.CommandContext(ctx, "docker", a...)
	e := new(bytes.Buffer)
	cmd.Stderr = e
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("%s", e.String())
	}
	return nil
}
