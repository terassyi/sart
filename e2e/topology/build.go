package topology

import (
	"context"
)

func (t *Topology) Build(ctx context.Context) error {

	for _, peer := range t.Peers {
		if err := peer.create(ctx); err != nil {
			return err
		}
	}

	for _, node := range t.Nodes {
		if err := node.create(ctx); err != nil {
			return err
		}
	}
	return nil
}

func (t *Topology) Remove(ctx context.Context) error {
	for _, node := range t.Nodes {
		if err := node.remove(ctx); err != nil {
			return err
		}
	}

	for _, peer := range t.Peers {
		if err := peer.remove(ctx); err != nil {
			return err
		}
	}

	return nil

}
