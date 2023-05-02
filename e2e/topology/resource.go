package topology

import (
	"bytes"
	"context"
	"fmt"
	"os/exec"
	"path/filepath"
	"time"

	"github.com/terassyi/sart/e2e/container"
)

type Topology struct {
	Name  string
	Nodes []Node
	Peers []Peer
}

type Peer struct {
	Name       string
	Interfaces []struct {
		NodeName  string
		Interface Interface
	}
	Cidr string
}

func (p *Peer) create(ctx context.Context) error {
	if err := container.CreateDockerNetwork(ctx, p.Name, p.Cidr); err != nil {
		return err
	}
	return nil
}

func (p *Peer) remove(ctx context.Context) error {
	return container.RemoveDockerNetwork(ctx, p.Name)
}

type Node struct {
	Name         string
	Interfaces   []Interface
	Image        string
	Volume       []Volume
	Privileged   bool
	InitCommands []string
	Config       string
	Commands     []string
}

type Volume struct {
	Source      string
	Destination string
}

func (n *Node) create(ctx context.Context) error {
	if len(n.Interfaces) == 0 {
		return fmt.Errorf("Interfaces must be specified at least one network")
	}
	volumes := []string{}
	for _, v := range n.Volume {
		abs, err := filepath.Abs(v.Source)
		if err != nil {
			return err
		}
		volumes = append(volumes, fmt.Sprintf("%s:%s", abs, v.Destination))
	}
	if err := container.CreateDockerContainer(ctx, n.Name, n.Image, n.Interfaces[0].PeerName, n.Interfaces[0].Address, n.Privileged, volumes, n.InitCommands, n.Commands); err != nil {
		return err
	}

	o := new(bytes.Buffer)
	e := new(bytes.Buffer)
	cctx, _ := context.WithTimeout(context.Background(), time.Minute)
	showLog := exec.CommandContext(cctx, "docker", "logs", "-f", "node1")
	showLog.Stdout = o
	showLog.Stderr = e
	err := showLog.Run()
	fmt.Println(o.String())
	fmt.Println(e.String())
	if err != nil {
		return err
	}

	if len(n.Interfaces) > 1 {
		for i := 1; i < len(n.Interfaces); i++ {
			if err := container.ConnectNetwork(ctx, n.Interfaces[i].PeerName, n.Interfaces[i].Address, n.Name); err != nil {
				return err
			}
		}
	}
	return nil
}

func (n *Node) remove(ctx context.Context) error {
	return container.RemoveDockerContainer(ctx, n.Name)
}

type Interface struct {
	Name     string
	PeerName string
	Address  string
}
