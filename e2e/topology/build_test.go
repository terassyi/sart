package topology

import (
	"bytes"
	"context"
	"fmt"
	"os/exec"
	"strings"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	"github.com/terassyi/sart/e2e/container"
)

func TestNode_create(t *testing.T) {
	for _, tt := range []struct {
		name  string
		node  Node
		peers []Peer
	}{
		{
			name: "sart_1",
			node: Node{
				Name: "test_node_create_sart_1",
				Interfaces: []Interface{
					{
						Name:     "iface0",
						PeerName: "test_node_create_peer_1",
						Address:  "192.168.0.2",
					},
				},
				Image:        "sart:dev",
				Privileged:   true,
				InitCommands: []string{"tail", "-f", "/dev/null"},
				Config:       "",
				Commands:     []string{},
			},
			peers: []Peer{
				{
					Name: "test_node_create_peer_1",
					Interfaces: []struct {
						NodeName  string
						Interface Interface
					}{},
					Cidr: "192.168.0.0/24",
				},
			},
		},
		{
			name: "sart_2",
			node: Node{
				Name: "test_node_create_sart_2",
				Interfaces: []Interface{
					{
						Name:     "iface0",
						PeerName: "test_node_create_peer_1",
						Address:  "192.168.0.2",
					},
					{
						Name:     "iface1",
						PeerName: "test_node_create_peer_2",
						Address:  "192.168.1.100",
					},
				},
				Image:        "sart:dev",
				Privileged:   true,
				InitCommands: []string{"tail", "-f", "/dev/null"},
				Config:       "",
				Commands:     []string{},
			},
			peers: []Peer{
				{
					Name: "test_node_create_peer_1",
					Interfaces: []struct {
						NodeName  string
						Interface Interface
					}{},
					Cidr: "192.168.0.0/24",
				},
				{
					Name: "test_node_create_peer_2",
					Interfaces: []struct {
						NodeName  string
						Interface Interface
					}{},
					Cidr: "192.168.1.0/24",
				},
			},
		},
	} {
		tt := tt
		t.Run(tt.name, func(t *testing.T) {
			ctx := context.Background()
			defer func() {
				if err := container.RemoveDockerContainer(ctx, tt.node.Name); err != nil {
					t.Fatal(err)
				}
				for _, peer := range tt.peers {
					if err := container.RemoveDockerNetwork(ctx, peer.Name); err != nil {
						t.Fatal(err)
					}
				}
			}()
			for _, peer := range tt.peers {
				err := container.CreateDockerNetwork(ctx, peer.Name, peer.Cidr)
				require.NoError(t, err)
			}
			err := tt.node.create(ctx)
			require.NoError(t, err)

			for i := 0; i < 10; i++ {
				cmd := exec.Command("docker", "inspect", tt.node.Name, "--format", "{{.State.Status}}")
				out := new(bytes.Buffer)
				cmd.Stdout = out
				err = cmd.Run()
				require.NoError(t, err)
				status := out.String()
				if status == "" {
					time.Sleep(100 * time.Millisecond)
					continue
				}
				assert.Equal(t, "running\n", status)
			}
			for _, iface := range tt.node.Interfaces {
				cmd := exec.Command("docker", "inspect", tt.node.Name, "--format", fmt.Sprintf("{{.NetworkSettings.Networks.%s.IPAddress}}", iface.PeerName))
				out := new(bytes.Buffer)
				cmd.Stdout = out
				err = cmd.Run()
				require.NoError(t, err)
				address := strings.Split(out.String(), "\n")[0]
				assert.Equal(t, iface.Address, address)
			}
		})
	}
}

func TestPeer_create(t *testing.T) {
	t.Parallel()
	for _, tt := range []struct {
		name string
		peer Peer
	}{
		{
			name: "test-peer-network-1",
			peer: Peer{
				Name: "test-peer-network-1",
				Interfaces: []struct {
					NodeName  string
					Interface Interface
				}{},
				Cidr: "10.0.0.0/16",
			},
		},
		{
			name: "test-peer-network-2",
			peer: Peer{
				Name: "test-peer-network-2",
				Interfaces: []struct {
					NodeName  string
					Interface Interface
				}{},
				Cidr: "192.168.0.0/24",
			},
		},
	} {
		tt := tt
		t.Run(tt.name, func(t *testing.T) {
			ctx := context.Background()
			defer container.RemoveDockerNetwork(ctx, tt.peer.Name)
			err := tt.peer.create(ctx)
			require.NoError(t, err)
			for i := 0; i < 10; i++ {
				cmd := exec.Command("docker", "network", "inspect", tt.peer.Name, "--format", "{{.IPAM.Config.Subnet}}")
				out := new(bytes.Buffer)
				cmd.Stdout = out
				cidr := out.String()
				if cidr == "" {
					time.Sleep(100 * time.Millisecond)
					continue
				}
				assert.Equal(t, tt.peer.Cidr, cidr)
			}
		})
	}
}
