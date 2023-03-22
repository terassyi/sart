package speaker

import (
	"context"
	"fmt"

	"github.com/terassyi/sart/controller/pkg/proto"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
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

func (s *sartSpeaker) connect() (proto.BgpApiClient, error) {
	conn, err := grpc.Dial(fmt.Sprintf("%s:%d", s.endpointAddr, s.endpointPort), grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return nil, err
	}
	return proto.NewBgpApiClient(conn), nil
}
