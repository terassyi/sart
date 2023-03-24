package speaker

import (
	"context"
	"fmt"

	"github.com/terassyi/sart/controller/pkg/proto"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/protobuf/types/known/anypb"
	"sigs.k8s.io/controller-runtime/pkg/log"
)

const (
	SartDefaultEndpointPort uint32 = 5000
)

type sartSpeaker struct {
	endpointAddr string
	endpointPort uint32
}

func newSartSpeaker(endpointAddr string, endpointPort uint32) *sartSpeaker {
	return &sartSpeaker{
		endpointAddr: endpointAddr,
		endpointPort: endpointPort,
	}
}

func (s *sartSpeaker) HealthCheck(ctx context.Context) error {
	logger := log.FromContext(ctx)
	logger.Info("connect to bgp daemon in node", "address", s.endpointAddr, "port", s.endpointPort)
	conn, err := s.connect()
	if err != nil {
		return err
	}
	logger.Info("health check", "address", s.endpointAddr, "port", s.endpointPort)
	if _, err := conn.Health(ctx, &proto.HealthRequest{}); err != nil {
		return err
	}
	return nil
}

func (s *sartSpeaker) GetInfo(ctx context.Context) (*SpeakerInfo, error) {
	conn, err := s.connect()
	if err != nil {
		return nil, err
	}

	res, err := conn.GetBgpInfo(ctx, &proto.GetBgpInfoRequest{})
	if err != nil {
		return nil, err
	}
	return &SpeakerInfo{
		Asn:      res.GetInfo().GetAsn(),
		RouterId: res.GetInfo().GetRouterId(),
	}, nil
}

func (s *sartSpeaker) SetInfo(ctx context.Context, info SpeakerInfo) error {
	conn, err := s.connect()
	if err != nil {
		return err
	}
	if _, err := conn.SetAS(ctx, &proto.SetASRequest{Asn: info.Asn}); err != nil {
		return err
	}
	if _, err := conn.SetRouterId(ctx, &proto.SetRouterIdRequest{RouterId: info.RouterId}); err != nil {
		return err
	}

	return nil
}

func (s *sartSpeaker) AddPeer(ctx context.Context, peer PeerInfo) error {
	conn, err := s.connect()
	if err != nil {
		return err
	}
	family, err := protocolToFamily(peer.Protocol)
	if err != nil {
		return err
	}

	if _, err := conn.AddPeer(ctx, &proto.AddPeerRequest{
		Peer: &proto.Peer{
			Asn:      peer.PeerAsn,
			Address:  peer.PeerRouterId,
			RouterId: peer.PeerRouterId,
			Families: []*proto.AddressFamily{
				family,
			},
		},
	}); err != nil {
		return err
	}
	return nil
}

func (s *sartSpeaker) DeletePeer(ctx context.Context, addr string) error {
	conn, err := s.connect()
	if err != nil {
		return err
	}

	if _, err := conn.DeletePeer(ctx, &proto.DeletePeerRequest{
		Addr: addr,
	}); err != nil {
		return err
	}
	return nil
}

func (s *sartSpeaker) GetPeer(ctx context.Context, addr string) (*PeerInfo, error) {
	conn, err := s.connect()
	if err != nil {
		return nil, err
	}

	res, err := conn.GetNeighbor(ctx, &proto.GetNeighborRequest{
		Addr: addr,
	})
	if err != nil {
		return nil, err
	}
	return peerFromProtoPeer(res.GetPeer())
}

func (s *sartSpeaker) AddPath(ctx context.Context, path PathInfo) error {
	conn, err := s.connect()
	if err != nil {
		return err
	}

	family, err := protocolToFamily(path.Protocol)
	if err != nil {
		return err
	}

	// build attributes
	attrs := make([]*anypb.Any, 0)
	if path.Origin != "" {
		anyAttr, err := anypb.New(&proto.OriginAttribute{
			Value: parseOrigin(path.Origin),
		})
		if err != nil {
			return err
		}
		attrs = append(attrs, anyAttr)
	}
	if path.LocalPref != 0 {
		anyAttr, err := anypb.New(&proto.LocalPrefAttribute{
			Value: path.LocalPref,
		})
		if err != nil {
			return err
		}
		attrs = append(attrs, anyAttr)
	}

	if _, err := conn.AddPath(ctx, &proto.AddPathRequest{
		Family:     family,
		Prefixes:   []string{path.Prefix},
		Attributes: attrs,
	}); err != nil {
		return err
	}
	return nil
}

func (s *sartSpeaker) DeletePath(ctx context.Context, path PathInfo) error {
	conn, err := s.connect()
	if err != nil {
		return err
	}
	family, err := protocolToFamily(path.Protocol)
	if err != nil {
		return err
	}
	if _, err := conn.DeletePath(ctx, &proto.DeletePathRequest{
		Family:   family,
		Prefixes: []string{path.Prefix},
	}); err != nil {
		return err
	}
	return nil
}

func (s *sartSpeaker) connect() (proto.BgpApiClient, error) {
	conn, err := grpc.Dial(fmt.Sprintf("%s:%d", s.endpointAddr, s.endpointPort), grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return nil, err
	}
	return proto.NewBgpApiClient(conn), nil
}
