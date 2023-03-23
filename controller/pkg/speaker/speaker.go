package speaker

import (
	"context"
	"fmt"
)

type Speaker interface {
	HealthCheck(context.Context) error
	GetInfo(context.Context) (*SpeakerInfo, error)
	SetInfo(context.Context, SpeakerInfo) error
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
