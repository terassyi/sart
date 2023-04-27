package speaker

import (
	"context"
	"fmt"
)

var mockSpeakerStore map[string]*Mock = make(map[string]*Mock)

type Mock struct {
	endpointAddr string
	endpointPort uint32
	asn          uint32
	routerId     string
	peerMap      map[string]PeerInfo
	pathMap      map[string]PathInfo
}

func newMockSpeaker(endpointAddr string, endpointPort uint32) *Mock {
	key := fmt.Sprintf("%s:%d", endpointAddr, endpointPort)
	speaker, ok := mockSpeakerStore[key]
	if !ok {
		fmt.Printf("Store mock speaker: %s\n", key)
		speaker := &Mock{
			endpointAddr: endpointAddr,
			endpointPort: endpointPort,
			peerMap:      make(map[string]PeerInfo),
			pathMap:      make(map[string]PathInfo),
		}
		mockSpeakerStore[key] = speaker
		return speaker
	}
	fmt.Printf("Get mock speaker: %s: %v\n", key, speaker)
	return speaker
}

func ClearMockSpeakerStore() {
	mockSpeakerStore = make(map[string]*Mock)
}

func (s *Mock) HealthCheck(ctx context.Context) error {
	return nil
}

func (s *Mock) GetInfo(ctx context.Context) (*SpeakerInfo, error) {
	return &SpeakerInfo{
		Asn:      s.asn,
		RouterId: s.routerId,
	}, nil
}

func (s *Mock) SetInfo(ctx context.Context, info SpeakerInfo) error {
	s.asn = info.Asn
	s.routerId = info.RouterId
	return nil
}

func (s *Mock) GetPeer(ctx context.Context, peer string) (*PeerInfo, error) {
	p, ok := s.peerMap[peer]
	if !ok {
		return nil, fmt.Errorf("peer not found")
	}
	return &p, nil
}

func (s *Mock) AddPeer(ctx context.Context, peer PeerInfo) error {
	s.peerMap[peer.PeerRouterId] = peer
	return nil
}

func (s *Mock) DeletePeer(ctx context.Context, peer string) error {
	delete(s.peerMap, peer)
	return nil
}

func (s *Mock) AddPath(ctx context.Context, path PathInfo) error {
	s.pathMap[path.Prefix] = path
	return nil
}

func (s *Mock) DeletePath(ctx context.Context, path PathInfo) error {
	delete(s.pathMap, path.Prefix)
	return nil
}
