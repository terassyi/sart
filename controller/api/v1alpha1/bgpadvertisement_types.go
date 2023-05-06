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
	"github.com/terassyi/sart/controller/pkg/speaker"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
)

// EDIT THIS FILE!  THIS IS SCAFFOLDING FOR YOU TO OWN!
// NOTE: json tags are required.  Any new fields you add must have json tags for the fields to be serialized.

// BGPAdvertisementSpec defines the desired state of BGPAdvertisement
type BGPAdvertisementSpec struct {
	// INSERT ADDITIONAL SPEC FIELDS - desired state of cluster
	// Important: Run "make" to regenerate code after modifying this file

	Network string `json:"network"`

	// +kubebuilder:validation:Enum=service;pod
	// +kubebuilder:default=pod
	Type string `json:"type"`

	//+kubebuilder:validation:Enum=ipv4;ipv6
	//+kubebuilder:default:=ipv4
	Protocol string `json:"protocol,omitempty"`

	//+kubebuilder:validation:Enum=egp;igp;incomplete
	//+kubebuilder:default:=igp
	Origin string `json:"origin,omitempty"`

	LocalPref uint32   `json:"localPref,omitempty"`
	Nodes     []string `json:"nodes,omitempty"` // list of node names
}

// BGPAdvertisementStatus defines the observed state of BGPAdvertisement
type BGPAdvertisementStatus struct {
	// +kubebuilder:default:=Advertising
	Condition BGPAdvertisementCondition `json:"condition"`
	// +kubebuilder:default:=0
	Advertising uint32 `json:"advertising,omitempty"`
	// +kubebuilder:default:=0
	Advertised uint32 `json:"advertised,omitempty"`
}

type BGPAdvertisementCondition string

var (
	BGPAdvertisementConditionAdvertising = BGPAdvertisementCondition("Advertising")
	BGPAdvertisementConditionAdvertised  = BGPAdvertisementCondition("Advertised")
	BGPAdvertisementConditionUpdated     = BGPAdvertisementCondition("Updated")
	BGPAdvertisementConditionWithdrawn   = BGPAdvertisementCondition("Withdrawn")
)

//+kubebuilder:object:root=true
//+kubebuilder:subresource:status
// +kubebuilder:printcolumn:name="Network",type="string",JSONPath=".spec.network"
// +kubebuilder:printcolumn:name="Type",type="string",JSONPath=".spec.serviceType"
// +kubebuilder:printcolumn:name="Status",type="string",JSONPath=".status.condition"
// +kubebuilder:printcolumn:name="Advertising",type="integer",JSONPath=".status.advertising"
// +kubebuilder:printcolumn:name="Advertised",type="integer",JSONPath=".status.advertised"
// +kubebuilder:printcolumn:name="Age",type="date",JSONPath=".metadata.creationTimestamp"

// BGPAdvertisement is the Schema for the bgpadvertisements API
type BGPAdvertisement struct {
	metav1.TypeMeta   `json:",inline"`
	metav1.ObjectMeta `json:"metadata,omitempty"`

	Spec BGPAdvertisementSpec `json:"spec,omitempty"`
	// +kubebuilder:default:={condition:Advertising}
	Status BGPAdvertisementStatus `json:"status,omitempty"`
}

//+kubebuilder:object:root=true

// BGPAdvertisementList contains a list of BGPAdvertisement
type BGPAdvertisementList struct {
	metav1.TypeMeta `json:",inline"`
	metav1.ListMeta `json:"metadata,omitempty"`
	Items           []BGPAdvertisement `json:"items"`
}

func init() {
	SchemeBuilder.Register(&BGPAdvertisement{}, &BGPAdvertisementList{})
}

func (a *BGPAdvertisement) ToPathInfo() speaker.PathInfo {
	return speaker.PathInfo{
		Prefix:    a.Spec.Network,
		Protocol:  a.Spec.Protocol,
		Origin:    a.Spec.Origin,
		LocalPref: a.Spec.LocalPref,
	}
}

type Advertisement struct {
	Name      string `json:"name"`
	Namespace string `json:"namespace"`
	Prefix    string `json:"prefix"`
}
