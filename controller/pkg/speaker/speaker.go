package speaker

import (
	"context"
	"fmt"
	"time"

	"github.com/terassyi/sart/controller/pkg/proto"
)

type Speaker interface {
	HealthCheck(context.Context) error
	GetInfo(context.Context) (*SpeakerInfo, error)
	SetInfo(context.Context, SpeakerInfo) error
	GetPeer(context.Context, string) (*PeerInfo, error)
	AddPeer(context.Context, PeerInfo) error
	DeletePeer(context.Context, string) error
	AddPath(context.Context, PathInfo) error
	DeletePath(context.Context, PathInfo) error
}

type SpeakerType string

var (
	SpeakerTypeUnknown SpeakerType = SpeakerType("unknown")
	SpeakerTypeSart    SpeakerType = SpeakerType("sart")
)

func New(typ SpeakerType, endpointAddr string, endpointPort uint32) Speaker {
	switch typ {
	case SpeakerTypeSart:
		return newSartSpeaker(endpointAddr, endpointPort)
	default:
		return nil
	}
}

func ParseSpeakerType(t string) (SpeakerType, error) {
	switch t {
	case "sart":
		return SpeakerTypeSart, nil
	default:
		return SpeakerTypeUnknown, fmt.Errorf("invalid speaker type: %s", t)
	}
}

type SpeakerInfo struct {
	Asn      uint32
	RouterId string
}

type PeerInfo struct {
	LocalAsn      uint32
	LocalRouterId string
	PeerAsn       uint32
	PeerRouterId  string
	Protocol      string
	State         PeerState
	Uptime        time.Time
}

func peerFromProtoPeer(p *proto.Peer) (*PeerInfo, error) {
	return &PeerInfo{
		LocalAsn:      0,
		LocalRouterId: "",
		PeerAsn:       p.GetAsn(),
		PeerRouterId:  p.GetAddress(),
		Protocol:      familyToProtocol(p.GetFamilies()[0]), // TODO: handle multiple families
		State:         peerStateFromProtoState(p.GetState()),
		Uptime:        time.Time{},
	}, nil
}

func protocolToFamily(protocol string) (*proto.AddressFamily, error) {
	switch protocol {
	case "ipv4", "Ipv4", "IPv4":
		return &proto.AddressFamily{Afi: proto.AddressFamily_AFI_IP4, Safi: proto.AddressFamily_SAFI_UNICAST}, nil
	case "ipv6", "Ipv6", "IPv6":
		return &proto.AddressFamily{Afi: proto.AddressFamily_AFI_IP6, Safi: proto.AddressFamily_SAFI_UNICAST}, nil
	default:
		return nil, fmt.Errorf("invalid protocol: %s", protocol)
	}
}

func familyToProtocol(family *proto.AddressFamily) string {
	switch family.Afi {
	case proto.AddressFamily_AFI_IP4:
		return "ipv4"
	case proto.AddressFamily_AFI_IP6:
		return "ipv6"
	default:
		return "unknown"
	}
}

type PeerState string

var (
	PeerStateIdle        = PeerState("Idle")
	PeerStateConnect     = PeerState("Connect")
	PeerStateActive      = PeerState("Active")
	PeerStateOpenSent    = PeerState("OpenSent")
	PeerStateOpenConfirm = PeerState("OpenConfirm")
	PeerStateEstablished = PeerState("Established")
)

func peerStateFromProtoState(state proto.Peer_State) PeerState {
	switch state {
	case proto.Peer_IDLE:
		return PeerStateIdle
	case proto.Peer_CONNECT:
		return PeerStateConnect
	case proto.Peer_ACTIVE:
		return PeerStateActive
	case proto.Peer_OPEN_SENT:
		return PeerStateOpenSent
	case proto.Peer_OPEN_CONFIRM:
		return PeerStateOpenConfirm
	case proto.Peer_ESTABLISHED:
		return PeerStateEstablished
	default:
		return PeerStateIdle
	}
}

type PathInfo struct {
	Prefix    string
	Protocol  string
	Origin    string
	LocalPref uint32
}

func parseOrigin(origin string) uint32 {
	switch origin {
	case "igp":
		return 0
	case "ebp":
		return 1
	default:
		return 2
	}
}
