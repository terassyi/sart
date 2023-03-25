/*
Copyright 2023.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

package v1alpha1

import (
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
)

// EDIT THIS FILE!  THIS IS SCAFFOLDING FOR YOU TO OWN!
// NOTE: json tags are required.  Any new fields you add must have json tags for the fields to be serialized.

// BGPPeerSpec defines the desired state of BGPPeer
type BGPPeerSpec struct {
	// INSERT ADDITIONAL SPEC FIELDS - desired state of cluster
	// Important: Run "make" to regenerate code after modifying this file

	// +kubebuilder:validation:Required
	// +kubebuilder:validation:Minimum=0
	// +kubebuilder:validation:Maximum=4294967295
	PeerAsn uint32 `json:"peerAsn"`

	PeerRouterId string `json:"peerRouterId"`

	// +kubebuilder:validation:Required
	// +kubebuilder:validation:Minimum=0
	// +kubebuilder:validation:Maximum=4294967295
	LocalAsn uint32 `json:"localAsn,omitempty"`

	LocalRouterId string `json:"localRouterId,omitempty"`

	Node string `json:"node,omitempty"`

	// +kubebuilder:default:={afi:ipv4,safi:unicast}
	Family AddressFamily `json:"family,omitempty"`

	Advertisements []Advertisement `json:"advertisements,omitempty"`
}

// BGPPeerStatus defines the observed state of BGPPeer

//+kubebuilder:validation:Enum=Idle;Connect;Active;OpenSent;OpenConfirm;Established

type BGPPeerStatus string

var (
	BGPPeerStatusIdle        = BGPPeerStatus("Idle")
	BGPPeerStatusConnect     = BGPPeerStatus("Connect")
	BGPPeerStatusActive      = BGPPeerStatus("Active")
	BGPPeerStatusOpenSent    = BGPPeerStatus("OpenSent")
	BGPPeerStatusOpenConfirm = BGPPeerStatus("OpenConfirm")
	BGPPeerStatusEstablished = BGPPeerStatus("Established")
)

//+kubebuilder:object:root=true
//+kubebuilder:subresource:status
// +kubebuilder:printcolumn:name="Node",type="string",JSONPath=".spec.node",description="Local speaker"
// +kubebuilder:printcolumn:name="PeerASN",type="integer",JSONPath=".spec.peerAsn",description="AS Number"
// +kubebuilder:printcolumn:name="PeerRouterId",type="string",JSONPath=".spec.peerRouterId",description="Router Id"
// +kubebuilder:printcolumn:name="Status",type="string",JSONPath=".status"
// +kubebuilder:printcolumn:name="Age",type="date",JSONPath=".metadata.creationTimestamp"

// BGPPeer is the Schema for the bgppeers API
type BGPPeer struct {
	metav1.TypeMeta   `json:",inline"`
	metav1.ObjectMeta `json:"metadata,omitempty"`

	Spec   BGPPeerSpec   `json:"spec,omitempty"`
	Status BGPPeerStatus `json:"status,omitempty"`
}

//+kubebuilder:object:root=true

// BGPPeerList contains a list of BGPPeer
type BGPPeerList struct {
	metav1.TypeMeta `json:",inline"`
	metav1.ListMeta `json:"metadata,omitempty"`
	Items           []BGPPeer `json:"items"`
}

func init() {
	SchemeBuilder.Register(&BGPPeer{}, &BGPPeerList{})
}

type AddressFamily struct {
	//+kubebuilder:validation:Enum=ipv4;ipv6
	//+kubebuilder:default:=ipv4
	Afi string `json:"afi"`

	//+kubebuilder:validation:Enum=unicast;multicast
	//+kubebuilder:default:=unicast
	Safi string `json:"safi"`
}

type Peer struct {
	Name      string        `json:"name"`
	Namespace string        `json:"namespace"`
	Asn       uint32        `json:"asn"`
	RouterId  string        `json:"routerId"`
	Status    BGPPeerStatus `json:"status"`
}

func (p *Peer) DeepCopy() *Peer {
	return &Peer{
		Name:      p.Name,
		Namespace: p.Namespace,
		Asn:       p.Asn,
		RouterId:  p.RouterId,
		Status:    p.Status,
	}
}
