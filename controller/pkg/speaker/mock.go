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
	peerMap      map[string]*PeerInfo
	pathMap      map[string]*PathInfo
}

func newMockSpeaker(endpointAddr string, endpointPort uint32) *Mock {
	key := fmt.Sprintf("%s:%d", endpointAddr, endpointPort)
	speaker, ok := mockSpeakerStore[key]
	if !ok {
		speaker := &Mock{
			endpointAddr: endpointAddr,
			endpointPort: endpointPort,
			peerMap:      make(map[string]*PeerInfo),
			pathMap:      make(map[string]*PathInfo),
		}
		mockSpeakerStore[key] = speaker
		return speaker
	}
	return speaker
}

func ClearMockSpeakerStore() {
	mockSpeakerStore = make(map[string]*Mock)
}

func GetMockSpeakerPeer(speakerKey, peerKey string) (*PeerInfo, error) {
	speaker, ok := mockSpeakerStore[speakerKey]
	if !ok {
		return nil, fmt.Errorf("speaker not found: %s", speakerKey)
	}
	peer, ok := speaker.peerMap[peerKey]
	if !ok {
		return nil, fmt.Errorf("path not found: %s", peerKey)
	}
	return peer, nil
}

func GetMockSpeakerPath(speakerKey, pathKey string) (*PathInfo, error) {
	speaker, ok := mockSpeakerStore[speakerKey]
	if !ok {
		return nil, fmt.Errorf("speaker not found: %s", speakerKey)
	}
	path, ok := speaker.pathMap[pathKey]
	if !ok {
		return nil, fmt.Errorf("path not found: %s", pathKey)
	}
	return path, nil
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
		return nil, fmt.Errorf("peer not found: %s, %v", peer, s.peerMap)
	}
	return p, nil
}

func (s *Mock) AddPeer(ctx context.Context, peer PeerInfo) error {
	if peer.State == "" {
		peer.State = PeerStateIdle
	}
	s.peerMap[peer.PeerRouterId] = &peer
	return nil
}

func (s *Mock) DeletePeer(ctx context.Context, peer string) error {
	delete(s.peerMap, peer)
	return nil
}

func (s *Mock) AddPath(ctx context.Context, path PathInfo) error {
	s.pathMap[path.Prefix] = &path
	return nil
}

func (s *Mock) DeletePath(ctx context.Context, path PathInfo) error {
	delete(s.pathMap, path.Prefix)
	return nil
}
