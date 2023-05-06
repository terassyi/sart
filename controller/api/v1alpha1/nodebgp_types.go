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

// NodeBGPSpec defines the desired state of NodeBGP
type NodeBGPSpec struct {
	// INSERT ADDITIONAL SPEC FIELDS - desired state of cluster
	// Important: Run "make" to regenerate code after modifying this file

	// +kubebuilder:validation:Required
	// +kubebuilder:validation:Minimum=0
	// +kubebuilder:validation:Maximum=4294967295
	Asn uint32 `json:"asn"`

	RouterId string `json:"routerId"`

	Endpoint string `json:"endpoint,omitempty"`

	Peers []Peer `json:"peers,omitempty"`
}

// NodeBGPStatus defines the observed state of NodeBGP
type NodeBGPStatus string

var (
	NodeBGPStatusAvailable   = NodeBGPStatus("Available")
	NodeBGPStatusUnavailable = NodeBGPStatus("Unavailable")
)

//+kubebuilder:object:root=true
//+kubebuilder:subresource:status
// +kubebuilder:printcolumn:name="ASN",type="integer",JSONPath=".spec.asn",description="AS Number"
// +kubebuilder:printcolumn:name="RouterId",type="string",JSONPath=".spec.routerId",description="Router Id"
// +kubebuilder:printcolumn:name="Status",type="string",JSONPath=".status"
// +kubebuilder:printcolumn:name="Age",type="date",JSONPath=".metadata.creationTimestamp"

// NodeBGP is the Schema for the nodebgps API
type NodeBGP struct {
	metav1.TypeMeta   `json:",inline"`
	metav1.ObjectMeta `json:"metadata,omitempty"`

	Spec   NodeBGPSpec   `json:"spec,omitempty"`
	Status NodeBGPStatus `json:"status,omitempty"`
}

//+kubebuilder:object:root=true

// NodeBGPList contains a list of NodeBGP
type NodeBGPList struct {
	metav1.TypeMeta `json:",inline"`
	metav1.ListMeta `json:"metadata,omitempty"`
	Items           []NodeBGP `json:"items"`
}

func init() {
	SchemeBuilder.Register(&NodeBGP{}, &NodeBGPList{})
}
