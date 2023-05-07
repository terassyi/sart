package sartd

type GoBGPNeighbor struct {
	State GoBGPState `json:"state"`
}

type GoBGPState struct {
	NeighborAddress string `json:"neighbor_address"`
	PeerASN         int    `json:"peer_asn"`
	SessionState    int    `json:"session_state"`
}

type GoBGPPath struct {
	Nlri   Nlri   `json:"nlri"`
	Best   bool   `json:"best"`
	Source string `json:"source-id"`
}

type Nlri struct {
	Prefix string `json:"prefix"`
}
