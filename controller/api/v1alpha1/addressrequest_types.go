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

// AddressRequestSpec defines the desired state of AddressRequest
type AddressRequestSpec struct {
	// INSERT ADDITIONAL SPEC FIELDS - desired state of cluster
	// Important: Run "make" to regenerate code after modifying this file

	Kind string `json:"kind"`

	//+kubebuilder:validation:Enum=ipv4;ipv6
	//+kubebuilder:default:=ipv4
	Protocol string `json:"protocol,omitempty"`
}

// AddressRequestStatus defines the observed state of AddressRequest
type AddressRequestStatus struct {
	// INSERT ADDITIONAL STATUS FIELD - define observed state of cluster
	// Important: Run "make" to regenerate code after modifying this file
	Conditions []AddressRequestCondition `json:"conditions,omitempty"`
}

type AddressRequestCondition string

var (
	AddressRequestConditionRequested = AddressRequestCondition("Requested")
	AddressRequestConditionAllocated = AddressRequestCondition("Allocated")
	AddressRequestConditionReleased  = AddressRequestCondition("Released")
	AddressRequestConditionFailed    = AddressRequestCondition("Failed")
)

//+kubebuilder:object:root=true
//+kubebuilder:subresource:status

// AddressRequest is the Schema for the addressrequests API
type AddressRequest struct {
	metav1.TypeMeta   `json:",inline"`
	metav1.ObjectMeta `json:"metadata,omitempty"`

	Spec   AddressRequestSpec   `json:"spec,omitempty"`
	Status AddressRequestStatus `json:"status,omitempty"`
}

//+kubebuilder:object:root=true

// AddressRequestList contains a list of AddressRequest
type AddressRequestList struct {
	metav1.TypeMeta `json:",inline"`
	metav1.ListMeta `json:"metadata,omitempty"`
	Items           []AddressRequest `json:"items"`
}

func init() {
	SchemeBuilder.Register(&AddressRequest{}, &AddressRequestList{})
}
